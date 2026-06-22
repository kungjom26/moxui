//! Custom per-user dashboard configuration.
//!
//! Users can create their own dashboard layouts with draggable widgets
//! from available types: VM list, cluster health, resource usage, recent
//! audit, HA status, Ceph status.
//!
//! Config is persisted as a JSON file in the data directory.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::config::{AllDashboardConfigs, CustomDashboardConfig, WidgetConfig};

/// Service for managing custom dashboard configurations.
///
/// Configs are loaded from a JSON file at startup and written back on
/// every mutation. In-memory storage (`HashMap<String, CustomDashboardConfig>`)
/// provides fast reads.
pub struct DashboardCustomService {
    /// In-memory dashboard configs keyed by user ID.
    dashboards: Arc<RwLock<HashMap<String, CustomDashboardConfig>>>,
    /// Path to the persisted config file.
    file_path: PathBuf,
}

impl DashboardCustomService {
    /// Create a new service, loading existing configs from the data directory.
    ///
    /// If the config file doesn't exist, starts with an empty map.
    /// The file will be created on the first save.
    pub async fn new(data_dir: &str) -> Self {
        let path = PathBuf::from(data_dir).join("dashboards.json");
        let dashboards = if path.exists() {
            match tokio::fs::read_to_string(&path).await {
                Ok(content) => {
                    match serde_json::from_str::<AllDashboardConfigs>(&content) {
                        Ok(cfg) => {
                            info!(
                                path = %path.display(),
                                count = cfg.dashboards.len(),
                                "Loaded custom dashboard configs"
                            );
                            cfg.dashboards
                        }
                        Err(e) => {
                            warn!(
                                path = %path.display(),
                                error = %e,
                                "Failed to parse dashboards.json — starting empty"
                            );
                            HashMap::new()
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        path = %path.display(),
                        error = %e,
                        "Failed to read dashboards.json — starting empty"
                    );
                    HashMap::new()
                }
            }
        } else {
            HashMap::new()
        };

        Self {
            dashboards: Arc::new(RwLock::new(dashboards)),
            file_path: path,
        }
    }

    /// Create an in-memory service (no file I/O). Used in tests.
    pub fn new_in_memory() -> Self {
        Self {
            dashboards: Arc::new(RwLock::new(HashMap::new())),
            file_path: PathBuf::from("/tmp/moxui-test/dashboards.json"),
        }
    }

    /// Get the dashboard config for a specific user.
    ///
    /// Returns a default config if the user has no saved dashboard.
    pub async fn get_dashboard(&self, user_id: &str) -> CustomDashboardConfig {
        let map = self.dashboards.read().await;
        map.get(user_id).cloned().unwrap_or_else(|| {
            // Return a default dashboard with some starter widgets
            CustomDashboardConfig {
                user_id: user_id.to_string(),
                widgets: vec![
                    WidgetConfig {
                        id: "cluster-health-1".to_string(),
                        widget_type: "cluster_health".to_string(),
                        title: Some("Cluster Health".to_string()),
                        x: 1,
                        y: 1,
                        width: 6,
                        height: 2,
                    },
                    WidgetConfig {
                        id: "vm-list-1".to_string(),
                        widget_type: "vm_list".to_string(),
                        title: Some("VM Overview".to_string()),
                        x: 7,
                        y: 1,
                        width: 6,
                        height: 2,
                    },
                ],
                layout: vec![
                    crate::config::LayoutRow {
                        row: 1,
                        widgets: vec!["cluster-health-1".to_string(), "vm-list-1".to_string()],
                    },
                ],
            }
        })
    }

    /// Save (create or update) the dashboard config for a specific user.
    ///
    /// Persists to the JSON file after mutation.
    pub async fn save_dashboard(
        &self,
        user_id: &str,
        config: CustomDashboardConfig,
    ) -> Result<(), String> {
        {
            let mut map = self.dashboards.write().await;
            map.insert(user_id.to_string(), config);
        }
        self.persist().await
    }

    /// Delete the dashboard config for a specific user.
    pub async fn delete_dashboard(&self, user_id: &str) -> Result<(), String> {
        {
            let mut map = self.dashboards.write().await;
            map.remove(user_id);
        }
        self.persist().await
    }

    /// List all available widget types with metadata.
    #[must_use]
    pub fn available_widget_types() -> Vec<WidgetTypeInfo> {
        vec![
            WidgetTypeInfo {
                widget_type: "vm_list",
                label: "VM List",
                description: "List of virtual machines with status",
                default_width: 6,
                default_height: 3,
            },
            WidgetTypeInfo {
                widget_type: "cluster_health",
                label: "Cluster Health",
                description: "Health status of all clusters",
                default_width: 6,
                default_height: 2,
            },
            WidgetTypeInfo {
                widget_type: "resource_usage",
                label: "Resource Usage",
                description: "CPU and memory usage overview",
                default_width: 4,
                default_height: 2,
            },
            WidgetTypeInfo {
                widget_type: "recent_audit",
                label: "Recent Audit",
                description: "Recent audit log entries",
                default_width: 12,
                default_height: 3,
            },
            WidgetTypeInfo {
                widget_type: "ha_status",
                label: "HA Status",
                description: "High availability group status",
                default_width: 6,
                default_height: 2,
            },
            WidgetTypeInfo {
                widget_type: "ceph_status",
                label: "Ceph Status",
                description: "Ceph storage cluster health",
                default_width: 6,
                default_height: 2,
            },
        ]
    }

    /// Persist the current state to the JSON file.
    async fn persist(&self) -> Result<(), String> {
        // Ensure parent directory exists
        if let Some(parent) = self.file_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("Failed to create data directory: {e}"))?;
        }

        let map = self.dashboards.read().await;
        let all = AllDashboardConfigs {
            dashboards: map.clone(),
        };
        let content = serde_json::to_string_pretty(&all)
            .map_err(|e| format!("Failed to serialize dashboards: {e}"))?;

        tokio::fs::write(&self.file_path, &content)
            .await
            .map_err(|e| format!("Failed to write dashboard config: {e}"))?;

        info!(
            path = %self.file_path.display(),
            "Dashboard configs persisted"
        );
        Ok(())
    }
}

/// Metadata about an available widget type.
pub struct WidgetTypeInfo {
    /// Widget type identifier (e.g. "vm_list").
    pub widget_type: &'static str,
    /// Human-readable label.
    pub label: &'static str,
    /// Short description.
    pub description: &'static str,
    /// Default grid width.
    pub default_width: u32,
    /// Default grid height.
    pub default_height: u32,
}
