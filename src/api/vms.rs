//! VM list + detail + write endpoint handlers.
//!
//! Aggregation logic for `/api/v1/vms` lives here. Write endpoints
//! (`/start`, `/stop`, `/shutdown`, `/reboot`) translate user-facing actions
//! into Proxmox API calls and report the resulting UPID so callers can
//! poll `/tasks/{upid}/status` if they need completion confirmation.

use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::auth::{require_role, AuthContext, Role};
use crate::error::{AppError, AppResult};
use crate::proxmox::types::VmResource;
use crate::state::AppState;

/// Optional search query for `GET /api/v1/vms`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct VmsQuery {
    /// Filter VM names by this substring (case-insensitive contains match).
    /// When absent, all VMs are returned.
    #[serde(default)]
    pub search: Option<String>,
}

/// One VM row in the aggregate response, tagged with the cluster it came from.
#[derive(Debug, Clone, Serialize)]
pub struct VmRow {
    /// Cluster the VM lives in.
    pub cluster: String,
    /// Proxmox VMID (unique within a cluster).
    pub vmid: u32,
    /// VM/LXC name.
    pub name: String,
    /// Node currently hosting the VM.
    pub node: String,
    /// Status string from Proxmox (`running`, `stopped`, `paused`).
    pub status: String,
    /// CPU usage fraction `[0.0, 1.0]`, if reported.
    pub cpu: Option<f64>,
    /// Allocated CPU cores, if reported.
    pub cpus: Option<f64>,
    /// Used memory in bytes, if reported.
    pub mem: Option<u64>,
    /// Configured memory in bytes, if reported.
    pub maxmem: Option<u64>,
    /// Uptime in seconds, if reported.
    pub uptime: Option<u64>,
    /// Semicolon-separated tags, if any.
    pub tags: Option<String>,
}

impl From<(String, VmResource)> for VmRow {
    fn from((cluster, r): (String, VmResource)) -> Self {
        Self {
            cluster,
            vmid: r.vmid,
            name: r.name,
            node: r.node,
            status: r.status,
            cpu: r.cpu,
            cpus: r.cpus,
            mem: r.mem,
            maxmem: r.maxmem,
            uptime: r.uptime,
            tags: r.tags,
        }
    }
}

/// `GET /api/v1/vms` response payload.
#[derive(Debug, Clone, Serialize)]
pub struct VmsResponse {
    /// Aggregated VM list, one row per (cluster, vmid).
    pub vms: Vec<VmRow>,
    /// Per-cluster list errors (`cluster_name -> error_message`).
    /// Empty when every cluster returned successfully.
    pub errors: HashMap<String, String>,
}

/// `GET /api/v1/vms` — list VMs across every configured Proxmox cluster.
///
/// Supports an optional `?search=foo` query parameter that filters
/// results to VMs whose name contains the search string (case-insensitive).
pub async fn list_vms(
    State(state): State<AppState>,
    Query(query): Query<VmsQuery>,
) -> Json<VmsResponse> {
    use futures_util::future::join_all;

    let futs = state.clients().map(|c| {
        let name = c.name().to_string();
        async move {
            let result = c.list_vms().await;
            (name, result)
        }
    });

    let results = join_all(futs).await;

    let mut vms = Vec::new();
    let mut errors = HashMap::new();
    for (name, result) in results {
        match result {
            Ok(list) => {
                vms.extend(list.into_iter().map(|r| VmRow::from((name.clone(), r))));
            }
            Err(e) => {
                errors.insert(name, e.to_string());
            }
        }
    }

    // Apply optional search filter (case-insensitive name contains match).
    if let Some(ref search) = query.search {
        let lower_search = search.to_lowercase();
        vms.retain(|row| row.name.to_lowercase().contains(&lower_search));
    }

    Json(VmsResponse { vms, errors })
}

/// `GET /api/v1/vms/:cluster/:vmid` — VM detail (single VM).
///
/// Filters `/cluster/resources?type=vm` for the requested VMID. Returns
/// 404 if not found or the cluster is not configured.
pub async fn vm_detail(
    State(state): State<AppState>,
    Path((cluster, vmid)): Path<(String, u32)>,
) -> AppResult<Json<VmRow>> {
    let client = state
        .client(&cluster)
        .ok_or_else(|| AppError::NotFound(format!("cluster '{cluster}' not configured")))?;

    let vms = client.list_vms().await?;
    let row = vms
        .into_iter()
        .find(|r| r.vmid == vmid)
        .map(|r| VmRow::from((cluster.clone(), r)))
        .ok_or_else(|| AppError::NotFound(format!("VM {vmid} not found in {cluster}")))?;

    Ok(Json(row))
}

/// `GET /api/v1/vms/:cluster/:node/:vmid/config` — full VM configuration.
///
/// Distinct from `vm_detail` which returns live status from
/// `/cluster/resources?type=vm`. This endpoint calls
/// `/nodes/{node}/qemu/{vmid}/config` and returns the editable spec
/// (cores, memory, disks, boot order, …). The UI surfaces it in the
/// Config tab of the VM detail view.
pub async fn vm_config_handler(
    State(state): State<AppState>,
    Path((cluster, node, vmid)): Path<(String, String, u32)>,
) -> AppResult<Json<crate::proxmox::types::VmConfig>> {
    let client = state
        .client(&cluster)
        .ok_or_else(|| AppError::NotFound(format!("cluster '{cluster}' not configured")))?;

    let config = client.vm_config(&node, vmid).await?;
    Ok(Json(config))
}

/// Response from a state-changing VM action (start/stop/etc.).
#[derive(Debug, Clone, Serialize)]
pub struct VmActionResponse {
    /// VMID the action was applied to.
    pub vmid: u32,
    /// Action that was performed (e.g. `start`).
    pub action: String,
    /// Proxmox UPID (e.g. `UPID:pve11:00001234:...`) — caller can poll
    /// `/nodes/{node}/tasks/{upid}/status` for completion.
    pub upid: String,
}

/// Request body for `POST /api/v1/vms/:cluster/:node/:vmid/delete`.
///
/// All fields are optional; defaults match Proxmox's safe defaults.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DeleteVmRequest {
    /// Destroy disks in addition to removing the VM config. Default `false`
    /// (config-only removal — Proxmox's default).
    #[serde(default)]
    pub purge: bool,
    /// Force removal even if the VM is running (Proxmox will refuse
    /// `delete` on a running VM unless `force=true` is set). Default `false`.
    #[serde(default)]
    pub force: bool,
    /// Skip Proxmox's config lock (HA, replication, etc.). Default `false`.
    /// Operators should rarely set this; it's intended for clean-up after
    /// broken locks.
    #[serde(default)]
    pub skiplock: bool,
}

/// `POST /api/v1/vms/:cluster/:node/:vmid/:action`
/// VM action dispatcher (start | stop | shutdown | reboot | delete).
///
/// Requires `Operator` or higher role. Returns 401 if no/invalid token,
/// 403 if role is too low.
///
/// Returns the Proxmox UPID so the caller can poll for completion.
/// Allowed actions are whitelisted — anything else is `400 Bad Request`.
///
/// `delete` accepts a JSON body of [`DeleteVmRequest`] for
/// `purge`/`force`/`skiplock` options. Other actions ignore the body.
pub async fn vm_action_handler(
    State(state): State<AppState>,
    auth: AuthContext,
    Path((cluster, node, vmid, action)): Path<(String, String, u32, String)>,
    body: Option<Json<DeleteVmRequest>>,
) -> AppResult<Json<VmActionResponse>> {
    // RBAC: only Operator+ can mutate VMs. require_role returns a 403
    // Response on denial; we need to surface it as AppError::Forbidden
    // so it composes with the rest of the handler's error type.
    if let Err(resp) = require_role(&auth, Role::Operator) {
        let status = resp.status();
        let err = if status == axum::http::StatusCode::FORBIDDEN {
            AppError::Forbidden("operator role required".into())
        } else {
            AppError::Internal(format!("auth middleware returned {status}"))
        };
        return Err(err);
    }
    match action.as_str() {
        "start" | "stop" | "shutdown" | "reboot" => {
            vm_action(&state, &cluster, &node, vmid, &action).await
        }
        "delete" => {
            // Body is optional; an empty POST deletes the VM config
            // (Proxmox default). Without body, defaults from
            // DeleteVmRequest::default are used (purge=false).
            let opts = body.map(|Json(b)| b).unwrap_or_default();
            vm_delete(&state, &cluster, &node, vmid, opts).await
        }
        other => Err(AppError::BadRequest(format!(
            "unknown action '{other}'; expected start|stop|shutdown|reboot|delete"
        ))),
    }
}

/// Shared logic for all VM write actions (start/stop/shutdown/reboot).
///
/// Proxmox endpoint shape (per skill reference):
/// ```text
/// POST /api2/json/nodes/{node}/qemu/{vmid}/status/{action}
/// Cookie: PVEAuthCookie=...
/// CSRFPreventionToken: ...
///
/// Response: {"data": "UPID:pve11:00001234:..."}
/// ```
async fn vm_action(
    state: &AppState,
    cluster: &str,
    node: &str,
    vmid: u32,
    action: &str,
) -> AppResult<Json<VmActionResponse>> {
    let client = state
        .client(cluster)
        .ok_or_else(|| AppError::NotFound(format!("cluster '{cluster}' not configured")))?;

    let path = format!("nodes/{node}/qemu/{vmid}/status/{action}");
    let upid: String = client.post(&path).await?;

    Ok(Json(VmActionResponse {
        vmid,
        action: action.to_string(),
        upid,
    }))
}

/// Delete a VM.
///
/// Proxmox endpoint:
/// ```text
/// POST /api2/json/nodes/{node}/qemu/{vmid}?purge=0|1&force=0|1&skiplock=0|1
/// Cookie: PVEAuthCookie=...
/// CSRFPreventionToken: ...
///
/// Response: {"data": "UPID:pve11:00001234:..."}
/// ```
///
/// Note: Proxmox uses POST (not HTTP DELETE) for state-changing operations.
/// The query params control destroy-vs-remove and lock behavior.
async fn vm_delete(
    state: &AppState,
    cluster: &str,
    node: &str,
    vmid: u32,
    opts: DeleteVmRequest,
) -> AppResult<Json<VmActionResponse>> {
    let client = state
        .client(cluster)
        .ok_or_else(|| AppError::NotFound(format!("cluster '{cluster}' not configured")))?;

    let upid = client
        .delete_vm(node, vmid, opts.purge, opts.force, opts.skiplock)
        .await?;

    Ok(Json(VmActionResponse {
        vmid,
        action: "delete".to_string(),
        upid,
    }))
}

/// Request body for `POST /api/v1/vms/:cluster/:node/:vmid/migrate`.
#[derive(Debug, Clone, Deserialize)]
pub struct MigrateRequest {
    /// Target node to migrate the VM to.
    pub target: String,
    /// Whether to perform a live migration (online). Default `true`.
    #[serde(default = "default_online")]
    pub online: bool,
}

fn default_online() -> bool {
    true
}

/// `POST /api/v1/vms/:cluster/:node/:vmid/migrate` — migrate a VM to another node.
///
/// Requires `Operator` role or higher.
pub async fn migrate_vm_handler(
    State(state): State<AppState>,
    auth: AuthContext,
    Path((cluster, node, vmid)): Path<(String, String, u32)>,
    Json(body): Json<MigrateRequest>,
) -> AppResult<Json<VmActionResponse>> {
    if let Err(resp) = require_role(&auth, Role::Operator) {
        let status = resp.status();
        let err = if status == axum::http::StatusCode::FORBIDDEN {
            AppError::Forbidden("operator role required".into())
        } else {
            AppError::Internal(format!("auth middleware returned {status}"))
        };
        return Err(err);
    }

    let client = state
        .client(&cluster)
        .ok_or_else(|| AppError::NotFound(format!("cluster '{cluster}' not configured")))?;

    let upid = client.migrate_vm(&node, vmid, &body.target, body.online).await?;

    Ok(Json(VmActionResponse {
        vmid,
        action: "migrate".to_string(),
        upid,
    }))
}

// ── Bulk VM Operations ──────────────────────────────────────────────────

/// Reference to a single VM in a bulk action request.
#[derive(Debug, Clone, Deserialize)]
pub struct BulkVmRef {
    /// Cluster the VM belongs to.
    pub cluster: String,
    /// Node hosting the VM.
    pub node: String,
    /// VMID.
    pub vmid: u32,
}

/// Request body for bulk VM actions.
#[derive(Debug, Clone, Deserialize)]
pub struct BulkActionRequest {
    /// List of VMs to act on.
    pub vms: Vec<BulkVmRef>,
}

/// One row in a bulk action response.
#[derive(Debug, Clone, Serialize)]
pub struct BulkResultRow {
    /// Cluster the VM belongs to.
    pub cluster: String,
    /// Node hosting the VM.
    pub node: String,
    /// VMID.
    pub vmid: u32,
    /// Action that was applied.
    pub action: String,
    /// Status (`ok` or `error`).
    pub status: String,
    /// UPID or error message.
    pub message: String,
}

/// Response from a bulk action.
#[derive(Debug, Clone, Serialize)]
pub struct BulkActionResponse {
    /// Individual results per VM.
    pub results: Vec<BulkResultRow>,
}

/// Execute an action against a list of VMs concurrently using `join_all`.
async fn bulk_action(
    state: &AppState,
    action: &str,
    vms: Vec<BulkVmRef>,
) -> Json<BulkActionResponse> {
    use futures_util::future::join_all;

    let futs = vms.into_iter().map(|vm| {
        let cluster = vm.cluster.clone();
        let node = vm.node.clone();
        let action_str = action.to_string();
        async move {
            let client = match state.client(&cluster) {
                Some(c) => c,
                None => {
                    let err = format!("cluster '{cluster}' not configured");
                    return BulkResultRow {
                        cluster,
                        node: node.clone(),
                        vmid: vm.vmid,
                        action: action_str,
                        status: "error".to_string(),
                        message: err,
                    };
                }
            };

            let path = format!("nodes/{node}/qemu/{}/status/{action_str}", vm.vmid);
            match client.post::<String>(&path).await {
                Ok(upid) => BulkResultRow {
                    cluster,
                    node,
                    vmid: vm.vmid,
                    action: action_str,
                    status: "ok".to_string(),
                    message: upid,
                },
                Err(e) => BulkResultRow {
                    cluster,
                    node,
                    vmid: vm.vmid,
                    action: action_str,
                    status: "error".to_string(),
                    message: e.to_string(),
                },
            }
        }
    });

    let results = join_all(futs).await;
    Json(BulkActionResponse { results })
}

/// `POST /api/v1/vms/bulk/start` — start multiple VMs concurrently.
pub async fn bulk_start(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(body): Json<BulkActionRequest>,
) -> AppResult<Json<BulkActionResponse>> {
    if let Err(resp) = require_role(&auth, Role::Operator) {
        let status = resp.status();
        let err = if status == axum::http::StatusCode::FORBIDDEN {
            AppError::Forbidden("operator role required".into())
        } else {
            AppError::Internal(format!("auth middleware returned {status}"))
        };
        return Err(err);
    }
    Ok(bulk_action(&state, "start", body.vms).await)
}

/// `POST /api/v1/vms/bulk/stop` — stop multiple VMs concurrently.
pub async fn bulk_stop(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(body): Json<BulkActionRequest>,
) -> AppResult<Json<BulkActionResponse>> {
    if let Err(resp) = require_role(&auth, Role::Operator) {
        let status = resp.status();
        let err = if status == axum::http::StatusCode::FORBIDDEN {
            AppError::Forbidden("operator role required".into())
        } else {
            AppError::Internal(format!("auth middleware returned {status}"))
        };
        return Err(err);
    }
    Ok(bulk_action(&state, "stop", body.vms).await)
}

/// `POST /api/v1/vms/bulk/reboot` — reboot multiple VMs concurrently.
pub async fn bulk_reboot(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(body): Json<BulkActionRequest>,
) -> AppResult<Json<BulkActionResponse>> {
    if let Err(resp) = require_role(&auth, Role::Operator) {
        let status = resp.status();
        let err = if status == axum::http::StatusCode::FORBIDDEN {
            AppError::Forbidden("operator role required".into())
        } else {
            AppError::Internal(format!("auth middleware returned {status}"))
        };
        return Err(err);
    }
    Ok(bulk_action(&state, "reboot", body.vms).await)
}

/// `POST /api/v1/vms/bulk/delete` — delete multiple VMs concurrently.
pub async fn bulk_delete(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(body): Json<BulkActionRequest>,
) -> AppResult<Json<BulkActionResponse>> {
    if let Err(resp) = require_role(&auth, Role::Operator) {
        let status = resp.status();
        let err = if status == axum::http::StatusCode::FORBIDDEN {
            AppError::Forbidden("operator role required".into())
        } else {
            AppError::Internal(format!("auth middleware returned {status}"))
        };
        return Err(err);
    }
    // For bulk delete, we reuse the delete_vm client method per VM.
    use futures_util::future::join_all;

    let futs = body.vms.into_iter().map(|vm| {
        let cluster = vm.cluster.clone();
        let node = vm.node.clone();
        let state = state.clone();
        async move {
            let client = match state.client(&cluster) {
                Some(c) => c,
                None => {
                    let err = format!("cluster '{cluster}' not configured");
                    return BulkResultRow {
                        cluster,
                        node: node.clone(),
                        vmid: vm.vmid,
                        action: "delete".to_string(),
                        status: "error".to_string(),
                        message: err,
                    };
                }
            };

            match client.delete_vm(&node, vm.vmid, false, false, false).await {
                Ok(upid) => BulkResultRow {
                    cluster,
                    node,
                    vmid: vm.vmid,
                    action: "delete".to_string(),
                    status: "ok".to_string(),
                    message: upid,
                },
                Err(e) => BulkResultRow {
                    cluster,
                    node,
                    vmid: vm.vmid,
                    action: "delete".to_string(),
                    status: "error".to_string(),
                    message: e.to_string(),
                },
            }
        }
    });

    let results = join_all(futs).await;
    Ok(Json(BulkActionResponse { results }))
}

#[cfg(test)]
mod list_vms_contract_tests {
    //! Contract tests for `GET /api/v1/vms`.
    //!
    //! Verifies that the wire shape returned by the backend matches
    //! what the Alpine.js frontend expects in `ui/static/app.js` and
    //! `ui/index.html`:
    //!   - top-level keys: `vms` (Vec) and `errors` (HashMap)
    //!   - each row has: cluster, vmid, name, node, status, cpu (Option<f64>),
    //!     cpus, mem, maxmem, uptime, tags
    //!   - `vms` includes QEMU VMs (frontend does not filter by kind)
    //!   - auth required (401 without Bearer)
    //!
    //! The mock Proxmox server returns one QEMU VM (103, running)
    //! and one LXC container (201, running) — list_vms returns both
    //! because the upstream API has no kind filter at this layer
    //! (filtering is done client-side in the LXC endpoint).

    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use secrecy::SecretString;
    use tower::ServiceExt;

    use crate::audit::AuditStore;
    use crate::auth::{Claims, JwtService, UserStore};
    use crate::config::{
        AuthConfig, ClusterConfig, Config, DatabaseConfig, LoggingConfig, ServerConfig,
    };
    use crate::proxmox::ProxmoxClient;
    use crate::state::AppState;
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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

    fn authed_get(uri: &str, token: &str) -> Request<Body> {
        Request::builder()
            .uri(uri)
            .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap()
    }

    async fn setup_app() -> (wiremock::MockServer, axum::Router) {
        let server = MockServer::start().await;

        // /access/ticket — login (ProxmoxClient::new does not call this,
        // but we mount it anyway in case future changes pre-warm).
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

        // /cluster/resources?type=vm — both QEMU and LXC, all the fields
        // the frontend reads (cpu, cpus, mem, maxmem, uptime, tags).
        Mock::given(method("GET"))
            .and(path("/api2/json/cluster/resources"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "vmid": 103, "name": "web-1", "node": "pve11",
                        "type": "qemu", "status": "running",
                        "cpu": 0.05, "cpus": 2.0,
                        "mem": 1_073_741_824_u64, "maxmem": 2_147_483_648_u64,
                        "uptime": 86_400_u64,
                        "tags": "prod;web"
                    },
                    {
                        "vmid": 201, "name": "web-lxc", "node": "pve11",
                        "type": "lxc", "status": "running",
                        "cpu": 0.02, "cpus": 1.0,
                        "mem": 268_435_456_u64, "maxmem": 536_870_912_u64,
                        "uptime": 3_600_u64,
                        "tags": "staging"
                    },
                    {
                        "vmid": 999, "name": "old-vm", "node": "pve11",
                        "type": "qemu", "status": "stopped",
                        "cpu": null, "cpus": 1.0,
                        "mem": 0_u64, "maxmem": 1_073_741_824_u64,
                        "uptime": 0_u64,
                        "tags": null
                    }
                ]
            })))
            .mount(&server)
            .await;

        let config = ClusterConfig {
            name: "homelab".to_string(),
            url: server.uri(),
            username: "root@pam".to_string(),
            password: SecretString::new("test-pw".to_string().into_boxed_str()),
            realm: "pam".to_string(),
            insecure_skip_verify: true,
            ca_cert_pem: None,
        };
        let client = ProxmoxClient::new(config).await.unwrap();
        let audit = std::sync::Arc::new(AuditStore::open_in_memory().unwrap());
        let app_cfg = Config {
            server: ServerConfig {
                bind: "127.0.0.1:0".to_string(),
                workers: 0,
                tls: None,
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
            tracing: crate::observability::tracing::TracingConfig::default(),
            data_dir: "/tmp/moxui-test".to_string(),
            webhook: crate::config::WebhookConfig::default(),
        };
        let priv_pem = include_bytes!("../../tests/fixtures/test_jwt_priv.pem");
        let pub_pem = include_bytes!("../../tests/fixtures/test_jwt_pub.pem");
        let jwt = JwtService::new(priv_pem, pub_pem, "test", "test").expect("test jwt");
        let state = AppState::new(
            app_cfg,
            vec![client],
            audit,
            jwt,
            UserStore::new(),
            None,
            None,
            None,
            None,
            None,
            Arc::new(crate::dashboard_custom::DashboardCustomService::new_in_memory()),
        );
        let app = crate::api::router(state);
        (server, app)
    }

    #[tokio::test]
    async fn test_list_vms_requires_auth() {
        let (_server, app) = setup_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/vms")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_list_vms_response_matches_frontend_contract() {
        // We can't easily extract the JwtService from the router after
        // it's been wrapped, so we mint a fresh token with the same
        // keypair the router was built with.
        let (_server, app) = setup_app().await;
        let priv_pem = include_bytes!("../../tests/fixtures/test_jwt_priv.pem");
        let pub_pem = include_bytes!("../../tests/fixtures/test_jwt_pub.pem");
        let jwt = JwtService::new(priv_pem, pub_pem, "test", "test").expect("test jwt");
        let token = operator_token(&std::sync::Arc::new(jwt));

        let resp = app
            .oneshot(authed_get("/api/v1/vms", &token))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body_bytes = to_bytes(resp.into_body(), 16 * 1024).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        // Top-level shape: `{ vms: [...], errors: {} }`.
        assert!(body.get("vms").is_some(), "missing `vms` key");
        assert!(body.get("errors").is_some(), "missing `errors` key");
        assert!(body["vms"].is_array());
        assert!(body["errors"].is_object());
        assert_eq!(
            body["errors"].as_object().unwrap().len(),
            0,
            "no per-cluster errors expected in this mock"
        );

        let vms = body["vms"].as_array().unwrap();
        assert_eq!(vms.len(), 3, "expected 3 mock resources (2 qemu + 1 lxc)");

        // Every row has the fields the frontend reads.
        let required = [
            "cluster", "vmid", "name", "node", "status", "cpu", "cpus", "mem", "maxmem", "uptime",
            "tags",
        ];
        for row in vms {
            for field in required {
                assert!(row.get(field).is_some(), "row missing `{field}`: {row}");
            }
        }

        // Spot-check the running qemu VM row.
        let web1 = vms.iter().find(|r| r["vmid"] == 103).expect("web-1 row");
        assert_eq!(web1["name"], "web-1");
        assert_eq!(web1["cluster"], "homelab");
        assert_eq!(web1["status"], "running");
        // f64 — mock returns the literal 0.05; assert the value is in
        // a tight band rather than exact-compare to avoid the
        // clippy::float_cmp lint while still catching regressions.
        let cpu = web1["cpu"].as_f64().unwrap();
        assert!((cpu - 0.05).abs() < 1e-9, "cpu {cpu} should be 0.05");
        assert_eq!(web1["uptime"].as_u64().unwrap(), 86_400);
        assert_eq!(web1["tags"], "prod;web");

        // Spot-check the stopped VM (null fields preserved).
        let old = vms.iter().find(|r| r["vmid"] == 999).expect("old-vm row");
        assert_eq!(old["status"], "stopped");
        assert!(old["cpu"].is_null(), "stopped VM should have null cpu");
        assert!(old["tags"].is_null(), "untagged VM should have null tags");
    }

    #[tokio::test]
    async fn test_list_vms_includes_lxc_resources() {
        // The Alpine frontend does not filter by kind — list_vms returns
        // both qemu and lxc. Verify both are present so the UI table
        // shows them (and so the user can navigate to them).
        let (_server, app) = setup_app().await;
        let priv_pem = include_bytes!("../../tests/fixtures/test_jwt_priv.pem");
        let pub_pem = include_bytes!("../../tests/fixtures/test_jwt_pub.pem");
        let jwt = JwtService::new(priv_pem, pub_pem, "test", "test").expect("test jwt");
        let token = operator_token(&std::sync::Arc::new(jwt));

        let resp = app
            .oneshot(authed_get("/api/v1/vms", &token))
            .await
            .unwrap();
        let body_bytes = to_bytes(resp.into_body(), 16 * 1024).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        let vms = body["vms"].as_array().unwrap();
        let has_qemu = vms.iter().any(|r| r["vmid"] == 103);
        let has_lxc = vms.iter().any(|r| r["vmid"] == 201);
        assert!(has_qemu, "qemu VM 103 should be in response");
        assert!(
            has_lxc,
            "LXC 201 should also be in response (list_vms is unfiltered)"
        );
    }
}

// =====================================================================
// Day 12 — VM config endpoint tests (Overview/Config tab of detail view).
// =====================================================================

#[cfg(test)]
mod vm_config_contract_tests {
    //! Contract tests for `GET /api/v1/vms/:cluster/:node/:vmid/config`.
    //!
    //! Verifies the wire shape returned by the backend matches what
    //! the Alpine frontend's Config tab expects in
    //! `ui/static/app.js` + `ui/index.html`:
    //!   - top-level `VmConfig` shape (name, cores, memory, bios, …)
    //!   - auth required (401 without Bearer)
    //!   - 404 when the cluster is not configured
    //!
    //! The mock Proxmox server returns a single config for VMID 103
    //! with the fields the operator UI cares about; other fields are
    //! silently dropped (serde default behavior).

    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use secrecy::SecretString;
    use tower::ServiceExt;

    use crate::audit::AuditStore;
    use crate::auth::{Claims, JwtService, UserStore};
    use crate::config::{
        AuthConfig, ClusterConfig, Config, DatabaseConfig, LoggingConfig, ServerConfig,
    };
    use crate::proxmox::ProxmoxClient;
    use crate::state::AppState;
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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

    fn authed_get(uri: &str, token: &str) -> Request<Body> {
        Request::builder()
            .uri(uri)
            .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap()
    }

    async fn setup_app() -> (wiremock::MockServer, axum::Router) {
        let server = MockServer::start().await;

        // /access/ticket — login.
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

        // /nodes/pve11/qemu/103/config — VM 103 spec.
        Mock::given(method("GET"))
            .and(path("/api2/json/nodes/pve11/qemu/103/config"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "name": "web-1",
                    "description": "Primary web frontend",
                    "cores": 4,
                    "sockets": 1,
                    "memory": 4096,
                    "balloon": 0,
                    "boot": "order=scsi0",
                    "bios": "ovmf",
                    "machine": "pc-q35-8.1",
                    "scsihw": "virtio-scsi-pci",
                    "cpu": "host",
                    "tags": "prod;web",
                    "template": 0,
                    "onboot": 1,
                    "agent": 1
                }
            })))
            .mount(&server)
            .await;

        let config = ClusterConfig {
            name: "homelab".to_string(),
            url: server.uri(),
            username: "root@pam".to_string(),
            password: SecretString::new("test-pw".to_string().into_boxed_str()),
            realm: "pam".to_string(),
            insecure_skip_verify: true,
            ca_cert_pem: None,
        };
        let client = ProxmoxClient::new(config).await.unwrap();
        let audit = std::sync::Arc::new(AuditStore::open_in_memory().unwrap());
        let app_cfg = Config {
            server: ServerConfig {
                bind: "127.0.0.1:0".to_string(),
                workers: 0,
                tls: None,
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
            tracing: crate::observability::tracing::TracingConfig::default(),
            data_dir: "/tmp/moxui-test".to_string(),
            webhook: crate::config::WebhookConfig::default(),
        };
        let priv_pem = include_bytes!("../../tests/fixtures/test_jwt_priv.pem");
        let pub_pem = include_bytes!("../../tests/fixtures/test_jwt_pub.pem");
        let jwt = JwtService::new(priv_pem, pub_pem, "test", "test").expect("test jwt");
        let state = AppState::new(
            app_cfg,
            vec![client],
            audit,
            jwt,
            UserStore::new(),
            None,
            None,
            None,
            None,
            None,
            Arc::new(crate::dashboard_custom::DashboardCustomService::new_in_memory()),
        );
        let app = crate::api::router(state);
        (server, app)
    }

    #[tokio::test]
    async fn test_vm_config_requires_auth() {
        let (_server, app) = setup_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/vms/homelab/pve11/103/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_vm_config_response_matches_frontend_contract() {
        // The Alpine frontend's Config tab reads these fields:
        //   name, cores, sockets, memory, boot, bios, scsihw, cpu, tags,
        //   description, onboot, agent, template
        let (_server, app) = setup_app().await;
        let priv_pem = include_bytes!("../../tests/fixtures/test_jwt_priv.pem");
        let pub_pem = include_bytes!("../../tests/fixtures/test_jwt_pub.pem");
        let jwt = JwtService::new(priv_pem, pub_pem, "test", "test").expect("test jwt");
        let token = operator_token(&std::sync::Arc::new(jwt));

        let resp = app
            .oneshot(authed_get("/api/v1/vms/homelab/pve11/103/config", &token))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body_bytes = to_bytes(resp.into_body(), 16 * 1024).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        // Required fields the frontend reads (some are Option<T>, so we
        // assert they're present in the JSON shape rather than non-null —
        // `null` is a legitimate value).
        let required = [
            "name",
            "cores",
            "sockets",
            "memory",
            "boot",
            "bios",
            "scsihw",
            "cpu",
            "tags",
            "description",
            "onboot",
            "agent",
            "template",
        ];
        for field in required {
            assert!(body.get(field).is_some(), "config missing `{field}`");
        }

        // Spot-check concrete values.
        assert_eq!(body["name"], "web-1");
        assert_eq!(body["cores"], 4);
        assert_eq!(body["memory"], 4096);
        assert_eq!(body["bios"], "ovmf");
        assert_eq!(body["tags"], "prod;web");
        assert_eq!(body["template"], 0);
    }

    #[tokio::test]
    async fn test_vm_config_returns_404_for_unknown_cluster() {
        let (_server, app) = setup_app().await;
        let priv_pem = include_bytes!("../../tests/fixtures/test_jwt_priv.pem");
        let pub_pem = include_bytes!("../../tests/fixtures/test_jwt_pub.pem");
        let jwt = JwtService::new(priv_pem, pub_pem, "test", "test").expect("test jwt");
        let token = operator_token(&std::sync::Arc::new(jwt));

        let resp = app
            .oneshot(authed_get(
                "/api/v1/vms/UNKNOWN-CLUSTER/pve11/103/config",
                &token,
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
