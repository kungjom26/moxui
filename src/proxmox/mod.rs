//! Proxmox VE API client.
//!
//! Handles ticket-based auth, connection pooling, circuit breaking,
//! and request retries. See `~/.hermes/profiles/moxui-coder/skills/moxui-proxmox-api/SKILL.md`
//! (LOCAL-ONLY) for detailed API reference.

pub mod auth;
pub mod circuit_breaker;
pub mod client;
pub mod retry;
pub mod types;

pub use client::ProxmoxClient;
