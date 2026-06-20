# 🚀 MoxUI — Future Roadmap (v1.2 → v3.0)

> **Purpose:** Features ที่ **ไม่อยู่ใน v1.0.0** แต่มีแผนจะทำในอนาคต — จัดเรียงตาม phase ที่คาดว่าจะ ship
>
> **Status:** Planning (ไม่ commit ตายตัว — ปรับตาม feedback)
>
> **กฎการเพิ่ม feature เข้า roadmap นี้:**
> - ต้องมี clear use case
> - ต้อง align กับ MoxUI's mission (modern UI for Proxmox)
> - ต้องไม่ทำซ้ำ Proxmox native UI (เสริม ไม่แทน)

---

## 📅 Timeline Overview

```
v1.0.0 (Q3 2026)        MVP + Production-ready
v1.1   (Q4 2026)        Polish + community features
v1.2   (Q1 2027)        Power user features
v2.0   (Q3 2027)        Major: advanced cluster mgmt
v3.0   (Q4 2027)        Multi-region + cloud
```

---

## 🟢 v1.1 (Q4 2026) — Polish & Community

**Theme:** ขัดเกลา UX + เพิ่ม feature ที่ user community ขอ

### 1.1.1 VM Improvements

| Feature | Description | Priority |
|---|---|---|
| **Bulk operations** | Start/stop/tag/delete หลาย VM พร้อมกัน พร้อม progress bar | High |
| **VM create wizard** | สร้าง VM แบบ 5-step wizard (general → CPU/RAM → disk → network → OS) | High |
| **VM clone** | Clone VM (full / linked) | Medium |
| **VM migrate** | Online migrate ระหว่าง nodes ภายใน cluster | Medium |
| **VM template management** | สร้าง template จาก VM, deploy VM จาก template | Medium |
| **Custom column chooser** | ให้ user เลือก column ที่จะแสดงใน VM list | Low |

### 1.1.2 UI/UX Polish

| Feature | Description | Priority |
|---|---|---|
| **PWA support** | Install เป็น app, offline cache | High |
| **Mobile app (Tauri)** | Native shell wrapper for desktop shortcut | Medium |
| **Theme: more options** | Solarized, Monokai, custom accent color | Low |
| **Dashboard customization** | Reorder widgets, hide/show | Medium |
| **Quick actions bar** | "Start all production", "Stop all dev" | Medium |
| **Saved views** | Bookmark filter+sort combos | Low |

### 1.1.3 Observability Enhancements

| Feature | Description | Priority |
|---|---|---|
| **OpenTelemetry tracing** | OTLP export to Jaeger/Tempo | High |
| **Stats export (CSV/JSON)** | Download historical stats | Medium |
| **Custom date range** | Stats chart: any date range | Low |
| **Comparison view** | Compare 2 VMs side-by-side | Low |

### 1.1.4 Quality of Life

| Feature | Description | Priority |
|---|---|---|
| **Bulk import (CSV/JSON)** | Bulk create VMs from spreadsheet | Medium |
| **CLI tool (moxui-cli)** | Command-line interface for scripting | High |
| **Terraform provider** | Infrastructure-as-Code integration | High |
| **Ansible collection** | Automation playbook integration | Medium |

---

## 🟢 v1.2 (Q1 2027) — Power User Features

**Theme:** สำหรับ admin ที่ต้องการ advanced control

### 1.2.1 Advanced VM

| Feature | Description | Priority |
|---|---|---|
| **VM hardware hot-plug** | Add/remove disk, NIC, RAM without reboot | High |
| **VM resource limits** | CPU quota, IOPS limit, network bandwidth cap | High |
| **VM console recording** | Record VNC session to file (webm) | Medium |
| **Multi-monitor console** | Multiple display in console | Medium |
| **VM console sharing** | Share console URL with read-only access | Medium |

### 1.2.2 Advanced Auth

| Feature | Description | Priority |
|---|---|---|
| **WebAuthn / Passkey (full)** | Yubikey, Touch ID, Windows Hello — complete rollout | High |
| **OIDC SSO (GitHub, Okta, Auth0)** | เพิ่ม provider อื่นนอกจาก Google | High |
| **LDAP / Active Directory** | Enterprise directory integration | Medium |
| **SAML SSO** | Enterprise federation | Low |
| **Custom role builder** | สร้าง role เอง กำหนด permission แต่ละตัว | Medium |
| **API keys (machine-to-machine)** | Long-lived tokens for scripts | High |
| **mTLS for Proxmox API** | Mutual TLS instead of ticket | Low |

### 1.2.3 Storage

| Feature | Description | Priority |
|---|---|---|
| **Create storage pool** | Add new storage via UI (LVM/ZFS/dir) | Medium |
| **Storage migration** | Move VM disk between storage | Medium |
| **Disk resize** | Grow/shrink VM disk | Medium |
| **Disk import/export** | Upload/download disk images (qcow2, raw, vmdk) | Medium |

### 1.2.4 Network

| Feature | Description | Priority |
|---|---|---|
| **Bridge creation** | Add new bridge via UI | Medium |
| **VLAN management** | Create/edit VLANs | Medium |
| **IPAM (IP address management)** | Track which IP is used by which VM | High |
| **Network traffic analysis** | Per-VM bandwidth over time | Low |

### 1.2.5 HA & Cluster

| Feature | Description | Priority |
|---|---|---|
| **HA group management** | Configure HA groups, priorities | Medium |
| **HA status dashboard** | See which VMs are HA-managed, failover history | High |
| **Cluster join/leave** | Add/remove nodes via UI | Medium |
| **Cluster backup config** | Backup Proxmox cluster config | Low |

---

## 🔵 v2.0 (Q3 2027) — Advanced Cluster Management

**Theme:** สำหรับ Proxmox Cluster ขนาดใหญ่ (10+ nodes) และ multi-cluster

### 2.0.1 Multi-cluster Advanced

| Feature | Description | Priority |
|---|---|---|
| **Cross-cluster live migration** | VM migrate ระหว่าง clusters | High |
| **Cross-cluster replication** | Replicate VM ระหว่าง clusters (async) | High |
| **Cross-cluster backup** | Backup VM from cluster A to storage in cluster B | Medium |
| **Cluster federation view** | แสดงทุก cluster ในมุมมองเดียว (geographic) | Medium |

### 2.0.2 Ceph Integration

| Feature | Description | Priority |
|---|---|---|
| **Ceph cluster dashboard** | OSD/PG/MDS/MGR status | High |
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

### 2.0.4 Advanced Storage

| Feature | Description | Priority |
|---|---|---|
| **ZFS pool management** | Create/edit ZFS pools, datasets | Medium |
| **ZFS snapshots UI** | Browse/restore ZFS snapshots | Medium |
| **iSCSI target management** | Add/remove iSCSI targets | Low |
| **NFS share management** | Add/remove NFS exports | Low |

### 2.0.5 VM Advanced

| Feature | Description | Priority |
|---|---|---|
| **Live migration UI** | Trigger + monitor live migration across nodes | High |
| **Cross-host live migration w/o shared storage** | Use replication-based migration | High |
| **Resource overcommit warnings** | Alert when overcommit ratio too high | Medium |
| **Right-sizing recommendations** | ML-based suggestion for optimal VM size | Low |

---

## 🟣 v3.0 (Q4 2027) — Cloud & Enterprise

**Theme:** Multi-region, multi-tenancy, enterprise features

### 3.0.1 Multi-Region

| Feature | Description | Priority |
|---|---|---|
| **Geographic cluster view** | Map view of clusters worldwide | High |
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
|---|---hook---|---|
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
| **Plugin system** | 3rd-party plugins (custom pages, widgets) | High |
| **Webhook integrations** | Slack, Discord, Teams, PagerDuty | High |
| **Prometheus exporter** | Expose MoxUI metrics externally | Medium |
| **OpenAPI/Swagger spec** | Generate client SDKs | High |
| **MoxUI Mobile (React Native)** | Native iOS/Android app | Medium |

---

## 🌟 Wild Ideas (v4.0+ / "Nice to think about")

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

## 🔄 How to contribute

**Community สามารถ contribute feature ใหม่ได้:**

1. เปิด GitHub Issue ที่ `kungjom26/moxui` (template: Feature Request)
2. ระบุ use case + alternative + ความคล้ายกับ feature ที่มีอยู่
3. รอ maintainer review
4. ถ้า approved → เพิ่มใน roadmap ที่ phase ที่เหมาะสม

---

## 📈 Adoption Metrics (target ต่อ release)

| Release | Downloads | Active clusters | GitHub stars |
|---|---|---|---|
| v1.0.0 | 1,000+ | 100+ | 500+ |
| v1.1 | 5,000+ | 500+ | 1,500+ |
| v1.2 | 15,000+ | 2,000+ | 4,000+ |
| v2.0 | 50,000+ | 10,000+ | 10,000+ |
| v3.0 | 100,000+ | 25,000+ | 25,000+ |

---

## 🎯 TL;DR

- **v1.0.0** = MVP + production-ready
- **v1.1** = polish + community requests
- **v1.2** = power user features + advanced auth
- **v2.0** = advanced cluster mgmt + Ceph + SDN
- **v3.0** = multi-region + multi-tenancy + cloud + AI

**ถ้า feature ที่อยากได้ไม่อยู่ใน list** → บอกมา จะเพิ่มใน phase ที่เหมาะสม

---

**Last updated:** 2026-06-20 (synced with PROPOSAL.md decisions)