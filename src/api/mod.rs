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
        .layer(axum::middleware::from_fn(security_headers_middleware))
        .with_state(state)
}

/// Middleware that adds production security headers to every response.
///
/// Headers (only set when not already present, so handlers can override):
/// - `Strict-Transport-Security: max-age=31536000; includeSubDomains`
/// - `X-Content-Type-Options: nosniff`
/// - `X-Frame-Options: DENY`
/// - `Referrer-Policy: no-referrer`
/// - `Content-Security-Policy: default-src 'self'`
async fn security_headers_middleware(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::http::{header, HeaderValue};
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    if !headers.contains_key(header::STRICT_TRANSPORT_SECURITY) {
        headers.insert(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        );
    }
    if !headers.contains_key("x-content-type-options") {
        headers.insert(
            "x-content-type-options",
            HeaderValue::from_static("nosniff"),
        );
    }
    if !headers.contains_key("x-frame-options") {
        headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    }
    if !headers.contains_key(header::REFERRER_POLICY) {
        headers.insert(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        );
    }
    if !headers.contains_key(header::CONTENT_SECURITY_POLICY) {
        headers.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("default-src 'self'"),
        );
    }
    response
}
