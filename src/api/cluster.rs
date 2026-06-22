//! Cluster-wide status and configuration endpoints.
//!
//! These endpoints return cluster-level information from Proxmox:
//! cluster status (nodes), cluster config (datacenter), cluster options,
//! cluster audit log, and recent cluster tasks.
//!
//! Since no single `:cluster` param is in the route path, we aggregate
//! responses across all configured clusters for each endpoint.

use std::collections::HashMap;

use axum::{extract::State, Json};

use crate::state::AppState;

/// Helper: collect a list of (cluster_name, value) from all clients.
async fn collect_aggregated<T, F>(state: &AppState, fetch: F) -> HashMap<String, T>
where
    T: serde::de::DeserializeOwned,
    F: Fn(&crate::proxmox::client::ProxmoxClient) -> String + Send + Copy,
    T: Send + 'static,
{
    use futures_util::future::join_all;

    let futs: Vec<_> = state
        .clients()
        .map(|c| {
            let name = c.name().to_string();
            let path = fetch(c);
            async move {
                let result: Result<T, _> = c.get(&path).await;
                (name, result)
            }
        })
        .collect();

    let results = join_all(futs).await;

    let mut data = HashMap::new();
    for (name, result) in results {
        match result {
            Ok(value) => {
                data.insert(name, value);
            }
            Err(e) => {
                tracing::warn!("cluster endpoint error for '{name}': {e}");
            }
        }
    }
    data
}

/// `GET /api/v1/cluster/status` — cluster status for all configured clusters.
///
/// Returns per-cluster status information (node list, quorum state, etc.).
/// This is a read-only endpoint — auth middleware at the router level
/// handles access control.
pub async fn cluster_status(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let data: HashMap<String, serde_json::Value> =
        collect_aggregated(&state, |_c| "cluster/status".to_string()).await;

    let mut errors: HashMap<String, String> = HashMap::new();
    let mut statuses = Vec::new();
    for (cluster, value) in data {
        statuses.push(serde_json::json!({
            "cluster": cluster,
            "data": value,
        }));
    }

    Json(serde_json::json!({
        "clusters": statuses,
        "errors": errors,
    }))
}

/// `GET /api/v1/cluster/config` — datacenter/cluster config for all clusters.
///
/// Returns cluster configuration options. Read-only — auth middleware
/// at the router level handles access control.
pub async fn cluster_config(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let data: HashMap<String, serde_json::Value> =
        collect_aggregated(&state, |_c| "cluster/config".to_string()).await;

    let mut configs = Vec::new();
    for (cluster, value) in data {
        configs.push(serde_json::json!({
            "cluster": cluster,
            "data": value,
        }));
    }

    Json(serde_json::json!({
        "clusters": configs,
    }))
}

/// `GET /api/v1/cluster/options` — datacenter options for all clusters.
///
/// Returns datacenter-wide options (keyboard layout, console, etc.).
/// Read-only — auth middleware at the router level handles access control.
pub async fn cluster_options(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    // Proxmox returns cluster options at /cluster/options.
    let data: HashMap<String, serde_json::Value> =
        collect_aggregated(&state, |_c| "cluster/options".to_string()).await;

    let mut options = Vec::new();
    for (cluster, value) in data {
        options.push(serde_json::json!({
            "cluster": cluster,
            "data": value,
        }));
    }

    Json(serde_json::json!({
        "clusters": options,
    }))
}

/// `GET /api/v1/cluster/log` — cluster audit log for all clusters.
///
/// Returns recent cluster log entries. The Proxmox endpoint is
/// `/cluster/log` and returns the cluster audit log.
/// Read-only — auth middleware at the router level handles access control.
pub async fn cluster_log(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let data: HashMap<String, serde_json::Value> =
        collect_aggregated(&state, |_c| "cluster/log".to_string()).await;

    let mut logs = Vec::new();
    for (cluster, value) in data {
        logs.push(serde_json::json!({
            "cluster": cluster,
            "data": value,
        }));
    }

    Json(serde_json::json!({
        "clusters": logs,
    }))
}

/// `GET /api/v1/cluster/tasks` — recent cluster tasks for all clusters.
///
/// Returns recent task history across the cluster. The Proxmox
/// endpoint is `/cluster/tasks` and returns a list of recent tasks.
/// Read-only — auth middleware at the router level handles access control.
pub async fn cluster_tasks(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let data: HashMap<String, serde_json::Value> =
        collect_aggregated(&state, |_c| "cluster/tasks".to_string()).await;

    let mut all_tasks = Vec::new();
    for (cluster, value) in data {
        all_tasks.push(serde_json::json!({
            "cluster": cluster,
            "data": value,
        }));
    }

    Json(serde_json::json!({
        "clusters": all_tasks,
    }))
}
