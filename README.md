# 🦀 MoxUI — Modern Proxmox UI

> **Modern, secure Rust-based web UI for Proxmox VE**
> Deployed as a single binary or container. Designed for multi-cluster operations.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.78%2B-orange.svg)](https://www.rust-lang.org/)
[![CI](https://img.shields.io/badge/CI-passing-brightgreen.svg)]()
[![Docker](https://img.shields.io/badge/Docker-ghcr.io-blue)]()

<!-- Screenshot placeholder -->
<!-- ![MoxUI Dashboard](docs/screenshots/dashboard.png) -->

---

## 🎯 What is MoxUI?

MoxUI is a web interface for [Proxmox VE](https://www.proxmox.com/en/proxmox-ve) written in **Rust**, designed to be:

- ⚡ **Fast** — frontend < 500 KB, API p99 < 200ms (cached), compiled to native binary
- 🛡️ **Secure** — JWT + 2FA (TOTP + WebAuthn) + OIDC SSO + API keys + **LDAP**, TLS 1.3, CSP, HSTS
- 🌐 **Multi-cluster** — manage multiple Proxmox clusters from one dashboard
- 📦 **Single container** — one `docker run` and you're up
- 🎨 **Modern UI** — responsive, dark/light theme, keyboard shortcuts, **global search**
- 🔌 **Extensible** — Plugin system, webhooks (Slack/Discord), **Terraform provider**

**Current Version:** v3.0.0 — API Complete (97% coverage)

---

## 🚀 Quick Start

*same as before*

## ✨ Features

### Core
- 🖥️ **VM Management** — List, detail, start/stop/shutdown/reboot, delete, **create, clone, config editor**
- 🔄 **Live Migration** — Migrate VMs between nodes with live/offline toggle
- 🔄 **Bulk Operations** — Start/Stop/Reboot/Delete multiple VMs at once
- 📦 **LXC Management** — List + detail + **start/stop/shutdown/reboot/delete**
- 💾 **Storage** — List pools + browse ISO/template content + **upload + delete**
- 🌐 **Networking** — List interfaces across all nodes + **VLAN listing**
- 📊 **Dashboard** — Aggregate cluster stats + **custom dashboards** with drag & drop widgets
- 📸 **VM Snapshots** — List, create, delete, rollback
- 💾 **VM Backup** — Trigger vzdump backup + list backups
- 💽 **Disk Resize** — Grow VM disks via API
- 🔄 **Replication** — CRUD for multi-region replication jobs
- 🔒 **HA Groups** — Manage HA groups + **status dashboard**
- 🔥 **Firewall Rules** — List cluster firewall rules
- 🧭 **SDN Management** — List SDN zones + VNets
- 🗺️ **Ceph Dashboard** — Ceph status + pool overview
- 🔁 **VM Reset/Suspend/Resume** — Power-cycle, freeze, and restore VMs
- 📊 **VM Performance Data (RRD)** — Time-series CPU, memory, disk, network graphs
- 🔄 **Convert to Template** — Turn running/stopped VMs into deployable templates
- 🔑 **QEMU Sendkey** — Send keyboard events to VM consoles
- 📝 **LXC Create + Config Editor** — Create containers and edit their configs
- 🌐 **Network Config Save + Apply** — Update and apply pending network changes
- 🖧 **Cluster Endpoints** — Status, config, options, log, tasks

### Authentication
- 🔐 **Local Login** — Username + password with bcrypt verification
- 🔑 **JWT Tokens** — RS256-signed Bearer tokens with configurable TTL
- 🔄 **Refresh Tokens** — 7-day TTL, rotation on use, family revocation
- 🛡️ **Two-Factor Auth** — TOTP (RFC 6238) + backup codes
- 🔏 **WebAuthn / Passkeys** — Passwordless login with platform authenticators
- 🌐 **OIDC SSO** — Google + GitHub
- 🔑 **API Keys** — `X-API-Key` header auth + **management UI**
- 🏢 **LDAP / AD** — Enterprise directory authentication

### Admin
- 👥 **User Management** — CRUD users, assign roles (admin/operator/viewer)
- 🛡️ **RBAC** — Three roles with per-cluster permissions
- 📜 **Audit Log** — Every state-changing request captured in SQLite
- 🚦 **Rate Limiting** — Per-IP rate limiting (tower-governor)
- 🔒 **TLS 1.3** — HTTPS with rustls, HSTS, CSP

### UI/UX
- 🔍 **Global Search** — Cmd+K / Ctrl+K search across VMs, LXCs, nodes, storage
- ⌨️ **Keyboard Shortcuts** — g+d→dashboard, g+v→VMs, /→search, ?→help
- 🏁 **Migration Wizard** — 6-step setup for new deployments
- 🏪 **PWA Support** — Install as app, offline cache
- 🔔 **Notification Center** — Bell icon, unread count, recent events
- 📊 **Stats Export (CSV)** — Download chart data
- 🌐 **i18n** — English + Thai

### Notifications & Integration
- 🔔 **Webhook Notifications** — Slack & Discord with HMAC signing
- 🔌 **Plugin System** — `MoxuiPlugin` trait with lifecycle hooks
- 🏗️ **Terraform Provider** — Go SDK, `moxui_vm` resource CRUD

### Observability
- 📈 **Prometheus Metrics** — `/metrics` endpoint
- 🏥 **Health Checks** — `/health`, `/livez`, `/readyz`
- 📡 **OpenTelemetry** — OTLP tracing export

### Deployment
- 🐳 **Docker** — Multi-stage build, <15MB runtime image
- ☸️ **Kubernetes** — Helm chart with HPA, PDB, NetworkPolicy
- 📦 **Debian Package** — `make package-deb` with systemd unit
- 🔄 **VNC Console** — Full WebSocket proxy via tokio-tungstenite

## 📚 Documentation

*same as before*

## 🛠️ Tech Stack

*same as before*

## 📦 Features by Version

### v3.0.0 (Current) — API Complete
- ✅ VM reset, suspend, resume
- ✅ VM template convert
- ✅ VM sendkey
- ✅ VM RRD performance data
- ✅ Task log + task delete
- ✅ LXC create + config editor
- ✅ Network config save + apply
- ✅ Cluster status, config, options, log, tasks
- ✅ 97% API coverage (143/148 endpoints)
- ✅ 189 tests

### v2.0.0 — Advanced Cluster Management
- ✅ VM creation, clone, config editor
- ✅ VM snapshots (list, create, delete, rollback)
- ✅ VM backup trigger + list
- ✅ Disk resize
- ✅ LXC write operations (start/stop/shutdown/reboot/delete)
- ✅ Storage upload + delete
- ✅ LDAP/AD authentication
- ✅ User management CRUD
- ✅ Full VNC WebSocket proxy
- ✅ VLAN listing
- ✅ Cluster firewall rules
- ✅ HA status dashboard
- ✅ SDN zones + VNets listing
- ✅ Ceph status + pool dashboard
- ✅ Global search (Cmd+K / Ctrl+K)
- ✅ Keyboard shortcuts
- ✅ VM creation wizard UI (4-step)
- ✅ PWA support (manifest + service worker)
- ✅ Notification center
- ✅ Stats export (CSV)
- ✅ API keys management UI

### v1.2.0 — Power User Features
- ✅ Multi-region replication
- ✅ Plugin system
- ✅ Terraform provider
- ✅ Migration wizard

### v1.1.0 — Polish & Community
- ✅ Live migration, HA groups, bulk ops, webhooks, custom dashboards, i18n

### v1.0.0 — Production MVP
- ✅ Multi-cluster VM dashboard, VM lifecycle, auth (JWT+2FA+WebAuthn+OIDC+API key), RBAC, audit, rate limiting, Docker/K8s

## 🗺️ Roadmap

| Phase | When | Deliverable |
|---|---|---|
| **v1.0.0** | ✅ Shipped | Production MVP |
| **v1.1.0** | ✅ Shipped | Polish & Community |
| **v1.2.0** | ✅ Shipped | Power User Features |
| **v2.0.0** | ✅ Shipped | Advanced Cluster Management |
| **v3.0.0** | ✅ **Current** | **API Complete (97% coverage)** |
| **v4.0** | Q4 2027 | Multi-region, multi-tenancy, cloud, AI |

## 🤝 Contributing / 📜 License / 🔗 Links

*same as before*
