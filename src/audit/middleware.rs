//! Axum middleware that logs state-changing requests to the audit store.
//!
//! Wraps every request, lets it run to completion, then records the
//! summary. Read-only methods (`GET`/`HEAD`/`OPTIONS`) bypass the store —
//! they would generate high-volume, low-signal noise. Any non-2xx response
//! is always logged regardless of method (security-relevant).
//!
//! All recorded rows share a per-request UUID (`X-Request-Id` header) so
//! downstream logs/responses can be correlated.

use std::sync::Arc;
use std::time::Instant;

use axum::{
    body::Body,
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::audit::store::{AuditEntry, AuditStore};
use crate::state::AppState;

/// Default header name used to read/propagate the request id.
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Axum middleware function — apply with
/// `axum::middleware::from_fn_with_state(state.clone(), audit_middleware)`.
///
/// # Behavior
///
/// 1. Read or generate `X-Request-Id`.
/// 2. Stamp the response with the same `X-Request-Id`.
/// 3. Time the inner handler.
/// 4. Decide whether to persist (`POST|PUT|PATCH|DELETE` OR non-2xx).
/// 5. Build an [`AuditEntry`] and call [`AuditStore::log`].
/// 6. Forward the response unchanged.
///
/// `audit_store` is taken from `AppState.audit` (cloned as `Arc`).
pub async fn audit_middleware(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let audit: Arc<AuditStore> = state.audit.clone();

    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let headers = request.headers().clone();

    let request_id = extract_or_generate_request_id(&headers);
    let remote_addr = extract_remote_addr(&headers);
    let user_agent = extract_header(&headers, axum::http::header::USER_AGENT);

    let start = Instant::now();
    // If `require_auth` middleware ran before us, it stashed the JWT
    // `Claims` in the request extensions. Pull the user id from there
    // so audit rows are attributable. On public routes (no auth
    // middleware) this is `None`.
    let user_id = request
        .extensions()
        .get::<crate::auth::Claims>()
        .map(|c| c.sub.clone());
    let mut response = next.run(request).await;
    // Cap duration_ms at u64::MAX — `as_millis()` returns u128, which may
    // exceed u64 range in extreme cases (≈ 585 million years of uptime).
    // Saturate instead of wrapping.
    let duration_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
    let status = response.status().as_u16();

    // Stamp the response with the same request id for correlation.
    if let Ok(value) = axum::http::HeaderValue::from_str(&request_id) {
        response.headers_mut().insert(REQUEST_ID_HEADER, value);
    }

    // (user_id was extracted from request extensions before next.run)

    let should_log = is_state_changing(&method) || !(200..300).contains(&status);
    if should_log {
        let entry = AuditEntry {
            ts: chrono::Utc::now().timestamp(),
            request_id: request_id.clone(),
            method: method.as_str().to_string(),
            path,
            status,
            duration_ms,
            remote_addr,
            user_agent,
            user_id,
        };
        // Don't fail the request if audit logging fails — log to tracing.
        if let Err(e) = audit.log(&entry) {
            tracing::error!(
                request_id = %request_id,
                error = %e,
                "failed to write audit log entry"
            );
        }
    }

    response
}

/// `true` for methods that change server state.
fn is_state_changing(method: &axum::http::Method) -> bool {
    matches!(
        *method,
        axum::http::Method::POST
            | axum::http::Method::PUT
            | axum::http::Method::PATCH
            | axum::http::Method::DELETE
    )
}

fn extract_or_generate_request_id(headers: &HeaderMap) -> String {
    extract_header(headers, REQUEST_ID_HEADER).unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn extract_header(
    headers: &HeaderMap,
    name: impl axum::http::header::AsHeaderName,
) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

fn extract_remote_addr(headers: &HeaderMap) -> Option<String> {
    // Prefer forwarded-for (behind a trusted proxy); fall back to direct.
    extract_header(headers, "x-forwarded-for").or_else(|| extract_header(headers, "x-real-ip"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_state_changing() {
        assert!(is_state_changing(&axum::http::Method::POST));
        assert!(is_state_changing(&axum::http::Method::PUT));
        assert!(is_state_changing(&axum::http::Method::PATCH));
        assert!(is_state_changing(&axum::http::Method::DELETE));
        assert!(!is_state_changing(&axum::http::Method::GET));
        assert!(!is_state_changing(&axum::http::Method::HEAD));
        assert!(!is_state_changing(&axum::http::Method::OPTIONS));
    }

    #[test]
    fn test_extract_or_generate_request_id_reuses_provided() {
        let mut headers = HeaderMap::new();
        headers.insert(
            REQUEST_ID_HEADER,
            axum::http::HeaderValue::from_static("abc-123"),
        );
        assert_eq!(extract_or_generate_request_id(&headers), "abc-123");
    }

    #[test]
    fn test_extract_or_generate_request_id_creates_when_missing() {
        let headers = HeaderMap::new();
        let id = extract_or_generate_request_id(&headers);
        // UUID v4 = 36 chars, 4 dashes
        assert_eq!(id.len(), 36);
        assert!(id.contains('-'));
    }
}
