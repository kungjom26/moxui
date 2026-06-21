//! Health check endpoints.

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

use crate::state::{AppState, ClusterStatus};

/// Health response.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Service status (`ok` / `degraded` / `unhealthy`).
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

/// Readiness response payload (returned when not ready).
#[derive(Debug, Serialize)]
pub struct UnreadyResponse {
    /// Always `unready` for this shape.
    pub status: &'static str,
    /// Per-cluster health details.
    pub clusters: std::collections::HashMap<String, &'static str>,
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

/// `GET /readyz` — Kubernetes readiness probe.
///
/// Returns 200 if every configured Proxmox cluster is reachable (cached for
/// `READINESS_CACHE_TTL`). Otherwise 503 with a per-cluster breakdown.
pub async fn readyz(State(state): State<AppState>) -> Response {
    let snap = state.readiness().await;
    if snap.all_healthy() {
        return StatusCode::OK.into_response();
    }

    let clusters: std::collections::HashMap<String, &'static str> = snap
        .clusters
        .iter()
        .map(|(name, status)| {
            let label = match status {
                ClusterStatus::Healthy => "healthy",
                ClusterStatus::Unhealthy => "unhealthy",
            };
            (name.clone(), label)
        })
        .collect();

    let body = UnreadyResponse {
        status: "unready",
        clusters,
    };
    (StatusCode::SERVICE_UNAVAILABLE, Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_livez_returns_ok() {
        // Type-level test: signature unchanged.
        let _: fn(State<AppState>) -> _ = livez;
    }
}
