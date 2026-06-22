//! Storage list + content + write endpoint handlers.
//!
//! Read-only views over `/nodes/{node}/storage` and
//! `/nodes/{node}/storage/{storage}/content`. Write endpoints cover
//! upload and delete operations. All handlers are gated behind
//! `require_auth` (Viewer+) at the router level. Write endpoints
//! additionally check for `Operator+` role.

use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;

use crate::auth::{require_role, AuthContext, Role};
use crate::error::{AppError, AppResult};
use crate::proxmox::types::{StorageContent, StorageResource};
use crate::state::AppState;

/// One storage row in the aggregate response, tagged with the cluster+node
/// it came from.
#[derive(Debug, Clone, Serialize)]
pub struct StorageRow {
    /// Cluster the storage belongs to.
    pub cluster: String,
    /// Node that reported this storage.
    pub node: String,
    /// Storage identifier (e.g. `local`, `ceph-pool`).
    pub storage: String,
    /// Storage type (e.g. `dir`, `zfspool`, `rbd`, `nfs`).
    pub kind: String,
    /// Total size in bytes.
    pub total: u64,
    /// Used size in bytes.
    pub used: u64,
    /// Available size in bytes.
    pub avail: u64,
    /// Usage fraction `[0.0, 1.0]`, if reported.
    pub used_fraction: Option<f64>,
    /// Whether the storage is enabled.
    pub enabled: Option<u8>,
    /// Whether the storage is shared across the cluster.
    pub shared: Option<u8>,
    /// Human-readable content types (e.g. `images,rootdir,iso,vztmpl`).
    pub content: Option<String>,
}

impl From<(String, String, StorageResource)> for StorageRow {
    fn from((cluster, node, s): (String, String, StorageResource)) -> Self {
        Self {
            cluster,
            node,
            storage: s.storage,
            kind: s.kind,
            total: s.total,
            used: s.used,
            avail: s.avail,
            used_fraction: s.used_fraction,
            enabled: s.enabled,
            shared: s.shared,
            content: s.content,
        }
    }
}

/// `GET /api/v1/storages` response payload — aggregated across every
/// configured cluster/node pair.
#[derive(Debug, Clone, Serialize)]
pub struct StoragesResponse {
    /// Aggregated storage list, one row per (cluster, node, storage).
    pub storages: Vec<StorageRow>,
    /// Per-(cluster, node) list errors. Empty when every node returned ok.
    pub errors: HashMap<String, String>,
}

/// `GET /api/v1/storages` — list storage pools across all configured clusters
/// and their nodes.
///
/// For each cluster we need to enumerate the cluster's node list (so we know
/// which `/nodes/{node}/storage` to call). In Phase 0 the cluster config does
/// not yet expose a node list, so we only call against the cluster-level
/// `/storage` endpoint via the cluster client (which routes to the local
/// node). This may yield duplicate rows when a cluster has multiple nodes;
/// deduping by `(cluster, storage)` is the caller's responsibility until we
/// model a real node list.
pub async fn list_storages(State(state): State<AppState>) -> Json<StoragesResponse> {
    use futures_util::future::join_all;

    let futs = state.clients().map(|c| {
        let name = c.name().to_string();
        async move {
            // Cluster-level `/storage` aggregates every node's storages.
            let path = "storage";
            let result: Result<Vec<StorageResource>, _> = c.get(path).await;
            (name, result)
        }
    });

    let results = join_all(futs).await;

    let mut storages = Vec::new();
    let mut errors = HashMap::new();
    for (name, result) in results {
        match result {
            Ok(list) => {
                // `node` is optional in the cluster-level payload; default
                // to the cluster name when the upstream omits it.
                storages.extend(
                    list.into_iter()
                        .map(|s| StorageRow::from((name.clone(), name.clone(), s))),
                );
            }
            Err(e) => {
                errors.insert(name, e.to_string());
            }
        }
    }

    Json(StoragesResponse { storages, errors })
}

/// `GET /api/v1/storages/:cluster/:node/:storage/content` — list volumes in
/// a storage pool (ISO images, templates, backups, etc.).
pub async fn storage_content(
    State(state): State<AppState>,
    Path((cluster, node, storage)): Path<(String, String, String)>,
) -> AppResult<Json<Vec<StorageContent>>> {
    let client = state
        .client(&cluster)
        .ok_or_else(|| AppError::NotFound(format!("cluster '{cluster}' not configured")))?;

    let content = client.storage_content(&node, &storage).await?;
    Ok(Json(content))
}

// ── Batch 1: Storage Write Operations ───────────────────────────────────────

/// `POST /api/v1/storages/:cluster/:node/:storage/upload` — upload a file to a storage pool.
///
/// Accepts a JSON body with `content` (content type) and `data` (base64-encoded file bytes)
/// and `filename` (the original filename). The handler decodes the base64 data and
/// forwards it as a multipart upload to Proxmox.
///
/// Requires `Operator` role or higher.
pub async fn storage_upload_handler(
    State(state): State<AppState>,
    auth: AuthContext,
    Path((cluster, node, storage)): Path<(String, String, String)>,
    Json(body): Json<serde_json::Value>,
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

    // Extract fields from JSON body
    let content_type = body
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("missing 'content' field (iso|vztmpl|snippets)".into()))?
        .to_string();

    let filename = body
        .get("filename")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("missing 'filename' field".into()))?
        .to_string();

    let data_b64 = body
        .get("data")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("missing 'data' field (base64-encoded file)".into()))?;

    // Decode base64 data
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD
        .decode(data_b64)
        .map_err(|e| AppError::BadRequest(format!("invalid base64 data: {e}")))?;

    let resp = client
        .upload_file(&node, &storage, &content_type, &filename, data)
        .await?;

    Ok(Json(serde_json::json!({
        "volid": resp.volid,
        "msg": resp.msg,
        "storage": storage,
        "cluster": cluster,
    })))
}

/// `DELETE /api/v1/storages/:cluster/:node/:storage/content/:volid` — delete a volume
/// from a storage pool.
///
/// Requires `Operator` role or higher.
pub async fn delete_storage_content_handler(
    State(state): State<AppState>,
    auth: AuthContext,
    Path((cluster, node, storage, volid)): Path<(String, String, String, String)>,
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

    let upid = client.delete_storage_content(&node, &storage, &volid).await?;

    Ok(Json(serde_json::json!({
        "volid": volid,
        "upid": upid,
        "storage": storage,
        "cluster": cluster,
    })))
}
