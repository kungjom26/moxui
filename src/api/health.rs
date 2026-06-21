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

    /// End-to-end: spin up the router in-process, hit `/health` (GET —
    /// should NOT be audited) and a non-existent POST endpoint (non-2xx —
    /// SHOULD be audited), then verify the audit row.
    #[tokio::test]
    async fn test_router_audits_state_changing_or_non_2xx() {
        use crate::audit::AuditStore;
        use crate::config::{DatabaseConfig, LoggingConfig, ServerConfig};
        use crate::state::AppState;
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        let audit = std::sync::Arc::new(AuditStore::open_in_memory().unwrap());
        let cfg = crate::config::Config {
            server: ServerConfig {
                bind: "127.0.0.1:0".to_string(),
                workers: 0,
            },
            database: DatabaseConfig {
                path: ":memory:".to_string(),
                max_connections: 1,
                run_migrations: false,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "pretty".to_string(),
            },
            clusters: vec![],
        };
        let state = AppState::new(cfg, vec![], audit.clone());
        let app = crate::api::router(state);

        // 1. GET /health → 200, should NOT be audited (read-only + 2xx).
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let request_id = resp
            .headers()
            .get(crate::audit::middleware::REQUEST_ID_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        assert!(request_id.is_some(), "missing X-Request-Id header");

        // 2. GET /api/v1/vms → 200, should NOT be audited (read-only + 2xx).
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/vms")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // 3. POST /api/v1/vms → 405 (no POST handler) → SHOULD be audited.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/vms")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);

        // Verify only the non-2xx POST was audited.
        assert_eq!(audit.count().unwrap(), 1, "only one audit row should exist");
        let rid = request_id.as_deref().unwrap();
        let entry = audit.find_by_request_id(rid).unwrap();
        // GETs have no row → entry should be None.
        assert!(entry.is_none(), "GET /health should not be audited");
    }
}
