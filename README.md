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
- 🛡️ **Secure** — JWT + 2FA (TOTP + WebAuthn) + OIDC SSO + API keys, TLS 1.3, CSP, HSTS
- 🌐 **Multi-cluster** — manage multiple Proxmox clusters from one dashboard
- 📦 **Single container** — one `docker run` and you're up
- 🎨 **Modern UI** — responsive, dark/light theme, keyboard shortcuts
- 🔌 **Extensible** — Plugin system, webhooks (Slack/Discord), Terraform provider

**Current Version:** v1.2.0 — Power User Features

---

## 🚀 Quick Start

### Docker (recommended)

```bash
# Generate JWT keys
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out jwt_priv.pem
openssl pkey -in jwt_priv.pem -pubout -out jwt_pub.pem

# Create config directory
mkdir -p moxui-config

# Create config.yaml (see config.example.yaml)
cat > moxui-config/config.yaml << 'EOF'
server:
  bind: "0.0.0.0:8080"
database:
  path: "/var/lib/moxui/data/moxui.db"
logging:
  level: "info"
  format: "json"
clusters:
  - name: "homelab"
    url: "https://192.168.1.11:8006"
    username: "root@pam"
    password: "${MOXUI_PROXMOX_PASSWORD}"
    realm: "pam"
    insecure_skip_verify: false
auth:
  jwt_private_key_pem_path: "/etc/moxui/jwt_priv.pem"
  jwt_public_key_pem_path: "/etc/moxui/jwt_pub.pem"
  users:
    - id: "u-admin"
      username: "admin"
      password_hash: "$2b$12$..."
      role: "admin"
EOF

# Run
docker run -d \
  --name moxui \
  -p 8080:8080 \
  -v $(pwd)/moxui-config:/etc/moxui:ro \
  -v moxui-data:/var/lib/moxui/data \
  -e MOXUI_PROXMOX_PASSWORD=your_proxmox_password \
  ghcr.io/kungjom26/moxui:latest
```

Access **http://localhost:8080** → login → connect your Proxmox cluster.

> **Note:** The config above uses plain HTTP. Add `server.tls` or use a reverse proxy (Caddy, nginx) for production TLS.

### docker-compose

```yaml
# docker-compose.yml
services:
  moxui:
    image: ghcr.io/kungjom26/moxui:latest
    restart: unless-stopped
    ports:
      - "8080:8080"
    volumes:
      - ./config.yaml:/etc/moxui/config.yaml:ro
      - ./jwt_priv.pem:/etc/moxui/jwt_priv.pem:ro
      - ./jwt_pub.pem:/etc/moxui/jwt_pub.pem:ro
      - moxui_data:/var/lib/moxui/data
    environment:
      - MOXUI_PROXMOX_HOMELAB_PASSWORD=${MOXUI_PROXMOX_HOMELAB_PASSWORD}

volumes:
  moxui_data:
```

### Binary

```bash
# Download pre-built binary from GitHub releases
# or build from source:
cargo install moxui
moxui --config /etc/moxui/config.yaml
```

### Kubernetes (Helm)

```bash
helm upgrade --install moxui ./deploy/k8s/moxui \
  --namespace moxui --create-namespace \
  --set secrets.jwtPrivateKey="$(cat jwt_priv.pem)" \
  --set secrets.jwtPublicKey="$(cat jwt_pub.pem)" \
  --set ingress.host=moxui.example.com
```

---

## ✨ Features

### Core
- 🖥️ **VM Management** — List, detail, start/stop/shutdown/reboot, delete (with purge/force/skiplock)
- 🔄 **Live Migration** — Migrate VMs between nodes with live/offline toggle
- 🔄 **Bulk Operations** — Start/Stop/Reboot/Delete multiple VMs at once
- 📦 **LXC Management** — List + detail (read-only)
- 💾 **Storage** — List pools + browse ISO/template content
- 🌐 **Networking** — List interfaces (bridges, bonds, VLANs, physical) across all nodes
- 📊 **Dashboard** — Aggregate cluster stats + **custom dashboards** with drag & drop widgets
- 🔄 **Replication** — CRUD for multi-region replication jobs with status monitoring
- 🔒 **HA Groups** — Manage High Availability groups (create, edit, delete)

### Authentication
- 🔐 **Local Login** — Username + password with bcrypt verification
- 🔑 **JWT Tokens** — RS256-signed Bearer tokens with configurable TTL
- 🔄 **Refresh Tokens** — 7-day TTL, rotation on use, SHA-256 hashed storage, family revocation
- 🛡️ **Two-Factor Auth** — TOTP (RFC 6238) + backup codes
- 🔏 **WebAuthn / Passkeys** — Passwordless login with platform authenticators
- 🌐 **OIDC SSO** — Google (OpenID Connect) + GitHub (OAuth2)
- 🔑 **API Keys** — `X-API-Key` header authentication for automation

### Security
- 🛡️ **RBAC** — Three roles: `admin` / `operator` / `viewer` with per-cluster permissions
- 📜 **Audit Log** — Every state-changing request captured in SQLite
- 🚦 **Rate Limiting** — Per-IP rate limiting (tower-governor)
- 🔒 **TLS 1.3** — HTTPS with rustls (optional), HSTS, CSP, X-Frame-Options
- 🌍 **CORS** — Configurable allowed origins
- 🧹 **Secret Hygiene** — Passwords in `SecretString` (zeroed on drop)

### Observability
- 📈 **Prometheus Metrics** — `/metrics` endpoint
- 🏥 **Health Checks** — `/health`, `/livez`, `/readyz` (Kubernetes-ready)
- 🔍 **Structured Logging** — JSON output for log aggregation
- 📡 **OpenTelemetry** — OTLP tracing export (Jaeger, Tempo, etc.)

### Notifications & Integration
- 🔔 **Webhook Notifications** — Slack & Discord integration with HMAC signing
- 🔌 **Plugin System** — Extensible via `MoxuiPlugin` trait (audit logger, webhook bridge included)
- 🏗️ **Terraform Provider** — Infrastructure-as-Code with `moxui_vm` resource
- 🌐 **i18n** — English + Thai (extensible via `locales/*.json`)

### Deployment
- 🐳 **Docker** — Multi-stage build, <15MB runtime image, non-root user
- ☸️ **Kubernetes** — Helm chart with HPA, PDB, NetworkPolicy, ServiceMonitor
- 📦 **Debian Package** — `make package-deb` for bare-metal install
- 🏭 **systemd** — Hardened service unit with `ProtectSystem=strict`
- 🔄 **VNC Console** — Secure token-based VM console via noVNC (ticket endpoint ready, WS proxy incoming)
- 🏁 **Migration Wizard** — 6-step setup UI for new deployments

---

## 📚 Documentation

| Doc | Description |
|---|---|
| [Installation Guide](docs/installation.md) | Build & install: pre-built binary, cargo install, Docker |
| [Configuration Reference](docs/configuration.md) | All YAML fields, env vars, defaults |
| [Authentication Guide](docs/authentication.md) | Auth flow: local, 2FA, WebAuthn, OIDC, API keys |
| [Deployment Guide](docs/deployment.md) | Production: Docker, K8s/Helm, TLS, reverse proxy |
| [Development Guide](docs/development.md) | Setup dev env, code structure, testing, CI |
| [Proxmox API Coverage](docs/proxmox-api-coverage.md) | Which API endpoints are supported |

---

## 🛠️ Tech Stack

| Layer | Tech |
|---|---|
| Backend | Rust 1.78+ · axum 0.7 · tokio · rusqlite · reqwest |
| Frontend | Alpine.js · Tailwind CSS · uPlot · noVNC |
| Auth | JWT (RS256) · bcrypt · TOTP (RFC 6238) · WebAuthn · OIDC · OAuth2 |
| Plugins | `MoxuiPlugin` trait · PluginRegistry · Lifecycle hooks |
| IaC | Terraform provider (Go SDK v2) |
| Replication | CRUD API · Job status monitoring |
| Notifications | Slack · Discord · HMAC · Retry with backoff |
| i18n | JSON locale files · `$t()` key-based translation |
| Deployment | Docker · docker-compose · Helm · systemd |
| Observability | Prometheus · OpenTelemetry · tracing |

---

## 📦 Features by Version

### v1.2.0 (Current)
- ✅ Phase 4 + Phase 5 features (see below)

### v1.1.0 — Polish & Community
- ✅ **Live Migration UI** — Migrate VMs between nodes with target + live flag
- ✅ **HA Group Management** — CRUD for High Availability groups
- ✅ **Bulk Operations** — Start/Stop/Reboot/Delete multiple VMs at once
- ✅ **Webhook Notifications** — Slack & Discord integration with HMAC signing
- ✅ **Custom Dashboards** — Drag & drop widget grid, per-user layouts
- ✅ **Internationalization** — English + Thai (199 keys each)

### v1.0.0 — Production MVP
- ✅ Multi-cluster VM dashboard with aggregate stats
- ✅ VM lifecycle: list, detail, start, stop, shutdown, reboot, delete
- ✅ Live migration between nodes
- ✅ LXC and storage read endpoints
- ✅ Network interface listing (bridges, bonds, VLANs)
- ✅ Secure auth: JWT + refresh tokens + TOTP 2FA + WebAuthn + OIDC SSO
- ✅ RBAC with per-cluster permissions (admin/operator/viewer)
- ✅ Full audit logging with pagination, filtering, sorting
- ✅ Rate limiting, CORS, API key auth
- ✅ Prometheus metrics + health endpoints + OpenTelemetry tracing
- ✅ TLS 1.3 with rustls, security headers (HSTS, CSP, X-Frame-Options)
- ✅ Docker multi-stage build + Helm chart + Debian package
- ✅ 170+ tests, Criterion benchmarks, CI gates

### v1.2.0 — Power User Features
- ✅ **Multi-Region Replication** — CRUD API, scheduling, status monitoring
- ✅ **Plugin System** — `MoxuiPlugin` trait with lifecycle hooks, 2 built-in plugins
- ✅ **Terraform Provider** — Go SDK with `moxui_vm` resource (CRUD + acceptance tests)
- ✅ **Migration Wizard** — 6-step setup wizard in the UI

### v2.0+ (Planned)
- 🔜 VM/LXC creation and configuration editing
- 🔜 Storage content upload and management
- 🔜 VM snapshots, templates, backup configuration
- 🔜 Full VNC WebSocket proxy
- 🔜 LDAP/AD authentication
- 🔜 User management UI
- 🔜 Ceph dashboard
- 🔜 SDN management

---

## 🗺️ Roadmap

| Phase | When | Deliverable |
|---|---|---|
| **v1.0.0** | ✅ Shipped | Production MVP — auth, read + VM control, audit, Docker/K8s |
| **v1.1.0** | ✅ Shipped | Polish & Community — live migration, HA groups, bulk ops, webhooks, custom dashboards, i18n |
| **v1.2.0** | ✅ Shipped | Power User — replication, plugin system, Terraform provider, migration wizard |
| **v2.0** | Q3 2026 | Cluster management, Ceph, SDN, firewalls, LDAP |
| **v3.0** | Q4 2027 | Multi-region, multi-tenancy, cloud, AI-assisted ops |

---

## 🤝 Contributing

We welcome contributions! See the [Development Guide](docs/development.md) for details.

1. Read the [Development Guide](docs/development.md)
2. Pick a task from the issue tracker
3. Run `make check-all` before submitting a PR
4. Open a PR with tests + docs

---

## 📜 License

MIT — see [LICENSE](./LICENSE)

Copyright (c) 2026 kungjom26

---

## 🔗 Links

- **Repository:** https://github.com/kungjom26/moxui
- **Container:** ghcr.io/kungjom26/moxui
- **Issues:** https://github.com/kungjom26/moxui/issues
- **Proxmox API docs:** https://pve.proxmox.com/pve-docs/api-viewer/

---

> Built with 🦀 Rust and 🦐 by กุ้งจ่อม (Hermes Agent)
