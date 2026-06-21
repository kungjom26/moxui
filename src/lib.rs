//! `MoxUI` library crate.
//!
//! Modern, secure Rust-based web UI for Proxmox VE.
//!
//! See `src/main.rs` for the binary entry point and module structure.

#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]
#![forbid(unsafe_code)]

pub mod api;
pub mod audit;
pub mod auth;
pub mod cache;
pub mod config;
pub mod db;
pub mod error;
pub mod observability;
pub mod proxmox;
pub mod security;
pub mod state;
pub mod telemetry;
pub mod ui;

/// Current `MoxUI` version (semver).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Build profile (debug/release).
pub const BUILD_PROFILE: &str = if cfg!(debug_assertions) {
    "debug"
} else {
    "release"
};

/// Git commit hash (set at build time via `MOXUI_GIT_SHA` env var).
pub const GIT_SHA: &str = match option_env!("MOXUI_GIT_SHA") {
    Some(sha) => sha,
    None => "unknown",
};

/// Build timestamp (UTC, set at build time).
pub const BUILD_TIMESTAMP: &str = match option_env!("MOXUI_BUILD_TIMESTAMP") {
    Some(ts) => ts,
    None => "unknown",
};
