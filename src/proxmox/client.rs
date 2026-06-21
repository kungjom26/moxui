//! Proxmox API client (ticket auth + connection pool + circuit breaker).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;

use crate::config::ClusterConfig;
use crate::error::{AppError, AppResult};
use crate::proxmox::auth::{SecretTicket, Ticket, TicketResponse};
use crate::proxmox::circuit_breaker::CircuitBreaker;
use crate::proxmox::retry::RetryPolicy;

use reqwest::Client;

/// Timeout (seconds) for [`ProxmoxClient::ping`] — bounded so a slow or
/// unreachable cluster cannot stall endpoints that probe reachability
/// (e.g. `/readyz`).
pub const PING_TIMEOUT_SECS: u64 = 3;

/// Timeout (seconds) for [`ProxmoxClient::list_vms`] — bounded so a slow
/// or unreachable cluster cannot stall `/api/v1/vms` indefinitely.
pub const LIST_VMS_TIMEOUT_SECS: u64 = 10;

/// Timeout (seconds) for [`ProxmoxClient::post`] — bounded so a slow or
/// unreachable cluster cannot stall write endpoints (`vm_action`, etc.)
/// indefinitely. Same rationale as [`LIST_VMS_TIMEOUT_SECS`].
pub const POST_TIMEOUT_SECS: u64 = 10;

/// Proxmox API client for one cluster.
pub struct ProxmoxClient {
    /// Cluster configuration.
    config: ClusterConfig,
    /// HTTP client (connection pool).
    http: Client,
    /// Current ticket (auto-refreshed). Wrapped in [`SecretTicket`] so
    /// plaintext never leaks via Debug/println/eyeball.
    ticket: Arc<RwLock<Option<SecretTicket>>>,
    /// Circuit breaker.
    circuit_breaker: CircuitBreaker,
    /// Retry policy (consumed by `request()` helper in Day 2).
    #[allow(dead_code)]
    retry_policy: RetryPolicy,
}

impl ProxmoxClient {
    /// Create a new Proxmox client for the given cluster.
    #[allow(clippy::unused_async)] // async signature kept for API stability — will become async when login pre-warm is added
    pub async fn new(config: ClusterConfig) -> AppResult<Self> {
        let mut builder = Client::builder()
            .danger_accept_invalid_certs(config.insecure_skip_verify)
            .timeout(Duration::from_secs(30));

        // If CA cert is provided, use it for verification
        if let Some(ca_pem) = &config.ca_cert_pem {
            let cert = reqwest::Certificate::from_pem(ca_pem.as_bytes())
                .map_err(|e| AppError::Config(format!("Invalid CA cert: {e}")))?;
            builder = builder.add_root_certificate(cert);
        }

        let http = builder
            .build()
            .map_err(|e| AppError::Config(format!("Failed to build HTTP client: {e}")))?;

        Ok(Self {
            config,
            http,
            ticket: Arc::new(RwLock::new(None)),
            circuit_breaker: CircuitBreaker::default(),
            retry_policy: RetryPolicy::default(),
        })
    }

    /// Get the cluster URL.
    pub fn url(&self) -> &str {
        &self.config.url
    }

    /// Get the cluster name.
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Check if a request is allowed (circuit breaker state).
    pub fn allow_request(&self) -> bool {
        self.circuit_breaker.allow_request()
    }

    /// Login to Proxmox and get a ticket.
    pub async fn login(&self) -> AppResult<()> {
        use secrecy::ExposeSecret;
        let url = format!("{}/api2/json/access/ticket", self.config.url);
        let resp = self
            .http
            .post(&url)
            .json(&serde_json::json!({
                "username": &self.config.username,
                "password": self.config.password.expose_secret(),
            }))
            .send()
            .await?;

        if !resp.status().is_success() {
            self.circuit_breaker.record_failure();
            return Err(AppError::Proxmox(format!(
                "Login failed with status {}",
                resp.status()
            )));
        }

        let body: TicketResponse = resp.json().await?;
        let mut guard = self.ticket.write().await;
        *guard = Some(SecretTicket::new(body.data));
        self.circuit_breaker.record_success();
        Ok(())
    }

    /// Ensure we have a valid ticket (refresh if <5min to expiry).
    pub async fn ensure_ticket(&self) -> AppResult<()> {
        let should_refresh = {
            let guard = self.ticket.read().await;
            match guard.as_ref() {
                Some(secret) => {
                    let now = chrono::Utc::now().timestamp();
                    secret.expose_ticket().expires_at - now < 300 // refresh 5 min before expiry
                }
                None => true,
            }
        };

        if should_refresh {
            self.login().await?;
        }
        Ok(())
    }

    /// Get current ticket (forces login if missing).
    pub async fn current_ticket(&self) -> AppResult<Ticket> {
        self.ensure_ticket().await?;
        let guard = self.ticket.read().await;
        guard
            .as_ref()
            .map(|s| s.expose_ticket().clone())
            .ok_or_else(|| AppError::Internal("ticket missing after ensure".into()))
    }

    /// Get the underlying HTTP client (for requests).
    pub fn http(&self) -> &Client {
        &self.http
    }

    /// Issue an authenticated GET to a Proxmox API endpoint and decode the JSON
    /// `data` field into the requested type.
    ///
    /// # Arguments
    ///
    /// * `path` — path under `/api2/json/...` (e.g., `"/version"` or
    ///   `"/nodes/pve11/status"`). Leading `/` is optional.
    ///
    /// # Type bounds
    ///
    /// `T` must implement `serde::de::DeserializeOwned` (owns its data, no
    /// lifetimes). This is satisfied by all `proxmox::types::*` structs and
    /// any other DTO deserialized from Proxmox JSON.
    ///
    /// # Errors
    ///
    /// * `AppError::Proxmox` on HTTP / non-2xx / JSON-decode failures
    /// * `AppError::Internal` if login succeeded but no ticket was stored
    pub async fn get<T>(&self, path: &str) -> AppResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let ticket = self.current_ticket().await?;
        let url = self.build_url(path);
        let resp = self
            .http
            .get(&url)
            .header("Cookie", format!("PVEAuthCookie={}", ticket.ticket))
            .send()
            .await?;
        self.handle_response::<T>(resp).await
    }

    /// Build a full URL from a relative API path.
    fn build_url(&self, path: &str) -> String {
        let path = path.trim_start_matches('/');
        format!(
            "{}/api2/json/{}",
            self.config.url.trim_end_matches('/'),
            path
        )
    }

    /// Decode a response, recording success/failure on the circuit breaker.
    async fn handle_response<T>(&self, resp: reqwest::Response) -> AppResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        if !resp.status().is_success() {
            self.circuit_breaker.record_failure();
            return Err(AppError::Proxmox(format!(
                "GET failed with status {}",
                resp.status()
            )));
        }
        let body: crate::proxmox::types::ApiResponse<T> = resp.json().await?;
        self.circuit_breaker.record_success();
        Ok(body.data)
    }

    /// Issue a state-changing request (POST/PUT/DELETE) to Proxmox.
    ///
    /// Auto-acquires ticket AND includes the CSRF token header (required
    /// by Proxmox for all write operations). Returns the deserialized
    /// response data or the raw UPID string for fire-and-forget calls.
    ///
    /// # Arguments
    ///
    /// * `method` — HTTP method (`POST`/`PUT`/`DELETE`).
    /// * `path` — API path under `/api2/json/`.
    /// * `body` — optional JSON body (use `&Empty::new()` for no body).
    pub async fn post<T: serde::de::DeserializeOwned + Default>(&self, path: &str) -> AppResult<T> {
        let ticket = self.current_ticket().await?;
        let url = self.build_url(path);
        let fut = self
            .http
            .post(&url)
            .header("Cookie", format!("PVEAuthCookie={}", ticket.ticket))
            .header("CSRFPreventionToken", &ticket.csrf_token)
            .send();
        let resp = match tokio::time::timeout(Duration::from_secs(POST_TIMEOUT_SECS), fut).await {
            Ok(result) => result?,
            Err(_elapsed) => {
                return Err(AppError::Proxmox(format!(
                    "POST {path} timed out after {POST_TIMEOUT_SECS}s"
                )))
            }
        };
        self.handle_response::<T>(resp).await
    }

    /// Record success (reset circuit breaker).
    pub fn record_success(&self) {
        self.circuit_breaker.record_success();
    }

    /// Record failure (may open circuit breaker).
    pub fn record_failure(&self) {
        self.circuit_breaker.record_failure();
    }

    /// List all VMs/LXC across the cluster via `/cluster/resources?type=vm`.
    ///
    /// Returns one [`crate::proxmox::types::VmResource`] per VM/LXC.
    /// Errors propagate as `AppError::Proxmox`.
    ///
    /// Bounded by `LIST_VMS_TIMEOUT_SECS` so a slow/unreachable cluster
    /// does not stall `/api/v1/vms` indefinitely.
    pub async fn list_vms(&self) -> AppResult<Vec<crate::proxmox::types::VmResource>> {
        match tokio::time::timeout(
            Duration::from_secs(LIST_VMS_TIMEOUT_SECS),
            self.get("cluster/resources?type=vm"),
        )
        .await
        {
            Ok(result) => result,
            Err(_elapsed) => Err(AppError::Proxmox(format!(
                "list_vms timed out after {LIST_VMS_TIMEOUT_SECS}s"
            ))),
        }
    }

    /// Lightweight ping — fetch `/version` to verify reachability + ticket cache.
    ///
    /// Returns `Ok(())` on any 2xx (ticket can be acquired AND version returns).
    /// On failure, the circuit breaker is updated.
    ///
    /// Bounded by a short timeout (`PING_TIMEOUT`) so a slow/unreachable
    /// cluster does not stall `/readyz` or other endpoints that depend on
    /// reachability.
    pub async fn ping(&self) -> AppResult<()> {
        // Clone the ticket future into a timeout-wrapped block. We can't
        // wrap the whole call in `timeout` because `self` is borrowed for
        // the entire duration.
        let ping = self.get::<crate::proxmox::types::Version>("version");
        match tokio::time::timeout(Duration::from_secs(PING_TIMEOUT_SECS), ping).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_elapsed) => Err(AppError::Proxmox(format!(
                "ping timed out after {PING_TIMEOUT_SECS}s"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cluster_config() -> ClusterConfig {
        ClusterConfig {
            name: "test".to_string(),
            url: "https://test.local:8006".to_string(),
            username: "root@pam".to_string(),
            password: secrecy::SecretString::new("test".to_string().into_boxed_str()),
            realm: "pam".to_string(),
            insecure_skip_verify: true,
            ca_cert_pem: None,
        }
    }

    #[tokio::test]
    async fn test_client_creation() {
        let config = test_cluster_config();
        let client = ProxmoxClient::new(config).await;
        assert!(client.is_ok());
    }

    #[test]
    fn test_url_and_name() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let config = test_cluster_config();
        let client = rt.block_on(ProxmoxClient::new(config)).unwrap();
        assert_eq!(client.name(), "test");
        assert_eq!(client.url(), "https://test.local:8006");
    }

    #[test]
    fn test_build_url_normalizes_paths() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let config = test_cluster_config();
        let client = rt.block_on(ProxmoxClient::new(config)).unwrap();
        // No leading slash, no trailing slash on base
        assert_eq!(
            client.build_url("version"),
            "https://test.local:8006/api2/json/version"
        );
        // Leading slash gets trimmed
        assert_eq!(
            client.build_url("/version"),
            "https://test.local:8006/api2/json/version"
        );
        // Nested path
        assert_eq!(
            client.build_url("nodes/pve11/status"),
            "https://test.local:8006/api2/json/nodes/pve11/status"
        );
    }

    /// End-to-end: mock a Proxmox `/access/ticket` POST and a `/version` GET
    /// and verify the client can log in, cache the ticket, and use it on the
    /// follow-up authenticated call.
    #[tokio::test]
    async fn test_login_and_get_against_wiremock() {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        // Mock /access/ticket (login) — returns a fake ticket.
        // Body shape matches our `Ticket` struct: ticket, csrf_token, username, expires_at.
        // We don't match the request body — reqwest serializes the login JSON
        // body so `body_string("")` won't match. Path + method is enough for
        // the mock to fire.
        Mock::given(method("POST"))
            .and(path("/api2/json/access/ticket"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "ticket": "PVE:root@pam:deadbeef::SIG",
                    "csrf_token": "csrf-token-abc",
                    "username": "root@pam",
                    "expires_at": 9_999_999_999_i64
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        // Mock /version — expects the cookie from login
        Mock::given(method("GET"))
            .and(path("/api2/json/version"))
            .and(header("Cookie", "PVEAuthCookie=PVE:root@pam:deadbeef::SIG"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "version": "8.2.4",
                    "release": "8.2",
                    "repoid": "deadbeef"
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        // Build a client pointing at the mock server
        let config = ClusterConfig {
            name: "mock".to_string(),
            url: server.uri(),
            username: "root@pam".to_string(),
            password: secrecy::SecretString::new("test-pw".to_string().into_boxed_str()),
            realm: "pam".to_string(),
            insecure_skip_verify: true,
            ca_cert_pem: None,
        };
        let client = ProxmoxClient::new(config).await.unwrap();

        // GET should trigger login first, then return the version
        let v: crate::proxmox::types::Version = client.get("version").await.unwrap();
        assert_eq!(v.version, "8.2.4");
        assert_eq!(v.release, "8.2");
    }

    /// End-to-end: verify `list_vms()` returns parsed `VmResource` rows from
    /// Proxmox `/cluster/resources?type=vm`.
    #[tokio::test]
    async fn test_list_vms_against_wiremock() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api2/json/access/ticket"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "ticket": "PVE:root@pam:deadbeef::SIG",
                    "csrf_token": "csrf-token-abc",
                    "username": "root@pam",
                    "expires_at": 9_999_999_999_i64
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/api2/json/cluster/resources"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "vmid": 100,
                        "name": "web-1",
                        "node": "pve11",
                        "status": "running",
                        "cpu": 0.05,
                        "cpus": 2.0,
                        "mem": 1_073_741_824_u64,
                        "maxmem": 2_147_483_648_u64,
                        "uptime": 3600,
                        "tags": "prod;web"
                    },
                    {
                        "vmid": 101,
                        "name": "db-1",
                        "node": "pve12",
                        "status": "stopped",
                        "cpu": 0.0,
                        "cpus": 4.0,
                        "mem": 0_u64,
                        "maxmem": 4_294_967_296_u64,
                        "tags": null
                    }
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let config = ClusterConfig {
            name: "mock".to_string(),
            url: server.uri(),
            username: "root@pam".to_string(),
            password: secrecy::SecretString::new("test-pw".to_string().into_boxed_str()),
            realm: "pam".to_string(),
            insecure_skip_verify: true,
            ca_cert_pem: None,
        };
        let client = ProxmoxClient::new(config).await.unwrap();
        let vms = client.list_vms().await.unwrap();
        assert_eq!(vms.len(), 2);

        assert_eq!(vms[0].vmid, 100);
        assert_eq!(vms[0].name, "web-1");
        assert_eq!(vms[0].status, "running");
        assert_eq!(vms[0].node, "pve11");
        assert_eq!(vms[0].tags.as_deref(), Some("prod;web"));

        assert_eq!(vms[1].vmid, 101);
        assert_eq!(vms[1].name, "db-1");
        assert_eq!(vms[1].status, "stopped");
        assert_eq!(vms[1].tags, None);
    }

    /// End-to-end: `ping()` succeeds when the cluster is reachable.
    #[tokio::test]
    async fn test_ping_succeeds_against_wiremock() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api2/json/access/ticket"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "ticket": "PVE:root@pam:deadbeef::SIG",
                    "csrf_token": "csrf-token-abc",
                    "username": "root@pam",
                    "expires_at": 9_999_999_999_i64
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/api2/json/version"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "version": "8.2.4",
                    "release": "8.2",
                    "repoid": "deadbeef"
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let config = ClusterConfig {
            name: "mock".to_string(),
            url: server.uri(),
            username: "root@pam".to_string(),
            password: secrecy::SecretString::new("test-pw".to_string().into_boxed_str()),
            realm: "pam".to_string(),
            insecure_skip_verify: true,
            ca_cert_pem: None,
        };
        let client = ProxmoxClient::new(config).await.unwrap();
        assert!(client.ping().await.is_ok());
    }

    /// End-to-end: `post()` issues a CSRF-protected state-changing call and
    /// returns the UPID Proxmox sent back. Verifies that the CSRF header
    /// is sent (required by Proxmox for all write operations).
    #[tokio::test]
    async fn test_post_with_csrf_header() {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api2/json/access/ticket"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "ticket": "PVE:root@pam:deadbeef::SIG",
                    "csrf_token": "csrf-token-abc",
                    "username": "root@pam",
                    "expires_at": 9_999_999_999_i64
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/api2/json/nodes/pve11/qemu/103/status/start"))
            .and(header("Cookie", "PVEAuthCookie=PVE:root@pam:deadbeef::SIG"))
            .and(header("CSRFPreventionToken", "csrf-token-abc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": "UPID:pve11:00001234:00000000:60F0EEEE:qmstart:103:root@pam:"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let config = ClusterConfig {
            name: "mock".to_string(),
            url: server.uri(),
            username: "root@pam".to_string(),
            password: secrecy::SecretString::new("test-pw".to_string().into_boxed_str()),
            realm: "pam".to_string(),
            insecure_skip_verify: true,
            ca_cert_pem: None,
        };
        let client = ProxmoxClient::new(config).await.unwrap();
        let upid: String = client
            .post("nodes/pve11/qemu/103/status/start")
            .await
            .unwrap();
        assert!(upid.starts_with("UPID:pve11:"));
    }
}

#[cfg(test)]
mod vm_action_integration_tests {
    //! End-to-end tests for the `vm_start`/`vm_stop`/etc. handlers.
    //!
    //! Spins up a wiremock Proxmox server, mounts login + action mocks,
    //! builds a router with real audit + state, and verifies:
    //! - the action reaches Proxmox with cookie + CSRF headers,
    //! - the response includes the UPID,
    //! - the audit log records the action.
    use super::*;
    use crate::audit::AuditStore;
    use crate::auth::{Claims, JwtService, UserStore};
    use crate::config::{AuthConfig, DatabaseConfig, LoggingConfig, ServerConfig};
    use crate::state::AppState;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    /// Mint a fresh Operator-role JWT for the test app's issuer/audience.
    fn operator_token(jwt: &std::sync::Arc<JwtService>) -> String {
        let now = chrono::Utc::now().timestamp();
        let claims = Claims {
            sub: "u-test".to_string(),
            username: "tester".to_string(),
            role: "operator".to_string(),
            iat: now,
            exp: now + 600,
        };
        jwt.encode(&claims).expect("encode token")
    }

    /// Build a `Request<Body>` with the `Authorization: Bearer *** header
    /// so protected routes accept it.
    fn authed_get(uri: &str, token: &str) -> Request<Body> {
        Request::builder()
            .uri(uri)
            .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap()
    }

    fn authed_post(uri: &str, token: &str) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap()
    }

    async fn setup_with_mock() -> (wiremock::MockServer, AppState, std::sync::Arc<AuditStore>) {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        // /access/ticket — accepts any POST.
        Mock::given(method("POST"))
            .and(path("/api2/json/access/ticket"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "ticket": "PVE:root@pam:testticket::SIG",
                    "csrf_token": "test-csrf",
                    "username": "root@pam",
                    "expires_at": 9_999_999_999_i64
                }
            })))
            .mount(&server)
            .await;

        // /cluster/resources?type=vm — two VMs.
        Mock::given(method("GET"))
            .and(path("/api2/json/cluster/resources"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "vmid": 103, "name": "web-1", "node": "pve11",
                        "status": "running", "cpu": 0.05, "cpus": 2.0,
                        "mem": 1_073_741_824_u64, "maxmem": 2_147_483_648_u64
                    },
                    {
                        "vmid": 104, "name": "db-1", "node": "pve12",
                        "status": "stopped", "cpu": 0.0, "cpus": 4.0,
                        "mem": 0_u64, "maxmem": 4_294_967_296_u64
                    }
                ]
            })))
            .mount(&server)
            .await;

        // /nodes/pve11/qemu/103/status/start — returns UPID.
        Mock::given(method("POST"))
            .and(path("/api2/json/nodes/pve11/qemu/103/status/start"))
            .and(header(
                "Cookie",
                "PVEAuthCookie=PVE:root@pam:testticket::SIG",
            ))
            .and(header("CSRFPreventionToken", "test-csrf"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": "UPID:pve11:00001234:00000000:60F0EEEE:qmstart:103:root@pam:"
            })))
            .mount(&server)
            .await;

        let config = ClusterConfig {
            name: "homelab".to_string(),
            url: server.uri(),
            username: "root@pam".to_string(),
            password: secrecy::SecretString::new("test-pw".to_string().into_boxed_str()),
            realm: "pam".to_string(),
            insecure_skip_verify: true,
            ca_cert_pem: None,
        };
        let client = ProxmoxClient::new(config).await.unwrap();
        let audit = std::sync::Arc::new(AuditStore::open_in_memory().unwrap());
        let app_cfg = crate::config::Config {
            server: ServerConfig {
                bind: "127.0.0.1:0".to_string(),
                workers: 0,
            },
            database: DatabaseConfig {
                path: ":memory:".to_string(),
                max_connections: 1,
                run_migrations: false,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "pretty".to_string(),
            },
            clusters: vec![],
            auth: AuthConfig::default(),
        };
        let priv_pem = include_bytes!("../../tests/fixtures/test_jwt_priv.pem");
        let pub_pem = include_bytes!("../../tests/fixtures/test_jwt_pub.pem");
        let jwt = JwtService::new(priv_pem, pub_pem, "test", "test").expect("test jwt");
        let state = AppState::new(app_cfg, vec![client], audit.clone(), jwt, UserStore::new());
        (server, state, audit)
    }

    #[tokio::test]
    async fn test_vm_detail_returns_404_for_missing() {
        let (_server, state, _audit) = setup_with_mock().await;
        let token = operator_token(&state.jwt);
        let app = crate::api::router(state);

        let resp = app
            .oneshot(authed_get("/api/v1/vms/homelab/999", &token))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_vm_detail_returns_row_for_existing() {
        let (_server, state, _audit) = setup_with_mock().await;
        let token = operator_token(&state.jwt);
        let app = crate::api::router(state);

        let resp = app
            .oneshot(authed_get("/api/v1/vms/homelab/103", &token))
            .await
            .unwrap();
        let status = resp.status();
        let body_bytes = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let body_str = String::from_utf8_lossy(&body_bytes);
        eprintln!("detail status={status} body={body_str}");
        assert_eq!(status, StatusCode::OK);
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(body["vmid"], 103);
        assert_eq!(body["name"], "web-1");
        assert_eq!(body["cluster"], "homelab");
        assert_eq!(body["status"], "running");
    }

    #[tokio::test]
    async fn test_vm_start_returns_upid_and_audits() {
        let (_server, state, audit) = setup_with_mock().await;
        let token = operator_token(&state.jwt);
        let app = crate::api::router(state);

        let resp = app
            .oneshot(authed_post("/api/v1/vms/homelab/pve11/103/start", &token))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body_bytes = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(body["vmid"], 103);
        assert_eq!(body["action"], "start");
        assert!(body["upid"].as_str().unwrap().starts_with("UPID:pve11:"));

        // Audit log captured the POST.
        assert_eq!(audit.count().unwrap(), 1, "expected exactly 1 audit row");
    }
}
