//! # Configuration
//!
//! [`CortexConfig`] holds everything needed to connect to the Cortex API.
//!
//! ## Loading Priority
//!
//! Configuration is loaded from the first source that provides a value:
//!
//! 1. Explicit struct fields (programmatic construction)
//! 2. Environment variables (`EMOTIV_CLIENT_ID`, `EMOTIV_CLIENT_SECRET`, etc.)
//! 3. TOML config file at an explicit path
//! 4. `./cortex.toml` in the current directory
//! 5. `~/.config/emotiv-cortex/cortex.toml`
//!
//! Individual fields can always be overridden by environment variables,
//! even when loading from a file.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::{CortexError, CortexResult};

/// Default Cortex WebSocket URL (localhost, self-signed TLS).
pub const DEFAULT_CORTEX_URL: &str = "wss://localhost:6868";

/// Default RPC call timeout in seconds.
const DEFAULT_RPC_TIMEOUT_SECS: u64 = 10;

/// Default stream subscribe timeout in seconds.
const DEFAULT_SUBSCRIBE_TIMEOUT_SECS: u64 = 15;

/// Default headset connection timeout in seconds.
const DEFAULT_HEADSET_CONNECT_TIMEOUT_SECS: u64 = 30;

/// Default reconnect base delay in seconds.
const DEFAULT_RECONNECT_BASE_DELAY_SECS: u64 = 1;

/// Default reconnect max delay in seconds.
const DEFAULT_RECONNECT_MAX_DELAY_SECS: u64 = 60;

/// Default max reconnect attempts (0 = unlimited).
const DEFAULT_RECONNECT_MAX_ATTEMPTS: u32 = 0;

/// Default health check interval in seconds.
const DEFAULT_HEALTH_INTERVAL_SECS: u64 = 30;

/// Default max consecutive health check failures before reconnect.
const DEFAULT_HEALTH_MAX_FAILURES: u32 = 3;

/// Configuration for connecting to the Emotiv Cortex API.
///
/// # Examples
///
/// ## From environment variables
///
/// ```no_run
/// use emotiv_cortex_v2::config::CortexConfig;
///
/// // Set EMOTIV_CLIENT_ID and EMOTIV_CLIENT_SECRET env vars, then:
/// let config = CortexConfig::from_env().expect("Missing env vars");
/// ```
///
/// ## From a TOML file
///
/// ```no_run
/// use emotiv_cortex_v2::config::CortexConfig;
///
/// let config = CortexConfig::from_file("cortex.toml").expect("Bad config");
/// ```
///
/// ## Programmatic
///
/// ```
/// use emotiv_cortex_v2::config::CortexConfig;
///
/// let config = CortexConfig::new("my-client-id", "my-client-secret");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CortexConfig {
    /// Cortex API client ID from the [Emotiv Developer Portal](https://www.emotiv.com/developer/).
    pub client_id: String,

    /// Cortex API client secret.
    pub client_secret: String,

    /// WebSocket URL for the Cortex service.
    #[serde(default = "default_cortex_url")]
    pub cortex_url: String,

    /// Emotiv license key for commercial/premium features.
    #[serde(default)]
    pub license: Option<String>,

    /// Request decontaminated EEG data (motion artifact removal).
    #[serde(default = "default_true")]
    pub decontaminated: bool,

    /// Allow insecure TLS connections to non-localhost hosts.
    /// Only enable this for development/testing.
    #[serde(default)]
    pub allow_insecure_tls: bool,

    /// Timeout configuration.
    #[serde(default)]
    pub timeouts: TimeoutConfig,

    /// Auto-reconnect configuration.
    #[serde(default)]
    pub reconnect: ReconnectConfig,

    /// Health monitoring configuration.
    #[serde(default)]
    pub health: HealthConfig,
}

/// Timeout settings for various Cortex operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutConfig {
    /// Timeout for individual JSON-RPC calls, in seconds.
    #[serde(default = "default_rpc_timeout")]
    pub rpc_timeout_secs: u64,

    /// Timeout for stream subscribe operations, in seconds.
    #[serde(default = "default_subscribe_timeout")]
    pub subscribe_timeout_secs: u64,

    /// Timeout for headset connection, in seconds.
    #[serde(default = "default_headset_connect_timeout")]
    pub headset_connect_timeout_secs: u64,
}

/// Auto-reconnect behavior when the WebSocket connection drops.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconnectConfig {
    /// Enable auto-reconnect on connection loss.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Initial delay before the first reconnect attempt, in seconds.
    #[serde(default = "default_reconnect_base_delay")]
    pub base_delay_secs: u64,

    /// Maximum delay between reconnect attempts (exponential backoff cap), in seconds.
    #[serde(default = "default_reconnect_max_delay")]
    pub max_delay_secs: u64,

    /// Maximum number of reconnect attempts. 0 means unlimited.
    #[serde(default = "default_reconnect_max_attempts")]
    pub max_attempts: u32,
}

/// Health monitoring configuration (periodic heartbeat).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthConfig {
    /// Enable periodic health checks.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Interval between health check calls, in seconds.
    #[serde(default = "default_health_interval")]
    pub interval_secs: u64,

    /// Number of consecutive health check failures before triggering reconnect.
    #[serde(default = "default_health_max_failures")]
    pub max_consecutive_failures: u32,
}

// ─── Defaults ───────────────────────────────────────────────────────────

fn default_cortex_url() -> String {
    DEFAULT_CORTEX_URL.to_string()
}

fn default_true() -> bool {
    true
}

fn default_rpc_timeout() -> u64 {
    DEFAULT_RPC_TIMEOUT_SECS
}

fn default_subscribe_timeout() -> u64 {
    DEFAULT_SUBSCRIBE_TIMEOUT_SECS
}

fn default_headset_connect_timeout() -> u64 {
    DEFAULT_HEADSET_CONNECT_TIMEOUT_SECS
}

fn default_reconnect_base_delay() -> u64 {
    DEFAULT_RECONNECT_BASE_DELAY_SECS
}

fn default_reconnect_max_delay() -> u64 {
    DEFAULT_RECONNECT_MAX_DELAY_SECS
}

fn default_reconnect_max_attempts() -> u32 {
    DEFAULT_RECONNECT_MAX_ATTEMPTS
}

fn default_health_interval() -> u64 {
    DEFAULT_HEALTH_INTERVAL_SECS
}

fn default_health_max_failures() -> u32 {
    DEFAULT_HEALTH_MAX_FAILURES
}

// ─── Default impls ──────────────────────────────────────────────────────

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            rpc_timeout_secs: DEFAULT_RPC_TIMEOUT_SECS,
            subscribe_timeout_secs: DEFAULT_SUBSCRIBE_TIMEOUT_SECS,
            headset_connect_timeout_secs: DEFAULT_HEADSET_CONNECT_TIMEOUT_SECS,
        }
    }
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            base_delay_secs: DEFAULT_RECONNECT_BASE_DELAY_SECS,
            max_delay_secs: DEFAULT_RECONNECT_MAX_DELAY_SECS,
            max_attempts: DEFAULT_RECONNECT_MAX_ATTEMPTS,
        }
    }
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_secs: DEFAULT_HEALTH_INTERVAL_SECS,
            max_consecutive_failures: DEFAULT_HEALTH_MAX_FAILURES,
        }
    }
}

// ─── CortexConfig impl ─────────────────────────────────────────────────

impl CortexConfig {
    /// Create a config with just client credentials (all other fields use defaults).
    pub fn new(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            cortex_url: default_cortex_url(),
            license: None,
            decontaminated: true,
            allow_insecure_tls: false,
            timeouts: TimeoutConfig::default(),
            reconnect: ReconnectConfig::default(),
            health: HealthConfig::default(),
        }
    }

    /// Load config from environment variables.
    ///
    /// Required: `EMOTIV_CLIENT_ID`, `EMOTIV_CLIENT_SECRET`
    ///
    /// Optional: `EMOTIV_CORTEX_URL`, `EMOTIV_LICENSE`
    pub fn from_env() -> CortexResult<Self> {
        let client_id =
            std::env::var("EMOTIV_CLIENT_ID").map_err(|_| CortexError::ConfigError {
                reason: "EMOTIV_CLIENT_ID environment variable not set".into(),
            })?;
        let client_secret =
            std::env::var("EMOTIV_CLIENT_SECRET").map_err(|_| CortexError::ConfigError {
                reason: "EMOTIV_CLIENT_SECRET environment variable not set".into(),
            })?;

        let mut config = Self::new(client_id, client_secret);

        if let Ok(url) = std::env::var("EMOTIV_CORTEX_URL") {
            config.cortex_url = url;
        }
        if let Ok(license) = std::env::var("EMOTIV_LICENSE") {
            config.license = Some(license);
        }

        Ok(config)
    }

    /// Load config from a TOML file, with environment variable overrides.
    ///
    /// Environment variables take precedence over file values for
    /// `client_id`, `client_secret`, `cortex_url`, and `license`.
    pub fn from_file(path: impl AsRef<Path>) -> CortexResult<Self> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path).map_err(|e| CortexError::ConfigError {
            reason: format!("Failed to read config file '{}': {}", path.display(), e),
        })?;
        let mut config: Self = toml::from_str(&contents)?;

        // Environment variable overrides
        if let Ok(id) = std::env::var("EMOTIV_CLIENT_ID") {
            config.client_id = id;
        }
        if let Ok(secret) = std::env::var("EMOTIV_CLIENT_SECRET") {
            config.client_secret = secret;
        }
        if let Ok(url) = std::env::var("EMOTIV_CORTEX_URL") {
            config.cortex_url = url;
        }
        if let Ok(license) = std::env::var("EMOTIV_LICENSE") {
            config.license = Some(license);
        }

        Ok(config)
    }

    /// Discover and load config from the standard search path:
    ///
    /// 1. Explicit path (if `Some`)
    /// 2. `CORTEX_CONFIG` environment variable
    /// 3. `./cortex.toml`
    /// 4. `~/.config/emotiv-cortex/cortex.toml`
    ///
    /// Falls back to environment-variable-only config if no file is found.
    pub fn discover(explicit_path: Option<&Path>) -> CortexResult<Self> {
        // 1. Explicit path
        if let Some(path) = explicit_path {
            return Self::from_file(path);
        }

        // 2. CORTEX_CONFIG env var
        if let Ok(path) = std::env::var("CORTEX_CONFIG") {
            let path = PathBuf::from(path);
            if path.exists() {
                return Self::from_file(&path);
            }
        }

        // 3. ./cortex.toml
        let local_path = PathBuf::from("cortex.toml");
        if local_path.exists() {
            return Self::from_file(&local_path);
        }

        // 4. ~/.config/emotiv-cortex/cortex.toml
        if let Some(config_dir) = dirs_config_path() {
            if config_dir.exists() {
                return Self::from_file(&config_dir);
            }
        }

        // 5. Environment variables only
        Self::from_env()
    }

    /// Returns `true` if insecure TLS should be allowed for the configured URL.
    ///
    /// Insecure TLS is always allowed for `localhost` and `127.0.0.1`
    /// (the Cortex service uses a self-signed cert). For other hosts,
    /// `allow_insecure_tls` must be explicitly set.
    pub fn should_accept_invalid_certs(&self) -> bool {
        if is_localhost(&self.cortex_url) {
            return true;
        }
        self.allow_insecure_tls
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────

/// Check if a WebSocket URL points to localhost.
fn is_localhost(url: &str) -> bool {
    let authority = url
        .strip_prefix("wss://")
        .or_else(|| url.strip_prefix("ws://"))
        .unwrap_or(url);

    // Handle IPv6 bracket notation: [::1]:6868
    if let Some(rest) = authority.strip_prefix('[') {
        let host = rest.split(']').next().unwrap_or("");
        return host == "::1";
    }

    // Regular host:port — split on last colon to separate port
    let host = if let Some(idx) = authority.rfind(':') {
        &authority[..idx]
    } else {
        authority
    };
    matches!(host, "localhost" | "127.0.0.1")
}

/// Platform-appropriate config directory path.
fn dirs_config_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA")
            .ok()
            .map(|dir| PathBuf::from(dir).join("emotiv-cortex").join("cortex.toml"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME").ok().map(|dir| {
            PathBuf::from(dir)
                .join(".config")
                .join("emotiv-cortex")
                .join("cortex.toml")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        saved: Vec<(&'static str, Option<OsString>)>,
    }

    impl EnvGuard {
        fn capture(keys: &[&'static str]) -> Self {
            let saved = keys.iter().map(|k| (*k, std::env::var_os(k))).collect();
            Self { saved }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.saved {
                if let Some(value) = value {
                    std::env::set_var(key, value);
                } else {
                    std::env::remove_var(key);
                }
            }
        }
    }

    struct CurrentDirGuard {
        original: PathBuf,
    }

    impl CurrentDirGuard {
        fn enter(path: &Path) -> Self {
            let original = std::env::current_dir().unwrap();
            std::env::set_current_dir(path).unwrap();
            Self { original }
        }
    }

    impl Drop for CurrentDirGuard {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.original).unwrap();
        }
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "emotiv-cortex-config-tests-{}-{}-{}",
            label,
            std::process::id(),
            now
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write_minimal_config(path: &Path, id: &str, secret: &str, url: &str) {
        fs::write(
            path,
            format!(
                r#"
client_id = "{}"
client_secret = "{}"
cortex_url = "{}"
"#,
                id, secret, url
            ),
        )
        .unwrap();
    }

    #[test]
    fn test_new_defaults() {
        let config = CortexConfig::new("id", "secret");
        assert_eq!(config.client_id, "id");
        assert_eq!(config.client_secret, "secret");
        assert_eq!(config.cortex_url, DEFAULT_CORTEX_URL);
        assert!(config.decontaminated);
        assert!(!config.allow_insecure_tls);
        assert_eq!(config.timeouts.rpc_timeout_secs, DEFAULT_RPC_TIMEOUT_SECS);
        assert!(config.reconnect.enabled);
        assert!(config.health.enabled);
    }

    #[test]
    fn test_is_localhost() {
        assert!(is_localhost("wss://localhost:6868"));
        assert!(is_localhost("wss://127.0.0.1:6868"));
        assert!(is_localhost("ws://localhost:6868"));
        assert!(is_localhost("wss://[::1]:6868"));
        assert!(!is_localhost("wss://example.com:6868"));
        assert!(!is_localhost("wss://192.168.1.100:6868"));
    }

    #[test]
    fn test_should_accept_invalid_certs() {
        let mut config = CortexConfig::new("id", "secret");
        // Localhost always allowed
        assert!(config.should_accept_invalid_certs());

        // Non-localhost denied by default
        config.cortex_url = "wss://remote.example.com:6868".into();
        assert!(!config.should_accept_invalid_certs());

        // Non-localhost allowed with explicit flag
        config.allow_insecure_tls = true;
        assert!(config.should_accept_invalid_certs());
    }

    #[test]
    fn test_deserialize_toml() {
        let toml_str = r#"
            client_id = "test-id"
            client_secret = "test-secret"
            cortex_url = "wss://localhost:9999"
            license = "ABCD-1234"
            decontaminated = false

            [timeouts]
            rpc_timeout_secs = 30

            [reconnect]
            enabled = false
            max_attempts = 5

            [health]
            interval_secs = 60
        "#;

        let config: CortexConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.client_id, "test-id");
        assert_eq!(config.cortex_url, "wss://localhost:9999");
        assert_eq!(config.license, Some("ABCD-1234".into()));
        assert!(!config.decontaminated);
        assert_eq!(config.timeouts.rpc_timeout_secs, 30);
        assert!(!config.reconnect.enabled);
        assert_eq!(config.reconnect.max_attempts, 5);
        assert_eq!(config.health.interval_secs, 60);
    }

    #[test]
    fn test_from_env_requires_credentials_and_applies_overrides() {
        let _lock = env_lock();
        let _env = EnvGuard::capture(&[
            "EMOTIV_CLIENT_ID",
            "EMOTIV_CLIENT_SECRET",
            "EMOTIV_CORTEX_URL",
            "EMOTIV_LICENSE",
        ]);

        std::env::remove_var("EMOTIV_CLIENT_ID");
        std::env::remove_var("EMOTIV_CLIENT_SECRET");
        std::env::remove_var("EMOTIV_CORTEX_URL");
        std::env::remove_var("EMOTIV_LICENSE");

        let missing_id = CortexConfig::from_env().unwrap_err();
        assert!(matches!(missing_id, CortexError::ConfigError { .. }));
        assert!(
            missing_id.to_string().contains("EMOTIV_CLIENT_ID"),
            "unexpected error: {missing_id}"
        );

        std::env::set_var("EMOTIV_CLIENT_ID", "env-id");
        let missing_secret = CortexConfig::from_env().unwrap_err();
        assert!(matches!(missing_secret, CortexError::ConfigError { .. }));
        assert!(
            missing_secret.to_string().contains("EMOTIV_CLIENT_SECRET"),
            "unexpected error: {missing_secret}"
        );

        std::env::set_var("EMOTIV_CLIENT_SECRET", "env-secret");
        std::env::set_var("EMOTIV_CORTEX_URL", "wss://env.example:6868");
        std::env::set_var("EMOTIV_LICENSE", "LICENSE-FROM-ENV");

        let config = CortexConfig::from_env().unwrap();
        assert_eq!(config.client_id, "env-id");
        assert_eq!(config.client_secret, "env-secret");
        assert_eq!(config.cortex_url, "wss://env.example:6868");
        assert_eq!(config.license.as_deref(), Some("LICENSE-FROM-ENV"));
    }

    #[test]
    fn test_from_file_env_overrides_precedence() {
        let _lock = env_lock();
        let _env = EnvGuard::capture(&[
            "EMOTIV_CLIENT_ID",
            "EMOTIV_CLIENT_SECRET",
            "EMOTIV_CORTEX_URL",
            "EMOTIV_LICENSE",
        ]);

        let dir = unique_temp_dir("from-file-overrides");
        let config_path = dir.join("cortex.toml");
        fs::write(
            &config_path,
            r#"
client_id = "file-id"
client_secret = "file-secret"
cortex_url = "wss://file.example:6868"
license = "FILE-LICENSE"
"#,
        )
        .unwrap();

        std::env::set_var("EMOTIV_CLIENT_ID", "env-id");
        std::env::set_var("EMOTIV_CLIENT_SECRET", "env-secret");
        std::env::set_var("EMOTIV_CORTEX_URL", "wss://env.example:6868");
        std::env::set_var("EMOTIV_LICENSE", "ENV-LICENSE");

        let config = CortexConfig::from_file(&config_path).unwrap();
        assert_eq!(config.client_id, "env-id");
        assert_eq!(config.client_secret, "env-secret");
        assert_eq!(config.cortex_url, "wss://env.example:6868");
        assert_eq!(config.license.as_deref(), Some("ENV-LICENSE"));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn test_discover_search_priority() {
        let _lock = env_lock();
        let mut env_keys = vec![
            "EMOTIV_CLIENT_ID",
            "EMOTIV_CLIENT_SECRET",
            "EMOTIV_CORTEX_URL",
            "EMOTIV_LICENSE",
            "CORTEX_CONFIG",
        ];
        #[cfg(target_os = "windows")]
        env_keys.push("APPDATA");
        #[cfg(not(target_os = "windows"))]
        env_keys.push("HOME");
        let _env = EnvGuard::capture(&env_keys);

        let root = unique_temp_dir("discover-priority");
        let cwd = root.join("cwd");
        fs::create_dir_all(&cwd).unwrap();

        let explicit_path = root.join("explicit.toml");
        let env_path = root.join("env.toml");
        write_minimal_config(&explicit_path, "explicit-id", "explicit-secret", "wss://explicit");
        write_minimal_config(&env_path, "env-file-id", "env-file-secret", "wss://env-file");

        let home_root = root.join("home-root");
        let home_config = {
            #[cfg(target_os = "windows")]
            {
                std::env::set_var("APPDATA", &home_root);
                home_root.join("emotiv-cortex").join("cortex.toml")
            }
            #[cfg(not(target_os = "windows"))]
            {
                std::env::set_var("HOME", &home_root);
                home_root
                    .join(".config")
                    .join("emotiv-cortex")
                    .join("cortex.toml")
            }
        };
        fs::create_dir_all(home_config.parent().unwrap()).unwrap();
        write_minimal_config(&home_config, "home-id", "home-secret", "wss://home");
        std::env::remove_var("EMOTIV_CLIENT_ID");
        std::env::remove_var("EMOTIV_CLIENT_SECRET");
        std::env::remove_var("EMOTIV_CORTEX_URL");

        {
            let _cwd = CurrentDirGuard::enter(&cwd);

            std::env::set_var("CORTEX_CONFIG", env_path.to_string_lossy().to_string());
            write_minimal_config(
                &cwd.join("cortex.toml"),
                "local-id",
                "local-secret",
                "wss://local",
            );

            let explicit = CortexConfig::discover(Some(&explicit_path)).unwrap();
            assert_eq!(explicit.client_id, "explicit-id");

            let via_env_pointer = CortexConfig::discover(None).unwrap();
            assert_eq!(via_env_pointer.client_id, "env-file-id");

            std::env::remove_var("CORTEX_CONFIG");
            let via_local = CortexConfig::discover(None).unwrap();
            assert_eq!(via_local.client_id, "local-id");

            fs::remove_file(cwd.join("cortex.toml")).unwrap();
            let via_home = CortexConfig::discover(None).unwrap();
            assert_eq!(via_home.client_id, "home-id");

            fs::remove_file(&home_config).unwrap();
            std::env::set_var("EMOTIV_CLIENT_ID", "fallback-id");
            std::env::set_var("EMOTIV_CLIENT_SECRET", "fallback-secret");
            std::env::set_var("EMOTIV_CORTEX_URL", "wss://fallback");
            let via_env_only = CortexConfig::discover(None).unwrap();
            assert_eq!(via_env_only.client_id, "fallback-id");
            assert_eq!(via_env_only.client_secret, "fallback-secret");
            assert_eq!(via_env_only.cortex_url, "wss://fallback");
        }

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn test_from_file_missing_and_invalid_errors() {
        let _lock = env_lock();
        let dir = unique_temp_dir("from-file-errors");

        let missing = CortexConfig::from_file(dir.join("missing.toml")).unwrap_err();
        assert!(matches!(missing, CortexError::ConfigError { .. }));
        assert!(
            missing.to_string().contains("Failed to read config file"),
            "unexpected error: {missing}"
        );

        let invalid_path = dir.join("invalid.toml");
        fs::write(&invalid_path, "client_id = [").unwrap();
        let invalid = CortexConfig::from_file(&invalid_path).unwrap_err();
        assert!(matches!(invalid, CortexError::ConfigError { .. }));

        fs::remove_dir_all(dir).unwrap();
    }
}
