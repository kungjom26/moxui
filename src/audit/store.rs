//! SQLite-backed audit log store (rusqlite + r2d2 pool).
//!
//! ## Schema
//!
//! ```sql
//! CREATE TABLE IF NOT EXISTS audit_log (
//!     id          INTEGER PRIMARY KEY AUTOINCREMENT,
//!     ts          INTEGER NOT NULL,        -- unix seconds (UTC)
//!     request_id  TEXT    NOT NULL,        -- UUIDv4
//!     method      TEXT    NOT NULL,
//!     path        TEXT    NOT NULL,
//!     status      INTEGER NOT NULL,
//!     duration_ms INTEGER NOT NULL,
//!     remote_addr TEXT,
//!     user_agent  TEXT,
//!     user_id     TEXT                    -- reserved (NULL until auth lands)
//! );
//! CREATE INDEX IF NOT EXISTS idx_audit_ts         ON audit_log(ts);
//! CREATE INDEX IF NOT EXISTS idx_audit_request_id ON audit_log(request_id);
//! ```
//!
//! All write paths are serialized via a single writer connection — audit
//! volume is low and write throughput is plenty. Reads use pool connections.

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use thiserror::Error;

/// Errors from the [`AuditStore`].
#[derive(Debug, Error)]
pub enum AuditStoreError {
    /// `SQLite` returned an error.
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// Connection-pool error (rusqlite pool).
    #[error("pool error: {0}")]
    Pool(String),
    /// Invalid pagination parameters.
    #[error("invalid pagination: {0}")]
    Pagination(String),
}

/// One row of the `audit_log` table, including the auto-increment `id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntryRow {
    /// Auto-increment primary key.
    pub id: i64,
    /// The core audit entry fields.
    pub entry: AuditEntry,
}

/// Query parameters for [`AuditStore::query`].
#[derive(Debug, Clone, Default)]
pub struct AuditQuery {
    /// Page number (1-indexed, default 1).
    pub page: u64,
    /// Rows per page (default 50, max 500).
    pub per_page: u64,
    /// Optional filter: HTTP method (e.g. `\"POST\"`).
    pub method: Option<String>,
    /// Optional filter: path substring match.
    pub path: Option<String>,
    /// Optional filter: exact HTTP status code.
    pub status: Option<u16>,
    /// Optional filter: minimum `ts` (inclusive, unix seconds).
    pub from_ts: Option<i64>,
    /// Optional filter: maximum `ts` (inclusive, unix seconds).
    pub to_ts: Option<i64>,
    /// Optional filter: exact request id.
    pub request_id: Option<String>,
    /// Sort direction (default `\"DESC\"` so newest first).
    pub sort_dir: SortDir,
}

/// Sort direction for audit queries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SortDir {
    /// Newest first.
    #[default]
    Desc,
    /// Oldest first.
    Asc,
}

/// Paginated result from [`AuditStore::query`].
#[derive(Debug, Clone)]
pub struct AuditQueryResult {
    /// Entries on the current page.
    pub entries: Vec<AuditEntryRow>,
    /// Total number of matching entries across all pages.
    pub total: u64,
    /// Current page number.
    pub page: u64,
    /// Rows per page.
    pub per_page: u64,
    /// Total number of pages.
    pub pages: u64,
}

/// One row of the [`audit_log`](store::ensure_schema) table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    /// Unix timestamp (seconds, UTC).
    pub ts: i64,
    /// `UUIDv4` request id (matches `X-Request-Id` header).
    pub request_id: String,
    /// HTTP method (e.g. `POST`).
    pub method: String,
    /// Request path (no query string).
    pub path: String,
    /// HTTP status code.
    pub status: u16,
    /// Wall-clock duration of the handler in milliseconds.
    pub duration_ms: u64,
    /// Caller IP (from `X-Forwarded-For` / `X-Real-IP`), if present.
    pub remote_addr: Option<String>,
    /// `User-Agent` header value, if present.
    pub user_agent: Option<String>,
    /// Authenticated user id, reserved (always `None` for now).
    pub user_id: Option<String>,
}

/// Audit log store backed by `SQLite` (single-writer).
///
/// Cloning the store is cheap — the inner `Connection` is wrapped in an
/// `Arc<Mutex<...>>`.
#[derive(Clone)]
pub struct AuditStore {
    inner: std::sync::Arc<std::sync::Mutex<Connection>>,
}

impl AuditStore {
    /// Open (or create) a `SQLite` file at `path` and ensure the schema exists.
    ///
    /// # Errors
    ///
    /// Returns [`AuditStoreError::Sqlite`] if the file cannot be opened or the
    /// schema migration fails.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, AuditStoreError> {
        let conn = Connection::open(path)?;
        Self::ensure_schema(&conn)?;
        Ok(Self {
            inner: std::sync::Arc::new(std::sync::Mutex::new(conn)),
        })
    }

    /// Open an in-memory database (used by tests).
    pub fn open_in_memory() -> Result<Self, AuditStoreError> {
        let conn = Connection::open_in_memory()?;
        Self::ensure_schema(&conn)?;
        Ok(Self {
            inner: std::sync::Arc::new(std::sync::Mutex::new(conn)),
        })
    }

    /// Create the [`audit_log`] table + indexes if missing.
    fn ensure_schema(conn: &Connection) -> Result<(), AuditStoreError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS audit_log (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                ts          INTEGER NOT NULL,
                request_id  TEXT    NOT NULL,
                method      TEXT    NOT NULL,
                path        TEXT    NOT NULL,
                status      INTEGER NOT NULL,
                duration_ms INTEGER NOT NULL,
                remote_addr TEXT,
                user_agent  TEXT,
                user_id     TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_audit_ts         ON audit_log(ts);
            CREATE INDEX IF NOT EXISTS idx_audit_request_id ON audit_log(request_id);
            ",
        )?;
        Ok(())
    }

    /// Insert one [`AuditEntry`].
    pub fn log(&self, entry: &AuditEntry) -> Result<(), AuditStoreError> {
            let entry = entry.clone();
            let guard = self.inner.lock().expect("audit store mutex poisoned");
            let duration_ms_i64 = i64::try_from(entry.duration_ms).unwrap_or(i64::MAX);
            guard.execute(
                "INSERT INTO audit_log
                    (ts, request_id, method, path, status, duration_ms,
                     remote_addr, user_agent, user_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    entry.ts,
                    entry.request_id,
                    entry.method,
                    entry.path,
                    entry.status,
                    duration_ms_i64,
                    entry.remote_addr,
                    entry.user_agent,
                    entry.user_id,
                ],
            )?;
            Ok(())
        }

        /// Paginated query with optional filters.
        ///
        /// Returns entries sorted by `ts` (descending by default), with total
        /// count for pagination.
        ///
        /// # Errors
        ///
        /// Returns [`AuditStoreError::Pagination`] if `page` or `per_page` are
        /// zero, or `per_page` exceeds 500.
        /// Returns [`AuditStoreError::Sqlite`] on SQLite errors.
        pub fn query(&self, q: &AuditQuery) -> Result<AuditQueryResult, AuditStoreError> {
            if q.page == 0 {
                return Err(AuditStoreError::Pagination("page must be ≥ 1".into()));
            }
            if q.per_page == 0 || q.per_page > 500 {
                return Err(AuditStoreError::Pagination(
                    "per_page must be between 1 and 500".into(),
                ));
            }

            let guard = self.inner.lock().expect("audit store mutex poisoned");
            let (where_clause, params, next_idx) = Self::build_where(q);
            let order = match q.sort_dir {
                SortDir::Desc => "DESC",
                SortDir::Asc => "ASC",
            };
            let offset = (q.page - 1) * q.per_page;

            // Count total matching rows.
            let count_sql = format!("SELECT COUNT(*) FROM audit_log WHERE {where_clause}");
            let total: i64 = guard.query_row(
                &count_sql,
                rusqlite::params_from_iter(params.iter().map(std::convert::AsRef::as_ref)),
                |row| row.get(0),
            )?;
            let total = u64::try_from(total).unwrap_or(0);

            // Fetch page.
            let data_sql = format!(
                "SELECT id, ts, request_id, method, path, status, duration_ms, \
                        remote_addr, user_agent, user_id \
                 FROM audit_log WHERE {where_clause} \
                 ORDER BY ts {order}, id {order} \
                 LIMIT ?{n1} OFFSET ?{n2}",
                n1 = next_idx,
                n2 = next_idx + 1,
            );

            let mut all_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
            for p in params {
                all_params.push(p);
            }
            all_params.push(Box::new(i64::try_from(q.per_page).unwrap_or(50)));
            all_params.push(Box::new(i64::try_from(offset).unwrap_or(0)));

            let mut stmt = guard.prepare(&data_sql)?;
            let rows = stmt.query_map(
                rusqlite::params_from_iter(all_params.iter().map(std::convert::AsRef::as_ref)),
                Self::map_row,
            )?;

            let entries: Vec<AuditEntryRow> = rows.collect::<Result<_, _>>()?;
            let pages = if total == 0 {
                0
            } else {
                ((total - 1) / q.per_page) + 1
            };

            Ok(AuditQueryResult {
                entries,
                total,
                page: q.page,
                per_page: q.per_page,
                pages,
            })
        }

        /// Build WHERE clause and parameter list for an [`AuditQuery`].
        fn build_where(
            q: &AuditQuery,
        ) -> (
            String,
            Vec<Box<dyn rusqlite::types::ToSql>>,
            usize,
        ) {
            let mut conditions: Vec<String> = vec!["1 = ?1".to_string()];
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
            let mut idx = 1usize;

            params.push(Box::new(1));

            macro_rules! add_param {
                ($col:expr, $val:expr) => {{
                    idx += 1;
                    conditions.push(format!("{} = ?{idx}", $col));
                    params.push($val);
                }};
            }

            if let Some(ref method) = q.method {
                add_param!("method", Box::new(method.clone()));
            }
            if let Some(ref path) = q.path {
                idx += 1;
                conditions.push(format!("INSTR(path, ?{idx}) > 0"));
                params.push(Box::new(path.clone()));
            }
            if let Some(status) = q.status {
                add_param!("status", Box::new(i64::from(status)));
            }
            if let Some(ts) = q.from_ts {
                add_param!("ts", Box::new(ts));
            }
            if let Some(ts) = q.to_ts {
                add_param!("ts", Box::new(ts));
            }
            if let Some(ref rid) = q.request_id {
                add_param!("request_id", Box::new(rid.clone()));
            }

            (conditions.join(" AND "), params, idx + 1)
        }

        /// Map a SQLite row to an [`AuditEntryRow`].
        fn map_row(row: &rusqlite::Row) -> rusqlite::Result<AuditEntryRow> {
            Ok(AuditEntryRow {
                id: row.get(0)?,
                entry: AuditEntry {
                    ts: row.get(1)?,
                    request_id: row.get(2)?,
                    method: row.get(3)?,
                    path: row.get(4)?,
                    status: row.get::<_, u16>(5)?,
                    duration_ms: {
                        let d: i64 = row.get(6)?;
                        u64::try_from(d.max(0)).unwrap_or(0)
                    },
                    remote_addr: row.get(7)?,
                    user_agent: row.get(8)?,
                    user_id: row.get(9)?,
                },
            })
        }

        /// Return the number of rows in the audit log (test helper).
        #[cfg(test)]
        pub fn count(&self) -> Result<usize, AuditStoreError> {
            let guard = self.inner.lock().expect("audit store mutex poisoned");
            let n: i64 = guard.query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))?;
            Ok(usize::try_from(n).unwrap_or(0))
        }

        /// Look up an entry by its request id (returns the first match).
        pub fn find_by_request_id(
        &self,
        request_id: &str,
    ) -> Result<Option<AuditEntry>, AuditStoreError> {
        let guard = self.inner.lock().expect("audit store mutex poisoned");
        let row = guard
            .query_row(
                "SELECT ts, request_id, method, path, status, duration_ms,
                        remote_addr, user_agent, user_id
                 FROM audit_log
                 WHERE request_id = ?1
                 LIMIT 1",
                params![request_id],
                |row| {
                    let duration_ms_i64: i64 = row.get(5)?;
                    Ok(AuditEntry {
                        ts: row.get(0)?,
                        request_id: row.get(1)?,
                        method: row.get(2)?,
                        path: row.get(3)?,
                        status: row.get::<_, u16>(4)?,
                        duration_ms: u64::try_from(duration_ms_i64.max(0)).unwrap_or(0),
                        remote_addr: row.get(6)?,
                        user_agent: row.get(7)?,
                        user_id: row.get(8)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(request_id: &str, method: &str, status: u16) -> AuditEntry {
        AuditEntry {
            ts: 1_700_000_000,
            request_id: request_id.to_string(),
            method: method.to_string(),
            path: "/api/v1/vms".to_string(),
            status,
            duration_ms: 12,
            remote_addr: Some("127.0.0.1".to_string()),
            user_agent: Some("test/1.0".to_string()),
            user_id: None,
        }
    }

    #[test]
    fn test_log_and_find_by_request_id() {
        let store = AuditStore::open_in_memory().unwrap();
        store.log(&entry("req-1", "POST", 201)).unwrap();
        store.log(&entry("req-2", "DELETE", 200)).unwrap();
        assert_eq!(store.count().unwrap(), 2);

        let found = store.find_by_request_id("req-1").unwrap().unwrap();
        assert_eq!(found.method, "POST");
        assert_eq!(found.status, 201);
        assert_eq!(found.request_id, "req-1");
        assert_eq!(found.remote_addr.as_deref(), Some("127.0.0.1"));
    }

    #[test]
    fn test_find_by_request_id_missing_returns_none() {
        let store = AuditStore::open_in_memory().unwrap();
        assert!(store.find_by_request_id("nope").unwrap().is_none());
    }

    #[test]
    fn test_open_creates_file_and_schema() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("audit.db");
        let store = AuditStore::open(&path).unwrap();
        store.log(&entry("req-x", "POST", 200)).unwrap();
        // Re-open to verify the schema was persisted.
        drop(store);
        let store2 = AuditStore::open(&path).unwrap();
        assert_eq!(store2.count().unwrap(), 1);
    }
}
