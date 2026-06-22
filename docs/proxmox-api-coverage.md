# Proxmox VE API Coverage

> MoxUI's coverage of the [Proxmox VE API](https://pve.proxmox.com/pve-docs/api-viewer/).

**Legend:** ✅ Supported | 🚧 Planned | ❌ Not planned

---

## Node Endpoints

| Method | Path | Status | Notes |
|---|---|---|---|
| `GET` | `/nodes` | ✅ | Node list (via cluster resources) |
| `GET` | `/nodes/{node}/status` | ✅ | Via dashboard version/resource queries |
| `GET` | `/nodes/{node}/dns` | ❌ | Not planned for MVP |
| `GET` | `/nodes/{node}/time` | ❌ | Not planned for MVP |
| `GET` | `/nodes/{node}/syslog` | ❌ | Not planned for MVP |

---

## QEMU / VM Endpoints

| Method | Path | Status | Notes |
|---|---|---|---|
| `GET` | `/cluster/resources?type=vm` | ✅ | Cross-cluster VM list |
| `GET` | `/nodes/{node}/qemu` | ✅ | Via cluster resources |
| `GET` | `/nodes/{node}/qemu/{vmid}/config` | ✅ | VM configuration |
| `GET` | `/nodes/{node}/qemu/{vmid}/status/current` | ✅ | Single VM detail |
| `POST` | `/nodes/{node}/qemu/{vmid}/status/start` | ✅ | Start VM |
| `POST` | `/nodes/{node}/qemu/{vmid}/status/stop` | ✅ | Stop VM |
| `POST` | `/nodes/{node}/qemu/{vmid}/status/shutdown` | ✅ | Shutdown VM (ACPI) |
| `POST` | `/nodes/{node}/qemu/{vmid}/status/reboot` | ✅ | Reboot VM |
| `POST` | `/nodes/{node}/qemu/{vmid}/status/reset` | ❌ | Planned for v1.1 |
| `POST` | `/nodes/{node}/qemu/{vmid}/status/suspend` | ❌ | Planned for v1.1 |
| `POST` | `/nodes/{node}/qemu/{vmid}/status/resume` | ❌ | Planned for v1.1 |
| `DELETE` | `/nodes/{node}/qemu/{vmid}` | ✅ | Delete VM (with purge/force/skiplock options) |
| `PUT` | `/nodes/{node}/qemu` | ❌ | VM creation — planned for v1.1 |
| `POST` | `/nodes/{node}/qemu` | ❌ | VM cloning — planned for v1.1 |
| `POST` | `/nodes/{node}/qemu/{vmid}/migrate` | ❌ | Migration — planned for v1.1 |
| `POST` | `/nodes/{node}/qemu/{vmid}/template` | ❌ | Convert to template — planned for v1.1 |
| `POST` | `/nodes/{node}/qemu/{vmid}/snapshot` | ❌ | Snapshot — planned for v1.1 |
| `POST` | `/nodes/{node}/qemu/{vmid}/sendkey` | ❌ | Planned for v1.1 |
| `POST` | `/nodes/{node}/qemu/{vmid}/monitor` | ❌ | QEMU monitor access — not planned |
| `POST` | `/nodes/{node}/qemu/{vmid}/vncproxy` | ✅ | VNC console proxy (ticket-based) |
| `GET` | `/nodes/{node}/qemu/{vmid}/vncwebsocket` | 🚧 | WebSocket proxy (Phase 2 follow-up) |
| `PUT` | `/nodes/{node}/qemu/{vmid}/config` | ❌ | VM config update — planned for v1.1 |
| `GET` | `/nodes/{node}/qemu/{vmid}/rrddata` | ❌ | RRD stats — planned for v1.1 |

---

## LXC / Container Endpoints

| Method | Path | Status | Notes |
|---|---|---|---|
| `GET` | `/cluster/resources?type=lxc` | ✅ | Cross-cluster LXC list |
| `GET` | `/nodes/{node}/lxc` | ✅ | Via cluster resources |
| `GET` | `/nodes/{node}/lxc/{vmid}/status/current` | ✅ | Single LXC detail |
| `POST` | `/nodes/{node}/lxc/{vmid}/status/start` | 🚧 | Planned for v1.1 |
| `POST` | `/nodes/{node}/lxc/{vmid}/status/stop` | 🚧 | Planned for v1.1 |
| `POST` | `/nodes/{node}/lxc/{vmid}/status/shutdown` | 🚧 | Planned for v1.1 |
| `POST` | `/nodes/{node}/lxc/{vmid}/status/reboot` | 🚧 | Planned for v1.1 |
| `DELETE` | `/nodes/{node}/lxc/{vmid}` | 🚧 | Planned for v1.1 |
| `PUT` | `/nodes/{node}/lxc` | ❌ | LXC creation — planned for v1.1 |
| `PUT` | `/nodes/{node}/lxc/{vmid}/config` | ❌ | LXC config update — planned for v1.1 |

---

## Storage Endpoints

| Method | Path | Status | Notes |
|---|---|---|---|
| `GET` | `/storage` | ✅ | List all storage pools |
| `GET` | `/nodes/{node}/storage` | ✅ | Per-node storage |
| `GET` | `/nodes/{node}/storage/{storage}/content` | ✅ | List ISO/template images |
| `POST` | `/nodes/{node}/storage/{storage}/upload` | ❌ | Upload — planned for v1.1 |
| `DELETE` | `/nodes/{node}/storage/{storage}/content/{volid}` | ❌ | Delete content — planned for v1.1 |
| `POST` | `/nodes/{node}/storage/{storage}/download-url` | ❌ | Download URL — planned for v1.1 |

---

## Network Endpoints

| Method | Path | Status | Notes |
|---|---|---|---|
| `GET` | `/cluster/network` | ✅ | Cluster-level (Proxmox 8+) |
| `GET` | `/nodes/{node}/network` | ✅ | Per-node interface listing |
| `PUT` | `/nodes/{node}/network` | ❌ | Network config — not planned for MVP |
| `POST` | `/nodes/{node}/network` | ❌ | Not planned for MVP |

---

## Access / Auth Endpoints

| Method | Path | Status | Notes |
|---|---|---|---|
| `POST` | `/access/ticket` | ✅ | Proxmox auth (internal) |
| `PUT` | `/access/users` | ❌ | User management — planned for v1.1 |
| `GET` | `/access/users` | ❌ | Not planned (MoxUI manages its own users) |
| `GET` | `/access/domains` | ❌ | Not planned for MVP |

---

## Cluster Endpoints

| Method | Path | Status | Notes |
|---|---|---|---|
| `GET` | `/version` | ✅ | Version check (reachability probe) |
| `GET` | `/cluster/status` | ❌ | HA status — planned for v1.1 |
| `GET` | `/cluster/config` | ❌ | Cluster config — planned for v1.1 |
| `GET` | `/cluster/ha/status` | ❌ | HA resources — planned for v1.1 |
| `GET` | `/cluster/ha/groups` | ❌ | HA groups — planned for v1.1 |
| `GET` | `/cluster/replication` | ✅ | Replication — read list of replication jobs |
| `POST` | `/cluster/replication` | 🚧 | Replication — create a new replication job |
| `PUT` | `/cluster/replication/{id}` | 🚧 | Replication — update existing job |
| `DELETE` | `/cluster/replication/{id}` | 🚧 | Replication — delete a replication job |
| `POST` | `/cluster/replication/{id}/schedule_now` | 🚧 | Replication — trigger immediate replication |
| `GET` | `/cluster/replication/{id}/log` | 🚧 | Replication — fetch job log |
| `GET` | `/cluster/options` | ❌ | Cluster options — planned for v1.1 |
| `GET` | `/cluster/firewall` | ❌ | Firewall rules — planned for v1.1 |
| `GET` | `/cluster/log` | ❌ | Cluster log — planned for v1.1 |
| `GET` | `/cluster/tasks` | ❌ | Cluster tasks — planned for v1.1 |

---

## Task Endpoints

| Method | Path | Status | Notes |
|---|---|---|---|
| `GET` | `/nodes/{node}/tasks/{upid}/status` | ✅ | Task status polling |
| `GET` | `/nodes/{node}/tasks/{upid}/log` | ❌ | Task log — planned for v1.1 |
| `DELETE` | `/nodes/{node}/tasks/{upid}` | ❌ | Task cleanup — planned for v1.1 |

---

## MoxUI-Specific Endpoints

These are served directly by MoxUI, not proxied to Proxmox:

| Method | Path | Status | Description |
|---|---|---|---|
| `GET` | `/health` | ✅ | Detailed health JSON |
| `GET` | `/livez` | ✅ | Kubernetes liveness probe |
| `GET` | `/readyz` | ✅ | Kubernetes readiness probe |
| `GET` | `/metrics` | ✅ | Prometheus metrics |
| `POST` | `/api/v1/auth/login` | ✅ | Username/password login |
| `POST` | `/api/v1/auth/refresh` | ✅ | Refresh token rotation |
| `POST` | `/api/v1/auth/logout` | ✅ | Logout + token revocation |
| `POST` | `/api/v1/auth/2fa/complete` | ✅ | 2FA TOTP completion |
| `POST` | `/api/v1/auth/2fa/setup` | ✅ | Enable 2FA (generate secret) |
| `POST` | `/api/v1/auth/2fa/verify` | ✅ | Verify TOTP code |
| `POST` | `/api/v1/auth/2fa/disable` | ✅ | Disable 2FA |
| `POST` | `/api/v1/auth/oidc/login` | ✅ | Start OIDC SSO flow |
| `POST` | `/api/v1/auth/oidc/callback` | ✅ | Complete OIDC SSO flow |
| `POST` | `/api/v1/auth/webauthn/register/start` | ✅ | Start passkey registration |
| `POST` | `/api/v1/auth/webauthn/register/complete` | ✅ | Complete passkey registration |
| `POST` | `/api/v1/auth/webauthn/login/start` | ✅ | Start passkey login |
| `POST` | `/api/v1/auth/webauthn/login/complete` | ✅ | Complete passkey login |
| `GET` | `/api/v1/auth/me` | ✅ | Current user claims |
| `GET` | `/api/v1/dashboard` | ✅ | Aggregate cluster dashboard |
| `GET` | `/api/v1/vms` | ✅ | Cross-cluster VM list |
| `GET` | `/api/v1/vms/:cluster/:vmid` | ✅ | Single VM detail |
| `POST` | `/api/v1/vms/:cluster/:node/:vmid/:action` | ✅ | VM actions (start/stop/shutdown/reboot) |
| `DELETE` | `/api/v1/vms/:cluster/:node/:vmid` | ✅ | Delete VM |
| `GET` | `/api/v1/vms/:cluster/:node/:vmid/config` | ✅ | VM config |
| `POST` | `/api/v1/vms/:cluster/:node/:vmid/vnc/ticket` | ✅ | VNC console ticket |
| `GET` | `/api/v1/vms/:cluster/:node/:vmid/vnc/ws` | 🚧 | VNC WebSocket proxy (stub) |
| `GET` | `/api/v1/lxcs` | ✅ | Cross-cluster LXC list |
| `GET` | `/api/v1/lxcs/:cluster/:node/:vmid` | ✅ | Single LXC detail |
| `GET` | `/api/v1/storages` | ✅ | Storage pool list |
| `GET` | `/api/v1/storages/:cluster/:node/:storage/content` | ✅ | Storage content |
| `GET` | `/api/v1/networks` | ✅ | Network interface list |
| `GET` | `/api/v1/networks/:cluster/:node` | ✅ | Per-node network list |
| `GET` | `/api/v1/tasks/:cluster/:node/:upid` | ✅ | Task status polling |
| `GET` | `/api/v1/audit` | ✅ | Paginated audit log |
| `GET` | `/` | ✅ | Frontend SPA shell |
| `GET` | `/static/*` | ✅ | Embedded static assets |

---

## Coverage Summary

| Category | Total | Supported | Planned | Not Planned | Coverage |
|---|---|---|---|---|---|
| MoxUI API | 35 | 33 | 2 | 0 | **94%** |
| QEMU/VM | 18 | 8 | 5 | 5 | **44%** |
| LXC | 7 | 2 | 5 | 0 | **29%** |
| Storage | 5 | 2 | 3 | 0 | **40%** |
| Network | 4 | 2 | 0 | 2 | **50%** |
| Cluster | 9 | 1 | 8 | 0 | **11%** |
| Access | 4 | 1 | 1 | 2 | **25%** |
| **Total** | **82** | **49** | **24** | **9** | **60%** |

> Focused on **read operations + VM control** for v1.0. Write operations (create, update, delete) for LXC, storage, and advanced cluster features are planned for v1.1+.
