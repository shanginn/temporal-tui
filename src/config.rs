use std::{
    collections::{BTreeMap, HashMap},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::service::{ClientTlsConfig, PayloadCodecConfig, TemporalConnectionConfig};

const CONFIG_SCHEMA_VERSION: u32 = 2;
const KEYRING_SERVICE: &str = "io.temporal.temporal-tui";

/// Persisted application settings. Secret values are represented only by references.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UserConfig {
    pub schema_version: u32,
    pub default_profile: Option<String>,
    pub profiles: BTreeMap<String, ConnectionProfile>,
    pub filters: BTreeMap<String, SavedFilter>,
    pub ui: UiPreferences,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            default_profile: None,
            profiles: BTreeMap::new(),
            filters: BTreeMap::new(),
            ui: UiPreferences::default(),
        }
    }
}

impl UserConfig {
    /// Validate the complete configuration before it can affect a connection.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported schemas, invalid names, incomplete TLS
    /// configuration, invalid URLs, or dangling default-profile references.
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != CONFIG_SCHEMA_VERSION {
            bail!(
                "unsupported config schema {}; expected {}",
                self.schema_version,
                CONFIG_SCHEMA_VERSION
            );
        }

        if let Some(default_profile) = &self.default_profile
            && !self.profiles.contains_key(default_profile)
        {
            bail!("default profile `{default_profile}` does not exist");
        }

        for (name, profile) in &self.profiles {
            validate_profile_name(name)?;
            profile
                .validate()
                .with_context(|| format!("profile `{name}` is invalid"))?;
        }
        for (name, filter) in &self.filters {
            validate_display_name(name, "filter")?;
            if filter.query.contains('\0') {
                bail!("filter `{name}` contains a NUL byte");
            }
        }
        self.ui.validate()?;
        Ok(())
    }
}

/// Persisted defaults for the interactive terminal experience.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiPreferences {
    pub page_size: usize,
    pub refresh_seconds: u64,
    pub auto_refresh: bool,
    pub color: bool,
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            page_size: 200,
            refresh_seconds: 5,
            auto_refresh: true,
            color: true,
        }
    }
}

impl UiPreferences {
    fn validate(&self) -> Result<()> {
        if !(1..=5_000).contains(&self.page_size) {
            bail!("ui.page_size must be between 1 and 5000");
        }
        if !(1..=3_600).contains(&self.refresh_seconds) {
            bail!("ui.refresh_seconds must be between 1 and 3600");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LegacyUserConfigV1 {
    schema_version: u32,
    default_profile: Option<String>,
    profiles: BTreeMap<String, ConnectionProfile>,
    filters: BTreeMap<String, SavedFilter>,
}

impl Default for LegacyUserConfigV1 {
    fn default() -> Self {
        Self {
            schema_version: 1,
            default_profile: None,
            profiles: BTreeMap::new(),
            filters: BTreeMap::new(),
        }
    }
}

impl From<LegacyUserConfigV1> for UserConfig {
    fn from(legacy: LegacyUserConfigV1) -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            default_profile: legacy.default_profile,
            profiles: legacy.profiles,
            filters: legacy.filters,
            ui: UiPreferences::default(),
        }
    }
}

/// Non-secret connection profile stored in the user config.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConnectionProfile {
    pub address: String,
    pub namespace: String,
    pub tls: ProfileTls,
    pub api_key: Option<SecretSource>,
    pub headers: BTreeMap<String, String>,
    pub secret_headers: BTreeMap<String, SecretSource>,
    pub payload_codec: Option<ProfilePayloadCodec>,
    pub web_ui_url: Option<String>,
    pub read_only: bool,
}

impl Default for ConnectionProfile {
    fn default() -> Self {
        Self {
            address: "127.0.0.1:7233".to_string(),
            namespace: "default".to_string(),
            tls: ProfileTls::default(),
            api_key: None,
            headers: BTreeMap::new(),
            secret_headers: BTreeMap::new(),
            payload_codec: None,
            web_ui_url: Some("http://127.0.0.1:8233".to_string()),
            read_only: false,
        }
    }
}

impl ConnectionProfile {
    fn validate(&self) -> Result<()> {
        validate_temporal_address(&self.address)?;
        if self.namespace.trim().is_empty() {
            bail!("namespace must not be empty");
        }
        if self.tls.client_certificate.is_some() != self.tls.client_private_key.is_some() {
            bail!("TLS client certificate and private key must be configured together");
        }
        if let Some(url) = &self.web_ui_url {
            let parsed = Url::parse(url).context("web_ui_url is not a valid URL")?;
            if !matches!(parsed.scheme(), "http" | "https") {
                bail!("web_ui_url must use http or https");
            }
            if !parsed.username().is_empty() || parsed.password().is_some() {
                bail!("web_ui_url must not contain credentials");
            }
            if parsed.fragment().is_some() {
                bail!("web_ui_url must not contain a fragment");
            }
        }
        for key in self.headers.keys().chain(self.secret_headers.keys()) {
            validate_header_name(key)?;
        }
        for key in self.headers.keys() {
            if self.secret_headers.contains_key(key) {
                bail!("header `{key}` is configured as both public and secret");
            }
        }
        if self
            .headers
            .keys()
            .any(|key| looks_sensitive_header_name(key))
        {
            bail!("sensitive headers must use secret_headers");
        }
        if self
            .headers
            .values()
            .any(|value| contains_invalid_header_bytes(value))
        {
            bail!("header values must not contain NUL, CR, or LF bytes");
        }
        if let Some(secret) = &self.api_key {
            secret.validate()?;
        }
        for secret in self.secret_headers.values() {
            secret.validate()?;
        }
        if let Some(codec) = &self.payload_codec {
            codec.validate()?;
        }
        Ok(())
    }
}

/// Non-secret Codec Server endpoint and secret references stored in a profile.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProfilePayloadCodec {
    pub endpoint: String,
    pub headers: BTreeMap<String, String>,
    pub secret_headers: BTreeMap<String, SecretSource>,
}

impl ProfilePayloadCodec {
    fn validate(&self) -> Result<()> {
        if self.endpoint.trim().is_empty() {
            bail!("Payload Codec endpoint must not be empty");
        }
        let rendered = self.endpoint.replace("{namespace}", "namespace");
        let url = Url::parse(&rendered).context("Payload Codec endpoint is not a valid URL")?;
        if !matches!(url.scheme(), "http" | "https") {
            bail!("Payload Codec endpoint must use http or https");
        }
        if !url.username().is_empty() || url.password().is_some() {
            bail!("Payload Codec endpoint must not contain credentials");
        }
        if url.fragment().is_some() {
            bail!("Payload Codec endpoint must not contain a fragment");
        }
        for key in self.headers.keys().chain(self.secret_headers.keys()) {
            validate_header_name(key)?;
            if matches!(
                key.as_str(),
                "content-type" | "content-length" | "host" | "x-namespace"
            ) {
                bail!("Payload Codec header `{key}` is managed by temporal-tui");
            }
        }
        for key in self.headers.keys() {
            if self.secret_headers.contains_key(key) {
                bail!("Payload Codec header `{key}` is both public and secret");
            }
        }
        if self
            .headers
            .keys()
            .any(|key| looks_sensitive_header_name(key))
        {
            bail!("sensitive Payload Codec headers must use secret_headers");
        }
        if self.headers.values().any(|value| {
            value
                .bytes()
                .any(|byte| matches!(byte, b'\0' | b'\r' | b'\n'))
        }) {
            bail!("Payload Codec header values contain invalid bytes");
        }
        for secret in self.secret_headers.values() {
            secret.validate()?;
        }
        Ok(())
    }
}

/// TLS file references stored in a profile.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProfileTls {
    pub enabled: bool,
    pub server_ca: Option<PathBuf>,
    pub client_certificate: Option<PathBuf>,
    pub client_private_key: Option<PathBuf>,
    pub server_name: Option<String>,
}

/// Where a secret is resolved at runtime. Secret material is never serialized.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum SecretSource {
    Env { variable: String },
    Keyring,
}

impl SecretSource {
    fn validate(&self) -> Result<()> {
        if let Self::Env { variable } = self
            && (variable.is_empty()
                || !variable
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'))
        {
            bail!("secret environment variable name is invalid");
        }
        Ok(())
    }
}

/// A named Temporal Visibility query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedFilter {
    pub query: String,
}

/// A profile after secret references have been resolved.
pub struct ResolvedProfile {
    pub connection: TemporalConnectionConfig,
    pub namespace: String,
    pub web_ui_url: Option<String>,
    pub read_only: bool,
}

/// Location-aware config persistence.
#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    /// Discover the platform config path, unless an explicit path was supplied.
    ///
    /// # Errors
    ///
    /// Returns an error when the operating system has no discoverable home/config
    /// directories.
    pub fn discover(explicit: Option<PathBuf>) -> Result<Self> {
        if let Some(path) = explicit {
            return Ok(Self { path });
        }
        let base = BaseDirs::new().context("could not determine the user config directory")?;
        Ok(Self {
            path: base.config_dir().join("temporal-tui").join("config.toml"),
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Load and validate the config, or return defaults when it does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error when the file is unreadable, malformed, invalid, from
    /// an unsupported schema, or cannot be migrated safely.
    pub fn load(&self) -> Result<UserConfig> {
        let raw = match fs::read_to_string(&self.path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(UserConfig::default());
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("could not read {}", self.path.display()));
            }
        };
        let document: toml::Value = toml::from_str(&raw)
            .with_context(|| format!("could not parse {}", self.path.display()))?;
        let schema_version = document
            .get("schema_version")
            .and_then(toml::Value::as_integer)
            .context("config is missing an integer schema_version")
            .and_then(|version| {
                u32::try_from(version)
                    .context("config schema_version is outside the supported range")
            })?;

        match schema_version {
            CONFIG_SCHEMA_VERSION => {
                let config: UserConfig = toml::from_str(&raw)
                    .with_context(|| format!("could not parse {}", self.path.display()))?;
                config
                    .validate()
                    .with_context(|| format!("could not validate {}", self.path.display()))?;
                Ok(config)
            }
            1 => self.migrate_v1(&raw),
            version if version > CONFIG_SCHEMA_VERSION => bail!(
                "config schema {version} is newer than supported schema {CONFIG_SCHEMA_VERSION}; upgrade temporal-tui"
            ),
            version => bail!(
                "unsupported config schema {version}; supported schema is {CONFIG_SCHEMA_VERSION}"
            ),
        }
    }

    /// Atomically save a validated config with user-only permissions on Unix.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, directory creation, writing, syncing, or
    /// replacement fails.
    pub fn save(&self, config: &UserConfig) -> Result<()> {
        config.validate()?;
        let serialized = toml::to_string_pretty(config).context("could not serialize config")?;
        let parent = self
            .path
            .parent()
            .context("config path must have a parent directory")?;
        fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;

        let mut options = atomic_write_file::OpenOptions::new();
        #[cfg(unix)]
        {
            atomic_write_file::unix::OpenOptionsExt::preserve_mode(&mut options, false);
            std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        }
        let mut file = options.open(&self.path).with_context(|| {
            format!(
                "could not create an atomic update for {}",
                self.path.display()
            )
        })?;
        file.write_all(serialized.as_bytes())
            .with_context(|| format!("could not write {}", self.path.display()))?;
        file.commit()
            .with_context(|| format!("could not atomically replace {}", self.path.display()))?;
        Ok(())
    }

    fn migrate_v1(&self, raw: &str) -> Result<UserConfig> {
        let legacy: LegacyUserConfigV1 = toml::from_str(raw)
            .with_context(|| format!("could not parse schema 1 config {}", self.path.display()))?;
        if legacy.schema_version != 1 {
            bail!(
                "schema 1 migration received schema {}",
                legacy.schema_version
            );
        }
        let migrated = UserConfig::from(legacy);
        migrated
            .validate()
            .context("schema 1 config is invalid after migration")?;
        self.save_migration_backup(1, raw.as_bytes())?;
        self.save(&migrated)
            .context("could not persist migrated schema 2 config")?;
        Ok(migrated)
    }

    fn save_migration_backup(&self, schema_version: u32, original: &[u8]) -> Result<()> {
        let backup_path = self
            .path
            .with_extension(format!("toml.v{schema_version}.bak"));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&backup_path) {
            Ok(mut file) => {
                file.write_all(original)
                    .with_context(|| format!("could not write {}", backup_path.display()))?;
                file.sync_all()
                    .with_context(|| format!("could not sync {}", backup_path.display()))?;
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing = fs::read(&backup_path)
                    .with_context(|| format!("could not read {}", backup_path.display()))?;
                if existing == original {
                    Ok(())
                } else {
                    bail!(
                        "migration backup {} already exists with different contents",
                        backup_path.display()
                    )
                }
            }
            Err(error) => {
                Err(error).with_context(|| format!("could not create {}", backup_path.display()))
            }
        }
    }

    /// Resolve a profile and all referenced secrets.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown profile or an unavailable/missing secret.
    pub fn resolve_profile(
        &self,
        config: &UserConfig,
        profile_name: &str,
    ) -> Result<ResolvedProfile> {
        let profile = config
            .profiles
            .get(profile_name)
            .with_context(|| format!("profile `{profile_name}` does not exist"))?;
        profile.validate()?;

        let api_key = profile
            .api_key
            .as_ref()
            .map(|source| resolve_secret(profile_name, "api-key", source))
            .transpose()?;
        let mut headers: HashMap<String, String> = profile
            .headers
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        for (key, source) in &profile.secret_headers {
            headers.insert(
                key.clone(),
                resolve_secret(profile_name, &format!("header/{key}"), source)?,
            );
        }
        if api_key
            .as_deref()
            .is_some_and(|value| value.is_empty() || contains_invalid_header_bytes(value))
        {
            bail!("resolved API key is empty or contains invalid bytes");
        }
        if headers
            .values()
            .any(|value| contains_invalid_header_bytes(value))
        {
            bail!("resolved gRPC header contains invalid bytes");
        }
        let payload_codec = profile
            .payload_codec
            .as_ref()
            .map(|codec| {
                let mut headers = codec
                    .headers
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect::<HashMap<_, _>>();
                for (key, source) in &codec.secret_headers {
                    headers.insert(
                        key.clone(),
                        resolve_secret(
                            profile_name,
                            &format!("payload-codec/header/{key}"),
                            source,
                        )?,
                    );
                }
                Ok::<_, anyhow::Error>(PayloadCodecConfig {
                    endpoint: codec.endpoint.clone(),
                    headers,
                })
            })
            .transpose()?;

        let tls_enabled = profile.tls.enabled
            || api_key.is_some()
            || profile.tls.server_ca.is_some()
            || profile.tls.client_certificate.is_some()
            || profile.address.starts_with("https://");
        Ok(ResolvedProfile {
            connection: TemporalConnectionConfig {
                address: profile.address.clone(),
                api_key,
                headers,
                tls: tls_enabled.then(|| ClientTlsConfig {
                    server_ca: profile.tls.server_ca.clone(),
                    client_certificate: profile.tls.client_certificate.clone(),
                    client_private_key: profile.tls.client_private_key.clone(),
                    server_name: profile.tls.server_name.clone(),
                }),
                payload_codec,
            },
            namespace: profile.namespace.clone(),
            web_ui_url: profile.web_ui_url.clone(),
            read_only: profile.read_only,
        })
    }

    /// Store an API key in the OS credential manager for a profile.
    ///
    /// # Errors
    ///
    /// Returns an error when the profile name is invalid, the secret is empty, or
    /// the platform credential manager rejects the operation.
    pub fn set_api_key(&self, profile_name: &str, secret: &str) -> Result<()> {
        validate_profile_name(profile_name)?;
        if secret.is_empty() {
            bail!("API key must not be empty");
        }
        keyring_entry(profile_name, "api-key")?
            .set_password(secret)
            .context("could not save API key in the OS credential manager")
    }

    /// Delete a profile API key from the OS credential manager.
    ///
    /// # Errors
    ///
    /// Returns an error when the platform credential manager rejects the operation.
    pub fn delete_api_key(&self, profile_name: &str) -> Result<()> {
        validate_profile_name(profile_name)?;
        keyring_entry(profile_name, "api-key")?
            .delete_credential()
            .context("could not delete API key from the OS credential manager")
    }
}

fn resolve_secret(profile_name: &str, field: &str, source: &SecretSource) -> Result<String> {
    match source {
        SecretSource::Env { variable } => std::env::var(variable)
            .with_context(|| format!("required secret environment variable `{variable}` is not set")),
        SecretSource::Keyring => keyring_entry(profile_name, field)?
            .get_password()
            .with_context(|| {
                format!(
                    "could not read `{field}` for profile `{profile_name}` from the OS credential manager"
                )
            }),
    }
}

fn keyring_entry(profile_name: &str, field: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, &format!("{profile_name}/{field}"))
        .context("could not open the OS credential manager")
}

fn validate_profile_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        bail!("profile names must be 1-64 ASCII letters, digits, `.`, `_`, or `-`");
    }
    Ok(())
}

fn validate_display_name(name: &str, kind: &str) -> Result<()> {
    if name.trim().is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
        bail!("{kind} names must be 1-80 printable characters");
    }
    Ok(())
}

fn validate_header_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.ends_with("-bin")
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        bail!("invalid gRPC header name `{name}`");
    }
    Ok(())
}

fn validate_temporal_address(address: &str) -> Result<()> {
    let address = address.trim();
    if address.is_empty() {
        bail!("address must not be empty");
    }
    if address.chars().any(char::is_control) {
        bail!("address must not contain control characters");
    }
    let normalized = if address.contains("://") {
        address.to_string()
    } else {
        format!("http://{address}")
    };
    let parsed = Url::parse(&normalized).context("address is not a valid Temporal URL")?;
    if !matches!(parsed.scheme(), "http" | "https") {
        bail!("address must use http or https when a scheme is supplied");
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        bail!("address must not contain credentials");
    }
    if parsed.path() != "/" || parsed.query().is_some() || parsed.fragment().is_some() {
        bail!("address must not contain a path, query, or fragment");
    }
    Ok(())
}

fn contains_invalid_header_bytes(value: &str) -> bool {
    value
        .bytes()
        .any(|byte| matches!(byte, b'\0' | b'\r' | b'\n'))
}

fn looks_sensitive_header_name(name: &str) -> bool {
    ["authorization", "cookie", "token", "secret", "api-key"]
        .iter()
        .any(|needle| name.contains(needle))
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn store_in(directory: &TempDir) -> ConfigStore {
        ConfigStore {
            path: directory.path().join("config.toml"),
        }
    }

    #[test]
    fn config_round_trip_never_contains_secret_material() {
        let directory = TempDir::new().unwrap();
        let store = store_in(&directory);
        let mut config = UserConfig {
            default_profile: Some("cloud".to_string()),
            ..UserConfig::default()
        };
        config.profiles.insert(
            "cloud".to_string(),
            ConnectionProfile {
                address: "example.tmprl.cloud:7233".to_string(),
                namespace: "production.a1b2c".to_string(),
                api_key: Some(SecretSource::Env {
                    variable: "TEMPORAL_PRODUCTION_API_KEY".to_string(),
                }),
                ..ConnectionProfile::default()
            },
        );
        config.filters.insert(
            "failed today".to_string(),
            SavedFilter {
                query: "ExecutionStatus = 'Failed'".to_string(),
            },
        );

        store.save(&config).unwrap();
        let raw = fs::read_to_string(store.path()).unwrap();
        assert!(!raw.contains("actual-api-key"));
        assert!(raw.contains("TEMPORAL_PRODUCTION_API_KEY"));
        assert_eq!(store.load().unwrap(), config);
    }

    #[test]
    fn migrates_schema_one_once_and_keeps_an_exact_private_backup() {
        let directory = TempDir::new().unwrap();
        let store = store_in(&directory);
        let legacy = "schema_version = 1\ndefault_profile = \"dev\"\n\n[profiles.dev]\naddress = \"127.0.0.1:7233\"\nnamespace = \"default\"\n";
        fs::write(store.path(), legacy).unwrap();

        let migrated = store.load().unwrap();
        assert_eq!(migrated.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(migrated.default_profile.as_deref(), Some("dev"));
        assert_eq!(migrated.ui, UiPreferences::default());

        let backup = store.path().with_extension("toml.v1.bak");
        assert_eq!(fs::read(&backup).unwrap(), legacy.as_bytes());
        let persisted = fs::read_to_string(store.path()).unwrap();
        assert!(persisted.contains("schema_version = 2"));
        assert!(persisted.contains("[ui]"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            assert_eq!(
                fs::metadata(store.path()).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(&backup).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }

        let backup_before = fs::read(&backup).unwrap();
        assert_eq!(store.load().unwrap(), migrated);
        assert_eq!(fs::read(&backup).unwrap(), backup_before);

        fs::write(store.path(), legacy).unwrap();
        assert_eq!(store.load().unwrap(), migrated);
        assert_eq!(fs::read(&backup).unwrap(), backup_before);
    }

    #[test]
    fn refuses_to_overwrite_a_conflicting_migration_backup() {
        let directory = TempDir::new().unwrap();
        let store = store_in(&directory);
        let legacy = "schema_version = 1\n";
        fs::write(store.path(), legacy).unwrap();
        fs::write(
            store.path().with_extension("toml.v1.bak"),
            "different original",
        )
        .unwrap();

        let error = store.load().unwrap_err().to_string();
        assert!(error.contains("already exists with different contents"));
        assert_eq!(fs::read_to_string(store.path()).unwrap(), legacy);
    }

    #[test]
    fn rejects_missing_newer_and_invalid_ui_schemas() {
        let directory = TempDir::new().unwrap();
        let store = store_in(&directory);

        fs::write(store.path(), "default_profile = \"dev\"\n").unwrap();
        assert!(
            store
                .load()
                .unwrap_err()
                .to_string()
                .contains("missing an integer schema_version")
        );

        fs::write(store.path(), "schema_version = 99\n").unwrap();
        assert!(
            store
                .load()
                .unwrap_err()
                .to_string()
                .contains("newer than supported")
        );

        fs::write(store.path(), "schema_version = 2\n\n[ui]\npage_size = 0\n").unwrap();
        assert!(format!("{:#}", store.load().unwrap_err()).contains("ui.page_size"));
    }

    #[cfg(unix)]
    #[test]
    fn atomic_save_repairs_insecure_existing_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let directory = TempDir::new().unwrap();
        let store = store_in(&directory);
        fs::write(store.path(), "obsolete").unwrap();
        fs::set_permissions(store.path(), fs::Permissions::from_mode(0o644)).unwrap();

        store.save(&UserConfig::default()).unwrap();

        assert_eq!(
            fs::metadata(store.path()).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn rejects_plaintext_sensitive_headers() {
        let profile = ConnectionProfile {
            headers: BTreeMap::from([("authorization".to_string(), "Bearer secret".to_string())]),
            ..ConnectionProfile::default()
        };
        assert!(profile.validate().is_err());

        let profile = ConnectionProfile {
            payload_codec: Some(ProfilePayloadCodec {
                endpoint: "https://codec.example".to_string(),
                headers: BTreeMap::from([(
                    "authorization".to_string(),
                    "Bearer secret".to_string(),
                )]),
                ..ProfilePayloadCodec::default()
            }),
            ..ConnectionProfile::default()
        };
        assert!(profile.validate().is_err());
    }

    #[test]
    fn rejects_credentials_and_control_bytes_in_rendered_connection_fields() {
        let profile = ConnectionProfile {
            address: "https://operator:secret@temporal.example:7233".to_string(),
            ..ConnectionProfile::default()
        };
        assert!(
            profile
                .validate()
                .unwrap_err()
                .to_string()
                .contains("credentials")
        );

        let profile = ConnectionProfile {
            web_ui_url: Some("https://operator:secret@ui.example".to_string()),
            ..ConnectionProfile::default()
        };
        assert!(
            profile
                .validate()
                .unwrap_err()
                .to_string()
                .contains("credentials")
        );

        let profile = ConnectionProfile {
            headers: BTreeMap::from([("x-owner".to_string(), "safe\r\ninjected".to_string())]),
            ..ConnectionProfile::default()
        };
        assert!(profile.validate().unwrap_err().to_string().contains("CR"));
    }

    #[test]
    fn validates_tls_pairs_and_default_profile() {
        let mut config = UserConfig {
            default_profile: Some("missing".to_string()),
            ..UserConfig::default()
        };
        assert!(config.validate().is_err());

        config.default_profile = Some("broken".to_string());
        config.profiles.insert(
            "broken".to_string(),
            ConnectionProfile {
                tls: ProfileTls {
                    client_certificate: Some("client.pem".into()),
                    ..ProfileTls::default()
                },
                ..ConnectionProfile::default()
            },
        );
        assert!(config.validate().is_err());
    }

    #[test]
    fn resolves_environment_backed_secrets() {
        let store = ConfigStore {
            path: PathBuf::from("unused"),
        };
        let variable = "PATH".to_string();
        let expected = std::env::var(&variable).expect("test process should have PATH");
        let mut config = UserConfig::default();
        config.profiles.insert(
            "dev".to_string(),
            ConnectionProfile {
                api_key: Some(SecretSource::Env {
                    variable: variable.clone(),
                }),
                payload_codec: Some(ProfilePayloadCodec {
                    endpoint: "https://codec.example/{namespace}".to_string(),
                    secret_headers: BTreeMap::from([(
                        "authorization".to_string(),
                        SecretSource::Env {
                            variable: variable.clone(),
                        },
                    )]),
                    ..ProfilePayloadCodec::default()
                }),
                ..ConnectionProfile::default()
            },
        );
        let resolved = store.resolve_profile(&config, "dev").unwrap();
        assert_eq!(
            resolved.connection.api_key.as_deref(),
            Some(expected.as_str())
        );
        let codec = resolved.connection.payload_codec.unwrap();
        assert_eq!(codec.endpoint, "https://codec.example/{namespace}");
        assert_eq!(
            codec.headers.get("authorization").map(String::as_str),
            Some(expected.as_str())
        );
    }
}
