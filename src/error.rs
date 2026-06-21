//! Application-wide error types.
//!
//! All errors propagate as `AppError` or `AppResult<T>`.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Top-level error type for `MoxUI`.
#[derive(Debug, Error)]
pub enum AppError {
    /// Resource not found (HTTP 404).
    #[error("Not found: {0}")]
    NotFound(String),

    /// Invalid request (HTTP 400).
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// Unauthorized (HTTP 401).
    ///
    /// Holds an internal-only reason string. The HTTP response is
    /// always a generic `"Unauthorized"` so we don't leak which auth
    /// gate rejected the caller — the reason is logged server-side
    /// via the `tracing` integration when this becomes a response.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// Forbidden (HTTP 403). Same log-vs-response split as `Unauthorized`.
    #[error("Forbidden: {0}")]
    Forbidden(String),

    /// Conflict (HTTP 409).
    #[error("Conflict: {0}")]
    Conflict(String),

    /// Too many requests (HTTP 429).
    #[error("Too many requests: {0}")]
    TooManyRequests(String),

    /// Proxmox API error.
    #[error("Proxmox error: {0}")]
    Proxmox(String),

    /// Database error.
    #[error("Database error: {0}")]
    Database(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Internal error (HTTP 500).
    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Log the reason for auth failures server-side before we
        // strip it from the response. This is the one place we get
        // to see *why* a token was rejected without leaking that
        // information back to the caller.
        if let AppError::Unauthorized(reason) = &self {
            tracing::debug!(reason = %reason, "auth rejected");
        }
        if let AppError::Forbidden(reason) = &self {
            tracing::debug!(reason = %reason, "auth forbidden");
        }
        let (status, message) = match &self {
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            // Auth: reason stays server-side, caller sees a fixed
            // generic message so probe traffic can't distinguish
            // "token expired" from "token invalid" from "missing".
            AppError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            AppError::Forbidden(_) => (StatusCode::FORBIDDEN, "Forbidden".to_string()),
            AppError::Conflict(_) => (StatusCode::CONFLICT, self.to_string()),
            AppError::TooManyRequests(_) => (StatusCode::TOO_MANY_REQUESTS, self.to_string()),
            AppError::Proxmox(_)
            | AppError::Database(_)
            | AppError::Config(_)
            | AppError::Internal(_) => {
                tracing::error!(error = %self, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

/// Convenient `Result` alias.
pub type AppResult<T> = Result<T, AppError>;

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError::Database(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Internal(format!("JSON error: {err}"))
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::Proxmox(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = AppError::NotFound("VM 103".to_string());
        assert_eq!(err.to_string(), "Not found: VM 103");
    }
}
