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
use serde::{Deserialize, Serialize};

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
    /// OpenTelemetry tracing configuration.
    #[serde(default)]
    pub tracing: crate::observability::tracing::TracingConfig,
    /// Webhook notification configuration.
    #[serde(default)]
    pub webhook: WebhookConfig,
    /// Data directory for persistent storage (dashboards, etc.).
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
    /// Enabled plugin names (e.g. `["audit_logger", "webhook_bridge"]`).
    #[serde(default)]
    pub plugins: Vec<String>,
}

fn default_data_dir() -> String {
    "/var/lib/moxui".to_string()
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
    /// HMAC secret used to sign short-lived VNC session tokens.
    /// Required when the VNC endpoint is enabled — the server
    /// fails closed if it's absent. Minimum 32 bytes recommended.
    /// Path is read at startup; secret lives in process memory only.
    #[serde(default)]
    pub vnc_token_secret_pem_path: Option<String>,
    /// Rate limiting configuration (tower-governor).
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
    /// CORS configuration (tower-http cors).
    #[serde(default)]
    pub cors: CorsConfig,
    /// API key-based authentication (alternative to JWT).
    #[serde(default)]
    pub api_key: ApiKeyConfig,
    /// WebAuthn / passkey configuration.
    #[serde(default)]
    pub webauthn: WebauthnConfig,
    /// OIDC / OAuth2 SSO configuration (Google, GitHub).
    #[serde(default)]
    pub oidc: OidcConfig,
    /// LDAP / Active Directory authentication configuration.
    #[serde(default)]
    pub ldap: LdapConfig,
}

/// WebAuthn / passkey configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct WebauthnConfig {
    /// Whether WebAuthn / passkey auth is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Relying Party ID (domain, e.g. `moxui.example.com`).
    #[serde(default = "default_webauthn_rp_id")]
    pub rp_id: String,
    /// Relying Party origin (e.g. `https://moxui.example.com`).
    #[serde(default = "default_webauthn_rp_origin")]
    pub rp_origin: String,
    /// Relying Party display name.
    #[serde(default = "default_webauthn_rp_name")]
    pub rp_name: String,
}

fn default_webauthn_rp_id() -> String {
    "localhost".to_string()
}
fn default_webauthn_rp_origin() -> String {
    "http://localhost:8080".to_string()
}
fn default_webauthn_rp_name() -> String {
    "MoxUI".to_string()
}

impl Default for WebauthnConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            rp_id: "localhost".to_string(),
            rp_origin: "http://localhost:8080".to_string(),
            rp_name: "MoxUI".to_string(),
        }
    }
}

/// OIDC / OAuth2 SSO configuration.
///
/// When enabled, users can sign in via Google or GitHub OIDC/OAuth2.
/// Providers are configured under `auth.oidc.providers`.
#[derive(Debug, Clone, Deserialize)]
pub struct OidcConfig {
    /// Enable OIDC / OAuth2 SSO.
    #[serde(default)]
    pub enabled: bool,
    /// OIDC / OAuth2 provider configurations.
    #[serde(default)]
    pub providers: Vec<OidcProviderConfig>,
}

impl Default for OidcConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            providers: vec![],
        }
    }
}

/// LDAP / Active Directory authentication configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LdapConfig {
    /// Enable LDAP / AD authentication.
    #[serde(default)]
    pub enabled: bool,
    /// LDAP server URL (e.g., `ldap://dc01.example.com:389` or `ldaps://...:636`).
    pub url: Option<String>,
    /// Base DN for user searches (e.g., `dc=example,dc=com`).
    pub base_dn: Option<String>,
    /// Bind DN for initial LDAP connection (e.g., `cn=admin,dc=example,dc=com`).
    pub bind_dn: Option<String>,
    /// Password for the bind DN.
    pub bind_password: Option<String>,
    /// LDAP search filter template. `{username}` is replaced with the login name.
    /// Default: `(&(objectClass=person)(uid={username}))`.
    #[serde(default = "default_ldap_filter")]
    pub user_filter: String,
    /// Attribute name that holds the username (default: `uid`).
    #[serde(default = "default_ldap_username_attr")]
    pub username_attr: String,
    /// Attribute name that holds the display name (default: `displayName`).
    #[serde(default = "default_ldap_displayname_attr")]
    pub displayname_attr: String,
    /// Attribute name that holds the email (default: `mail`).
    #[serde(default = "default_ldap_email_attr")]
    pub email_attr: String,
    /// Default role assigned to new LDAP users (default: `viewer`).
    #[serde(default = "default_ldap_default_role")]
    pub default_role: String,
    /// Automatically create user accounts on successful LDAP login (default: true).
    #[serde(default = "default_true")]
    pub auto_create: bool,
}

fn default_ldap_filter() -> String {
    "(&(objectClass=person)(uid={username}))".to_string()
}
fn default_ldap_username_attr() -> String {
    "uid".to_string()
}
fn default_ldap_displayname_attr() -> String {
    "displayName".to_string()
}
fn default_ldap_email_attr() -> String {
    "mail".to_string()
}
fn default_ldap_default_role() -> String {
    "viewer".to_string()
}

/// Configuration for a single OIDC / OAuth2 provider (Google or GitHub).
#[derive(Debug, Clone, Deserialize)]
pub struct OidcProviderConfig {
    /// Provider name. Must be `"google"` or `"github"`.
    pub name: String,
    /// OAuth2 client ID.
    pub client_id: String,
    /// OAuth2 client secret.
    pub client_secret: String,
    /// Redirect URL registered with the provider
    /// (e.g. `http://localhost:8080/api/v1/auth/oidc/callback`).
    pub redirect_url: String,
}

/// Rate limiting configuration (tower-governor).
#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum requests per second per IP.
    #[serde(default = "default_rate_per_sec")]
    pub requests_per_second: u64,
    /// Burst size — peak requests allowed before rate limiting kicks in.
    #[serde(default = "default_rate_burst")]
    pub burst_size: u32,
}

fn default_rate_per_sec() -> u64 {
    5
}
fn default_rate_burst() -> u32 {
    10
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_second: 5,
            burst_size: 10,
        }
    }
}

/// CORS configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct CorsConfig {
    /// Allowed origins (e.g. `https://moxui.example.com`).
    /// Empty = allow all origins (development).
    #[serde(default)]
    pub allowed_origins: Vec<String>,
    /// Max age for preflight cache in seconds (default 86400 = 24h).
    #[serde(default = "default_cors_max_age")]
    pub max_age_secs: u64,
}

fn default_cors_max_age() -> u64 {
    86400
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: vec![],
            max_age_secs: 86400,
        }
    }
}

/// API key authentication configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ApiKeyConfig {
    /// Enable API key authentication.
    #[serde(default)]
    pub enabled: bool,
    /// The API key value (shared secret). Set via config file or
    /// `MOXUI_AUTH__API_KEY__KEY` env var.
    #[serde(default)]
    pub key: Option<String>,
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
    /// Clusters this user is allowed to access.
    /// Empty = access to all clusters (admin default).
    /// When non-empty, the user can only see/operate on these clusters.
    /// Cluster names must match the `name` field in `ClusterConfig`.
    /// For admin users, this is typically left empty.
    #[serde(default)]
    pub allowed_clusters: Vec<String>,
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

/// Webhook notification configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct WebhookConfig {
    /// Whether webhook dispatch is enabled globally.
    #[serde(default)]
    pub enabled: bool,
    /// Default retry count for webhook delivery.
    #[serde(default = "default_webhook_retry")]
    pub retry_count: u32,
    /// Timeout in seconds for webhook HTTP requests.
    #[serde(default = "default_webhook_timeout")]
    pub timeout_secs: u64,
    /// Webhook endpoint configurations.
    #[serde(default)]
    pub endpoints: Vec<WebhookEndpointConfig>,
}

fn default_webhook_retry() -> u32 {
    3
}

fn default_webhook_timeout() -> u64 {
    10
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            retry_count: 3,
            timeout_secs: 10,
            endpoints: vec![],
        }
    }
}

/// Configuration for a single webhook endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct WebhookEndpointConfig {
    /// Human-readable name for the endpoint (e.g. "Slack #ops").
    pub name: String,
    /// URL to send the webhook POST to.
    pub url: String,
    /// Event types this endpoint subscribes to (e.g. `["VmStarted", "VmStopped"]`).
    /// Empty list subscribes to all events.
    #[serde(default)]
    pub events: Vec<String>,
    /// Optional HMAC secret for signing payloads (HMAC-SHA256).
    #[serde(default)]
    pub secret: Option<String>,
    /// Maximum retries for this endpoint (overrides global default).
    #[serde(default)]
    pub max_retries: Option<u32>,
}

/// Custom dashboard configuration per user.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CustomDashboardConfig {
    /// User ID this config belongs to.
    pub user_id: String,
    /// Widget configurations.
    #[serde(default)]
    pub widgets: Vec<WidgetConfig>,
    /// Layout rows defining the grid.
    #[serde(default)]
    pub layout: Vec<LayoutRow>,
}

/// Configuration for a single dashboard widget.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetConfig {
    /// Unique widget ID (e.g. "widget-1").
    pub id: String,
    /// Widget type.
    #[serde(rename = "type")]
    pub widget_type: String,
    /// Widget title (optional override).
    #[serde(default)]
    pub title: Option<String>,
    /// Grid position (column start, 1-indexed, 1-12).
    pub x: u32,
    /// Grid position (row start).
    pub y: u32,
    /// Widget width in grid columns (1-12).
    pub width: u32,
    /// Widget height in grid rows.
    pub height: u32,
}

/// A layout row in the custom dashboard grid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutRow {
    /// Row index (1-based).
    pub row: u32,
    /// Widget IDs in this row, in order.
    pub widgets: Vec<String>,
}

/// All custom dashboard configs, keyed by user ID, serialized as JSON file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AllDashboardConfigs {
    /// Map of user_id -> dashboard config.
    pub dashboards: std::collections::HashMap<String, CustomDashboardConfig>,
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
