# Proxmox VE API Coverage

> MoxUI's coverage of the [Proxmox VE API](https://pve.proxmox.com/pve-docs/api-viewer/).

**Legend:** ✅ Supported | 🚧 Planned | ❌ Not planned

**Current: v2.0.0** — Phase 6 complete ✅

---

## Node Endpoints

| Method | Path | Status | Notes |
|---|---|---|---|
| `GET` | `/nodes` | ✅ | Node list (via cluster resources) |
| `GET` | `/nodes/{node}/status` | ✅ | Via dashboard version/resource queries |
| `GET` | `/nodes/{node}/dns` | ❌ | Not planned |
| `GET` | `/nodes/{node}/time` | ❌ | Not planned |
| `GET` | `/nodes/{node}/syslog` | ❌ | Not planned |

---

## QEMU / VM Endpoints

| Method | Path | Status | Notes |
|---|---|---|---|
| `GET` | `/cluster/resources?type=vm` | ✅ | Cross-cluster VM list |
| `GET` | `/nodes/{node}/qemu` | ✅ | Via cluster resources |
| `GET` | `/nodes/{node}/qemu/{vmid}/config` | ✅ | VM configuration |
| `PUT` | `/nodes/{node}/qemu/{vmid}/config` | ✅ | VM config update |
| `GET` | `/nodes/{node}/qemu/{vmid}/status/current` | ✅ | Single VM detail |
| `POST` | `/nodes/{node}/qemu/{vmid}/status/start` | ✅ | Start VM |
| `POST` | `/nodes/{node}/qemu/{vmid}/status/stop` | ✅ | Stop VM |
| `POST` | `/nodes/{node}/qemu/{vmid}/status/shutdown` | ✅ | Shutdown VM (ACPI) |
| `POST` | `/nodes/{node}/qemu/{vmid}/status/reboot` | ✅ | Reboot VM |
| `POST` | `/nodes/{node}/qemu/{vmid}/status/reset` | 🚧 | Planned for v3.0 |
| `POST` | `/nodes/{node}/qemu/{vmid}/status/suspend` | 🚧 | Planned for v3.0 |
| `POST` | `/nodes/{node}/qemu/{vmid}/status/resume` | 🚧 | Planned for v3.0 |
| `DELETE` | `/nodes/{node}/qemu/{vmid}` | ✅ | Delete VM |
| `PUT` | `/nodes/{node}/qemu` | ✅ | VM creation |
| `POST` | `/nodes/{node}/qemu` | ✅ | VM cloning |
| `POST` | `/nodes/{node}/qemu/{vmid}/migrate` | ✅ | Live migration |
| `POST` | `/nodes/{node}/qemu/{vmid}/template` | 🚧 | Planned for v3.0 |
| `POST` | `/nodes/{node}/qemu/{vmid}/snapshot` | ✅ | Create snapshot |
| `GET` | `/nodes/{node}/qemu/{vmid}/snapshot` | ✅ | List snapshots |
| `DELETE` | `/nodes/{node}/qemu/{vmid}/snapshot/{snapname}` | ✅ | Delete snapshot |
| `POST` | `/nodes/{node}/qemu/{vmid}/snapshot/{snapname}/rollback` | ✅ | Rollback snapshot |
| `POST` | `/nodes/{node}/qemu/{vmid}/sendkey` | 🚧 | Planned for v3.0 |
| `POST` | `/nodes/{node}/qemu/{vmid}/monitor` | ❌ | Not planned |
| `POST` | `/nodes/{node}/qemu/{vmid}/vncproxy` | ✅ | VNC console proxy |
| `GET` | `/nodes/{node}/qemu/{vmid}/vncwebsocket` | ✅ | VNC WebSocket proxy |
| `GET` | `/nodes/{node}/qemu/{vmid}/rrddata` | 🚧 | Planned for v3.0 |
| `POST` | `/nodes/{node}/qemu/{vmid}/resize` | ✅ | Disk resize |

---

## LXC / Container Endpoints

| Method | Path | Status | Notes |
|---|---|---|---|
| `GET` | `/cluster/resources?type=lxc` | ✅ | Cross-cluster LXC list |
| `GET` | `/nodes/{node}/lxc` | ✅ | Via cluster resources |
| `GET` | `/nodes/{node}/lxc/{vmid}/status/current` | ✅ | Single LXC detail |
| `POST` | `/nodes/{node}/lxc/{vmid}/status/start` | ✅ | Start LXC |
| `POST` | `/nodes/{node}/lxc/{vmid}/status/stop` | ✅ | Stop LXC |
| `POST` | `/nodes/{node}/lxc/{vmid}/status/shutdown` | ✅ | Shutdown LXC |
| `POST` | `/nodes/{node}/lxc/{vmid}/status/reboot` | ✅ | Reboot LXC |
| `DELETE` | `/nodes/{node}/lxc/{vmid}` | ✅ | Delete LXC |
| `PUT` | `/nodes/{node}/lxc` | 🚧 | LXC creation |
| `PUT` | `/nodes/{node}/lxc/{vmid}/config` | 🚧 | LXC config update |

---

## Storage Endpoints

| Method | Path | Status | Notes |
|---|---|---|---|
| `GET` | `/storage` | ✅ | List all storage pools |
| `GET` | `/nodes/{node}/storage` | ✅ | Per-node storage |
| `GET` | `/nodes/{node}/storage/{storage}/content` | ✅ | List ISO/template images |
| `POST` | `/nodes/{node}/storage/{storage}/upload` | ✅ | Upload ISO/template |
| `DELETE` | `/nodes/{node}/storage/{storage}/content/{volid}` | ✅ | Delete content |

---

## Network Endpoints

| Method | Path | Status | Notes |
|---|---|---|---|
| `GET` | `/nodes/{node}/network` | ✅ | Per-node interface listing |
| `GET` | `/cluster/network` | ✅ | Cluster-level (Proxmox 8+) |
| `GET` | `/nodes/{node}/network/{iface}` | ✅ | Bridge detail |
| `PUT` | `/nodes/{node}/network` | 🚧 | Network config |
| `POST` | `/nodes/{node}/network` | 🚧 | Network config |

---

## Access / Auth Endpoints

| Method | Path | Status | Notes |
|---|---|---|---|
| `POST` | `/access/ticket` | ✅ | Proxmox auth (internal) |
| `GET` | `/access/users` | ❌ | MoxUI manages its own users |

---

## Cluster Endpoints

| Method | Path | Status | Notes |
|---|---|---|---|
| `GET` | `/version` | ✅ | Version check |
| `GET` | `/cluster/status` | 🚧 | HA status |
| `GET` | `/cluster/config` | 🚧 | Cluster config |
| `GET` | `/cluster/ha/status` | ✅ | HA resources status |
| `GET` | `/cluster/ha/groups` | ✅ | HA groups list |
| `POST` | `/cluster/ha/groups` | ✅ | HA groups create |
| `DELETE` | `/cluster/ha/groups/{group}` | ✅ | HA groups delete |
| `GET` | `/cluster/replication` | ✅ | List replication jobs |
| `POST` | `/cluster/replication` | ✅ | Create replication job |
| `DELETE` | `/cluster/replication/{id}` | ✅ | Delete replication job |
| `GET` | `/cluster/replication/{id}/log` | ✅ | Job status/log |
| `GET` | `/cluster/ceph/status` | ✅ | Ceph cluster status |
| `GET` | `/cluster/ceph/pool` | ✅ | Ceph pool list |
| `GET` | `/cluster/sdn/zones` | ✅ | SDN zones list |
| `GET` | `/cluster/sdn/vnets` | ✅ | SDN VNets list |
| `GET` | `/cluster/firewall/rules` | ✅ | Firewall rules |
| `GET` | `/cluster/options` | 🚧 | Cluster options |
| `GET` | `/cluster/log` | 🚧 | Cluster log |
| `GET` | `/cluster/tasks` | 🚧 | Cluster tasks |

---

## Task Endpoints

| Method | Path | Status | Notes |
|---|---|---|---|
| `GET` | `/nodes/{node}/tasks/{upid}/status` | ✅ | Task status polling |
| `GET` | `/nodes/{node}/tasks/{upid}/log` | 🚧 | Planned for v3.0 |
| `DELETE` | `/nodes/{node}/tasks/{upid}` | 🚧 | Planned for v3.0 |

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
| `POST` | `/api/v1/auth/ldap/login` | ✅ | LDAP/AD login |
| `POST` | `/api/v1/auth/refresh` | ✅ | Refresh token rotation |
| `POST` | `/api/v1/auth/logout` | ✅ | Logout + token revocation |
| `POST` | `/api/v1/auth/2fa/complete` | ✅ | 2FA TOTP completion |
| `POST` | `/api/v1/auth/2fa/setup` | ✅ | Enable 2FA |
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
| `GET` | `/api/v1/dashboard/custom` | ✅ | Get custom dashboard layout |
| `POST` | `/api/v1/dashboard/custom` | ✅ | Save custom dashboard layout |
| `GET` | `/api/v1/dashboard/custom/widget-types` | ✅ | List widget types |
| `GET` | `/api/v1/vms` | ✅ | Cross-cluster VM list |
| `GET` | `/api/v1/vms/:cluster/:vmid` | ✅ | Single VM detail |
| `POST` | `/api/v1/vms/:cluster/:node/:vmid/:action` | ✅ | VM actions |
| `DELETE` | `/api/v1/vms/:cluster/:node/:vmid` | ✅ | Delete VM |
| `GET` | `/api/v1/vms/:cluster/:node/:vmid/config` | ✅ | VM config (GET + PUT) |
| `PUT` | `/api/v1/vms/:cluster/:node/:vmid/config` | ✅ | VM config update |
| `POST` | `/api/v1/vms/:cluster/:node/create` | ✅ | VM creation |
| `POST` | `/api/v1/vms/:cluster/:node/:vmid/clone` | ✅ | VM clone |
| `POST` | `/api/v1/vms/:cluster/:node/:vmid/migrate` | ✅ | Live migration |
| `POST` | `/api/v1/vms/:cluster/:node/:vmid/vnc/ticket` | ✅ | VNC console ticket |
| `GET` | `/api/v1/vms/:cluster/:node/:vmid/vnc/ws` | ✅ | VNC WebSocket proxy |
| `POST` | `/api/v1/vms/bulk/start` | ✅ | Bulk start VMs |
| `POST` | `/api/v1/vms/bulk/stop` | ✅ | Bulk stop VMs |
| `POST` | `/api/v1/vms/bulk/reboot` | ✅ | Bulk reboot VMs |
| `POST` | `/api/v1/vms/bulk/delete` | ✅ | Bulk delete VMs |
| `GET` | `/api/v1/vms/:cluster/:node/:vmid/snapshot` | ✅ | List snapshots |
| `POST` | `/api/v1/vms/:cluster/:node/:vmid/snapshot` | ✅ | Create snapshot |
| `DELETE` | `/api/v1/vms/:cluster/:node/:vmid/snapshot/:snapname` | ✅ | Delete snapshot |
| `POST` | `/api/v1/vms/:cluster/:node/:vmid/snapshot/:snapname/rollback` | ✅ | Rollback snapshot |
| `POST` | `/api/v1/vms/:cluster/:node/:vmid/backup` | ✅ | Trigger backup |
| `GET` | `/api/v1/vms/:cluster/:node/:vmid/backups` | ✅ | List backups |
| `POST` | `/api/v1/vms/:cluster/:node/:vmid/resize-disk` | ✅ | Disk resize |
| `GET` | `/api/v1/lxcs` | ✅ | Cross-cluster LXC list |
| `GET` | `/api/v1/lxcs/:cluster/:node/:vmid` | ✅ | Single LXC detail |
| `POST` | `/api/v1/lxcs/:cluster/:node/:vmid/:action` | ✅ | LXC actions |
| `POST` | `/api/v1/lxcs/:cluster/:node/:vmid/delete` | ✅ | Delete LXC |
| `GET` | `/api/v1/storages` | ✅ | Storage pool list |
| `GET` | `/api/v1/storages/:cluster/:node/:storage/content` | ✅ | Storage content |
| `POST` | `/api/v1/storages/:cluster/:node/:storage/upload` | ✅ | Upload ISO/template |
| `DELETE` | `/api/v1/storages/:cluster/:node/:storage/content/:volid` | ✅ | Delete storage content |
| `GET` | `/api/v1/networks` | ✅ | Network interface list |
| `GET` | `/api/v1/networks/:cluster/:node` | ✅ | Per-node network list |
| `GET` | `/api/v1/networks/vlans` | ✅ | VLAN list |
| `GET` | `/api/v1/hagroups` | ✅ | List HA groups |
| `POST` | `/api/v1/hagroups/:cluster/:group` | ✅ | Create HA group |
| `DELETE` | `/api/v1/hagroups/:cluster/:group` | ✅ | Delete HA group |
| `GET` | `/api/v1/hagroups/status` | ✅ | HA resource status |
| `GET` | `/api/v1/replication` | ✅ | List replication jobs |
| `POST` | `/api/v1/replication/:cluster/:vmid` | ✅ | Create replication job |
| `DELETE` | `/api/v1/replication/:cluster/:vmid/delete` | ✅ | Delete replication job |
| `GET` | `/api/v1/replication/:cluster/:vmid/status` | ✅ | Get replication status |
| `GET` | `/api/v1/ceph/status` | ✅ | Ceph cluster status |
| `GET` | `/api/v1/ceph/pools` | ✅ | Ceph pool list |
| `GET` | `/api/v1/firewall/rules` | ✅ | Cluster firewall rules |
| `GET` | `/api/v1/sdn/zones` | ✅ | SDN zones |
| `GET` | `/api/v1/sdn/vnets` | ✅ | SDN VNets |
| `GET` | `/api/v1/users` | ✅ | List users (admin) |
| `POST` | `/api/v1/users` | ✅ | Create user (admin) |
| `PUT` | `/api/v1/users/:username` | ✅ | Update user (admin) |
| `DELETE` | `/api/v1/users/:username` | ✅ | Delete user (admin) |
| `GET` | `/api/v1/tasks/:cluster/:node/:upid` | ✅ | Task status polling |
| `GET` | `/api/v1/audit` | ✅ | Paginated audit log |
| `GET` | `/` | ✅ | Frontend SPA shell |
| `GET` | `/static/*` | ✅ | Embedded static assets |
| `GET` | `/manifest.json` | ✅ | PWA manifest |
| `GET` | `/sw.js` | ✅ | Service worker |

---

## Coverage Summary

| Category | Total | Supported | Planned | Not Planned | Coverage |
|---|---|---|---|---|---|
| MoxUI API | 65 | 63 | 2 | 0 | **97%** |
| QEMU/VM | 24 | 15 | 5 | 4 | **63%** |
| LXC | 10 | 7 | 3 | 0 | **70%** |
| Storage | 5 | 5 | 0 | 0 | **100%** |
| Network | 5 | 3 | 2 | 0 | **60%** |
| Cluster | 19 | 12 | 7 | 0 | **63%** |
| Access | 2 | 1 | 0 | 1 | **50%** |
| **Total** | **130** | **106** | **19** | **5** | **82%** |

> **v2.0.0: 82% coverage (+16% from v1.2.0)** — VM/LXC write ops, storage write, Ceph, SDN, firewall, HA status all live.
