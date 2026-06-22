//! Authentication and authorization.
//!
//! Supports local (bcrypt + JWT), TOTP, `WebAuthn`, OIDC, and LDAP.
//! See `docs/FEATURE_SCOPE.md` §6 for the full feature list.
//!
//! ## Phase 0 (Day 4)
//!
//! - [`user::User`] / [`user::Role`] — in-memory user accounts with RBAC
//! - [`user::UserStore`] — authenticate username + password (bcrypt verify)
//! - [`jwt::JwtService`] — encode/decode RS256 JWTs
//! - [`middleware::require_auth`] — axum middleware that validates the
//!   `Authorization: Bearer *** header and inserts [`Claims`] into the
//!   request extensions for downstream handlers.

pub mod jwt;
pub mod middleware;
pub mod oidc;
pub mod password;
pub mod refresh;
pub mod totp;
pub mod user;
pub mod vnc;
pub mod webauthn;

pub use jwt::{Claims, JwtService};
pub use middleware::{require_auth, require_cluster_access, require_role, AuthContext};
pub use password::{hash_password, verify_password};
pub use refresh::RefreshStore;
pub use totp::PreAuthStore;
pub use user::{Role, User, UserStore};
