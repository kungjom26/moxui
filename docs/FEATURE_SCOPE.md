# 📋 MoxUI — Feature Scope (MVP v1.0.0)

> **Purpose:** รายการ feature ทุกตัวใน MVP — ว่ามีอะไร ไม่มีอะไร priority แค่ไหน พร้อม acceptance criteria ที่ test ได้จริง
>
> **Tier legend:**
> - **🔴 MUST** = ไม่มี = ไม่ใช่ MVP (blocker สำหรับ v1.0.0)
> - **🟡 SHOULD** = มีใน v1.0.0 แต่ถ้าติดขัดเวลา ship ทีหลังได้ (v1.1)
> - **🟢 COULD** = nice-to-have, ship ถ้ามีเวลา (v1.2+)
> - **⏸️ LATER** = ไม่ทำใน v1.0.0 แต่มี roadmap แน่นอน — ดู [`FUTURE_ROADMAP.md`](../FUTURE_ROADMAP.md)
>
> **Decision (2026-06-20):** WON'T tier ถูกเปลี่ยนเป็น **LATER** — features เหล่านี้มีแผนชัดเจนในอนาคต ไม่ใช่ "ไม่ทำ"
>
> **Deploy order:** homelab ก่อน → scale production ✅
> **Auth model:** ใส่ครบทุกตัว (local + 2FA + OIDC + RBAC + WebAuthn) ✅

---

## 🎯 MVP Goal (1 ประโยค)

> Deploy MoxUI container เดียว แล้วใช้ dashboard เดียวดู VM/LXC ทุก cluster ของ พี่เสือ ได้ พร้อม start/stop VM, เปิด console, audit log, 2FA — ปลอดภัย รวดเร็ว เสถียร

---

## 1. 🖥️ VM (QEMU) Management

### 1.1 List & View

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| V-001 | **List all VMs across clusters** | 🔴 MUST | Dashboard รวม VM ทุก cluster | - แสดง VM ≥ 100 รายการใน < 500ms<br>- Filter by cluster, node, status, tag<br>- Sort ทุก column |
| V-002 | **VM list with state badge** | 🔴 MUST | แถบสี status | - running=green, stopped=gray, paused=yellow<br>- อัปเดต real-time (poll 2s) |
| V-003 | **VM detail page** | 🔴 MUST | หน้ารายละเอียด VM | - แสดง config, resources, network, events<br>- URL: `/vms/{cluster}/{node}/{vmid}`<br>- Tabs: Overview, Console, Stats, Snapshots, Backup, Config |
| V-004 | **VM search** | 🟡 SHOULD | ค้นหาข้าม cluster | - Search by name, vmid, IP, tag<br>- Debounce 300ms<br>- Results in < 200ms |
| V-005 | **VM filter by tag** | 🟡 SHOULD | Filter by Proxmox tag | - Multi-tag AND/OR<br>- Tag suggestions (autocomplete) |
| V-006 | **VM bulk select + action** | 🟢 COULD | เลือกหลาย VM | - Select 10+ VMs, start/stop/tag/delete พร้อมกัน<br>- Progress indicator |
| V-007 | **VM template management** | ⏸️ LATER | จัดการ template | - Proxmox UI ใช้งานง่ายกว่าสำหรับ template |

### 1.2 VM Operations (Write)

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| V-101 | **Start VM** | 🔴 MUST | เริ่ม VM | - Confirm dialog (configurable)<br>- Audit log<br>- Loading state, success/error toast<br>- Handle VM already running (idempotent) |
| V-102 | **Stop VM (graceful)** | 🔴 MUST | หยุด VM | - Default timeout 60s<br>- Configurable timeout<br>- Audit log |
| V-103 | **Stop VM (force)** | 🔴 MUST | ฆ่า VM ทันที | - Confirm dialog with VM name typed<br>- Audit log<br>- Required: role=admin OR owner |
| V-104 | **Reboot VM** | 🔴 MUST | รีสตาร์ท VM | - Graceful first, force fallback after timeout<br>- Audit log |
| V-105 | **Shutdown VM (ACPI)** | 🟡 SHOULD | ส่ง ACPI shutdown | - ต่างจาก stop graceful<br>- ใช้สำหรับ Windows/Linux ที่รองรับ ACPI |
| V-106 | **Pause / Resume VM** | 🟡 SHOULD | หยุดชั่วคราว | - ใช้สำหรับ snapshot หรือ debug |
| V-107 | **Delete VM** | 🔴 MUST | ลบ VM | - Confirm dialog with VM name typed<br>- Options: keep disk / delete disk<br>- Soft delete (disabled by default)<br>- Audit log<br>- Required: role=admin |
| V-108 | **Create VM (simple)** | 🟢 COULD | สร้าง VM แบบง่าย | - ใช้ Proxmox wizard ดีกว่า — defer to v1.2 |
| V-109 | **Clone VM** | ⏸️ LATER | โคลน VM | - ใช้ Proxmox native UI |
| V-110 | **Migrate VM** | ⏸️ LATER | Live migration | - v2+ feature |

### 1.3 VM Console

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| V-201 | **VNC console (noVNC)** | 🔴 MUST | เปิด console ผ่าน browser | - WebSocket proxy to Proxmox VNC port<br>- Auto-reconnect on disconnect<br>- Clipboard sync (text only)<br>- Resize handling<br>- Ctrl+Alt+Del button<br>- Fullscreen toggle |
| V-201a | **SPICE console** | ⏸️ LATER | SPICE protocol | - ซับซ้อนกว่า, ใช้ VNC เป็นหลักพอ |
| V-202 | **Console permission check** | 🔴 MUST | Block ถ้าไม่มีสิทธิ์ | - 401 if not auth<br>- 403 if no console permission |

### 1.4 VM Stats

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| V-301 | **Live CPU usage** | 🔴 MUST | กราฟ CPU | - Update every 2s<br>- uPlot chart<br>- Range: 1h default, 5m/15m/24h options |
| V-302 | **Live RAM usage** | 🔴 MUST | กราฟ memory | - Used vs total<br>- Percentage |
| V-303 | **Live Network I/O** | 🟡 SHOULD | กราฟ network | - RX/TX Mbps<br>- Per-NIC breakdown |
| V-304 | **Live Disk I/O** | 🟡 SHOULD | กราฟ disk | - Read/Write IOPS + throughput |
| V-305 | **Stats export** | ⏸️ LATER | Download CSV | - v1.2 feature |

### 1.5 VM Snapshots

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| V-401 | **List snapshots** | 🟡 SHOULD | แสดง snapshot ทั้งหมด | - Name, date, size, parent<br>- Include RAM state flag |
| V-402 | **Create snapshot** | 🟡 SHOULD | สร้าง snapshot | - Name input, optional description<br>- Include RAM state checkbox<br>- Quiesce option (filesystem freeze) |
| V-403 | **Rollback to snapshot** | 🟡 SHOULD | ย้อนกลับ snapshot | - Confirm dialog<br>- VM must be stopped (or live rollback with warning) |
| V-404 | **Delete snapshot** | 🟡 SHOULD | ลบ snapshot | - Confirm dialog<br>- Audit log |

### 1.6 VM Backup

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| V-501 | **Trigger backup (vzdump)** | 🟡 SHOULD | สั่ง backup | - Storage selector<br>- Mode: snapshot / suspend / stop<br>- Compression: none / gzip / lzo / zstd<br>- Schedule vs ad-hoc |
| V-502 | **List backups** | 🟡 SHOULD | แสดง backup files | - Across all storage<br>- Filter by VM, date, size |
| V-503 | **Restore from backup** | ⏸️ LATER | Restore wizard | - ใช้ Proxmox native UI ดีกว่า |
| V-504 | **Backup schedule CRUD** | ⏸️ LATER | Schedule config | - v2+ feature |

---

## 2. 📦 LXC Container Management

### 2.1 List & View

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| L-001 | **List all LXC across clusters** | 🟡 SHOULD | เหมือน VM list แต่ LXC | - Filter by cluster, node, status, tag |
| L-002 | **LXC detail page** | 🟡 SHOULD | เหมือน VM detail | - Tabs เหมือนกัน |
| L-003 | **LXC search/filter** | 🟡 SHOULD | | - Same as VM |

### 2.2 LXC Operations

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| L-101 | **Start/Stop/Reboot LXC** | 🟡 SHOULD | เหมือน VM | - Audit log |
| L-102 | **LXC console (xterm.js)** | 🟡 SHOULD | Terminal console | - ไม่ใช่ VNC — ใช้ xterm.js + WebSocket → pct enter<br>- Required: xterm.js ~30 KB |
| L-103 | **LXC snapshot** | 🟢 COULD | เหมือน VM snapshot | - v1.2 |

### 2.3 LXC NOT in MVP

| ID | Feature | Tier |
|---|---|---|
| L-201 | Create / Clone / Migrate LXC | ⏸️ LATER |
| L-202 | LXC backup/restore UI | ⏸️ LATER |
| L-203 | LXC resources scaling (CPU/RAM/disk hot-plug) | ⏸️ LATER |

---

## 3. 🌐 Network Management

### 3.1 Bridges & VLANs

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| N-001 | **List bridges per node** | 🟡 SHOULD | แสดง bridge ทั้งหมด | - Bridge name, VLAN-aware, ports, comment |
| N-002 | **Bridge detail** | 🟡 SHOULD | สมาชิก + config | - Member interfaces, VLANs |
| N-003 | **List VLANs** | 🟢 COULD | VLAN per bridge | - v1.2 |
| N-004 | **Create/Edit bridge** | ⏸️ LATER | | - Proxmox UI ดีกว่า |

### 3.2 SDN (Software Defined Networking)

| ID | Feature | Tier |
|---|---|---|
| N-101 | SDN Zone/VNet/Subnet management | ⏸️ LATER (v2+) |

---

## 4. 💾 Storage Management

### 4.1 Storage Pools

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| S-001 | **List storage pools per cluster** | 🟡 SHOULD | รวมทุก node | - Name, type (lvm/zfs/nfs/...), total, used, free, % |
| S-002 | **Storage detail** | 🟡 SHOULD | content types + VMs | - Which VMs use it, ISO list |
| S-003 | **Storage usage chart** | 🟢 COULD | pie/donut chart | - v1.2 |

### 4.2 ISO Library

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| S-101 | **List ISO per storage** | 🟡 SHOULD | รายการ ISO ทุก storage | - Name, size, storage, uploaded date |
| S-102 | **Upload ISO** | 🟡 SHOULD | drag & drop upload | - Max 50 GB (configurable)<br>- Progress bar<br>- Validation: .iso extension<br>- Upload to selected storage |
| S-103 | **Delete ISO** | 🟡 SHOULD | ลบ ISO | - Confirm dialog<br>- Audit log |
| S-104 | **Download ISO** | 🟢 COULD | download กลับ | - v1.2 |

### 4.3 Storage NOT in MVP

| ID | Feature | Tier |
|---|---|---|
| S-201 | Create storage pool | ⏸️ LATER |
| S-202 | ZFS/LVM management | ⏸️ LATER |
| S-203 | Ceph management | ⏸️ LATER (v3+) |

---

## 5. 🖧 Node / Host Management

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| H-001 | **Cluster overview** | 🔴 MUST | สรุปทุก cluster | - Nodes, total CPU/RAM/Storage, VM count, status |
| H-002 | **Node list** | 🟡 SHOULD | รายการ node | - Name, status (online/offline), CPU/RAM/Storage |
| H-003 | **Node detail** | 🟡 SHOULD | รายละเอียด node | - CPU model, RAM, disks, NICs, uptime, services |
| H-004 | **Node services status** | 🟡 SHOULD | pveproxy, pvedaemon, corosync | - Color-coded status |
| H-005 | **Node CPU/RAM/NET chart** | 🟡 SHOULD | กราฟ over time | - Same as VM stats |
| H-006 | **Reboot / Shutdown node** | ⏸️ LATER | | - Dangerous — Proxmox UI only |
| H-007 | **Shell in node** | ⏸️ LATER | | - Direct SSH only |

---

## 6. 🔐 Authentication & Authorization

### 6.1 Local Auth

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| A-001 | **Login (email + password)** | 🔴 MUST | Local user db | - bcrypt verify<br>- Lockout after 5 failed (15 min) |
| A-002 | **JWT (RS256) access token** | 🔴 MUST | 15 min TTL | - Public key verify<br>- Claims: sub, role, exp, iat |
| A-003 | **Refresh token rotation** | 🔴 MUST | 7 day TTL, single-use | - Hash stored in DB<br>- Revoke list on logout |
| A-004 | **Logout** | 🔴 MUST | Revoke refresh token | - Clear cookie + revoke DB entry |
| A-005 | **Password change** | 🔴 MUST | เปลี่ยนรหัสผ่าน | - Old password verify<br>- New password policy check |
| A-006 | **Password reset (admin)** | 🟡 SHOULD | admin reset password | - Generate random, force change on next login |
| A-007 | **Bootstrap admin (first boot)** | 🔴 MUST | สร้าง admin คนแรก | - Read from env or config<br>- Force password change on first login |

### 6.2 2FA / MFA

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| A-101 | **TOTP (Google Authenticator)** | 🔴 MUST | RFC 6238 | - QR code setup<br>- 6-digit code verify<br>- Backup codes (8, single-use) |
| A-102 | **TOTP required for admin** | 🔴 MUST | admin บังคับ 2FA | - Block login without 2FA<br>- Enforce at first login |
| A-103 | **TOTP optional for others** | 🟡 SHOULD | user เลือกได้ | - Settings page |
| A-104 | **WebAuthn / Passkey** | 🔴 MUST | Yubikey, Touch ID, Windows Hello — **auth model ใส่ครบ** | - เพิ่ม hardware key, biometric<br>- Fallback TOTP ถ้าไม่มี key<br>- Required: 1 passkey ต่อ user + TOTP fallback |

### 6.3 SSO / Federation

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| A-201 | **OIDC SSO (Google)** | 🔴 MUST | Login with Google — **auth model ใส่ครบ** | - OAuth2 + PKCE<br>- Auto-create user on first login<br>- Map email → MoxUI user |
| A-202 | **OIDC SSO (GitHub)** | 🟡 SHOULD | Login with GitHub | - OAuth2 + PKCE<br>- Map username → MoxUI user |
| A-203 | **OIDC SSO (Okta/Auth0)** | 🟡 SHOULD | Generic OIDC provider | - Configurable issuer + client_id<br>- Enterprise-friendly |
| A-204 | **LDAP / Active Directory** | 🔴 MUST | Enterprise directory — **auth model ใส่ครบ** | - LDAP bind + search<br>- Map AD groups → MoxUI roles<br>- TLS connection (LDAPS / StartTLS) |

### 6.4 Roles & Permissions

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| A-301 | **Role: admin** | 🔴 MUST | ทุกอย่าง | - Full access |
| A-302 | **Role: operator** | 🔴 MUST | create/start/stop VM | - No delete, no user mgmt |
| A-303 | **Role: viewer** | 🔴 MUST | read-only | - No write operations |
| A-304 | **Per-cluster permission** | 🟡 SHOULD | จำกัด cluster ที่ user เห็น | - Many-to-many user ↔ cluster<br>- Filter at API level |
| A-305 | **Custom role (Phase 3)** | ⏸️ LATER | | - v2+ |

---

## 7. 📊 Dashboard & Visualization

### 7.1 Multi-cluster Dashboard

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| D-001 | **Cluster summary cards** | 🔴 MUST | 1 card per cluster | - Total CPU/RAM/Storage/VMs/Running<br>- Cluster status (healthy/warning/error) |
| D-002 | **Cross-cluster VM table** | 🔴 MUST | VM ทุก cluster ในตารางเดียว | - Column: cluster, node, name, status, CPU, RAM, IP, tags<br>- Sortable, filterable |
| D-003 | **Resource overview chart** | 🟡 SHOULD | กราฟ CPU/RAM/Net | - Host-level (not VM)<br>- Real-time (5s refresh) |
| D-004 | **Recent events / alerts** | 🟡 SHOULD | notification feed | - VM stopped > 7 days, backup failed, node offline<br>- Click to navigate to source |
| D-005 | **Quick actions** | 🟢 COULD | shortcuts | - "Start all production VMs", "Stop all dev VMs" |

### 7.2 Custom Dashboards

| ID | Feature | Tier |
|---|---|---|
| D-101 | User-defined widgets | ⏸️ LATER (v2+) |
| D-102 | Saved view / bookmarks | ⏸️ LATER (v2+) |

---

## 8. 📜 Audit & Compliance

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| AU-001 | **Audit log capture** | 🔴 MUST | ทุก mutating action | - User, action, target, IP, user-agent, timestamp, result<br>- Async write (ไม่ block request) |
| AU-002 | **Audit log viewer** | 🟡 SHOULD | หน้าดู audit log | - Filter by user, action, date, cluster<br>- Export CSV/JSON<br>- Admin only |
| AU-003 | **Audit log retention** | 🟡 SHOULD | auto-purge old logs | - Configurable (default 90 days)<br>- Compression after 30 days<br>- Daily cleanup job |
| AU-004 | **Audit log integrity** | 🟢 COULD | HMAC signing | - v1.2 — prevent tampering |
| AU-005 | **Audit alert** | ⏸️ LATER | alert on suspicious | - v2+ |

---

## 9. 🔍 Search & Filter

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| F-001 | **Global search (Cmd+K)** | 🟡 SHOULD | ค้นหาทุกอย่าง | - VM, LXC, node, storage, ISO, user<br>- Keyboard-driven (Mousetrap)<br>- Results in < 100ms |
| F-002 | **Tag-based filter** | 🟡 SHOULD | filter ตาม tag | - Multi-tag, AND/OR logic |
| F-003 | **Saved filters** | ⏸️ LATER | bookmark filter | - v2+ |

---

## 10. 🔔 Notifications

### 10.1 In-app

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| NT-001 | **Toast notifications** | 🔴 MUST | success/error feedback | - Auto-dismiss 3-5s<br>- Stack multiple<br>- Dark/light theme |
| NT-002 | **Notification center** | 🟡 SHOULD | รวม notification | - Top-right bell icon<br>- Mark as read<br>- Filter unread/all |
| NT-003 | **Browser push notifications** | ⏸️ LATER | Web Push API | - v2+ |

### 10.2 External (webhook)

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| NT-101 | **Webhook → Slack** | 🟢 COULD | ส่งเข้า Slack | - v1.2 |
| NT-102 | **Webhook → Discord** | 🟢 COULD | ส่งเข้า Discord | - v1.2 |
| NT-103 | **Webhook → generic URL** | 🟢 COULD | POST JSON | - v1.2 |

---

## 11. ⚙️ Settings & Configuration

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| SE-001 | **User profile page** | 🟡 SHOULD | แก้ password, 2FA, theme | - Change password<br>- Setup/disable 2FA<br>- Theme preference |
| SE-002 | **User management (admin)** | 🟡 SHOULD | CRUD users | - Create, edit role, delete, reset password<br>- Per-cluster permission assignment |
| SE-003 | **Cluster config (admin)** | 🟡 SHOULD | เพิ่ม/แก้ cluster | - Name, URL, credentials<br>- Test connection button |
| SE-004 | **System settings** | 🟡 SHOULD | global config | - Session timeout, rate limits, audit retention<br>- Most require restart |
| SE-005 | **Theme toggle** | 🔴 MUST | dark/light | - Per-user preference |
| SE-006 | **Language (i18n)** | ⏸️ LATER | Thai/English | - v2+ |

---

## 12. 📈 Observability (MoxUI self-monitoring)

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| O-001 | **Health endpoint** | 🔴 MUST | `/health`, `/livez`, `/readyz` | - 200 OK with version + uptime<br>- 503 if degraded |
| O-002 | **Prometheus metrics** | 🟡 SHOULD | `/metrics` | - RED metrics (Rate, Errors, Duration)<br>- USE metrics (Proxmox client)<br>- Cache hit/miss<br>- Audit log counter |
| O-003 | **OpenTelemetry tracing** | 🟢 COULD | OTLP export | - v1.2 (Phase 3 Day 26) |
| O-004 | **Structured logs (JSON)** | 🟡 SHOULD | Loki-compatible | - All logs include request_id, user_id<br>- Log levels configurable |
| O-005 | **Log viewer (UI)** | ⏸️ LATER | | - Use Loki/Grafana instead |

---

## 13. 🚀 Deployment & Operations

### 13.1 Deployment

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| DP-001 | **Single-container Docker deploy** | 🔴 MUST | one command | - `docker run` works<br>- Health check passes |
| DP-002 | **docker-compose with reverse proxy** | 🔴 MUST | Caddy + MoxUI | - TLS auto via Caddy<br>- Volume for data |
| DP-003 | **Helm chart for K8s** | 🟡 SHOULD | Production K8s deploy | - HPA, PDB, NetworkPolicy, Ingress |
| DP-004 | **systemd unit** | 🟢 COULD | Bare-metal deploy | - v1.2 |

### 13.2 Backup & Restore

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| DP-101 | **Backup MoxUI data** | 🟡 SHOULD | SQLite + config | - Shell script + CronJob<br>- Optional off-cluster (S3/NFS) |
| DP-102 | **Restore from backup** | 🟡 SHOULD | Recovery | - Tested runbook |
| DP-103 | **Migration to new version** | 🟡 SHOULD | Upgrade path | - Read old DB, migrate, write new |

### 13.3 Operations

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| DP-201 | **Graceful shutdown** | 🔴 MUST | Drain connections | - SIGTERM → stop accepting → drain → close DB |
| DP-202 | **Auto-restart on crash** | 🔴 MUST | Docker/K8s restart policy | - State preserved in SQLite |
| DP-203 | **Log rotation** | 🟡 SHOULD | ไม่เต็ม disk | - Daily rotate, compress, retain 30 days |

---

## 14. 🎨 UI/UX (cross-cutting)

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| UX-001 | **Responsive layout** | 🔴 MUST | mobile/tablet/desktop | - Breakpoints: 768/1024/1280<br>- Sidebar collapse on mobile |
| UX-002 | **Keyboard shortcuts** | 🟡 SHOULD | ทำงานเร็ว | - g+d / g+v / g+s (navigate)<br>- /, c, ?, Esc |
| UX-003 | **Dark/Light theme** | 🔴 MUST | theme toggle | - CSS variables<br>- localStorage persist |
| UX-004 | **Loading states** | 🔴 MUST | skeleton + spinner | - ทุก async action |
| UX-005 | **Empty states** | 🟡 SHOULD | illustration + message | - "No VMs yet", "Connect first cluster" |
| UX-006 | **Error states** | 🟡 SHOULD | retry + error message | - Network error, 500, 404 |
| UX-007 | **Confirm dialogs** | 🔴 MUST | ป้องกัน accidental action | - Type VM name for destructive |
| UX-008 | **Toasts** | 🔴 MUST | success/error feedback | - Top-right, auto-dismiss |
| UX-009 | **Accessibility (WCAG AA)** | 🟡 SHOULD | a11y audit | - ARIA labels, keyboard nav, contrast |
| UX-010 | **Internationalization** | ⏸️ LATER | i18n framework | - v2+ (Thai/English) |
| UX-011 | **PWA / install prompt** | ⏸️ LATER | install as app | - v2+ |

---

## 15. 🔧 Developer Experience

| ID | Feature | Tier | Description | Acceptance Criteria |
|---|---|---|---|---|
| DX-001 | **CI (GitHub Actions)** | 🔴 MUST | build + test + clippy + fmt | - Run on every PR |
| DX-002 | **Release workflow** | 🔴 MUST | tagged release + image | - Push image to ghcr.io<br>- Sign binary |
| DX-003 | **Pre-commit hooks** | 🟡 SHOULD | clippy + fmt locally | - Optional but recommended |
| DX-004 | **Makefile** | 🟡 SHOULD | common tasks | - `make build`, `make test`, `make run`, `make docker` |
| DX-005 | **Mock Proxmox server** | 🟡 SHOULD | integration tests | - axum-based mock for tests |
| DX-006 | **Benchmark suite** | 🟢 COULD | criterion benches | - v1.2 |

---

## 📊 Summary by Tier

| Tier | Count | % |
|---|---|---|
| 🔴 MUST | ~54 | ~32% |
| 🟡 SHOULD | ~65 | ~38% |
| 🟢 COULD | ~18 | ~11% |
| ⏸️ LATER | ~31 | ~19% |
| **Total** | **~168** | **100%** |

> ดู ⏸️ LATER features ใน [`FUTURE_ROADMAP.md`](../FUTURE_ROADMAP.md)

## 🎯 v1.0.0 Definition of Done

**MUST (54 features) — all required for v1.0.0:**

### Auth (14) — ใส่ครบทุกตัวตามที่ พี่เสือ approve
- [ ] A-001 Login (email + password)
- [ ] A-002 JWT RS256
- [ ] A-003 Refresh token rotation
- [ ] A-004 Logout
- [ ] A-005 Password change
- [ ] A-007 Bootstrap admin
- [ ] A-101 TOTP
- [ ] A-102 2FA required for admin
- [ ] A-104 **WebAuthn / Passkey**
- [ ] A-201 **OIDC SSO (Google)**
- [ ] A-204 **LDAP / Active Directory**
- [ ] A-301/302/303 Roles (admin/operator/viewer)
- [ ] A-304 Per-cluster permission
- [ ] A-101x Backup codes

### VM (8)
- [ ] V-001 List VMs cross-cluster
- [ ] V-002 State badge
- [ ] V-003 VM detail page
- [ ] V-101 Start
- [ ] V-102 Stop (graceful)
- [ ] V-103 Stop (force)
- [ ] V-104 Reboot
- [ ] V-107 Delete
- [ ] V-201 VNC console (noVNC)
- [ ] V-301/302 CPU/RAM chart

### Dashboard (2)
- [ ] D-001 Cluster summary cards
- [ ] D-002 Cross-cluster VM table

### Audit (1)
- [ ] AU-001 Audit log capture

### Deployment (4)
- [ ] DP-001 Single-container Docker
- [ ] DP-002 docker-compose + reverse proxy
- [ ] DP-201 Graceful shutdown
- [ ] DP-202 Auto-restart

### UI (5)
- [ ] UX-001 Responsive
- [ ] UX-003 Dark/Light theme
- [ ] UX-004 Loading states
- [ ] UX-007 Confirm dialogs
- [ ] UX-008 Toasts

### Observability (1)
- [ ] O-001 Health endpoint

### DevEx (2)
- [ ] DX-001 CI (GitHub Actions)
- [ ] DX-002 Release workflow

**Total MUST: ~32 features**

## 🟡 SHOULD (target v1.0.0, may defer to v1.1 if time-constrained)

~35 features ใน v1.0.0 — เช่น:
- V-401/402/403/404 VM snapshots
- V-501/502 VM backup trigger/list
- L-001/002/101/102 LXC support
- N-001/002 Network bridges
- S-001/002/101/102/103 Storage + ISO
- A-201 OIDC SSO (Google)
- A-304 Per-cluster permission
- D-003/004 Dashboard charts + alerts
- AU-002/003 Audit log viewer + retention
- F-001 Global search
- NT-002 Notification center
- SE-001/002/003/004 Settings pages
- O-002 Prometheus metrics
- O-004 Structured logs
- DP-003 Helm chart
- DP-101/102/103 Backup + restore + migration
- DP-203 Log rotation
- UX-002/005/006/009 Shortcuts/empty/error/a11y
- DX-003/004/005 Pre-commit/Makefile/Mock server

## 🟢 COULD (v1.2 — Phase 3+)

~15 features เช่น WebAuthn, custom dashboards, webhook integration, PWA, OIDC SSO สำหรับ GitHub/Okta

## ⏸️ LATER (จะทำในอนาคต — ดู FUTURE_ROADMAP.md)

~31 features เช่น Live migration, SDN, Ceph UI, VM create wizard, plugin system, i18n, cross-cluster replication — ดูรายละเอียดทั้งหมดที่ timeline + priority ใน [`../FUTURE_ROADMAP.md`](../FUTURE_ROADMAP.md)

**Quick reference ตาม phase:**
- **v1.1** (Q4 2026): VM create wizard, bulk operations, PWA, Terraform provider, CLI tool
- **v1.2** (Q1 2027): WebAuthn, OIDC (GitHub/Okta), LDAP, hardware hot-plug, IPAM
- **v2.0** (Q3 2027): Ceph dashboard, SDN management, cross-cluster live migration, ZFS UI
- **v3.0** (Q4 2027): Multi-region, multi-tenancy, hybrid cloud, ML recommendations, plugin system

---

## 🔗 Cross-cutting Concerns (always required)

| Concern | Status |
|---|---|
| **Security headers** (CSP, HSTS, X-Frame-Options, etc.) | 🔴 MUST |
| **Rate limiting** (login + API) | 🔴 MUST |
| **Input validation** | 🔴 MUST |
| **TLS 1.3 only** | 🔴 MUST |
| **No secrets in logs** | 🔴 MUST |
| **Parameterised queries** (no SQLi) | 🔴 MUST |
| **Audit log on every mutation** | 🔴 MUST |
| **Structured logging** | 🟡 SHOULD |
| **Metrics** | 🟡 SHOULD |
| **Tracing** | 🟢 COULD |
| **Run as non-root** | 🔴 MUST |
| **Read-only container rootfs** | 🟡 SHOULD |
| **Cap-drop ALL** | 🟡 SHOULD |

---

## 📅 Mapping to Roadmap

| Phase | Days | Features delivered |
|---|---|---|
| **Phase 0** | Week 1 | Foundation, ProxmoxClient, cache, API shell, /health |
| **Phase 1** | Week 2 | V-001/002/003/101/102/103/104/107/201/301/302, D-001/002, UI shell |
| **Phase 2** | Week 3 | A-001..303, A-101/102, AU-001, O-001, DP-001/002/201/202, security headers, rate limiting, **MVP v0.1.0-alpha** |
| **Phase 3** | Week 4 | Multi-cluster, A-201/304, O-002, O-004, F-001, NT-002 |
| **Phase 4** | Week 5-6 | LXC, snapshots, backup, network, storage, ISO, **v1.0.0** |

---

## 🎯 ขอ feedback จาก พี่เสือ

**ตรวจสอบ:**

1. **MVP scope ขนาดนี้พอมั้ย?** 32 MUST features + ~25 SHOULD = ~57 features สำหรับ v1.0.0 — คิดว่า tight หรือพอดี?
2. **MUST list ตรงไหนควรลด?** เช่น VM-201 console ต้องมีใน MVP มั้ย?
3. **MUST list ตรงไหนควรเพิ่ม?** มีอะไรที่พี่เสือขาดไม่ได้?
4. **SHOULD → MUST?** ตัวไหนใน SHOULD ที่ควรเป็น MUST เพราะใช้บ่อย?
5. **COULD → SHOULD?** ตัวไหนที่อยากได้ใน v1.0.0 ไม่ต้องรอ v1.2?
6. **WON'T → reconsider?** ตัวไหนที่ควรทำใน MVP ที่ผม skip ไป?

อยากให้กุ้งจ่อมปรับ scope ตรงไหน บอกได้เลย 🦐