//! HTTP API layer (axum handlers).

pub mod health;

use axum::{routing::get, Router};

use crate::state::AppState;

/// Build the main API router.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/livez", get(health::livez))
        .route("/readyz", get(health::readyz))
        .with_state(state)
}
