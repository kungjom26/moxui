# 🦀 MoxUI — Project Proposal

> **Tagline:** Modern, secure Rust-based web UI for Proxmox VE — deployed as a single container, designed for multi-cluster operations.
>
> **Author:** กุ้งจ่อม (Hermes Agent) for พี่เสือ
> **Date:** 2026-06-20
> **Status:** Proposal — awaiting approval
> **License:** MIT ✅ (decided 2026-06-20)
> **Repository:** `github.com/kungjom26/moxui` ✅ (decided 2026-06-20)

---

## 1. 🎯 Executive Summary

**MoxUI** (Modern Proxmox UI) คือ web application ที่เขียนด้วย **Rust** ทำหน้าที่เป็น **alternative web interface** สำหรับ Proxmox VE โดย deploy เป็น **Docker container** แยกจาก Proxmox node ไม่ต้องแตะต้อง infrastructure เดิม

**ปัญหาที่แก้:**

| Pain Point | Proxmox Native UI | MoxUI |
|---|---|---|
| Frontend size | ExtJS 5-8 MB | Alpine.js + Tailwind < 500 KB |
| Initial load time | 2-5s (cold) | < 500ms |
| Mobile experience | Limited | Fully responsive PWA-ready |
| Multi-cluster management | ต้อง login ทีละ cluster | Single dashboard รวมทุก cluster |
| Customization | ยาก (Sencha Cmd, Perl) | Source เปิด แก้ไขได้ |
| Audit log | Per-cluster | Aggregate ข้าม cluster |
| Authentication | PAM + 2FA (Proxmox native) | + OIDC/SSO + 2FA + RBAC |
| Modern UX | Desktop-classic | Modern dashboard (Grafana-like) |

**Why Rust:**
- Single binary deploy (~15 MB) → ไม่ต้องมี Python venv / Node modules
- Memory safety + performance (ไม่มี GC pauses)
- True async (tokio) — handle thousands of concurrent connections
- Strong type system → fewer runtime bugs
- ของจริงในการศึกษา Rust เป็น skill เพิ่ม

---

## 2. 🏗️ Architecture

### 2.1 High-level overview

```
┌─────────────────────────────────────────────────────────────────┐
│  Browser (พี่เสือ / admin)                                      │
│    ↑ HTTPS (TLS 1.3) + JWT (RS256, 15min access + 7d refresh) │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  Reverse Proxy (nginx/Caddy/Traefik)                     │  │
│  │  - TLS termination + HSTS + CSP                          │  │
│  │  - Rate limiting (login 5/min, API 100/min)              │  │
│  │  - Security headers                                      │  │
│  └──────────────────────────────────────────────────────────┘  │
│                          ↓                                       │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  MoxUI Container (single Rust binary, ~15 MB)           │  │
│  │  ──────────────────────────────────────────────────────  │  │
│  │  axum (HTTP) + tokio (async) + reqwest (Proxmox client) │  │
│  │  ┌────────────────────────────────────────────────────┐ │  │
│  │  │ Middleware chain: RequestID → Trace → Auth → RBAC  │ │  │
│  │  │ → RateLimit → Timeout → Handler                    │ │  │
│  │  └────────────────────────────────────────────────────┘ │  │
│  │                          ↓                               │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │  │
│  │  │ Cache (moka) │  │ Background   │  │ SQLite (WAL) │  │  │
│  │  │ TTL+LRU      │  │ Poller (5s)  │  │ users + audit│  │  │
│  │  └──────────────┘  └──────────────┘  └──────────────┘  │  │
│  │                          ↓                               │  │
│  │  ┌────────────────────────────────────────────────────┐ │  │
│  │  │ ProxmoxClient (per-cluster)                        │ │  │
│  │  │ - Ticket auth + auto-refresh                       │ │  │
│  │  │ - Connection pool (keep-alive)                     │ │  │
│  │  │ - Circuit breaker + retry w/ backoff               │ │  │
│  │  └────────────────────────────────────────────────────┘ │  │
│  └──────────────────────────────────────────────────────────┘  │
│                          ↓                                       │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  Proxmox VE Clusters (1+ clusters)                      │  │
│  │  - homelab (pve11 + pve12 + pve13)                      │  │
│  │  - production (PVE corp)                                 │  │
│  │  - staging (PVE test)                                    │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                  │
│  Sidecar observability (optional):                              │
│  Prometheus ← /metrics | Loki ← JSON logs | Tempo ← OTel traces │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 Tech Stack

| Layer | Technology | Why |
|---|---|---|
| **Language** | Rust 1.78+ (edition 2021) | Memory safety, performance, single binary |
| **Web framework** | axum 0.7 | tokio team, ergonomic, middleware ecosystem |
| **Async runtime** | tokio 1.x | De facto standard for async Rust |
| **HTTP client** | reqwest 0.12 | Most mature, async, connection pooling |
| **TLS** | rustls 0.23 | Pure Rust, no OpenSSL dependency, fast |
| **WebSocket** | axum::ws + tokio-tungstenite | For VNC console proxy + live updates |
| **Database** | rusqlite (bundled) | Zero-dep, embedded, WAL mode, fast |
| **JWT** | jsonwebtoken (RS256) | Asymmetric — public key verify |
| **Password hash** | bcrypt (cost=12) | Industry standard |
| **2FA** | totp-rs | TOTP (Google Authenticator, Authy) |
| **OAuth/OIDC** | oauth2 crate | SSO integration (Phase 2) |
| **Config** | figment | TOML/YAML/ENV layered config |
| **Logging** | tracing + tracing-subscriber | Structured JSON logs |
| **Metrics** | prometheus crate | Standard observability |
| **Tracing** | opentelemetry + tracing-opentelemetry | Distributed tracing |
| **Embedded UI** | rust-embed | Bundle HTML/CSS/JS into binary |
| **CLI** | clap | Argument parsing |
| **Error handling** | thiserror + anyhow | Ergonomic error chain |
| **Cache** | moka | Async LRU+TTL cache |
| **Container** | Docker (multi-stage) | Standard deployment |
| **CI/CD** | GitHub Actions | Build + test + sign + publish image |

### 2.3 Frontend stack

| Library | Version | Size | Purpose |
|---|---|---|---|
| Alpine.js | 3.x | 15 KB | Reactivity (no build) |
| Tailwind CSS | 3.x JIT | 50 KB | Utility-first CSS (build-time compile) |
| uPlot | 1.x | 45 KB | Time-series charts (faster than Chart.js) |
| noVNC | 1.x | 150 KB | VNC console client |
| Mousetrap | 1.x | 5 KB | Keyboard shortcuts |
| **Total payload** | | **~265 KB** | Plus HTML/CSS ≈ 400 KB |

**No build step at runtime** — Tailwind compiled once at build time, all JS/CSS inlined.

---

## 3. 📦 Feature Scope

### 3.1 MVP (Phase 0-2) — 4 weeks

| Feature | Description |
|---|---|
| **Multi-cluster dashboard** | ดู VM/LXC/storage ทุก cluster ในหน้าเดียว |
| **VM/LXC list + filter** | Sortable table, tag-based filter, cross-cluster search |
| **VM detail view** | Overview, stats, console (noVNC), config, snapshot, backup |
| **Basic CRUD** | Start/stop/reboot/delete VM/LXC |
| **Console access** | WebSocket → VNC proxy (works for any VM/CT) |
| **Auth: local user + 2FA** | SQLite + bcrypt + TOTP |
| **Audit log** | Every action recorded with user/IP/timestamp/result |
| **Single-cluster config** | Start with one cluster, add more later |

### 3.2 Production-ready (Phase 3) — 2 weeks

| Feature | Description |
|---|---|
| **OIDC SSO** | Login with Google/GitHub/Okta |
| **RBAC + per-cluster permissions** | admin / operator / viewer, restrict clusters |
| **Multi-cluster config** | Dashboard aggregates 2+ clusters |
| **Prometheus metrics** | `/metrics` endpoint with cache stats, request latency, etc. |
| **OpenTelemetry tracing** | Distributed trace to Jaeger/Tempo |
| **Helm chart** | Deploy to Kubernetes |
| **Automated backup of MoxUI itself** | Config + SQLite dump to remote |
| **WebAuthn / Passkey** | Yubikey, Touch ID support |

### 3.3 Future (Phase 4+) — Out of scope for v1

| Feature | Notes |
|---|---|
| **Live migration UI** | Trigger + monitor progress |
| **HA management** | Configure groups, watch fencing |
| **Ceph dashboard** | Aggregate OSD/PG/MDS stats |
| **SDN management** | Zone/VNet/Subnet editing |
| **Bulk operations** | Apply tags / start 100 VMs at once |
| **Webhook → Slack/Discord** | Notify on state changes |
| **Custom dashboards** | User-defined widgets |

### 3.4 Explicitly NOT doing

- ❌ Replace Proxmox API — we **proxy** to it
- ❌ Manage KVM/QEMU directly — Proxmox does that
- ❌ Cluster management (corosync, PMXCFS) — out of scope
- ❌ Storage replication setup — Proxmox handles
- ❌ Certificate management for Proxmox itself — out of scope
- ❌ Replace Proxmox UI entirely — complement, not replace

---

## 4. 🎯 Non-Functional Requirements

### 4.1 Performance

| Metric | Target |
|---|---|
| API response time (p50) | < 50ms (cached) / < 300ms (uncached) |
| API response time (p99) | < 200ms (cached) / < 1s (uncached) |
| Frontend FCP | < 500ms |
| Frontend TTI | < 1s |
| Memory footprint | < 100 MB at idle, < 500 MB under load |
| Concurrent clients | 500+ active WebSocket |
| Proxmox API calls | < 10 req/s to any single cluster (throttled) |
| Binary size | < 20 MB (release, stripped) |
| Container image | < 80 MB (distroless) |

### 4.2 Stability

| Metric | Target |
|---|---|
| Uptime | 99.9% (excluding planned maintenance) |
| MTBF | > 720 hours (30 days) |
| MTTR | < 15 minutes |
| Crash recovery | Auto-restart via Docker/K8s, state preserved in SQLite |
| Data loss | Zero (WAL mode + transactions) |
| Graceful degradation | If 1 cluster down → others still work |
| Backward compatibility | Proxmox 7.x and 8.x supported |

### 4.3 Security

| Requirement | Implementation |
|---|---|
| TLS 1.3 only | rustls config |
| Strong ciphers | AEAD only (ChaCha20-Poly1305, AES-GCM) |
| HSTS preload | `max-age=63072000; includeSubDomains; preload` |
| CSP strict | `default-src 'self'; script-src 'self'; object-src 'none'` |
| JWT RS256 | Public/private key pair, 15min TTL |
| Refresh token rotation | 7-day refresh, single-use |
| bcrypt cost 12 | ~250ms hash |
| 2FA TOTP | RFC 6238 |
| Audit log immutable | Append-only DB, signed entries (Phase 3) |
| Rate limiting | 5 login/min, 100 API/min per user |
| Input validation | Strict types, length limits, char whitelist |
| No SQL injection | Parameterized queries only |
| No shell injection | `Command::new().arg()` not shell |
| Secret management | Env vars + optional Vault integration |
| Container security | Non-root, read-only rootfs, cap-drop ALL |

### 4.4 Observability

| Requirement | Implementation |
|---|---|
| Structured logs | tracing + JSON output (Loki-compatible) |
| Request ID | UUID per request, propagated to all logs |
| Metrics | Prometheus `/metrics` (RED + USE metrics) |
| Distributed tracing | OpenTelemetry → Jaeger/Tempo |
| Health checks | `/livez`, `/readyz`, `/health` (detailed) |
| Audit log UI | Web interface for compliance team |

---

## 5. 🏛️ Project Structure

```
moxui/
├── Cargo.toml                          # workspace root
├── Cargo.lock                          # committed
├── README.md                           # quickstart
├── LICENSE                             # Apache-2.0
├── PROPOSAL.md                         # ← this file
├── ROADMAP.md                          # ← detailed plan
├── ARCHITECTURE.md                     # technical deep-dive
├── SECURITY.md                         # security model + threat model
├── CHANGELOG.md                        # release notes
├── .env.example                        # env var template (no secrets)
├── .gitignore
├── Dockerfile                          # multi-stage
├── docker-compose.yml                  # local dev
├── docker-compose.prod.yml             # production with observability
├── Makefile                            # common tasks
│
├── src/
│   ├── main.rs                         # entry: CLI + tokio + axum
│   ├── lib.rs                          # library exports
│   ├── config.rs                       # config loader (figment)
│   ├── error.rs                        # AppError + IntoResponse
│   ├── state.rs                        # AppState (db, http, config)
│   ├── telemetry.rs                    # tracing + metrics setup
│   │
│   ├── auth/
│   │   ├── mod.rs
│   │   ├── jwt.rs                      # encode/decode RS256
│   │   ├── password.rs                 # bcrypt
│   │   ├── refresh.rs                  # refresh token rotation
│   │   ├── totp.rs                     # TOTP 2FA
│   │   ├── session.rs                  # server-side session
│   │   ├── oauth.rs                    # OIDC (Phase 3)
│   │   ├── webauthn.rs                 # Passkey (Phase 3)
│   │   └── middleware.rs               # axum middleware
│   │
│   ├── proxmox/
│   │   ├── mod.rs                      # ProxmoxClient struct
│   │   ├── auth.rs                     # ticket/CSRF lifecycle
│   │   ├── circuit_breaker.rs          # circuit breaker pattern
│   │   ├── retry.rs                    # exponential backoff
│   │   ├── cluster.rs                  # cluster + node info
│   │   ├── vms.rs                      # QEMU VMs
│   │   ├── lxc.rs                      # LXC containers
│   │   ├── storage.rs                  # storage pools + ISO
│   │   ├── network.rb                  # bridges + SDN
│   │   ├── snapshot.rs                 # VM/LXC snapshots
│   │   ├── backup.rs                   # vzdump
│   │   ├── tasks.rs                    # long-running task tracking
│   │   ├── console.rs                  # WebSocket → VNC
│   │   └── types.rs                    # strongly-typed responses
│   │
│   ├── api/
│   │   ├── mod.rs                      # router assembly
│   │   ├── middleware.rs               # RequestID, Trace, RateLimit, Timeout
│   │   ├── auth.rs                     # /api/auth/*
│   │   ├── dashboard.rs                # /api/dashboard (multi-cluster)
│   │   ├── clusters.rs                 # /api/clusters/*
│   │   ├── vms.rs                      # /api/vms/*
│   │   ├── lxc.rs                      # /api/lxc/*
│   │   ├── storage.rs                  # /api/storage/*
│   │   ├── network.rs                  # /api/network/*
│   │   ├── tasks.rs                    # /api/tasks/*
│   │   ├── console.rs                  # /api/console/*
│   │   ├── metrics.rs                  # /metrics (Prometheus)
│   │   ├── health.rs                   # /health, /livez, /readyz
│   │   └── ui.rs                       # serve embedded UI
│   │
│   ├── cache/
│   │   ├── mod.rs                      # moka-based cache
│   │   ├── poller.rs                   # background task: poll Proxmox
│   │   └── invalidation.rs             # cache invalidation strategy
│   │
│   ├── db/
│   │   ├── mod.rs                      # connection pool (r2d2_sqlite)
│   │   ├── migrations.rs               # schema setup
│   │   ├── users.rs                    # user CRUD
│   │   ├── audit.rs                    # audit log
│   │   ├── refresh_tokens.rs           # refresh token store
│   │   └── api_keys.rs                 # API keys (Phase 3)
│   │
│   ├── audit/
│   │   └── mod.rs                      # audit logging helpers
│   │
│   ├── security/
│   │   ├── mod.rs
│   │   ├── rate_limit.rs               # tower-governor
│   │   ├── csrf.rs                     # CSRF tokens
│   │   ├── headers.rs                  # security headers middleware
│   │   └── validation.rs               # input validators
│   │
│   ├── observability/
│   │   ├── mod.rs
│   │   ├── tracing.rs                  # OpenTelemetry init
│   │   └── metrics.rs                  # Prometheus metrics
│   │
│   └── ui/
│       ├── mod.rs                      # serve via rust-embed
│       └── assets.rs                   # inline replacement
│
├── ui/                                  # frontend assets (build target)
│   ├── index.html
│   ├── app.js
│   ├── app.css
│   └── vendor/
│       ├── alpine.min.js
│       ├── tailwind.css                # pre-compiled
│       ├── uPlot.iife.min.js
│       ├── mousetrap.min.js
│       └── novnc/
│
├── tests/                              # integration tests
│   ├── api_auth.rs
│   ├── api_vms.rs
│   ├── api_dashboard.rs
│   └── proxmox_mock.rs                 # mock Proxmox server for tests
│
├── benches/                            # criterion benchmarks
│   ├── vm_list.rs
│   └── cache.rs
│
├── deploy/
│   ├── docker-compose.yml
│   ├── docker-compose.prod.yml
│   ├── nginx.conf
│   ├── caddy.example
│   ├── prometheus.yml
│   ├── grafana-dashboard.json
│   ├── k8s/
│   │   ├── deployment.yaml
│   │   ├── service.yaml
│   │   ├── ingress.yaml
│   │   ├── configmap.yaml
│   │   ├── secret.yaml.example
│   │   └── kustomization.yaml
│   └── systemd/
│       └── moxui.service
│
├── scripts/
│   ├── gen-jwt-keys.sh                 # generate RS256 keypair
│   ├── init-db.sh                      # bootstrap SQLite
│   ├── backup.sh                       # backup config + DB
│   └── health-check.sh
│
└── docs/
    ├── installation.md
    ├── configuration.md
    ├── authentication.md
    ├── deployment.md
    ├── proxmox-api-coverage.md
    ├── development.md
    └── images/
```

---

## 6. 🚀 Deployment Options

### 6.1 Docker (recommended for single-host)

```bash
docker run -d \
  --name moxui \
  -p 8080:8080 \
  -v moxui-data:/home/moxui/data \
  -v ./config.yaml:/etc/moxui/config.yaml:ro \
  -e MOXUI_JWT_PRIVATE_KEY="$(cat jwt-private.pem)" \
  -e MOXUI_JWT_PUBLIC_KEY="$(cat jwt-public.pem)" \
  -e MOXUI_PROXMOX_HOMELAB_PASSWORD="***" \
  ghcr.io/kungjom26/moxui:latest
```

### 6.2 Docker Compose (with reverse proxy)

```yaml
services:
  caddy:
    image: caddy:2
    ports: ["443:443", "80:80"]
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile:ro
      - caddy-data:/data
  
  moxui:
    image: ghcr.io/kungjom26/moxui:latest
    expose: ["8080"]
    depends_on: [caddy]
    volumes:
      - moxui-data:/home/moxui/data
    env_file: .env
  
  prometheus:
    image: prom/prometheus
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml:ro
    ports: ["9090:9090"]

volumes:
  caddy-data:
  moxui-data:
```

### 6.3 Kubernetes (Helm chart)

```yaml
# values.yaml
replicaCount: 2
image:
  repository: ghcr.io/kungjom26/moxui
  tag: "1.0.0"
ingress:
  enabled: true
  className: nginx
  hosts:
    - host: moxui.example.com
      paths: ["/"]
  tls:
    - secretName: moxui-tls
      hosts: [moxui.example.com]
config:
  clusters:
    - name: homelab
      url: https://pve11.local:8006
secrets:
  jwtPrivateKey: moxui-jwt-private
  proxmoxPasswords: moxui-proxmox-creds
autoscaling:
  enabled: true
  minReplicas: 2
  maxReplicas: 10
  targetCPUUtilizationPercentage: 70
```

### 6.4 Bare metal (systemd)

```ini
# /etc/systemd/system/moxui.service
[Unit]
Description=MoxUI - Modern Proxmox UI
After=network.target

[Service]
Type=simple
User=moxui
Group=moxui
ExecStart=/usr/local/bin/moxui --config /etc/moxui/config.yaml
Restart=on-failure
RestartSec=5
EnvironmentFile=/etc/moxui/moxui.env

[Install]
WantedBy=multi-user.target
```

---

## 7. 🔐 Security Model (summary)

### 7.1 Threat model

| Threat | Mitigation |
|---|---|
| Brute-force login | Rate limit + account lockout + 2FA |
| Stolen JWT | Short TTL (15 min) + refresh token rotation + revoke list |
| Stolen refresh token | Single-use, rotation, DB revoke list |
| Stolen password | bcrypt + 2FA + breach detection |
| XSS | CSP + Alpine `x-text` (escape) + no `innerHTML` |
| CSRF | JWT in Authorization header (not cookie) — N/A by default |
| SQL injection | Parameterized queries (rusqlite prepared statements) |
| Shell injection | `Command::new().arg()` not shell, strict input validation |
| MITM | TLS 1.3 + HSTS + cert pinning (optional) |
| DDoS | Rate limit + connection limits + reverse proxy |
| Supply chain | `cargo audit`, `cargo deny`, pinned dependencies, signed releases |
| Container escape | Non-root, read-only rootfs, cap-drop ALL, distroless image |
| Secret leak | Env vars, no plaintext in config/git, optional Vault |
| Insider threat | Audit log immutable + RBAC + per-cluster permissions |
| Backup theft | Encrypted backups, separate credentials |

### 7.2 Security headers (every response)

```
Strict-Transport-Security: max-age=63072000; includeSubDomains; preload
Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; connect-src 'self' wss:; object-src 'none'; frame-ancestors 'none'
X-Frame-Options: DENY
X-Content-Type-Options: nosniff
Referrer-Policy: strict-origin-when-cross-origin
Permissions-Policy: camera=(), microphone=(), geolocation=(), payment=()
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
Cross-Origin-Resource-Policy: same-origin
```

---

## 8. 💰 Cost / Resource Estimate

### 8.1 Build cost

| Resource | Quantity | Note |
|---|---|---|
| Rust toolchain setup | 1 time | cargo install — 5 min |
| Cargo build (cold) | ~5-10 min | first build |
| Cargo build (warm) | ~30-60s | incremental |
| Docker image build | ~3-5 min | multi-stage |
| Total dev setup | ~30 min | including deps |

### 8.2 Runtime cost

| Resource | At idle | Under load (100 clients) |
|---|---|---|
| CPU | < 1% | < 5% |
| Memory | ~50 MB | ~200 MB |
| Disk | ~500 MB (image) | +50 MB (cache) |
| Network | < 1 KB/s | < 100 KB/s |

### 8.3 Maintenance cost

| Task | Frequency |
|---|---|
| Update dependencies | monthly (`cargo update`) |
| Run `cargo audit` | weekly (CI) |
| Review audit log | monthly |
| Rotate JWT keys | annually |
| Backup config + DB | daily |
| Update Proxmox API client | when Proxmox major version |

---

## 9. 🎓 Learning Outcomes

ถ้าทำ project นี้จบ จะได้ความรู้:

| Skill | Level |
|---|---|
| **Rust** (async, ownership, traits, lifetimes) | Intermediate → Advanced |
| **axum + tokio** web stack | Production-ready |
| **Proxmox API** (JSON-RPC, ticket auth, CSRF) | Expert |
| **Multi-cluster management** patterns | Intermediate |
| **WebSocket proxy** (VNC) | Intermediate |
| **Docker multi-stage builds** | Production-ready |
| **Kubernetes deployment** | Intermediate |
| **Prometheus + OpenTelemetry** | Intermediate |
| **Security engineering** (JWT, RBAC, CSP, TLS) | Production-ready |
| **Performance engineering** (cache, pool, profiling) | Intermediate |

---

## 10. ✅ Success Criteria

### 10.1 MVP success (Phase 2 end)

- [ ] Single-container deploy on homelab cluster
- [ ] View all VMs across pve11/pve12/pve13 in one dashboard
- [ ] Start/stop/reboot any VM from UI
- [ ] Console access via noVNC works
- [ ] Local auth + 2FA functional
- [ ] Audit log captures every action
- [ ] p99 latency < 1s for cached data
- [ ] Container runs < 100 MB memory
- [ ] No crashes during 7-day soak test
- [ ] All security headers present
- [ ] Unit test coverage > 80%
- [ ] Integration tests pass

### 10.2 Production success (Phase 3 end)

- [ ] OIDC SSO working with Google
- [ ] RBAC enforced (3 test users: admin/operator/viewer)
- [ ] 2 clusters connected (homelab + production)
- [ ] Prometheus metrics scraped, Grafana dashboard works
- [ ] Load test: 500 concurrent users, p99 < 1s
- [ ] Helm chart deploys to k3s successfully
- [ ] Security audit (cargo audit, cargo deny, manual review) passes
- [ ] Documentation complete (install, configure, deploy, troubleshoot)
- [ ] SBOM generated, signed release
- [ ] Disaster recovery runbook tested

---

## 11. 🔄 Risks & Open Questions

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Proxmox API changes between versions | Medium | Medium | Pin to major version, abstract client |
| libvirt ticket auth breaks | Low | High | Fallback to API token (PVE 7.3+) |
| Rust learning curve too steep | Medium | Medium | Start with POC, incremental commits |
| Container image too large | Low | Low | Multi-stage build, distroless, musl |
| WebSocket proxy for noVNC tricky | Medium | Medium | Reference noVNC docs, test early |
| Proxmox has no OIDC (relies on realm=pve) | High | Low | Use OIDC at MoxUI layer, then MoxUI auths to Proxmox with stored creds |
| Cargo dependency supply chain | Medium | High | `cargo audit`, `cargo deny`, pin versions |
✅ Decisions ที่ lock แล้ว (2026-06-20)

| Decision | Value |
|---|---|
| **License** | MIT ✅ |
| **Repository location** | public GitHub `kungjom26/moxui` ✅ |
| **Deploy order** | homelab ก่อน → scale production ✅ |
| **Auth model** | ใส่ครบทุกตัว (local + 2FA TOTP + WebAuthn + OIDC Google + LDAP/AD + RBAC) ✅ |
| **Container registry** | ghcr.io (GitHub Container Registry) ✅ |
| **Domain name** | configurable at deploy time (ไม่ hardcode — ใส่ผ่าน env var / config) ✅ |
| **Initial Proxmox credentials** | ใช้ root account ของ Proxmox (PAM realm) ✅ |

---

## 12. 📚 References

- **Proxmox API**: https://pve.proxmox.com/pve-docs/api-viewer/
- **Proxmox source** (study): https://github.com/proxmox/pve-manager
- **axum docs**: https://docs.rs/axum/latest/axum/
- **tokio docs**: https://tokio.rs/
- **Rust async book**: https://rust-lang.github.io/async-book/
- **Cargo book**: https://doc.rust-lang.org/cargo/
- **OWASP Top 10**: https://owasp.org/www-project-top-ten/
- **CIS Docker Benchmark**: https://www.cisecurity.org/benchmark/docker
- **Mozilla Web Security Guidelines**: https://infosec.mozilla.org/guidelines/web_security

---

## 13. 🤝 Decision Requested

**พี่เสือ ต้องตัดสินใจ:**

1. **License** — Apache-2.0 / MIT / AGPL?
2. **Repository location** — public GitHub `kungjom26/moxui`?
3. **MVP scope** — agree with Phase 0-2 features above?
4. **Time budget** — 4 weeks (MVP) or 6 weeks (+ production-ready)?
5. **Initial deploy** — homelab first, then production?
6. **Auth model** — local+2FA only for v1, or include OIDC SSO in MVP?

---

**Status:** Awaiting approval to proceed to ROADMAP.md (detailed week-by-week plan)