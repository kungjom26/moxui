//! LXC container list + detail + write endpoint handlers.
//!
//! Read-only views over `/cluster/resources?type=vm` filtered to `type=lxc`.
//! Write endpoints (start/stop/shutdown/reboot/delete) translate user-facing
//! actions into Proxmox API calls and report the resulting UPID.
//!
//! All handlers are gated behind `require_auth` (Viewer+) at the router
//! level. Write endpoints additionally check for `Operator+` role.

use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;

use crate::auth::{require_role, AuthContext, Role};
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

// ── Batch 1: LXC Write Operations ──────────────────────────────────────────

/// `POST /api/v1/lxcs/:cluster/:node/:vmid/:action` — perform an action on an LXC container.
///
/// Supported actions: `start`, `stop`, `shutdown`, `reboot`.
/// Requires `Operator` role or higher.
pub async fn lxc_action_handler(
    State(state): State<AppState>,
    auth: AuthContext,
    Path((cluster, node, vmid, action)): Path<(String, String, u32, String)>,
    body: Option<Json<crate::proxmox::types::LxcActionRequest>>,
) -> AppResult<Json<serde_json::Value>> {
    if let Err(resp) = require_role(&auth, Role::Operator) {
        let status = resp.status();
        let err = if status == axum::http::StatusCode::FORBIDDEN {
            AppError::Forbidden("operator role required".into())
        } else {
            AppError::Internal(format!("auth middleware returned {status}"))
        };
        return Err(err);
    }

    match action.as_str() {
        "start" | "stop" | "shutdown" | "reboot" => {
            let client = state
                .client(&cluster)
                .ok_or_else(|| AppError::NotFound(format!("cluster '{cluster}' not configured")))?;

            let opts = body.map(|Json(b)| b).unwrap_or_default();
            let mut params: Vec<(String, String)> = Vec::new();
            if let Some(f) = opts.force {
                params.push(("force".to_string(), if f { "1" } else { "0" }.to_string()));
            }
            if let Some(t) = opts.timeout {
                params.push(("timeout".to_string(), t.to_string()));
            }

            let upid = client.lxc_action(&node, vmid, &action, params).await?;

            Ok(Json(serde_json::json!({
                "vmid": vmid,
                "action": action,
                "upid": upid,
                "cluster": cluster,
            })))
        }
        other => Err(AppError::BadRequest(format!(
            "unknown lxc action '{other}'; expected start|stop|shutdown|reboot"
        ))),
    }
}

/// `POST /api/v1/lxcs/:cluster/:node/:vmid/delete` — delete an LXC container.
///
/// Requires `Operator` role or higher.
pub async fn lxc_delete_handler(
    State(state): State<AppState>,
    auth: AuthContext,
    Path((cluster, node, vmid)): Path<(String, String, u32)>,
    body: Option<Json<crate::proxmox::types::DeleteLxcRequest>>,
) -> AppResult<Json<serde_json::Value>> {
    if let Err(resp) = require_role(&auth, Role::Operator) {
        let status = resp.status();
        let err = if status == axum::http::StatusCode::FORBIDDEN {
            AppError::Forbidden("operator role required".into())
        } else {
            AppError::Internal(format!("auth middleware returned {status}"))
        };
        return Err(err);
    }

    let client = state
        .client(&cluster)
        .ok_or_else(|| AppError::NotFound(format!("cluster '{cluster}' not configured")))?;

    let opts = body.map(|Json(b)| b).unwrap_or_default();
    let upid = client
        .lxc_delete(&node, vmid, opts.purge, opts.force, opts.skiplock)
        .await?;

    Ok(Json(serde_json::json!({
        "vmid": vmid,
        "action": "delete",
        "upid": upid,
        "cluster": cluster,
    })))
}
