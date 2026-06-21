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
        // `duration_ms` is u64 in the struct but SQLite `INTEGER` is i64;
        // values exceeding `i64::MAX` ms (≈ 292 million years) are not
        // representable in practice, but suppress the pedantic warning.
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

    /// Return the number of rows in the audit log (test helper).
    #[cfg(test)]
    pub fn count(&self) -> Result<usize, AuditStoreError> {
        let guard = self.inner.lock().expect("audit store mutex poisoned");
        let n: i64 = guard.query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))?;
        Ok(usize::try_from(n).unwrap_or(0))
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
