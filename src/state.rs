//! Shared application state.
//!
//! Holds all dependencies that handlers need via `axum::extract::State<AppState>`.

use std::sync::Arc;

use crate::config::Config;
use crate::proxmox::ProxmoxClient;

/// Shared application state (cloned for each handler).
#[derive(Clone)]
pub struct AppState {
    /// Application configuration.
    pub config: Arc<Config>,
    /// One Proxmox client per cluster (same order as `config.clusters`).
    /// Empty if no clusters are configured.
    pub clients: Arc<Vec<ProxmoxClient>>,
}

impl AppState {
    /// Create new state from a loaded config and pre-built Proxmox clients.
    ///
    /// Build the `Vec<ProxmoxClient>` ahead of time (e.g. in `main`) so
    /// handler timeouts/failures happen at startup, not on first request.
    pub fn new(config: Config, clients: Vec<ProxmoxClient>) -> Self {
        Self {
            config: Arc::new(config),
            clients: Arc::new(clients),
        }
    }

    /// Look up a Proxmox client by cluster name.
    pub fn client(&self, name: &str) -> Option<&ProxmoxClient> {
        self.clients.iter().find(|c| c.name() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ClusterConfig;
    use secrecy::SecretString;

    fn test_config() -> Config {
        Config {
            server: crate::config::ServerConfig {
                bind: "0.0.0.0:8080".to_string(),
                workers: 0,
            },
            database: crate::config::DatabaseConfig {
                path: "moxui.db".to_string(),
                max_connections: 8,
                run_migrations: true,
            },
            logging: crate::config::LoggingConfig {
                level: "info".to_string(),
                format: "pretty".to_string(),
            },
            clusters: vec![],
        }
    }

    #[tokio::test]
    async fn test_state_creation_empty() {
        let state = AppState::new(test_config(), vec![]);
        assert_eq!(state.config.server.bind, "0.0.0.0:8080");
        assert_eq!(state.clients.len(), 0);
        assert!(state.client("anything").is_none());
    }

    #[tokio::test]
    async fn test_state_look_up_client_by_name() {
        let cfg = test_config();
        // Make two clients in two separate runtimes (each `new` returns a `Self`)
        let cluster1 = ClusterConfig {
            name: "homelab".to_string(),
            url: "https://pve-homelab:8006".to_string(),
            username: "root@pam".to_string(),
            password: SecretString::new("x".to_string().into_boxed_str()),
            realm: "pam".to_string(),
            insecure_skip_verify: true,
            ca_cert_pem: None,
        };
        let cluster2 = ClusterConfig {
            name: "prod".to_string(),
            url: "https://pve-prod:8006".to_string(),
            username: "root@pam".to_string(),
            password: SecretString::new("y".to_string().into_boxed_str()),
            realm: "pam".to_string(),
            insecure_skip_verify: true,
            ca_cert_pem: None,
        };
        let c1 = ProxmoxClient::new(cluster1).await.unwrap();
        let c2 = ProxmoxClient::new(cluster2).await.unwrap();
        let state = AppState::new(cfg, vec![c1, c2]);

        assert_eq!(state.clients.len(), 2);
        assert!(state.client("homelab").is_some());
        assert!(state.client("prod").is_some());
        assert!(state.client("nonexistent").is_none());
    }
}
