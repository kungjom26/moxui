//! Dashboard endpoint — aggregate cluster health and resource stats.
//!
//! `GET /api/v1/dashboard` returns a per-cluster breakdown plus
//! aggregate totals across all configured Proxmox clusters.
//!
//! ## Per-cluster permissions
//!
//! The dashboard filters clusters by the calling user's `allowed_clusters`.
//! Admin users (or users with no cluster restrictions) see all clusters.
//! Restricted users see only their explicitly allowed clusters.

use axum::{extract::State, Json};
use serde::Serialize;

use crate::auth::AuthContext;
use crate::error::AppResult;
use crate::proxmox::types::{StorageResource, Version, VmResource};
use crate::state::AppState;

/// Per-cluster dashboard data.
#[derive(Debug, Clone, Serialize)]
pub struct ClusterDashboard {
    /// Cluster name.
    pub name: String,
    /// Whether the cluster is reachable (version endpoint responded).
    pub reachable: bool,
    /// Proxmox VE version string, if reachable.
    pub version: Option<String>,
    /// Proxmox VE release string, if reachable.
    pub release: Option<String>,
    /// Total QEMU VMs.
    pub vms_total: u32,
    /// Running QEMU VMs.
    pub vms_running: u32,
    /// Stopped QEMU VMs.
    pub vms_stopped: u32,
    /// Total LXC containers.
    pub lxcs_total: u32,
    /// Running LXC containers.
    pub lxcs_running: u32,
    /// Stopped LXC containers.
    pub lxcs_stopped: u32,
    /// Number of storage pools.
    pub storage_pools: u32,
    /// Total allocated CPU cores (across all VMs and LXCs).
    pub allocated_cpus: f64,
    /// Total configured memory in bytes (across all VMs and LXCs).
    pub allocated_memory: u64,
    /// Error message if the cluster data could not be fully fetched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Aggregate totals across all clusters.
#[derive(Debug, Clone, Serialize)]
pub struct DashboardTotals {
    /// Total QEMU VMs across all clusters.
    pub vms_total: u32,
    /// Total running QEMU VMs across all clusters.
    pub vms_running: u32,
    /// Total stopped QEMU VMs across all clusters.
    pub vms_stopped: u32,
    /// Total LXC containers across all clusters.
    pub lxcs_total: u32,
    /// Total running LXC containers across all clusters.
    pub lxcs_running: u32,
    /// Total stopped LXC containers across all clusters.
    pub lxcs_stopped: u32,
    /// Total storage pools across all clusters.
    pub storage_pools: u32,
    /// Total allocated CPU cores across all clusters.
    pub allocated_cpus: f64,
    /// Total configured memory in bytes across all clusters.
    pub allocated_memory: u64,
}

/// `GET /api/v1/dashboard` response payload.
#[derive(Debug, Clone, Serialize)]
pub struct DashboardResponse {
    /// Per-cluster breakdown.
    pub clusters: Vec<ClusterDashboard>,
    /// Aggregate totals across all clusters.
    pub totals: DashboardTotals,
}

/// `GET /api/v1/dashboard` — aggregate dashboard across all clusters.
///
/// Fetches version, resource, and storage data from every cluster the
/// calling user is allowed to access, in parallel via `join_all`.
/// Clusters that fail to respond are reported with `reachable: false`
/// and an error message rather than failing the entire request.
///
/// ## Permission filtering
///
/// - Admin users (or users with no cluster restrictions): see all clusters.
/// - Restricted users: see only their explicitly allowed clusters.
pub async fn dashboard(
    State(state): State<AppState>,
    auth: AuthContext,
) -> Json<DashboardResponse> {
    use futures_util::future::join_all;

    let username = &auth.claims.username;

    // Get all clients the user is allowed to access.
    let allowed_clients = state.clients_for_user(username);

    let futs = allowed_clients.into_iter().map(|c| {
        let name = c.name().to_string();
        async move { (name, collect_cluster_dashboard(c).await) }
    });

    let results = join_all(futs).await;

    let mut clusters = Vec::new();
    let mut totals = DashboardTotals {
        vms_total: 0,
        vms_running: 0,
        vms_stopped: 0,
        lxcs_total: 0,
        lxcs_running: 0,
        lxcs_stopped: 0,
        storage_pools: 0,
        allocated_cpus: 0.0,
        allocated_memory: 0,
    };

    for (name, result) in results {
        match result {
            Ok(cd) => {
                totals.vms_total += cd.vms_total;
                totals.vms_running += cd.vms_running;
                totals.vms_stopped += cd.vms_stopped;
                totals.lxcs_total += cd.lxcs_total;
                totals.lxcs_running += cd.lxcs_running;
                totals.lxcs_stopped += cd.lxcs_stopped;
                totals.storage_pools += cd.storage_pools;
                totals.allocated_cpus += cd.allocated_cpus;
                totals.allocated_memory += cd.allocated_memory;
                clusters.push(cd);
            }
            Err(e) => {
                clusters.push(ClusterDashboard {
                    name,
                    reachable: false,
                    version: None,
                    release: None,
                    vms_total: 0,
                    vms_running: 0,
                    vms_stopped: 0,
                    lxcs_total: 0,
                    lxcs_running: 0,
                    lxcs_stopped: 0,
                    storage_pools: 0,
                    allocated_cpus: 0.0,
                    allocated_memory: 0,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    Json(DashboardResponse { clusters, totals })
}

/// Collect dashboard data for a single Proxmox cluster.
///
/// Fetches version (reachability), resources (VMs/LXCs), and storage
/// pools. If the version call fails, the cluster is reported as
/// unreachable rather than propagating the error. Resource/storage
/// failures after a successful version probe are propagated.
async fn collect_cluster_dashboard(
    client: &crate::proxmox::ProxmoxClient,
) -> AppResult<ClusterDashboard> {
    let name = client.name().to_string();

    // Check reachability first — if the version endpoint fails, report
    // the cluster as unreachable and bail out early with partial data.
    let version: Version = match client.get("version").await {
        Ok(v) => v,
        Err(e) => {
            return Ok(ClusterDashboard {
                name,
                reachable: false,
                version: None,
                release: None,
                vms_total: 0,
                vms_running: 0,
                vms_stopped: 0,
                lxcs_total: 0,
                lxcs_running: 0,
                lxcs_stopped: 0,
                storage_pools: 0,
                allocated_cpus: 0.0,
                allocated_memory: 0,
                error: Some(e.to_string()),
            });
        }
    };

    // Fetch resources and storage in parallel within this cluster.
    let (resources, storages) = futures_util::future::join(
        client.get::<Vec<VmResource>>("cluster/resources?type=vm"),
        client.get::<Vec<StorageResource>>("storage"),
    )
    .await;

    let resources = resources?;
    let storages = storages?;

    let mut vms_total = 0u32;
    let mut vms_running = 0u32;
    let mut vms_stopped = 0u32;
    let mut lxcs_total = 0u32;
    let mut lxcs_running = 0u32;
    let mut lxcs_stopped = 0u32;
    let mut allocated_cpus = 0.0f64;
    let mut allocated_memory = 0u64;

    for r in &resources {
        match r.kind.as_str() {
            "qemu" => {
                vms_total += 1;
                match r.status.as_str() {
                    "running" => vms_running += 1,
                    "stopped" => vms_stopped += 1,
                    _ => {}
                }
            }
            "lxc" => {
                lxcs_total += 1;
                match r.status.as_str() {
                    "running" => lxcs_running += 1,
                    "stopped" => lxcs_stopped += 1,
                    _ => {}
                }
            }
            _ => continue,
        }
        if let Some(cpus) = r.cpus {
            allocated_cpus += cpus;
        }
        if let Some(maxmem) = r.maxmem {
            allocated_memory += maxmem;
        }
    }

    Ok(ClusterDashboard {
        name,
        reachable: true,
        version: Some(version.version),
        release: Some(version.release),
        vms_total,
        vms_running,
        vms_stopped,
        lxcs_total,
        lxcs_running,
        lxcs_stopped,
        storage_pools: storages.len() as u32,
        allocated_cpus,
        allocated_memory,
        error: None,
    })
}
