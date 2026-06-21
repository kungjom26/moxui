//! HTTP API layer (axum handlers).

pub mod health;
pub mod vms;

use axum::{routing::get, Router};

use crate::state::AppState;

/// Build the main API router.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/livez", get(health::livez))
        .route("/readyz", get(health::readyz))
        .route("/api/v1/vms", get(vms::list_vms))
        .with_state(state)
}
