//! LXC container list + detail endpoint handlers.
//!
//! Read-only views over `/cluster/resources?type=vm` filtered to `type=lxc`.
//! All handlers are gated behind `require_auth` (Viewer+) at the router
//! level — no write actions are exposed in Phase 0.

use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::proxmox::types::{LxcStatus, VmResource};
use crate::state::AppState;

/// One LXC row in the aggregate response, tagged with the cluster it came from.
#[derive(Debug, Clone, Serialize)]
pub struct LxcRow {
    /// Cluster the container lives in.
    pub cluster: String,
    /// Proxmox VMID.
    pub vmid: u32,
    /// Container name.
    pub name: String,
    /// Node currently hosting the container.
    pub node: String,
    /// Status string from Proxmox (`running`, `stopped`).
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
    /// Template flag (`1` if this is a template, `0` otherwise).
    pub template: Option<u8>,
}

impl From<(String, VmResource)> for LxcRow {
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
            template: r.template,
        }
    }
}

/// `GET /api/v1/lxcs` response payload.
#[derive(Debug, Clone, Serialize)]
pub struct LxcsResponse {
    /// Aggregated LXC list, one row per (cluster, vmid).
    pub lxcs: Vec<LxcRow>,
    /// Per-cluster list errors (`cluster_name -> error_message`).
    /// Empty when every cluster returned successfully.
    pub errors: HashMap<String, String>,
}

/// `GET /api/v1/lxcs` — list LXC containers across every configured cluster.
pub async fn list_lxcs(State(state): State<AppState>) -> Json<LxcsResponse> {
    use futures_util::future::join_all;

    let futs = state.clients().map(|c| {
        let name = c.name().to_string();
        async move {
            let result = c.list_lxcs().await;
            (name, result)
        }
    });

    let results = join_all(futs).await;

    let mut lxcs = Vec::new();
    let mut errors = HashMap::new();
    for (name, result) in results {
        match result {
            Ok(list) => {
                lxcs.extend(list.into_iter().map(|r| LxcRow::from((name.clone(), r))));
            }
            Err(e) => {
                errors.insert(name, e.to_string());
            }
        }
    }

    Json(LxcsResponse { lxcs, errors })
}

/// `GET /api/v1/lxcs/:cluster/:node/:vmid` — single LXC status.
///
/// Returns 404 if the cluster is not configured, the node is not a known
/// Proxmox node, or the vmid does not match a live container.
pub async fn lxc_detail(
    State(state): State<AppState>,
    Path((cluster, node, vmid)): Path<(String, String, u32)>,
) -> AppResult<Json<LxcStatus>> {
    let client = state
        .client(&cluster)
        .ok_or_else(|| AppError::NotFound(format!("cluster '{cluster}' not configured")))?;

    let status = client
        .lxc_detail(&node, vmid)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("LXC {vmid} not found in {cluster}/{node}")))?;

    Ok(Json(status))
}
