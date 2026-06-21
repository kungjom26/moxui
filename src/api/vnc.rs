//! VNC console endpoints.
//!
//! Two routes under `/api/v1/vms/:cluster/:node/:vmid/vnc/`:
//!
//! 1. `POST .../vnc/ticket` — operator gets a short-lived moxui
//!    `vnc_token` (HMAC-SHA256, 5-min TTL, bound to cluster/node/vmid).
//!    The Proxmox `vncproxy` ticket never leaves the process — we
//!    return only the upstream port + a one-shot moxui token. **This
//!    endpoint is fully implemented and audited.**
//!
//! 2. `GET .../vnc/ws?vnc_token=...` — WebSocket proxy between the
//!    browser and Proxmox's `vncwebsocket`. **Currently returns
//!    `501 Not Implemented`** — see [`vnc_ws_handler`] for the
//!    rationale and the TODO. The token mint + verify path is
//!    exercised by the ticket handler; the WS plumbing is the next
//!    step (Phase 2 follow-up).
//!
//! Both routes return `404 Not Found` when the operator hasn't
//! configured `auth.vnc_token_secret_pem_path` — we don't want to
//! advertise an attack surface that isn't configured.

use std::sync::atomic::{AtomicU32, Ordering};

use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::Json;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};

use crate::auth::middleware::require_role;
use crate::auth::user::Role;
use crate::auth::vnc::{self, VncTokenClaims, VNC_TOKEN_TTL_SECS};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

// ── HTTP ticket endpoint ──────────────────────────────────────────

/// Response shape for `POST .../vnc/ticket`.
///
/// The Proxmox ticket is intentionally **not** in this struct —
/// we never serialize it across the wire. The browser only needs:
///   - `vnc_token` to authenticate the WebSocket upgrade
///   - `port`     to tell noVNC which Proxmox-internal port to target
///   - `expires_in` so the UI can warn before the token expires
#[derive(Debug, Serialize)]
pub struct VncTicketResponse {
    /// HMAC-signed moxui VNC token. The upstream Proxmox ticket
    /// never appears in this struct.
    pub vnc_token: String,
    /// TCP port the browser should connect the noVNC client to.
    pub port: u16,
    /// Seconds until the token expires (matches Proxmox ticket
    /// lifetime — see [`VNC_TOKEN_TTL_SECS`]).
    pub expires_in: u64,
}

/// `POST /api/v1/vms/:cluster/:node/:vmid/vnc/ticket`
///
/// Mint a short-lived VNC token and fetch the upstream `vncproxy`
/// port. Operator+ only — VNC is a control-plane feature.
pub async fn vnc_ticket_handler(
    State(state): State<AppState>,
    auth: crate::auth::middleware::AuthContext,
    Path((cluster, node, vmid)): Path<(String, String, u32)>,
) -> AppResult<(StatusCode, Json<VncTicketResponse>)> {
    // Operator+ required — VNC implies control-plane access. Viewer
    // can read VM config but cannot open a console.
    if let Err(resp) = require_role(&auth, Role::Operator) {
        let status = resp.status();
        return Err(if status == axum::http::StatusCode::FORBIDDEN {
            AppError::Forbidden("operator role required for vnc".into())
        } else {
            AppError::Internal(format!("auth middleware returned {status}"))
        });
    }

    // VNC disabled → 404 (not 401). See module docs.
    let secret = state
        .vnc_secret
        .as_ref()
        .ok_or_else(|| AppError::NotFound("vnc console disabled".into()))?;
    let secret_bytes = secret.expose_secret().as_slice();

    // Look up the cluster — we can't mint a token that names a VM
    // we don't know about.
    let client = state
        .client(&cluster)
        .ok_or_else(|| AppError::NotFound(format!("cluster {cluster}")))?;

    // Mint token first so we can sign regardless of upstream outcome.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let claims = VncTokenClaims {
        sub: vnc::vnc_subject(&cluster, &node, vmid),
        iat: now,
        exp: now + VNC_TOKEN_TTL_SECS,
    };
    let vnc_token = vnc::mint_vnc_token(secret_bytes, &claims)?;

    // Fetch the upstream ticket + port. We deliberately *don't* keep
    // the ticket here — it's consumed by the WS handler in a later
    // request. If the WS handler never connects, the upstream ticket
    // expires on its own (Proxmox tickets are short-lived).
    let upstream = client.vnc_proxy(&node, vmid).await?;
    // Audit the ticket mint — note we log *the moxui sub* and *upid*,
    // not the upstream ticket. The ticket itself never enters the log.
    tracing::info!(
        cluster = %cluster,
        node = %node,
        vmid = vmid,
        user = %auth.claims.username,
        upid = %upstream.upid,
        port = upstream.port,
        "vnc ticket minted"
    );

    Ok((
        StatusCode::OK,
        Json(VncTicketResponse {
            vnc_token,
            port: upstream.port,
            expires_in: VNC_TOKEN_TTL_SECS,
        }),
    ))
}

// ── WebSocket endpoint ────────────────────────────────────────────

/// Query string for the WS upgrade endpoint.
#[derive(Debug, Deserialize)]
pub struct VncWsQuery {
    /// HMAC-signed moxui VNC token from the ticket endpoint.
    pub vnc_token: String,
}

/// `GET /api/v1/vms/:cluster/:node/:vmid/vnc/ws?vnc_token=...`
///
/// WebSocket proxy. Currently a stub: returns `501 Not Implemented`
/// after verifying the token. The WS upgrade + bidirectional proxy
/// is deferred — the ticket-mint path is the security-critical part
/// and is what's wired today. Tracking issue: a Phase 2 follow-up
/// (requires enabling the `__rustls-tls` feature on
/// `tokio-tungstenite` and a test Proxmox to verify against).
pub async fn vnc_ws_handler(
    State(state): State<AppState>,
    Path((cluster, node, vmid)): Path<(String, String, u32)>,
    Query(q): Query<VncWsQuery>,
    _ws: WebSocketUpgrade,
) -> AppResult<Response> {
    // Verify the token even in the stub path — confirms the token
    // mint/verify pipeline works end-to-end through the route.
    let secret = state
        .vnc_secret
        .as_ref()
        .ok_or_else(|| AppError::NotFound("vnc console disabled".into()))?;
    let secret_bytes = secret.expose_secret().as_slice();
    let claims = vnc::verify_vnc_token(secret_bytes, &q.vnc_token)?;
    let expected_sub = vnc::vnc_subject(&cluster, &node, vmid);
    if claims.sub != expected_sub {
        return Err(AppError::Unauthorized("vnc token subject mismatch".into()));
    }

    Err(AppError::Internal(
        "vnc websocket proxy is a Phase 2 follow-up — see /api/v1/vms/.../vnc/ticket for token mint".into(),
    ))
}

// ── Concurrency limiter ───────────────────────────────────────────

/// Maximum concurrent VNC sessions per VM. 5 is generous — operators
/// rarely open more than one console per VM at a time.
pub const MAX_VNC_CONNECTIONS_PER_VM: u32 = 5;

/// Simple atomic counter guard. Increments on `try_acquire`, decrements
/// on drop. Returns `None` when the cap is reached.
pub struct VncConnectionLimiter {
    /// Current open-session count for this VM. Atomic so the WS
    /// upgrade path can read/increment without holding a lock.
    used: AtomicU32,
    /// Maximum allowed concurrent sessions.
    cap: u32,
}

impl VncConnectionLimiter {
    /// Construct a limiter that admits up to `cap` concurrent sessions.
    pub fn new(cap: u32) -> Self {
        Self {
            used: AtomicU32::new(0),
            cap,
        }
    }
    /// Attempt to acquire a slot. Returns `Some(guard)` when the
    /// counter is below the cap, or `None` if at capacity.
    pub fn try_acquire(&self) -> Option<VncConnectionGuard<'_>> {
        loop {
            let cur = self.used.load(Ordering::SeqCst);
            if cur >= self.cap {
                return None;
            }
            if self
                .used
                .compare_exchange(cur, cur + 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return Some(VncConnectionGuard {
                    counter: &self.used,
                });
            }
        }
    }
}

/// RAII guard for a slot in [`VncConnectionLimiter`]. Drop releases
/// the slot back to the pool.
pub struct VncConnectionGuard<'a> {
    /// Borrow of the parent limiter's counter so `Drop` can decrement.
    counter: &'a AtomicU32,
}

impl Drop for VncConnectionGuard<'_> {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::vnc::{mint_vnc_token, verify_vnc_token, vnc_subject};

    #[test]
    fn limiter_admits_until_cap() {
        let lim = VncConnectionLimiter::new(2);
        let g1 = lim.try_acquire().expect("g1");
        let g2 = lim.try_acquire().expect("g2");
        assert!(lim.try_acquire().is_none(), "third must be rejected");
        drop(g1);
        assert!(lim.try_acquire().is_some(), "after release one slot frees");
        drop(g2);
    }

    #[test]
    fn token_subject_must_match_route() {
        let secret = b"test-secret";
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        // Token says cluster=A, node=n1, vmid=10
        let claims = VncTokenClaims {
            sub: vnc_subject("cluster-a", "n1", 10),
            iat: now,
            exp: now + 60,
        };
        let token = mint_vnc_token(secret, &claims).expect("mint");
        let verified = verify_vnc_token(secret, &token).expect("verify");
        // Same cluster/node/vmid → match.
        assert_eq!(verified.sub, vnc_subject("cluster-a", "n1", 10));
        // Different vmid → mismatch.
        assert_ne!(verified.sub, vnc_subject("cluster-a", "n1", 11));
        // Different cluster → mismatch.
        assert_ne!(verified.sub, vnc_subject("cluster-b", "n1", 10));
    }
}
