//! Audit log API endpoints.
//!
//! - `GET /api/v1/audit` — paginated, filterable audit log

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::audit::{AuditEntryRow, AuditQuery, SortDir};
use crate::state::AppState;

/// Query parameters for `GET /api/v1/audit`.
#[derive(Debug, Deserialize)]
pub struct AuditQueryParams {
    /// Page number (1-indexed, default 1).
    pub page: Option<u64>,
    /// Rows per page (default 50, max 500).
    pub per_page: Option<u64>,
    /// Filter by HTTP method (e.g. `POST`).
    pub method: Option<String>,
    /// Filter by path substring.
    pub path: Option<String>,
    /// Filter by exact HTTP status code.
    pub status: Option<u16>,
    /// Filter by minimum timestamp (unix seconds, inclusive).
    pub from_ts: Option<i64>,
    /// Filter by maximum timestamp (unix seconds, inclusive).
    pub to_ts: Option<i64>,
    /// Filter by exact request id.
    pub request_id: Option<String>,
    /// Sort direction: `asc` or `desc` (default `desc`).
    pub sort: Option<String>,
}

impl From<AuditQueryParams> for AuditQuery {
    fn from(p: AuditQueryParams) -> Self {
        let sort_dir = match p.sort.as_deref() {
            Some("asc") => SortDir::Asc,
            _ => SortDir::Desc,
        };
        AuditQuery {
            page: p.page.unwrap_or(1),
            per_page: p.per_page.unwrap_or(50),
            method: p.method,
            path: p.path,
            status: p.status,
            from_ts: p.from_ts,
            to_ts: p.to_ts,
            request_id: p.request_id,
            sort_dir,
        }
    }
}

/// JSON serialization wrapper for an audit entry row.
#[derive(Debug, Serialize)]
pub struct AuditEntryResponse {
    /// Auto-increment primary key.
    pub id: i64,
    /// Unix timestamp (seconds, UTC).
    pub ts: i64,
    /// UUIDv4 request id (matches `X-Request-Id` header).
    pub request_id: String,
    /// HTTP method (e.g. `POST`).
    pub method: String,
    /// Request path (no query string).
    pub path: String,
    /// HTTP status code.
    pub status: u16,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Caller IP, if present.
    pub remote_addr: Option<String>,
    /// User-Agent header value, if present.
    pub user_agent: Option<String>,
    /// Authenticated user id, if present.
    pub user_id: Option<String>,
}

impl From<AuditEntryRow> for AuditEntryResponse {
    fn from(row: AuditEntryRow) -> Self {
        Self {
            id: row.id,
            ts: row.entry.ts,
            request_id: row.entry.request_id,
            method: row.entry.method,
            path: row.entry.path,
            status: row.entry.status,
            duration_ms: row.entry.duration_ms,
            remote_addr: row.entry.remote_addr,
            user_agent: row.entry.user_agent,
            user_id: row.entry.user_id,
        }
    }
}

/// Response for `GET /api/v1/audit`.
#[derive(Debug, Serialize)]
pub struct AuditResponse {
    /// Entries on the current page.
    pub entries: Vec<AuditEntryResponse>,
    /// Total number of matching entries across all pages.
    pub total: u64,
    /// Current page number.
    pub page: u64,
    /// Rows per page.
    pub per_page: u64,
    /// Total number of pages.
    pub pages: u64,
}

/// `GET /api/v1/audit` — paginated, filterable audit log.
///
/// Requires authentication. Returns the list of audit log entries matching
/// the provided filters, sorted by timestamp descending (newest first) by
/// default.
pub async fn list_audit(
    State(state): State<AppState>,
    Query(params): Query<AuditQueryParams>,
) -> Result<Json<AuditResponse>, Response> {
    let query: AuditQuery = params.into();

    match state.audit.query(&query) {
        Ok(result) => {
            let entries: Vec<AuditEntryResponse> = result
                .entries
                .into_iter()
                .map(AuditEntryResponse::from)
                .collect();
            Ok(Json(AuditResponse {
                entries,
                total: result.total,
                page: result.page,
                per_page: result.per_page,
                pages: result.pages,
            }))
        }
        Err(e) => {
            tracing::error!(error = %e, "audit query failed");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "audit query failed"})),
            )
                .into_response())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::AuditEntry;
    use crate::audit::AuditStore;
    use crate::auth::{jwt::JwtService, user::UserStore};
    use crate::config::{AuthConfig, Config, ServerConfig};
    use axum::{body::Body, http::Request, http::StatusCode, routing::get, Router};
    use secrecy::SecretBox as _;
    use std::sync::Arc;
    use tower::ServiceExt;

    const PRIV_PEM: &str = include_str!("../../tests/fixtures/test_jwt_priv.pem");
    const PUB_PEM: &str = include_str!("../../tests/fixtures/test_jwt_pub.pem");

    fn test_state(audit: Arc<AuditStore>) -> AppState {
        let jwt = JwtService::new(PRIV_PEM.as_bytes(), PUB_PEM.as_bytes(), "test", "test")
            .expect("test keypair");

        let cfg = Config {
            server: ServerConfig {
                bind: "0.0.0.0:8080".to_string(),
                workers: 0,
                tls: None,
            },
            database: crate::config::DatabaseConfig {
                path: ":memory:".to_string(),
                max_connections: 8,
                run_migrations: true,
            },
            logging: crate::config::LoggingConfig {
                level: "info".to_string(),
                format: "pretty".to_string(),
            },
            clusters: vec![],
            auth: AuthConfig::default(),
        };

        AppState::new(cfg, vec![], audit, jwt, UserStore::new(), None, None)
    }

    fn seed_entries(store: &AuditStore, count: usize) {
        for i in 0..count {
            store
                .log(&AuditEntry {
                    ts: 1_700_000_000 + i as i64,
                    request_id: format!("req-{i}"),
                    method: if i % 2 == 0 {
                        "POST".into()
                    } else {
                        "DELETE".into()
                    },
                    path: if i % 3 == 0 {
                        "/api/v1/vms".into()
                    } else {
                        "/api/v1/auth/login".into()
                    },
                    status: if i % 2 == 0 { 201 } else { 200 },
                    duration_ms: (i * 10) as u64,
                    remote_addr: Some("10.0.0.1".into()),
                    user_agent: Some("test-agent".into()),
                    user_id: None,
                })
                .unwrap();
        }
    }

    #[tokio::test]
    async fn test_audit_unauthenticated_returns_401() {
        let audit = Arc::new(AuditStore::open_in_memory().unwrap());
        let state = test_state(audit);

        let app = Router::new()
            .route("/api/v1/audit", get(list_audit))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                crate::auth::require_auth,
            ))
            .with_state(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/audit")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_audit_pagination() {
        let audit = Arc::new(AuditStore::open_in_memory().unwrap());
        seed_entries(&audit, 25);
        let _state = test_state(audit.clone()); // kept for future integration tests

        // Query directly via AuditStore for unit-testy simplicity
        let result = audit
            .query(&AuditQuery {
                page: 1,
                per_page: 10,
                ..Default::default()
            })
            .unwrap();

        assert_eq!(
            result.entries.len(),
            10,
            "first page should have 10 entries"
        );
        assert_eq!(result.total, 25, "total should be 25");
        assert_eq!(result.page, 1);
        assert_eq!(result.per_page, 10);
        assert_eq!(result.pages, 3, "25 entries / 10 per page = 3 pages");

        // Second page
        let result = audit
            .query(&AuditQuery {
                page: 2,
                per_page: 10,
                ..Default::default()
            })
            .unwrap();

        assert_eq!(result.entries.len(), 10);
        assert_eq!(result.page, 2);

        // Third page (last)
        let result = audit
            .query(&AuditQuery {
                page: 3,
                per_page: 10,
                ..Default::default()
            })
            .unwrap();

        assert_eq!(result.entries.len(), 5);
        assert_eq!(result.page, 3);
    }

    #[tokio::test]
    async fn test_audit_filter_by_method() {
        let audit = Arc::new(AuditStore::open_in_memory().unwrap());
        seed_entries(&audit, 10);
        let _state = test_state(audit.clone()); // kept for future integration tests

        let result = audit
            .query(&AuditQuery {
                page: 1,
                per_page: 50,
                method: Some("POST".into()),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(result.total, 5, "half of 10 entries are POST");
        assert!(result.entries.iter().all(|e| e.entry.method == "POST"));
    }

    #[tokio::test]
    async fn test_audit_filter_by_path_substring() {
        let audit = Arc::new(AuditStore::open_in_memory().unwrap());
        seed_entries(&audit, 12);

        let result = audit
            .query(&AuditQuery {
                page: 1,
                per_page: 50,
                path: Some("vms".into()),
                ..Default::default()
            })
            .unwrap();

        // Entries where i % 3 == 0 use "/api/v1/vms" — 0,3,6,9 = 4 entries
        assert_eq!(result.total, 4);
        assert!(result.entries.iter().all(|e| e.entry.path.contains("vms")));
    }

    #[tokio::test]
    async fn test_audit_filter_by_status() {
        let audit = Arc::new(AuditStore::open_in_memory().unwrap());
        seed_entries(&audit, 10);

        let result = audit
            .query(&AuditQuery {
                page: 1,
                per_page: 50,
                status: Some(201),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(result.total, 5);
        assert!(result.entries.iter().all(|e| e.entry.status == 201));
    }

    #[tokio::test]
    async fn test_audit_filter_by_request_id() {
        let audit = Arc::new(AuditStore::open_in_memory().unwrap());
        seed_entries(&audit, 10);

        let result = audit
            .query(&AuditQuery {
                page: 1,
                per_page: 50,
                request_id: Some("req-5".into()),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(result.total, 1);
        assert_eq!(result.entries[0].entry.request_id, "req-5");
    }

    #[tokio::test]
    async fn test_audit_sort_asc() {
        let audit = Arc::new(AuditStore::open_in_memory().unwrap());
        seed_entries(&audit, 5);

        let result = audit
            .query(&AuditQuery {
                page: 1,
                per_page: 50,
                sort_dir: SortDir::Asc,
                ..Default::default()
            })
            .unwrap();

        // By default ascending — ts starts at 1700000000, goes up
        assert_eq!(result.entries.len(), 5);
        for i in 0..4 {
            assert!(
                result.entries[i].entry.ts <= result.entries[i + 1].entry.ts,
                "ts should be ascending"
            );
        }
    }

    #[tokio::test]
    async fn test_audit_empty_store() {
        let audit = Arc::new(AuditStore::open_in_memory().unwrap());

        let result = audit
            .query(&AuditQuery {
                page: 1,
                per_page: 50,
                ..Default::default()
            })
            .unwrap();

        assert_eq!(result.entries.len(), 0);
        assert_eq!(result.total, 0);
        assert_eq!(result.pages, 0);
    }

    #[tokio::test]
    async fn test_audit_invalid_pagination() {
        let audit = Arc::new(AuditStore::open_in_memory().unwrap());

        let result = audit.query(&AuditQuery {
            page: 0,
            per_page: 50,
            ..Default::default()
        });
        assert!(result.is_err(), "page 0 should be rejected");

        let result = audit.query(&AuditQuery {
            page: 1,
            per_page: 0,
            ..Default::default()
        });
        assert!(result.is_err(), "per_page 0 should be rejected");

        let result = audit.query(&AuditQuery {
            page: 1,
            per_page: 501,
            ..Default::default()
        });
        assert!(result.is_err(), "per_page > 500 should be rejected");
    }
}
