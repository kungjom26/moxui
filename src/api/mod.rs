//! HTTP API layer (axum handlers).

pub mod audit;
pub mod auth;
pub mod ceph;
pub mod dashboard;
pub mod firewall;
pub mod hagroups;
pub mod health;
pub mod lxcs;
pub mod networks;
pub mod replication;
pub mod sdn;
pub mod storages;
pub mod tasks;
pub mod vms;
pub mod vnc;
pub mod webauthn;

use axum::{
    middleware::from_fn_with_state,
    routing::{delete, get, post, put},
    Router,
};

use crate::auth::{require_auth, require_cluster_access};
use crate::security::{cors_layer, RateLimitLayer};
use crate::state::AppState;

/// Build the main API router with auth + audit middleware applied.
///
/// Routes:
/// - `GET  /metrics`                         — Prometheus metrics (before auth)
/// - `GET  /health`                          — detailed health JSON
/// - `GET  /livez`                           — k8s liveness
/// - `GET  /readyz`                          — k8s readiness (Proxmox ping)
/// - `POST /api/v1/auth/login`               — username + password → JWT
/// - `GET  /api/v1/auth/me`                  — current user (auth required)
/// - `GET  /api/v1/vms`                      — list all VMs
/// - `GET  /api/v1/vms/:cluster/:vmid`       — single VM detail
/// - `POST /api/v1/vms/:cluster/:node/:vmid/:action` — VM actions
///   (auth required, Operator+ role, `action` ∈ `start`|`stop`|`shutdown`|`reboot`)
/// - `POST /api/v1/vms/:cluster/:node/:vmid/vnc/ticket` — mint short-lived VNC token
/// - `GET  /api/v1/vms/:cluster/:node/:vmid/vnc/ws`   — WebSocket upgrade
///
/// Uses `:param` syntax (axum 0.7) — `{param}` style requires axum 0.8.
pub fn router(state: AppState) -> Router {
    // Public routes — no auth required.
    let public = Router::new()
        .route("/metrics", get(health::metrics_handler))
        .route("/health", get(health::health))
        .route("/livez", get(health::livez))
        .route("/readyz", get(health::readyz))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/refresh", post(auth::refresh))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/auth/2fa/complete", post(auth::two_factor_complete))
        .route("/api/v1/auth/oidc/login", post(auth::oidc_login))
        .route("/api/v1/auth/oidc/callback", post(auth::oidc_callback))
        .route(
            "/api/v1/auth/webauthn/login/start",
            post(webauthn::login_start),
        )
        .route(
            "/api/v1/auth/webauthn/login/complete",
            post(webauthn::login_complete),
        )
        .route("/api/v1/auth/ldap/login", post(auth::ldap_login));

    // Authenticated routes — require a valid Bearer token.
    // These are routes without `:cluster` path params (no per-cluster check needed).
    let protected = Router::new()
        .route("/api/v1/auth/me", get(auth::me))
        .route("/api/v1/dashboard", get(dashboard::dashboard))
        .route("/api/v1/dashboard/custom", get(dashboard::get_custom_dashboard).post(dashboard::save_custom_dashboard))
        .route("/api/v1/dashboard/custom/widget-types", get(dashboard::get_available_widget_types))
        .route("/api/v1/audit", get(audit::list_audit))
        .route("/api/v1/vms", get(vms::list_vms))
        .route("/api/v1/vms/bulk/start", post(vms::bulk_start))
        .route("/api/v1/vms/bulk/stop", post(vms::bulk_stop))
        .route("/api/v1/vms/bulk/reboot", post(vms::bulk_reboot))
        .route("/api/v1/vms/bulk/delete", post(vms::bulk_delete))
        .route("/api/v1/lxcs", get(lxcs::list_lxcs))
        .route("/api/v1/storages", get(storages::list_storages))
        .route("/api/v1/networks", get(networks::list_networks))
        .route("/api/v1/hagroups", get(hagroups::list_ha_groups))
        .route("/api/v1/hagroups/:cluster/:group", post(hagroups::create_ha_group).delete(hagroups::delete_ha_group))
        .route("/api/v1/replication", get(replication::list_replication_jobs))
        .route("/api/v1/auth/2fa/setup", post(auth::two_factor_setup))
        .route("/api/v1/auth/2fa/verify", post(auth::two_factor_verify))
        .route("/api/v1/auth/2fa/disable", post(auth::two_factor_disable))
        .route(
            "/api/v1/auth/webauthn/register/start",
            post(webauthn::register_start),
        )
        .route(
            "/api/v1/auth/webauthn/register/complete",
            post(webauthn::register_complete),
        )
        // ── Batch 2: Auth + Ceph + Network + VNC Proxy ──
        .route("/api/v1/users", get(auth::list_users).post(auth::create_user))
        .route("/api/v1/users/:username", put(auth::update_user).delete(auth::delete_user))
        .route("/api/v1/ceph/status", get(ceph::ceph_status))
        .route("/api/v1/ceph/pools", get(ceph::ceph_pools))
        .route("/api/v1/firewall/rules", get(firewall::firewall_rules))
        .route("/api/v1/sdn/zones", get(sdn::sdn_zones))
        .route("/api/v1/sdn/vnets", get(sdn::sdn_vnets))
        .route("/api/v1/hagroups/status", get(hagroups::ha_status))
        .route("/api/v1/networks/vlans", get(networks::list_vlans))
        .route_layer(from_fn_with_state(state.clone(), require_auth));

    // Cluster-scoped routes — require auth + cluster-level permissions.
    // These all carry a `:cluster` path parameter. The require_cluster_access
    // middleware checks that the user is allowed to access the named cluster.
    // NOTE: route_layer wraps inside-out, so require_auth (inner) runs first,
    // then require_cluster_access (outer) — exactly what we need.
    let cluster_scoped = Router::new()
        .route("/api/v1/vms/:cluster/:vmid", get(vms::vm_detail))
        .route(
            "/api/v1/vms/:cluster/:node/:vmid/:action",
            post(vms::vm_action_handler),
        )
        .route(
            "/api/v1/vms/:cluster/:node/:vmid/migrate",
            post(vms::migrate_vm_handler),
        )
        // ── Batch 1: VM write routes ──
        .route(
            "/api/v1/vms/:cluster/:node/create",
            post(vms::create_vm_handler),
        )
        .route(
            "/api/v1/vms/:cluster/:node/:vmid/clone",
            post(vms::clone_vm_handler),
        )
        .route(
            "/api/v1/vms/:cluster/:node/:vmid/config",
            get(vms::vm_config_handler).put(vms::update_vm_config_handler),
        )
        .route(
            "/api/v1/vms/:cluster/:node/:vmid/snapshot",
            get(vms::list_snapshots_handler).post(vms::create_snapshot_handler),
        )
        .route(
            "/api/v1/vms/:cluster/:node/:vmid/snapshot/:snapname",
            delete(vms::delete_snapshot_handler),
        )
        .route(
            "/api/v1/vms/:cluster/:node/:vmid/snapshot/:snapname/rollback",
            post(vms::rollback_snapshot_handler),
        )
        .route(
            "/api/v1/vms/:cluster/:node/:vmid/backup",
            post(vms::backup_vm_handler),
        )
        .route(
            "/api/v1/vms/:cluster/:node/:vmid/backups",
            get(vms::list_backups_handler),
        )
        .route(
            "/api/v1/vms/:cluster/:node/:vmid/resize-disk",
            post(vms::resize_disk_handler),
        )
        // ── LXC routes ──
        .route("/api/v1/lxcs/:cluster/:node/:vmid", get(lxcs::lxc_detail))
        .route(
            "/api/v1/lxcs/:cluster/:node/:vmid/:action",
            post(lxcs::lxc_action_handler),
        )
        .route(
            "/api/v1/lxcs/:cluster/:node/:vmid/delete",
            post(lxcs::lxc_delete_handler),
        )
        // ── Storage routes ──
        .route(
            "/api/v1/storages/:cluster/:node/:storage/content",
            get(storages::storage_content),
        )
        .route(
            "/api/v1/storages/:cluster/:node/:storage/upload",
            post(storages::storage_upload_handler),
        )
        .route(
            "/api/v1/storages/:cluster/:node/:storage/content/:volid",
            delete(storages::delete_storage_content_handler),
        )
        .route(
            "/api/v1/networks/:cluster/:node",
            get(networks::node_networks),
        )
        .route(
            "/api/v1/tasks/:cluster/:node/:upid",
            get(tasks::task_status),
        )
        .route(
            "/api/v1/vms/:cluster/:node/:vmid/vnc/ticket",
            post(vnc::vnc_ticket_handler),
        )
        .route(
            "/api/v1/vms/:cluster/:node/:vmid/vnc/ws",
            get(vnc::vnc_ws_handler),
        )
        .route(
            "/api/v1/replication/:cluster/:vmid",
            post(replication::create_replication_job),
        )
        .route(
            "/api/v1/replication/:cluster/:vmid/delete",
            delete(replication::delete_replication_job),
        )
        .route(
            "/api/v1/replication/:cluster/:vmid/status",
            get(replication::get_replication_status),
        )
        // require_auth runs first (inner), require_cluster_access runs second (outer).
        .route_layer(from_fn_with_state(state.clone(), require_cluster_access))
        .route_layer(from_fn_with_state(state.clone(), require_auth));

    public
        .merge(protected)
        .merge(cluster_scoped)
        .merge(crate::ui::router::<crate::state::AppState>())
        .layer(from_fn_with_state(
            state.clone(),
            crate::audit::audit_middleware,
        ))
        .layer(RateLimitLayer::new(&state.config.auth.rate_limit))
        .layer(cors_layer(&state.config.auth.cors))
        .layer(axum::middleware::from_fn(security_headers_middleware))
        .with_state(state)
}

/// Security headers middleware — exposed as `pub` so the merged
/// API+UI router (built in `main.rs`) can apply it to UI responses
/// too. Without this, `/`, `/static/*` etc. would not get the
/// HSTS / X-Content-Type-Options / X-Frame-Options / Referrer-Policy
/// / CSP headers that the API endpoints receive.
pub async fn security_headers_middleware(
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
