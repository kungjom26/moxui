//! Refresh token rotation (7-day TTL, issue / verify / revoke / rotate).
//!
//! ## Security model
//!
//! - Refresh tokens are opaque 32-byte random strings (base64-encoded).
//! - Only the SHA-256 hash is stored in the [`RefreshStore`] — the plaintext
//!   token is never persisted.
//! - Rotation invalidates the previous token on each use (refresh token
//!   rotation, draft-ietf-oauth-security-topics §4.12).
//! - 7-day TTL configured via `auth.refresh_token_ttl_secs` in config.
//! - Single-use: once a token is used to refresh, it's revoked and a new one
//!   is issued. If a revoked token is replayed, all tokens for that user are
//!   revoked (family detection — see [`RefreshStore::detect_family`]).
//!
//! ## Endpoints
//!
//! - `POST /api/v1/auth/refresh` — exchange a refresh token for a new JWT +
//!   new refresh token (rotation).
//! - `POST /api/v1/auth/logout`   — revoke a refresh token explicitly.
//!
//! ## Thread safety
//!
//! [`RefreshStore`] is backed by `std::sync::RwLock<HashMap<…>>` and wrapped
//! in `Arc` inside `AppState`. Reads/writes are fine-grained: lookups hold a
//! read lock; mutations (revoke / insert) hold a write lock.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Default TTL for refresh tokens: 7 days (in seconds).
pub const DEFAULT_REFRESH_TTL_SECS: i64 = 7 * 24 * 3600;

/// An in-memory refresh token record.
///
/// The actual token string is never stored — only its SHA-256 hash.
/// The plaintext token is returned to the client exactly once (on issue).
#[derive(Debug, Clone)]
pub struct StoredRefreshToken {
    /// Unique id (UUID v4, key in the store).
    pub id: String,
    /// User id (matches [`crate::auth::Claims::sub`]).
    pub user_id: String,
    /// SHA-256 hex digest of the raw token. We never store the raw token.
    pub token_hash: String,
    /// Expiry (unix timestamp).
    pub expires_at: i64,
    /// Revoked flag. Revoked tokens are rejected — they exist only for
    /// replay detection (family revocation).
    pub revoked: bool,
    /// When this record was created (unix timestamp).
    pub created_at: i64,
}

/// Shared, thread-safe, in-memory refresh token store.
///
/// Follows the same pattern as [`crate::auth::UserStore`]: an `Arc`-wrapped
/// inner struct with `RwLock` for concurrent access.
#[derive(Debug, Clone, Default)]
pub struct RefreshStore {
    tokens: Arc<RwLock<HashMap<String, StoredRefreshToken>>>,
}

/// The result of issuing a new refresh token: the opaque token string
/// (returned to the client once) plus the record id.
pub struct IssuedRefreshToken {
    /// Opaque token string (base64, 32 bytes random). Hand this to the
    /// client — it will **not** be retrievable from the store again.
    pub token: String,
    /// Internal record id (UUID).
    pub id: String,
    /// User id that owns this token.
    pub user_id: String,
    /// Expiry timestamp.
    pub expires_at: i64,
}

impl RefreshStore {
    /// Create an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Issue a new refresh token for the given user.
    ///
    /// Returns the opaque token string. This is the **only** time the
    /// plaintext is available — after this call, only the SHA-256 hash
    /// is kept in memory.
    pub fn issue(&self, user_id: &str, ttl_secs: i64) -> IssuedRefreshToken {
        let id = Uuid::new_v4().to_string();
        let raw = generate_token();
        let hash = hash_token(&raw);
        let now = chrono::Utc::now().timestamp();

        let record = StoredRefreshToken {
            id: id.clone(),
            user_id: user_id.to_string(),
            token_hash: hash,
            expires_at: now + ttl_secs,
            revoked: false,
            created_at: now,
        };

        let mut map = self.tokens.write().expect("refresh store lock");
        map.insert(id.clone(), record);

        IssuedRefreshToken {
            token: raw,
            id,
            user_id: user_id.to_string(),
            expires_at: now + ttl_secs,
        }
    }

    /// Verify a refresh token and return the associated record id + user_id.
    ///
    /// Returns `None` when:
    /// - the hash does not match any stored token,
    /// - the token is revoked,
    /// - the token has expired.
    pub fn verify(&self, raw_token: &str) -> Option<(String, String)> {
        let hash = hash_token(raw_token);
        let map = self.tokens.read().expect("refresh store lock");

        // Linear scan: we can't index by hash since the token is random and
        // the store is small (O(users × tokens) — at most a few dozen entries).
        for record in map.values() {
            if record.token_hash == hash {
                if record.revoked {
                    return None;
                }
                if chrono::Utc::now().timestamp() > record.expires_at {
                    return None;
                }
                return Some((record.id.clone(), record.user_id.clone()));
            }
        }
        None
    }

    /// Revoke a single refresh token by id.
    ///
    /// Returns `true` if the token was found and revoked, `false` if the
    /// id doesn't exist in the store.
    pub fn revoke(&self, id: &str) -> bool {
        let mut map = self.tokens.write().expect("refresh store lock");
        if let Some(record) = map.get_mut(id) {
            record.revoked = true;
            true
        } else {
            false
        }
    }

    /// Revoke **all** refresh tokens for a given user. Used for:
    /// - Family detection (a revoked token was replayed → attacker has it).
    /// - Admin force-logout.
    /// - Password change.
    pub fn revoke_all_for_user(&self, user_id: &str) -> usize {
        let mut map = self.tokens.write().expect("refresh store lock");
        let mut count = 0;
        for record in map.values_mut() {
            if record.user_id == user_id && !record.revoked {
                record.revoked = true;
                count += 1;
            }
        }
        count
    }

    /// Rotate: verify the current token, revoke it, issue a new one.
    ///
    /// Returns `None` if the token is invalid/revoked/expired.
    /// On success, returns the new token pair.
    ///
    /// ⚠️ **Replay detection**: if the old token was **already** revoked
    /// (meaning someone replayed it), we revoke **all** tokens for that
    /// user ([family revocation], draft-ietf-oauth-security-topics §4.12).
    pub fn rotate(
        &self,
        raw_token: &str,
        ttl_secs: i64,
    ) -> Option<IssuedRefreshToken> {
        let hash = hash_token(raw_token);
        let mut map = self.tokens.write().expect("refresh store lock");

        let found = map.iter().find(|(_, r)| r.token_hash == hash).map(|(id, r)| (id.clone(), r.clone()));

        match found {
            None => None,
            Some((id, record)) => {
                if chrono::Utc::now().timestamp() > record.expires_at {
                    return None; // expired
                }
                if record.revoked {
                    // Replay attack! Someone already used this token.
                    // Revoke ALL tokens for this user.
                    let user_id = record.user_id.clone();
                    for r in map.values_mut() {
                        if r.user_id == user_id {
                            r.revoked = true;
                        }
                    }
                    return None;
                }

                // Revoke the old token and issue a new one.
                let user_id = record.user_id.clone();
                if let Some(r) = map.get_mut(&id) {
                    r.revoked = true;
                }

                let new_id = Uuid::new_v4().to_string();
                let new_raw = generate_token();
                let new_hash = hash_token(&new_raw);
                let now = chrono::Utc::now().timestamp();

                let new_record = StoredRefreshToken {
                    id: new_id.clone(),
                    user_id: user_id.clone(),
                    token_hash: new_hash,
                    expires_at: now + ttl_secs,
                    revoked: false,
                    created_at: now,
                };
                map.insert(new_id.clone(), new_record);

                Some(IssuedRefreshToken {
                    token: new_raw,
                    id: new_id,
                    user_id,
                    expires_at: now + ttl_secs,
                })
            }
        }
    }

    /// Number of stored tokens (for tests / metrics).
    #[must_use]
    pub fn count(&self) -> usize {
        let map = self.tokens.read().expect("refresh store lock");
        map.len()
    }

    /// Number of active (non-revoked, non-expired) tokens (for tests / metrics).
    #[must_use]
    pub fn active_count(&self) -> usize {
        let now = chrono::Utc::now().timestamp();
        let map = self.tokens.read().expect("refresh store lock");
        map.values()
            .filter(|r| !r.revoked && r.expires_at > now)
            .count()
    }
}

/// Generate a cryptographically random 32-byte token, base64-encoded (no
/// padding). 32 bytes = 256 bits of entropy — sufficient for bearer tokens.
fn generate_token() -> String {
    use rand::RngCore;
    let mut buf = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD_NO_PAD, buf)
}

/// SHA-256 hex digest of a token. Used for storage — the plaintext is
/// never persisted.
fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> RefreshStore {
        RefreshStore::new()
    }

    #[test]
    fn test_issue_and_verify() {
        let store = test_store();
        let issued = store.issue("u-alice", 3600);
        assert!(!issued.token.is_empty());
        assert_eq!(issued.expires_at - chrono::Utc::now().timestamp(), 3600);

        let verified = store.verify(&issued.token);
        assert!(verified.is_some());
        let (id, user_id) = verified.unwrap();
        assert_eq!(id, issued.id);
        assert_eq!(user_id, "u-alice");
    }

    #[test]
    fn test_wrong_token_rejected() {
        let store = test_store();
        assert!(store.verify("bogus-token").is_none());
    }

    #[test]
    fn test_revoked_token_rejected() {
        let store = test_store();
        let issued = store.issue("u-alice", 3600);
        store.revoke(&issued.id);
        assert!(store.verify(&issued.token).is_none());
    }

    #[test]
    fn test_expired_token_rejected() {
        let store = test_store();
        let issued = store.issue("u-alice", -1); // already expired
        assert!(store.verify(&issued.token).is_none());
    }

    #[test]
    fn test_rotate_issues_new_token() {
        let store = test_store();
        let issued = store.issue("u-alice", 3600);

        let rotated = store.rotate(&issued.token, 3600).expect("rotate");
        assert_ne!(rotated.token, issued.token);
        assert_ne!(rotated.id, issued.id);

        // Old token should be revoked
        assert!(store.verify(&issued.token).is_none());
        // New token should be valid
        assert!(store.verify(&rotated.token).is_some());
    }

    #[test]
    fn test_rotate_replay_detection_revokes_all() {
        let store = test_store();
        let t1 = store.issue("u-alice", 3600);
        let t2 = store.issue("u-alice", 3600);

        // Use t1 (first rotation)
        let _r1 = store.rotate(&t1.token, 3600).expect("first rotate");
        // Replay t1 (should trigger family revocation)
        assert!(store.rotate(&t1.token, 3600).is_none());
        // t2 should also be revoked now
        assert!(store.verify(&t2.token).is_none());
    }

    #[test]
    fn test_revoke_all_for_user() {
        let store = test_store();
        let t1 = store.issue("u-alice", 3600);
        let t2 = store.issue("u-alice", 3600);
        let t3 = store.issue("u-bob", 3600);

        let count = store.revoke_all_for_user("u-alice");
        assert_eq!(count, 2); // t1 and t2 revoked
        assert!(store.verify(&t1.token).is_none());
        assert!(store.verify(&t2.token).is_none());
        assert!(store.verify(&t3.token).is_some()); // bob's still valid
    }

    #[test]
    fn test_counters() {
        let store = test_store();
        assert_eq!(store.count(), 0);
        assert_eq!(store.active_count(), 0);

        let _t1 = store.issue("u-alice", 3600);
        assert_eq!(store.count(), 1);
        assert_eq!(store.active_count(), 1);

        let _t2 = store.issue("u-alice", -1); // expired
        assert_eq!(store.count(), 2);
        assert_eq!(store.active_count(), 1); // only t1 is active
    }

    #[test]
    fn test_different_users_have_independent_tokens() {
        let store = test_store();
        let alice = store.issue("u-alice", 3600);
        let bob = store.issue("u-bob", 3600);

        assert_eq!(store.active_count(), 2);

        let (_, alice_id) = store.verify(&alice.token).unwrap();
        let (_, bob_id) = store.verify(&bob.token).unwrap();
        assert_eq!(alice_id, "u-alice");
        assert_eq!(bob_id, "u-bob");
    }

    #[test]
    fn test_hash_is_deterministic() {
        let token = "test-token-value";
        let h1 = hash_token(token);
        let h2 = hash_token(token);
        assert_eq!(h1, h2);
    }
}