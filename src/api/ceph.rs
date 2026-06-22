//! Ceph dashboard API handlers.
//!
//! Proxies to Proxmox Ceph API endpoints:
//! - `GET /api/v1/ceph/status` — cluster health / status
//! - `GET /api/v1/ceph/pools`  — list Ceph pools

use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// `GET /api/v1/ceph/status` — fetch Ceph cluster status from each cluster.
///
/// Proxmox endpoint: `GET /api2/json/cluster/ceph/status`
pub async fn ceph_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    use futures_util::future::join_all;

    let futs = state.clients().map(|c| {
        let name = c.name().to_string();
        async move {
            let result: Result<serde_json::Value, _> = c.get("cluster/ceph/status").await;
            (name, result)
        }
    });

    let results = join_all(futs).await;

    let mut data = HashMap::new();
    let mut errors = HashMap::new();
    for (name, result) in results {
        match result {
            Ok(status) => {
                data.insert(name, status);
            }
            Err(e) => {
                errors.insert(name, e.to_string());
            }
        }
    }

    Json(serde_json::json!({
        "data": data,
        "errors": errors,
    }))
}

/// `GET /api/v1/ceph/pools` — list Ceph pools from each cluster.
///
/// Proxmox endpoint: `GET /api2/json/cluster/ceph/pool`
pub async fn ceph_pools(State(state): State<AppState>) -> Json<serde_json::Value> {
    use futures_util::future::join_all;

    let futs = state.clients().map(|c| {
        let name = c.name().to_string();
        async move {
            let result: Result<serde_json::Value, _> = c.get("cluster/ceph/pool").await;
            (name, result)
        }
    });

    let results = join_all(futs).await;

    let mut data = HashMap::new();
    let mut errors = HashMap::new();
    for (name, result) in results {
        match result {
            Ok(pools) => {
                data.insert(name, pools);
            }
            Err(e) => {
                errors.insert(name, e.to_string());
            }
        }
    }

    Json(serde_json::json!({
        "data": data,
        "errors": errors,
    }))
}
