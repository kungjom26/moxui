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

/// `POST /api/v1/auth/login` — verify username + password, return a JWT + refresh token.
pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, Response> {
    let Some(user) = state.users.authenticate(&req.username, &req.password) else {
        tracing::info!(username = %req.username, "login failed");
        return Err(unauthorized_response("invalid username or password"));
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
            tracing::error!(error = %e, "JWT encode failed");
            return Err(internal_response("failed to sign token"));
        }
    };

    // Issue a refresh token.
    let refresh = state
        .refresh_store
        .issue(&user.id, DEFAULT_REFRESH_TTL_SECS);

    tracing::info!(username = %user.username, role = %user.role, "login ok");
    Ok(Json(LoginResponse {
        token,
        expires_in: state.config.auth.jwt_lifetime_secs,
        token_type: "Bearer",
        user: UserView::from(user),
        refresh_token: refresh.token,
    }))
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
        };
        let v = UserView::from(&u);
        assert_eq!(v.id, "u1");
        assert_eq!(v.username, "alice");
        assert_eq!(v.role, "admin");
        assert_eq!(v.email, Some("a@x".to_string()));
    }
}