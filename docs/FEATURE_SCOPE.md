# 📋 MoxUI — Feature Scope (v1.2.0)

> **Purpose:** รายการ feature ทุกตัว — ว่ามีอะไร ไม่มีอะไร priority แค่ไหน พร้อม acceptance criteria ที่ test ได้จริง
>
> **Tier legend:**
> - **🔴 MUST** = ไม่มี = ไม่ใช่ MVP (blocker สำหรับ v1.0.0)
> - **🟡 SHOULD** = มีใน v1.0.0 แต่ถ้าติดขัดเวลาช่วง ship ทีหลังได้ (v1.1)
> - **🟢 COULD** = nice-to-have, ship ถ้ามีเวลา (v1.2+)
> - **⏸️ LATER** = ไม่ทำใน v1.0.0 แต่มี roadmap แน่นอน — ดู [`FUTURE_ROADMAP.md`](../FUTURE_ROADMAP.md)
>
> **Decision (2026-06-20):** WON'T tier ถูกเปลี่ยนเป็น **LATER** — features เหล่านี้มีแผนชัดเจนในอนาคต ไม่ใช่ "ไม่ทำ"
>
> **Current version: v1.2.0** — Phase 4 (Polish & Community) + Phase 5 (Power User) complete ✅
>
> **Next: v2.0** — Advanced Cluster Management 🔜

---

## 🎯 Project Goal (1 ประโยค)

> Deploy MoxUI container เดียว แล้วใช้ dashboard เดียวดู VM/LXC ทุก cluster ได้ พร้อม start/stop/migrate VM, bulk operations, HA group management, webhook notifications, custom dashboards, i18n, multi-region replication, plugin system, และ Terraform provider — ปลอดภัย รวดเร็ว เสถียร

---

## 1. 🖥️ VM (QEMU) Management

### 1.1 List & View

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| V-001 | **List all VMs across clusters** | 🔴 MUST | Dashboard รวม VM ทุก cluster | - แสดง VM ≥ 100 รายการใน < 500ms<br>- Filter by cluster, node, status, tag<br>- Sort ทุก column | ✅ v1.0.0 |
| V-002 | **VM list with state badge** | 🔴 MUST | แถบสี status | - running=green, stopped=gray, paused=yellow<br>- อัปเดต real-time (poll 2s) | ✅ v1.0.0 |
| V-003 | **VM detail page** | 🔴 MUST | หน้ารายละเอียด VM | - แสดง config, resources, network, events<br>- URL: `/vms/{cluster}/{node}/{vmid}`<br>- Tabs: Overview, Console, Stats, Snapshots, Backup, Config | ✅ v1.0.0 |
| V-004 | **VM search** | 🟡 SHOULD | ค้นหาข้าม cluster | - Search by name, vmid, IP, tag<br>- Debounce 300ms<br>- Results in < 200ms | ✅ v1.0.0 |
| V-005 | **VM filter by tag** | 🟡 SHOULD | Filter by Proxmox tag | - Multi-tag AND/OR<br>- Tag suggestions (autocomplete) | ✅ v1.0.0 |
| V-006 | **VM bulk select + action** | 🟢 COULD | เลือกหลาย VM | - Select 10+ VMs, start/stop/tag/delete พร้อมกัน<br>- Progress indicator | ✅ v1.1.0 |
| V-007 | **VM template management** | ⏸️ LATER | จัดการ template | - Proxmox UI ใช้งานง่ายกว่าสำหรับ template | ✅ v2.0 |

### 1.2 VM Operations (Write)

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| V-101 | **Start VM** | 🔴 MUST | เริ่ม VM | - Confirm dialog (configurable)<br>- Audit log<br>- Loading state, success/error toast<br>- Handle VM already running (idempotent) | ✅ v1.0.0 |
| V-102 | **Stop VM (graceful)** | 🔴 MUST | หยุด VM | - Default timeout 60s<br>- Configurable timeout<br>- Audit log | ✅ v1.0.0 |
| V-103 | **Stop VM (force)** | 🔴 MUST | ฆ่า VM ทันที | - Confirm dialog with VM name typed<br>- Audit log<br>- Required: role=admin OR owner | ✅ v1.0.0 |
| V-104 | **Reboot VM** | 🔴 MUST | รีสตาร์ท VM | - Graceful first, force fallback after timeout<br>- Audit log | ✅ v1.0.0 |
| V-105 | **Shutdown VM (ACPI)** | 🟡 SHOULD | ส่ง ACPI shutdown | - ต่างจาก stop graceful<br>- ใช้สำหรับ Windows/Linux ที่รองรับ ACPI | ✅ v1.0.0 |
| V-106 | **Pause / Resume VM** | 🟡 SHOULD | หยุดชั่วคราว | - ใช้สำหรับ snapshot หรือ debug | 🚧 v2.0 |
| V-107 | **Delete VM** | 🔴 MUST | ลบ VM | - Confirm dialog with VM name typed<br>- Options: keep disk / delete disk<br>- Soft delete (disabled by default)<br>- Audit log<br>- Required: role=admin | ✅ v1.0.0 |
| V-108 | **Create VM (simple)** | 🟢 COULD | สร้าง VM แบบง่าย | - ใช้ Proxmox wizard ดีกว่า — defer to v2.0 | 🚧 v2.0 |
| V-109 | **Clone VM** | ⏸️ LATER | โคลน VM | - ใช้ Proxmox native UI | 🚧 v2.0 |
| V-110 | **Migrate VM** | ⏸️ LATER | Live migration | ✅ **Done v1.1.0** — `POST /api/v1/vms/:cluster/:node/:vmid/migrate` | ✅ v1.1.0 |

### 1.3 VM Console

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| V-201 | **VNC console (noVNC)** | 🔴 MUST | เปิด console ผ่าน browser | - WebSocket proxy to Proxmox VNC port<br>- Auto-reconnect on disconnect<br>- Clipboard sync (text only)<br>- Resize handling<br>- Ctrl+Alt+Del button<br>- Fullscreen toggle | ✅ v1.0.0 |
| V-201a | **SPICE console** | ⏸️ LATER | SPICE protocol | - ซับซ้อนกว่า, ใช้ VNC เป็นหลักพอ | 🚧 v2.0+ |
| V-202 | **Console permission check** | 🔴 MUST | Block ถ้าไม่มีสิทธิ์ | - 401 if not auth<br>- 403 if no console permission | ✅ v1.0.0 |

### 1.4 VM Stats

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| V-301 | **Live CPU usage** | 🔴 MUST | กราฟ CPU | - Update every 2s<br>- uPlot chart<br>- Range: 1h default, 5m/15m/24h options | ✅ v1.0.0 |
| V-302 | **Live RAM usage** | 🔴 MUST | กราฟ memory | - Used vs total<br>- Percentage | ✅ v1.0.0 |
| V-303 | **Live Network I/O** | 🟡 SHOULD | กราฟ network | - RX/TX Mbps<br>- Per-NIC breakdown | ✅ v1.0.0 |
| V-304 | **Live Disk I/O** | 🟡 SHOULD | กราฟ disk | - Read/Write IOPS + throughput | ✅ v1.0.0 |
| V-305 | **Stats export** | ⏸️ LATER | Download CSV | - v2.0 feature | 🚧 v2.0 |

### 1.5 VM Snapshots

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| V-401 | **List snapshots** | 🟡 SHOULD | แสดง snapshot ทั้งหมด | - Name, date, size, parent<br>- Include RAM state flag | 🚧 v2.0 |
| V-402 | **Create snapshot** | 🟡 SHOULD | สร้าง snapshot | - Name input, optional description<br>- Include RAM state checkbox<br>- Quiesce option (filesystem freeze) | 🚧 v2.0 |
| V-403 | **Rollback to snapshot** | 🟡 SHOULD | ย้อนกลับ snapshot | - Confirm dialog<br>- VM must be stopped (or live rollback with warning) | 🚧 v2.0 |
| V-404 | **Delete snapshot** | 🟡 SHOULD | ลบ snapshot | - Confirm dialog<br>- Audit log | 🚧 v2.0 |

### 1.6 VM Backup

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| V-501 | **Trigger backup (vzdump)** | 🟡 SHOULD | สั่ง backup | - Storage selector<br>- Mode: snapshot / suspend / stop<br>- Compression: none / gzip / lzo / zstd<br>- Schedule vs ad-hoc | 🚧 v2.0 |
| V-502 | **List backups** | 🟡 SHOULD | แสดง backup files | - Across all storage<br>- Filter by VM, date, size | 🚧 v2.0 |
| V-503 | **Restore from backup** | ⏸️ LATER | Restore wizard | - ใช้ Proxmox native UI ดีกว่า | 🚧 v2.0 |
| V-504 | **Backup schedule CRUD** | ⏸️ LATER | Schedule config | - v2+ feature | 🚧 v2.0 |

---

## 2. 📦 LXC Container Management

### 2.1 List & View

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| L-001 | **List all LXC across clusters** | 🟡 SHOULD | เหมือน VM list แต่ LXC | - Filter by cluster, node, status, tag | ✅ v1.0.0 |
| L-002 | **LXC detail page** | 🟡 SHOULD | เหมือน VM detail | - Tabs เหมือนกัน | ✅ v1.0.0 |
| L-003 | **LXC search/filter** | 🟡 SHOULD | | - Same as VM | ✅ v1.0.0 |

### 2.2 LXC Operations

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| L-101 | **Start/Stop/Reboot LXC** | 🟡 SHOULD | เหมือน VM | - Audit log | 🚧 v2.0 |
| L-102 | **LXC console (xterm.js)** | 🟡 SHOULD | Terminal console | - ไม่ใช่ VNC — ใช้ xterm.js + WebSocket → pct enter<br>- Required: xterm.js ~30 KB | 🚧 v2.0 |
| L-103 | **LXC snapshot** | 🟢 COULD | เหมือน VM snapshot | - v2.0 | 🚧 v2.0 |

### 2.3 LXC NOT in MVP

| ID | Feature | Tier |
|---|---|---|
| L-201 | Create / Clone / Migrate LXC | ⏸️ LATER |
| L-202 | LXC backup/restore UI | ⏸️ LATER |
| L-203 | LXC resources scaling (CPU/RAM/disk hot-plug) | ⏸️ LATER |

---

## 3. 🌐 Network Management

### 3.1 Bridges & VLANs

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| N-001 | **List bridges per node** | 🟡 SHOULD | แสดง bridge ทั้งหมด | - Bridge name, VLAN-aware, ports, comment | ✅ v1.0.0 |
| N-002 | **Bridge detail** | 🟡 SHOULD | สมาชิก + config | - Member interfaces, VLANs | ✅ v1.0.0 |
| N-003 | **List VLANs** | 🟢 COULD | VLAN per bridge | - v2.0 | 🚧 v2.0 |
| N-004 | **Create/Edit bridge** | ⏸️ LATER | | - Proxmox UI ดีกว่า | 🚧 v2.0 |

### 3.2 SDN (Software Defined Networking)

| ID | Feature | Tier |
|---|---|---|
| N-101 | SDN Zone/VNet/Subnet management | ⏸️ LATER (v2+) |

---

## 4. 💾 Storage Management

### 4.1 Storage Pools

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| S-001 | **List storage pools per cluster** | 🟡 SHOULD | รวมทุก node | - Name, type (lvm/zfs/nfs/...), total, used, free, % | ✅ v1.0.0 |
| S-002 | **Storage detail** | 🟡 SHOULD | content types + VMs | - Which VMs use it, ISO list | ✅ v1.0.0 |
| S-003 | **Storage usage chart** | 🟢 COULD | pie/donut chart | - v2.0 | 🚧 v2.0 |

### 4.2 ISO Library

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| S-101 | **List ISO per storage** | 🟡 SHOULD | รายการ ISO ทุก storage | - Name, size, storage, uploaded date | ✅ v1.0.0 |
| S-102 | **Upload ISO** | 🟡 SHOULD | drag & drop upload | - Max 50 GB (configurable)<br>- Progress bar<br>- Validation: .iso extension<br>- Upload to selected storage | 🚧 v2.0 |
| S-103 | **Delete ISO** | 🟡 SHOULD | ลบ ISO | - Confirm dialog<br>- Audit log | 🚧 v2.0 |
| S-104 | **Download ISO** | 🟢 COULD | download กลับ | - v2.0 | 🚧 v2.0 |

### 4.3 Storage NOT in MVP

| ID | Feature | Tier |
|---|---|---|
| S-201 | Create storage pool | ⏸️ LATER |
| S-202 | ZFS/LVM management | ⏸️ LATER |
| S-203 | Ceph management | ⏸️ LATER (v3+) |

---

## 5. 🖧 Node / Host Management

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| H-001 | **Cluster overview** | 🔴 MUST | สรุปทุก cluster | - Nodes, total CPU/RAM/Storage, VM count, status | ✅ v1.0.0 |
| H-002 | **Node list** | 🟡 SHOULD | รายการ node | - Name, status (online/offline), CPU/RAM/Storage | ✅ v1.0.0 |
| H-003 | **Node detail** | 🟡 SHOULD | รายละเอียด node | - CPU model, RAM, disks, NICs, uptime, services | ✅ v1.0.0 |
| H-004 | **Node services status** | 🟡 SHOULD | pveproxy, pvedaemon, corosync | - Color-coded status | ✅ v1.0.0 |
| H-005 | **Node CPU/RAM/NET chart** | 🟡 SHOULD | กราฟ over time | - Same as VM stats | ✅ v1.0.0 |
| H-006 | **Reboot / Shutdown node** | ⏸️ LATER | | - Dangerous — Proxmox UI only | 🚧 v2.0+ |
| H-007 | **Shell in node** | ⏸️ LATER | | - Direct SSH only | 🚧 v2.0+ |

---

## 6. 🔐 Authentication & Authorization

### 6.1 Local Auth

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| A-001 | **Login (email + password)** | 🔴 MUST | Local user db | - bcrypt verify<br>- Lockout after 5 failed (15 min) | ✅ v1.0.0 |
| A-002 | **JWT (RS256) access token** | 🔴 MUST | 15 min TTL | - Public key verify<br>- Claims: sub, role, exp, iat | ✅ v1.0.0 |
| A-003 | **Refresh token rotation** | 🔴 MUST | 7 day TTL, single-use | - Hash stored in DB<br>- Revoke list on logout | ✅ v1.0.0 |
| A-004 | **Logout** | 🔴 MUST | Revoke refresh token | - Clear cookie + revoke DB entry | ✅ v1.0.0 |
| A-005 | **Password change** | 🔴 MUST | เปลี่ยนรหัสผ่าน | - Old password verify<br>- New password policy check | ✅ v1.0.0 |
| A-006 | **Password reset (admin)** | 🟡 SHOULD | admin reset password | - Generate random, force change on next login | 🚧 v2.0 |
| A-007 | **Bootstrap admin (first boot)** | 🔴 MUST | สร้าง admin คนแรก | - Read from env or config<br>- Force password change on first login | ✅ v1.0.0 |

### 6.2 2FA / MFA

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| A-101 | **TOTP (Google Authenticator)** | 🔴 MUST | RFC 6238 | - QR code setup<br>- 6-digit code verify<br>- Backup codes (8, single-use) | ✅ v1.0.0 |
| A-102 | **TOTP required for admin** | 🔴 MUST | admin บังคับ 2FA | - Block login without 2FA<br>- Enforce at first login | ✅ v1.0.0 |
| A-103 | **TOTP optional for others** | 🟡 SHOULD | user เลือกได้ | - Settings page | ✅ v1.0.0 |
| A-104 | **WebAuthn / Passkey** | 🔴 MUST | Yubikey, Touch ID, Windows Hello | - เพิ่ม hardware key, biometric<br>- Fallback TOTP ถ้าไม่มี key<br>- Required: 1 passkey ต่อ user + TOTP fallback | ✅ v1.0.0 |

### 6.3 SSO / Federation

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| A-201 | **OIDC SSO (Google)** | 🔴 MUST | Login with Google | - OAuth2 + PKCE<br>- Auto-create user on first login<br>- Map email → MoxUI user | ✅ v1.0.0 |
| A-202 | **OIDC SSO (GitHub)** | 🟡 SHOULD | Login with GitHub | - OAuth2 + PKCE<br>- Map username → MoxUI user | ✅ v1.0.0 |
| A-203 | **OIDC SSO (Okta/Auth0)** | 🟡 SHOULD | Generic OIDC provider | - Configurable issuer + client_id<br>- Enterprise-friendly | 🚧 v2.0 |
| A-204 | **LDAP / Active Directory** | 🔴 MUST | Enterprise directory | - LDAP bind + search<br>- Map AD groups → MoxUI roles<br>- TLS connection (LDAPS / StartTLS) | 🚧 v2.0 |

### 6.4 Roles & Permissions

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| A-301 | **Role: admin** | 🔴 MUST | ทุกอย่าง | - Full access | ✅ v1.0.0 |
| A-302 | **Role: operator** | 🔴 MUST | create/start/stop VM | - No delete, no user mgmt | ✅ v1.0.0 |
| A-303 | **Role: viewer** | 🔴 MUST | read-only | - No write operations | ✅ v1.0.0 |
| A-304 | **Per-cluster permission** | 🟡 SHOULD | จำกัด cluster ที่ user เห็น | - Many-to-many user ↔ cluster<br>- Filter at API level | ✅ v1.0.0 |
| A-305 | **Custom role (Phase 3)** | ⏸️ LATER | | - v2+ | 🚧 v2.0 |

---

## 7. 📊 Dashboard & Visualization

### 7.1 Multi-cluster Dashboard

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| D-001 | **Cluster summary cards** | 🔴 MUST | 1 card per cluster | - Total CPU/RAM/Storage/VMs/Running<br>- Cluster status (healthy/warning/error) | ✅ v1.0.0 |
| D-002 | **Cross-cluster VM table** | 🔴 MUST | VM ทุก cluster ในตารางเดียว | - Column: cluster, node, name, status, CPU, RAM, IP, tags<br>- Sortable, filterable | ✅ v1.0.0 |
| D-003 | **Resource overview chart** | 🟡 SHOULD | กราฟ CPU/RAM/Net | - Host-level (not VM)<br>- Real-time (5s refresh) | ✅ v1.0.0 |
| D-004 | **Recent events / alerts** | 🟡 SHOULD | notification feed | - VM stopped > 7 days, backup failed, node offline<br>- Click to navigate to source | ✅ v1.0.0 |
| D-005 | **Quick actions** | 🟢 COULD | shortcuts | - "Start all production VMs", "Stop all dev VMs" | ✅ v1.1.0 |

### 7.2 Custom Dashboards

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| D-101 | **User-defined widgets** | ⏸️ LATER (v2+) | | **✅ Done v1.1.0** — Drag & drop widget grid | ✅ v1.1.0 |
| D-102 | **Saved view / bookmarks** | ⏸️ LATER (v2+) | | 🚧 v2.0 |

---

## 8. 📜 Audit & Compliance

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| AU-001 | **Audit log capture** | 🔴 MUST | ทุก mutating action | - User, action, target, IP, user-agent, timestamp, result<br>- Async write (ไม่ block request) | ✅ v1.0.0 |
| AU-002 | **Audit log viewer** | 🟡 SHOULD | หน้าดู audit log | - Filter by user, action, date, cluster<br>- Export CSV/JSON<br>- Admin only | ✅ v1.0.0 |
| AU-003 | **Audit log retention** | 🟡 SHOULD | auto-purge old logs | - Configurable (default 90 days)<br>- Compression after 30 days<br>- Daily cleanup job | ✅ v1.0.0 |
| AU-004 | **Audit log integrity** | 🟢 COULD | HMAC signing | - v2.0 — prevent tampering | 🚧 v2.0 |
| AU-005 | **Audit alert** | ⏸️ LATER | alert on suspicious | - v2+ | 🚧 v2.0 |

---

## 9. 🔍 Search & Filter

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| F-001 | **Global search (Cmd+K)** | 🟡 SHOULD | ค้นหาทุกอย่าง | - VM, LXC, node, storage, ISO, user<br>- Keyboard-driven (Mousetrap)<br>- Results in < 100ms | 🚧 v2.0 |
| F-002 | **Tag-based filter** | 🟡 SHOULD | filter ตาม tag | - Multi-tag, AND/OR logic | ✅ v1.0.0 |
| F-003 | **Saved filters** | ⏸️ LATER | bookmark filter | - v2+ | 🚧 v2.0 |

---

## 10. 🔔 Notifications

### 10.1 In-app

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| NT-001 | **Toast notifications** | 🔴 MUST | success/error feedback | - Auto-dismiss 3-5s<br>- Stack multiple<br>- Dark/light theme | ✅ v1.0.0 |
| NT-002 | **Notification center** | 🟡 SHOULD | รวม notification | - Top-right bell icon<br>- Mark as read<br>- Filter unread/all | 🚧 v2.0 |
| NT-003 | **Browser push notifications** | ⏸️ LATER | Web Push API | - v2+ | 🚧 v2.0 |

### 10.2 External (webhook)

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| NT-101 | **Webhook → Slack** | 🟢 COULD | ส่งเข้า Slack | **✅ Done v1.1.0** | ✅ v1.1.0 |
| NT-102 | **Webhook → Discord** | 🟢 COULD | ส่งเข้า Discord | **✅ Done v1.1.0** | ✅ v1.1.0 |
| NT-103 | **Webhook → generic URL** | 🟢 COULD | POST JSON | **✅ Done v1.1.0** | ✅ v1.1.0 |

---

## 11. ⚙️ Settings & Configuration

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| SE-001 | **User profile page** | 🟡 SHOULD | แก้ password, 2FA, theme | - Change password<br>- Setup/disable 2FA<br>- Theme preference | ✅ v1.0.0 |
| SE-002 | **User management (admin)** | 🟡 SHOULD | CRUD users | - Create, edit role, delete, reset password<br>- Per-cluster permission assignment | 🚧 v2.0 |
| SE-003 | **Cluster config (admin)** | 🟡 SHOULD | เพิ่ม/แก้ cluster | - Name, URL, credentials<br>- Test connection button | ✅ v1.0.0 |
| SE-004 | **System settings** | 🟡 SHOULD | global config | - Session timeout, rate limits, audit retention<br>- Most require restart | ✅ v1.0.0 |
| SE-005 | **Theme toggle** | 🔴 MUST | dark/light | - Per-user preference | ✅ v1.0.0 |
| SE-006 | **Language (i18n)** | ⏸️ LATER | Thai/English | **✅ Done v1.1.0** | ✅ v1.1.0 |

---

## 12. 📈 Observability (MoxUI self-monitoring)

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| O-001 | **Health endpoint** | 🔴 MUST | `/health`, `/livez`, `/readyz` | - 200 OK with version + uptime<br>- 503 if degraded | ✅ v1.0.0 |
| O-002 | **Prometheus metrics** | 🟡 SHOULD | `/metrics` | - RED metrics (Rate, Errors, Duration)<br>- USE metrics (Proxmox client)<br>- Cache hit/miss<br>- Audit log counter | ✅ v1.0.0 |
| O-003 | **OpenTelemetry tracing** | 🟢 COULD | OTLP export | ✅ v1.0.0 |
| O-004 | **Structured logs (JSON)** | 🟡 SHOULD | Loki-compatible | - All logs include request_id, user_id<br>- Log levels configurable | ✅ v1.0.0 |
| O-005 | **Log viewer (UI)** | ⏸️ LATER | | - Use Loki/Grafana instead | 🚧 v2.0+ |

---

## 13. 🚀 Deployment & Operations

### 13.1 Deployment

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| DP-001 | **Single-container Docker deploy** | 🔴 MUST | one command | - `docker run` works<br>- Health check passes | ✅ v1.0.0 |
| DP-002 | **docker-compose with reverse proxy** | 🔴 MUST | Caddy + MoxUI | - TLS auto via Caddy<br>- Volume for data | ✅ v1.0.0 |
| DP-003 | **Helm chart for K8s** | 🟡 SHOULD | Production K8s deploy | - HPA, PDB, NetworkPolicy, Ingress | ✅ v1.0.0 |
| DP-004 | **systemd unit** | 🟢 COULD | Bare-metal deploy | ✅ v1.0.0 |

### 13.2 Backup & Restore

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| DP-101 | **Backup MoxUI data** | 🟡 SHOULD | SQLite + config | - Shell script + CronJob<br>- Optional off-cluster (S3/NFS) | ✅ v1.0.0 |
| DP-102 | **Restore from backup** | 🟡 SHOULD | Recovery | - Tested runbook | ✅ v1.0.0 |
| DP-103 | **Migration to new version** | 🟡 SHOULD | Upgrade path | - Read old DB, migrate, write new | ✅ v1.0.0 |

### 13.3 Operations

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| DP-201 | **Graceful shutdown** | 🔴 MUST | Drain connections | - SIGTERM → stop accepting → drain → close DB | ✅ v1.0.0 |
| DP-202 | **Auto-restart on crash** | 🔴 MUST | Docker/K8s restart policy | - State preserved in SQLite | ✅ v1.0.0 |
| DP-203 | **Log rotation** | 🟡 SHOULD | ไม่เต็ม disk | - Daily rotate, compress, retain 30 days | ✅ v1.0.0 |

---

## 14. 🎨 UI/UX (cross-cutting)

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| UX-001 | **Responsive layout** | 🔴 MUST | mobile/tablet/desktop | - Breakpoints: 768/1024/1280<br>- Sidebar collapse on mobile | ✅ v1.0.0 |
| UX-002 | **Keyboard shortcuts** | 🟡 SHOULD | ทำงานเร็ว | - g+d / g+v / g+s (navigate)<br>- /, c, ?, Esc | 🚧 v2.0 |
| UX-003 | **Dark/Light theme** | 🔴 MUST | theme toggle | - CSS variables<br>- localStorage persist | ✅ v1.0.0 |
| UX-004 | **Loading states** | 🔴 MUST | skeleton + spinner | - ทุก async action | ✅ v1.0.0 |
| UX-005 | **Empty states** | 🟡 SHOULD | illustration + message | - "No VMs yet", "Connect first cluster" | ✅ v1.0.0 |
| UX-006 | **Error states** | 🟡 SHOULD | retry + error message | - Network error, 500, 404 | ✅ v1.0.0 |
| UX-007 | **Confirm dialogs** | 🔴 MUST | ป้องกัน accidental action | - Type VM name for destructive | ✅ v1.0.0 |
| UX-008 | **Toasts** | 🔴 MUST | success/error feedback | - Top-right, auto-dismiss | ✅ v1.0.0 |
| UX-009 | **Accessibility (WCAG AA)** | 🟡 SHOULD | a11y audit | - ARIA labels, keyboard nav, contrast | 🚧 v2.0 |
| UX-010 | **Internationalization** | ⏸️ LATER | i18n framework | **✅ Done v1.1.0** (English + Thai) | ✅ v1.1.0 |
| UX-011 | **PWA / install prompt** | ⏸️ LATER | install as app | - v2+ | 🚧 v2.0 |

---

## 15. 🔧 Developer Experience

| ID | Feature | Tier | Description | Acceptance Criteria | Status |
|---|---|---|---|---|---|
| DX-001 | **CI (GitHub Actions)** | 🔴 MUST | build + test + clippy + fmt | - Run on every PR | ✅ v1.0.0 |
| DX-002 | **Release workflow** | 🔴 MUST | tagged release + image | - Push image to ghcr.io<br>- Sign binary | ✅ v1.0.0 |
| DX-003 | **Pre-commit hooks** | 🟡 SHOULD | clippy + fmt locally | - Optional but recommended | ✅ v1.0.0 |
| DX-004 | **Makefile** | 🟡 SHOULD | common tasks | - `make build`, `make test`, `make run`, `make docker` | ✅ v1.0.0 |
| DX-005 | **Mock Proxmox server** | 🟡 SHOULD | integration tests | - axum-based mock for tests | ✅ v1.0.0 |
| DX-006 | **Benchmark suite** | 🟢 COULD | criterion benches | ✅ v1.0.0 |

---

## 📊 Summary by Tier

| Tier | Count | % | Completed |
|---|---|---|---|
| 🔴 MUST | ~54 | ~32% | ✅ All |
| 🟡 SHOULD | ~65 | ~38% | ✅ Most |
| 🟢 COULD | ~18 | ~11% | ✅ Most |
| ⏸️ LATER | ~31 | ~19% | ✅ Phase 4+5 done |
| **Total** | **~168** | **100%** | **~75% shipped** |

> ดู ⏸️ LATEGIT features ที่เหลือใน [`FUTURE_ROADMAP.md`](../FUTURE_ROADMAP.md)

## 🎯 Version History

| Version | Focus | Status |
|---|---|---|
| **v1.0.0** | Production-ready MVP (Phases 0–3) | ✅ Shipped |
| **v1.1.0** | Polish & Community (Phase 4) | ✅ Shipped |
| **v1.2.0** | Power User Features (Phase 5) | ✅ Shipped — **Current** |
| **v2.0** | Advanced Cluster Management | 🔜 Q3 2026 |

## 🔗 Cross-cutting Concerns (always required)

| Concern | Status |
|---|---|
| **Security headers** (CSP, HSTS, X-Frame-Options, etc.) | ✅ v1.0.0 |
| **Rate limiting** (login + API) | ✅ v1.0.0 |
| **Input validation** | ✅ v1.0.0 |
| **TLS 1.3 only** | ✅ v1.0.0 |
| **No secrets in logs** | ✅ v1.0.0 |
| **Parameterised queries** (no SQLi) | ✅ v1.0.0 |
| **Audit log on every mutation** | ✅ v1.0.0 |
| **Structured logging** | ✅ v1.0.0 |
| **Metrics** | ✅ v1.0.0 |
| **Tracing** | ✅ v1.0.0 |
| **Run as non-root** | ✅ v1.0.0 |
| **Read-only container rootfs** | ✅ v1.0.0 |
