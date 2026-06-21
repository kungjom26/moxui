//! Proxmox ticket-based authentication.

use serde::{Deserialize, Serialize};

/// Authentication ticket from `/access/ticket` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticket {
    /// `PVEAuthCookie` value (used as `Cookie: PVEAuthCookie=...`).
    pub ticket: String,
    /// `CSRFPreventionToken` (required for POST/PUT/DELETE).
    pub csrf_token: String,
    /// Username (e.g., `root@pam`).
    pub username: String,
    /// Unix timestamp when ticket expires.
    pub expires_at: i64,
}

/// Raw response from `/api2/json/access/ticket`.
#[derive(Debug, Deserialize)]
pub(crate) struct TicketResponse {
    pub data: Ticket,
}
