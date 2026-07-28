//! Native authentication for a `temporal-local-auth` protected Temporal endpoint.
//!
//! Access tokens live only in memory. The OS credential manager stores a
//! rotating refresh token and its fixed expiry under
//! `io.temporal.temporal-tui/temporal-auth/v1/<profile-binding>`.

use std::{
    collections::HashMap,
    fmt,
    fs::{self, File, OpenOptions, TryLockError},
    io,
    net::IpAddr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex as StdMutex, OnceLock},
    time::Duration,
};

use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use directories::BaseDirs;
use futures_util::StreamExt;
use reqwest::{Client, RequestBuilder, StatusCode, redirect::Policy};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::sync::Mutex;
use url::{Host, Url};
use zeroize::Zeroizing;

const KEYRING_SERVICE: &str = "io.temporal.temporal-tui";
const KEYRING_FIELD: &str = "temporal-auth";
const OAUTH_CLIENT_ID: &str = "temporal-cli";
const MAX_RESPONSE_BYTES: usize = 1 << 20;
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);
const REFRESH_EARLY: TimeDelta = TimeDelta::seconds(60);
const MIN_ACCESS_LIFETIME: TimeDelta = TimeDelta::seconds(30);
const LOCK_WAIT: Duration = Duration::from_secs(20);
const LOCK_POLL: Duration = Duration::from_millis(50);
const BINDING_DOMAIN: &[u8] = b"temporal-tui/auth-credential/v1";
const MAX_USERINFO_SUB_CHARS: usize = 512;
const MAX_USERINFO_NAME_CHARS: usize = 256;
const MAX_USERINFO_USERNAME_CHARS: usize = 320;
const MAX_USERINFO_EMAIL_CHARS: usize = 320;
const MAX_USERINFO_PERMISSIONS: usize = 256;
const MAX_USERINFO_PERMISSION_CHARS: usize = 256;

/// Non-secret metadata required to resume an authenticated session.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalAuthProfile {
    /// Auth service base URL.
    pub url: String,
    /// Account name used during login. This is metadata, never a password.
    pub username: String,
    /// Same-origin OAuth token endpoint advertised by the auth service.
    pub token_endpoint: String,
    /// Permit cleartext HTTP only when every endpoint involved is loopback.
    #[serde(default)]
    pub allow_insecure: bool,
}

impl fmt::Debug for TemporalAuthProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TemporalAuthProfile")
            .field("url", &self.url)
            .field("username", &"[redacted]")
            .field("token_endpoint", &self.token_endpoint)
            .field("allow_insecure", &self.allow_insecure)
            .finish()
    }
}

impl TemporalAuthProfile {
    /// Validate URL, origin, username, and local-development security rules.
    ///
    /// # Errors
    ///
    /// Returns an error when the profile would send credentials to an
    /// untrusted or malformed endpoint.
    pub fn validate(&self) -> AuthResult<()> {
        validate_username(&self.username)?;
        let base = parse_auth_url(&self.url, self.allow_insecure)?;
        let token_endpoint = parse_endpoint_url(&self.token_endpoint, self.allow_insecure)?;
        ensure_same_origin(&base, &token_endpoint)?;
        Ok(())
    }
}

/// Public identity returned by `/oauth/userinfo`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct UserInfo {
    pub sub: String,
    pub name: String,
    pub preferred_username: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
}

/// Result of a password login.
#[derive(Clone)]
pub struct LoginResult {
    pub session: AuthSession,
    pub profile: TemporalAuthProfile,
    pub temporal_address: String,
    pub temporal_tls: bool,
}

impl fmt::Debug for LoginResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoginResult")
            .field("session", &"[redacted]")
            .field("profile", &self.profile)
            .field("temporal_address", &self.temporal_address)
            .field("temporal_tls", &self.temporal_tls)
            .finish()
    }
}

/// A fixed-message credential-store failure that cannot disclose a secret.
#[derive(Debug, Clone, Copy, Error)]
#[error("credential manager operation failed")]
pub struct CredentialStoreError;

/// Injectable storage boundary for the OS credential manager.
pub trait CredentialStore: Send + Sync {
    /// Read a credential, returning `None` when it is absent.
    ///
    /// # Errors
    ///
    /// Returns an opaque error when the credential manager cannot be accessed.
    fn get(&self, service: &str, item: &str) -> Result<Option<String>, CredentialStoreError>;

    /// Store or replace a credential.
    ///
    /// # Errors
    ///
    /// Returns an opaque error when the credential manager rejects the write.
    fn set(&self, service: &str, item: &str, value: &str) -> Result<(), CredentialStoreError>;

    /// Delete a credential. Implementations should treat absence as success.
    ///
    /// # Errors
    ///
    /// Returns an opaque error when the credential manager rejects the deletion.
    fn delete(&self, service: &str, item: &str) -> Result<(), CredentialStoreError>;
}

/// Production credential store backed by the platform credential manager.
#[derive(Debug, Default)]
pub struct OsCredentialStore;

impl CredentialStore for OsCredentialStore {
    fn get(&self, service: &str, item: &str) -> Result<Option<String>, CredentialStoreError> {
        let entry = keyring::Entry::new(service, item).map_err(|_| CredentialStoreError)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(CredentialStoreError),
        }
    }

    fn set(&self, service: &str, item: &str, value: &str) -> Result<(), CredentialStoreError> {
        keyring::Entry::new(service, item)
            .and_then(|entry| entry.set_password(value))
            .map_err(|_| CredentialStoreError)
    }

    fn delete(&self, service: &str, item: &str) -> Result<(), CredentialStoreError> {
        let entry = keyring::Entry::new(service, item).map_err(|_| CredentialStoreError)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(CredentialStoreError),
        }
    }
}

/// In-memory credential store for deterministic tests and embedding.
#[derive(Default)]
pub struct MemoryCredentialStore {
    values: StdMutex<HashMap<(String, String), String>>,
    failures: StdMutex<StoreFailures>,
}

#[derive(Debug, Default)]
struct StoreFailures {
    get: bool,
    set: bool,
    delete: bool,
}

impl fmt::Debug for MemoryCredentialStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let entry_count = self.values.lock().map_or(0, |values| values.len());
        formatter
            .debug_struct("MemoryCredentialStore")
            .field("entry_count", &entry_count)
            .finish_non_exhaustive()
    }
}

impl MemoryCredentialStore {
    /// Make selected operations fail. Intended for deterministic failure tests.
    pub fn set_failures(&self, get: bool, set: bool, delete: bool) {
        if let Ok(mut failures) = self.failures.lock() {
            *failures = StoreFailures { get, set, delete };
        }
    }
}

impl CredentialStore for MemoryCredentialStore {
    fn get(&self, service: &str, item: &str) -> Result<Option<String>, CredentialStoreError> {
        if self.failures.lock().map_err(|_| CredentialStoreError)?.get {
            return Err(CredentialStoreError);
        }
        Ok(self
            .values
            .lock()
            .map_err(|_| CredentialStoreError)?
            .get(&(service.to_owned(), item.to_owned()))
            .cloned())
    }

    fn set(&self, service: &str, item: &str, value: &str) -> Result<(), CredentialStoreError> {
        if self.failures.lock().map_err(|_| CredentialStoreError)?.set {
            return Err(CredentialStoreError);
        }
        self.values
            .lock()
            .map_err(|_| CredentialStoreError)?
            .insert((service.to_owned(), item.to_owned()), value.to_owned());
        Ok(())
    }

    fn delete(&self, service: &str, item: &str) -> Result<(), CredentialStoreError> {
        if self
            .failures
            .lock()
            .map_err(|_| CredentialStoreError)?
            .delete
        {
            return Err(CredentialStoreError);
        }
        self.values
            .lock()
            .map_err(|_| CredentialStoreError)?
            .remove(&(service.to_owned(), item.to_owned()));
        Ok(())
    }
}

/// Safe authentication errors. Response bodies and credentials are never
/// included in error text.
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("{0}")]
    InvalidProfile(&'static str),
    #[error("profile `{0}` is not logged in")]
    NotLoggedIn(String),
    #[error("profile `{0}` already has a login credential; log out first")]
    AlreadyLoggedIn(String),
    #[error("invalid username or password")]
    InvalidCredentials,
    #[error("could not initialize the authentication HTTP client")]
    HttpClient(#[source] reqwest::Error),
    #[error("{operation} request failed")]
    Request {
        operation: &'static str,
        #[source]
        source: reqwest::Error,
    },
    #[error("{operation} endpoint returned HTTP {status}")]
    Status {
        operation: &'static str,
        status: StatusCode,
    },
    #[error("{0}")]
    InvalidResponse(&'static str),
    #[error("authentication response is larger than 1 MiB")]
    ResponseTooLarge,
    #[error("could not decode the authentication response")]
    Decode(#[source] serde_json::Error),
    #[error("could not read the authentication credential")]
    CredentialRead(#[source] CredentialStoreError),
    #[error("could not persist the rotated authentication credential")]
    CredentialWrite(#[source] CredentialStoreError),
    #[error("the refresh grant was revoked, but its local credential could not be removed")]
    CredentialDelete(#[source] CredentialStoreError),
    #[error("could not initialize authentication coordination")]
    CoordinationUnavailable,
    #[error("could not coordinate authentication credential rotation")]
    Coordination(#[source] io::Error),
    #[error("another temporal-tui process is updating this login; retry")]
    CoordinationBusy,
}

pub type AuthResult<T> = Result<T, AuthError>;

impl AuthError {
    /// Whether retrying the same operation may succeed without a new login or
    /// profile change.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Request { .. }
            | Self::CredentialRead(_)
            | Self::CredentialWrite(_)
            | Self::CredentialDelete(_)
            | Self::Coordination(_)
            | Self::CoordinationBusy => true,
            Self::Status { status, .. } => {
                matches!(
                    *status,
                    StatusCode::REQUEST_TIMEOUT
                        | StatusCode::TOO_EARLY
                        | StatusCode::TOO_MANY_REQUESTS
                ) || status.is_server_error()
            }
            Self::InvalidProfile(_)
            | Self::NotLoggedIn(_)
            | Self::AlreadyLoggedIn(_)
            | Self::InvalidCredentials
            | Self::HttpClient(_)
            | Self::InvalidResponse(_)
            | Self::ResponseTooLarge
            | Self::Decode(_)
            | Self::CoordinationUnavailable => false,
        }
    }
}

#[async_trait]
trait CredentialCoordinator: Send + Sync {
    async fn acquire(&self, binding: &str) -> AuthResult<Box<dyn CredentialLockGuard>>;
}

trait CredentialLockGuard: Send {}

#[derive(Debug)]
struct FileCredentialCoordinator {
    root: PathBuf,
}

impl FileCredentialCoordinator {
    fn discover() -> AuthResult<Self> {
        let base = BaseDirs::new().ok_or(AuthError::CoordinationUnavailable)?;
        Ok(Self {
            root: base
                .data_local_dir()
                .join("temporal-tui")
                .join("auth-locks"),
        })
    }

    #[cfg(test)]
    fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn open_lock_file(&self, binding: &str) -> AuthResult<File> {
        create_private_lock_directory(&self.root)?;
        let path = self.root.join(format!("{binding}.lock"));
        reject_symlink(&path)?;

        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&path).map_err(AuthError::Coordination)?;
        if !file.metadata().map_err(AuthError::Coordination)?.is_file() {
            return Err(AuthError::Coordination(io::Error::new(
                io::ErrorKind::InvalidInput,
                "authentication lock is not a regular file",
            )));
        }
        #[cfg(unix)]
        fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o600))
            .map_err(AuthError::Coordination)?;
        Ok(file)
    }
}

#[async_trait]
impl CredentialCoordinator for FileCredentialCoordinator {
    async fn acquire(&self, binding: &str) -> AuthResult<Box<dyn CredentialLockGuard>> {
        let file = self.open_lock_file(binding)?;
        let deadline = tokio::time::Instant::now() + LOCK_WAIT;
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Box::new(FileCredentialLock { file })),
                Err(TryLockError::WouldBlock) if tokio::time::Instant::now() >= deadline => {
                    return Err(AuthError::CoordinationBusy);
                }
                Err(TryLockError::WouldBlock) => tokio::time::sleep(LOCK_POLL).await,
                Err(TryLockError::Error(error)) => {
                    return Err(AuthError::Coordination(error));
                }
            }
        }
    }
}

struct FileCredentialLock {
    file: File,
}

impl CredentialLockGuard for FileCredentialLock {}

impl Drop for FileCredentialLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

#[derive(Default)]
struct InProcessCredentialCoordinator {
    locks: StdMutex<HashMap<String, Arc<Mutex<()>>>>,
}

struct InProcessCredentialLock {
    _guard: tokio::sync::OwnedMutexGuard<()>,
}

impl CredentialLockGuard for InProcessCredentialLock {}

#[async_trait]
impl CredentialCoordinator for InProcessCredentialCoordinator {
    async fn acquire(&self, binding: &str) -> AuthResult<Box<dyn CredentialLockGuard>> {
        let lock = {
            let mut locks = self.locks.lock().map_err(|_| {
                AuthError::Coordination(io::Error::other(
                    "authentication lock registry was poisoned",
                ))
            })?;
            Arc::clone(
                locks
                    .entry(binding.to_owned())
                    .or_insert_with(|| Arc::new(Mutex::new(()))),
            )
        };
        Ok(Box::new(InProcessCredentialLock {
            _guard: lock.lock_owned().await,
        }))
    }
}

fn in_process_coordinator() -> Arc<dyn CredentialCoordinator> {
    static COORDINATOR: OnceLock<Arc<InProcessCredentialCoordinator>> = OnceLock::new();
    COORDINATOR
        .get_or_init(|| Arc::new(InProcessCredentialCoordinator::default()))
        .clone()
}

fn create_private_lock_directory(path: &Path) -> AuthResult<()> {
    reject_symlink(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

        let mut builder = fs::DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder.create(path).map_err(AuthError::Coordination)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(AuthError::Coordination)?;
    }
    #[cfg(not(unix))]
    fs::create_dir_all(path).map_err(AuthError::Coordination)?;
    reject_symlink(path)?;
    Ok(())
}

fn reject_symlink(path: &Path) -> AuthResult<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(AuthError::Coordination(io::Error::new(
                io::ErrorKind::InvalidInput,
                "authentication lock path must not be a symbolic link",
            )))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AuthError::Coordination(error)),
    }
}

#[derive(Clone)]
pub struct AuthSession {
    inner: Arc<AuthSessionInner>,
}

impl fmt::Debug for AuthSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthSession")
            .field("profile_name", &self.inner.profile_name)
            .field("profile", &self.inner.profile)
            .field("credentials", &"[redacted]")
            .finish()
    }
}

struct AuthSessionInner {
    profile_name: String,
    credential_binding: String,
    keyring_item: String,
    profile: TemporalAuthProfile,
    client: Client,
    store: Arc<dyn CredentialStore>,
    coordinator: Arc<dyn CredentialCoordinator>,
    state: Mutex<SessionState>,
}

struct SessionState {
    refresh: Option<RefreshCredential>,
    access: Option<AccessCredential>,
    pending: Option<PendingCredential>,
}

struct AccessCredential {
    token: Zeroizing<String>,
    expires_at: DateTime<Utc>,
    refresh_early: TimeDelta,
}

struct RefreshCredential {
    token: Zeroizing<String>,
    expires_at: DateTime<Utc>,
}

struct PendingCredential {
    refresh: RefreshCredential,
    access: AccessCredential,
    _lock: Box<dyn CredentialLockGuard>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedCredential {
    binding_sha256: String,
    refresh_token: String,
    refresh_expires_at: i64,
}

#[derive(Serialize)]
struct PersistedCredentialRef<'a> {
    binding_sha256: &'a str,
    refresh_token: &'a str,
    refresh_expires_at: i64,
}

#[derive(Serialize)]
struct LoginRequest<'a> {
    username: &'a str,
    password: &'a str,
}

#[derive(Deserialize)]
struct LoginResponse {
    access_token: String,
    refresh_token: String,
    token_type: String,
    expires_in: i64,
    refresh_expires_at: i64,
    #[serde(default)]
    token_endpoint: String,
    temporal: Option<TemporalEndpoint>,
}

#[derive(Deserialize)]
struct RefreshResponse {
    access_token: String,
    refresh_token: String,
    token_type: String,
    expires_in: i64,
    refresh_expires_at: i64,
}

#[derive(Deserialize)]
struct TemporalEndpoint {
    address: String,
    tls: bool,
}

impl AuthSession {
    /// Log in through the password endpoint and persist only the refresh
    /// credential in the OS credential manager.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid metadata, rejected credentials, an unsafe
    /// advertised endpoint, malformed tokens, or credential-manager failure.
    pub async fn login(
        profile_name: &str,
        auth_url: &str,
        username: &str,
        password: &str,
        address_override: Option<&str>,
        allow_insecure: bool,
    ) -> AuthResult<LoginResult> {
        Self::login_with_components(
            profile_name,
            auth_url,
            username,
            password,
            address_override,
            allow_insecure,
            Arc::new(OsCredentialStore),
            Arc::new(FileCredentialCoordinator::discover()?),
        )
        .await
    }

    /// Login variant with an injectable credential store.
    ///
    /// # Errors
    ///
    /// Returns the same safe validation, protocol, and storage errors as
    /// [`Self::login`].
    pub async fn login_with_store(
        profile_name: &str,
        auth_url: &str,
        username: &str,
        password: &str,
        address_override: Option<&str>,
        allow_insecure: bool,
        store: Arc<dyn CredentialStore>,
    ) -> AuthResult<LoginResult> {
        Self::login_with_components(
            profile_name,
            auth_url,
            username,
            password,
            address_override,
            allow_insecure,
            store,
            in_process_coordinator(),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn login_with_components(
        profile_name: &str,
        auth_url: &str,
        username: &str,
        password: &str,
        address_override: Option<&str>,
        allow_insecure: bool,
        store: Arc<dyn CredentialStore>,
        coordinator: Arc<dyn CredentialCoordinator>,
    ) -> AuthResult<LoginResult> {
        validate_profile_name(profile_name)?;
        validate_username(username)?;
        if password.is_empty() || password.len() > MAX_RESPONSE_BYTES {
            return Err(AuthError::InvalidProfile(
                "password must be between 1 byte and 1 MiB",
            ));
        }
        let base_url = parse_auth_url(auth_url, allow_insecure)?;
        let client = build_http_client()?;
        let login_url = child_endpoint(&base_url, "auth/token");
        let request = client
            .post(login_url)
            .header(reqwest::header::ACCEPT, "application/json")
            .json(&LoginRequest { username, password });
        let (status, body) = execute(request, "password login").await?;
        if status == StatusCode::UNAUTHORIZED {
            return Err(AuthError::InvalidCredentials);
        }
        if status != StatusCode::OK {
            return Err(AuthError::Status {
                operation: "password login",
                status,
            });
        }
        let mut response: LoginResponse = decode_json(&body)?;
        let token_endpoint =
            resolve_advertised_token_endpoint(&base_url, &response.token_endpoint)?;
        let temporal = if let Some(address) = address_override {
            TemporalEndpoint {
                address: address.to_owned(),
                tls: true,
            }
        } else {
            response.temporal.take().ok_or(AuthError::InvalidResponse(
                "server did not advertise a Temporal endpoint; provide an address override",
            ))?
        };
        let temporal_is_loopback = validate_temporal_address(&temporal.address)?;
        if !(temporal.tls || allow_insecure && temporal_is_loopback) {
            return Err(AuthError::InvalidResponse(
                "server advertised an insecure Temporal endpoint",
            ));
        }

        let now = Utc::now();
        let access = access_credential(
            std::mem::take(&mut response.access_token),
            &response.token_type,
            response.expires_in,
            now,
        )?;
        let refresh = refresh_credential(
            std::mem::take(&mut response.refresh_token),
            response.refresh_expires_at,
            now,
        )?;
        let profile = TemporalAuthProfile {
            url: canonical_base_url(base_url),
            username: username.to_owned(),
            token_endpoint: token_endpoint.to_string(),
            allow_insecure,
        };
        profile.validate()?;
        let credential_binding = credential_binding(profile_name, &profile)?;
        let keyring_item = credential_item(&credential_binding);
        let _lock = coordinator.acquire(&credential_binding).await?;
        if store
            .get(KEYRING_SERVICE, &keyring_item)
            .map_err(AuthError::CredentialRead)?
            .is_some()
        {
            revoke_credential(&client, &token_endpoint, &refresh).await?;
            return Err(AuthError::AlreadyLoggedIn(profile_name.to_owned()));
        }
        if let Err(error) =
            persist_refresh(store.as_ref(), &keyring_item, &credential_binding, &refresh)
        {
            let _ = revoke_credential(&client, &token_endpoint, &refresh).await;
            return Err(error);
        }

        let session = Self {
            inner: Arc::new(AuthSessionInner {
                profile_name: profile_name.to_owned(),
                credential_binding,
                keyring_item,
                profile: profile.clone(),
                client,
                store,
                coordinator,
                state: Mutex::new(SessionState {
                    refresh: Some(refresh),
                    access: Some(access),
                    pending: None,
                }),
            }),
        };
        Ok(LoginResult {
            session,
            profile,
            temporal_address: temporal.address,
            temporal_tls: temporal.tls,
        })
    }

    /// Load a session from the OS credential manager. No access token is read
    /// from disk; the first [`Self::access_token`] call refreshes it.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid profile, a missing or malformed stored
    /// credential, or an unavailable credential manager.
    pub fn load(profile_name: &str, profile: TemporalAuthProfile) -> AuthResult<Self> {
        Self::load_with_components(
            profile_name,
            profile,
            Arc::new(OsCredentialStore),
            Arc::new(FileCredentialCoordinator::discover()?),
        )
    }

    /// Load variant with an injectable credential store.
    ///
    /// # Errors
    ///
    /// Returns the same safe validation and storage errors as [`Self::load`].
    pub fn load_with_store(
        profile_name: &str,
        profile: TemporalAuthProfile,
        store: Arc<dyn CredentialStore>,
    ) -> AuthResult<Self> {
        Self::load_with_components(profile_name, profile, store, in_process_coordinator())
    }

    fn load_with_components(
        profile_name: &str,
        profile: TemporalAuthProfile,
        store: Arc<dyn CredentialStore>,
        coordinator: Arc<dyn CredentialCoordinator>,
    ) -> AuthResult<Self> {
        validate_profile_name(profile_name)?;
        profile.validate()?;
        let credential_binding = credential_binding(profile_name, &profile)?;
        let keyring_item = credential_item(&credential_binding);
        let refresh = read_refresh(
            store.as_ref(),
            &keyring_item,
            &credential_binding,
            profile_name,
        )?;
        Ok(Self {
            inner: Arc::new(AuthSessionInner {
                profile_name: profile_name.to_owned(),
                credential_binding,
                keyring_item,
                profile,
                client: build_http_client()?,
                store,
                coordinator,
                state: Mutex::new(SessionState {
                    refresh: Some(refresh),
                    access: None,
                    pending: None,
                }),
            }),
        })
    }

    /// Return a current bearer token, refreshing at most once when concurrent
    /// callers observe a missing or nearly-expired token.
    ///
    /// # Errors
    ///
    /// Returns an error when the session is absent or expired, refresh is
    /// rejected, or the rotated refresh credential cannot be persisted.
    pub async fn access_token(&self) -> AuthResult<String> {
        let mut state = self.inner.state.lock().await;
        self.commit_pending(&mut state)?;
        let now = Utc::now();
        if let Some(access) = &state.access
            && access.expires_at > now + access.refresh_early
        {
            return Ok(access.token.to_string());
        }
        self.refresh_locked(&mut state).await
    }

    /// Rotate the refresh credential and return the new bearer token.
    ///
    /// # Errors
    ///
    /// Returns an error when the session is absent or expired, refresh is
    /// rejected, or the rotated refresh credential cannot be persisted.
    pub async fn force_refresh(&self) -> AuthResult<String> {
        let mut state = self.inner.state.lock().await;
        if state.pending.is_some() {
            self.commit_pending(&mut state)?;
            return state
                .access
                .as_ref()
                .map(|access| access.token.to_string())
                .ok_or_else(|| AuthError::NotLoggedIn(self.inner.profile_name.clone()));
        }
        self.refresh_locked(&mut state).await
    }

    /// Time until a proactive refresh should run. A session without an access
    /// token, or one already inside the early-refresh window, returns zero.
    pub async fn next_refresh_delay(&self) -> Duration {
        let state = self.inner.state.lock().await;
        let Some(access) = &state.access else {
            return Duration::ZERO;
        };
        let remaining = access.expires_at - access.refresh_early - Utc::now();
        remaining.to_std().unwrap_or(Duration::ZERO)
    }

    /// Resolve the authenticated account from `/oauth/userinfo`.
    ///
    /// # Errors
    ///
    /// Returns an error when refresh or userinfo fails or the identity response
    /// is malformed.
    pub async fn userinfo(&self) -> AuthResult<UserInfo> {
        let token = Zeroizing::new(self.access_token().await?);
        let token_endpoint = parse_endpoint_url(
            &self.inner.profile.token_endpoint,
            self.inner.profile.allow_insecure,
        )?;
        let endpoint = sibling_endpoint(&token_endpoint, "userinfo")?;
        let request = self
            .inner
            .client
            .get(endpoint)
            .header(reqwest::header::ACCEPT, "application/json")
            .bearer_auth(token.as_str());
        let (status, body) = execute(request, "OAuth userinfo").await?;
        if status != StatusCode::OK {
            return Err(AuthError::Status {
                operation: "OAuth userinfo",
                status,
            });
        }
        let identity: UserInfo = decode_json(&body)?;
        if !valid_identity_field(&identity.sub, MAX_USERINFO_SUB_CHARS, false)
            || !valid_identity_field(&identity.name, MAX_USERINFO_NAME_CHARS, false)
            || !valid_identity_field(
                &identity.preferred_username,
                MAX_USERINFO_USERNAME_CHARS,
                false,
            )
            || identity
                .email
                .as_deref()
                .is_some_and(|email| !valid_identity_field(email, MAX_USERINFO_EMAIL_CHARS, false))
            || identity.permissions.len() > MAX_USERINFO_PERMISSIONS
            || identity.permissions.iter().any(|permission| {
                !valid_identity_field(permission, MAX_USERINFO_PERMISSION_CHARS, false)
            })
        {
            return Err(AuthError::InvalidResponse(
                "OAuth userinfo returned an invalid identity",
            ));
        }
        Ok(identity)
    }

    /// Revoke the refresh grant and then remove its local credential. Local
    /// credentials are retained if server-side revocation fails.
    ///
    /// # Errors
    ///
    /// Returns an error when the grant is absent, revocation fails, or the
    /// revoked local credential cannot be deleted.
    pub async fn logout(&self) -> AuthResult<()> {
        let mut state = self.inner.state.lock().await;
        let _lock = if state.pending.is_some() {
            None
        } else {
            Some(
                self.inner
                    .coordinator
                    .acquire(&self.inner.credential_binding)
                    .await?,
            )
        };
        if state.pending.is_none() {
            match self.read_current_refresh() {
                Ok(refresh) => state.refresh = Some(refresh),
                Err(error) => {
                    state.access = None;
                    state.refresh = None;
                    return Err(error);
                }
            }
        }
        let refresh = state
            .pending
            .as_ref()
            .map(|pending| &pending.refresh)
            .or(state.refresh.as_ref())
            .ok_or_else(|| AuthError::NotLoggedIn(self.inner.profile_name.clone()))?;
        let token_endpoint = parse_endpoint_url(
            &self.inner.profile.token_endpoint,
            self.inner.profile.allow_insecure,
        )?;
        revoke_credential(&self.inner.client, &token_endpoint, refresh).await?;
        state.access = None;
        let delete_result = self
            .inner
            .store
            .delete(KEYRING_SERVICE, &self.inner.keyring_item)
            .map_err(AuthError::CredentialDelete);
        state.refresh = None;
        state.pending = None;
        delete_result
    }

    #[must_use]
    pub fn profile(&self) -> &TemporalAuthProfile {
        &self.inner.profile
    }

    #[must_use]
    pub fn profile_name(&self) -> &str {
        &self.inner.profile_name
    }

    async fn refresh_locked(&self, state: &mut SessionState) -> AuthResult<String> {
        let lock = self
            .inner
            .coordinator
            .acquire(&self.inner.credential_binding)
            .await?;
        let refresh = match self.read_current_refresh() {
            Ok(refresh) => refresh,
            Err(error) => {
                state.access = None;
                state.refresh = None;
                return Err(error);
            }
        };
        if refresh.expires_at <= Utc::now() {
            return Err(AuthError::InvalidResponse(
                "the authentication session has expired; log in again",
            ));
        }
        let token_endpoint = parse_endpoint_url(
            &self.inner.profile.token_endpoint,
            self.inner.profile.allow_insecure,
        )?;
        let form = [
            ("grant_type", "refresh_token"),
            ("client_id", OAUTH_CLIENT_ID),
            ("refresh_token", refresh.token.as_str()),
        ];
        let request = self
            .inner
            .client
            .post(token_endpoint)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(encode_form(&form));
        let (status, body) = execute(request, "OAuth refresh").await?;
        if status != StatusCode::OK {
            return Err(AuthError::Status {
                operation: "OAuth refresh",
                status,
            });
        }
        let mut response: RefreshResponse = decode_json(&body)?;
        if response.refresh_expires_at != refresh.expires_at.timestamp() {
            return Err(AuthError::InvalidResponse(
                "OAuth refresh changed the fixed refresh expiry",
            ));
        }
        let response_time = Utc::now();
        let new_access = access_credential(
            std::mem::take(&mut response.access_token),
            &response.token_type,
            response.expires_in,
            response_time,
        )?;
        let new_refresh = refresh_credential(
            std::mem::take(&mut response.refresh_token),
            response.refresh_expires_at,
            response_time,
        )?;
        state.pending = Some(PendingCredential {
            refresh: new_refresh,
            access: new_access,
            _lock: lock,
        });
        self.commit_pending(state)?;
        state
            .access
            .as_ref()
            .map(|access| access.token.to_string())
            .ok_or_else(|| AuthError::NotLoggedIn(self.inner.profile_name.clone()))
    }

    fn commit_pending(&self, state: &mut SessionState) -> AuthResult<()> {
        let Some(pending) = state.pending.as_ref() else {
            return Ok(());
        };
        persist_refresh(
            self.inner.store.as_ref(),
            &self.inner.keyring_item,
            &self.inner.credential_binding,
            &pending.refresh,
        )?;
        let pending = state.pending.take().expect("pending was checked above");
        state.refresh = Some(pending.refresh);
        state.access = Some(pending.access);
        Ok(())
    }

    fn read_current_refresh(&self) -> AuthResult<RefreshCredential> {
        read_refresh(
            self.inner.store.as_ref(),
            &self.inner.keyring_item,
            &self.inner.credential_binding,
            &self.inner.profile_name,
        )
    }
}

fn build_http_client() -> AuthResult<Client> {
    Client::builder()
        .timeout(HTTP_TIMEOUT)
        .redirect(Policy::none())
        .user_agent(concat!("temporal-tui/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(AuthError::HttpClient)
}

async fn execute(
    request: RequestBuilder,
    operation: &'static str,
) -> AuthResult<(StatusCode, Zeroizing<Vec<u8>>)> {
    let response = request
        .send()
        .await
        .map_err(|source| AuthError::Request { operation, source })?;
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(AuthError::ResponseTooLarge);
    }
    let mut bytes = Zeroizing::new(Vec::new());
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|source| AuthError::Request { operation, source })?;
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(AuthError::ResponseTooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok((status, bytes))
}

fn decode_json<T: DeserializeOwned>(body: &[u8]) -> AuthResult<T> {
    serde_json::from_slice(body).map_err(AuthError::Decode)
}

fn encode_form(values: &[(&str, &str)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer.extend_pairs(values.iter().copied());
    serializer.finish()
}

fn persist_refresh(
    store: &dyn CredentialStore,
    keyring_item: &str,
    credential_binding: &str,
    refresh: &RefreshCredential,
) -> AuthResult<()> {
    let persisted = PersistedCredentialRef {
        binding_sha256: credential_binding,
        refresh_token: refresh.token.as_str(),
        refresh_expires_at: refresh.expires_at.timestamp(),
    };
    let rendered = Zeroizing::new(serde_json::to_string(&persisted).map_err(AuthError::Decode)?);
    store
        .set(KEYRING_SERVICE, keyring_item, &rendered)
        .map_err(AuthError::CredentialWrite)
}

fn read_refresh(
    store: &dyn CredentialStore,
    keyring_item: &str,
    credential_binding: &str,
    profile_name: &str,
) -> AuthResult<RefreshCredential> {
    let raw = store
        .get(KEYRING_SERVICE, keyring_item)
        .map_err(AuthError::CredentialRead)?
        .ok_or_else(|| AuthError::NotLoggedIn(profile_name.to_owned()))?;
    let raw = Zeroizing::new(raw);
    let mut persisted: PersistedCredential =
        serde_json::from_str(&raw).map_err(AuthError::Decode)?;
    let refresh_token = Zeroizing::new(std::mem::take(&mut persisted.refresh_token));
    if persisted.binding_sha256 != credential_binding {
        return Err(AuthError::InvalidResponse(
            "stored authentication credential belongs to a different profile",
        ));
    }
    stored_refresh_credential(refresh_token, persisted.refresh_expires_at)
}

async fn revoke_credential(
    client: &Client,
    token_endpoint: &Url,
    refresh: &RefreshCredential,
) -> AuthResult<()> {
    let endpoint = sibling_endpoint(token_endpoint, "revoke")?;
    let form = [
        ("client_id", OAUTH_CLIENT_ID),
        ("token", refresh.token.as_str()),
        ("token_type_hint", "refresh_token"),
    ];
    let request = client
        .post(endpoint)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(encode_form(&form));
    let (status, _) = execute(request, "OAuth revocation").await?;
    if status != StatusCode::OK {
        return Err(AuthError::Status {
            operation: "OAuth revocation",
            status,
        });
    }
    Ok(())
}

fn access_credential(
    token: String,
    token_type: &str,
    expires_in: i64,
    now: DateTime<Utc>,
) -> AuthResult<AccessCredential> {
    let token = Zeroizing::new(token);
    validate_bearer_token(&token)?;
    if !token_type.eq_ignore_ascii_case("Bearer") {
        return Err(AuthError::InvalidResponse(
            "OAuth endpoint returned a non-Bearer token",
        ));
    }
    let lifetime = TimeDelta::try_seconds(expires_in).ok_or(AuthError::InvalidResponse(
        "OAuth endpoint returned an invalid access-token lifetime",
    ))?;
    if lifetime <= MIN_ACCESS_LIFETIME {
        return Err(AuthError::InvalidResponse(
            "OAuth endpoint returned an already expired access token",
        ));
    }
    let expires_at = now
        .checked_add_signed(lifetime)
        .ok_or(AuthError::InvalidResponse(
            "OAuth endpoint returned an invalid access-token lifetime",
        ))?;
    let refresh_early = TimeDelta::seconds((expires_in / 5).clamp(1, REFRESH_EARLY.num_seconds()));
    Ok(AccessCredential {
        token,
        expires_at,
        refresh_early,
    })
}

fn refresh_credential(
    token: String,
    expires_at: i64,
    now: DateTime<Utc>,
) -> AuthResult<RefreshCredential> {
    let credential = stored_refresh_credential(Zeroizing::new(token), expires_at)?;
    if credential.expires_at <= now + MIN_ACCESS_LIFETIME {
        return Err(AuthError::InvalidResponse(
            "OAuth endpoint returned an expired refresh token",
        ));
    }
    Ok(credential)
}

fn stored_refresh_credential(
    token: Zeroizing<String>,
    expires_at: i64,
) -> AuthResult<RefreshCredential> {
    validate_bearer_token(&token)?;
    let expires_at = DateTime::from_timestamp(expires_at, 0).ok_or(AuthError::InvalidResponse(
        "OAuth endpoint returned an invalid refresh-token expiry",
    ))?;
    Ok(RefreshCredential { token, expires_at })
}

fn validate_bearer_token(token: &str) -> AuthResult<()> {
    if token.is_empty()
        || token.len() > MAX_RESPONSE_BYTES
        || token.chars().any(char::is_whitespace)
        || token.chars().any(char::is_control)
    {
        return Err(AuthError::InvalidResponse(
            "OAuth endpoint returned an invalid token",
        ));
    }
    Ok(())
}

fn parse_auth_url(value: &str, allow_insecure: bool) -> AuthResult<Url> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AuthError::InvalidProfile(
            "authentication URL must not be empty",
        ));
    }
    let parsed = Url::parse(value)
        .map_err(|_| AuthError::InvalidProfile("authentication URL is invalid"))?;
    validate_web_url(&parsed, allow_insecure)?;
    Ok(parsed)
}

fn parse_endpoint_url(value: &str, allow_insecure: bool) -> AuthResult<Url> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AuthError::InvalidProfile(
            "OAuth token endpoint must not be empty",
        ));
    }
    let parsed = Url::parse(value)
        .map_err(|_| AuthError::InvalidProfile("OAuth token endpoint is invalid"))?;
    validate_web_url(&parsed, allow_insecure)?;
    if parsed.path().is_empty() || parsed.path() == "/" {
        return Err(AuthError::InvalidProfile(
            "OAuth token endpoint must include a path",
        ));
    }
    Ok(parsed)
}

fn validate_web_url(url: &Url, allow_insecure: bool) -> AuthResult<()> {
    if url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.cannot_be_a_base()
    {
        return Err(AuthError::InvalidProfile(
            "authentication URLs cannot contain credentials, a query, or a fragment",
        ));
    }
    match url.scheme() {
        "https" => Ok(()),
        "http" if allow_insecure && url.host().is_some_and(|host| host_is_loopback(&host)) => {
            Ok(())
        }
        _ => Err(AuthError::InvalidProfile(
            "authentication URLs must use HTTPS; insecure HTTP is restricted to explicit loopback development",
        )),
    }
}

fn canonical_base_url(mut url: Url) -> String {
    let path = url.path().trim_end_matches('/').to_owned();
    url.set_path(&path);
    url.to_string().trim_end_matches('/').to_owned()
}

fn child_endpoint(base: &Url, suffix: &str) -> Url {
    let mut result = base.clone();
    let path = format!(
        "{}/{}",
        base.path().trim_end_matches('/'),
        suffix.trim_start_matches('/')
    );
    result.set_path(&path);
    result.set_query(None);
    result.set_fragment(None);
    result
}

fn resolve_advertised_token_endpoint(base: &Url, value: &str) -> AuthResult<Url> {
    let endpoint = if value.trim().is_empty() {
        child_endpoint(base, "oauth/token")
    } else {
        base.join(value.trim()).map_err(|_| {
            AuthError::InvalidResponse("server advertised an invalid OAuth token endpoint")
        })?
    };
    validate_web_url(&endpoint, base.scheme() == "http")?;
    ensure_same_origin(base, &endpoint)?;
    if endpoint.path().is_empty() || endpoint.path() == "/" {
        return Err(AuthError::InvalidResponse(
            "server advertised an invalid OAuth token endpoint",
        ));
    }
    Ok(endpoint)
}

fn sibling_endpoint(token_endpoint: &Url, name: &str) -> AuthResult<Url> {
    let path = token_endpoint.path().trim_end_matches('/');
    let (parent, _) = path.rsplit_once('/').ok_or(AuthError::InvalidProfile(
        "OAuth token endpoint path is invalid",
    ))?;
    let mut result = token_endpoint.clone();
    result.set_path(&format!("{parent}/{name}"));
    result.set_query(None);
    result.set_fragment(None);
    Ok(result)
}

fn ensure_same_origin(left: &Url, right: &Url) -> AuthResult<()> {
    if left.origin() != right.origin() {
        return Err(AuthError::InvalidProfile(
            "OAuth token endpoint must use the same origin as the authentication URL",
        ));
    }
    Ok(())
}

fn host_is_loopback(host: &Host<&str>) -> bool {
    match host {
        Host::Domain(domain) => {
            domain.eq_ignore_ascii_case("localhost")
                || domain
                    .parse::<IpAddr>()
                    .is_ok_and(|address| address.is_loopback())
        }
        Host::Ipv4(address) => address.is_loopback(),
        Host::Ipv6(address) => address.is_loopback(),
    }
}

fn validate_temporal_address(address: &str) -> AuthResult<bool> {
    if address.is_empty()
        || address != address.trim()
        || address.contains("://")
        || address.chars().any(char::is_control)
    {
        return Err(AuthError::InvalidResponse(
            "server advertised an invalid Temporal host:port",
        ));
    }
    let parsed = Url::parse(&format!("tcp://{address}")).map_err(|_| {
        AuthError::InvalidResponse("server advertised an invalid Temporal host:port")
    })?;
    if parsed.host().is_none()
        || parsed.port().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || !matches!(parsed.path(), "" | "/")
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(AuthError::InvalidResponse(
            "server advertised an invalid Temporal host:port",
        ));
    }
    Ok(parsed.host().is_some_and(|host| host_is_loopback(&host)))
}

fn validate_profile_name(profile_name: &str) -> AuthResult<()> {
    if profile_name.is_empty()
        || profile_name.len() > 64
        || !profile_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(AuthError::InvalidProfile(
            "profile names must be 1-64 ASCII letters, digits, `.`, `_`, or `-`",
        ));
    }
    Ok(())
}

fn validate_username(username: &str) -> AuthResult<()> {
    if username.is_empty()
        || username != username.trim()
        || username.chars().count() > 320
        || username.chars().any(unsafe_terminal_char)
    {
        return Err(AuthError::InvalidProfile(
            "username must be 1-320 non-control characters without surrounding whitespace",
        ));
    }
    Ok(())
}

fn credential_binding(profile_name: &str, profile: &TemporalAuthProfile) -> AuthResult<String> {
    validate_profile_name(profile_name)?;
    profile.validate()?;
    let base = parse_auth_url(&profile.url, profile.allow_insecure)?;
    let endpoint = parse_endpoint_url(&profile.token_endpoint, profile.allow_insecure)?;
    ensure_same_origin(&base, &endpoint)?;

    let canonical_base = canonical_base_url(base);
    let canonical_endpoint = endpoint.to_string();
    let mut hasher = Sha256::new();
    hasher.update(BINDING_DOMAIN);
    update_binding_field(&mut hasher, profile_name);
    update_binding_field(&mut hasher, &canonical_base);
    update_binding_field(&mut hasher, &canonical_endpoint);
    update_binding_field(&mut hasher, &profile.username);
    let digest = hasher.finalize();
    let mut rendered = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        write!(rendered, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Ok(rendered)
}

fn update_binding_field(hasher: &mut Sha256, value: &str) {
    let length = u64::try_from(value.len()).expect("string length fits in u64");
    hasher.update(length.to_be_bytes());
    hasher.update(value.as_bytes());
}

fn credential_item(credential_binding: &str) -> String {
    format!("{KEYRING_FIELD}/v1/{credential_binding}")
}

/// Return the non-secret OS credential-manager item for an authentication
/// profile. This is primarily useful for black-box integration tests and
/// credential-store adapters.
///
/// # Errors
///
/// Returns an error when the profile name or authentication metadata is
/// invalid.
#[doc(hidden)]
pub fn credential_item_for_profile(
    profile_name: &str,
    profile: &TemporalAuthProfile,
) -> AuthResult<String> {
    credential_binding_for_profile(profile_name, profile).map(|binding| credential_item(&binding))
}

/// Return the non-secret SHA-256 binding stored alongside a refresh
/// credential.
///
/// # Errors
///
/// Returns an error when the profile name or authentication metadata is
/// invalid.
#[doc(hidden)]
pub fn credential_binding_for_profile(
    profile_name: &str,
    profile: &TemporalAuthProfile,
) -> AuthResult<String> {
    credential_binding(profile_name, profile)
}

fn valid_identity_field(value: &str, max_chars: usize, allow_empty: bool) -> bool {
    if !allow_empty && value.trim().is_empty() {
        return false;
    }
    let mut count = 0;
    for character in value.chars() {
        count += 1;
        if count > max_chars || unsafe_terminal_char(character) {
            return false;
        }
    }
    allow_empty || count > 0
}

fn unsafe_terminal_char(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::{Ipv4Addr, Ipv6Addr, TcpListener},
        sync::{
            Mutex as StdMutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        thread,
    };

    use serde_json::{Value, json};

    use super::*;

    #[allow(clippy::too_many_arguments)]
    async fn login_with_test_store(
        profile_name: &str,
        auth_url: &str,
        username: &str,
        password: &str,
        address_override: Option<&str>,
        allow_insecure: bool,
        store: Arc<dyn CredentialStore>,
    ) -> AuthResult<LoginResult> {
        AuthSession::login_with_components(
            profile_name,
            auth_url,
            username,
            password,
            address_override,
            allow_insecure,
            store,
            in_process_coordinator(),
        )
        .await
    }

    fn load_with_test_store(
        profile_name: &str,
        profile: TemporalAuthProfile,
        store: Arc<dyn CredentialStore>,
    ) -> AuthResult<AuthSession> {
        AuthSession::load_with_components(profile_name, profile, store, in_process_coordinator())
    }

    #[test]
    fn profile_debug_redacts_username_and_validation_enforces_origin() {
        let profile = TemporalAuthProfile {
            url: "https://auth.example.test/base".to_owned(),
            username: "private-user@example.test".to_owned(),
            token_endpoint: "https://auth.example.test/base/oauth/token".to_owned(),
            allow_insecure: false,
        };
        profile.validate().unwrap();
        let rendered = format!("{profile:?}");
        assert!(!rendered.contains("private-user"));
        assert!(rendered.contains("[redacted]"));

        let cross_origin = TemporalAuthProfile {
            token_endpoint: "https://attacker.example/oauth/token".to_owned(),
            ..profile
        };
        assert!(cross_origin.validate().is_err());
    }

    #[test]
    fn insecure_http_is_limited_to_explicit_loopback() {
        let mut profile = TemporalAuthProfile {
            url: "http://127.0.0.1:8080".to_owned(),
            username: "admin".to_owned(),
            token_endpoint: "http://127.0.0.1:8080/oauth/token".to_owned(),
            allow_insecure: false,
        };
        assert!(profile.validate().is_err());
        profile.allow_insecure = true;
        profile.validate().unwrap();
        profile.url = "http://example.test".to_owned();
        profile.token_endpoint = "http://example.test/oauth/token".to_owned();
        assert!(profile.validate().is_err());
    }

    #[tokio::test]
    async fn login_persists_only_refresh_credential() {
        let password_seen = Arc::new(AtomicBool::new(false));
        let password_seen_in_handler = Arc::clone(&password_seen);
        let server = TestServer::new(move |request| {
            assert_eq!(request.method, "POST");
            assert_eq!(request.path, "/auth/token");
            let body: Value = serde_json::from_slice(&request.body).unwrap();
            password_seen_in_handler.store(
                body["password"] == "correct horse battery staple",
                Ordering::SeqCst,
            );
            TestResponse::json(
                200,
                &json!({
                    "access_token": "initial-access",
                    "refresh_token": "initial-refresh",
                    "token_type": "Bearer",
                    "expires_in": 900,
                    "refresh_expires_at": Utc::now().timestamp() + 7200,
                    "token_endpoint": format!("{}/oauth/token", request.origin),
                    "temporal": {"address":"127.0.0.1:7233","tls":false}
                }),
            )
        });
        let store = Arc::new(MemoryCredentialStore::default());
        let result = login_with_test_store(
            "local",
            &server.url,
            "admin",
            "correct horse battery staple",
            None,
            true,
            store.clone(),
        )
        .await
        .unwrap();
        assert!(password_seen.load(Ordering::SeqCst));
        assert_eq!(result.temporal_address, "127.0.0.1:7233");
        assert!(!result.temporal_tls);
        assert_eq!(
            result.session.access_token().await.unwrap(),
            "initial-access"
        );

        let raw = raw_credential(&store, "local", &result.profile);
        assert!(raw.contains("initial-refresh"));
        assert!(!raw.contains("initial-access"));
        assert!(!raw.contains("correct horse battery staple"));
        assert_eq!(
            serde_json::from_str::<Value>(&raw)
                .unwrap()
                .as_object()
                .unwrap()
                .len(),
            3
        );
    }

    #[tokio::test]
    async fn login_rejects_cross_origin_token_endpoint_without_persisting() {
        let server = TestServer::new(|_| {
            TestResponse::json(
                200,
                &json!({
                    "access_token":"access",
                    "refresh_token":"refresh",
                    "token_type":"Bearer",
                    "expires_in":900,
                    "refresh_expires_at":Utc::now().timestamp()+7200,
                    "token_endpoint":"http://127.0.0.2:12345/oauth/token",
                    "temporal":{"address":"127.0.0.1:7233","tls":false}
                }),
            )
        });
        let store = Arc::new(MemoryCredentialStore::default());
        let error = login_with_test_store(
            "local",
            &server.url,
            "admin",
            "password",
            None,
            true,
            store.clone(),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("same origin"));
        assert!(store.values.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn refresh_rotates_and_persists_before_returning_access() {
        let refresh_expiry = Utc::now().timestamp() + 7200;
        let server = TestServer::new(move |request| {
            assert_eq!(request.path, "/oauth/token");
            let form = parse_form(&request.body);
            assert_eq!(form["grant_type"], "refresh_token");
            assert_eq!(form["client_id"], OAUTH_CLIENT_ID);
            assert_eq!(form["refresh_token"], "old-refresh");
            TestResponse::json(
                200,
                &json!({
                    "access_token":"new-access",
                    "refresh_token":"new-refresh",
                    "token_type":"Bearer",
                    "expires_in":900,
                    "refresh_expires_at":refresh_expiry
                }),
            )
        });
        let store = Arc::new(MemoryCredentialStore::default());
        set_raw_credential(
            &store,
            "local",
            &local_profile(&server.url),
            "old-refresh",
            refresh_expiry,
        );
        let session =
            load_with_test_store("local", local_profile(&server.url), store.clone()).unwrap();
        assert_eq!(session.access_token().await.unwrap(), "new-access");
        let raw = raw_credential(&store, "local", &local_profile(&server.url));
        assert!(raw.contains("new-refresh"));
        assert!(!raw.contains("new-access"));
        assert!(!raw.contains("old-refresh"));
        assert_eq!(
            serde_json::from_str::<Value>(&raw).unwrap()["refresh_expires_at"],
            refresh_expiry
        );
    }

    #[tokio::test]
    async fn failed_rotation_persistence_never_exposes_access_and_can_retry_commit() {
        let refresh_expiry = Utc::now().timestamp() + 7200;
        let server = TestServer::new(move |_| {
            TestResponse::json(
                200,
                &json!({
                    "access_token":"withheld-access",
                    "refresh_token":"rotated-refresh",
                    "token_type":"Bearer",
                    "expires_in":900,
                    "refresh_expires_at":refresh_expiry
                }),
            )
        });
        let store = Arc::new(MemoryCredentialStore::default());
        set_raw_credential(
            &store,
            "local",
            &local_profile(&server.url),
            "old-refresh",
            refresh_expiry,
        );
        let session =
            load_with_test_store("local", local_profile(&server.url), store.clone()).unwrap();
        store.set_failures(false, true, false);
        let error = session.access_token().await.unwrap_err();
        assert!(matches!(error, AuthError::CredentialWrite(_)));
        assert!(
            raw_credential(&store, "local", &local_profile(&server.url)).contains("old-refresh")
        );

        store.set_failures(false, false, false);
        assert_eq!(session.access_token().await.unwrap(), "withheld-access");
        assert!(
            raw_credential(&store, "local", &local_profile(&server.url))
                .contains("rotated-refresh")
        );
        assert_eq!(server.request_count(), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_access_calls_share_one_refresh() {
        let refresh_expiry = Utc::now().timestamp() + 7200;
        let server = TestServer::new(move |_| {
            thread::sleep(Duration::from_millis(50));
            TestResponse::json(
                200,
                &json!({
                    "access_token":"shared-access",
                    "refresh_token":"rotated-refresh",
                    "token_type":"Bearer",
                    "expires_in":900,
                    "refresh_expires_at":refresh_expiry
                }),
            )
        });
        let store = Arc::new(MemoryCredentialStore::default());
        set_raw_credential(
            &store,
            "local",
            &local_profile(&server.url),
            "old-refresh",
            refresh_expiry,
        );
        let session = load_with_test_store("local", local_profile(&server.url), store).unwrap();
        let mut tasks = Vec::new();
        for _ in 0..12 {
            let clone = session.clone();
            tasks.push(tokio::spawn(async move { clone.access_token().await }));
        }
        for task in tasks {
            assert_eq!(task.await.unwrap().unwrap(), "shared-access");
        }
        assert_eq!(server.request_count(), 1);
    }

    #[tokio::test]
    async fn userinfo_uses_access_token_and_logout_revokes_then_deletes() {
        let refresh_expiry = Utc::now().timestamp() + 7200;
        let requests = Arc::new(StdMutex::new(Vec::new()));
        let requests_in_handler = Arc::clone(&requests);
        let server = TestServer::new(move |request| {
            requests_in_handler.lock().unwrap().push((
                request.method.clone(),
                request.path.clone(),
                request.body.clone(),
            ));
            match request.path.as_str() {
                "/oauth/token" => TestResponse::json(
                    200,
                    &json!({
                        "access_token":"userinfo-access",
                        "refresh_token":"rotated-refresh",
                        "token_type":"Bearer",
                        "expires_in":900,
                        "refresh_expires_at":refresh_expiry
                    }),
                ),
                "/oauth/userinfo" => {
                    assert_eq!(
                        request.headers.get("authorization").map(String::as_str),
                        Some("Bearer userinfo-access")
                    );
                    TestResponse::json(
                        200,
                        &json!({
                            "sub":"user-1",
                            "name":"admin",
                            "preferred_username":"admin",
                            "permissions":["read","write"]
                        }),
                    )
                }
                "/oauth/revoke" => {
                    let form = parse_form(&request.body);
                    assert_eq!(form["token"], "rotated-refresh");
                    assert_eq!(form["token_type_hint"], "refresh_token");
                    TestResponse::empty(200)
                }
                _ => TestResponse::empty(404),
            }
        });
        let store = Arc::new(MemoryCredentialStore::default());
        set_raw_credential(
            &store,
            "local",
            &local_profile(&server.url),
            "old-refresh",
            refresh_expiry,
        );
        let session =
            load_with_test_store("local", local_profile(&server.url), store.clone()).unwrap();
        let identity = session.userinfo().await.unwrap();
        assert_eq!(identity.preferred_username, "admin");
        assert_eq!(identity.permissions, ["read", "write"]);
        session.logout().await.unwrap();
        assert!(!has_credential(
            &store,
            "local",
            &local_profile(&server.url)
        ));
        assert!(matches!(
            session.access_token().await.unwrap_err(),
            AuthError::NotLoggedIn(_)
        ));
    }

    #[tokio::test]
    async fn failed_revocation_keeps_local_credential() {
        let refresh_expiry = Utc::now().timestamp() + 7200;
        let server = TestServer::new(|request| {
            assert_eq!(request.path, "/oauth/revoke");
            TestResponse::empty(503)
        });
        let store = Arc::new(MemoryCredentialStore::default());
        set_raw_credential(
            &store,
            "local",
            &local_profile(&server.url),
            "refresh",
            refresh_expiry,
        );
        let session =
            load_with_test_store("local", local_profile(&server.url), store.clone()).unwrap();
        let error = session.logout().await.unwrap_err();
        assert!(matches!(error, AuthError::Status { .. }));
        assert!(has_credential(&store, "local", &local_profile(&server.url)));
    }

    #[tokio::test]
    async fn expired_stored_session_can_still_be_revoked_and_removed() {
        let server = TestServer::new(|request| {
            assert_eq!(request.path, "/oauth/revoke");
            let form = parse_form(&request.body);
            assert_eq!(form["token"], "expired-refresh");
            TestResponse::empty(200)
        });
        let store = Arc::new(MemoryCredentialStore::default());
        set_raw_credential(
            &store,
            "local",
            &local_profile(&server.url),
            "expired-refresh",
            Utc::now().timestamp() - 60,
        );
        let session =
            load_with_test_store("local", local_profile(&server.url), store.clone()).unwrap();
        assert!(
            session
                .access_token()
                .await
                .unwrap_err()
                .to_string()
                .contains("expired")
        );
        session.logout().await.unwrap();
        assert!(!has_credential(
            &store,
            "local",
            &local_profile(&server.url)
        ));
    }

    #[tokio::test]
    async fn login_rejects_insecure_remote_temporal_endpoint() {
        let server = TestServer::new(|request| {
            TestResponse::json(
                200,
                &json!({
                    "access_token":"access",
                    "refresh_token":"refresh",
                    "token_type":"Bearer",
                    "expires_in":900,
                    "refresh_expires_at":Utc::now().timestamp()+7200,
                    "token_endpoint":format!("{}/oauth/token",request.origin),
                    "temporal":{"address":"temporal.example.test:7233","tls":false}
                }),
            )
        });
        let store = Arc::new(MemoryCredentialStore::default());
        let error =
            login_with_test_store("local", &server.url, "admin", "password", None, true, store)
                .await
                .unwrap_err();
        assert!(error.to_string().contains("insecure Temporal endpoint"));
    }

    #[tokio::test]
    async fn address_override_replaces_missing_advertised_endpoint_and_forces_tls() {
        let server = TestServer::new(|request| {
            TestResponse::json(
                200,
                &json!({
                    "access_token":"access",
                    "refresh_token":"refresh",
                    "token_type":"Bearer",
                    "expires_in":900,
                    "refresh_expires_at":Utc::now().timestamp()+7200,
                    "token_endpoint":format!("{}/oauth/token",request.origin)
                }),
            )
        });
        let store = Arc::new(MemoryCredentialStore::default());
        let result = login_with_test_store(
            "local",
            &server.url,
            "admin",
            "password",
            Some("temporal.example.test:443"),
            true,
            store,
        )
        .await
        .unwrap();
        assert_eq!(result.temporal_address, "temporal.example.test:443");
        assert!(result.temporal_tls);
    }

    #[tokio::test]
    async fn oversized_response_is_rejected_without_echoing_body() {
        let secret = "do-not-echo-this-secret";
        let body = format!("{secret}{}", "x".repeat(MAX_RESPONSE_BYTES));
        let server = TestServer::new(move |_| TestResponse::text(200, body.clone()));
        let store = Arc::new(MemoryCredentialStore::default());
        let error =
            login_with_test_store("local", &server.url, "admin", "password", None, true, store)
                .await
                .unwrap_err();
        assert!(matches!(error, AuthError::ResponseTooLarge));
        assert!(!error.to_string().contains(secret));
    }

    #[tokio::test]
    async fn oversized_password_is_rejected_before_network_or_storage() {
        let store = Arc::new(MemoryCredentialStore::default());
        let password = "x".repeat(MAX_RESPONSE_BYTES + 1);
        let error = login_with_test_store(
            "local",
            "http://127.0.0.1:1",
            "admin",
            &password,
            Some("127.0.0.1:7233"),
            true,
            store.clone(),
        )
        .await
        .unwrap_err();
        assert!(matches!(error, AuthError::InvalidProfile(_)));
        assert!(!error.to_string().contains(&password));
        assert!(store.values.lock().unwrap().is_empty());
    }

    #[test]
    fn credential_binding_is_canonical_and_separates_security_contexts() {
        let profile = TemporalAuthProfile {
            url: "https://AUTH.example.test:443/base/".to_owned(),
            username: "admin".to_owned(),
            token_endpoint: "https://auth.example.test/base/oauth/token".to_owned(),
            allow_insecure: false,
        };
        let canonical = TemporalAuthProfile {
            url: "https://auth.example.test/base".to_owned(),
            ..profile.clone()
        };
        let item = credential_item_for_profile("prod", &profile).unwrap();
        assert_eq!(
            item,
            credential_item_for_profile("prod", &canonical).unwrap()
        );
        assert_ne!(
            item,
            credential_item_for_profile(
                "prod",
                &TemporalAuthProfile {
                    username: "other-admin".to_owned(),
                    ..canonical.clone()
                }
            )
            .unwrap()
        );
        assert_ne!(
            item,
            credential_item_for_profile(
                "prod",
                &TemporalAuthProfile {
                    url: "https://other.example.test/base".to_owned(),
                    token_endpoint: "https://other.example.test/base/oauth/token".to_owned(),
                    ..canonical
                }
            )
            .unwrap()
        );
        assert!(item.starts_with("temporal-auth/v1/"));
        assert_eq!(item.rsplit('/').next().unwrap().len(), 64);
    }

    #[test]
    fn persisted_binding_mismatch_fails_closed() {
        let profile = local_profile("http://127.0.0.1:12345");
        let item = credential_item_for_profile("local", &profile).unwrap();
        let store = Arc::new(MemoryCredentialStore::default());
        store
            .set(
                KEYRING_SERVICE,
                &item,
                &json!({
                    "binding_sha256":"00".repeat(32),
                    "refresh_token":"must-not-be-used",
                    "refresh_expires_at":Utc::now().timestamp()+7200
                })
                .to_string(),
            )
            .unwrap();
        let error = load_with_test_store("local", profile, store).unwrap_err();
        assert!(matches!(error, AuthError::InvalidResponse(_)));
        assert!(!error.to_string().contains("must-not-be-used"));
    }

    #[tokio::test]
    async fn independent_sessions_reload_the_rotated_refresh_credential() {
        let refresh_expiry = Utc::now().timestamp() + 7200;
        let step = Arc::new(AtomicUsize::new(0));
        let step_in_handler = Arc::clone(&step);
        let server = TestServer::new(move |request| {
            let current = step_in_handler.fetch_add(1, Ordering::SeqCst);
            let form = parse_form(&request.body);
            let (expected, access, next) = match current {
                0 => ("refresh-0", "access-1", "refresh-1"),
                1 => ("refresh-1", "access-2", "refresh-2"),
                _ => panic!("unexpected refresh request"),
            };
            assert_eq!(form["refresh_token"], expected);
            TestResponse::json(
                200,
                &json!({
                    "access_token":access,
                    "refresh_token":next,
                    "token_type":"Bearer",
                    "expires_in":900,
                    "refresh_expires_at":refresh_expiry
                }),
            )
        });
        let profile = local_profile(&server.url);
        let store = Arc::new(MemoryCredentialStore::default());
        set_raw_credential(&store, "shared", &profile, "refresh-0", refresh_expiry);
        let first = load_with_test_store("shared", profile.clone(), store.clone()).unwrap();
        let second = load_with_test_store("shared", profile.clone(), store.clone()).unwrap();

        assert_eq!(first.access_token().await.unwrap(), "access-1");
        assert_eq!(second.access_token().await.unwrap(), "access-2");
        assert!(raw_credential(&store, "shared", &profile).contains("refresh-2"));
        assert_eq!(server.request_count(), 2);
    }

    #[tokio::test]
    async fn logout_reloads_and_revokes_the_latest_rotated_credential() {
        let refresh_expiry = Utc::now().timestamp() + 7200;
        let server = TestServer::new(move |request| match request.path.as_str() {
            "/oauth/token" => {
                let form = parse_form(&request.body);
                assert_eq!(form["refresh_token"], "refresh-0");
                TestResponse::json(
                    200,
                    &json!({
                        "access_token":"access-1",
                        "refresh_token":"refresh-1",
                        "token_type":"Bearer",
                        "expires_in":900,
                        "refresh_expires_at":refresh_expiry
                    }),
                )
            }
            "/oauth/revoke" => {
                let form = parse_form(&request.body);
                assert_eq!(form["token"], "refresh-1");
                TestResponse::empty(200)
            }
            _ => TestResponse::empty(404),
        });
        let profile = local_profile(&server.url);
        let store = Arc::new(MemoryCredentialStore::default());
        set_raw_credential(&store, "shared", &profile, "refresh-0", refresh_expiry);
        let stale = load_with_test_store("shared", profile.clone(), store.clone()).unwrap();
        let rotating = load_with_test_store("shared", profile.clone(), store.clone()).unwrap();
        assert_eq!(rotating.access_token().await.unwrap(), "access-1");
        stale.logout().await.unwrap();
        assert!(!has_credential(&store, "shared", &profile));
    }

    #[tokio::test]
    async fn userinfo_rejects_terminal_direction_controls() {
        let refresh_expiry = Utc::now().timestamp() + 7200;
        let server = TestServer::new(move |request| match request.path.as_str() {
            "/oauth/token" => TestResponse::json(
                200,
                &json!({
                    "access_token":"access",
                    "refresh_token":"refresh-1",
                    "token_type":"Bearer",
                    "expires_in":900,
                    "refresh_expires_at":refresh_expiry
                }),
            ),
            "/oauth/userinfo" => TestResponse::json(
                200,
                &json!({
                    "sub":"user-1",
                    "name":"safe\u{202e}txt",
                    "preferred_username":"admin",
                    "permissions":["read"]
                }),
            ),
            _ => TestResponse::empty(404),
        });
        let profile = local_profile(&server.url);
        let store = Arc::new(MemoryCredentialStore::default());
        set_raw_credential(&store, "local", &profile, "refresh-0", refresh_expiry);
        let session = load_with_test_store("local", profile, store).unwrap();
        let error = session.userinfo().await.unwrap_err();
        assert!(matches!(error, AuthError::InvalidResponse(_)));
        assert!(!error.to_string().contains('\u{202e}'));
        assert!(validate_username("safe\u{202e}txt").is_err());
    }

    #[test]
    fn retryability_distinguishes_transient_and_permanent_failures() {
        assert!(
            AuthError::Status {
                operation: "test",
                status: StatusCode::TOO_MANY_REQUESTS
            }
            .is_retryable()
        );
        assert!(
            AuthError::Status {
                operation: "test",
                status: StatusCode::SERVICE_UNAVAILABLE
            }
            .is_retryable()
        );
        assert!(
            !AuthError::Status {
                operation: "test",
                status: StatusCode::UNAUTHORIZED
            }
            .is_retryable()
        );
        assert!(!AuthError::InvalidResponse("invalid grant").is_retryable());
        assert!(AuthError::CredentialWrite(CredentialStoreError).is_retryable());
    }

    #[test]
    fn sixty_second_access_token_has_a_nonzero_refresh_delay() {
        let now = Utc::now();
        let access = access_credential("access".to_owned(), "Bearer", 60, now).unwrap();
        assert_eq!(access.refresh_early, TimeDelta::seconds(12));
        assert!(access.expires_at - access.refresh_early > now);
    }

    #[tokio::test]
    async fn file_lock_is_private_persistent_and_released_on_drop() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("locks");
        let coordinator = FileCredentialCoordinator::new(root.clone());
        let binding = "00".repeat(32);
        let first = coordinator.acquire(&binding).await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(100), coordinator.acquire(&binding))
                .await
                .is_err()
        );
        drop(first);
        let second = tokio::time::timeout(Duration::from_secs(1), coordinator.acquire(&binding))
            .await
            .unwrap()
            .unwrap();
        drop(second);
        let lock_path = root.join(format!("{binding}.lock"));
        assert!(lock_path.is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            assert_eq!(
                fs::metadata(&root).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(lock_path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn file_lock_rejects_a_symlink_directory() {
        let temporary = tempfile::tempdir().unwrap();
        let target = temporary.path().join("target");
        fs::create_dir(&target).unwrap();
        let root = temporary.path().join("locks");
        std::os::unix::fs::symlink(target, &root).unwrap();
        let coordinator = FileCredentialCoordinator::new(root);
        let error = coordinator.acquire(&"00".repeat(32)).await.err().unwrap();
        assert!(matches!(error, AuthError::Coordination(_)));
    }

    fn local_profile(server_url: &str) -> TemporalAuthProfile {
        TemporalAuthProfile {
            url: server_url.to_owned(),
            username: "admin".to_owned(),
            token_endpoint: format!("{server_url}/oauth/token"),
            allow_insecure: true,
        }
    }

    fn set_raw_credential(
        store: &MemoryCredentialStore,
        profile_name: &str,
        profile: &TemporalAuthProfile,
        token: &str,
        expires_at: i64,
    ) {
        let binding = credential_binding(profile_name, profile).unwrap();
        let value = json!({
            "binding_sha256":binding,
            "refresh_token":token,
            "refresh_expires_at":expires_at
        })
        .to_string();
        store
            .set(
                KEYRING_SERVICE,
                &credential_item_for_profile(profile_name, profile).unwrap(),
                &value,
            )
            .unwrap();
    }

    fn raw_credential(
        store: &MemoryCredentialStore,
        profile_name: &str,
        profile: &TemporalAuthProfile,
    ) -> String {
        store
            .get(
                KEYRING_SERVICE,
                &credential_item_for_profile(profile_name, profile).unwrap(),
            )
            .unwrap()
            .unwrap()
    }

    fn has_credential(
        store: &MemoryCredentialStore,
        profile_name: &str,
        profile: &TemporalAuthProfile,
    ) -> bool {
        store
            .get(
                KEYRING_SERVICE,
                &credential_item_for_profile(profile_name, profile).unwrap(),
            )
            .unwrap()
            .is_some()
    }

    fn parse_form(body: &[u8]) -> HashMap<String, String> {
        url::form_urlencoded::parse(body).into_owned().collect()
    }

    #[derive(Clone)]
    struct TestRequest {
        method: String,
        path: String,
        origin: String,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    }

    struct TestResponse {
        status: u16,
        content_type: &'static str,
        body: Vec<u8>,
    }

    impl TestResponse {
        fn json(status: u16, body: &Value) -> Self {
            Self {
                status,
                content_type: "application/json",
                body: serde_json::to_vec(&body).unwrap(),
            }
        }

        fn text(status: u16, body: String) -> Self {
            Self {
                status,
                content_type: "text/plain",
                body: body.into_bytes(),
            }
        }

        fn empty(status: u16) -> Self {
            Self {
                status,
                content_type: "application/json",
                body: Vec::new(),
            }
        }
    }

    struct TestServer {
        url: String,
        requests: Arc<AtomicUsize>,
        stop: Arc<AtomicBool>,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl TestServer {
        fn new(handler: impl Fn(TestRequest) -> TestResponse + Send + Sync + 'static) -> Self {
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
            listener.set_nonblocking(true).unwrap();
            let address = listener.local_addr().unwrap();
            let url = format!("http://{address}");
            let origin = url.clone();
            let requests = Arc::new(AtomicUsize::new(0));
            let requests_in_thread = Arc::clone(&requests);
            let stop = Arc::new(AtomicBool::new(false));
            let stop_in_thread = Arc::clone(&stop);
            let thread = thread::spawn(move || {
                while !stop_in_thread.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            stream.set_nonblocking(false).unwrap();
                            stream
                                .set_write_timeout(Some(Duration::from_secs(1)))
                                .unwrap();
                            let request = read_request(&mut stream, &origin);
                            requests_in_thread.fetch_add(1, Ordering::SeqCst);
                            let response = handler(request);
                            let _ = write_response(&mut stream, &response);
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(2));
                        }
                        Err(_) => break,
                    }
                }
            });
            Self {
                url,
                requests,
                stop,
                thread: Some(thread),
            }
        }

        fn request_count(&self) -> usize {
            self.requests.load(Ordering::SeqCst)
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    fn read_request(stream: &mut std::net::TcpStream, origin: &str) -> TestRequest {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut raw = Vec::new();
        let mut buffer = [0_u8; 4096];
        let header_end = loop {
            let count = stream.read(&mut buffer).unwrap();
            assert!(count > 0);
            raw.extend_from_slice(&buffer[..count]);
            if let Some(index) = raw.windows(4).position(|part| part == b"\r\n\r\n") {
                break index + 4;
            }
            assert!(raw.len() <= MAX_RESPONSE_BYTES);
        };
        let headers_raw = String::from_utf8(raw[..header_end].to_vec()).unwrap();
        let mut lines = headers_raw.split("\r\n");
        let mut request_line = lines.next().unwrap().split_whitespace();
        let method = request_line.next().unwrap().to_owned();
        let path = request_line.next().unwrap().to_owned();
        let mut headers = HashMap::new();
        for line in lines.filter(|line| !line.is_empty()) {
            let (name, value) = line.split_once(':').unwrap();
            headers.insert(name.to_ascii_lowercase(), value.trim().to_owned());
        }
        let content_length = headers
            .get("content-length")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        while raw.len() - header_end < content_length {
            let count = stream.read(&mut buffer).unwrap();
            assert!(count > 0);
            raw.extend_from_slice(&buffer[..count]);
        }
        TestRequest {
            method,
            path,
            origin: origin.to_owned(),
            headers,
            body: raw[header_end..header_end + content_length].to_vec(),
        }
    }

    fn write_response(
        stream: &mut std::net::TcpStream,
        response: &TestResponse,
    ) -> std::io::Result<()> {
        let reason = match response.status {
            200 => "OK",
            401 => "Unauthorized",
            404 => "Not Found",
            503 => "Service Unavailable",
            _ => "Test",
        };
        let headers = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            response.status,
            reason,
            response.content_type,
            response.body.len()
        );
        stream.write_all(headers.as_bytes())?;
        stream.write_all(&response.body)?;
        stream.flush()
    }

    #[test]
    fn imported_network_types_are_loopback() {
        assert!(IpAddr::V4(Ipv4Addr::LOCALHOST).is_loopback());
        assert!(IpAddr::V6(Ipv6Addr::LOCALHOST).is_loopback());
        assert!(validate_temporal_address("127.0.0.1:7233").unwrap());
        assert!(validate_temporal_address("[::1]:7233").unwrap());
    }
}
