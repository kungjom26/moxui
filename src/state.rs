//! Shared application state.
//!
//! Holds all dependencies that handlers need via `axum::extract::State<AppState>`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use crate::audit::AuditStore;
use crate::auth::{JwtService, UserStore};
use crate::config::Config;
use crate::proxmox::ProxmoxClient;

/// Default cache TTL for Proxmox readiness probes.
pub const READINESS_CACHE_TTL: Duration = Duration::from_secs(10);

/// Per-cluster reachability result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusterStatus {
    /// Cluster is reachable (ping returned 2xx).
    Healthy,
    /// Cluster is unreachable (ping failed).
    Unhealthy,
}

impl ClusterStatus {
    /// Returns `true` if the cluster is reachable.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }
}

/// Aggregated readiness snapshot for all configured clusters.
#[derive(Debug, Clone)]
pub struct ReadinessSnapshot {
    /// Per-cluster status, keyed by cluster name.
    pub clusters: HashMap<String, ClusterStatus>,
    /// When the snapshot was taken.
    pub checked_at: Instant,
}

impl ReadinessSnapshot {
    /// Returns `true` if every configured cluster is healthy.
    /// Empty cluster list → ready (nothing to check).
    pub fn all_healthy(&self) -> bool {
        self.clusters.values().all(ClusterStatus::is_healthy)
    }
}

/// Shared application state (cloned for each handler).
#[derive(Clone)]
pub struct AppState {
    /// Application configuration.
    pub config: Arc<Config>,
    /// One Proxmox client per cluster (same order as `config.clusters`).
    /// Empty if no clusters are configured.
    pub clients: Arc<Vec<ProxmoxClient>>,
    /// Cached readiness results per cluster name (TTL: `READINESS_CACHE_TTL`).
    readiness: Arc<RwLock<HashMap<String, (ClusterStatus, Instant)>>>,
    /// SQLite-backed audit log store. Cloned for each handler — cheap.
    pub audit: Arc<AuditStore>,
    /// JWT encoder/decoder (shared, `Clone`-cheap).
    pub jwt: Arc<JwtService>,
    /// In-memory user store. Cheap to clone (`Arc` inside).
    pub users: Arc<UserStore>,
}

impl AppState {
    /// Create new state from a loaded config and pre-built Proxmox clients.
    ///
    /// Build the `Vec<ProxmoxClient>` ahead of time (e.g. in `main`) so
    /// handler timeouts/failures happen at startup, not on first request.
    pub fn new(
        config: Config,
        clients: Vec<ProxmoxClient>,
        audit: Arc<AuditStore>,
        jwt: JwtService,
        users: UserStore,
    ) -> Self {
        Self {
            config: Arc::new(config),
            clients: Arc::new(clients),
            readiness: Arc::new(RwLock::new(HashMap::new())),
            audit,
            jwt: Arc::new(jwt),
            users: Arc::new(users),
        }
    }

    /// Look up a Proxmox client by cluster name.
    pub fn client(&self, name: &str) -> Option<&ProxmoxClient> {
        self.clients.iter().find(|c| c.name() == name)
    }

    /// Iterate over all configured Proxmox clients.
    pub fn clients(&self) -> impl Iterator<Item = &ProxmoxClient> {
        self.clients.iter()
    }

    /// Return a freshness-guarded readiness snapshot for all clusters.
    ///
    /// Reads the cache first; only re-pings clusters whose cached entry is
    /// older than `READINESS_CACHE_TTL`. Each ping is run concurrently with
    /// `tokio::join_all` so a slow/unreachable cluster does not stall the
    /// rest.
    pub async fn readiness(&self) -> ReadinessSnapshot {
        let now = Instant::now();

        // Step 1: figure out which clusters need a fresh probe.
        let mut to_probe: Vec<&ProxmoxClient> = Vec::new();
        {
            let cache = self.readiness.read().await;
            for client in self.clients.iter() {
                let stale = cache.get(client.name()).map_or(true, |(_, ts)| {
                    now.duration_since(*ts) > READINESS_CACHE_TTL
                });
                if stale {
                    to_probe.push(client);
                }
            }
        }

        // Step 2: ping only stale clusters in parallel.
        if !to_probe.is_empty() {
            let results: Vec<(&str, ClusterStatus)> = {
                let futs = to_probe.iter().map(|c| async move {
                    let status = if c.ping().await.is_ok() {
                        ClusterStatus::Healthy
                    } else {
                        ClusterStatus::Unhealthy
                    };
                    (c.name(), status)
                });
                futures_util::future::join_all(futs).await
            };

            let mut cache = self.readiness.write().await;
            for (name, status) in results {
                cache.insert(name.to_string(), (status, Instant::now()));
            }
        }

        // Step 3: snapshot the (now-fresh) cache.
        let cache = self.readiness.read().await;
        let clusters: HashMap<String, ClusterStatus> =
            cache.iter().map(|(k, (s, _))| (k.clone(), *s)).collect();
        ReadinessSnapshot {
            clusters,
            checked_at: now,
        }
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
                tls: None,
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
            auth: crate::config::AuthConfig::default(),
        }
    }

    fn test_jwt() -> JwtService {
        const PRIV_PEM: &str = include_str!("../tests/fixtures/test_jwt_priv.pem");
        const PUB_PEM: &str = include_str!("../tests/fixtures/test_jwt_pub.pem");
        JwtService::new(PRIV_PEM.as_bytes(), PUB_PEM.as_bytes(), "test", "test")
            .expect("test keypair")
    }

    #[tokio::test]
    async fn test_state_creation_empty() {
        let audit = std::sync::Arc::new(crate::audit::AuditStore::open_in_memory().unwrap());
        let state = AppState::new(test_config(), vec![], audit, test_jwt(), UserStore::new());
        assert_eq!(state.config.server.bind, "0.0.0.0:8080");
        assert_eq!(state.clients.len(), 0);
        assert!(state.client("anything").is_none());
    }

    #[tokio::test]
    async fn test_state_look_up_client_by_name() {
        let cfg = test_config();
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
        let audit = std::sync::Arc::new(crate::audit::AuditStore::open_in_memory().unwrap());
        let state = AppState::new(cfg, vec![c1, c2], audit, test_jwt(), UserStore::new());

        assert_eq!(state.clients.len(), 2);
        assert!(state.client("homelab").is_some());
        assert!(state.client("prod").is_some());
        assert!(state.client("nonexistent").is_none());
        assert_eq!(state.clients().count(), 2);
    }

    #[tokio::test]
    async fn test_readiness_with_no_clusters_is_ready() {
        let audit = std::sync::Arc::new(crate::audit::AuditStore::open_in_memory().unwrap());
        let state = AppState::new(test_config(), vec![], audit, test_jwt(), UserStore::new());
        let snap = state.readiness().await;
        assert!(snap.all_healthy(), "empty cluster list should be ready");
        assert_eq!(snap.clusters.len(), 0);
    }

    #[test]
    fn test_cluster_status_predicates() {
        assert!(ClusterStatus::Healthy.is_healthy());
        assert!(!ClusterStatus::Unhealthy.is_healthy());
    }
}
