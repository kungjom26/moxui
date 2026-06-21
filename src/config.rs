//! Configuration loading (figment: env + yaml + toml).
//!
//! Loads `MoxUI` configuration from multiple sources (priority high → low):
//! 1. CLI args (clap)
//! 2. Environment variables (`MOXUI_*`)
//! 3. Config file (`config.yaml` / `config.toml`)
//! 4. Defaults

use serde::{Deserialize, Serialize};

/// Top-level configuration for `MoxUI`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Server bind address.
    pub server: ServerConfig,
    /// Database configuration.
    pub database: DatabaseConfig,
    /// Logging configuration.
    pub logging: LoggingConfig,
    /// Proxmox clusters to connect to.
    pub clusters: Vec<ClusterConfig>,
}

/// HTTP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Bind address (e.g., `0.0.0.0:8080`).
    pub bind: String,
    /// Number of worker threads (0 = num CPUs).
    #[serde(default)]
    pub workers: usize,
}

/// Database configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Proxmox cluster connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// Unique cluster name.
    pub name: String,
    /// Proxmox API URL (e.g., `https://pve11.local:8006`).
    pub url: String,
    /// Username (e.g., `root@pam`).
    pub username: String,
    /// Password (from env var or config file).
    pub password: String,
    /// Auth realm (`pam`, `pve`, `openid`).
    #[serde(default = "default_realm")]
    pub realm: String,
    /// Skip TLS verification (insecure, OK for internal self-signed).
    #[serde(default)]
    pub insecure_skip_verify: bool,
    /// CA cert PEM (alternative to `insecure_skip_verify`).
    #[serde(default)]
    pub ca_cert_pem: Option<String>,
}

fn default_realm() -> String {
    "pam".to_string()
}

impl Config {
    /// Load configuration from default sources.
    pub fn load() -> Result<Self, anyhow::Error> {
        // TODO: implement figment loading
        Ok(Self {
            server: ServerConfig {
                bind: "0.0.0.0:8080".to_string(),
                workers: 0,
            },
            database: DatabaseConfig {
                path: "moxui.db".to_string(),
                max_connections: 8,
                run_migrations: true,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "pretty".to_string(),
            },
            clusters: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_load_default() {
        let config = Config::load().unwrap();
        assert_eq!(config.server.bind, "0.0.0.0:8080");
    }
}
