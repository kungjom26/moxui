//! Audit log subsystem.
//!
//! Persists every state-changing HTTP request to a `SQLite` table
//! (`audit_log`). Read-only requests (`GET`/`HEAD`/`OPTIONS`) are NOT
//! logged — they can produce huge volumes without security value. Only
//! methods that change server state (`POST`/`PUT`/`PATCH`/`DELETE`) are
//! recorded.
//!
//! ## Schema
//!
//! See [`store::ensure_schema`]. Key columns:
//! - `request_id` (`UUIDv4`) — correlates all log lines for one request
//! - `ts` — unix seconds (UTC)
//! - `method`/`path`/`status` — request summary
//! - `duration_ms` — wall-clock latency
//! - `remote_addr`/`user_agent` — caller details (may be `None`)
//! - `user_id` — reserved for the upcoming auth middleware (always `None` now)

pub mod middleware;
pub mod store;

pub use middleware::audit_middleware;
pub use store::{
    AuditEntry, AuditEntryRow, AuditQuery, AuditQueryResult, AuditStore, AuditStoreError,
    SortDir,
};
