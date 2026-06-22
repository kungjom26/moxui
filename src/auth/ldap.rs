//! LDAP / Active Directory authentication module.
//!
//! Provides bind + search + credential verification against an LDAP
//! directory. Supports both `ldap://` and `ldaps://` connections.
//!
//! ## Flow
//!
//! 1. Bind as the configured service account (`bind_dn` + `bind_password`).
//! 2. Search for the user entry using `user_filter` (with `{username}` replaced).
//! 3. Extract attributes (username, display name, email) from the entry.
//! 4. Attempt to re-bind as the found user DN with the supplied password.
//! 5. If re-bind succeeds, the credentials are valid.
//! 6. Optionally auto-create a local user account.

use ldap3::{LdapConnAsync, LdapConnSettings, Mod, ResultEntry, SearchEntry, Scope, SearchResult};
use tracing::{info, warn};

use crate::config::LdapConfig;

/// Information extracted from an LDAP entry after successful bind.
#[derive(Debug, Clone)]
pub struct LdapUserInfo {
    /// The user's DN.
    pub dn: String,
    /// Username (from `username_attr`).
    pub username: String,
    /// Display name (from `displayname_attr`, or username if not present).
    pub display_name: String,
    /// Email (from `email_attr`, may be empty).
    pub email: String,
}

/// Verify LDAP credentials by performing a bind + search + re-bind.
///
/// Returns `Some(LdapUserInfo)` on success (username + password are valid),
/// or `None` if authentication fails.
pub async fn verify_ldap_credentials(
    config: &LdapConfig,
    username: &str,
    password: &str,
) -> Option<LdapUserInfo> {
    let url = config.url.as_ref()?;
    let base_dn = config.base_dn.as_ref()?;
    let bind_dn = config.bind_dn.as_ref()?;
    let bind_password = config.bind_password.as_ref()?;

    // Step 1: Connect to LDAP server
    let settings = LdapConnSettings::new()
        .set_no_tls_verify(!url.starts_with("ldaps://"))
        .set_starttls(false);

    let (conn, mut ldap) = LdapConnAsync::with_settings(settings, url).await.ok()?;
    ldap3::drive!(conn);

    // Step 2: Bind as service account
    if ldap.simple_bind(bind_dn, bind_password).await.is_err() {
        warn!("LDAP bind failed for service account");
        return None;
    }

    // Step 3: Search for the user
    let filter = config.user_filter.replace("{username}", username);
    let attrs = vec![
        config.username_attr.clone(),
        config.displayname_attr.clone(),
        config.email_attr.clone(),
    ];

    let SearchResult(entries, _) = ldap
        .search(
            base_dn,
            Scope::Subtree,
            &filter,
            attrs,
        )
        .await
        .ok()?;

    if entries.is_empty() {
        info!("LDAP search returned no results for user: {username}");
        return None;
    }

    let entry = SearchEntry::construct(entries.into_iter().next()?);
    let user_dn = entry.dn;

    // Step 4: Extract attributes
    let username_val = entry
        .attrs
        .get(&config.username_attr)
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_else(|| username.to_string());

    let display_name = entry
        .attrs
        .get(&config.displayname_attr)
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_else(|| username_val.clone());

    let email = entry
        .attrs
        .get(&config.email_attr)
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default();

    // Step 5: Re-bind as the user to verify password
    if ldap.simple_bind(&user_dn, password).await.is_err() {
        info!("LDAP password verification failed for user: {username}");
        return None;
    }

    // Step 6: Unbind gracefully
    let _ = ldap.unbind().await;

    info!(username = %username_val, "LDAP authentication successful");

    Some(LdapUserInfo {
        dn: user_dn,
        username: username_val,
        display_name,
        email,
    })
}

/// Build a local user id from an LDAP user.
pub fn ldap_user_id(username: &str) -> String {
    format!("ldap-{username}")
}
