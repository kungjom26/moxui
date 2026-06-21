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
    #[error("Unauthorized")]
    Unauthorized,

    /// Forbidden (HTTP 403).
    #[error("Forbidden")]
    Forbidden,

    /// Conflict (HTTP 409).
    #[error("Conflict: {0}")]
    Conflict(String),

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
        let (status, message) = match &self {
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "Forbidden".to_string()),
            AppError::Conflict(_) => (StatusCode::CONFLICT, self.to_string()),
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
