//! HTTP API layer (axum handlers).

pub mod health;
pub mod vms;

use axum::{middleware::from_fn_with_state, routing::get, Router};

use crate::state::AppState;

/// Build the main API router with the audit middleware applied globally.
///
/// Layer order:
/// 1. `audit_middleware` — logs state-changing requests and stamps
///    `X-Request-Id` on every response.
/// 2. Route handlers (`/health`, `/livez`, `/readyz`, `/api/v1/vms`).
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/livez", get(health::livez))
        .route("/readyz", get(health::readyz))
        .route("/api/v1/vms", get(vms::list_vms))
        .layer(from_fn_with_state(
            state.clone(),
            crate::audit::audit_middleware,
        ))
        .with_state(state)
}
