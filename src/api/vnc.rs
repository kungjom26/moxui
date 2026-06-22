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
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::Json;
use futures_util::{SinkExt, StreamExt};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use tokio_tungstenite::connect_async_with_config;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

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
/// WebSocket proxy between the browser (noVNC) and Proxmox's
/// `vncwebsocket` endpoint. On upgrade we:
///
/// 1. Verify the signed `vnc_token`.
/// 2. Fetch a fresh Proxmox `vncproxy` ticket (short-lived).
/// 3. Connect to Proxmox's internal VNC WebSocket.
/// 4. Pipe data bidirectionally until either side disconnects.
pub async fn vnc_ws_handler(
    State(state): State<AppState>,
    Path((cluster, node, vmid)): Path<(String, String, u32)>,
    Query(q): Query<VncWsQuery>,
    ws: WebSocketUpgrade,
) -> AppResult<Response> {
    // 1. Verify the moxui VNC token.
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

    // 2. Look up the Proxmox client for this cluster.
    let client = state
        .client(&cluster)
        .ok_or_else(|| AppError::NotFound(format!("cluster {cluster}")))?;

    // 3. Fetch a fresh vncproxy ticket from Proxmox.
    let upstream = client.vnc_proxy(&node, vmid).await?;

    // 4. Build the upstream VNC WebSocket URL.
    let host_url = client.base_url();
    // Parse the host from the cluster URL (strip scheme + port).
    let ws_scheme = if host_url.starts_with("https") { "wss" } else { "ws" };
    let host = host_url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    let ws_url = format!(
        "{ws_scheme}://{host}/api2/json/nodes/{node}/qemu/{vmid}/vncwebsocket?port={port}&vncticket={ticket}",
        port = upstream.port,
        ticket = upstream.ticket.expose_secret(),
    );

    // 5. Concurrency limiter — check we haven't exceeded the per-VM cap.
    let limiter_key = format!("{cluster}:{node}:{vmid}");
    let limiter = {
        let mut limiters = state.vnc_limiters.lock().await;
        limiters
            .entry(limiter_key.clone())
            .or_insert_with(|| Arc::new(crate::api::vnc::VncConnectionLimiter::new(MAX_VNC_CONNECTIONS_PER_VM)))
            .clone()
    };
    let _guard = limiter.try_acquire().ok_or_else(|| {
        AppError::TooManyRequests("too many concurrent VNC sessions for this VM".into())
    })?;

    // 6. Perform the WebSocket upgrade.
    Ok(ws.on_upgrade(move |socket| {
        proxmox_vnc_proxy(socket, ws_url, state.config.auth.jwt_issuer.clone())
    }))
}

/// Bidirectional pipe between the browser's axum WebSocket and
/// Proxmox's VNC WebSocket (via tokio-tungstenite).
async fn proxmox_vnc_proxy(
    browser_ws: WebSocket,
    upstream_ws_url: String,
    _issuer: String,
) {
    // Build the tungstenite request from the URL.
    let request = match upstream_ws_url.into_client_request() {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "vnc: failed to build upstream WS request");
            return;
        }
    };

    // Connect to the Proxmox VNC WebSocket.
    let (upstream_ws, _upstream_resp) = match connect_async_with_config(
        request,
        None,
        false, // disable_auto_redirect
    )
    .await
    {
        Ok(connected) => connected,
        Err(e) => {
            tracing::error!(error = %e, "vnc: failed to connect to upstream WS");
            return;
        }
    };

    tracing::info!("vnc: WebSocket proxy connected, piping bidirectionally");

    // Split both sides into sender + receiver halves.
    let (mut browser_sender, mut browser_receiver) = browser_ws.split();
    let (mut upstream_sender, mut upstream_receiver) = upstream_ws.split();

    // Pipe: browser → upstream (binary frames only, VNC is raw RFB)
    let browser_to_upstream = tokio::spawn(async move {
        while let Some(msg) = browser_receiver.next().await {
            match msg {
                Ok(Message::Binary(data)) => {
                    if let Err(e) = upstream_sender
                        .send(tokio_tungstenite::tungstenite::Message::Binary(data))
                        .await
                    {
                        tracing::debug!(error = %e, "vnc: browser→upstream send error");
                        break;
                    }
                }
                Ok(Message::Close(_)) => {
                    let _ = upstream_sender
                        .send(tokio_tungstenite::tungstenite::Message::Close(None))
                        .await;
                    break;
                }
                Ok(Message::Ping(data)) => {
                    let _ = upstream_sender
                        .send(tokio_tungstenite::tungstenite::Message::Ping(data))
                        .await;
                }
                Ok(Message::Pong(data)) => {
                    let _ = upstream_sender
                        .send(tokio_tungstenite::tungstenite::Message::Pong(data))
                        .await;
                }
                Ok(Message::Text(_)) => {
                    // noVNC sends binary; ignore text frames
                }
                Err(e) => {
                    tracing::debug!(error = %e, "vnc: browser WS recv error");
                    break;
                }
            }
        }
    });

    // Pipe: upstream → browser (binary frames only)
    let upstream_to_browser = tokio::spawn(async move {
        while let Some(msg) = upstream_receiver.next().await {
            match msg {
                Ok(tokio_tungstenite::tungstenite::Message::Binary(data)) => {
                    if let Err(e) = browser_sender.send(Message::Binary(data)).await {
                        tracing::debug!(error = %e, "vnc: upstream→browser send error");
                        break;
                    }
                }
                Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => {
                    let _ = browser_sender.send(Message::Close(None)).await;
                    break;
                }
                Ok(tokio_tungstenite::tungstenite::Message::Ping(data)) => {
                    let _ = browser_sender.send(Message::Ping(data)).await;
                }
                Ok(tokio_tungstenite::tungstenite::Message::Pong(data)) => {
                    let _ = browser_sender.send(Message::Pong(data)).await;
                }
                Ok(tokio_tungstenite::tungstenite::Message::Text(_)) => {
                    // ignore text from upstream
                }
                Ok(tokio_tungstenite::tungstenite::Message::Frame(_)) => {
                    // ignore raw frames (handled by higher-level types)
                }
                Err(e) => {
                    tracing::debug!(error = %e, "vnc: upstream WS recv error");
                    break;
                }
            }
        }
    });

    // Wait for either direction to finish, then drop everything.
    tokio::select! {
        _ = browser_to_upstream => {},
        _ = upstream_to_browser => {},
    }

    tracing::info!("vnc: WebSocket proxy disconnected");
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
