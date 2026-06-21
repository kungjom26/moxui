//! `moxui-side` authentication endpoints.
//!
//! - `POST /api/v1/auth/login` — exchange `username` + `password` for a
//!   signed JWT (RS256). Rate-limited (5 req/sec per IP via
//!   `tower_governor`).
//! - `GET  /api/v1/auth/me`   — echo the [`Claims`] from the bearer
//!   token. Requires [`require_auth`] middleware.

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::auth::{AuthContext, Claims, User, UserStore};
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
    /// Bearer token (RS256 JWT). Send back as `Authorization: Bearer <token>`.
    pub token: String,
    /// Token lifetime in seconds (echoed for the client).
    pub expires_in: i64,
    /// Token type (always `Bearer`).
    pub token_type: &'static str,
    /// Logged-in user profile.
    pub user: UserView,
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

impl From<&User> for UserView {
    fn from(u: &User) -> Self {
        Self {
            id: u.id.clone(),
            username: u.username.clone(),
            display_name: u.display_name.clone(),
            email: u.email.clone(),
            role: u.role.to_string(),
        }
    }
}

/// `POST /api/v1/auth/login` — verify username + password, return a JWT.
pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, Response> {
    let Some(user) = state.users.authenticate(&req.username, &req.password) else {
        // Constant-ish: we always run bcrypt verify (even on unknown
        // user) in `authenticate`. So the response time is similar
        // for wrong-user vs wrong-password.
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
    tracing::info!(username = %user.username, role = %user.role, "login ok");
    Ok(Json(LoginResponse {
        token,
        expires_in: state.config.auth.jwt_lifetime_secs,
        token_type: "Bearer",
        user: UserView::from(user),
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
) -> User {
    use crate::auth::password::hash_password;
    use secrecy::SecretString;
    let hash = hash_password(plaintext_password).expect("bcrypt hash");
    User {
        id: id.to_string(),
        username: username.to_string(),
        display_name: username.to_string(),
        email: None,
        password_hash: SecretString::new(hash.into_boxed_str()),
        role,
        enabled: true,
    }
}

// We need a `users()` method on UserStore for the helper above. Provide
// it as a method on the type in user.rs.

impl UserStore {
    /// Iterate over all users (admin views / tests).
    #[cfg(test)]
    pub fn users(&self) -> impl Iterator<Item = &User> {
        self.users.values()
    }

    /// Look up a user by id.
    #[must_use]
    pub fn get_by_id(&self, id: &str) -> Option<&User> {
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
        let u = User {
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
