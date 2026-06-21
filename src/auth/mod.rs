//! Authentication and authorization.
//!
//! Supports local (bcrypt + JWT), TOTP, `WebAuthn`, OIDC, and LDAP.
//! See `docs/FEATURE_SCOPE.md` §6 for the full feature list.

pub mod jwt;
pub mod password;

pub use jwt::{Claims, JwtService};
pub use password::{hash_password, verify_password};
