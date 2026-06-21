//! HTTP API layer (axum handlers).

pub mod health;
pub mod vms;

use axum::{
    middleware::from_fn_with_state,
    routing::{get, post},
    Router,
};

use crate::state::AppState;

/// Build the main API router with the audit middleware applied globally.
///
/// Routes:
/// - `GET  /health`                          — detailed health JSON
/// - `GET  /livez`                           — k8s liveness
/// - `GET  /readyz`                          — k8s readiness (Proxmox ping)
/// - `GET  /api/v1/vms`                      — list all VMs
/// - `GET  /api/v1/vms/:cluster/:vmid`       — single VM detail
/// - `POST /api/v1/vms/:cluster/:node/:vmid/:action` — VM actions
///   (`action` ∈ `start` | `stop` | `shutdown` | `reboot`)
///
/// Uses `:param` syntax (axum 0.7) — `{param}` style requires axum 0.8.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/livez", get(health::livez))
        .route("/readyz", get(health::readyz))
        .route("/api/v1/vms", get(vms::list_vms))
        .route("/api/v1/vms/:cluster/:vmid", get(vms::vm_detail))
        .route(
            "/api/v1/vms/:cluster/:node/:vmid/:action",
            post(vms::vm_action_handler),
        )
        .layer(from_fn_with_state(
            state.clone(),
            crate::audit::audit_middleware,
        ))
        .with_state(state)
}
