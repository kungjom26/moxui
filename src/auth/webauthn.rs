//! WebAuthn / Passkey authentication (Phase 3).
//!
//! Uses the `webauthn-rs` crate to implement FIDO2 WebAuthn (passkeys)
//! as a second factor, complementing the TOTP 2FA implementation.
//!
//! ## Flow
//!
//! **Registration:**
//! 1. User logs in with password (or is already authenticated)
//! 2. `POST /api/v1/auth/webauthn/register/start` → returns challenge
//! 3. Browser calls `navigator.credentials.create()` with the challenge
//! 4. `POST /api/v1/auth/webauthn/register/complete` → stores credential
//!
//! **Authentication:**
//! 1. User logs in with password → if passkey enrolled, returns `passkey_required`
//! 2. Client calls `POST /api/v1/auth/webauthn/login/start` → gets assertion
//! 3. Browser calls `navigator.credentials.get()` with the assertion
//! 4. `POST /api/v1/auth/webauthn/login/complete` → returns JWT

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use url::Url;
use webauthn_rs::prelude::*;

/// In-memory WebAuthn state store.
///
/// ## Thread safety
///
/// Wraps two `RwLock<HashMap>` instances — one for credential state
/// and one for registration challenges. Cheaply cloneable via `Arc`.
#[derive(Clone)]
pub struct WebauthnState {
    /// Per-user WebAuthn credentials (passkeys).
    credentials: Arc<RwLock<HashMap<String, Vec<Passkey>>>>,
    /// Pending registration challenges (keyed by user-id).
    reg_state: Arc<RwLock<HashMap<String, PasskeyRegistration>>>,
    /// Pending authentication challenges (keyed by user-id).
    auth_state: Arc<RwLock<HashMap<String, PasskeyAuthentication>>>,
    /// The `Webauthn` instance (shared, stateless).
    webauthn: Arc<Webauthn>,
}

/// Wrapper error for WebAuthn operations.
#[derive(Debug)]
pub enum WebauthnStoreError {
    /// The underlying webauthn-rs error.
    Webauthn(WebauthnError),
    /// Lock contention or internal state error.
    Internal(String),
}

impl std::fmt::Display for WebauthnStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Webauthn(e) => write!(f, "webauthn: {e}"),
            Self::Internal(s) => write!(f, "internal: {s}"),
        }
    }
}

impl std::error::Error for WebauthnStoreError {}

impl From<WebauthnError> for WebauthnStoreError {
    fn from(e: WebauthnError) -> Self {
        Self::Webauthn(e)
    }
}

impl WebauthnState {
    /// Create a new WebAuthn state from configuration.
    ///
    /// # Errors
    ///
    /// Returns [`WebauthnStoreError`] if `rp_origin` is not a valid URL
    /// or the `WebauthnBuilder` fails.
    pub fn new(rp_id: &str, rp_origin: &str, rp_name: &str) -> Result<Self, WebauthnStoreError> {
        let rp_origin = Url::parse(rp_origin)
            .map_err(|_| WebauthnStoreError::Internal("invalid rp_origin URL".to_string()))?;

        let webauthn = WebauthnBuilder::new(rp_id, &rp_origin)
            .map_err(|e| WebauthnStoreError::Internal(format!("WebauthnBuilder: {e}")))?
            .rp_name(rp_name)
            .build()?;

        Ok(Self {
            credentials: Arc::new(RwLock::new(HashMap::new())),
            reg_state: Arc::new(RwLock::new(HashMap::new())),
            auth_state: Arc::new(RwLock::new(HashMap::new())),
            webauthn: Arc::new(webauthn),
        })
    }

    /// Start passkey registration for a user.
    /// Returns the creation challenge (send to browser) and the server state (store for later).
    pub fn start_registration(
        &self,
        user_id: &str,
        username: &str,
    ) -> Result<(CreationChallengeResponse, PasskeyRegistration), WebauthnStoreError> {
        let exclude: Option<Vec<CredentialID>> = self
            .credentials
            .read()
            .map_err(|e| WebauthnStoreError::Internal(format!("lock: {e}")))?
            .get(user_id)
            .map(|creds| creds.iter().map(|c| c.cred_id().clone()).collect());

        let user_uuid = uuid::Uuid::parse_str(user_id).unwrap_or_else(|_| uuid::Uuid::new_v4());

        let (ccr, reg_state) = self
            .webauthn
            .start_passkey_registration(user_uuid, username, username, exclude)?;

        self.reg_state
            .write()
            .map_err(|e| WebauthnStoreError::Internal(format!("lock: {e}")))?
            .insert(user_id.to_string(), reg_state.clone());

        Ok((ccr, reg_state))
    }

    /// Complete passkey registration.
    /// Validates the browser's response and stores the credential.
    pub fn finish_registration(
        &self,
        user_id: &str,
        rsp: &RegisterPublicKeyCredential,
    ) -> Result<Passkey, WebauthnStoreError> {
        let reg_state = self
            .reg_state
            .write()
            .map_err(|e| WebauthnStoreError::Internal(format!("lock: {e}")))?
            .remove(user_id)
            .ok_or(WebauthnStoreError::Internal(
                "no pending registration for this user".to_string(),
            ))?;

        let passkey = self.webauthn.finish_passkey_registration(rsp, &reg_state)?;

        self.credentials
            .write()
            .map_err(|e| WebauthnStoreError::Internal(format!("lock: {e}")))?
            .entry(user_id.to_string())
            .or_default()
            .push(passkey.clone());

        Ok(passkey)
    }

    /// Check whether a user has any passkeys enrolled.
    pub fn has_credentials(&self, user_id: &str) -> bool {
        self.credentials
            .read()
            .ok()
            .is_some_and(|c| c.get(user_id).is_some_and(|v| !v.is_empty()))
    }

    /// Start passkey authentication for a user.
    /// Returns the assertion challenge (send to browser) and the server state (store for later).
    pub fn start_authentication(
        &self,
        user_id: &str,
    ) -> Result<(RequestChallengeResponse, PasskeyAuthentication), WebauthnStoreError> {
        let creds = self
            .credentials
            .read()
            .map_err(|e| WebauthnStoreError::Internal(format!("lock: {e}")))?
            .get(user_id)
            .cloned()
            .unwrap_or_default();

        if creds.is_empty() {
            return Err(WebauthnStoreError::Internal(
                "no passkeys enrolled for this user".to_string(),
            ));
        }

        let (rcr, auth_state) = self.webauthn.start_passkey_authentication(&creds)?;

        self.auth_state
            .write()
            .map_err(|e| WebauthnStoreError::Internal(format!("lock: {e}")))?
            .insert(user_id.to_string(), auth_state.clone());

        Ok((rcr, auth_state))
    }

    /// Finish passkey authentication.
    /// Validates the assertion and returns the result on success.
    pub fn finish_authentication(
        &self,
        user_id: &str,
        rsp: &PublicKeyCredential,
    ) -> Result<AuthenticationResult, WebauthnStoreError> {
        let auth_state = self
            .auth_state
            .write()
            .map_err(|e| WebauthnStoreError::Internal(format!("lock: {e}")))?
            .remove(user_id)
            .ok_or(WebauthnStoreError::Internal(
                "no pending authentication for this user".to_string(),
            ))?;

        let result = self
            .webauthn
            .finish_passkey_authentication(rsp, &auth_state)?;

        // Update credential counter if auth was successful
        if result.needs_update() {
            let mut creds = self
                .credentials
                .write()
                .map_err(|e| WebauthnStoreError::Internal(format!("lock: {e}")))?
                .remove(user_id)
                .unwrap_or_default();

            for cred in &mut creds {
                if cred.cred_id() == result.cred_id() {
                    cred.update_credential(&result);
                }
            }

            self.credentials
                .write()
                .map_err(|e| WebauthnStoreError::Internal(format!("lock: {e}")))?
                .insert(user_id.to_string(), creds);
        }

        Ok(result)
    }
}

// ── Request / Response types for the API handlers ──────────────────

/// Response for `POST /webauthn/register/start`.
#[derive(Debug, Serialize)]
pub struct RegistrationStartResponse {
    /// The creation challenge to pass to `navigator.credentials.create()`.
    pub challenge: CreationChallengeResponse,
}

/// Request for `POST /webauthn/register/complete`.
#[derive(Debug, Deserialize)]
pub struct RegistrationCompleteRequest {
    /// The credential returned by the browser after `navigator.credentials.create()`.
    pub credential: RegisterPublicKeyCredential,
}

/// Response for `POST /webauthn/register/complete`.
#[derive(Debug, Serialize)]
pub struct RegistrationCompleteResponse {
    /// Always `"ok"` on success.
    pub status: &'static str,
}

/// Response for `POST /webauthn/login/start`.
#[derive(Debug, Serialize)]
pub struct LoginStartResponse {
    /// The assertion challenge to pass to `navigator.credentials.get()`.
    pub challenge: RequestChallengeResponse,
}

/// Request for `POST /webauthn/login/complete`.
#[derive(Debug, Deserialize)]
pub struct LoginCompleteRequest {
    /// The credential returned by the browser after `navigator.credentials.get()`.
    pub credential: PublicKeyCredential,
}

/// Response for `POST /webauthn/login/complete`.
#[derive(Debug, Serialize)]
pub struct LoginCompleteResponse {
    /// Always `"ok"` on success.
    pub status: &'static str,
    /// The authenticated user id.
    pub user_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> WebauthnState {
        WebauthnState::new("localhost", "http://localhost:8080", "MoxUI Test")
            .expect("WebauthnState::new")
    }

    #[test]
    fn test_new_webauthn_state() {
        let state = test_state();
        assert!(!state.has_credentials("nonexistent"));
    }

    #[test]
    fn test_has_credentials_empty() {
        let state = test_state();
        assert!(!state.has_credentials("u1"));
    }

    #[test]
    fn test_start_registration_returns_challenge() {
        let state = test_state();
        let result = state.start_registration("u1", "alice");
        assert!(result.is_ok(), "start_registration should succeed");
        let (ccr, _reg_state) = result.unwrap();
        assert!(
            !ccr.public_key.challenge.is_empty(),
            "challenge should be non-empty"
        );
    }

    #[test]
    fn test_start_authentication_fails_without_credentials() {
        let state = test_state();
        let result = state.start_authentication("u1");
        assert!(result.is_err(), "auth without credentials should fail");
    }

    #[test]
    fn test_webauthn_state_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<WebauthnState>();
        assert_sync::<WebauthnState>();
    }
}
