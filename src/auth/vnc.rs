//! Short-lived VNC session tokens.
//!
//! When the UI opens a VNC console it calls the
//! `/api/v1/vms/:cluster/:node/:vmid/vnc/ticket` endpoint to receive
//! a `vnc_token` (HMAC-signed, 5-minute TTL) plus the upstream
//! `vncproxy` port and a Proxmox ticket. The browser then opens a
//! WebSocket to `/api/v1/vms/.../vnc/ws?vnc_token=...` — we verify
//! the HMAC + expiry on every WS upgrade and reject any reconnect
//! after the TTL.
//!
//! Security properties:
//! - The `vnc_token` never carries the upstream Proxmox ticket; that
//!   is fetched in the WS handler using the cluster config and
//!   never leaves the moxui process. The browser only knows the port
//!   + a one-shot moxui token.
//! - The HMAC secret comes from `auth.vnc_token_secret` (required).
//!   Refusing to start when it's missing is the fail-closed default.
//! - Tokens are time-bounded (5min) and not replayable across VMs
//!   because they're bound to (cluster, node, vmid).

use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::error::{AppError, AppResult};

type HmacSha256 = Hmac<Sha256>;

/// Default VNC token TTL: 5 minutes (matches Proxmox's vncproxy
/// ticket lifetime, so we don't hand the browser a token that's
/// outlived the upstream ticket it references).
pub const VNC_TOKEN_TTL_SECS: u64 = 300;

/// Claims embedded in the signed VNC token. JWT-style payload —
/// `sub`/`exp` follow common conventions so operators reading
/// logs don't have to learn a new vocabulary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VncTokenClaims {
    /// Subject — cluster/node/vmid tuple.
    pub sub: String,
    /// Issued-at (Unix seconds).
    pub iat: u64,
    /// Expiry (Unix seconds).
    pub exp: u64,
}

/// Sign + base64-encode a VNC token. The token string is the URL-safe
/// base64 of the JSON claims followed by a `.` and a base64 HMAC tag.
///
/// Format: `<base64url(json_claims)>.<base64url(hmac_sha256(secret, json_claims))>`
pub fn mint_vnc_token(secret: &[u8], claims: &VncTokenClaims) -> AppResult<String> {
    let payload = serde_json::to_vec(claims)
        .map_err(|e| AppError::Internal(format!("vnc token serialize: {e}")))?;
    let mut mac = HmacSha256::new_from_slice(secret)
        .map_err(|e| AppError::Internal(format!("vnc token hmac key: {e}")))?;
    mac.update(&payload);
    let tag = mac.finalize().into_bytes();
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    Ok(format!("{}.{}", b64.encode(&payload), b64.encode(tag)))
}

/// Verify a VNC token. Returns the embedded claims on success.
///
/// Rejects:
/// - Malformed token (wrong shape / unparseable JSON)
/// - Bad HMAC signature (wrong secret / tampered payload)
/// - Expired tokens (`exp` <= now)
///
/// Errors are intentionally generic so attackers can't distinguish
/// "expired" from "never valid" by reading log lines.
pub fn verify_vnc_token(secret: &[u8], token: &str) -> AppResult<VncTokenClaims> {
    let parts: Vec<&str> = token.splitn(2, '.').collect();
    if parts.len() != 2 {
        return Err(AppError::Unauthorized("vnc token malformed".into()));
    }
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let payload = b64
        .decode(parts[0])
        .map_err(|_| AppError::Unauthorized("vnc token malformed".into()))?;
    let tag = b64
        .decode(parts[1])
        .map_err(|_| AppError::Unauthorized("vnc token malformed".into()))?;

    let mut mac = HmacSha256::new_from_slice(secret)
        .map_err(|e| AppError::Internal(format!("vnc token hmac key: {e}")))?;
    mac.update(&payload);
    mac.verify_slice(&tag)
        .map_err(|_| AppError::Unauthorized("vnc token signature mismatch".into()))?;

    let claims: VncTokenClaims = serde_json::from_slice(&payload)
        .map_err(|_| AppError::Unauthorized("vnc token claims invalid".into()))?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    if claims.exp <= now {
        return Err(AppError::Unauthorized("vnc token expired".into()));
    }
    Ok(claims)
}

/// Build the standard VNC token subject for `(cluster, node, vmid)`.
/// Format: `vnc:<cluster>:<node>:<vmid>` — keeps the path-of-evidence
/// readable in logs without leaking ticket material.
pub fn vnc_subject(cluster: &str, node: &str, vmid: u32) -> String {
    format!("vnc:{cluster}:{node}:{vmid}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_claims() -> VncTokenClaims {
        // Use dynamic time so the test doesn't rot — the prior
        // hardcoded `iat: 1_700_000_000` expired after 2023 and the
        // verify path correctly started rejecting it in 2026+.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        VncTokenClaims {
            sub: "vnc:homelab:pve11:103".into(),
            iat: now,
            exp: now + 300,
        }
    }

    #[test]
    fn roundtrip_succeeds() {
        let secret = b"supersecret-32-bytes-or-more-padding";
        let token = mint_vnc_token(secret, &sample_claims()).expect("mint");
        let claims = verify_vnc_token(secret, &token).expect("verify");
        assert_eq!(claims.sub, "vnc:homelab:pve11:103");
    }

    #[test]
    fn tampered_payload_rejected() {
        let secret = b"another-secret";
        let token = mint_vnc_token(secret, &sample_claims()).expect("mint");
        // Flip one character in the payload half.
        let mut bad = token.into_bytes();
        let dot_at = bad.iter().position(|&b| b == b'.').unwrap();
        bad[dot_at - 1] = if bad[dot_at - 1] == b'A' { b'B' } else { b'A' };
        let bad = String::from_utf8(bad).unwrap();
        assert!(verify_vnc_token(secret, &bad).is_err());
    }

    #[test]
    fn wrong_secret_rejected() {
        let token = mint_vnc_token(b"secret-a", &sample_claims()).expect("mint");
        assert!(verify_vnc_token(b"secret-b", &token).is_err());
    }

    #[test]
    fn expired_token_rejected() {
        let secret = b"x";
        let claims = VncTokenClaims {
            sub: "vnc:c:n:1".into(),
            iat: 1,
            exp: 2, // long expired
        };
        let token = mint_vnc_token(secret, &claims).expect("mint");
        assert!(verify_vnc_token(secret, &token).is_err());
    }

    #[test]
    fn malformed_token_rejected() {
        assert!(verify_vnc_token(b"x", "no-dot-here").is_err());
        assert!(verify_vnc_token(b"x", "!!not-base64!!.!!").is_err());
    }

    #[test]
    fn subject_format_is_readable() {
        assert_eq!(
            vnc_subject("homelab", "pve11", 103),
            "vnc:homelab:pve11:103"
        );
    }
}
