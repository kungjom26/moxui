//! User accounts and role-based access control (RBAC).
//!
//! For v1.0 the user store is in-memory and seeded from `config.yaml` at
//! startup. Persistent storage (`SQLite` / external IdP) is on the roadmap
//! but out of scope for Phase 0.
//!
//! ## Roles
//!
//! - `Admin`     — full access (cluster ops, user mgmt, settings)
//! - `Operator`  — can run VM actions (start/stop/reboot) on assigned clusters
//! - `Viewer`    — read-only (list/detail VM, `/health`, `/readyz`)
//!
//! Roles are hierarchical: `Admin` > `Operator` > `Viewer`.
//! Use [`Role::can`] to check if a role satisfies a required role.

use std::collections::HashMap;

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

use super::password::{hash_password, verify_password};

/// A `MoxUI` user account (in-memory record).
///
/// Password is **always** stored as a bcrypt hash — never plaintext.
/// On login we [`verify_password`] against the stored hash.
#[derive(Debug, Clone)]
pub struct User {
    /// Unique user id (also used as JWT `sub` claim).
    pub id: String,
    /// Login name (must be unique, case-sensitive).
    pub username: String,
    /// Display name (free-form, e.g. "MoxUI Admin" or "Ops Team Lead").
    pub display_name: String,
    /// Email (optional, not used for auth).
    pub email: Option<String>,
    /// Bcrypt hash of the password (output of [`super::hash_password`]).
    /// Stored as a `SecretString` so it can't be accidentally logged via Debug.
    pub password_hash: SecretString,
    /// Role used for RBAC.
    pub role: Role,
    /// Is this account enabled? Disabled accounts cannot log in (returns 403).
    pub enabled: bool,
    /// Base32-encoded TOTP secret (set during 2FA setup). `None` = 2FA not configured.
    pub totp_secret: Option<String>,
    /// Whether 2FA (TOTP) is fully enabled for this account.
    /// Set to `true` after the user verifies their first TOTP code.
    pub totp_enabled: bool,
    /// Bcrypt-hashed backup codes (8 single-use 8-digit codes).
    /// Generated when 2FA is set up, consumed one by one.
    pub backup_codes: Vec<secrecy::SecretString>,
}

/// RBAC role. Lower numeric value = higher privilege.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Full access — can mutate users + clusters.
    Admin,
    /// Can run VM/LXC actions (start/stop/reboot) on any cluster.
    Operator,
    /// Read-only — list/detail VMs, readiness, health.
    Viewer,
}

impl Role {
    /// `true` if this role satisfies the required role.
    ///
    /// `Admin` ≥ `Operator` ≥ `Viewer` (admin can do anything a viewer can).
    #[must_use]
    pub fn can(self, required: Role) -> bool {
        let my_rank = self.rank();
        let req_rank = required.rank();
        my_rank <= req_rank
    }

    /// String form (lowercase, matches serde rename_all).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Operator => "operator",
            Self::Viewer => "viewer",
        }
    }

    fn rank(self) -> u8 {
        match self {
            Self::Admin => 0,
            Self::Operator => 1,
            Self::Viewer => 2,
        }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Role {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "admin" => Ok(Self::Admin),
            "operator" => Ok(Self::Operator),
            "viewer" => Ok(Self::Viewer),
            other => Err(format!(
                "unknown role '{other}' (expected admin|operator|viewer)"
            )),
        }
    }
}

impl User {
    /// Build a [`User`] from a [`crate::config::UserConfig`].
    ///
    /// - `password_hash` set → use directly (production path).
    /// - `password` set → bcrypt-hash at startup (dev / first-boot).
    /// - neither set → `Err`.
    /// - unknown `role` → `Err`.
    #[allow(clippy::result_large_err)] // String is fine for startup config errors
    pub fn from_config(cfg: &crate::config::UserConfig) -> Result<Self, String> {
        let Ok(role) = cfg.role.parse() else {
            return Err(format!(
                "user '{}' has unknown role '{}'",
                cfg.username, cfg.role
            ));
        };
        let hash = if let Some(h) = &cfg.password_hash {
            h.clone()
        } else if let Some(p) = &cfg.password {
            tracing::warn!(
                username = %cfg.username,
                "user has plaintext password in config — hash it before deploying"
            );
            hash_password(p).map_err(|e| format!("bcrypt hash failed: {e}"))?
        } else {
            return Err(format!(
                "user '{}' has neither password_hash nor password",
                cfg.username
            ));
        };
        Ok(User {
            id: cfg.id.clone(),
            username: cfg.username.clone(),
            display_name: if cfg.display_name.is_empty() {
                cfg.username.clone()
            } else {
                cfg.display_name.clone()
            },
            email: cfg.email.clone(),
            password_hash: SecretString::new(hash.into_boxed_str()),
            role,
            enabled: cfg.enabled,
            totp_secret: None,
            totp_enabled: false,
            backup_codes: vec![],
        })
    }
}

/// In-memory user store. Cheap to clone (`Arc` inside) so it can live in
/// `AppState` and be queried from request handlers.
#[derive(Debug, Clone)]
pub struct UserStore {
    /// All users, keyed by username (case-sensitive).
    pub(crate) users: HashMap<String, User>,
    /// Per-user allowed clusters. Key = username, Value = allowed cluster names.
    /// Empty vec for a user means access to ALL clusters (admin / unrestricted).
    /// When non-empty, the user can only see/operate on those clusters.
    pub(crate) allowed_clusters: HashMap<String, Vec<String>>,
}

impl UserStore {
    /// Create an empty user store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            users: HashMap::new(),
            allowed_clusters: HashMap::new(),
        }
    }

    /// Build a store pre-populated with the given users. Duplicate usernames
    /// are silently dropped (first one wins). All users get unrestricted
    /// cluster access (empty allowed_clusters).
    #[must_use]
    pub fn with_users(users: Vec<User>) -> Self {
        let mut store = Self::new();
        for u in users {
            store.users.insert(u.username.clone(), u);
        }
        store
    }

    /// Look up a user by username. Returns `None` if not found.
    #[must_use]
    pub fn get(&self, username: &str) -> Option<&User> {
        self.users.get(username)
    }

    /// Number of users.
    #[must_use]
    pub fn len(&self) -> usize {
        self.users.len()
    }

    /// `true` if the store has no users.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.users.is_empty()
    }

    /// Build a `UserStore` from a list of [`crate::config::UserConfig`].
    /// Returns the first config error (bad role, missing password, etc).
    ///
    /// Also populates the per-user allowed_clusters map from the config's
    /// `allowed_clusters` field. Users with an empty vec can access all clusters.
    pub fn from_configs(configs: &[crate::config::UserConfig]) -> Result<Self, String> {
        let users: Result<Vec<User>, String> = configs.iter().map(User::from_config).collect();
        let users = users?;
        let allowed_clusters: HashMap<String, Vec<String>> = configs
            .iter()
            .map(|c| (c.username.clone(), c.allowed_clusters.clone()))
            .collect();
        let user_map: HashMap<String, User> =
            users.into_iter().map(|u| (u.username.clone(), u)).collect();
        Ok(Self {
            users: user_map,
            allowed_clusters,
        })
    }

    /// Verify a username + password. Returns the user on success.
    ///
    /// Fails (`None`) when:
    /// - the username doesn't exist,
    /// - the account is disabled,
    /// - the password doesn't match.
    pub fn authenticate(&self, username: &str, password: &str) -> Option<&User> {
        let user = self.users.get(username)?;
        if !user.enabled {
            return None;
        }
        let ok = verify_password(password, user.password_hash.expose_secret())
            .ok()
            .unwrap_or(false);
        ok.then_some(user)
    }

    /// Check if a user can access a specific cluster.
    ///
    /// A user can access a cluster if:
    /// - The user has no explicit cluster restrictions (allowed_clusters empty),
    /// - The cluster is in the user's allowed_clusters list.
    ///
    /// Returns `true` for unknown usernames (fail-open for backward compat).
    #[must_use]
    pub fn user_can_access_cluster(&self, username: &str, cluster: &str) -> bool {
        match self.allowed_clusters.get(username) {
            // No restrictions — user can access all clusters.
            None => true,
            // Empty restrictions list is treated as "all clusters".
            Some(restrictions) if restrictions.is_empty() => true,
            // Check if the cluster is in the allowed list.
            Some(restrictions) => restrictions.iter().any(|c| c == cluster),
        }
    }

    /// Returns the list of allowed clusters for a user, or `None` if the
    /// user has no restrictions (can access all clusters).
    ///
    /// An empty `Some(vec![])` also means all clusters (admin default).
    #[must_use]
    pub fn user_allowed_clusters(&self, username: &str) -> Option<&[String]> {
        self.allowed_clusters.get(username).map(|v| v.as_slice())
    }

    /// Returns cluster permissions for a user: `None` = unrestricted,
    /// `Some(vec)` = limited to these clusters.
    #[must_use]
    pub fn user_allowed_clusters_owned(&self, username: &str) -> Option<Vec<String>> {
        self.allowed_clusters.get(username).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::password::hash_password;

    fn make_user(name: &str, role: Role) -> User {
        let hash = hash_password("hunter2").expect("bcrypt hash");
        User {
            id: format!("u-{name}"),
            username: name.to_string(),
            display_name: name.to_string(),
            email: None,
            password_hash: SecretString::new(hash.into_boxed_str()),
            role,
            enabled: true,
            totp_secret: None,
            totp_enabled: false,
            backup_codes: vec![],
        }
    }

    #[test]
    fn role_can_is_hierarchical() {
        assert!(Role::Admin.can(Role::Viewer));
        assert!(Role::Admin.can(Role::Operator));
        assert!(Role::Admin.can(Role::Admin));
        assert!(Role::Operator.can(Role::Viewer));
        assert!(Role::Operator.can(Role::Operator));
        assert!(!Role::Operator.can(Role::Admin));
        assert!(Role::Viewer.can(Role::Viewer));
        assert!(!Role::Viewer.can(Role::Operator));
        assert!(!Role::Viewer.can(Role::Admin));
    }

    #[test]
    fn role_from_str_round_trip() {
        for r in [Role::Admin, Role::Operator, Role::Viewer] {
            assert_eq!(r.as_str().parse::<Role>().unwrap(), r);
        }
        assert!("unknown".parse::<Role>().is_err());
    }

    #[test]
    fn user_store_authenticates_correct_password() {
        let store = UserStore::with_users(vec![make_user("alice", Role::Admin)]);
        let user = store.authenticate("alice", "hunter2").unwrap();
        assert_eq!(user.username, "alice");
        assert_eq!(user.role, Role::Admin);
    }

    #[test]
    fn user_store_rejects_wrong_password() {
        let store = UserStore::with_users(vec![make_user("alice", Role::Admin)]);
        assert!(store.authenticate("alice", "wrong").is_none());
    }

    #[test]
    fn user_store_rejects_unknown_user() {
        let store = UserStore::with_users(vec![make_user("alice", Role::Admin)]);
        assert!(store.authenticate("bob", "hunter2").is_none());
    }

    #[test]
    fn user_store_rejects_disabled_account() {
        let mut u = make_user("alice", Role::Admin);
        u.enabled = false;
        let store = UserStore::with_users(vec![u]);
        assert!(store.authenticate("alice", "hunter2").is_none());
    }

    #[test]
    fn user_store_dedupes_on_collision() {
        let u1 = make_user("alice", Role::Admin);
        let mut u2 = make_user("alice", Role::Viewer);
        u2.id = "u-alice-2".to_string();
        let store = UserStore::with_users(vec![u1, u2]);
        assert_eq!(store.len(), 1);
        // One of the two roles was kept — we don't care which (HashMap
        // iteration order isn't stable for keys of the same hash).
        let kept = store.get("alice").unwrap().role;
        assert!(
            kept == Role::Admin || kept == Role::Viewer,
            "expected one of the two roles, got {kept:?}"
        );
    }
}
