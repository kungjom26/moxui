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

    /// Alias for [`Self::url`] — kept separate because the VNC WS
    /// proxy uses it heavily and `base_url` reads more naturally in
    /// that context.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.config.url
    }

    /// Build a rustls `ClientConfig` that mirrors the Proxmox HTTP
    /// client's TLS settings (CA cert + insecure-skip-verify).
    ///
    /// Used by the VNC WebSocket proxy so the upstream TLS
    /// configuration matches what ProxmoxClient itself uses. If
    /// `insecure_skip_verify` is set, we install a no-op verifier
    /// (same trade-off as the HTTP client).
    ///
    /// NOTE on CA cert handling: we deliberately avoid
    /// `rustls-pemfile` (RUSTSEC-2025-0134 — unmaintained). The
    /// upstream CA must be in the system trust store, or the
    /// operator sets `insecure_skip_verify: true`. This is a
    /// conscious trade-off documented in `Cargo.toml`.
    #[must_use]
    pub fn tls_connector(&self) -> Option<Arc<rustls::ClientConfig>> {
        // rustls 0.23 needs an Arc<CryptoProvider> + an explicit root
        // store to transition through the builder state machine.
        // We use the platform/webpki roots for normal mode and
        // short-circuit to the no-op verifier when the operator has
        // opted into `insecure_skip_verify`.
        //
        // If no default provider is installed we fall back to aws_lc_rs.
        // We never want this to fail because the VNC WS upgrade
        // pathway is opt-in (operator must set
        // `auth.vnc_token_secret_pem_path` to enable it).
        let provider: Arc<rustls::crypto::CryptoProvider> =
            match rustls::crypto::CryptoProvider::get_default() {
                Some(arc) => arc.clone(),
                None => Arc::new(rustls::crypto::aws_lc_rs::default_provider()),
            };
        let builder = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .ok()?;
        let config = if self.config.insecure_skip_verify {
            builder
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(NoCertVerifier))
                .with_no_client_auth()
        } else {
            // Use the webpki-roots bundle. For private CAs operators
            // should either install the cert in the OS trust store or
            // set `insecure_skip_verify: true` for the cluster.
            let mut roots = rustls::RootCertStore::empty();
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            builder.with_root_certificates(roots).with_no_client_auth()
        };
        Some(Arc::new(config))
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

    /// Issue a POST with form-style query parameters (Proxmox's preferred
    /// shape for most state-changing endpoints — e.g. VM delete with
    /// `purge=1&force=1`).
    ///
    /// Query values are URL-encoded by reqwest. Returns the deserialized
    /// response data (typically a `String` UPID).
    pub async fn post_with_query<T>(
        &self,
        path: &str,
        params: Vec<(String, String)>,
    ) -> AppResult<T>
    where
        T: serde::de::DeserializeOwned + Default,
    {
        let ticket = self.current_ticket().await?;
        let url = self.build_url(path);
        let fut = self
            .http
            .post(&url)
            .query(&params)
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

    /// Same as `post_with_query` but without the `Default` bound.
    ///
    /// Needed for response types that wrap fields in `SecretString`
    /// (VNC proxy tickets) — `SecretString` deliberately doesn't
    /// implement `Default` so a missing ticket can't be silently
    /// represented as empty.
    pub async fn post_no_default<T>(
        &self,
        path: &str,
        params: Vec<(String, String)>,
    ) -> AppResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let ticket = self.current_ticket().await?;
        let url = self.build_url(path);
        let fut = self
            .http
            .post(&url)
            .query(&params)
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

    /// Delete a QEMU VM.
    ///
    /// Proxmox endpoint: `POST /nodes/{node}/qemu/{vmid}` (Proxmox uses
    /// POST for all state-changing operations, including delete, with
    /// `purge`, `force`, and `skiplock` as form params).
    ///
    /// Returns the Proxmox UPID (e.g. `UPID:pve11:00001234:...`) so the
    /// caller can poll the task status.
    pub async fn delete_vm(
        &self,
        node: &str,
        vmid: u32,
        purge: bool,
        force: bool,
        skiplock: bool,
    ) -> AppResult<String> {
        let path = format!("nodes/{node}/qemu/{vmid}");
        let params = vec![
            (
                "purge".to_string(),
                if purge { "1" } else { "0" }.to_string(),
            ),
            (
                "force".to_string(),
                if force { "1" } else { "0" }.to_string(),
            ),
            (
                "skiplock".to_string(),
                if skiplock { "1" } else { "0" }.to_string(),
            ),
        ];
        self.post_with_query(&path, params).await
    }

    /// Read a QEMU VM's full configuration.
    ///
    /// Proxmox endpoint: `GET /nodes/{node}/qemu/{vmid}/config`.
    /// Returns the editable spec (cores, memory, disks, NICs, boot
    /// order, …) — distinct from `/cluster/resources?type=vm` which
    /// returns live status (cpu%, mem used, uptime). Both are needed
    /// for the Day 12 detail view (Overview = status, Config = spec).
    pub async fn vm_config(
        &self,
        node: &str,
        vmid: u32,
    ) -> AppResult<crate::proxmox::types::VmConfig> {
        let path = format!("nodes/{node}/qemu/{vmid}/config");
        self.get(&path).await
    }

    /// Read a Proxmox task's status (used to track async actions).
    ///
    /// Proxmox endpoint: `GET /nodes/{node}/tasks/{upid}/status`.
    /// The `upid` returned from start/stop/etc. is opaque — to poll
    /// it we need the node (which is encoded inside the UPID as
    /// `UPID:<node>:<pid>:...` but we take it as a parameter so the
    /// caller can use the same node it used to fire the action).
    pub async fn task_status(
        &self,
        node: &str,
        upid: &str,
    ) -> AppResult<crate::proxmox::types::TaskStatus> {
        // UPIDs (`UPID:node:piddigit:starttime:type:id:user@realm`) are
        // URL-path-safe — every byte is in the unreserved set per
        // RFC 3986. We don't percent-encode because the URL crate's
        // `form_urlencoded` helper operates on `application/x-www-form-
        // urlencoded` body encoding (where `+` means space, etc.) and is
        // not the right tool for path segments.
        let path = format!("nodes/{node}/tasks/{upid}/status");
        self.get(&path).await
    }

    /// Request a VNC proxy ticket + port from Proxmox.
    ///
    /// Proxmox endpoint: `POST /nodes/{node}/qemu/{vmid}/vncproxy`.
    /// Returns a short-lived `ticket` (one-shot) and the `port` to
    /// connect the VNC WebSocket to. The ticket is consumed by the
    /// next WebSocket connection — do not reuse or log it.
    ///
    /// We don't model the response as a `VmConfig`-style struct
    /// because Proxmox returns the Proxmox-specific ticket format
    /// (`PVEVNC:{base64(...)})` and we want to pass the raw strings
    /// through without leaking them through Debug derives.
    pub async fn vnc_proxy(
        &self,
        node: &str,
        vmid: u32,
    ) -> AppResult<crate::proxmox::types::VncProxyTicket> {
        // Proxmox's vncproxy endpoint is POST with no body and returns
        // a JSON object — `post_no_default` fits perfectly with an
        // empty params vec. We can't use `post<T>` because it requires
        // `Default` and our ticket type wraps the field in `SecretString`
        // (which doesn't impl `Default`).
        let path = format!("nodes/{node}/qemu/{vmid}/vncproxy");
        self.post_no_default(&path, Vec::new()).await
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

    /// List all LXC containers across cluster/nodes (read-only, RBAC: Viewer+).
    ///
    /// Uses the same `/cluster/resources?type=vm` endpoint as [`Self::list_vms`]
    /// — the API returns both QEMU VMs and LXC containers in one payload, so we
    /// filter on the wire side to keep the client simple. Bounded by
    /// [`LIST_VMS_TIMEOUT_SECS`].
    pub async fn list_lxcs(&self) -> AppResult<Vec<crate::proxmox::types::VmResource>> {
        match tokio::time::timeout(
            Duration::from_secs(LIST_VMS_TIMEOUT_SECS),
            self.get::<Vec<crate::proxmox::types::VmResource>>("cluster/resources?type=vm"),
        )
        .await
        {
            Ok(Ok(mut all)) => {
                all.retain(|r| r.kind == "lxc");
                Ok(all)
            }
            Ok(Err(e)) => Err(e),
            Err(_elapsed) => Err(AppError::Proxmox(format!(
                "list_lxcs timed out after {LIST_VMS_TIMEOUT_SECS}s"
            ))),
        }
    }

    /// Fetch a single LXC container by (node, vmid). Bounded by
    /// [`LIST_VMS_TIMEOUT_SECS`].
    ///
    /// Returns `Ok(None)` if the API responds but the container is not found
    /// (treated as 404 upstream). Returns `Err` on transport/parse failure.
    pub async fn lxc_detail(
        &self,
        node: &str,
        vmid: u32,
    ) -> AppResult<Option<crate::proxmox::types::LxcStatus>> {
        let path = format!("nodes/{node}/lxc/{vmid}/status/current");
        match tokio::time::timeout(
            Duration::from_secs(LIST_VMS_TIMEOUT_SECS),
            self.get::<Option<crate::proxmox::types::LxcStatus>>(&path),
        )
        .await
        {
            Ok(Ok(detail)) => Ok(detail),
            Ok(Err(e)) => Err(e),
            Err(_elapsed) => Err(AppError::Proxmox(format!(
                "lxc_detail timed out after {LIST_VMS_TIMEOUT_SECS}s"
            ))),
        }
    }

    /// List storage pools on a specific node. Bounded by
    /// [`LIST_VMS_TIMEOUT_SECS`].
    pub async fn list_storages(
        &self,
        node: &str,
    ) -> AppResult<Vec<crate::proxmox::types::StorageResource>> {
        let path = format!("nodes/{node}/storage");
        match tokio::time::timeout(
            Duration::from_secs(LIST_VMS_TIMEOUT_SECS),
            self.get::<Vec<crate::proxmox::types::StorageResource>>(&path),
        )
        .await
        {
            Ok(result) => result,
            Err(_elapsed) => Err(AppError::Proxmox(format!(
                "list_storages timed out after {LIST_VMS_TIMEOUT_SECS}s"
            ))),
        }
    }

    /// List volume contents of a storage pool (e.g. ISO images, backup
    /// files, container templates). Bounded by [`LIST_VMS_TIMEOUT_SECS`].
    pub async fn storage_content(
        &self,
        node: &str,
        storage: &str,
    ) -> AppResult<Vec<crate::proxmox::types::StorageContent>> {
        let path = format!("nodes/{node}/storage/{storage}/content");
        match tokio::time::timeout(
            Duration::from_secs(LIST_VMS_TIMEOUT_SECS),
            self.get::<Vec<crate::proxmox::types::StorageContent>>(&path),
        )
        .await
        {
            Ok(result) => result,
            Err(_elapsed) => Err(AppError::Proxmox(format!(
                "storage_content timed out after {LIST_VMS_TIMEOUT_SECS}s"
            ))),
        }
    }

    /// List network interfaces on a specific node (bridges, bonds, VLANs,
    /// physical NICs, aliases). Bounded by [`LIST_VMS_TIMEOUT_SECS`].
    ///
    /// Proxmox endpoint: `GET /nodes/{node}/network`
    /// Returns a flat list of all interfaces — the `kind` field
    /// distinguishes them (`bridge`, `bond`, `eth`, `vlan`, `alias`,
    /// `OVSBridge`).
    pub async fn list_networks(
        &self,
        node: &str,
    ) -> AppResult<Vec<crate::proxmox::types::NodeNetwork>> {
        let path = format!("nodes/{node}/network");
        match tokio::time::timeout(
            Duration::from_secs(LIST_VMS_TIMEOUT_SECS),
            self.get::<Vec<crate::proxmox::types::NodeNetwork>>(&path),
        )
        .await
        {
            Ok(result) => result,
            Err(_elapsed) => Err(AppError::Proxmox(format!(
                "list_networks timed out after {LIST_VMS_TIMEOUT_SECS}s"
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
    use crate::config::{AuthConfig, LoggingConfig, ServerConfig};
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

    #[allow(clippy::too_many_lines)]
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

        // /cluster/resources?type=vm — one QEMU VM + one LXC container.
        // Used by both list_vms (unfiltered) and list_lxcs (filter kind == "lxc").
        Mock::given(method("GET"))
            .and(path("/api2/json/cluster/resources"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "vmid": 103, "name": "web-1", "node": "pve11",
                        "type": "qemu", "status": "running",
                        "cpu": 0.05, "cpus": 2.0,
                        "mem": 1_073_741_824_u64, "maxmem": 2_147_483_648_u64
                    },
                    {
                        "vmid": 201, "name": "web-lxc", "node": "pve11",
                        "type": "lxc", "status": "running",
                        "cpu": 0.02, "cpus": 1.0,
                        "mem": 268_435_456_u64, "maxmem": 536_870_912_u64,
                        "template": 0
                    }
                ]
            })))
            .mount(&server)
            .await;

        // /nodes/pve11/lxc/201/status/current — LXC detail.
        Mock::given(method("GET"))
            .and(path("/api2/json/nodes/pve11/lxc/201/status/current"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "vmid": 201, "name": "web-lxc", "node": "pve11",
                    "status": "running", "cpu": 0.02, "cpus": 1.0,
                    "mem": 268_435_456_u64, "maxmem": 536_870_912_u64,
                    "template": 0
                }
            })))
            .mount(&server)
            .await;

        // /storage — cluster-level storage aggregation.
        Mock::given(method("GET"))
            .and(path("/api2/json/storage"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "storage": "local", "type": "dir",
                        "total": 100_u64, "used": 25_u64, "avail": 75_u64,
                        "used_fraction": 0.25, "enabled": 1, "shared": 0,
                        "content": "iso,vztmpl,backup"
                    },
                    {
                        "storage": "ceph-pool", "type": "rbd",
                        "total": 10_000_u64, "used": 4_000_u64, "avail": 6_000_u64,
                        "used_fraction": 0.4, "enabled": 1, "shared": 1,
                        "content": "images,rootdir"
                    }
                ]
            })))
            .mount(&server)
            .await;

        // /nodes/pve11/storage/local/content — storage content listing.
        Mock::given(method("GET"))
            .and(path("/api2/json/nodes/pve11/storage/local/content"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "volid": "local:iso/debian-12.iso",
                        "storage": "local", "content": "iso",
                        "volid_name": "debian-12.iso", "size": 800_000_000_u64,
                        "format": "iso", "ctime": 1_700_000_000_i64
                    },
                    {
                        "volid": "local:vztmpl/debian-12-standard.tar.zst",
                        "storage": "local", "content": "vztmpl",
                        "volid_name": "debian-12-standard.tar.zst",
                        "size": 100_000_000_u64, "format": "tgz",
                        "ctime": 1_700_000_000_i64
                    }
                ]
            })))
            .mount(&server)
            .await;

        // /cluster/network — cluster-level network list (used by list_networks).
        Mock::given(method("GET"))
            .and(path("/api2/json/network"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "iface": "vmbr0", "type": "bridge", "active": 1,
                        "address": "10.10.11.11/24", "gateway": "10.10.11.254",
                        "bridge_ports": "eno1", "autostart": 1,
                        "comments": "primary bridge"
                    },
                    {
                        "iface": "eno1", "type": "eth", "active": 1,
                        "autostart": 1
                    },
                    {
                        "iface": "eno1.18", "type": "vlan", "active": 1,
                        "address": "10.10.18.11/24",
                        "iface_vlan_raw_device": "eno1", "vlan_id": 18,
                        "autostart": 1
                    }
                ]
            })))
            .mount(&server)
            .await;

        // /nodes/pve11/network — per-node network list (used by node_networks).
        Mock::given(method("GET"))
            .and(path("/api2/json/nodes/pve11/network"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "iface": "vmbr0", "type": "bridge", "active": 1,
                        "address": "10.10.11.11/24", "gateway": "10.10.11.254",
                        "bridge_ports": "eno1", "autostart": 1
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

        // /nodes/pve11/qemu/103 — DELETE (POST with purge/force/skiplock).
        // Matches any query params (wiremock's `path` matcher ignores them).
        Mock::given(method("POST"))
            .and(path("/api2/json/nodes/pve11/qemu/103"))
            .and(header(
                "Cookie",
                "PVEAuthCookie=PVE:root@pam:testticket::SIG",
            ))
            .and(header("CSRFPreventionToken", "test-csrf"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": "UPID:pve11:00001235:00000000:60F0EEEE:qmdestroy:103:root@pam:"
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
                tls: None,
            },
            database: crate::config::DatabaseConfig {
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
        let state = AppState::new(
            app_cfg,
            vec![client],
            audit.clone(),
            jwt,
            UserStore::new(),
            None,
        );
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

    /// Build a POST with an optional JSON body and Bearer auth header.
    fn authed_post_json(uri: &str, token: &str, body: &str) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
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

    // ---- Day 8: VM delete (with body options) ----

    #[tokio::test]
    async fn test_vm_delete_with_body_returns_upid_and_audits() {
        let (_server, state, audit) = setup_with_mock().await;
        let token = operator_token(&state.jwt);
        let app = crate::api::router(state);

        // Body specifies purge=true, force=true, skiplock=false.
        let body = r#"{"purge":true,"force":true,"skiplock":false}"#;
        let resp = app
            .oneshot(authed_post_json(
                "/api/v1/vms/homelab/pve11/103/delete",
                &token,
                body,
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body_bytes = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(body["vmid"], 103);
        assert_eq!(body["action"], "delete");
        assert!(body["upid"].as_str().unwrap().starts_with("UPID:pve11:"));

        // Audit log captured the POST.
        assert_eq!(audit.count().unwrap(), 1, "expected exactly 1 audit row");
    }

    #[tokio::test]
    async fn test_vm_delete_with_empty_body_uses_defaults() {
        // No body — DeleteVmRequest::default() gives purge=false, force=false,
        // skiplock=false. The mock matches any query params so the call
        // succeeds. We just need to verify the empty-body path doesn't 400.
        let (_server, state, _audit) = setup_with_mock().await;
        let token = operator_token(&state.jwt);
        let app = crate::api::router(state);

        let resp = app
            .oneshot(authed_post("/api/v1/vms/homelab/pve11/103/delete", &token))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body_bytes = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(body["action"], "delete");
    }

    #[tokio::test]
    async fn test_vm_delete_rejects_viewer_role() {
        let (_server, state, _audit) = setup_with_mock().await;
        let token = viewer_token(&state.jwt);
        let app = crate::api::router(state);

        let resp = app
            .oneshot(authed_post("/api/v1/vms/homelab/pve11/103/delete", &token))
            .await
            .unwrap();
        // Viewer cannot perform any state-changing action.
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_vm_unknown_action_returns_400() {
        let (_server, state, _audit) = setup_with_mock().await;
        let token = operator_token(&state.jwt);
        let app = crate::api::router(state);

        let resp = app
            .oneshot(authed_post(
                "/api/v1/vms/homelab/pve11/103/snapshot",
                &token,
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    // ---- Day 5: LXC + storage read endpoints ----

    fn viewer_token(jwt: &crate::auth::JwtService) -> String {
        use crate::auth::{Claims, Role};
        let now = chrono::Utc::now().timestamp();
        let claims = Claims {
            sub: "viewer".to_string(),
            username: "viewer".to_string(),
            role: Role::Viewer.to_string(),
            exp: now + 300,
            iat: now,
        };
        jwt.encode(&claims).unwrap()
    }

    #[tokio::test]
    async fn test_list_lxcs_filters_out_qemu_vms() {
        let (_server, state, _audit) = setup_with_mock().await;
        let token = viewer_token(&state.jwt);
        let app = crate::api::router(state);

        let resp = app
            .oneshot(authed_get("/api/v1/lxcs", &token))
            .await
            .unwrap();
        let status = resp.status();
        let body_bytes = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        assert_eq!(
            status,
            StatusCode::OK,
            "body={}",
            String::from_utf8_lossy(&body_bytes)
        );
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        let lxcs = body["lxcs"].as_array().unwrap();
        assert_eq!(lxcs.len(), 1, "expected only the lxc, not the qemu vm");
        assert_eq!(lxcs[0]["vmid"], 201);
        assert_eq!(lxcs[0]["name"], "web-lxc");
        assert!(body["errors"].as_object().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_lxc_detail_returns_row_for_existing() {
        let (_server, state, _audit) = setup_with_mock().await;
        let token = viewer_token(&state.jwt);
        let app = crate::api::router(state);

        let resp = app
            .oneshot(authed_get("/api/v1/lxcs/homelab/pve11/201", &token))
            .await
            .unwrap();
        let status = resp.status();
        let body_bytes = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        assert_eq!(
            status,
            StatusCode::OK,
            "body={}",
            String::from_utf8_lossy(&body_bytes)
        );
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(body["vmid"], 201);
        assert_eq!(body["name"], "web-lxc");
        assert_eq!(body["status"], "running");
    }

    #[tokio::test]
    async fn test_list_storages_returns_aggregated_rows() {
        let (_server, state, _audit) = setup_with_mock().await;
        let token = viewer_token(&state.jwt);
        let app = crate::api::router(state);

        let resp = app
            .oneshot(authed_get("/api/v1/storages", &token))
            .await
            .unwrap();
        let status = resp.status();
        let body_bytes = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        assert_eq!(
            status,
            StatusCode::OK,
            "body={}",
            String::from_utf8_lossy(&body_bytes)
        );
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        let storages = body["storages"].as_array().unwrap();
        assert_eq!(storages.len(), 2, "expected local + ceph-pool");
        let names: Vec<&str> = storages
            .iter()
            .map(|s| s["storage"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"local"));
        assert!(names.contains(&"ceph-pool"));
    }

    #[tokio::test]
    async fn test_storage_content_returns_volumes() {
        let (_server, state, _audit) = setup_with_mock().await;
        let token = viewer_token(&state.jwt);
        let app = crate::api::router(state);

        let resp = app
            .oneshot(authed_get(
                "/api/v1/storages/homelab/pve11/local/content",
                &token,
            ))
            .await
            .unwrap();
        let status = resp.status();
        let body_bytes = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        assert_eq!(
            status,
            StatusCode::OK,
            "body={}",
            String::from_utf8_lossy(&body_bytes)
        );
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        let items = body.as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["volid"], "local:iso/debian-12.iso");
        assert_eq!(items[0]["content"], "iso");
    }

    #[tokio::test]
    async fn test_lxc_endpoint_requires_auth() {
        let (_server, state, _audit) = setup_with_mock().await;
        let app = crate::api::router(state);

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("GET")
                    .uri("/api/v1/lxcs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    // ---- Day 9: Network read endpoints ----

    #[tokio::test]
    async fn test_list_networks_aggregates_bridges_vlans() {
        let (_server, state, _audit) = setup_with_mock().await;
        let token = viewer_token(&state.jwt);
        let app = crate::api::router(state);

        let resp = app
            .oneshot(authed_get("/api/v1/networks", &token))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body_bytes = axum::body::to_bytes(resp.into_body(), 8192).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        let nets = body["networks"].as_array().expect("networks array");
        assert_eq!(
            nets.len(),
            3,
            "expected 3 mock interfaces (bridge/eth/vlan)"
        );

        // Bridge row.
        let bridge = nets
            .iter()
            .find(|n| n["iface"] == "vmbr0")
            .expect("vmbr0 in mock");
        // The aggregated handler returns NetworkRow (with `kind` field,
        // not the wire-shape `type`).
        assert_eq!(bridge["kind"], "bridge");
        assert_eq!(bridge["cluster"], "homelab");
        assert_eq!(bridge["address"], "10.10.11.11/24");
        assert_eq!(bridge["bridge_ports"], "eno1");

        // VLAN row.
        let vlan = nets
            .iter()
            .find(|n| n["iface"] == "eno1.18")
            .expect("eno1.18 in mock");
        assert_eq!(vlan["kind"], "vlan");
        assert_eq!(vlan["vlan_id"], 18);
        assert_eq!(vlan["iface_vlan_raw_device"], "eno1");

        // Errors map is empty.
        assert!(body["errors"].as_object().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_node_networks_returns_per_node_list() {
        let (_server, state, _audit) = setup_with_mock().await;
        let token = viewer_token(&state.jwt);
        let app = crate::api::router(state);

        let resp = app
            .oneshot(authed_get("/api/v1/networks/homelab/pve11", &token))
            .await
            .unwrap();
        let status = resp.status();
        let body_bytes = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        assert_eq!(
            status,
            StatusCode::OK,
            "body={}",
            String::from_utf8_lossy(&body_bytes)
        );
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        let nets = body.as_array().expect("array of NodeNetwork");
        assert_eq!(nets.len(), 1, "expected 1 per-node interface, body={body}");
        assert_eq!(nets[0]["iface"], "vmbr0", "body={body}");
        // The `kind` field is renamed to `type` in JSON (matches Proxmox's
        // wire shape — Rust-side struct uses `kind` for ergonomics).
        assert_eq!(nets[0]["type"], "bridge", "body={body}");
    }

    #[tokio::test]
    async fn test_networks_endpoint_requires_auth() {
        let (_server, state, _audit) = setup_with_mock().await;
        let app = crate::api::router(state);

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("GET")
                    .uri("/api/v1/networks")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_node_networks_returns_404_for_unknown_cluster() {
        let (_server, state, _audit) = setup_with_mock().await;
        let token = viewer_token(&state.jwt);
        let app = crate::api::router(state);

        let resp = app
            .oneshot(authed_get("/api/v1/networks/nonexistent/pve11", &token))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}

// ── TLS helpers ───────────────────────────────────────────────────

/// rustls server-cert verifier that accepts every certificate.
///
/// Only installed when the operator has set `insecure_skip_verify: true`
/// on the cluster config — same trade-off the upstream Proxmox HTTP
/// client makes, so VNC works in lab/test envs where TLS isn't fully
/// set up. Real clusters should leave `insecure_skip_verify: false`
/// and rely on the system trust roots.
#[derive(Debug)]
pub struct NoCertVerifier;

impl rustls::client::danger::ServerCertVerifier for NoCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::aws_lc_rs::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}
