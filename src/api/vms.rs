//! VM list + detail + write endpoint handlers.
//!
//! Aggregation logic for `/api/v1/vms` lives here. Write endpoints
//! (`/start`, `/stop`, `/shutdown`, `/reboot`) translate user-facing actions
//! into Proxmox API calls and report the resulting UPID so callers can
//! poll `/tasks/{upid}/status` if they need completion confirmation.

use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;

use crate::error::{AppError, AppResult};
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

/// `GET /api/v1/vms/:cluster/:vmid` — VM detail (single VM).
///
/// Filters `/cluster/resources?type=vm` for the requested VMID. Returns
/// 404 if not found or the cluster is not configured.
pub async fn vm_detail(
    State(state): State<AppState>,
    Path((cluster, vmid)): Path<(String, u32)>,
) -> AppResult<Json<VmRow>> {
    let client = state
        .client(&cluster)
        .ok_or_else(|| AppError::NotFound(format!("cluster '{cluster}' not configured")))?;

    let vms = client.list_vms().await?;
    let row = vms
        .into_iter()
        .find(|r| r.vmid == vmid)
        .map(|r| VmRow::from((cluster.clone(), r)))
        .ok_or_else(|| AppError::NotFound(format!("VM {vmid} not found in {cluster}")))?;

    Ok(Json(row))
}

/// Response from a state-changing VM action (start/stop/etc.).
#[derive(Debug, Clone, Serialize)]
pub struct VmActionResponse {
    /// VMID the action was applied to.
    pub vmid: u32,
    /// Action that was performed (e.g. `start`).
    pub action: String,
    /// Proxmox UPID (e.g. `UPID:pve11:00001234:...`) — caller can poll
    /// `/nodes/{node}/tasks/{upid}/status` for completion.
    pub upid: String,
}

/// `POST /api/v1/vms/:cluster/:node/:vmid/:action`
/// VM action dispatcher (start | stop | shutdown | reboot).
///
/// Returns the Proxmox UPID so the caller can poll for completion.
/// Allowed actions are whitelisted — anything else is `400 Bad Request`.
pub async fn vm_action_handler(
    State(state): State<AppState>,
    Path((cluster, node, vmid, action)): Path<(String, String, u32, String)>,
) -> AppResult<Json<VmActionResponse>> {
    match action.as_str() {
        "start" | "stop" | "shutdown" | "reboot" => {
            vm_action(&state, &cluster, &node, vmid, &action).await
        }
        other => Err(AppError::BadRequest(format!(
            "unknown action '{other}'; expected start|stop|shutdown|reboot"
        ))),
    }
}

/// Shared logic for all VM write actions.
///
/// Proxmox endpoint shape (per skill reference):
/// ```text
/// POST /api2/json/nodes/{node}/qemu/{vmid}/status/{action}
/// Cookie: PVEAuthCookie=...
/// CSRFPreventionToken: ...
///
/// Response: {"data": "UPID:pve11:00001234:..."}
/// ```
async fn vm_action(
    state: &AppState,
    cluster: &str,
    node: &str,
    vmid: u32,
    action: &str,
) -> AppResult<Json<VmActionResponse>> {
    let client = state
        .client(cluster)
        .ok_or_else(|| AppError::NotFound(format!("cluster '{cluster}' not configured")))?;

    let path = format!("nodes/{node}/qemu/{vmid}/status/{action}");
    let upid: String = client.post(&path).await?;

    Ok(Json(VmActionResponse {
        vmid,
        action: action.to_string(),
        upid,
    }))
}
