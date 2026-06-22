//! Replication job management API handlers.
//!
//! Provides CRUD for Proxmox replication jobs across all configured clusters.
//! Each cluster manages its own replication jobs independently.
//!
//! Routes:
//! - `GET    /api/v1/replication`                — list replication jobs across all clusters
//! - `POST   /api/v1/replication/:cluster/:vmid` — create replication job for a VM
//! - `DELETE /api/v1/replication/:cluster/:id`   — delete a replication job
//! - `GET    /api/v1/replication/:cluster/:id/status` — get replication job status

use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::auth::{require_role, AuthContext, Role};
use crate::error::{AppError, AppResult};
use crate::proxmox::types::{CreateReplicationJob, ReplicationJob, ReplicationStatus};
use crate::state::AppState;

/// One replication job row, tagged with the cluster it came from.
#[derive(Debug, Clone, Serialize)]
pub struct ReplicationJobRow {
    /// Cluster the replication job belongs to.
    pub cluster: String,
    /// Replication job ID.
    pub id: u64,
    /// Whether the job is enabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable: Option<u8>,
    /// Source node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_node: Option<String>,
    /// Source VM ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_vmid: Option<u32>,
    /// Target node/remote.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Target VM ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_vmid: Option<u32>,
    /// Replication rate limit in MB/s.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate: Option<u32>,
    /// Replication schedule in cron format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    /// Free-form comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Job type.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

impl From<(String, ReplicationJob)> for ReplicationJobRow {
    fn from((cluster, job): (String, ReplicationJob)) -> Self {
        Self {
            cluster,
            id: job.id,
            enable: job.enable,
            source_node: job.source_node,
            source_vmid: job.source_vmid,
            target: job.target,
            target_vmid: job.target_vmid,
            rate: job.rate,
            schedule: job.schedule,
            comment: job.comment,
            kind: job.kind,
        }
    }
}

/// `POST /api/v1/replication/:cluster/:vmid` request body.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateReplicationRequest {
    /// Source node.
    pub source_node: String,
    /// Target node/remote identifier.
    pub target: String,
    /// Target VM ID (defaults to source vmid).
    #[serde(default)]
    pub target_vmid: Option<u32>,
    /// Replication rate limit in MB/s.
    #[serde(default)]
    pub rate: Option<u32>,
    /// Schedule in cron format.
    pub schedule: String,
    /// Free-form comment.
    #[serde(default)]
    pub comment: Option<String>,
}

/// `GET /api/v1/replication` response.
#[derive(Debug, Clone, Serialize)]
pub struct ReplicationListResponse {
    /// Combined list from all clusters.
    pub replication_jobs: Vec<ReplicationJobRow>,
    /// Per-cluster errors.
    pub errors: HashMap<String, String>,
}

/// Replication action response.
#[derive(Debug, Clone, Serialize)]
pub struct ReplicationActionResponse {
    /// Cluster the operation was applied to.
    pub cluster: String,
    /// Replication job ID or VMID.
    pub id: String,
    /// Action performed.
    pub action: String,
    /// Proxmox result string.
    pub result: String,
}

/// Replication status response.
#[derive(Debug, Clone, Serialize)]
pub struct ReplicationStatusResponse {
    /// Cluster the job belongs to.
    pub cluster: String,
    /// Replication job ID.
    pub id: u64,
    /// Status entries.
    pub status: Vec<ReplicationStatus>,
}

/// `GET /api/v1/replication` — list replication jobs across all configured clusters.
pub async fn list_replication_jobs(
    State(state): State<AppState>,
) -> Json<ReplicationListResponse> {
    use futures_util::future::join_all;

    let futs = state.clients().map(|c| {
        let name = c.name().to_string();
        async move {
            let result = c.list_replication_jobs().await;
            (name, result)
        }
    });

    let results = join_all(futs).await;

    let mut replication_jobs = Vec::new();
    let mut errors = HashMap::new();
    for (name, result) in results {
        match result {
            Ok(list) => {
                replication_jobs
                    .extend(list.into_iter().map(|j| ReplicationJobRow::from((name.clone(), j))));
            }
            Err(e) => {
                errors.insert(name, e.to_string());
            }
        }
    }

    Json(ReplicationListResponse {
        replication_jobs,
        errors,
    })
}

/// `POST /api/v1/replication/:cluster/:vmid` — create a replication job for a VM.
///
/// Requires `Operator` role or higher.
pub async fn create_replication_job(
    State(state): State<AppState>,
    auth: AuthContext,
    Path((cluster, vmid)): Path<(String, u32)>,
    Json(body): Json<CreateReplicationRequest>,
) -> AppResult<Json<ReplicationActionResponse>> {
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

    let job = CreateReplicationJob {
        source_vmid: vmid,
        source_node: Some(body.source_node),
        target: body.target,
        target_vmid: body.target_vmid,
        rate: body.rate,
        schedule: body.schedule,
        comment: body.comment,
        enable: true,
    };

    let result = client.create_replication_job(&job).await?;

    Ok(Json(ReplicationActionResponse {
        cluster,
        id: vmid.to_string(),
        action: "create".to_string(),
        result,
    }))
}

/// `DELETE /api/v1/replication/:cluster/:id` — delete a replication job.
///
/// Requires `Operator` role or higher.
pub async fn delete_replication_job(
    State(state): State<AppState>,
    auth: AuthContext,
    Path((cluster, id)): Path<(String, u64)>,
) -> AppResult<Json<ReplicationActionResponse>> {
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

    let result = client.delete_replication_job(id).await?;

    Ok(Json(ReplicationActionResponse {
        cluster,
        id: id.to_string(),
        action: "delete".to_string(),
        result,
    }))
}

/// `GET /api/v1/replication/:cluster/:id/status` — get replication job status.
pub async fn get_replication_status(
    State(state): State<AppState>,
    Path((cluster, id)): Path<(String, u64)>,
) -> AppResult<Json<ReplicationStatusResponse>> {
    let client = state
        .client(&cluster)
        .ok_or_else(|| AppError::NotFound(format!("cluster '{cluster}' not configured")))?;

    let status = client.get_replication_status(id).await?;

    Ok(Json(ReplicationStatusResponse {
        cluster,
        id,
        status,
    }))
}
