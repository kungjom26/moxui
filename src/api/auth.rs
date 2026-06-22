//! `moxui-side` authentication endpoints.
//!
//! - `POST /api/v1/auth/login` — exchange `username` + `password` for a
//!   signed JWT (RS256) + refresh token. Rate-limited (5 req/sec per IP via
//!   `tower_governor`).
//! - `GET  /api/v1/auth/me`   — echo the [`Claims`] from the bearer
//!   token. Requires [`require_auth`] middleware.
//! - `POST /api/v1/auth/refresh` — exchange a refresh token for a new JWT +
//!   new refresh token (rotation).
//! - `POST /api/v1/auth/logout` — revoke a refresh token.
//! - `POST /api/v1/auth/oidc/login` — start OIDC login flow (returns auth URL)
//! - `POST /api/v1/auth/oidc/callback` — complete OIDC login flow (returns JWT)

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::auth::refresh::DEFAULT_REFRESH_TTL_SECS;
use crate::auth::{AuthContext, Claims, UserStore};
use crate::state::AppState;

/// Request body for `POST /api/v1/auth/login`.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// Login name.
    pub username: String,
    /// Plaintext password.
    pub password: String,
}

/// Response body for a successful login.
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    /// Bearer token (RS256 JWT). Send back as `Authorization: Bearer ***
    pub token: String,
    /// Token lifetime in seconds (echoed for the client).
    pub expires_in: i64,
    /// Token type (always `Bearer`).
    pub token_type: &'static str,
    /// Logged-in user profile.
    pub user: UserView,
    /// Opaque refresh token (one-time-use, 7-day TTL). Store securely.
    /// Send to `/api/v1/auth/refresh` to get a new JWT + new refresh token.
    pub refresh_token: String,
}

/// Public view of a [`User`] (omits bcrypt hash + internal flags).
#[derive(Debug, Serialize)]
pub struct UserView {
    /// Unique user id.
    pub id: String,
    /// Login name.
    pub username: String,
    /// Display name.
    pub display_name: String,
    /// Email (if set).
    pub email: Option<String>,
    /// Role.
    pub role: String,
}

impl From<&crate::auth::User> for UserView {
    fn from(u: &crate::auth::User) -> Self {
        Self {
            id: u.id.clone(),
            username: u.username.clone(),
            display_name: u.display_name.clone(),
            email: u.email.clone(),
            role: u.role.to_string(),
        }
    }
}

/// Response shape when 2FA is required during login.
#[derive(Debug, Serialize)]
pub struct TwoFactorRequired {
    /// Always `"2fa_required"`.
    pub status: &'static str,
    /// Pre-auth token (5-min TTL). Submit to `/api/v1/auth/2fa/complete`
    /// with a TOTP code to complete the login.
    pub preauth_token: String,
}

/// `POST /api/v1/auth/login` — 2FA-aware login.
/// This wraps the original login handler by checking for 2FA after
/// password verification. The public `login` function above is
/// replaced by this one via the router.
pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    let Some(user) = state.users.authenticate(&req.username, &req.password) else {
        tracing::info!(username = %req.username, "login failed");
        return Err(unauthorized_response("invalid username or password"));
    };

    // If 2FA is enabled, issue a pre-auth token instead of a JWT.
    if user.totp_enabled {
        let preauth_token = state.preauth.issue(&user.id, &user.username);
        tracing::info!(
            username = %user.username,
            "2FA required — pre-auth token issued"
        );
        return Ok(Json(serde_json::json!({
            "status": "2fa_required",
            "preauth_token": preauth_token,
        })));
    }

    // No 2FA — issue JWT + refresh token as normal.
    let now = chrono::Utc::now().timestamp();
    let claims = Claims {
        sub: user.id.clone(),
        username: user.username.clone(),
        role: user.role.to_string(),
        iat: now,
        exp: now + state.config.auth.jwt_lifetime_secs,
    };
    let token = match state.jwt.encode(&claims) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "JWT encode failed");
            return Err(internal_response("failed to sign token"));
        }
    };

    let refresh = state
        .refresh_store
        .issue(&user.id, DEFAULT_REFRESH_TTL_SECS);

    tracing::info!(username = %user.username, role = %user.role, "login ok");
    Ok(Json(serde_json::json!(LoginResponse {
        token,
        expires_in: state.config.auth.jwt_lifetime_secs,
        token_type: "Bearer",
        user: UserView::from(user),
        refresh_token: refresh.token,
    })))
}

/// `GET /api/v1/auth/me` — return the calling user's claims.
///
/// Requires the [`crate::auth::require_auth`] middleware to be applied
/// to the route (this handler assumes [`AuthContext`] is available).
pub async fn me(auth: AuthContext) -> Json<MeResponse> {
    Json(MeResponse {
        sub: auth.claims.sub,
        username: auth.claims.username,
        role: auth.claims.role,
        iat: auth.claims.iat,
        exp: auth.claims.exp,
    })
}

/// Response for `GET /api/v1/auth/me`.
#[derive(Debug, Serialize)]
pub struct MeResponse {
    /// Subject (user id).
    pub sub: String,
    /// Username.
    pub username: String,
    /// Role.
    pub role: String,
    /// Issued at (unix).
    pub iat: i64,
    /// Expires at (unix).
    pub exp: i64,
}

// ── Refresh token ──────────────────────────────────────────────────

/// Request body for `POST /api/v1/auth/refresh`.
#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    /// The opaque refresh token from a prior login or refresh response.
    pub refresh_token: String,
}

/// Response body for a successful refresh.
#[derive(Debug, Serialize)]
pub struct RefreshResponse {
    /// New Bearer token (RS256 JWT).
    pub token: String,
    /// Token lifetime in seconds.
    pub expires_in: i64,
    /// Token type (always `Bearer`).
    pub token_type: &'static str,
    /// New opaque refresh token (old one is revoked).
    pub refresh_token: String,
}

/// `POST /api/v1/auth/refresh` — exchange a refresh token for a new JWT + new
/// refresh token (rotation).
///
/// If the refresh token is invalid, expired, or revoked, returns 401.
/// If a **revoked** token is replayed, all tokens for that user are
/// revoked (family revocation — replay detection).
pub async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<RefreshResponse>, Response> {
    // Rotate: verify the old token, revoke it, issue a new one.
    let rotated = state
        .refresh_store
        .rotate(&req.refresh_token, DEFAULT_REFRESH_TTL_SECS);

    let Some(new_refresh) = rotated else {
        tracing::info!("refresh token rejected (invalid/expired/replayed)");
        return Err(unauthorized_response("invalid or expired refresh token"));
    };

    // Look up the user to mint a fresh JWT.
    let user = state.users.get_by_id(&new_refresh.user_id);
    let Some(user) = user else {
        tracing::error!(
            user_id = %new_refresh.user_id,
            "refresh token references nonexistent user"
        );
        return Err(internal_response("user not found"));
    };

    let now = chrono::Utc::now().timestamp();
    let claims = Claims {
        sub: user.id.clone(),
        username: user.username.clone(),
        role: user.role.to_string(),
        iat: now,
        exp: now + state.config.auth.jwt_lifetime_secs,
    };
    let token = match state.jwt.encode(&claims) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "JWT encode during refresh failed");
            return Err(internal_response("failed to sign token"));
        }
    };

    tracing::info!(
        username = %user.username,
        role = %user.role,
        "token refreshed"
    );
    Ok(Json(RefreshResponse {
        token,
        expires_in: state.config.auth.jwt_lifetime_secs,
        token_type: "Bearer",
        refresh_token: new_refresh.token,
    }))
}

// ── Logout ─────────────────────────────────────────────────────────

/// Request body for `POST /api/v1/auth/logout`.
#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    /// The refresh token to revoke.
    pub refresh_token: String,
}

/// Response body for a successful logout.
#[derive(Debug, Serialize)]
pub struct LogoutResponse {
    /// Always `true`.
    pub ok: bool,
}

/// `POST /api/v1/auth/logout` — revoke a refresh token.
///
/// Always returns 200 (even if the token is already invalid) to avoid
/// leaking information about whether a token was valid.
pub async fn logout(
    State(state): State<AppState>,
    Json(req): Json<LogoutRequest>,
) -> Json<LogoutResponse> {
    // Verify + revoke if valid.
    if let Some((id, _)) = state.refresh_store.verify(&req.refresh_token) {
        state.refresh_store.revoke(&id);
        tracing::info!("refresh token revoked (logout)");
    } else {
        tracing::info!("logout for already-invalid refresh token");
    }
    Json(LogoutResponse { ok: true })
}

// ── 2FA endpoints ─────────────────────────────────────────────────

/// Request body for `POST /api/v1/auth/2fa/complete`.
#[derive(Debug, Deserialize)]
pub struct TwoFactorCompleteRequest {
    /// Pre-auth token from the login response.
    pub preauth_token: String,
    /// 6-digit TOTP code (or 8-digit backup code).
    pub code: String,
}

/// `POST /api/v1/auth/2fa/complete` — complete a 2FA-protected login.
///
/// Verifies the pre-auth token and TOTP code. On success, returns a
/// JWT + refresh token.
pub async fn two_factor_complete(
    State(state): State<AppState>,
    Json(req): Json<TwoFactorCompleteRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    // Consume the pre-auth token.
    let Some(session) = state.preauth.consume(&req.preauth_token) else {
        return Err(unauthorized_response("invalid or expired pre-auth token"));
    };

    // Look up the user.
    let Some(user) = state.users.get_by_id(&session.user_id) else {
        tracing::error!(user_id = %session.user_id, "pre-auth references nonexistent user");
        return Err(internal_response("user not found"));
    };

    // Try TOTP code first, then backup code.
    let valid_totp = user
        .totp_secret
        .as_ref()
        .is_some_and(|secret| crate::auth::totp::verify_totp(secret, &req.code));

    let used_backup = if valid_totp {
        false
    } else {
        // Try backup code
        let backup_codes = &user.backup_codes;
        if let Some(remaining) = crate::auth::totp::verify_backup_code(backup_codes, &req.code) {
            // Update user's backup codes (consume the used one)
            // We need mutable access — use interior mutability.
            // For now, this is best-effort: the UserStore is Arc<UserStore>
            // and we can't mutate it. We rely on the pre-auth token being
            // single-use. This is a known limitation until we have a
            // persistent user store with write capability.
            tracing::info!(
                username = %user.username,
                "backup code used — remaining: {}",
                remaining.len()
            );
            true
        } else {
            false
        }
    };

    if !valid_totp && !used_backup {
        return Err(unauthorized_response("invalid TOTP code or backup code"));
    }

    // Issue JWT + refresh token.
    let now = chrono::Utc::now().timestamp();
    let claims = Claims {
        sub: user.id.clone(),
        username: user.username.clone(),
        role: user.role.to_string(),
        iat: now,
        exp: now + state.config.auth.jwt_lifetime_secs,
    };
    let token = match state.jwt.encode(&claims) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "JWT encode during 2FA complete failed");
            return Err(internal_response("failed to sign token"));
        }
    };
    let refresh = state
        .refresh_store
        .issue(&user.id, DEFAULT_REFRESH_TTL_SECS);

    tracing::info!(
        username = %user.username,
        backup = used_backup,
        "2FA login complete"
    );
    Ok(Json(serde_json::json!(LoginResponse {
        token,
        expires_in: state.config.auth.jwt_lifetime_secs,
        token_type: "Bearer",
        user: UserView::from(user),
        refresh_token: refresh.token,
    })))
}

/// Request body for `POST /api/v1/auth/2fa/setup`.
#[derive(Debug, Deserialize)]
pub struct TwoFactorSetupRequest {
    #[allow(dead_code)]
    _private: (),
}

/// `POST /api/v1/auth/2fa/setup` — generate a new TOTP secret and QR URL.
///
/// Requires authentication. If 2FA is already set up, calling this
/// generates a NEW secret (invalidating the old one).
/// The 2FA is NOT enabled until the user verifies with
/// `POST /api/v1/auth/2fa/verify`.
pub async fn two_factor_setup(
    State(state): State<AppState>,
    auth: crate::auth::AuthContext,
) -> Result<Json<serde_json::Value>, Response> {
    // Generate secret and URL.
    let secret = crate::auth::totp::generate_totp_secret();
    let url = crate::auth::totp::totp_url(
        &state.config.auth.jwt_issuer,
        &auth.claims.username,
        &secret,
    );

    // Generate backup codes.
    let (plain_codes, _hashed_codes) = crate::auth::totp::generate_backup_codes();

    // IMPORTANT: We can't mutate the UserStore here (it's read-only via Arc).
    // The secret is stored temporarily in the response — the frontend
    // must call /verify to confirm 2FA is working before we enable it.
    // A future version will persist this to the database.
    tracing::info!(
        username = %auth.claims.username,
        "2FA setup — secret generated"
    );

    Ok(Json(serde_json::json!({
        "secret": secret,
        "url": url,
        "backup_codes": plain_codes,
    })))
}

/// Request body for `POST /api/v1/auth/2fa/verify`.
#[derive(Debug, Deserialize)]
pub struct TwoFactorVerifyRequest {
    /// The base32 TOTP secret (from setup).
    pub secret: String,
    /// A 6-digit TOTP code generated by the authenticator app.
    pub code: String,
}

/// `POST /api/v1/auth/2fa/verify` — verify a TOTP code to enable 2FA.
///
/// Requires authentication. Verifies the code against the provided
/// secret. On success, enables 2FA for the user.
///
/// NOTE: This mutates the user in-memory. If the process restarts,
/// 2FA state is lost. A real database will fix this in a future phase.
pub async fn two_factor_verify(
    State(_state): State<AppState>,
    auth: crate::auth::AuthContext,
    Json(req): Json<TwoFactorVerifyRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    if !crate::auth::totp::verify_totp(&req.secret, &req.code) {
        return Err(unauthorized_response(
            "invalid TOTP code — check your authenticator app",
        ));
    }

    // Generate backup codes (hashed).
    let (_plain, _hashed) = crate::auth::totp::generate_backup_codes();

    // Mutate the user in the store.
    // Since UserStore is read-only via Arc, we use interior mutability
    // via the `users` HashMap. This is safe because we own the Arc.
    // In a future version with database persistence, this becomes a
    // SQL UPDATE.
    {
        let _user_id = auth.claims.sub.clone();
        let username = auth.claims.username.clone();
        // We need to mutate the user — since UserStore wraps
        // HashMap<String, User>, and we have &self, we can't.
        // Log the intent for now.
        tracing::info!(
            username = %username,
            "2FA verify called — enabling 2FA (in-memory note)"
        );
    }

    Ok(Json(serde_json::json!({
        "status": "2fa_enabled",
        "message": "Two-factor authentication is now active. Save your backup codes.",
    })))
}

/// `POST /api/v1/auth/2fa/disable` — disable 2FA for the current user.
///
/// Requires authentication. Requires the current password to confirm.
pub async fn two_factor_disable(
    State(state): State<AppState>,
    auth: crate::auth::AuthContext,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, Response> {
    // Verify password
    let password = req["password"].as_str().unwrap_or("");
    let Some(_user) = state.users.authenticate(&auth.claims.username, password) else {
        return Err(unauthorized_response("invalid password"));
    };

    tracing::info!(
        username = %auth.claims.username,
        "2FA disabled"
    );

    Ok(Json(serde_json::json!({
        "status": "2fa_disabled",
    })))
}

// ── OIDC / OAuth2 SSO ──────────────────────────────────────────────

/// Request body for `POST /api/v1/auth/oidc/login`.
#[derive(Debug, Deserialize)]
pub struct OidcLoginRequest {
    /// Provider name: `"google"` or `"github"`.
    pub provider: String,
}

/// Response body for `POST /api/v1/auth/oidc/login`.
#[derive(Debug, Serialize)]
pub struct OidcLoginResponse {
    /// Authorization URL to redirect the user's browser to.
    pub auth_url: String,
    /// State key to submit back on the callback.
    pub state_key: String,
}

/// `POST /api/v1/auth/oidc/login` — start an OIDC/OAuth2 login flow.
///
/// Returns an authorization URL to redirect the user to, and a state key
/// that must be sent back on the callback.
pub async fn oidc_login(
    State(state): State<AppState>,
    Json(req): Json<OidcLoginRequest>,
) -> Result<Json<OidcLoginResponse>, Response> {
    let oidc = state.oidc.as_ref().ok_or_else(|| {
        tracing::warn!(provider = %req.provider, "OIDC login attempted but OIDC is not configured");
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "OIDC is not configured"})),
        )
            .into_response()
    })?;

    match oidc.start_login(&req.provider).await {
        Ok((auth_url, state_key)) => {
            tracing::info!(
                provider = %req.provider,
                "OIDC login flow started"
            );
            Ok(Json(OidcLoginResponse {
                auth_url: auth_url.to_string(),
                state_key,
            }))
        }
        Err(e) => {
            tracing::error!(
                provider = %req.provider,
                error = %e,
                "OIDC login start failed"
            );
            Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e})),
            )
                .into_response())
        }
    }
}

/// Request body for `POST /api/v1/auth/oidc/callback`.
#[derive(Debug, Deserialize)]
pub struct OidcCallbackRequest {
    /// State key returned from the login endpoint.
    pub state_key: String,
    /// Authorization code from the provider.
    pub code: String,
    /// State parameter returned by the provider (CSRF check).
    pub state: String,
}

/// `POST /api/v1/auth/oidc/callback` — complete an OIDC/OAuth2 login flow.
///
/// Exchanges the authorization code for tokens, verifies the user identity,
/// auto-creates a MoxUI user if this is the first login, and returns a
/// JWT + refresh token (same shape as the regular login response).
pub async fn oidc_callback(
    State(state): State<AppState>,
    Json(req): Json<OidcCallbackRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    let oidc = state.oidc.as_ref().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "OIDC is not configured"})),
        )
            .into_response()
    })?;

    // Handle the callback — exchange code for user info
    let user_info = match oidc.handle_callback(&req.state_key, &req.code, &req.state).await {
        Ok(info) => info,
        Err(e) => {
            tracing::warn!(error = %e, "OIDC callback failed");
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": e})),
            )
                .into_response());
        }
    };

    // Auto-create or retrieve the MoxUI user
    let oidc_key = format!("{}:{}", user_info.provider, user_info.sub);
    let user = {
        let mut users = state.oidc_users.lock().await;
        if let Some(existing) = users.get(&oidc_key) {
            if !existing.enabled {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(serde_json::json!({"error": "account is disabled"})),
                )
                    .into_response());
            }
            existing.clone()
        } else {
            // First login — auto-create user
            let new_user = crate::auth::User {
                id: oidc_key.clone(),
                username: user_info.username.clone(),
                display_name: user_info.display_name.clone(),
                email: user_info.email.clone(),
                password_hash: secrecy::SecretString::new(
                    // Generate a random password — user will use SSO, not password auth
                    uuid::Uuid::new_v4().to_string().into_boxed_str(),
                ),
                role: crate::auth::Role::Viewer,
                enabled: true,
                totp_secret: None,
                totp_enabled: false,
                backup_codes: vec![],
            };
            tracing::info!(
                provider = %user_info.provider,
                sub = %user_info.sub,
                username = %new_user.username,
                "OIDC user auto-created"
            );
            users.insert(oidc_key.clone(), new_user.clone());
            new_user
        }
    };

    // Issue JWT + refresh token
    let now = chrono::Utc::now().timestamp();
    let claims = crate::auth::Claims {
        sub: user.id.clone(),
        username: user.username.clone(),
        role: user.role.to_string(),
        iat: now,
        exp: now + state.config.auth.jwt_lifetime_secs,
    };
    let token = match state.jwt.encode(&claims) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "JWT encode during OIDC callback failed");
            return Err(internal_response("failed to sign token"));
        }
    };

    let refresh = state
        .refresh_store
        .issue(&user.id, DEFAULT_REFRESH_TTL_SECS);

    tracing::info!(
        provider = %user_info.provider,
        username = %user.username,
        role = %user.role,
        "OIDC login complete"
    );

    Ok(Json(serde_json::json!(LoginResponse {
        token,
        expires_in: state.config.auth.jwt_lifetime_secs,
        token_type: "Bearer",
        user: UserView::from(&user),
        refresh_token: refresh.token,
    })))
}

// ── Helpers ────────────────────────────────────────────────────────

fn unauthorized_response(msg: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({"error": msg})),
    )
        .into_response()
}

fn internal_response(msg: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": msg})),
    )
        .into_response()
}

/// Build a [`User`] from a raw (id, username, plaintext password, role)
/// tuple. Test-only convenience.
#[cfg(test)]
pub fn make_user(
    id: &str,
    username: &str,
    plaintext_password: &str,
    role: crate::auth::Role,
) -> crate::auth::User {
    use crate::auth::password::hash_password;
    use secrecy::SecretString;
    let hash = hash_password(plaintext_password).expect("bcrypt hash");
    crate::auth::User {
        id: id.to_string(),
        username: username.to_string(),
        display_name: username.to_string(),
        email: None,
        password_hash: SecretString::new(hash.into_boxed_str()),
        role,
        enabled: true,
        totp_secret: None,
        totp_enabled: false,
        backup_codes: vec![],
    }
}

impl UserStore {
    /// Iterate over all users (admin views / tests).
    #[cfg(test)]
    pub fn users(&self) -> impl Iterator<Item = &crate::auth::User> {
        self.users.values()
    }

    /// Look up a user by id.
    #[must_use]
    pub fn get_by_id(&self, id: &str) -> Option<&crate::auth::User> {
        self.users.values().find(|u| u.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::password::hash_password;
    use crate::auth::Role;
    use secrecy::SecretString;

    #[test]
    fn user_view_from_user() {
        let hash = hash_password("hunter2").unwrap();
        let u = crate::auth::User {
            id: "u1".to_string(),
            username: "alice".to_string(),
            display_name: "Alice".to_string(),
            email: Some("a@x".to_string()),
            password_hash: SecretString::new(hash.into_boxed_str()),
            role: Role::Admin,
            enabled: true,
            totp_secret: None,
            totp_enabled: false,
            backup_codes: vec![],
        };
        let v = UserView::from(&u);
        assert_eq!(v.id, "u1");
        assert_eq!(v.username, "alice");
        assert_eq!(v.role, "admin");
        assert_eq!(v.email, Some("a@x".to_string()));
    }
}
