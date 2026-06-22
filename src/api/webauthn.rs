//! WebAuthn / Passkey API handlers.
//!
//! Routes (all under `/api/v1/auth/webauthn/`):
//!
//! **Registration** (auth required):
//! - `POST register/start`   — returns a passkey creation challenge
//! - `POST register/complete` — stores the registered credential
//!
//! **Authentication** (public):
//! - `POST login/start`      — returns an assertion challenge
//! - `POST login/complete`   — verifies assertion, issues JWT

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use crate::auth::webauthn::{
    LoginStartResponse, RegistrationCompleteRequest, RegistrationCompleteResponse,
    RegistrationStartResponse,
};
use crate::auth::{AuthContext, Claims};
use crate::state::AppState;

// ── Request bodies ─────────────────────────────────────────────────

/// Request body for `POST /api/v1/auth/webauthn/login/start`.
#[derive(Debug, Deserialize)]
pub struct LoginStartRequest {
    /// The username to start a passkey login for.
    pub username: String,
}

/// Request body for `POST /api/v1/auth/webauthn/login/complete`.
#[derive(Debug, Deserialize)]
pub struct LoginCompleteBody {
    /// The username authenticating with a passkey.
    pub username: String,
    /// The credential returned by the browser after
    /// `navigator.credentials.get()`.
    pub credential: webauthn_rs::prelude::PublicKeyCredential,
}

// ── Registration handlers ───────────────────────────────────────────

/// `POST /api/v1/auth/webauthn/register/start`
///
/// Starts passkey registration for the currently-authenticated user.
/// Returns a creation challenge that the browser passes to
/// `navigator.credentials.create()`.
pub async fn register_start(
    State(state): State<AppState>,
    auth: AuthContext,
) -> Result<Json<RegistrationStartResponse>, Response> {
    let webauthn = state.webauthn.as_ref().ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            "WebAuthn is not configured on this server",
        )
            .into_response()
    })?;

    let (challenge, _reg_state) = webauthn
        .start_registration(&auth.claims.sub, &auth.claims.username)
        .map_err(|e| {
            tracing::error!(error = %e, "webauthn register start failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("WebAuthn registration start failed: {e}"),
            )
                .into_response()
        })?;

    tracing::info!(
        username = %auth.claims.username,
        "webauthn registration started"
    );

    Ok(Json(RegistrationStartResponse { challenge }))
}

/// `POST /api/v1/auth/webauthn/register/complete`
///
/// Completes passkey registration. Validates the browser's credential
/// response and stores the new passkey for the authenticated user.
pub async fn register_complete(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<RegistrationCompleteRequest>,
) -> Result<Json<RegistrationCompleteResponse>, Response> {
    let webauthn = state.webauthn.as_ref().ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            "WebAuthn is not configured on this server",
        )
            .into_response()
    })?;

    webauthn
        .finish_registration(&auth.claims.sub, &req.credential)
        .map_err(|e| {
            tracing::error!(error = %e, "webauthn register complete failed");
            (
                StatusCode::BAD_REQUEST,
                format!("WebAuthn registration failed: {e}"),
            )
                .into_response()
        })?;

    tracing::info!(
        username = %auth.claims.username,
        "webauthn registration complete"
    );

    Ok(Json(RegistrationCompleteResponse { status: "ok" }))
}

// ── Login handlers ─────────────────────────────────────────────────

/// `POST /api/v1/auth/webauthn/login/start`
///
/// Starts passkey authentication for the given username. Returns an
/// assertion challenge that the browser passes to
/// `navigator.credentials.get()`.
pub async fn login_start(
    State(state): State<AppState>,
    Json(req): Json<LoginStartRequest>,
) -> Result<Json<LoginStartResponse>, Response> {
    let webauthn = state.webauthn.as_ref().ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            "WebAuthn is not configured on this server",
        )
            .into_response()
    })?;

    // Look up the user by username to get their id.
    let users = state.users.read().await;
    let user = users.get(&req.username).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("User '{}' not found", req.username),
        )
            .into_response()
    })?;

    let (challenge, _auth_state) = webauthn.start_authentication(&user.id).map_err(|e| {
        tracing::error!(error = %e, username = %req.username, "webauthn login start failed");
        (
            StatusCode::BAD_REQUEST,
            format!("WebAuthn login start failed: {e}"),
        )
            .into_response()
    })?;

    tracing::info!(username = %req.username, "webauthn login started");

    Ok(Json(LoginStartResponse { challenge }))
}

/// `POST /api/v1/auth/webauthn/login/complete`
///
/// Completes passkey authentication. Verifies the browser's assertion,
/// then issues a JWT + refresh token (identical to a successful
/// password login).
pub async fn login_complete(
    State(state): State<AppState>,
    Json(req): Json<LoginCompleteBody>,
) -> Result<Json<serde_json::Value>, Response> {
    let webauthn = state.webauthn.as_ref().ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            "WebAuthn is not configured on this server",
        )
            .into_response()
    })?;

    // Look up the user by username.
    let users = state.users.read().await;
    let user = users.get(&req.username).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("User '{}' not found", req.username),
        )
            .into_response()
    })?;

    // Verify the passkey assertion.
    webauthn
        .finish_authentication(&user.id, &req.credential)
        .map_err(|e| {
            tracing::error!(
                error = %e,
                username = %req.username,
                "webauthn login complete failed"
            );
            (
                StatusCode::BAD_REQUEST,
                format!("WebAuthn authentication failed: {e}"),
            )
                .into_response()
        })?;

    // Issue JWT + refresh token (same flow as password login).
    let now = chrono::Utc::now().timestamp();
    let claims = Claims {
        sub: user.id.clone(),
        username: user.username.clone(),
        role: user.role.to_string(),
        iat: now,
        exp: now + state.config.auth.jwt_lifetime_secs,
    };
    let token = state.jwt.encode(&claims).map_err(|e| {
        tracing::error!(error = %e, "JWT encode failed during webauthn login");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to sign token",
        )
            .into_response()
    })?;

    let refresh = state
        .refresh_store
        .issue(&user.id, crate::auth::refresh::DEFAULT_REFRESH_TTL_SECS);

    tracing::info!(
        username = %user.username,
        role = %user.role,
        "webauthn login ok"
    );

    Ok(Json(serde_json::json!({
        "token": token,
        "expires_in": state.config.auth.jwt_lifetime_secs,
        "token_type": "Bearer",
        "user": {
            "id": user.id,
            "username": user.username,
            "display_name": user.display_name,
            "email": user.email,
            "role": user.role.to_string(),
        },
        "refresh_token": refresh.token,
    })))
}
