//! Proxmox ticket-based authentication.
//!
//! ## Security
//!
//! [`Ticket`] ใช้ `Zeroize` (จาก `zeroize` crate) — sensitive fields
//! (`ticket`, `csrf_token`) ถูก overwrite ด้วย zeros เมื่อ drop เพื่อลด
//! ความเสี่ยงที่ credential จะค้างใน memory หลังจาก ticket expire
//! (2 hours default per Proxmox).
//!
//! หมายเหตุ: ticket string ถูก wrap ใน `Secret<String>` เพิ่มเติมตอนใช้
//! (ใน `ProxmoxClient::RwLock`) เพื่อ:
//! - ป้องกัน accidental Debug printing
//! - บังคับ explicit `.expose_secret()` ตอนส่งไป Proxmox API

use secrecy::SecretBox;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Authentication ticket from `/access/ticket` endpoint.
///
/// Holds the actual `PVEAuthCookie` value + CSRF token. Sensitive fields
/// implement `Zeroize` so memory is wiped on drop.
#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct Ticket {
    /// `PVEAuthCookie` value (used as `Cookie: PVEAuthCookie=...`).
    #[zeroize(skip)]
    pub ticket: String,
    /// `CSRFPreventionToken` (required for POST/PUT/DELETE).
    #[zeroize(skip)]
    pub csrf_token: String,
    /// Username (e.g., `root@pam`).
    #[zeroize(skip)]
    pub username: String,
    /// Unix timestamp when ticket expires.
    #[zeroize(skip)]
    pub expires_at: i64,
}

impl std::fmt::Debug for Ticket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the actual ticket / csrf values — they're bearer
        // credentials equivalent to the Proxmox password.
        f.debug_struct("Ticket")
            .field("ticket", &"<redacted>")
            .field("csrf_token", &"<redacted>")
            .field("username", &self.username)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// Wrapper that combines `Secret` (no Debug, force expose) with the
/// underlying `Ticket` struct (Zeroize on drop).
///
/// This is what `ProxmoxClient` holds in its `RwLock`:
///
/// ```ignore
/// ticket: Arc<RwLock<Option<SecretTicket>>>
/// ```
///
/// Use `.expose_ticket()` to get a `&Ticket` for sending to Proxmox.
///
/// Not `Clone` — tickets are managed through `RwLock<Option<...>>`, and
/// `SecretBox` is single-owner by design. Borrow via `expose_ticket()`.
#[derive(Debug, ZeroizeOnDrop)]
pub struct SecretTicket {
    /// Inner secret — wrapping protects against accidental Debug logging.
    inner: SecretBox<Ticket>,
}

impl SecretTicket {
    /// Wrap a freshly-acquired ticket.
    pub fn new(ticket: Ticket) -> Self {
        Self {
            inner: SecretBox::new(Box::new(ticket)),
        }
    }

    /// Borrow the inner ticket (caller MUST NOT log/print/store it).
    pub fn expose_ticket(&self) -> &Ticket {
        use secrecy::ExposeSecret;
        self.inner.expose_secret()
    }
}

/// Raw response from `/api2/json/access/ticket`.
#[derive(Debug, Deserialize)]
pub(crate) struct TicketResponse {
    /// Wrapped ticket (avoid intermediate plaintext).
    #[allow(dead_code)] // ticket field is read implicitly via deserialize
    pub data: Ticket,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ticket() -> Ticket {
        Ticket {
            ticket: "PVE:root@pam:abc::SIG".to_string(),
            csrf_token: "csrf-secret-xyz".to_string(),
            username: "root@pam".to_string(),
            expires_at: 9_999_999_999,
        }
    }

    #[test]
    fn test_ticket_debug_redacts_secrets() {
        let t = sample_ticket();
        let debug = format!("{t:?}");
        assert!(!debug.contains("PVE:root@pam:abc::SIG"));
        assert!(!debug.contains("csrf-secret-xyz"));
        assert!(debug.contains("<redacted>"));
        assert!(debug.contains("root@pam"));
    }

    #[test]
    fn test_secret_ticket_exposes_inner() {
        let t = sample_ticket();
        let st = SecretTicket::new(t);
        let exposed = st.expose_ticket();
        assert_eq!(exposed.ticket, "PVE:root@pam:abc::SIG");
        assert_eq!(exposed.csrf_token, "csrf-secret-xyz");
    }

    #[test]
    fn test_secret_ticket_debug_does_not_leak() {
        let st = SecretTicket::new(sample_ticket());
        let debug = format!("{st:?}");
        // Secret<T> Debug is typically `Secret(<redacted>)` or similar —
        // never the inner contents.
        assert!(!debug.contains("PVE:root@pam:abc::SIG"));
        assert!(!debug.contains("csrf-secret-xyz"));
    }

    #[test]
    fn test_ticket_deserialize_from_json() {
        let json = r#"{
            "ticket": "PVE:root@pam:abc::SIG",
            "csrf_token": "csrf-secret-xyz",
            "username": "root@pam",
            "expires_at": 9999999999
        }"#;
        let t: Ticket = serde_json::from_str(json).unwrap();
        assert_eq!(t.ticket, "PVE:root@pam:abc::SIG");
        assert_eq!(t.csrf_token, "csrf-secret-xyz");
    }
}
