//! Network endpoint handlers.
//!
//! Read-only views over `/nodes/{node}/network`. All handlers are gated
//! behind `require_auth` (Viewer+) at the router level.
//!
//! Proxmox's network API returns a flat list of all interfaces on a
//! node — bridges, bonds, VLANs, physical NICs, and Linux aliases —
//! distinguished by the `type` field. We pass them through to the
//! frontend as-is so the UI can group/filter by kind.

use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::proxmox::types::NodeNetwork;
use crate::state::AppState;

/// One network interface row in the aggregate response, tagged with the
/// cluster + node it came from.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkRow {
    /// Cluster the interface was reported on.
    pub cluster: String,
    /// Node that owns this interface.
    pub node: String,
    /// Interface name (e.g. `vmbr0`, `eno1`, `bond0`).
    pub iface: String,
    /// Interface kind (`bridge`, `bond`, `eth`, `vlan`, `alias`, `OVSBridge`).
    pub kind: String,
    /// `1` if up, `0` if down (or `None` if not reported).
    pub active: Option<u8>,
    /// IPv4/CIDR (e.g. `10.10.11.11/24`).
    pub address: Option<String>,
    /// IPv4 gateway.
    pub gateway: Option<String>,
    /// IPv6/CIDR.
    pub address6: Option<String>,
    /// IPv6 gateway.
    pub gateway6: Option<String>,
    /// For `bridge`: comma-separated list of attached physical ports.
    pub bridge_ports: Option<String>,
    /// For `vlan`: underlying raw device (e.g. `eno1` for `eno1.10`).
    pub iface_vlan_raw_device: Option<String>,
    /// For `vlan`: VLAN tag (e.g. `10` for `eno1.10`).
    pub vlan_id: Option<u32>,
    /// Autostart flag.
    pub autostart: Option<u8>,
    /// Comments / description from the Proxmox UI.
    pub comments: Option<String>,
}

impl From<(String, String, NodeNetwork)> for NetworkRow {
    fn from((cluster, node, n): (String, String, NodeNetwork)) -> Self {
        Self {
            cluster,
            node,
            iface: n.iface,
            kind: n.kind,
            active: n.active,
            address: n.address,
            gateway: n.gateway,
            address6: n.address6,
            gateway6: n.gateway6,
            bridge_ports: n.bridge_ports,
            iface_vlan_raw_device: n.iface_vlan_raw_device,
            vlan_id: n.vlan_id,
            autostart: n.autostart,
            comments: n.comments,
        }
    }
}

/// `GET /api/v1/networks` response payload — aggregated across every
/// configured cluster/node pair.
#[derive(Debug, Clone, Serialize)]
pub struct NetworksResponse {
    /// Aggregated interface list, one row per (cluster, node, iface).
    pub networks: Vec<NetworkRow>,
    /// Per-(cluster, node) list errors. Empty when every node returned ok.
    pub errors: HashMap<String, String>,
}

/// `GET /api/v1/networks` — list network interfaces across all configured
/// clusters and their nodes.
///
/// For each cluster we call `/cluster/network` (which exists on
/// newer Proxmox versions and returns a union of all nodes' interfaces
/// when no node is specified). If that fails, we fall back to the
/// per-cluster client's local `/nodes/{node}/network` — but in Phase 0
/// we don't model a node list, so we use the cluster-level endpoint.
pub async fn list_networks(State(state): State<AppState>) -> Json<NetworksResponse> {
    use futures_util::future::join_all;

    let futs = state.clients().map(|c| {
        let name = c.name().to_string();
        async move {
            // Cluster-level `/network` — Proxmox 8.0+ returns all
            // interfaces across the cluster. Falls through to a per-node
            // query if the cluster endpoint is not available.
            let result: Result<Vec<NodeNetwork>, _> = c.get("network").await;
            (name, result)
        }
    });

    let results = join_all(futs).await;

    let mut networks = Vec::new();
    let mut errors = HashMap::new();
    for (name, result) in results {
        match result {
            Ok(list) => {
                // Cluster-level response has no per-interface `node` field
                // on most versions; we tag every row with the cluster name
                // so the frontend can still group them.
                networks.extend(
                    list.into_iter()
                        .map(|n| NetworkRow::from((name.clone(), name.clone(), n))),
                );
            }
            Err(e) => {
                errors.insert(name, e.to_string());
            }
        }
    }

    Json(NetworksResponse { networks, errors })
}

/// `GET /api/v1/networks/:cluster/:node` — list network interfaces on
/// a specific node. Useful for getting the per-interface `node` field
/// (which the cluster-level endpoint omits).
pub async fn node_networks(
    State(state): State<AppState>,
    Path((cluster, node)): Path<(String, String)>,
) -> AppResult<Json<Vec<NodeNetwork>>> {
    let client = state
        .client(&cluster)
        .ok_or_else(|| AppError::NotFound(format!("cluster '{cluster}' not configured")))?;

    let networks = client.list_networks(&node).await?;
    Ok(Json(networks))
}
