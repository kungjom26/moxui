# Changelog

All notable changes to MoxUI are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [3.0.0] — 2026-06-23

### Phase 7 — API Complete (v3.0.0)

All Proxmox VE API endpoints now covered (97%). 19 new endpoints across VMs, LXC, networking, cluster, and tasks.

#### VM Actions

- **Reset VM** — `POST .../status/reset` via `:action` handler (power-cycle)
- **Suspend VM** — `POST .../status/suspend` via `:action` handler (freeze to disk/RAM)
- **Resume VM** — `POST .../status/resume` via `:action` handler (restore from suspend)

#### VM Template

- **Convert to template** — `POST /api/v1/vms/:cluster/:node/:vmid/template` — QEMU VM → deployable template

#### VM Sendkey

- **Send keyboard event** — `POST /api/v1/vms/:cluster/:node/:vmid/sendkey` — QEMU key injection

#### VM Performance Data

- **RRD data** — `GET /api/v1/vms/:cluster/:node/:vmid/rrddata?timeframe=hour|day|week|month|year` — time-series VM stats

#### Task Operations

- **Task log** — `GET /api/v1/tasks/:cluster/:node/:upid/log` — fetch task log lines
- **Task delete** — `POST /api/v1/tasks/:cluster/:node/:upid/delete` — forget/remove task

#### LXC

- **Create container** — `POST /api/v1/lxcs/:cluster/:node/create` — full LXC creation with hostname, OS template, CPU, RAM, storage, networking
- **LXC config** — `GET /api/v1/lxcs/:cluster/:node/:vmid/config` + `PUT` — read/write container config

#### Network

- **Save pending config** — `PUT /api/v1/networks/:cluster/:node/config` — stage network changes
- **Apply config** — `POST /api/v1/networks/:cluster/:node/apply` — apply pending changes

#### Cluster Endpoints

- **Cluster status** — `GET /api/v1/cluster/status` — per-datacenter node/quorum info
- **Cluster config** — `GET /api/v1/cluster/config` — datacenter configuration
- **Cluster options** — `GET /api/v1/cluster/options` — datacenter-wide options
- **Cluster log** — `GET /api/v1/cluster/log` — cluster audit log
- **Cluster tasks** — `GET /api/v1/cluster/tasks` — recent cluster task list

#### New Module

- **`src/api/cluster.rs`** — 5 aggregated cluster endpoints

#### Statistics

| Metric | v3.0.0 |
|---|---|
| Source lines | ~14,000 |
| Test count | 189 (+19) |
| Files changed | 12 |
| New API endpoints | 15 |
| API coverage | 97% (143/148) |
| Proxmox client methods | 43 (+17) |
| New types | 9 |

---

## [2.0.0] — 2026-06-22

### Phase 6 — Advanced Cluster Management

The biggest release yet: 21 features spanning VM/LXC/Storage write operations, LDAP auth, user management, VNC WebSocket proxy, Ceph/SDN/firewall dashboards, and major frontend UX improvements.

#### VM Write Operations

- **VM creation** — `POST /api/v1/vms/:cluster/:node/create` with OS, CPU, RAM, disk, network config
- **VM clone** — `POST /api/v1/vms/:cluster/:node/:vmid/clone` (full + linked)
- **VM config editor** — `PUT /api/v1/vms/:cluster/:node/:vmid/config` for changing CPU, RAM, disk, network
- **VM snapshots CRUD** — `GET/POST/DELETE /api/v1/vms/:cluster/:node/:vmid/snapshot` + rollback
- **VM backup** — `POST /api/v1/vms/:cluster/:node/:vmid/backup` with storage/mode/compression
- **VM backup list** — `GET /api/v1/vms/:cluster/:node/:vmid/backups` scanning storage content
- **Disk resize** — `POST /api/v1/vms/:cluster/:node/:vmid/resize-disk` (cloudinit + scsi/virtio)

#### LXC Write Operations

- **LXC actions** — `POST /api/v1/lxcs/:cluster/:node/:vmid/:action` for start/stop/shutdown/reboot
- **LXC delete** — `POST /api/v1/lxcs/:cluster/:node/:vmid/delete`

#### Storage Write Operations

- **Storage upload** — `POST /api/v1/storages/:cluster/:node/:storage/upload` (base64 → multipart)
- **Storage content delete** — `DELETE /api/v1/storages/:cluster/:node/:storage/content/:volid`

#### LDAP/AD Authentication

- **`src/auth/ldap.rs`** — LDAP bind + search + re-bind flow with auto-create
- **`LdapConfig`** — URL, base_dn, bind credentials, search filter, attribute mapping
- **`POST /api/v1/auth/ldap/login`** — LDAP login handler with auto-user-creation
- **Config integration** — `auth.ldap` section with fail-closed defaults

#### User Management

- **Admin user CRUD** — `GET/POST /api/v1/users`, `PUT/DELETE /api/v1/users/:username`
- **`UserStore` mutation methods** — `add_user()`, `update_user()`, `delete_user()`, `list_users()`
- **`AppState.users`** — Changed to `Arc<RwLock<UserStore>>` for concurrent read/write

#### VNC WebSocket Proxy

- **Full wire-up** — `proxmox_vnc_proxy()` function in `src/api/vnc.rs`
- Token verification → Proxmox ticket fetch → upstream WS connect → bidirectional pipe
- Uses `tokio-tungstenite` with rustls TLS for upstream connection
- Handles Binary, Close, Ping, Pong, Text, Frame messages

#### Ceph Dashboard

- **`GET /api/v1/ceph/status`** — Proxies to `cluster/ceph/status`
- **`GET /api/v1/ceph/pools`** — Proxies to `cluster/ceph/pool`

#### Network & Cluster

- **`GET /api/v1/networks/vlans`** — VLAN listing across all bridges
- **`GET /api/v1/firewall/rules`** — Cluster firewall rules proxy
- **`GET /api/v1/hagroups/status`** — HA resource status dashboard
- **`GET /api/v1/sdn/zones`** — SDN zone listing
- **`GET /api/v1/sdn/vnets`** — SDN VNet listing

#### Frontend UX

- **Global search (Cmd+K / Ctrl+K)** — Search overlay across VMs, LXCs, storage, nodes. Keyboard navigation (↑/↓/Enter/Esc), click navigates to detail
- **Keyboard shortcuts** — `g+d` dashboard, `g+v` VMs, `g+s` storage, `g+n` network, `g+l` LXCs, `/` search, `?` help, `Esc` close
- **VM creation wizard** — 4-step wizard: General → System → Storage → Network + Summary
- **PWA support** — `manifest.json`, `sw.js` service worker (cache-first), install prompt
- **Notification center** — Bell icon with unread badge, poll every 10s, mark read/all
- **Stats export (CSV)** — Download VM chart data as CSV
- **API keys management page** — Create/revoke API keys, list with last used

#### i18n Updates

- **96 new keys** in both `en.json` and `th.json` for all new UI features

#### Statistics

| Metric | Value |
|---|---|
| Source lines (lib + bin) | ~12,000 |
| Test count | 170 |
| Files changed | 24 |
| New API endpoints | 25+ |
| LDAP support | Yes |
| VNC WS proxy | Working |
| Search | Cmd+K global |
| PWA | manifest + SW |
| Keyboard shortcuts | 10+ |
| i18n keys (EN/TH) | 295 each |

---

## [1.2.0] — 2026-06-22

### Phase 5 — Power User Features

*Previous entry — see archive CHANGELOG for full content.*

## [1.1.0] — 2026-06-22

### Phase 4 — Polish & Community

*Previous entry — see archive CHANGELOG for full content.*

## [1.0.0] — 2026-06-22

### Production Release — v1.0.0 MVP

*Previous entry — see archive CHANGELOG for full content.*

## [0.2.0] — 2026-06-22

## [0.1.1] — 2026-06-22

## [0.1.0] — 2026-06-21

## [0.0.0] — 2026-06-20

---

[3.0.0]: https://github.com/kungjom26/moxui/releases/tag/v3.0.0
[2.0.0]: https://github.com/kungjom26/moxui/releases/tag/v2.0.0
[1.2.0]: https://github.com/kungjom26/moxui/releases/tag/v1.2.0
[1.1.0]: https://github.com/kungjom26/moxui/releases/tag/v1.1.0
[1.0.0]: https://github.com/kungjom26/moxui/releases/tag/v1.0.0
[0.2.0]: https://github.com/kungjom26/moxui/releases/tag/v0.2.0
[0.1.1]: https://github.com/kungjom26/moxui/releases/tag/v0.1.1
[0.1.0]: https://github.com/kungjom26/moxui/releases/tag/v0.1.0
