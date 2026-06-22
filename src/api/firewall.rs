//! Cluster firewall API handlers.
//!
//! - `GET /api/v1/firewall/rules` — list cluster-level firewall rules

use std::collections::HashMap;

use axum::{extract::State, Json};

use crate::state::AppState;

/// `GET /api/v1/firewall/rules` — list cluster-wide firewall rules.
///
/// Proxmox endpoint: `GET /api2/json/cluster/firewall/rules`
pub async fn firewall_rules(State(state): State<AppState>) -> Json<serde_json::Value> {
    use futures_util::future::join_all;

    let futs = state.clients().map(|c| {
        let name = c.name().to_string();
        async move {
            let result: Result<serde_json::Value, _> = c.get("cluster/firewall/rules").await;
            (name, result)
        }
    });

    let results = join_all(futs).await;

    let mut data = HashMap::new();
    let mut errors = HashMap::new();
    for (name, result) in results {
        match result {
            Ok(rules) => {
                data.insert(name, rules);
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
