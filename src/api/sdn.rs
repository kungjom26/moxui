//! SDN (Software-Defined Networking) API handlers.
//!
//! Routes:
//! - `GET /api/v1/sdn/zones`  — list SDN zones
//! - `GET /api/v1/sdn/vnets`  — list SDN virtual networks

use std::collections::HashMap;

use axum::{extract::State, Json};

use crate::state::AppState;

/// `GET /api/v1/sdn/zones` — list SDN zones across all clusters.
///
/// Proxmox endpoint: `GET /api2/json/cluster/sdn/zones`
pub async fn sdn_zones(State(state): State<AppState>) -> Json<serde_json::Value> {
    use futures_util::future::join_all;

    let futs = state.clients().map(|c| {
        let name = c.name().to_string();
        async move {
            let result: Result<serde_json::Value, _> = c.get("cluster/sdn/zones").await;
            (name, result)
        }
    });

    let results = join_all(futs).await;

    let mut data = HashMap::new();
    let mut errors = HashMap::new();
    for (name, result) in results {
        match result {
            Ok(zones) => {
                data.insert(name, zones);
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

/// `GET /api/v1/sdn/vnets` — list SDN virtual networks across all clusters.
///
/// Proxmox endpoint: `GET /api2/json/cluster/sdn/vnets`
pub async fn sdn_vnets(State(state): State<AppState>) -> Json<serde_json::Value> {
    use futures_util::future::join_all;

    let futs = state.clients().map(|c| {
        let name = c.name().to_string();
        async move {
            let result: Result<serde_json::Value, _> = c.get("cluster/sdn/vnets").await;
            (name, result)
        }
    });

    let results = join_all(futs).await;

    let mut data = HashMap::new();
    let mut errors = HashMap::new();
    for (name, result) in results {
        match result {
            Ok(vnets) => {
                data.insert(name, vnets);
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
