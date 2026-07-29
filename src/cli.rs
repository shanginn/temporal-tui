use std::{
    collections::{BTreeMap, HashMap},
    io::{Read, Write},
    path::PathBuf,
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use zeroize::Zeroizing;

use crate::{
    app::{AppConfig, ProfileSummary, SavedQuery},
    auth::{AuthError, AuthSession, TemporalAuthProfile},
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

    /// Temporal CLI forwards this host-enforced timeout to extensions.
    #[arg(long, global = true, hide = true)]
    pub command_timeout: Option<String>,

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
    /// Sign in to a protected self-hosted Temporal deployment.
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// Print the active config path.
    ConfigPath,
}

#[derive(Clone, Subcommand)]
pub enum AuthCommand {
    /// Sign in with a local username and a password read without echo.
    Login {
        /// Temporal auth base URL.
        #[arg(long, env = "TEMPORAL_AUTH_URL")]
        url: Option<String>,
        /// Local username. Prompted when omitted.
        #[arg(long)]
        username: Option<String>,
        /// Temporal gRPC address override when the auth service does not advertise one.
        #[arg(long)]
        address: Option<String>,
        /// Namespace stored in a newly created profile.
        #[arg(long, short = 'n')]
        namespace: Option<String>,
        /// Read the password from stdin instead of a terminal prompt.
        #[arg(long)]
        password_stdin: bool,
        /// Permit loopback-only HTTP for local development and tests.
        #[arg(long, hide = true)]
        allow_http: bool,
        /// Make the selected profile the default.
        #[arg(long)]
        set_default: bool,
    },
    /// Show the current signed-in identity and session status.
    Whoami,
    /// Revoke the refresh grant and remove its local credential.
    Logout,
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
    pub auth: Option<LaunchAuth>,
    pub app: AppConfig,
}

/// Non-secret metadata required to open a refreshable authenticated session.
pub struct LaunchAuth {
    pub profile_name: String,
    pub profile: TemporalAuthProfile,
}

impl Cli {
    /// Refuse host-enforced timeouts around interrupt-unsafe commands.
    ///
    /// # Errors
    ///
    /// Returns an error when Temporal CLI could forcibly kill the dashboard,
    /// authentication, credential storage, config migration, or a config
    /// mutation mid-operation.
    pub fn validate_command_timeout_safety(&self) -> Result<()> {
        if self.command_timeout.is_none() {
            return Ok(());
        }
        if !matches!(&self.command, Some(CliCommand::ConfigPath)) {
            bail!(
                "--command-timeout is supported only for the read-only local `temporal tui \
                 config-path` command; a forced timeout cannot safely interrupt the dashboard, \
                 authentication, credential storage, config loading or migration, or config \
                 mutations"
            );
        }
        Ok(())
    }

    /// Execute a config subcommand.
    ///
    /// Returns `true` when a subcommand was handled and the TUI must not start.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid input, failed persistence, secret-store
    /// failures, or an explicitly unconfirmed destructive operation.
    pub async fn run_config_command(&self, store: &ConfigStore) -> Result<bool> {
        let Some(command) = &self.command else {
            return Ok(false);
        };
        if matches!(command, CliCommand::ConfigPath) {
            println!("{}", store.path().display());
            return Ok(true);
        }
        let mut config = store.load()?;
        match command {
            CliCommand::Profile { command } => {
                run_profile_command(command, store, &mut config)?;
            }
            CliCommand::Filter { command } => {
                run_filter_command(command, store, &mut config)?;
            }
            CliCommand::Auth { command } => {
                run_auth_command(command, self.profile.as_deref(), store, &mut config).await?;
            }
            CliCommand::ConfigPath => {
                unreachable!("config-path returns before loading the config")
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

        let (
            mut connection,
            profile_namespace,
            profile_web_url,
            profile_read_only,
            mut profile_auth,
        ) = if let Some(name) = profile_name.as_deref() {
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
                resolved.auth,
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
                None,
            )
        };

        if let Some(address) = &self.address {
            connection.address.clone_from(address);
        }
        if let Some(api_key) = &self.api_key {
            connection.api_key = Some(api_key.clone());
            profile_auth = None;
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

        let inherited_tls = connection.tls.is_some();
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
        let auth_requires_tls = profile_auth.as_ref().is_some_and(|auth| {
            !auth.allow_insecure || !temporal_address_is_loopback(&connection.address)
        });
        let tls_enabled = inherited_tls
            || self.tls
            || connection.api_key.is_some()
            || auth_requires_tls
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
                auth_enabled: profile.auth.is_some(),
                codec_enabled: profile.payload_codec.is_some(),
                is_default: config.default_profile.as_deref() == Some(name),
            })
            .collect();
        let auth = profile_auth
            .map(|profile| {
                Ok::<_, anyhow::Error>(LaunchAuth {
                    profile_name: profile_name
                        .clone()
                        .context("authenticated launch requires a selected profile")?,
                    profile,
                })
            })
            .transpose()?;

        Ok(LaunchConfig {
            auth,
            app: AppConfig {
                address: connection.address.clone(),
                profile_name,
                namespace: self.namespace.clone().unwrap_or(profile_namespace),
                query: self.query.clone().unwrap_or_default(),
                page_size: self.page_size.unwrap_or(config.ui.page_size),
                refresh_interval: Duration::from_secs(
                    self.refresh_seconds.unwrap_or(config.ui.refresh_seconds),
                ),
                auto_refresh: config.ui.auto_refresh && !self.no_auto_refresh,
                color: color_enabled(
                    config.ui.color,
                    self.no_color,
                    std::env::var_os("NO_COLOR").is_some(),
                ),
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
                let auth = if profile.auth.is_some() {
                    "login"
                } else if profile.api_key.is_some() {
                    "api-key"
                } else {
                    "direct"
                };
                println!(
                    "{name}{default}\t{}\t{}\t{mode}\t{auth}",
                    profile.address, profile.namespace,
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
            if config
                .profiles
                .get(name)
                .is_some_and(|profile| profile.auth.is_some())
            {
                bail!("profile `{name}` is signed in; run `auth logout` before replacing it");
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
                    auth: None,
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
            if profile.auth.is_some() {
                bail!("profile `{name}` uses local login; run `auth logout` first");
            }
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
            if profile.auth.is_some() {
                config.profiles.insert(name.clone(), profile);
                bail!("profile `{name}` is signed in; run `auth logout` before removing it");
            }
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

async fn run_auth_command(
    command: &AuthCommand,
    selected_profile: Option<&str>,
    store: &ConfigStore,
    config: &mut UserConfig,
) -> Result<()> {
    let profile_name = selected_profile
        .map(str::to_owned)
        .or_else(|| config.default_profile.clone())
        .unwrap_or_else(|| "default".to_string());

    match command {
        AuthCommand::Login {
            url,
            username,
            address,
            namespace,
            password_stdin,
            allow_http,
            set_default,
        } => {
            if let Some(existing) = config.profiles.get(&profile_name) {
                if existing.auth.is_some() {
                    bail!(
                        "profile `{profile_name}` is already signed in; run `auth logout` before signing in again"
                    );
                }
                if existing.api_key.is_some() {
                    bail!("profile `{profile_name}` uses an API key; clear it before signing in");
                }
                if existing.headers.contains_key("authorization")
                    || existing.secret_headers.contains_key("authorization")
                {
                    bail!(
                        "profile `{profile_name}` already defines an authorization header; remove it before signing in"
                    );
                }
            }

            if *password_stdin && (url.is_none() || username.is_none()) {
                bail!("--password-stdin requires --url and --username");
            }
            let auth_url = match url {
                Some(url) => url.trim().to_string(),
                None => prompt_line("Temporal auth URL: ")?,
            };
            let username = match username {
                Some(username) => username.trim().to_string(),
                None => prompt_line("Username: ")?,
            };
            if auth_url.is_empty() {
                bail!("Temporal auth URL must not be empty");
            }
            if username.is_empty() {
                bail!("username must not be empty");
            }

            let password = Zeroizing::new(if *password_stdin {
                read_password_from_stdin()?
            } else {
                rpassword::prompt_password("Password: ").context("could not read password")?
            });
            let login = AuthSession::login(
                &profile_name,
                &auth_url,
                &username,
                password.as_str(),
                address.as_deref(),
                *allow_http,
            )
            .await?;

            let mut updated = config.clone();
            let is_new = !updated.profiles.contains_key(&profile_name);
            let profile = updated.profiles.entry(profile_name.clone()).or_default();
            profile.address.clone_from(&login.temporal_address);
            profile.namespace = namespace
                .clone()
                .unwrap_or_else(|| profile.namespace.clone());
            profile.tls.enabled = login.temporal_tls;
            profile.api_key = None;
            profile.auth = Some(login.profile.clone());
            if is_new {
                profile.web_ui_url = Some(login.profile.url.clone());
            }
            if *set_default || updated.default_profile.is_none() {
                updated.default_profile = Some(profile_name.clone());
            }

            if let Err(save_error) = store.save(&updated) {
                let cleanup = login.session.logout().await;
                return match cleanup {
                    Ok(()) => Err(save_error.context(
                        "could not save the signed-in profile; the new session was revoked",
                    )),
                    Err(cleanup_error) => Err(anyhow!(
                        "could not save the signed-in profile ({save_error}); cleanup also failed ({cleanup_error})"
                    )),
                };
            }
            *config = updated;
            println!("Signed in as {username}.");
            println!("Profile: {profile_name}");
            println!(
                "Temporal endpoint: {} ({})",
                login.temporal_address,
                if login.temporal_tls {
                    "TLS"
                } else {
                    "local development"
                }
            );
            println!("Run: temporal tui --profile {profile_name}");
            println!("Standalone: temporal-tui --profile {profile_name}");
        }
        AuthCommand::Whoami => {
            let profile = config
                .profiles
                .get(&profile_name)
                .with_context(|| format!("profile `{profile_name}` does not exist"))?;
            let auth = profile
                .auth
                .clone()
                .with_context(|| format!("profile `{profile_name}` is not signed in"))?;
            let session = AuthSession::load(&profile_name, auth)?;
            let identity = session.userinfo().await?;
            println!("Profile: {profile_name}");
            println!("User: {}", identity.preferred_username);
            println!("Temporal endpoint: {}", profile.address);
            println!("Session: active");
        }
        AuthCommand::Logout => {
            let auth = config
                .profiles
                .get(&profile_name)
                .with_context(|| format!("profile `{profile_name}` does not exist"))?
                .auth
                .clone()
                .with_context(|| format!("profile `{profile_name}` is not signed in"))?;
            let session = match AuthSession::load(&profile_name, auth) {
                Ok(session) => session,
                Err(AuthError::NotLoggedIn(_)) => {
                    let Some(profile) = config.profiles.get_mut(&profile_name) else {
                        bail!("profile `{profile_name}` disappeared while signing out");
                    };
                    profile.auth = None;
                    store.save(config).context(
                        "the local credential was already absent, but stale profile metadata could not be removed",
                    )?;
                    println!("Profile `{profile_name}` had no local login credential.");
                    println!(
                        "Removed stale local login metadata; no server grant could be revoked."
                    );
                    return Ok(());
                }
                Err(error) => return Err(error.into()),
            };
            if let Err(error) = session.logout().await {
                if !matches!(error, AuthError::NotLoggedIn(_)) {
                    return Err(error.into());
                }
                let Some(profile) = config.profiles.get_mut(&profile_name) else {
                    bail!("profile `{profile_name}` disappeared while signing out");
                };
                profile.auth = None;
                store.save(config).context(
                    "the login was removed by another process, but stale profile metadata could not be removed",
                )?;
                println!("Profile `{profile_name}` was already signed out.");
                println!("Removed stale local login metadata.");
                return Ok(());
            }

            let Some(profile) = config.profiles.get_mut(&profile_name) else {
                bail!("profile `{profile_name}` disappeared while signing out");
            };
            profile.auth = None;
            store.save(config).context(
                "session was revoked and its credential removed, but the profile could not be updated",
            )?;
            println!("Signed out profile `{profile_name}`.");
            println!("The server session was revoked and local credentials were removed.");
        }
    }
    Ok(())
}

fn prompt_line(prompt: &str) -> Result<String> {
    let mut stderr = std::io::stderr().lock();
    stderr
        .write_all(prompt.as_bytes())
        .context("could not write login prompt")?;
    stderr.flush().context("could not flush login prompt")?;
    drop(stderr);

    let mut value = String::new();
    std::io::stdin()
        .read_line(&mut value)
        .context("could not read login input")?;
    Ok(value.trim().to_string())
}

fn read_password_from_stdin() -> Result<String> {
    const MAX_PASSWORD_BYTES: u64 = 1 << 20;

    let mut bytes = zeroize::Zeroizing::new(Vec::new());
    std::io::stdin()
        .lock()
        .take(MAX_PASSWORD_BYTES + 1)
        .read_to_end(&mut bytes)
        .context("could not read password from stdin")?;
    if bytes.len() as u64 > MAX_PASSWORD_BYTES {
        bail!("password input is larger than 1 MiB");
    }
    while matches!(bytes.last(), Some(b'\n' | b'\r')) {
        bytes.pop();
    }
    let password = std::str::from_utf8(&bytes)
        .context("password input is not valid UTF-8")?
        .to_string();
    if password.is_empty() {
        bail!("password must not be empty");
    }
    Ok(password)
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
    if value
        .bytes()
        .any(|byte| matches!(byte, b'\0' | b'\r' | b'\n'))
    {
        return Err("header value must not contain NUL, CR, or LF bytes".to_string());
    }
    if key.ends_with("-bin")
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err("header key must be lowercase ASCII and must not end in -bin".to_string());
    }
    if ["authorization", "cookie", "token", "secret", "api-key"]
        .iter()
        .any(|needle| key.contains(needle))
    {
        return Err(
            "sensitive headers must use an API-key, Codec auth, or profile secret option"
                .to_string(),
        );
    }
    Ok((key, value))
}

fn temporal_address_is_loopback(address: &str) -> bool {
    use url::Host;

    let normalized = if address.contains("://") {
        address.to_string()
    } else {
        format!("http://{address}")
    };
    let Ok(parsed) = url::Url::parse(&normalized) else {
        return false;
    };
    match parsed.host() {
        Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

const fn color_enabled(preference: bool, cli_disabled: bool, no_color_env: bool) -> bool {
    preference && !cli_disabled && !no_color_env
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
    use crate::auth::TemporalAuthProfile;

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
    fn auth_login_has_no_password_argument() {
        assert!(
            Cli::try_parse_from([
                "temporal-tui",
                "--profile",
                "rubase",
                "auth",
                "login",
                "--url",
                "https://temporal.example.com",
                "--username",
                "admin",
                "--password",
                "must-not-enter-process-arguments",
            ])
            .is_err()
        );
        Cli::try_parse_from([
            "temporal-tui",
            "--profile",
            "rubase",
            "auth",
            "login",
            "--url",
            "https://temporal.example.com",
            "--username",
            "admin",
            "--password-stdin",
        ])
        .unwrap();
    }

    #[test]
    fn authenticated_profile_launch_is_refreshable_and_tls_protected() {
        let (_directory, store, mut config) = empty_store();
        config.default_profile = Some("rubase".to_string());
        config.profiles.insert(
            "rubase".to_string(),
            ConnectionProfile {
                address: "temporal-grpc.example.com:443".to_string(),
                auth: Some(TemporalAuthProfile {
                    url: "https://temporal.example.com".to_string(),
                    username: "admin".to_string(),
                    token_endpoint: "https://temporal.example.com/oauth/token".to_string(),
                    allow_insecure: false,
                }),
                ..ConnectionProfile::default()
            },
        );

        let launch = Cli::try_parse_from(["temporal-tui"])
            .unwrap()
            .launch_config(&store, &config)
            .unwrap();
        assert!(launch.auth.is_some());
        assert!(launch.connection.api_key.is_none());
        assert!(launch.connection.tls.is_some());
        assert!(launch.app.profiles[0].auth_enabled);
    }

    #[test]
    fn address_override_cannot_turn_loopback_login_into_plaintext_remote_auth() {
        let (_directory, store, mut config) = empty_store();
        config.default_profile = Some("dev".to_string());
        config.profiles.insert(
            "dev".to_string(),
            ConnectionProfile {
                address: "127.0.0.1:7233".to_string(),
                auth: Some(TemporalAuthProfile {
                    url: "http://127.0.0.1:8080".to_string(),
                    username: "admin".to_string(),
                    token_endpoint: "http://127.0.0.1:8080/oauth/token".to_string(),
                    allow_insecure: true,
                }),
                ..ConnectionProfile::default()
            },
        );

        let launch = Cli::try_parse_from(["temporal-tui", "--address", "temporal.example.com:443"])
            .unwrap()
            .launch_config(&store, &config)
            .unwrap();
        assert!(launch.auth.is_some());
        assert!(launch.connection.tls.is_some());
    }

    #[test]
    fn parses_safe_headers() {
        assert_eq!(
            parse_header("x-owner=temporal-tui").unwrap(),
            ("x-owner".to_string(), "temporal-tui".to_string())
        );
        assert!(parse_header("Bad Header=value").is_err());
        assert!(parse_header("payload-bin=value").is_err());
        assert!(parse_header("authorization=Bearer secret").is_err());
        assert!(parse_header("x-owner=safe\r\ninjected").is_err());
    }

    #[test]
    fn color_preference_honors_cli_and_no_color_environment() {
        assert!(color_enabled(true, false, false));
        assert!(!color_enabled(false, false, false));
        assert!(!color_enabled(true, true, false));
        assert!(!color_enabled(true, false, true));
    }

    #[test]
    fn rejects_unbounded_refresh_and_page_sizes() {
        assert!(Cli::try_parse_from(["temporal-tui", "--page-size", "0"]).is_err());
        assert!(Cli::try_parse_from(["temporal-tui", "--refresh-seconds", "3601"]).is_err());
    }

    #[test]
    fn accepts_temporal_cli_command_timeout_passthrough() {
        let cli = Cli::try_parse_from([
            "temporal-tui",
            "--command-timeout",
            "5s",
            "--profile",
            "production",
            "config-path",
        ])
        .unwrap();
        assert_eq!(cli.command_timeout.as_deref(), Some("5s"));
        assert_eq!(cli.profile.as_deref(), Some("production"));
    }

    #[test]
    fn rejects_forced_timeout_for_interactive_terminal_safety() {
        let cli = Cli::try_parse_from(["temporal-tui", "--command-timeout", "5s"]).unwrap();
        let error = cli
            .validate_command_timeout_safety()
            .unwrap_err()
            .to_string();
        assert!(error.contains("cannot safely interrupt the dashboard"));
    }

    #[test]
    fn rejects_forced_timeout_around_config_access_mutations_and_authentication() {
        for arguments in [
            vec!["temporal-tui", "--command-timeout", "5s", "profile", "list"],
            vec!["temporal-tui", "--command-timeout", "5s", "filter", "list"],
            vec![
                "temporal-tui",
                "--command-timeout",
                "5s",
                "auth",
                "login",
                "--url",
                "https://temporal.example.com",
                "--username",
                "admin",
                "--password-stdin",
            ],
            vec![
                "temporal-tui",
                "--command-timeout",
                "5s",
                "profile",
                "set-api-key",
                "production",
                "--from-env",
                "TEMPORAL_TEST_API_KEY",
            ],
            vec![
                "temporal-tui",
                "--command-timeout",
                "5s",
                "filter",
                "save",
                "stuck",
                "ExecutionStatus = 'Running'",
            ],
        ] {
            let cli = Cli::try_parse_from(arguments).unwrap();
            let error = cli
                .validate_command_timeout_safety()
                .unwrap_err()
                .to_string();
            assert!(error.contains("cannot safely interrupt"));
        }
    }

    #[test]
    fn allows_forced_timeout_only_for_config_path() {
        Cli::try_parse_from(["temporal-tui", "--command-timeout", "5s", "config-path"])
            .unwrap()
            .validate_command_timeout_safety()
            .unwrap();
    }

    #[tokio::test]
    async fn config_path_does_not_load_or_migrate_the_config() {
        let (_directory, store, _config) = empty_store();
        std::fs::write(store.path(), "this is not valid TOML").unwrap();
        let cli = Cli::try_parse_from(["temporal-tui", "--command-timeout", "5s", "config-path"])
            .unwrap();

        assert!(cli.run_config_command(&store).await.unwrap());
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
    fn ui_preferences_are_defaults_and_cli_flags_only_tighten_them() {
        let (_directory, store, mut config) = empty_store();
        config.ui.page_size = 321;
        config.ui.refresh_seconds = 17;
        config.ui.auto_refresh = false;
        config.ui.color = false;

        let cli = Cli::try_parse_from(["temporal-tui"]).unwrap();
        let launch = cli.launch_config(&store, &config).unwrap();
        assert_eq!(launch.app.page_size, 321);
        assert_eq!(launch.app.refresh_interval, Duration::from_secs(17));
        assert!(!launch.app.auto_refresh);
        assert!(!launch.app.color);

        config.ui.auto_refresh = true;
        config.ui.color = true;
        let cli = Cli::try_parse_from([
            "temporal-tui",
            "--page-size",
            "123",
            "--refresh-seconds",
            "9",
            "--no-auto-refresh",
            "--no-color",
        ])
        .unwrap();
        let launch = cli.launch_config(&store, &config).unwrap();
        assert_eq!(launch.app.page_size, 123);
        assert_eq!(launch.app.refresh_interval, Duration::from_secs(9));
        assert!(!launch.app.auto_refresh);
        assert!(!launch.app.color);
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
