//! VM list endpoint.
//!
//! Aggregates VMs from every configured Proxmox cluster. Errors from one
//! cluster do not fail the whole request — that cluster's VMs are simply
//! omitted and the error is reported in the `errors` map.

use std::collections::HashMap;

use axum::{extract::State, Json};
use serde::Serialize;

use crate::proxmox::types::VmResource;
use crate::state::AppState;

/// One VM row in the aggregate response, tagged with the cluster it came from.
#[derive(Debug, Clone, Serialize)]
pub struct VmRow {
    /// Cluster the VM lives in.
    pub cluster: String,
    /// Proxmox VMID (unique within a cluster).
    pub vmid: u32,
    /// VM/LXC name.
    pub name: String,
    /// Node currently hosting the VM.
    pub node: String,
    /// Status string from Proxmox (`running`, `stopped`, `paused`).
    pub status: String,
    /// CPU usage fraction `[0.0, 1.0]`, if reported.
    pub cpu: Option<f64>,
    /// Allocated CPU cores, if reported.
    pub cpus: Option<f64>,
    /// Used memory in bytes, if reported.
    pub mem: Option<u64>,
    /// Configured memory in bytes, if reported.
    pub maxmem: Option<u64>,
    /// Uptime in seconds, if reported.
    pub uptime: Option<u64>,
    /// Semicolon-separated tags, if any.
    pub tags: Option<String>,
}

impl From<(String, VmResource)> for VmRow {
    fn from((cluster, r): (String, VmResource)) -> Self {
        Self {
            cluster,
            vmid: r.vmid,
            name: r.name,
            node: r.node,
            status: r.status,
            cpu: r.cpu,
            cpus: r.cpus,
            mem: r.mem,
            maxmem: r.maxmem,
            uptime: r.uptime,
            tags: r.tags,
        }
    }
}

/// `GET /api/v1/vms` response payload.
#[derive(Debug, Clone, Serialize)]
pub struct VmsResponse {
    /// Aggregated VM list, one row per (cluster, vmid).
    pub vms: Vec<VmRow>,
    /// Per-cluster list errors (`cluster_name -> error_message`).
    /// Empty when every cluster returned successfully.
    pub errors: HashMap<String, String>,
}

/// `GET /api/v1/vms` — list VMs across every configured Proxmox cluster.
///
/// Concurrency: clusters are queried in parallel with `join_all`. A single
/// cluster failure does not fail the whole request.
pub async fn list_vms(State(state): State<AppState>) -> Json<VmsResponse> {
    use futures_util::future::join_all;

    let futs = state.clients().map(|c| {
        let name = c.name().to_string();
        async move {
            let result = c.list_vms().await;
            (name, result)
        }
    });

    let results = join_all(futs).await;

    let mut vms = Vec::new();
    let mut errors = HashMap::new();
    for (name, result) in results {
        match result {
            Ok(list) => {
                vms.extend(list.into_iter().map(|r| VmRow::from((name.clone(), r))));
            }
            Err(e) => {
                errors.insert(name, e.to_string());
            }
        }
    }

    Json(VmsResponse { vms, errors })
}
