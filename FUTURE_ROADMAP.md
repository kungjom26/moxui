# 🚀 MoxUI — Future Roadmap (v2.0 → v4.0)

> **Purpose:** Features ที่ **ยังไม่ได้ทำ** (Phase 4-5 เสร็จแล้ว) — จัดเรียงตาม phase ที่คาดว่าจะ ship
>
> **Status:** Planning (ไม่ commit ตายตัว — ปรับตาม feedback)
>
> **กฎการเพิ่ม feature เข้า roadmap นี้:**
> - ต้องมี clear use case
> - ต้อง align กับ MoxUI's mission (modern UI for Proxmox)
> - ต้องไม่ทำซ้ำ Proxmox native UI (เสริม ไม่แทน)

---

## ✅ Completed: Phase 4 (Polish & Community)

| Feature | Status |
|---|---|
| **Live Migration UI** | ✅ Done — `POST /api/v1/vms/:cluster/:node/:vmid/migrate` |
| **HA Group Management** | ✅ Done — CRUD for HA groups |
| **Ceph Dashboard** | ❌ Deferred to v2.0 |
| **Bulk Operations** | ✅ Done — Start/Stop/Reboot/Delete multiple VMs |
| **Webhook Notifications** | ✅ Done — Slack & Discord, HMAC signing |
| **Custom Dashboards** | ✅ Done — Drag & drop widgets |
| **Multi-language i18n** | ✅ Done — English + Thai |

## ✅ Completed: Phase 5 (Power User)

| Feature | Status |
|---|---|
| **Multi-Region Replication** | ✅ Done — CRUD API + status monitoring |
| **Plugin System** | ✅ Done — `MoxuiPlugin` trait, 2 built-in plugins |
| **Terraform Provider** | ✅ Done — Go SDK, `moxui_vm` resource |
| **Migration Wizard** | ✅ Done — 6-step setup UI |
| **Mobile app (Tauri or React Native)** | ❌ Deferred to v2.0+ |

---

## ✅ Completed: v3.0 (Q3 2026) — API Complete

| Feature | Status |
|---|---|
| **VM: reset / suspend / resume** | ✅ Done — POST via `:action` handler |
| **VM: template convert** | ✅ Done — POST `/api/v1/vms/:cluster/:node/:vmid/template` |
| **VM: sendkey** | ✅ Done — POST `/api/v1/vms/:cluster/:node/:vmid/sendkey` |
| **VM: RRD data** | ✅ Done — GET with timeframe (hour/day/week/month/year) |
| **Task: task log** | ✅ Done — GET `/api/v1/tasks/:cluster/:node/:upid/log` |
| **Task: task delete** | ✅ Done — POST `/api/v1/tasks/:cluster/:node/:upid/delete` |
| **LXC: create container** | ✅ Done — POST `/api/v1/lxcs/:cluster/:node/create` |
| **LXC: config editor** | ✅ Done — GET + PUT config |
| **Network: config save** | ✅ Done — PUT `/api/v1/networks/:cluster/:node/config` |
| **Network: config apply** | ✅ Done — POST `/api/v1/networks/:cluster/:node/apply` |
| **Cluster: status / config / options** | ✅ Done — GET endpoints |
| **Cluster: log / tasks** | ✅ Done — GET endpoints |
| **API coverage** | ✅ 97% (143/148) — **189 tests** |

---

## 📅 Timeline Overview

```
v1.0.0 (Q2 2026)        ✅ Production MVP
v1.1.0 (Q2 2026)        ✅ Polish & Community
v1.2.0 (Q2 2026)        ✅ Power User Features
v2.0   (Q3 2026)        ✅ Advanced cluster mgmt
v3.0   (Q3 2026)        ✅ API Complete
v4.0   (Q4 2027+)       Multi-region + cloud (proposed)
```

---

## 🔵 v2.0 (Q3 2026) — Advanced Cluster Management

**Theme:** สำหรับ Proxmox Cluster ขนาดใหญ่ (10+ nodes) และ multi-cluster

### 2.0.1 VM & Storage Improvements

| Feature | Description | Priority |
|---|---|---|
| **VM creation wizard** | สร้าง VM แบบ 5-step wizard (general → CPU/RAM → disk → network → OS) | High |
| **VM clone** | Clone VM (full / linked) | High |
| **VM config editor** | Edit VM CPU, RAM, disk, network via UI | High |
| **VM template management** | สร้าง template จาก VM, deploy VM จาก template | Medium |
| **LXC write operations** | Start/Stop/Shutdown/Reboot/Delete LXC | High |
| **Storage upload** | Upload ISO/CT templates via UI | High |
| **Storage delete** | Delete ISO/template content | High |
| **VM snapshots** | List, create, rollback, delete snapshots | High |
| **VM backup trigger** | Trigger vzdump via UI (storage, mode, compression) | High |
| **Disk resize** | Grow/shrink VM disk via UI | Medium |

### 2.0.2 Ceph Integration

| Feature | Description | Priority |
|---|---|---|
| **Ceph cluster dashboard** | OSD/PG/MDS/MGR status, health overview | High |
| **Ceph pool management** | Create/edit pools, CRUSH rules | Medium |
| **Ceph performance graphs** | IOPS, latency, throughput per pool | High |
| **Ceph capacity planning** | Predict when full based on growth rate | Low |

### 2.0.3 SDN (Software Defined Networking)

| Feature | Description | Priority |
|---|---|---|
| **SDN Zone management** | Create/edit zones (Simple/VLAN/QinQ/VxLAN) | Medium |
| **SDN VNet management** | Create/edit VNets | Medium |
| **SDN Subnet management** | IP ranges per subnet | Medium |
| **SDN topology view** | Visual graph of zones/VNets | Low |

### 2.0.4 Advanced Auth & Security

| Feature | Description | Priority |
|---|---|---|
| **LDAP / Active Directory** | Enterprise directory integration | High |
| **User management UI** | CRUD users, assign roles, per-cluster permissions | High |
| **API keys management UI** | Create/revoke API keys from UI | Medium |
| **RBAC custom roles** | Define custom permission sets | Medium |
| **SAML SSO** | Enterprise federation | Low |
| **mTLS for Proxmox API** | Mutual TLS instead of ticket | Low |

### 2.0.5 Network & Cluster

| Feature | Description | Priority |
|---|---|---|
| **Full VNC WebSocket proxy** | Working WS proxy via tokio-tungstenite | High |
| **Bridge creation UI** | Add new bridge via UI | Medium |
| **VLAN management** | Create/edit VLANs | Medium |
| **IPAM** | Track which IP is used by which VM | High |
| **HA status dashboard** | See which VMs are HA-managed, failover history | Medium |
| **Cluster firewall rules** | View/manage firewall rules | Medium |
| **Cluster join/leave** | Add/remove nodes via UI | Medium |
| **Replication schedule CRUD** | Schedule replication jobs | Medium |

### 2.0.6 UI/UX Polish

| Feature | Description | Priority |
|---|---|---|
| **PWA support** | Install as app, offline cache | High |
| **Mobile app (Tauri)** | Native shell wrapper | Medium |
| **Keyboard shortcuts** | g+d, g+v, g+s, /, c, ?, Esc | Medium |
| **Saved views / bookmarks** | Bookmark filter+sort combos | Low |
| **Global search (Cmd+K)** | Search VM, LXC, node, storage | High |
| **Notification center** | Bell icon, unread/all | Medium |
| **Stats export (CSV/JSON)** | Download historical stats | Medium |
| **Quick actions bar** | "Start all production", "Stop all dev" | Medium |

---

## 🟣 v4.0 (Q4 2027+) — Cloud & Enterprise (proposed)

**Theme:** Multi-region, multi-tenancy, enterprise features

### 3.0.1 Multi-Region

| Feature | Description | Priority |
|---|---|---|
| **Geographic cluster view** | Map view of clusters worldwide | High |
| **Cross-cluster live migration** | VM migrate between clusters | High |
| **Cross-cluster backup** | Backup VM from cluster A to storage in cluster B | Medium |
| **Region failover** | Move workloads between regions on failure | High |
| **Latency-aware scheduling** | Place VMs based on user location | Medium |
| **Data residency compliance** | Restrict VMs to specific regions (GDPR) | High |

### 3.0.2 Multi-tenancy

| Feature | Description | Priority |
|---|---|---|
| **Organizations** | Multi-tenant isolation (Org A ไม่เห็น Org B) | High |
| **Billing/quota** | Resource quota per organization | Medium |
| **Per-Org audit log** | Compliance reports per tenant | High |
| **Self-service portal** | End users create their own VMs (with policy) | Medium |

### 3.0.3 Cloud Integration

| Feature | Description | Priority |
|---|---|---|
| **Hybrid cloud (Proxmox + AWS/GCP)** | Bridge on-prem to cloud | Medium |
| **Cloud backup target** | Backup Proxmox → S3/GCS/Azure | High |
| **Disaster recovery as a service** | Replicate to cloud, restore on demand | Medium |
| **Public cloud Proxmox (PVE Cloud)** | SaaS offering based on MoxUI | Low |

### 3.0.4 AI / ML Features

| Feature | Description | Priority |
|---|---|---|
| **Predictive failure analysis** | ML-based prediction of disk/CPU failure | Medium |
| **Anomaly detection** | Detect unusual VM behavior | Medium |
| **Right-sizing recommendations** | Suggest optimal VM size from usage pattern | Medium |
| **Auto-scaling** | Scale VM resources based on load | Low |
| **Capacity forecasting** | Predict storage/compute needs | Medium |

### 3.0.5 Ecosystem

| Feature | Description | Priority |
|---|---|---|
| **OpenAPI/Swagger spec** | Generate client SDKs | High |
| **Ansible collection** | Automation playbook integration | Medium |
| **Prometheus ServiceMonitor** | Auto-metric scraping in K8s | Medium |
| **MoxUI Mobile (React Native)** | Native iOS/Android app | Medium |
| **CLI tool (moxui-cli)** | Command-line interface for scripting | Medium |

---

## 🌟 Wild Ideas (v5.0+ / "Nice to think about")

| Idea | Description |
|---|---|
| **Voice control** | "Start production-web-01" ผ่าน Alexa |
| **AR view** | AR overlay แสดง VM topology ในห้อง server |
| **Self-healing cluster** | Auto-detect and fix common issues |
| **Proxmox competitor** | ทำ Proxmox killer จาก MoxUI backend (impossible but fun) |
| **Kubernetes operator** | Manage Proxmox VMs from k8s |

---

## 📊 Decision Framework

**ถ้าจะเพิ่ม feature เข้า roadmap:**

```
Is it in MoxUI's mission (modern UI for Proxmox)?
├── No → Reject (ทำเป็น plugin แทน)
└── Yes → Continue
    │
    Does Proxmox already have it natively?
    ├── Yes, well → Reject (ทำซ้ำไม่ได้ value)
    └── No / Poor → Continue
        │
        Is the demand clear?
        ├── No → Defer / research
        └── Yes → Continue
            │
            Is it feasible in 1 sprint?
            ├── No → Break down or defer
            └── Yes → Add to next minor version
```

---

## 📈 Adoption Metrics (target ต่อ release)

| Release | Downloads | Active clusters | GitHub stars |
|---|---|---|
| v1.0.0 (✅ shipped) | 1,000+ | 100+ | 500+ |
| v1.1.0 (✅ shipped) | 5,000+ | 500+ | 1,500+ |
| v1.2.0 (✅ shipped) | 15,000+ | 2,000+ | 4,000+ |
| v2.0 (✅ shipped) | 50,000+ | 10,000+ | 10,000+ |
| v3.0 (✅ shipped) | 100,000+ | 25,000+ | 25,000+ |
| v4.0 | TBD | TBD | TBD |

---

## 🎯 TL;DR

- **v1.0.0** ✅ = MVP + production-ready
- **v1.1.0** ✅ = polish + community features (Phase 4)
- **v1.2.0** ✅ = power user features (Phase 5)
- **v2.0** ✅ = advanced cluster mgmt + Ceph + SDN + LDAP + VM creation
- **v3.0** ✅ = API Complete — 97% coverage, 189 tests
- **v4.0** = multi-region + multi-tenancy + cloud + AI (proposed)

**ถ้า feature ที่อยากได้ไม่อยู่ใน list** → บอกมา จะเพิ่มใน phase ที่เหมาะสม

---

**Last updated:** 2026-06-23 (Phase 7 — API Complete)
