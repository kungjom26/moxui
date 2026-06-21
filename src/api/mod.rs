//! HTTP API layer (axum handlers).

pub mod auth;
pub mod health;
pub mod lxcs;
pub mod storages;
pub mod vms;

use axum::{
    middleware::from_fn_with_state,
    routing::{get, post},
    Router,
};

use crate::auth::require_auth;
use crate::state::AppState;

/// Build the main API router with auth + audit middleware applied.
///
/// Routes:
/// - `GET  /health`                          — detailed health JSON
/// - `GET  /livez`                           — k8s liveness
/// - `GET  /readyz`                          — k8s readiness (Proxmox ping)
/// - `POST /api/v1/auth/login`               — username + password → JWT
/// - `GET  /api/v1/auth/me`                  — current user (auth required)
/// - `GET  /api/v1/vms`                      — list all VMs
/// - `GET  /api/v1/vms/:cluster/:vmid`       — single VM detail
/// - `POST /api/v1/vms/:cluster/:node/:vmid/:action` — VM actions
///   (auth required, Operator+ role, `action` ∈ `start`|`stop`|`shutdown`|`reboot`)
///
/// Uses `:param` syntax (axum 0.7) — `{param}` style requires axum 0.8.
pub fn router(state: AppState) -> Router {
    // Public routes — no auth required.
    let public = Router::new()
        .route("/health", get(health::health))
        .route("/livez", get(health::livez))
        .route("/readyz", get(health::readyz))
        .route("/api/v1/auth/login", post(auth::login));

    // Authenticated routes — require a valid Bearer token.
    let protected = Router::new()
        .route("/api/v1/auth/me", get(auth::me))
        .route("/api/v1/vms", get(vms::list_vms))
        .route("/api/v1/vms/:cluster/:vmid", get(vms::vm_detail))
        .route(
            "/api/v1/vms/:cluster/:node/:vmid/:action",
            post(vms::vm_action_handler),
        )
        .route("/api/v1/lxcs", get(lxcs::list_lxcs))
        .route("/api/v1/lxcs/:cluster/:node/:vmid", get(lxcs::lxc_detail))
        .route("/api/v1/storages", get(storages::list_storages))
        .route(
            "/api/v1/storages/:cluster/:node/:storage/content",
            get(storages::storage_content),
        )
        .route_layer(from_fn_with_state(state.clone(), require_auth));

    public
        .merge(protected)
        .layer(from_fn_with_state(
            state.clone(),
            crate::audit::audit_middleware,
        ))
        .with_state(state)
}
