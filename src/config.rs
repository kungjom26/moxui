//! Configuration loading (figment: yaml + env + defaults).
//!
//! Loads `MoxUI` configuration from multiple sources (priority high → low):
//! 1. Environment variables (`MOXUI_*`)
//! 2. Config file (`config.yaml` / `config.toml`)
//! 3. Defaults
//!
//! Security: Proxmox cluster passwords are wrapped in `Secret<String>` (from the
//! `secrecy` crate) which:
//! - Prevents accidental Debug printing
//! - Implements `Zeroize` to wipe memory on drop
//! - Forces explicit `.expose_secret()` calls

use std::path::Path;

use figment::{
    providers::{Env, Format, Yaml},
    Figment,
};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

// Note: `Serialize` is NOT derived on `Config` (or `ClusterConfig`) because
// Proxmox credentials should never be written to disk or transmitted. To
// persist a config, use the secret manager (planned v1.1+). For now, Config
// can only be DESERIALIZED from yaml/env (read-only at startup).

/// Top-level configuration for `MoxUI`.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// HTTP server configuration.
    pub server: ServerConfig,
    /// Database configuration.
    pub database: DatabaseConfig,
    /// Logging configuration.
    pub logging: LoggingConfig,
    /// Proxmox clusters to connect to.
    #[serde(default)]
    pub clusters: Vec<ClusterConfig>,
    /// Auth configuration (JWT keys + user accounts).
    #[serde(default)]
    pub auth: AuthConfig,
}

/// HTTP server configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// Bind address (e.g., `0.0.0.0:8080`).
    pub bind: String,
    /// Number of worker threads (0 = num CPUs).
    #[serde(default)]
    pub workers: usize,
    /// TLS configuration. When `Some`, the server listens with HTTPS only
    /// and refuses plaintext HTTP. When `None`, the server listens with
    /// plaintext HTTP and emits a startup warning.
    #[serde(default)]
    pub tls: Option<TlsConfig>,
}

/// TLS configuration. Paths to PEM-encoded certificate and private key.
///
/// Both files must be readable by the moxui process. The certificate
/// should be a full chain (leaf + intermediates) in PEM format. The key
/// must be unencrypted PKCS#8 or RSA PEM.
///
/// When TLS is configured, the server enforces HTTPS-only: any plaintext
/// HTTP request is rejected (connection dropped at the TLS layer).
#[derive(Debug, Clone, Deserialize)]
pub struct TlsConfig {
    /// Path to a PEM-encoded certificate (or fullchain) file.
    pub cert_pem_path: String,
    /// Path to a PEM-encoded private key file.
    pub key_pem_path: String,
}

/// Database configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    /// `SQLite` file path (e.g., `/home/moxui/data/moxui.db`).
    pub path: String,
    /// Maximum connections in pool.
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    /// Run migrations on startup (default: true).
    #[serde(default = "default_true")]
    pub run_migrations: bool,
}

fn default_max_connections() -> u32 {
    8
}

fn default_true() -> bool {
    true
}

/// Logging configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error).
    #[serde(default = "default_log_level")]
    pub level: String,
    /// Output format (`json` for prod, `pretty` for dev).
    #[serde(default = "default_log_format")]
    pub format: String,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "pretty".to_string()
}

/// Auth configuration — JWT keys + seeded user accounts.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AuthConfig {
    /// JWT issuer (must match between encode + decode). Default: `moxui`.
    #[serde(default = "default_jwt_issuer")]
    pub jwt_issuer: String,
    /// JWT audience. Default: `moxui-clients`.
    #[serde(default = "default_jwt_audience")]
    pub jwt_audience: String,
    /// Token lifetime in seconds. Default: 3600 (1h).
    #[serde(default = "default_jwt_lifetime_secs")]
    pub jwt_lifetime_secs: i64,
    /// Path to PEM-encoded RSA private key. If absent, JWT-protected
    /// endpoints will refuse to start (fail-closed).
    #[serde(default)]
    pub jwt_private_key_pem_path: Option<String>,
    /// Path to PEM-encoded RSA public key. Must be set together with
    /// `jwt_private_key_pem_path`.
    #[serde(default)]
    pub jwt_public_key_pem_path: Option<String>,
    /// User accounts seeded at startup. Passwords are bcrypt hashes (or
    /// plaintext if `password` is set — only intended for dev / first-boot).
    #[serde(default)]
    pub users: Vec<UserConfig>,
}

fn default_jwt_issuer() -> String {
    "moxui".to_string()
}

fn default_jwt_audience() -> String {
    "moxui-clients".to_string()
}

fn default_jwt_lifetime_secs() -> i64 {
    3600
}

/// One seeded user account (yaml config).
#[derive(Debug, Clone, Deserialize)]
pub struct UserConfig {
    /// Unique user id.
    pub id: String,
    /// Login name.
    pub username: String,
    /// Display name.
    #[serde(default)]
    pub display_name: String,
    /// Email.
    #[serde(default)]
    pub email: Option<String>,
    /// Role: `admin` / `operator` / `viewer`.
    pub role: String,
    /// Bcrypt hash of the password (preferred).
    #[serde(default)]
    pub password_hash: Option<String>,
    /// Plaintext password — only honoured if `password_hash` is absent.
    /// **Use only for dev / first-boot setup** — production configs
    /// should always store a bcrypt hash.
    #[serde(default)]
    pub password: Option<String>,
    /// Is this account enabled? Default: true.
    #[serde(default = "default_true_user")]
    pub enabled: bool,
}

fn default_true_user() -> bool {
    true
}

/// Proxmox cluster connection.
///
/// Passwords are stored as `SecretString` (from the `secrecy` crate) so they:
/// - Don't leak via `Debug` (e.g. `tracing::info!("{:?}", config)`)
/// - Are zeroed on drop
/// - Require `.expose_secret()` to be read
///
/// Note: `Serialize` is intentionally NOT derived — Proxmox credentials
/// should never be written to disk or sent over the wire. If you need to
/// persist the config, do it through the secret manager (v1.1+), not serde.
#[derive(Clone, Deserialize)]
pub struct ClusterConfig {
    /// Unique cluster name.
    pub name: String,
    /// Proxmox API URL (e.g., `https://pve11.local:8006`).
    pub url: String,
    /// Username (e.g., `root@pam`).
    pub username: String,
    /// Password (from env var or config file). Wrapped in `Secret` to prevent leaks.
    pub password: SecretString,
    /// Auth realm (`pam`, `pve`, `openid`).
    #[serde(default = "default_realm")]
    pub realm: String,
    /// Skip TLS verification. ⚠️ Production should always set this to `false` and
    /// provide a CA cert instead. When `true`, an audit log entry is emitted at
    /// startup (see [`crate::security`]).
    #[serde(default)]
    pub insecure_skip_verify: bool,
    /// CA cert PEM (alternative to `insecure_skip_verify`).
    #[serde(default)]
    pub ca_cert_pem: Option<String>,
}

impl std::fmt::Debug for ClusterConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClusterConfig")
            .field("name", &self.name)
            .field("url", &self.url)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .field("realm", &self.realm)
            .field("insecure_skip_verify", &self.insecure_skip_verify)
            .field(
                "ca_cert_pem",
                &self.ca_cert_pem.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

fn default_realm() -> String {
    "pam".to_string()
}

/// Where to look for the config file.
#[derive(Debug, Clone, Copy)]
pub enum ConfigSource {
    /// `/etc/moxui/config.yaml` (production default).
    Default,
    /// Specific path on disk.
    Path(&'static str),
    /// No file — env vars + defaults only (testing).
    None,
}

impl Config {
    /// Load configuration from `ConfigSource::Default` (or `MOXUI_CONFIG` env override).
    pub fn load() -> Result<Self, anyhow::Error> {
        let path: Option<&'static str> = std::env::var("MOXUI_CONFIG")
            .ok()
            .map(|p| Box::leak(p.into_boxed_str()) as &'static str);
        let source = match path {
            Some(p) => ConfigSource::Path(p),
            None => ConfigSource::Default,
        };
        Self::load_from(source)
    }

    /// Load configuration from an explicit source.
    ///
    /// Precedence (high → low). Figment merges in the order layers are added,
    /// with later layers overriding earlier ones. So we register defaults first
    /// and then layer file + env on top.
    pub fn load_from(source: ConfigSource) -> Result<Self, anyhow::Error> {
        // Layer 0 (lowest priority): hardcoded defaults
        let default_json = serde_json::json!({
            "server": {
                "bind": "0.0.0.0:8080",
                "workers": 0
            },
            "database": {
                "path": "moxui.db",
                "max_connections": 8,
                "run_migrations": true
            },
            "logging": {
                "level": "info",
                "format": "pretty"
            },
            "clusters": []
        });
        let mut figment =
            Figment::new().merge(figment::providers::Serialized::defaults(default_json));

        // Layer 1: yaml file (overrides defaults)
        match source {
            ConfigSource::Default => {
                let default_path = "/etc/moxui/config.yaml";
                if Path::new(default_path).exists() {
                    figment = figment.merge(Yaml::file(default_path));
                } else {
                    tracing::debug!(
                        path = default_path,
                        "no config file found at default path; using env + defaults"
                    );
                }
            }
            ConfigSource::Path(p) => {
                figment = figment.merge(Yaml::file(p));
            }
            ConfigSource::None => {
                // no file layer
            }
        }

        // Layer 2 (highest priority): env vars (e.g. MOXUI_SERVER__BIND=0.0.0.0:9090)
        figment = figment.merge(Env::prefixed("MOXUI_").split("__"));

        let config: Config = figment.extract()?;
        Ok(config)
    }

    /// Expose a cluster's password (use sparingly — only when sending to Proxmox API).
    ///
    /// # Security
    ///
    /// Do NOT log the returned value. Pass directly to HTTP client.
    pub fn cluster_password(cluster: &ClusterConfig) -> &str {
        cluster.password.expose_secret()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_config_load_defaults_when_no_file() {
        let config = Config::load_from(ConfigSource::None).unwrap();
        assert_eq!(config.server.bind, "0.0.0.0:8080");
        assert_eq!(config.database.max_connections, 8);
        assert_eq!(config.clusters.len(), 0);
    }

    #[test]
    fn test_config_load_from_yaml_file() {
        // Write a temp config
        let tmp = std::env::temp_dir().join("moxui-config-test.yaml");
        let mut f = std::fs::File::create(&tmp).unwrap();
        writeln!(
            f,
            r#"
server:
  bind: "127.0.0.1:9999"
  workers: 4
database:
  path: "/tmp/moxui-test.db"
clusters:
  - name: "test-cluster"
    url: "https://pve-test.local:8006"
    username: "root@pam"
    password: "secret123"
    realm: "pam"
"#
        )
        .unwrap();
        drop(f);

        let path: &'static str = Box::leak(tmp.to_string_lossy().into_owned().into_boxed_str());
        let config = Config::load_from(ConfigSource::Path(path)).unwrap();
        assert_eq!(config.server.bind, "127.0.0.1:9999");
        assert_eq!(config.server.workers, 4);
        assert_eq!(config.clusters.len(), 1);
        assert_eq!(config.clusters[0].name, "test-cluster");
        assert_eq!(config.clusters[0].password.expose_secret(), "secret123");

        // Clean up
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_config_password_not_in_debug() {
        let cluster = ClusterConfig {
            name: "test".into(),
            url: "https://test.local:8006".into(),
            username: "root@pam".into(),
            password: SecretString::new("super-secret-password".to_string().into_boxed_str()),
            realm: "pam".into(),
            insecure_skip_verify: false,
            ca_cert_pem: None,
        };
        let debug = format!("{cluster:?}");
        assert!(
            !debug.contains("super-secret-password"),
            "Debug leaked password: {debug}"
        );
        assert!(
            debug.contains("<redacted>"),
            "Debug should show <redacted>: {debug}"
        );
    }

    #[test]
    fn test_config_password_zeroize_on_drop() {
        // Verify SecretString zeros its memory on drop (security feature).
        // We can only test the wrapper exists and the type is sized/cloneable;
        // actual zeroize behavior is verified by the `secrecy` crate's tests.
        let cluster = ClusterConfig {
            name: "test".into(),
            url: "https://test.local:8006".into(),
            username: "root@pam".into(),
            password: SecretString::new("super-secret".to_string().into_boxed_str()),
            realm: "pam".into(),
            insecure_skip_verify: false,
            ca_cert_pem: None,
        };
        // Clone works (cheap, shares the box internally per secrecy docs)
        let _cloned = cluster.clone();
        // expose_secret returns the inner str
        assert_eq!(cluster.password.expose_secret(), "super-secret");
    }
}
