use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use crate::{
    app::{AppConfig, ProfileSummary, SavedQuery},
    config::{
        ConfigStore, ConnectionProfile, ProfilePayloadCodec, ProfileTls, SavedFilter, SecretSource,
        UserConfig,
    },
    service::{ClientTlsConfig, PayloadCodecConfig, TemporalConnectionConfig},
};

/// Terminal dashboard and control plane for Temporal.
#[derive(Clone, Parser)]
#[command(author, version, about, long_about = None)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent CLI switches map directly to explicit user intent"
)]
pub struct Cli {
    /// Alternate config file. Defaults to the platform user config directory.
    #[arg(long, global = true, env = "TEMPORAL_TUI_CONFIG")]
    pub config: Option<PathBuf>,

    /// Named connection profile.
    #[arg(long, global = true, env = "TEMPORAL_PROFILE")]
    pub profile: Option<String>,

    /// Temporal frontend address. A scheme is optional.
    #[arg(long, env = "TEMPORAL_ADDRESS")]
    pub address: Option<String>,

    /// Namespace selected at startup.
    #[arg(long, short = 'n', env = "TEMPORAL_NAMESPACE")]
    pub namespace: Option<String>,

    /// Temporal Cloud API key. Prefer `profile set-api-key`.
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

    /// Temporal Codec Server base URL; `{namespace}` is expanded in its path.
    #[arg(long, env = "TEMPORAL_CODEC_ENDPOINT")]
    pub codec_endpoint: Option<String>,

    /// Codec Server HTTP header in KEY=VALUE form. May be repeated.
    #[arg(long = "codec-header", value_parser = parse_header)]
    pub codec_headers: Vec<(String, String)>,

    /// Codec Server Authorization header. Prefer the environment variable.
    #[arg(long, env = "TEMPORAL_CODEC_AUTH", hide_env_values = true)]
    pub codec_auth: Option<String>,

    /// Initial Temporal visibility query.
    #[arg(long, short = 'q')]
    pub query: Option<String>,

    /// Workflows loaded per cursor page.
    #[arg(long, value_parser = parse_page_size)]
    pub page_size: Option<usize>,

    /// Automatic refresh interval in seconds.
    #[arg(long, value_parser = parse_refresh_seconds)]
    pub refresh_seconds: Option<u64>,

    /// Start with automatic refresh disabled.
    #[arg(long)]
    pub no_auto_refresh: bool,

    /// Disable colors while retaining text status labels.
    #[arg(long)]
    pub no_color: bool,

    /// Block every Temporal mutation, including signals.
    #[arg(long, env = "TEMPORAL_TUI_READ_ONLY")]
    pub read_only: bool,

    /// Base URL of Temporal Web UI.
    #[arg(long, env = "TEMPORAL_WEB_URL")]
    pub web_ui_url: Option<String>,

    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

/// Non-interactive config administration.
#[derive(Clone, Subcommand)]
#[allow(
    clippy::large_enum_variant,
    reason = "Clap owns this short-lived command model and direct fields keep generated help coherent"
)]
pub enum CliCommand {
    /// Manage connection profiles.
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
    /// Manage saved visibility queries.
    Filter {
        #[command(subcommand)]
        command: FilterCommand,
    },
    /// Print the active config path.
    ConfigPath,
}

#[derive(Clone, Subcommand)]
#[allow(
    clippy::large_enum_variant,
    reason = "the profile create command intentionally exposes all persisted connection settings"
)]
pub enum ProfileCommand {
    /// List configured profiles without resolving secrets.
    List,
    /// Print one redacted profile as TOML.
    Show { name: String },
    /// Create a connection profile.
    Create {
        name: String,
        #[arg(long)]
        address: String,
        #[arg(long, short = 'n', default_value = "default")]
        namespace: String,
        #[arg(long)]
        tls: bool,
        #[arg(long)]
        tls_ca: Option<PathBuf>,
        #[arg(long)]
        tls_cert: Option<PathBuf>,
        #[arg(long)]
        tls_key: Option<PathBuf>,
        #[arg(long)]
        tls_server_name: Option<String>,
        #[arg(long = "header", value_parser = parse_header)]
        headers: Vec<(String, String)>,
        #[arg(long)]
        codec_endpoint: Option<String>,
        #[arg(long = "codec-header", value_parser = parse_header)]
        codec_headers: Vec<(String, String)>,
        #[arg(long)]
        codec_auth_env: Option<String>,
        #[arg(long)]
        api_key_env: Option<String>,
        #[arg(long)]
        web_ui_url: Option<String>,
        #[arg(long)]
        read_only: bool,
        #[arg(long)]
        set_default: bool,
        /// Replace an existing profile with the same name.
        #[arg(long)]
        replace: bool,
    },
    /// Select the default profile.
    SetDefault { name: String },
    /// Read an API key without echo and store it in the OS credential manager.
    SetApiKey {
        name: String,
        /// Read the API key from this environment variable instead of prompting.
        #[arg(long)]
        from_env: Option<String>,
    },
    /// Remove an API key reference and delete its OS credential.
    ClearApiKey {
        name: String,
        #[arg(long)]
        yes: bool,
    },
    /// Remove a profile.
    Remove {
        name: String,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Clone, Subcommand)]
pub enum FilterCommand {
    /// List saved visibility queries.
    List,
    /// Save or replace a visibility query.
    Save {
        name: String,
        query: String,
        #[arg(long)]
        replace: bool,
    },
    /// Remove a saved visibility query.
    Remove {
        name: String,
        #[arg(long)]
        yes: bool,
    },
}

/// Fully merged launch settings.
pub struct LaunchConfig {
    pub connection: TemporalConnectionConfig,
    pub app: AppConfig,
}

impl Cli {
    /// Execute a config subcommand.
    ///
    /// Returns `true` when a subcommand was handled and the TUI must not start.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid input, failed persistence, secret-store
    /// failures, or an explicitly unconfirmed destructive operation.
    pub fn run_config_command(&self, store: &ConfigStore) -> Result<bool> {
        let Some(command) = &self.command else {
            return Ok(false);
        };
        let mut config = store.load()?;
        match command {
            CliCommand::ConfigPath => println!("{}", store.path().display()),
            CliCommand::Profile { command } => {
                run_profile_command(command, store, &mut config)?;
            }
            CliCommand::Filter { command } => {
                run_filter_command(command, store, &mut config)?;
            }
        }
        Ok(true)
    }

    /// Merge defaults, selected profile, environment-backed CLI flags, and
    /// explicit CLI overrides into one launch configuration.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid TLS pairs, duplicate headers, an invalid
    /// profile, an unavailable secret, or malformed Web UI URL.
    pub fn launch_config(&self, store: &ConfigStore, config: &UserConfig) -> Result<LaunchConfig> {
        let profile_name = self
            .profile
            .clone()
            .or_else(|| config.default_profile.clone());

        let (mut connection, profile_namespace, profile_web_url, profile_read_only) =
            if let Some(name) = profile_name.as_deref() {
                let mut resolution_config = config.clone();
                if self.api_key.is_some()
                    && let Some(profile) = resolution_config.profiles.get_mut(name)
                {
                    profile.api_key = None;
                }
                let resolved = store.resolve_profile(&resolution_config, name)?;
                (
                    resolved.connection,
                    resolved.namespace,
                    resolved.web_ui_url,
                    resolved.read_only,
                )
            } else {
                (
                    TemporalConnectionConfig {
                        address: "127.0.0.1:7233".to_string(),
                        api_key: None,
                        headers: HashMap::new(),
                        tls: None,
                        payload_codec: None,
                    },
                    "default".to_string(),
                    Some("http://127.0.0.1:8233".to_string()),
                    false,
                )
            };

        if let Some(address) = &self.address {
            connection.address.clone_from(address);
        }
        if let Some(api_key) = &self.api_key {
            connection.api_key = Some(api_key.clone());
        }
        for (key, value) in &self.headers {
            if connection
                .headers
                .insert(key.clone(), value.clone())
                .is_some()
            {
                bail!("duplicate gRPC header after merging profile and CLI: {key}");
            }
        }
        if let Some(endpoint) = &self.codec_endpoint {
            let headers = connection
                .payload_codec
                .take()
                .map_or_else(HashMap::new, |codec| codec.headers);
            connection.payload_codec = Some(PayloadCodecConfig {
                endpoint: endpoint.clone(),
                headers,
            });
        }
        if !self.codec_headers.is_empty() || self.codec_auth.is_some() {
            let codec = connection.payload_codec.as_mut().context(
                "--codec-header/--codec-auth requires --codec-endpoint or a profile Codec Server",
            )?;
            for (key, value) in &self.codec_headers {
                if codec.headers.insert(key.clone(), value.clone()).is_some() {
                    bail!("duplicate Codec Server header after merging profile and CLI: {key}");
                }
            }
            if let Some(authorization) = &self.codec_auth {
                codec
                    .headers
                    .insert("authorization".to_string(), authorization.clone());
            }
        }

        let mut tls = connection.tls.take().unwrap_or(ClientTlsConfig {
            server_ca: None,
            client_certificate: None,
            client_private_key: None,
            server_name: None,
        });
        if self.tls_ca.is_some() {
            tls.server_ca.clone_from(&self.tls_ca);
        }
        if self.tls_cert.is_some() {
            tls.client_certificate.clone_from(&self.tls_cert);
        }
        if self.tls_key.is_some() {
            tls.client_private_key.clone_from(&self.tls_key);
        }
        if self.tls_server_name.is_some() {
            tls.server_name.clone_from(&self.tls_server_name);
        }
        if tls.client_certificate.is_some() != tls.client_private_key.is_some() {
            bail!("--tls-cert and --tls-key must be supplied together");
        }
        let tls_enabled = self.tls
            || connection.api_key.is_some()
            || tls.server_ca.is_some()
            || tls.client_certificate.is_some()
            || connection.address.starts_with("https://")
            || profile_name
                .as_ref()
                .and_then(|name| config.profiles.get(name))
                .is_some_and(|profile| profile.tls.enabled);
        connection.tls = tls_enabled.then_some(tls);

        let web_ui_url = self.web_ui_url.clone().or(profile_web_url);
        if let Some(web_ui_url) = &web_ui_url {
            let parsed = url::Url::parse(web_ui_url).context("invalid Temporal Web UI URL")?;
            if !matches!(parsed.scheme(), "http" | "https") {
                bail!("Temporal Web UI URL must use http or https");
            }
        }
        let mut saved_queries = config
            .filters
            .iter()
            .map(|(name, filter)| SavedQuery {
                name: name.clone(),
                query: filter.query.clone(),
            })
            .collect::<Vec<_>>();
        saved_queries.sort_by(|left, right| left.name.cmp(&right.name));
        let codec_enabled = connection.payload_codec.is_some();
        let profiles = config
            .profiles
            .iter()
            .map(|(name, profile)| ProfileSummary {
                name: name.clone(),
                address: profile.address.clone(),
                namespace: profile.namespace.clone(),
                read_only: profile.read_only,
                codec_enabled: profile.payload_codec.is_some(),
                is_default: config.default_profile.as_deref() == Some(name),
            })
            .collect();

        Ok(LaunchConfig {
            app: AppConfig {
                address: connection.address.clone(),
                profile_name,
                namespace: self.namespace.clone().unwrap_or(profile_namespace),
                query: self.query.clone().unwrap_or_default(),
                page_size: self.page_size.unwrap_or(200),
                refresh_interval: Duration::from_secs(self.refresh_seconds.unwrap_or(5)),
                auto_refresh: !self.no_auto_refresh,
                color: !self.no_color,
                read_only: self.read_only || profile_read_only,
                force_read_only: self.read_only,
                codec_enabled,
                web_ui_url,
                saved_queries,
                profiles,
            },
            connection,
        })
    }
}

fn run_profile_command(
    command: &ProfileCommand,
    store: &ConfigStore,
    config: &mut UserConfig,
) -> Result<()> {
    match command {
        ProfileCommand::List => {
            for (name, profile) in &config.profiles {
                let default = if config.default_profile.as_deref() == Some(name) {
                    " *"
                } else {
                    ""
                };
                let mode = if profile.read_only {
                    "read-only"
                } else {
                    "control"
                };
                println!(
                    "{name}{default}\t{}\t{}\t{mode}",
                    profile.address, profile.namespace
                );
            }
        }
        ProfileCommand::Show { name } => {
            let profile = config
                .profiles
                .get(name)
                .with_context(|| format!("profile `{name}` does not exist"))?;
            print!("{}", toml::to_string_pretty(profile)?);
        }
        ProfileCommand::Create {
            name,
            address,
            namespace,
            tls,
            tls_ca,
            tls_cert,
            tls_key,
            tls_server_name,
            headers,
            codec_endpoint,
            codec_headers,
            codec_auth_env,
            api_key_env,
            web_ui_url,
            read_only,
            set_default,
            replace,
        } => {
            if config.profiles.contains_key(name) && !replace {
                bail!("profile `{name}` already exists; pass --replace to overwrite it");
            }
            let mut unique_headers = BTreeMap::new();
            for (key, value) in headers {
                if unique_headers.insert(key.clone(), value.clone()).is_some() {
                    bail!("duplicate gRPC header: {key}");
                }
            }
            if codec_endpoint.is_none() && (!codec_headers.is_empty() || codec_auth_env.is_some()) {
                bail!("Codec Server headers require --codec-endpoint");
            }
            let payload_codec = codec_endpoint
                .as_ref()
                .map(|endpoint| {
                    let mut headers = BTreeMap::new();
                    for (key, value) in codec_headers {
                        if headers.insert(key.clone(), value.clone()).is_some() {
                            bail!("duplicate Codec Server header: {key}");
                        }
                    }
                    let secret_headers =
                        codec_auth_env
                            .as_ref()
                            .map_or_else(BTreeMap::new, |variable| {
                                BTreeMap::from([(
                                    "authorization".to_string(),
                                    SecretSource::Env {
                                        variable: variable.clone(),
                                    },
                                )])
                            });
                    Ok::<_, anyhow::Error>(ProfilePayloadCodec {
                        endpoint: endpoint.clone(),
                        headers,
                        secret_headers,
                    })
                })
                .transpose()?;
            config.profiles.insert(
                name.clone(),
                ConnectionProfile {
                    address: address.clone(),
                    namespace: namespace.clone(),
                    tls: ProfileTls {
                        enabled: *tls,
                        server_ca: tls_ca.clone(),
                        client_certificate: tls_cert.clone(),
                        client_private_key: tls_key.clone(),
                        server_name: tls_server_name.clone(),
                    },
                    api_key: api_key_env.as_ref().map(|variable| SecretSource::Env {
                        variable: variable.clone(),
                    }),
                    headers: unique_headers,
                    secret_headers: BTreeMap::new(),
                    payload_codec,
                    web_ui_url: web_ui_url.clone(),
                    read_only: *read_only,
                },
            );
            if *set_default || config.default_profile.is_none() {
                config.default_profile = Some(name.clone());
            }
            store.save(config)?;
            println!("saved profile `{name}`");
        }
        ProfileCommand::SetDefault { name } => {
            if !config.profiles.contains_key(name) {
                bail!("profile `{name}` does not exist");
            }
            config.default_profile = Some(name.clone());
            store.save(config)?;
            println!("default profile: {name}");
        }
        ProfileCommand::SetApiKey { name, from_env } => {
            let profile = config
                .profiles
                .get_mut(name)
                .with_context(|| format!("profile `{name}` does not exist"))?;
            let secret = if let Some(variable) = from_env {
                std::env::var(variable)
                    .with_context(|| format!("environment variable `{variable}` is not set"))?
            } else {
                rpassword::prompt_password("Temporal API key: ")
                    .context("could not read API key")?
            };
            store.set_api_key(name, &secret)?;
            profile.api_key = Some(SecretSource::Keyring);
            if let Err(error) = store.save(config) {
                let _ = store.delete_api_key(name);
                return Err(error);
            }
            println!("stored API key for `{name}` in the OS credential manager");
        }
        ProfileCommand::ClearApiKey { name, yes } => {
            if !yes {
                bail!("refusing to delete API key without --yes");
            }
            let profile = config
                .profiles
                .get_mut(name)
                .with_context(|| format!("profile `{name}` does not exist"))?;
            let used_keyring = matches!(profile.api_key, Some(SecretSource::Keyring));
            profile.api_key = None;
            store.save(config)?;
            if used_keyring {
                store.delete_api_key(name)?;
            }
            println!("removed API key for `{name}`");
        }
        ProfileCommand::Remove { name, yes } => {
            if !yes {
                bail!("refusing to remove profile without --yes");
            }
            let profile = config
                .profiles
                .remove(name)
                .with_context(|| format!("profile `{name}` does not exist"))?;
            if config.default_profile.as_deref() == Some(name) {
                config.default_profile = None;
            }
            store.save(config)?;
            if matches!(profile.api_key, Some(SecretSource::Keyring)) {
                store.delete_api_key(name)?;
            }
            println!("removed profile `{name}`");
        }
    }
    Ok(())
}

fn run_filter_command(
    command: &FilterCommand,
    store: &ConfigStore,
    config: &mut UserConfig,
) -> Result<()> {
    match command {
        FilterCommand::List => {
            for (name, filter) in &config.filters {
                println!("{name}\t{}", filter.query);
            }
        }
        FilterCommand::Save {
            name,
            query,
            replace,
        } => {
            if config.filters.contains_key(name) && !replace {
                bail!("filter `{name}` already exists; pass --replace to overwrite it");
            }
            config.filters.insert(
                name.clone(),
                SavedFilter {
                    query: query.clone(),
                },
            );
            store.save(config)?;
            println!("saved filter `{name}`");
        }
        FilterCommand::Remove { name, yes } => {
            if !yes {
                bail!("refusing to remove filter without --yes");
            }
            config
                .filters
                .remove(name)
                .with_context(|| format!("filter `{name}` does not exist"))?;
            store.save(config)?;
            println!("removed filter `{name}`");
        }
    }
    Ok(())
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
    use tempfile::TempDir;

    use super::*;

    fn empty_store() -> (TempDir, ConfigStore, UserConfig) {
        let directory = TempDir::new().unwrap();
        let store = ConfigStore::discover(Some(directory.path().join("config.toml"))).unwrap();
        (directory, store, UserConfig::default())
    }

    #[test]
    fn rejects_half_of_mtls_pair() {
        let cli = Cli::try_parse_from(["temporal-tui", "--tls-cert", "client.pem"])
            .expect("CLI should parse before cross-field validation");
        let (_directory, store, config) = empty_store();
        assert!(cli.launch_config(&store, &config).is_err());
    }

    #[test]
    fn api_key_enables_tls() {
        let cli = Cli::try_parse_from(["temporal-tui", "--api-key", "secret"]).unwrap();
        let (_directory, store, config) = empty_store();
        assert!(
            cli.launch_config(&store, &config)
                .unwrap()
                .connection
                .tls
                .is_some()
        );
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

    #[test]
    fn profile_settings_merge_with_safe_cli_overrides() {
        let (_directory, store, mut config) = empty_store();
        config.default_profile = Some("dev".to_string());
        config.profiles.insert(
            "dev".to_string(),
            ConnectionProfile {
                address: "dev.example:7233".to_string(),
                namespace: "payments".to_string(),
                read_only: true,
                ..ConnectionProfile::default()
            },
        );
        let cli = Cli::try_parse_from(["temporal-tui", "--namespace", "orders"]).unwrap();
        let launch = cli.launch_config(&store, &config).unwrap();
        assert_eq!(launch.connection.address, "dev.example:7233");
        assert_eq!(launch.app.namespace, "orders");
        assert!(launch.app.read_only);
        assert!(!launch.app.force_read_only);
        assert_eq!(launch.app.profiles.len(), 1);
        assert_eq!(launch.app.profiles[0].name, "dev");
        assert_eq!(launch.app.profiles[0].namespace, "payments");
        assert!(launch.app.profiles[0].is_default);
    }

    #[test]
    fn codec_cli_settings_enable_remote_payload_conversion() {
        let (_directory, store, config) = empty_store();
        let cli = Cli::try_parse_from([
            "temporal-tui",
            "--codec-endpoint",
            "http://127.0.0.1:8081/{namespace}",
            "--codec-header",
            "x-owner=temporal-tui",
            "--codec-auth",
            "Bearer test-token",
        ])
        .unwrap();
        let launch = cli.launch_config(&store, &config).unwrap();
        let codec = launch.connection.payload_codec.unwrap();
        assert!(launch.app.codec_enabled);
        assert_eq!(
            codec.endpoint,
            "http://127.0.0.1:8081/{namespace}".to_string()
        );
        assert_eq!(
            codec.headers.get("authorization").map(String::as_str),
            Some("Bearer test-token")
        );
        assert_eq!(
            codec.headers.get("x-owner").map(String::as_str),
            Some("temporal-tui")
        );
    }
}
