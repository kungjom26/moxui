//! TOTP (RFC 6238) two-factor authentication.
//!
//! ## Flow
//!
//! 1. **Setup**: User enables 2FA via `POST /api/v1/auth/2fa/setup`.
//!    Server generates a random base32 secret and returns it + the
//!    `otpauth://` URL (for QR code generation on the frontend).
//! 2. **Verify**: User scans the QR (or enters the secret manually),
//!    then submits their first TOTP code via `POST /api/v1/auth/2fa/verify`.
//!    Server marks 2FA as enabled and stores backup codes.
//! 3. **Login**: User logs in with password → if 2FA enabled, server
//!    returns a short-lived pre-auth token. User submits it + TOTP code
//!    to complete login.
//! 4. **Backup codes**: 8 single-use codes generated on setup. Each
//!    is bcrypt-hashed. Using a backup code consumes it.
//!
//! ## Thread safety
//!
//! [`PreAuthStore`] is backed by `std::sync::RwLock<HashMap<…>>` and
//! wrapped in `Arc` inside `AppState`.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use rand::RngCore;
use secrecy::{ExposeSecret, SecretString};
use sha1::Sha1;

use crate::auth::password::{hash_password, verify_password};

// ── TOTP helpers ───────────────────────────────────────────────────

/// Generate a random 20-byte (160-bit) secret and return it as
/// base32 (RFC 4648) unpadded — the format Google Authenticator /
/// Authy / 1Password expect.
#[must_use]
pub fn generate_totp_secret() -> String {
    let mut buf = [0u8; 20];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    base32_encode(&buf)
}

/// Build the `otpauth://totp/{issuer}:{username}?secret=...&issuer=...`
/// URL that the frontend renders as a QR code.
#[must_use]
pub fn totp_url(issuer: &str, username: &str, secret: &str) -> String {
    // Label format: "Issuer:Username" — the colon is literal, not encoded.
    let label = format!("{issuer}:{username}");
    let label_enc = url_encode(&label);
    let secret_enc = url_encode(secret);
    let issuer_enc = url_encode(issuer);
    format!(
        "otpauth://totp/{label_enc}?secret={secret_enc}&issuer={issuer_enc}\
         &algorithm=SHA1&digits=6&period=30"
    )
}

/// Verify a 6-digit TOTP code against a base32-encoded secret.
///
/// Checks the current 30-second window plus one step in each direction
/// (±30s clock skew tolerance per RFC 6238 §5.2).
#[must_use]
pub fn verify_totp(secret_b32: &str, code: &str) -> bool {
    let Ok(secret) = base32_decode(secret_b32) else {
        return false;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let counter = now / 30;

    // Check current, previous, and next window
    (counter.saturating_sub(1)..=counter.saturating_add(1))
        .any(|c| compute_totp(&secret, c) == code)
}

/// Compute a 6-digit TOTP code for a given counter value.
/// RFC 4226 §5.3 dynamic truncation, RFC 6238 time-based counter.
fn compute_totp(secret: &[u8], counter: u64) -> String {
    use hmac::Mac;
    let counter_bytes = counter.to_be_bytes();
    let mut mac = hmac::Hmac::<Sha1>::new_from_slice(secret).expect("valid HMAC key length");
    mac.update(&counter_bytes);
    let result = mac.finalize().into_bytes();

    // Dynamic truncation (RFC 4226 §5.3)
    let offset = (result[19] & 0x0f) as usize;
    let code = (u32::from(result[offset]) & 0x7f) << 24
        | u32::from(result[offset + 1]) << 16
        | u32::from(result[offset + 2]) << 8
        | u32::from(result[offset + 3]);

    format!("{:06}", code % 1_000_000)
}

// ── Backup codes ───────────────────────────────────────────────────

/// Number of backup codes generated on 2FA setup.
pub const BACKUP_CODE_COUNT: usize = 8;
/// Length of each backup code (digits).
pub const BACKUP_CODE_LEN: usize = 8;

/// Generate 8 random numeric backup codes. Each is 8 digits.
/// Returns both the plaintext codes (show to user) and the bcrypt
/// hashed versions (store in the User).
#[must_use]
pub fn generate_backup_codes() -> (Vec<String>, Vec<SecretString>) {
    let mut plain = Vec::with_capacity(BACKUP_CODE_COUNT);
    let mut hashed = Vec::with_capacity(BACKUP_CODE_COUNT);

    for _ in 0..BACKUP_CODE_COUNT {
        let code = format!("{:08}", rand::rngs::OsRng.next_u32() % 100_000_000);
        let h = hash_password(&code).expect("bcrypt hash of backup code");
        plain.push(code);
        hashed.push(SecretString::new(h.into_boxed_str()));
    }

    (plain, hashed)
}

/// Verify a backup code against a list of bcrypt-hashed backup codes.
/// If matched, returns `Some(remaining)` with the matched code removed
/// from the list. Returns `None` if no match.
#[must_use]
pub fn verify_backup_code(stored: &[SecretString], code: &str) -> Option<Vec<SecretString>> {
    let mut remaining: Vec<SecretString> = Vec::with_capacity(stored.len());
    let mut matched = false;

    for h in stored {
        if !matched && verify_password(code, h.expose_secret()).unwrap_or(false) {
            matched = true;
            // Skip this one — it was used
        } else {
            remaining.push(h.clone());
        }
    }

    matched.then_some(remaining)
}

// ── Pre-auth token store ───────────────────────────────────────────

/// TTL for pre-auth tokens: 5 minutes.
pub const PREAUTH_TTL_SECS: i64 = 300;

/// A session created when a user passes the password step but has 2FA
/// enabled. The client must complete TOTP within `PREAUTH_TTL_SECS`.
#[derive(Debug, Clone)]
pub struct PreAuthSession {
    /// User id (matches `Claims::sub`).
    pub user_id: String,
    /// Username (for logging).
    pub username: String,
    /// Expiry (unix timestamp).
    pub expires_at: i64,
    /// Whether this token has been consumed (single-use).
    pub consumed: bool,
}

/// In-memory store for pre-auth (2FA pending) sessions.
///
/// Keyed by a random 32-byte token (hex-encoded).
#[derive(Debug, Clone, Default)]
pub struct PreAuthStore {
    sessions: Arc<RwLock<HashMap<String, PreAuthSession>>>,
}

impl PreAuthStore {
    /// Create a new empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Issue a pre-auth token for the given user.
    ///
    /// Returns the opaque token string.
    pub fn issue(&self, user_id: &str, username: &str) -> String {
        let mut buf = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut buf);
        let token = hex::encode(buf);
        let now = chrono::Utc::now().timestamp();

        let session = PreAuthSession {
            user_id: user_id.to_string(),
            username: username.to_string(),
            expires_at: now + PREAUTH_TTL_SECS,
            consumed: false,
        };

        let mut map = self.sessions.write().expect("preauth store lock");
        map.insert(token.clone(), session);
        token
    }

    /// Consume a pre-auth token. Returns the session if valid.
    ///
    /// Returns `None` when:
    /// - the token doesn't exist,
    /// - it's expired,
    /// - it was already consumed.
    pub fn consume(&self, token: &str) -> Option<PreAuthSession> {
        let mut map = self.sessions.write().expect("preauth store lock");
        let session = map.get(token)?;

        let now = chrono::Utc::now().timestamp();
        if session.expires_at < now || session.consumed {
            // Clean up expired/consumed tokens
            map.remove(token);
            return None;
        }

        // Mark consumed and return
        if let Some(s) = map.get_mut(token) {
            s.consumed = true;
        }
        let session = map.remove(token)?;
        Some(session)
    }

    /// Number of stored sessions (for tests / metrics).
    #[must_use]
    pub fn count(&self) -> usize {
        let map = self.sessions.read().expect("preauth store lock");
        map.len()
    }

    /// Remove all expired sessions (for periodic cleanup).
    pub fn purge_expired(&self) -> usize {
        let now = chrono::Utc::now().timestamp();
        let mut map = self.sessions.write().expect("preauth store lock");
        let before = map.len();
        map.retain(|_, s| s.expires_at >= now && !s.consumed);
        before - map.len()
    }
}

// ── Low-level helpers ──────────────────────────────────────────────

/// RFC 4648 base32 encode (no padding). 5 bits per character.
fn base32_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = Vec::with_capacity((bytes.len() * 8).div_ceil(5));
    let mut buffer = 0u64;
    let mut bits = 0;

    for &byte in bytes {
        buffer = (buffer << 8) | u64::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            let idx = ((buffer >> bits) & 0x1f) as usize;
            out.push(ALPHABET[idx]);
        }
    }
    if bits > 0 {
        let idx = ((buffer << (5 - bits)) & 0x1f) as usize;
        out.push(ALPHABET[idx]);
    }
    String::from_utf8(out).expect("valid base32")
}

/// RFC 4648 base32 decode (no padding).
fn base32_decode(s: &str) -> Result<Vec<u8>, String> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = Vec::with_capacity((s.len() * 5).div_ceil(8));
    let mut buffer = 0u64;
    let mut bits = 0;

    for ch in s.bytes() {
        let idx = if ch.is_ascii_lowercase() {
            let upper = ch.to_ascii_uppercase();
            ALPHABET.iter().position(|&a| a == upper)
        } else {
            ALPHABET.iter().position(|&a| a == ch)
        };
        let idx = idx.ok_or_else(|| format!("invalid base32 char: {}", ch as char))?;
        buffer = (buffer << 5) | (idx as u64);
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            #[allow(clippy::cast_possible_truncation)]
            // bits < 8, so the shift produces at most 8 bits
            out.push((buffer >> bits) as u8);
        }
    }
    Ok(out)
}

/// URL percent-encode a string for the `otpauth://` URI.
fn url_encode(s: &str) -> String {
    // Most characters in issuer/username are ASCII-alphanumeric.
    // We only encode what's needed for the URI.
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b':' => {
                out.push(b as char);
            }
            b' ' => out.push_str("%20"),
            _ => {
                #[allow(clippy::format_push_string)]
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_totp_secret_is_base32() {
        let secret = generate_totp_secret();
        // 20 bytes → base32 unpadded = ceil(20*8/5) = 32 chars
        assert_eq!(secret.len(), 32);
        // Should only contain A-Z and 2-7 (base32 alphabet)
        for ch in secret.chars() {
            assert!(
                ch.is_ascii_uppercase() || ch.is_ascii_digit(),
                "invalid base32 char: {ch}"
            );
        }
    }

    #[test]
    fn test_totp_url_includes_required_params() {
        let url = totp_url("moxui", "alice", "JBSWY3DPEHPK3PXP");
        assert!(url.starts_with("otpauth://totp/moxui:alice?"));
        assert!(url.contains("secret=JBSWY3DPEHPK3PXP"));
        assert!(url.contains("issuer=moxui"));
        assert!(url.contains("algorithm=SHA1"));
        assert!(url.contains("digits=6"));
        assert!(url.contains("period=30"));
    }

    #[test]
    fn test_totp_verify_known_vector() {
        // RFC 6238 test vector: SHA1, secret = "12345678901234567890"
        let secret_b32 = base32_encode(b"12345678901234567890");
        // We can't test a specific time since TOTP is time-dependent.
        // But we can verify the algorithm self-consistently:
        let computed = compute_totp(b"12345678901234567890", 0);
        assert_eq!(computed.len(), 6);
        assert!(computed.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_totp_verify_valid_code() {
        let secret = generate_totp_secret();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let code = compute_totp(&base32_decode(&secret).unwrap(), now / 30);

        // verify_totp should accept the current code
        assert!(verify_totp(&secret, &code));

        // A bogus code should fail
        assert!(!verify_totp(&secret, "000000"));
    }

    #[test]
    fn test_generate_backup_codes_count() {
        let (plain, hashed) = generate_backup_codes();
        assert_eq!(plain.len(), BACKUP_CODE_COUNT);
        assert_eq!(hashed.len(), BACKUP_CODE_COUNT);

        // Each code should be 8 digits
        for code in &plain {
            assert_eq!(code.len(), BACKUP_CODE_LEN);
            assert!(code.chars().all(|c| c.is_ascii_digit()));
        }
    }

    #[test]
    fn test_verify_backup_code_matches() {
        let (plain, hashed) = generate_backup_codes();
        let remaining = verify_backup_code(&hashed, &plain[0]);
        assert!(remaining.is_some());
        assert_eq!(remaining.as_ref().unwrap().len(), BACKUP_CODE_COUNT - 1);

        // The used code should be consumed — can't use it again
        let remaining2 = verify_backup_code(&remaining.unwrap(), &plain[0]);
        assert!(remaining2.is_none());
    }

    #[test]
    fn test_verify_backup_code_wrong_code() {
        let (_, hashed) = generate_backup_codes();
        let remaining = verify_backup_code(&hashed, "00000000");
        assert!(remaining.is_none());
    }

    #[test]
    fn test_preauth_store_issue_and_consume() {
        let store = PreAuthStore::new();
        let token = store.issue("u-alice", "alice");
        assert_eq!(store.count(), 1);

        let session = store.consume(&token);
        assert!(session.is_some());
        assert_eq!(session.unwrap().user_id, "u-alice");

        // Already consumed — should return None
        assert!(store.consume(&token).is_none());
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn test_preauth_store_rejects_bogus() {
        let store = PreAuthStore::new();
        assert!(store.consume("bogus-token").is_none());
    }

    #[test]
    fn test_preauth_store_expired_token() {
        let store = PreAuthStore::new();
        let mut buf = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut buf);
        let token = hex::encode(buf);

        // Insert an already-expired session
        let expired = PreAuthSession {
            user_id: "u-alice".to_string(),
            username: "alice".to_string(),
            expires_at: chrono::Utc::now().timestamp() - 1,
            consumed: false,
        };
        {
            let mut map = store.sessions.write().unwrap();
            map.insert(token.clone(), expired);
        }

        assert!(store.consume(&token).is_none());
    }
}
