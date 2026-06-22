//! OIDC / OAuth2 SSO support (Google + GitHub).
//!
//! Uses the `openidconnect` crate for Google (full OpenID Connect with
//! discovery) and a manual OAuth2 flow for GitHub (which does not support
//! OpenID Connect). Both flows are unified behind [`OidcService`].

use std::collections::HashMap;
use std::sync::Arc;

use openidconnect::core::CoreProviderMetadata;
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
};
use tokio::sync::Mutex;

use crate::config::OidcProviderConfig;

/// Time-to-live for pending OIDC state entries (5 minutes).
const PENDING_STATE_TTL_MINUTES: i64 = 5;

// ── Types ───────────────────────────────────────────────────────────

/// Pending OIDC / OAuth2 authorization state, retained across the user's
/// browser redirect.
#[derive(Debug, Clone)]
pub struct PendingOidcState {
    /// The provider name (e.g. "google", "github").
    pub provider_name: String,
    /// PKCE code verifier secret (stored as String since PkceCodeVerifier is not Clone).
    pub pkce_verifier_secret: String,
    /// Nonce (used to verify the ID token for OIDC providers).
    pub nonce_secret: Option<String>,
    /// CSRF token secret.
    pub csrf_token_secret: String,
    /// When this state was created (for TTL enforcement).
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Unified user info returned after a successful OIDC / OAuth2 callback.
#[derive(Debug, Clone)]
pub struct OidcUserInfo {
    /// Provider name (e.g. "google", "github").
    pub provider: String,
    /// Provider-specific unique user ID (e.g. Google `sub`, GitHub user id).
    pub sub: String,
    /// Preferred username.
    pub username: String,
    /// Email address (may be `None` if the provider didn't return one).
    pub email: Option<String>,
    /// Display name.
    pub display_name: String,
}

/// Internal representation of a configured OAuth2/OIDC provider.
enum ProviderClient {
    /// Google — full OpenID Connect with discovery.
    Google {
        /// The full OIDC client (type inferred from from_provider_metadata).
        // We store it as a boxed dyn-like opaque. Since the type is complex
        // and changes after from_provider_metadata, we store raw data and
        // reconstruct per-call.
        client_id: String,
        client_secret: String,
        redirect_url: String,
        issuer_url: String,
    },
    /// GitHub — OAuth2 only (no OIDC support).
    GitHub {
        /// OAuth2 client ID.
        client_id: String,
        /// OAuth2 client secret.
        client_secret: String,
        /// Registered redirect URL.
        redirect_url: String,
    },
}

// ── Service ─────────────────────────────────────────────────────────

/// Manages OIDC / OAuth2 providers for SSO login.
///
/// Holds configured provider clients and an in-memory store of pending
/// authorization states (PKCE challenges + nonces) keyed by a random
/// state key returned to the client.
pub struct OidcService {
    /// Configured providers, keyed by name ("google", "github").
    providers: HashMap<String, ProviderClient>,
    /// Pending OIDC/OAuth2 states, keyed by a random state key.
    /// The state key is returned to the frontend and submitted back
    /// on the callback, allowing us to retrieve the PKCE verifier.
    state_store: Arc<Mutex<HashMap<String, PendingOidcState>>>,
}

impl OidcService {
    /// Build an [`OidcService`] from the list of configured providers.
    ///
    /// Returns `None` if the provider list is empty. Returns an error
    /// if any provider configuration is invalid.
    pub async fn new(configs: &[OidcProviderConfig]) -> Result<Option<Self>, String> {
        if configs.is_empty() {
            return Ok(None);
        }

        let mut providers = HashMap::new();

        for cfg in configs {
            match cfg.name.to_lowercase().as_str() {
                "google" => {
                    // Validate the Google config by doing discovery at startup
                    let issuer_url = IssuerUrl::new("https://accounts.google.com".to_string())
                        .map_err(|e| format!("invalid issuer URL: {e}"))?;

                    let http_client = reqwest::Client::new();
                    CoreProviderMetadata::discover_async(issuer_url, &http_client)
                        .await
                        .map_err(|e| format!("Google OIDC discovery failed: {e}"))?;

                    providers.insert(
                        "google".to_string(),
                        ProviderClient::Google {
                            client_id: cfg.client_id.clone(),
                            client_secret: cfg.client_secret.clone(),
                            redirect_url: cfg.redirect_url.clone(),
                            issuer_url: "https://accounts.google.com".to_string(),
                        },
                    );
                    tracing::info!("OIDC provider 'google' configured");
                }
                "github" => {
                    providers.insert(
                        "github".to_string(),
                        ProviderClient::GitHub {
                            client_id: cfg.client_id.clone(),
                            client_secret: cfg.client_secret.clone(),
                            redirect_url: cfg.redirect_url.clone(),
                        },
                    );
                    tracing::info!("OAuth2 provider 'github' configured");
                }
                other => {
                    return Err(format!(
                        "unknown OIDC provider '{other}' — expected 'google' or 'github'"
                    ));
                }
            }
        }

        Ok(Some(Self {
            providers,
            state_store: Arc::new(Mutex::new(HashMap::new())),
        }))
    }

    /// Start a login flow for the given provider.
    ///
    /// Returns an authorization URL to redirect the user to, and a
    /// state key that must be submitted back on the callback.
    pub async fn start_login(&self, provider_name: &str) -> Result<(url::Url, String), String> {
        let state_key = uuid::Uuid::new_v4().to_string();

        match self.providers.get(provider_name) {
            Some(ProviderClient::Google {
                client_id,
                client_secret,
                redirect_url,
                issuer_url: _,
            }) => {
                // Build the OIDC client via discovery
                let issuer_url = IssuerUrl::new("https://accounts.google.com".to_string())
                    .map_err(|e| format!("invalid issuer URL: {e}"))?;
                let http_client = reqwest::Client::new();
                let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, &http_client)
                    .await
                    .map_err(|e| format!("Google OIDC discovery failed: {e}"))?;

                let client = openidconnect::core::CoreClient::from_provider_metadata(
                    provider_metadata,
                    ClientId::new(client_id.clone()),
                    Some(ClientSecret::new(client_secret.clone())),
                )
                .set_redirect_uri(
                    RedirectUrl::new(redirect_url.clone())
                        .map_err(|e| format!("invalid redirect URL: {e}"))?,
                );

                // Generate PKCE challenge + verifier
                let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
                let nonce = Nonce::new_random();
                let nonce_secret = nonce.secret().clone();

                let (auth_url, csrf_token, _nonce) = client
                    .authorize_url(
                        openidconnect::core::CoreAuthenticationFlow::AuthorizationCode,
                        || CsrfToken::new_random(),
                        move || nonce.clone(),
                    )
                    .set_pkce_challenge(pkce_challenge)
                    .add_scope(Scope::new("openid".to_string()))
                    .add_scope(Scope::new("email".to_string()))
                    .add_scope(Scope::new("profile".to_string()))
                    .url();

                // Store the state
                let mut store = self.state_store.lock().await;
                store.insert(
                    state_key.clone(),
                    PendingOidcState {
                        provider_name: "google".to_string(),
                        pkce_verifier_secret: pkce_verifier.secret().clone(),
                        nonce_secret: Some(nonce_secret),
                        csrf_token_secret: csrf_token.secret().clone(),
                        created_at: chrono::Utc::now(),
                    },
                );

                Ok((auth_url, state_key))
            }
            Some(ProviderClient::GitHub {
                client_id,
                client_secret: _,
                redirect_url,
            }) => {
                // GitHub uses plain OAuth2 — build the authorize URL manually
                let csrf_token = CsrfToken::new_random();
                let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

                let mut auth_url = url::Url::parse("https://github.com/login/oauth/authorize")
                    .map_err(|e| format!("invalid GitHub auth URL: {e}"))?;

                auth_url
                    .query_pairs_mut()
                    .append_pair("client_id", client_id)
                    .append_pair("redirect_uri", redirect_url)
                    .append_pair("state", csrf_token.secret())
                    .append_pair("code_challenge", pkce_challenge.as_str())
                    .append_pair("code_challenge_method", "S256")
                    .append_pair("scope", "read:user user:email");

                // Store the state
                let mut store = self.state_store.lock().await;
                store.insert(
                    state_key.clone(),
                    PendingOidcState {
                        provider_name: "github".to_string(),
                        pkce_verifier_secret: pkce_verifier.secret().clone(),
                        nonce_secret: None,
                        csrf_token_secret: csrf_token.secret().clone(),
                        created_at: chrono::Utc::now(),
                    },
                );

                Ok((auth_url, state_key))
            }
            None => Err(format!(
                "unknown OIDC provider '{provider_name}' — expected 'google' or 'github'"
            )),
        }
    }

    /// Complete the OIDC/OAuth2 login flow by exchanging the authorization
    /// code for tokens and retrieving user info.
    ///
    /// Returns the OIDC user info on success.
    pub async fn handle_callback(
        &self,
        state_key: &str,
        code: &str,
        state: &str,
    ) -> Result<OidcUserInfo, String> {
        // Retrieve and remove the pending state
        let pending = {
            let mut store = self.state_store.lock().await;
            store.remove(state_key).ok_or_else(|| {
                "invalid or expired OIDC state key — the login session may have timed out"
                    .to_string()
            })?
        };

        // Check TTL
        let elapsed = chrono::Utc::now() - pending.created_at;
        if elapsed.num_minutes() > PENDING_STATE_TTL_MINUTES {
            return Err("OIDC login session expired (state TTL exceeded)".to_string());
        }

        // Verify CSRF state
        let csrf_parts: Vec<&str> = state.split('.').collect();
        let csrf_value = csrf_parts.first().copied().unwrap_or(state);
        if csrf_value != pending.csrf_token_secret {
            return Err("CSRF state mismatch — possible replay attack".to_string());
        }

        match pending.provider_name.as_str() {
            "google" => self.handle_google_callback(pending, code).await,
            "github" => self.handle_github_callback(pending, code).await,
            other => Err(format!("unknown provider in pending state: '{other}'")),
        }
    }

    /// Handle the Google OIDC callback (exchange code, verify ID token).
    async fn handle_google_callback(
        &self,
        pending: PendingOidcState,
        code: &str,
    ) -> Result<OidcUserInfo, String> {
        let ProviderClient::Google {
            client_id,
            client_secret,
            redirect_url,
            issuer_url: _,
        } = self
            .providers
            .get("google")
            .ok_or("Google provider not configured")?
        else {
            return Err("internal error: google provider type mismatch".to_string());
        };

        // Build the OIDC client via discovery
        let issuer_url =
            IssuerUrl::new("https://accounts.google.com".to_string())
                .map_err(|e| format!("invalid issuer URL: {e}"))?;
        let http_client = reqwest::Client::new();
        let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, &http_client)
            .await
            .map_err(|e| format!("Google OIDC discovery failed: {e}"))?;

        let client = openidconnect::core::CoreClient::from_provider_metadata(
            provider_metadata,
            ClientId::new(client_id.clone()),
            Some(ClientSecret::new(client_secret.clone())),
        )
        .set_redirect_uri(
            RedirectUrl::new(redirect_url.clone())
                .map_err(|e| format!("invalid redirect URL: {e}"))?,
        );

        // Reconstruct the PKCE verifier
        let pkce_verifier = PkceCodeVerifier::new(pending.pkce_verifier_secret.clone());

        // Exchange the authorization code for tokens
        let token_response = client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .map_err(|e| format!("Google token exchange setup failed: {e}"))?
            .set_pkce_verifier(pkce_verifier)
            .request_async(&http_client)
            .await
            .map_err(|e| format!("Google token exchange failed: {e}"))?;

        // Get the ID token
        let id_token = token_response
            .extra_fields()
            .id_token()
            .ok_or("Google did not return an ID token".to_string())?
            .clone();

        // Verify the ID token
        let nonce_secret = pending
            .nonce_secret
            .ok_or("missing nonce for Google callback")?;
        let nonce = Nonce::new(nonce_secret);

        let verifier = client.id_token_verifier();
        let claims = id_token
            .claims(&verifier, &nonce)
            .map_err(|e| format!("Google ID token verification failed: {e}"))?;

        let sub = claims.subject().to_string();
        let email = claims.email().map(|e| e.to_string());
        let preferred_username = claims
            .preferred_username()
            .map(|n| n.to_string())
            .unwrap_or_else(|| sub.split('@').next().unwrap_or("google-user").to_string());
        let display_name = claims
            .name()
            .and_then(|n| n.get(None))
            .map(|n| n.to_string())
            .unwrap_or_else(|| preferred_username.clone());

        let username = Self::sanitize_username(&preferred_username);

        tracing::info!(
            provider = "google",
            sub = %sub,
            username = %username,
            "Google OIDC login successful"
        );

        Ok(OidcUserInfo {
            provider: "google".to_string(),
            sub,
            username,
            email,
            display_name,
        })
    }

    /// Handle the GitHub OAuth2 callback (exchange code, fetch user info).
    async fn handle_github_callback(
        &self,
        pending: PendingOidcState,
        code: &str,
    ) -> Result<OidcUserInfo, String> {
        let ProviderClient::GitHub {
            client_id,
            client_secret,
            redirect_url,
        } = self
            .providers
            .get("github")
            .ok_or("GitHub provider not configured")?
        else {
            return Err("internal error: github provider type mismatch".to_string());
        };

        // Exchange the authorization code for an access token
        let http_client = reqwest::Client::new();
        let token_params = [
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("code", code),
            ("redirect_uri", redirect_url.as_str()),
            ("code_verifier", &pending.pkce_verifier_secret),
        ];

        let token_resp = http_client
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .form(&token_params)
            .send()
            .await
            .map_err(|e| format!("GitHub token exchange request failed: {e}"))?;

        let token_body: serde_json::Value = token_resp
            .json()
            .await
            .map_err(|e| format!("GitHub token exchange parse failed: {e}"))?;

        let access_token = token_body["access_token"]
            .as_str()
            .ok_or_else(|| {
                let err = token_body["error_description"]
                    .as_str()
                    .unwrap_or(token_body["error"].as_str().unwrap_or("unknown"));
                format!("GitHub token exchange failed: {err}")
            })?
            .to_string();

        // Fetch user info from GitHub API
        let user_resp = http_client
            .get("https://api.github.com/user")
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {access_token}"))
            .header("User-Agent", "MoxUI/0.1")
            .send()
            .await
            .map_err(|e| format!("GitHub user info request failed: {e}"))?;

        let user_data: serde_json::Value = user_resp
            .json()
            .await
            .map_err(|e| format!("GitHub user info parse failed: {e}"))?;

        let sub = user_data["id"].to_string();
        let login = user_data["login"]
            .as_str()
            .unwrap_or("github-user")
            .to_string();
        let display_name = user_data["name"].as_str().unwrap_or(&login).to_string();
        let email = user_data["email"].as_str().map(|e| e.to_string());

        // Try to get primary verified email if not public
        let email = if email.is_some() {
            email
        } else {
            // Fetch emails from GitHub
            let emails_resp = http_client
                .get("https://api.github.com/user/emails")
                .header("Accept", "application/json")
                .header("Authorization", format!("Bearer {access_token}"))
                .header("User-Agent", "MoxUI/0.1")
                .send()
                .await
                .map_err(|e| format!("GitHub emails request failed: {e}"))?;

            if let Ok(emails) = emails_resp.json::<Vec<serde_json::Value>>().await {
                emails
                    .into_iter()
                    .find(|e| e["primary"].as_bool().unwrap_or(false))
                    .and_then(|e| e["email"].as_str().map(|s| s.to_string()))
            } else {
                None
            }
        };

        let username = Self::sanitize_username(&login);

        tracing::info!(
            provider = "github",
            sub = %sub,
            username = %username,
            "GitHub OAuth2 login successful"
        );

        Ok(OidcUserInfo {
            provider: "github".to_string(),
            sub,
            username,
            email,
            display_name,
        })
    }

    /// Sanitize a provider username to be safe as a MoxUI username:
    /// lowercase, replace non-alphanumeric chars with underscores.
    fn sanitize_username(raw: &str) -> String {
        let sanitized: String = raw
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        sanitized.to_lowercase()
    }
}
