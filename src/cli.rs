use std::{collections::HashMap, path::PathBuf, time::Duration};

use anyhow::{Result, bail};
use clap::Parser;

use crate::{
    app::AppConfig,
    service::{ClientTlsConfig, TemporalConnectionConfig},
};

/// Terminal dashboard and control plane for Temporal.
#[derive(Debug, Clone, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Temporal frontend address. A scheme is optional.
    #[arg(long, env = "TEMPORAL_ADDRESS", default_value = "127.0.0.1:7233")]
    pub address: String,

    /// Namespace selected at startup.
    #[arg(
        long,
        short = 'n',
        env = "TEMPORAL_NAMESPACE",
        default_value = "default"
    )]
    pub namespace: String,

    /// Temporal Cloud API key.
    #[arg(long, env = "TEMPORAL_API_KEY", hide_env_values = true)]
    pub api_key: Option<String>,

    /// Enable TLS even when the address does not use an https scheme.
    #[arg(long, env = "TEMPORAL_TLS")]
    pub tls: bool,

    /// PEM-encoded server CA certificate.
    #[arg(long, env = "TEMPORAL_TLS_CA")]
    pub tls_ca: Option<PathBuf>,

    /// PEM-encoded mTLS client certificate.
    #[arg(long, env = "TEMPORAL_TLS_CERT")]
    pub tls_cert: Option<PathBuf>,

    /// PEM-encoded mTLS client private key.
    #[arg(long, env = "TEMPORAL_TLS_KEY", hide_env_values = true)]
    pub tls_key: Option<PathBuf>,

    /// TLS server name override.
    #[arg(long, env = "TEMPORAL_TLS_SERVER_NAME")]
    pub tls_server_name: Option<String>,

    /// Additional gRPC header in KEY=VALUE form. May be repeated.
    #[arg(long = "header", value_parser = parse_header)]
    pub headers: Vec<(String, String)>,

    /// Initial Temporal visibility query.
    #[arg(long, short = 'q', default_value = "")]
    pub query: String,

    /// Maximum workflows loaded per refresh.
    #[arg(long, default_value_t = 200, value_parser = parse_page_size)]
    pub page_size: usize,

    /// Automatic refresh interval in seconds.
    #[arg(long, default_value_t = 5, value_parser = parse_refresh_seconds)]
    pub refresh_seconds: u64,

    /// Start with automatic refresh disabled.
    #[arg(long)]
    pub no_auto_refresh: bool,

    /// Disable colors while retaining text status labels.
    #[arg(long)]
    pub no_color: bool,
}

impl Cli {
    /// Build and validate Temporal connection configuration.
    ///
    /// # Errors
    ///
    /// Returns an error when only half of an mTLS credential pair is supplied
    /// or when a gRPC header name is repeated.
    pub fn connection_config(&self) -> Result<TemporalConnectionConfig> {
        if self.tls_cert.is_some() != self.tls_key.is_some() {
            bail!("--tls-cert and --tls-key must be supplied together");
        }

        let tls = self.tls
            || self.api_key.is_some()
            || self.tls_ca.is_some()
            || self.tls_cert.is_some()
            || self.address.starts_with("https://");

        let mut headers = HashMap::with_capacity(self.headers.len());
        for (key, value) in &self.headers {
            if headers.insert(key.clone(), value.clone()).is_some() {
                bail!("duplicate gRPC header: {key}");
            }
        }

        Ok(TemporalConnectionConfig {
            address: self.address.clone(),
            api_key: self.api_key.clone(),
            headers,
            tls: tls.then(|| ClientTlsConfig {
                server_ca: self.tls_ca.clone(),
                client_certificate: self.tls_cert.clone(),
                client_private_key: self.tls_key.clone(),
                server_name: self.tls_server_name.clone(),
            }),
        })
    }

    /// Build application configuration.
    #[must_use]
    pub fn app_config(&self) -> AppConfig {
        AppConfig {
            address: self.address.clone(),
            namespace: self.namespace.clone(),
            query: self.query.clone(),
            page_size: self.page_size,
            refresh_interval: Duration::from_secs(self.refresh_seconds),
            auto_refresh: !self.no_auto_refresh,
            color: !self.no_color,
        }
    }
}

fn parse_header(value: &str) -> Result<(String, String), String> {
    let Some((key, value)) = value.split_once('=') else {
        return Err("headers must use KEY=VALUE syntax".to_string());
    };
    let key = key.trim().to_ascii_lowercase();
    let value = value.trim().to_string();
    if key.is_empty() || value.is_empty() {
        return Err("header key and value must not be empty".to_string());
    }
    if key.ends_with("-bin")
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err("header key must be lowercase ASCII and must not end in -bin".to_string());
    }
    Ok((key, value))
}

fn parse_page_size(value: &str) -> Result<usize, String> {
    parse_bounded(value, 1, 5_000, "page size")
}

fn parse_refresh_seconds(value: &str) -> Result<u64, String> {
    parse_bounded(value, 1, 3_600, "refresh interval")
}

fn parse_bounded<T>(value: &str, minimum: T, maximum: T, label: &str) -> Result<T, String>
where
    T: std::str::FromStr + PartialOrd + Copy + std::fmt::Display,
    T::Err: std::fmt::Display,
{
    let parsed = value
        .parse::<T>()
        .map_err(|error| format!("invalid {label}: {error}"))?;
    if parsed < minimum || parsed > maximum {
        return Err(format!("{label} must be between {minimum} and {maximum}"));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_half_of_mtls_pair() {
        let cli = Cli::try_parse_from(["temporal-tui", "--tls-cert", "client.pem"])
            .expect("CLI should parse before cross-field validation");
        assert!(cli.connection_config().is_err());
    }

    #[test]
    fn api_key_enables_tls() {
        let cli = Cli::try_parse_from(["temporal-tui", "--api-key", "secret"]).unwrap();
        assert!(cli.connection_config().unwrap().tls.is_some());
    }

    #[test]
    fn parses_safe_headers() {
        assert_eq!(
            parse_header("x-owner=temporal-tui").unwrap(),
            ("x-owner".to_string(), "temporal-tui".to_string())
        );
        assert!(parse_header("Bad Header=value").is_err());
        assert!(parse_header("payload-bin=value").is_err());
    }

    #[test]
    fn rejects_unbounded_refresh_and_page_sizes() {
        assert!(Cli::try_parse_from(["temporal-tui", "--page-size", "0"]).is_err());
        assert!(Cli::try_parse_from(["temporal-tui", "--refresh-seconds", "3601"]).is_err());
    }
}
