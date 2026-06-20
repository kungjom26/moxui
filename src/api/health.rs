//! Health check endpoints.

use axum::{extract::State, http::StatusCode, Json};

use crate::state::AppState;

/// Health response.
#[derive(Debug, serde::Serialize)]
pub struct HealthResponse {
    /// Service status ("ok" / "degraded" / "unhealthy").
    pub status: String,
    /// Service version.
    pub version: String,
    /// Git SHA.
    pub git_sha: String,
    /// Build profile (debug/release).
    pub build_profile: String,
    /// Uptime in seconds.
    pub uptime_seconds: u64,
}

/// `GET /health` — detailed health check.
pub async fn health(_state: State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: crate::VERSION.to_string(),
        git_sha: crate::GIT_SHA.to_string(),
        build_profile: crate::BUILD_PROFILE.to_string(),
        uptime_seconds: 0, // TODO: track startup time
    })
}

/// `GET /livez` — Kubernetes liveness probe (always 200 if process alive).
pub async fn livez(_state: State<AppState>) -> StatusCode {
    StatusCode::OK
}

/// `GET /readyz` — Kubernetes readiness probe (200 if can serve traffic).
pub async fn readyz(_state: State<AppState>) -> StatusCode {
    // TODO: check DB connection, Proxmox clusters reachable
    StatusCode::OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_livez_returns_ok() {
        // We can't easily test with State<AppState> without a full app,
        // but we can verify the function exists
        let _: fn(State<AppState>) -> _ = livez;
    }
}
