//! HA group management API handlers.
//!
//! Provides CRUD for Proxmox HA groups across all configured clusters.
//! Each cluster manages its own HA groups independently.
//!
//! Routes:
//! - `GET    /api/v1/hagroups`                 — list HA groups across all clusters
//! - `POST   /api/v1/hagroups/:cluster/:group` — create/update HA group
//! - `DELETE /api/v1/hagroups/:cluster/:group` — delete HA group

use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::auth::{require_role, AuthContext, Role};
use crate::error::{AppError, AppResult};
use crate::proxmox::types::HaGroup;
use crate::state::AppState;

/// One HA group row, tagged with the cluster it came from.
#[derive(Debug, Clone, Serialize)]
pub struct HaGroupRow {
    /// Cluster the HA group belongs to.
    pub cluster: String,
    /// HA group name.
    pub group: String,
    /// Comma-separated list of allowed nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nodes: Option<String>,
    /// Group type.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Free-form comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Whether auto-failback is disabled (`1` = disabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nofailback: Option<u8>,
    /// Whether the group is restricted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restricted: Option<u8>,
}

impl From<(String, HaGroup)> for HaGroupRow {
    fn from((cluster, g): (String, HaGroup)) -> Self {
        Self {
            cluster,
            group: g.group,
            nodes: g.nodes,
            kind: g.kind,
            comment: g.comment,
            nofailback: g.nofailback,
            restricted: g.restricted,
        }
    }
}

/// `GET /api/v1/hagroups` response.
#[derive(Debug, Clone, Serialize)]
pub struct HaGroupsResponse {
    /// Combined list from all clusters.
    pub groups: Vec<HaGroupRow>,
    /// Per-cluster errors.
    pub errors: HashMap<String, String>,
}

/// `POST /api/v1/hagroups/:cluster/:group` request body.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct HaGroupCreateRequest {
    /// Comma-separated list of allowed nodes.
    #[serde(default)]
    pub nodes: Option<String>,
    /// Free-form comment.
    #[serde(default)]
    pub comment: Option<String>,
    /// Disable auto-failback.
    #[serde(default)]
    pub nofailback: bool,
    /// Restrict group.
    #[serde(default)]
    pub restricted: bool,
}

/// Response from a create/update/delete HA group operation.
#[derive(Debug, Clone, Serialize)]
pub struct HaGroupActionResponse {
    /// Cluster the operation was applied to.
    pub cluster: String,
    /// HA group name.
    pub group: String,
    /// Action performed.
    pub action: String,
    /// Proxmox UPID or result string.
    pub result: String,
}

/// `GET /api/v1/hagroups` — list HA groups across all configured clusters.
pub async fn list_ha_groups(
    State(state): State<AppState>,
) -> Json<HaGroupsResponse> {
    use futures_util::future::join_all;

    let futs = state.clients().map(|c| {
        let name = c.name().to_string();
        async move {
            let result = c.list_ha_groups().await;
            (name, result)
        }
    });

    let results = join_all(futs).await;

    let mut groups = Vec::new();
    let mut errors = HashMap::new();
    for (name, result) in results {
        match result {
            Ok(list) => {
                groups.extend(list.into_iter().map(|g| HaGroupRow::from((name.clone(), g))));
            }
            Err(e) => {
                errors.insert(name, e.to_string());
            }
        }
    }

    Json(HaGroupsResponse { groups, errors })
}

/// `POST /api/v1/hagroups/:cluster/:group` — create or update an HA group.
///
/// Requires `Operator` role or higher.
pub async fn create_ha_group(
    State(state): State<AppState>,
    auth: AuthContext,
    Path((cluster, group)): Path<(String, String)>,
    Json(body): Json<HaGroupCreateRequest>,
) -> AppResult<Json<HaGroupActionResponse>> {
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

    let result = client
        .create_ha_group(
            &group,
            body.nodes.as_deref(),
            body.comment.as_deref(),
            body.nofailback,
            body.restricted,
        )
        .await?;

    Ok(Json(HaGroupActionResponse {
        cluster,
        group,
        action: "create".to_string(),
        result,
    }))
}

/// `DELETE /api/v1/hagroups/:cluster/:group` — delete an HA group.
///
/// Requires `Operator` role or higher.
pub async fn delete_ha_group(
    State(state): State<AppState>,
    auth: AuthContext,
    Path((cluster, group)): Path<(String, String)>,
) -> AppResult<Json<HaGroupActionResponse>> {
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

    let result = client.delete_ha_group(&group).await?;

    Ok(Json(HaGroupActionResponse {
        cluster,
        group,
        action: "delete".to_string(),
        result,
    }))
}
