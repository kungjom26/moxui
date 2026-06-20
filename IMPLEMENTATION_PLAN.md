# 🛠️ MoxUI — Implementation Plan (Coding → Implementation → UAT → Deploy)

> **Purpose:** แผนการทำงานฉบับสมบูรณ์ ตั้งแต่เริ่มเขียนโค้ดจน deploy production
>
> **Audience:** พี่เสือ (project owner) + กุ้งจ่อม (developer)
>
> **Structure:** 4 major stages × N sub-phases พร้อม deliverables, acceptance criteria, risk mitigation
>
> **Last updated:** 2026-06-20

---

## 📋 Table of Contents

1. [Stage Overview](#1-stage-overview)
2. [Decisions Locked](#2-decisions-locked)
3. [STAGE 1: CODING (Week 1-6)](#3-stage-1-coding)
4. [STAGE 2: IMPLEMENTATION (Week 7)](#4-stage-2-implementation)
5. [STAGE 3: UAT (Week 8)](#5-stage-3-uat)
6. [STAGE 4: DEPLOY (Week 9)](#6-stage-4-deploy)
7. [Cross-stage Concerns](#7-cross-stage-concerns)
8. [Risk Register](#8-risk-register)
9. [Success Metrics](#9-success-metrics)
10. [Communication Plan](#10-communication-plan)

---

## 1. Stage Overview

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│  STAGE 1: CODING (6 weeks)                                                   │
│  ────────────────────────                                                    │
│  Phase 0: Foundation         Week 1    ✅ Ready to start                     │
│  Phase 1: Core API + UI      Week 2                                       │
│  Phase 2: Auth + Audit       Week 3                                       │
│  Phase 3: Multi-cluster      Week 4                                       │
│  Phase 4: WebAuthn + Helm    Week 5                                       │
│  Phase 5: Docs + Release     Week 6                                       │
│                                                                              │
│                            ↓                                                │
│                                                                              │
│  STAGE 2: IMPLEMENTATION (1 week)            Internal dogfooding             │
│  ──────────────────────────────────          ──────────────────              │
│  Day 1: First-boot setup                     Deploy on homelab               │
│  Day 2: Configure cluster                    Connect pve11/12/13             │
│  Day 3: User setup                           Create users + 2FA             │
│  Day 4: Smoke test                           All features exercised         │
│  Day 5: Edge cases                           Bug fixes                       │
│  Day 6: Performance baseline                 Measure p99, memory             │
│  Day 7: Sign-off                             Internal MVP ready              │
│                                                                              │
│                            ↓                                                │
│                                                                              │
│  STAGE 3: UAT (1 week)                       User Acceptance Testing          │
│  ──────────────────────                      ──────────────────────          │
│  Day 1: Test plan + setup                    Test cases defined              │
│  Day 2-3: Functional testing                 All 54 MUST features tested     │
│  Day 4: Security testing                    Penetration + audit            │
│  Day 5: Load testing                         500 concurrent users           │
│  Day 6: Bug bash + fixes                     Fix all P0/P1                  │
│  Day 7: UAT sign-off                         Ready for production deploy    │
│                                                                              │
│                            ↓                                                │
│                                                                              │
│  STAGE 4: DEPLOY (1 week)                    Production rollout              │
│  ──────────────────────                      ──────────────────              │
│  Day 1: Pre-deploy checklist                 All green                      │
│  Day 2: Deploy staging                       Internal staging                │
│  Day 3: Deploy homelab (real usage)          pve11/12/13 in production       │
│  Day 4: Monitor + tune                       Watch metrics                  │
│  Day 5: Deploy production cluster            Production cluster               │
│  Day 6: DNS cutover                          Go live                        │
│  Day 7: Post-deploy review                   Lessons learned                │
│                                                                              │
│                                                                              │
│  Total: 9 weeks (Coding 6 + Implementation 1 + UAT 1 + Deploy 1)            │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Total duration:** 9 weeks (1 week = 5 working days)

---

## 2. Decisions Locked

| # | Decision | Value | Date |
|---|---|---|---|
| 1 | License | MIT ✅ | 2026-06-20 |
| 2 | Repository | `github.com/kungjom26/moxui` ✅ | 2026-06-20 |
| 3 | Container registry | ghcr.io ✅ | 2026-06-20 |
| 4 | Deploy order | homelab ก่อน → scale production ✅ | 2026-06-20 |
| 5 | Auth model | ครบทุกตัว (local + 2FA TOTP + WebAuthn + OIDC + LDAP + RBAC) ✅ | 2026-06-20 |
| 6 | Domain name | configurable at deploy time ✅ | 2026-06-20 |
| 7 | Initial Proxmox credentials | root account of Proxmox (PAM realm) ✅ | 2026-06-20 |

**Open questions ที่เหลือ (ไม่ block Phase 0):**
- Time budget สำหรับ v1.0.0 (4 หรือ 6 weeks)
- Proxmox user creation (ใช้ root existing vs create service account)
- Backup retention default (7d / 30d / 90d)

---

## 3. STAGE 1: CODING (Week 1-6)

### Phase 0 — Foundation (Week 1)

#### Day 1 — Project setup

**Goals:** Cargo project + dependencies + module skeleton

| Task | Owner | Deliverable |
|---|---|---|
| Install Rust toolchain | กุ้งจ่อม | `rustup`, `cargo`, `clippy`, `rustfmt` |
| `cargo init --name moxui` | กุ้งจ่อม | `Cargo.toml` skeleton |
| Add dependencies (axum, tokio, reqwest, rusqlite, etc.) | กุ้งจ่อม | `Cargo.toml` with all deps |
| Create module skeleton | กุ้งจ่อม | `src/{auth,proxmox,api,cache,db,observability,security}/` |
| Setup `.env.example` + `config.example.yaml` | กุ้งจ่อม | Template files |
| First `cargo check` passes | กุ้งจ่อม | Build succeeds |
| First commit | กุ้งจ่อม | Pushed to GitHub |

**Acceptance criteria:**
- [ ] `cargo build` succeeds
- [ ] `cargo run` prints "Hello, MoxUI v0.1.0-alpha" and exits
- [ ] GitHub Actions workflow triggers on push
- [ ] CI passes (build + test + clippy + fmt)

#### Day 2 — Proxmox auth + client

**Goals:** Connect to Proxmox, authenticate, list cluster info

| Task | Owner | Deliverable |
|---|---|---|
| Implement `ProxmoxClient` struct | กุ้งจ่อม | `src/proxmox/client.rs` |
| Ticket-based auth + auto-refresh | กุ้งจ่อม | `src/proxmox/auth.rs` |
| Connection pool (reqwest keep-alive) | กุ้งจ่อม | Built into client |
| `get()`, `post()` helpers | กุ้งจ่อม | Generic HTTP methods |
| Circuit breaker (per cluster) | กุ้งจ่อม | `src/proxmox/circuit_breaker.rs` |
| Retry with exponential backoff | กุ้งจ่อม | `src/proxmox/retry.rs` |
| Smoke test against pve11 | กุ้งจ่อม | `examples/list_clusters.rs` works |

**Acceptance criteria:**
- [ ] Can login to pve11 with root@pam + password
- [ ] Auto-refresh ticket 5 min before expiry
- [ ] `GET /api2/json/version` returns Proxmox version
- [ ] Connection pool reuses TCP connections (verify with strace)
- [ ] Circuit breaker opens after 5 failures, closes after 30s
- [ ] Retry 3x with exponential backoff on transient errors

#### Day 3 — Strongly-typed responses + VM list

**Goals:** Type-safe API responses, list all VMs

| Task | Owner | Deliverable |
|---|---|---|
| Define response structs | กุ้งจ่อม | `src/proxmox/types.rs` |
| `proxmox::vms::list_all()` | กุ้งจ่อม | List VMs across cluster |
| `proxmox::cluster::get_status()` | กุ้งจ่อม | Cluster + nodes info |
| Unit tests with sample JSON | กุ้งจ่อม | `src/proxmox/types_test.rs` |
| Integration test (mock Proxmox) | กุ้งจ่อม | `tests/proxmox_mock.rs` |
| Example CLI | กุ้งจ่อม | `cargo run --example list_vms` |

**Acceptance criteria:**
- [ ] `VmListEntry` deserializes from real Proxmox API response
- [ ] List all VMs from pve11 successfully
- [ ] Unit tests pass (10+ test cases)
- [ ] Mock Proxmox server works for integration tests

#### Day 4 — Axum server + first endpoints

**Goals:** Web server up, first API endpoints

| Task | Owner | Deliverable |
|---|---|---|
| `axum::serve` with tokio runtime | กุ้งจ่อม | `src/main.rs` |
| AppState struct | กุ้งจ่อม | `src/state.rs` |
| `/health` endpoint | กุ้งจ่อม | `src/api/health.rs` |
| `/api/v1/cluster/info` | กุ้งจ่อม | Returns Proxmox version + nodes |
| `/api/v1/vms` | กุ้งจ่อม | Lists VMs |
| AppError type + IntoResponse | กุ้งจ่อม | `src/error.rs` |
| RequestID middleware | กุ้งจ่อม | UUID per request |

**Acceptance criteria:**
- [ ] `curl http://localhost:8080/health` returns 200 OK
- [ ] `curl http://localhost:8080/api/v1/cluster/info` returns JSON
- [ ] `curl http://localhost:8080/api/v1/vms` returns VM list
- [ ] All responses include `X-Request-Id` header
- [ ] Error responses are JSON with consistent shape

#### Day 5 — Background poller + cache

**Goals:** Cache layer + background polling

| Task | Owner | Deliverable |
|---|---|---|
| `moka` cache wrapper | กุ้งจ่อม | `src/cache/mod.rs` |
| Background poller (5s interval) | กุ้งจ่อม | `src/cache/poller.rs` |
| Cache invalidation | กุ้งจ่อม | `src/cache/invalidation.rs` |
| Cache hit/miss metrics | กุ้งจ่อม | Counter |
| Refactor endpoints to read cache | กุ้งจ่อม | VM list from cache |
| Load test (1000 req/s) | กุ้งจ่อม | Verify cache works |

**Acceptance criteria:**
- [ ] Cache hits return in < 5ms
- [ ] Cache miss triggers Proxmox API call
- [ ] Background poller refreshes cache every 5s
- [ ] Cache invalidation works on writes (when implemented)
- [ ] 1000 concurrent requests to `/api/v1/vms` complete in < 1s

#### Day 6-7 — Refactor + tests + docs

**Goals:** Clean codebase, good test coverage

| Task | Owner | Deliverable |
|---|---|---|
| Module-level docs (`///`) | กุ้งจ่อม | All public APIs documented |
| Integration tests | กุ้งจ่อม | `tests/` directory |
| `cargo clippy --all-targets -- -D warnings` | กุ้งจ่อม | Zero warnings |
| `cargo fmt --check` | กุ้งจ่อม | All formatted |
| `cargo audit` | กุ้งจ่อม | No known vulnerabilities |
| Update CHANGELOG.md | กุ้งจ่อม | v0.0.1-alpha entry |
| Tag v0.0.1-alpha | กุ้งจ่อม | Git tag |

**Acceptance criteria:**
- [ ] Unit test coverage > 60%
- [ ] All clippy warnings fixed
- [ ] No `cargo audit` advisories
- [ ] Documentation builds (`cargo doc`)

### Phase 1 — Core API + UI (Week 2)

#### Day 8 — VM write operations

**Goals:** VM lifecycle (start/stop/reboot/delete)

| Task | Owner | Deliverable |
|---|---|---|
| `proxmox::vms::start()` | กุ้งจ่อม | POST to Proxmox |
| `proxmox::vms::stop()` | กุ้งจ่อม | Graceful + force |
| `proxmox::vms::reboot()` | กุ้งจ่อม | |
| `proxmox::vms::delete()` | กุ้งจ่อม | With options |
| API endpoints | กุ้งจ่อม | `/api/v1/vms/:id/start`, etc. |
| Request validation | กุ้งจ่อม | VmActionRequest struct |
| Task tracking (long-running) | กุ้งจ่อม | Return task_id |

**Acceptance criteria:**
- [ ] Start VM via API → VM transitions to running
- [ ] Stop VM via API → VM transitions to stopped
- [ ] Delete VM via API → VM removed
- [ ] Invalid actions return 4xx with error message

#### Day 9 — LXC + storage + network (read)

**Goals:** Read-only endpoints for LXC, storage, network

| Task | Owner | Deliverable |
|---|---|---|
| `proxmox::lxc::*` | กุ้งจ่อม | LXC list, start, stop |
| `proxmox::storage::*` | กุ้งจ่อม | Storage pools, ISO |
| `proxmox::network::*` | กุ้งจ่อม | Bridges, VLANs |
| API endpoints | กุ้งจ่อม | `/api/v1/lxc`, `/api/v1/storage`, `/api/v1/network` |

**Acceptance criteria:**
- [ ] All read endpoints work
- [ ] LXC operations functional

#### Day 10 — Frontend skeleton

**Goals:** SPA shell with routing

| Task | Owner | Deliverable |
|---|---|---|
| Vendor Alpine.js + Tailwind + Mousetrap | กุ้งจ่อม | `ui/vendor/` |
| Build minimal HTML | กุ้งจ่อม | `ui/index.html` |
| Hash routing | กุ้งจ่อม | `ui/app.js` |
| Theme toggle | กุ้งจ่อม | Dark/light |
| Embed via `rust-embed` | กุ้งจ่อม | `src/ui/mod.rs` |
| `/` and `/static/*` endpoints | กุ้งจ่อม | `src/api/ui.rs` |

**Acceptance criteria:**
- [ ] Open `http://localhost:8080` → blank UI
- [ ] Navigate `#/vms`, `#/lxc`, etc.
- [ ] Theme toggle works

#### Day 11 — VM list page

**Goals:** Working VM list with live updates

| Task | Owner | Deliverable |
|---|---|---|
| VM list table component | กุ้งจ่อม | Alpine.js |
| Fetch + render | กุ้งจ่อม | GET /api/v1/vms |
| Polling (2s) | กุ้งจ่อม | With ETag support |
| Search + filter | กุ้งจ่อม | Debounced |
| Sort by column | กุ้งจ่อม | Click header |
| Bulk select + action | กุ้งจ่อม | Multi-select |

**Acceptance criteria:**
- [ ] See all VMs in table
- [ ] Filter by name/IP/tag
- [ ] Polling updates running/stopped state
- [ ] Click row → navigate to detail

#### Day 12 — VM detail page

**Goals:** Full VM detail UI

| Task | Owner | Deliverable |
|---|---|---|
| Detail layout (tabs) | กุ้งจ่อม | Overview/Console/Stats/Config |
| Overview tab | กุ้งจ่อม | Resources + network |
| Stats tab (uPlot) | กุ้งจ่อม | CPU + RAM charts |
| Action buttons | กุ้งจ่อม | Start/Stop/Reboot/Delete |
| Confirm modal | กุ้งจ่อม | Destructive actions |
| Toasts | กุ้งจ่อม | Success/error feedback |

**Acceptance criteria:**
- [ ] All tabs work
- [ ] Charts update in real-time
- [ ] Confirm before delete
- [ ] Toasts appear and auto-dismiss

#### Day 13 — VNC console (noVNC WebSocket proxy)

**Goals:** Working VNC console in browser

| Task | Owner | Deliverable |
|---|---|---|
| WebSocket proxy in Rust | กุ้งจ่อม | `src/api/console.rs` |
| Embed noVNC | กุ้งจ่อม | `ui/vendor/novnc/` |
| WebSocket endpoint | กุ้งจ่อม | `/api/v1/console/:node/:vmid` |
| Frontend integration | กุ้งจ่อม | Connect → display |
| Resize handling | กุ้งจ่อม | Auto-resize canvas |
| Disconnect handling | กุ้งจ่อม | Reconnect button |

**Acceptance criteria:**
- [ ] Open console for running VM
- [ ] See desktop in browser
- [ ] Keyboard input works
- [ ] Resize works
- [ ] Disconnect handled gracefully

#### Day 14 — Polish + tests

**Goals:** Phase 1 complete

| Task | Owner | Deliverable |
|---|---|---|
| Integration tests for all endpoints | กุ้งจ่อม | 20+ tests |
| `cargo bench` for critical paths | กุ้งจ่อม | Benchmark suite |
| Update CHANGELOG.md | กุ้งจ่อม | v0.0.2 entry |
| Tag v0.0.2 | กุ้งจ่อม | |

**Acceptance criteria:**
- [ ] All endpoints tested
- [ ] Performance baseline established
- [ ] No regressions

### Phase 2 — Auth + Audit + Docker (Week 3) 🎉 MVP

#### Day 15-16 — Local auth + JWT

**Goals:** Secure authentication

| Task | Owner | Deliverable |
|---|---|---|
| `auth::password.rs` (bcrypt) | กุ้งจ่อม | Hash/verify |
| `auth::jwt.rs` (RS256) | กุ้งจ่อม | Encode/decode |
| `auth::refresh.rs` | กุ้งจ่อม | Rotation |
| User table migrations | กุ้งจ่อม | `migrations/V001__initial_schema.sql` |
| `/api/v1/auth/login` | กุ้งจ่อม | Login endpoint |
| `/api/v1/auth/refresh` | กุ้งจ่อม | Refresh endpoint |
| `/api/v1/auth/logout` | กุ้งจ่อม | Logout endpoint |
| `Auth` middleware | กุ้งจ่อม | Extract JWT → User |

**Acceptance criteria:**
- [ ] Login with username + password → JWT + refresh
- [ ] Use JWT to access protected endpoint
- [ ] Refresh token rotates (old one invalid)
- [ ] Logout revokes refresh token

#### Day 17 — RBAC + audit log

**Goals:** Authorization + audit trail

| Task | Owner | Deliverable |
|---|---|---|
| `db::audit.rs` | กุ้งจ่อม | AuditEntryBuilder |
| `audit_log` table + triggers | กุ้งจ่อม | Migration V004 |
| RBAC middleware | กุ้งจ่อม | Check role |
| Audit all mutating actions | กุ้งจ่อม | Wrap handlers |
| `/api/v1/audit` (admin) | กุ้งจ่อม | View audit log |
| Audit log UI page | กุ้งจ่อม | Sortable + filterable |

**Acceptance criteria:**
- [ ] Operator cannot delete VM (403)
- [ ] Every mutation logged
- [ ] Audit log queryable by user/action/cluster/date

#### Day 18 — Rate limiting + security headers

**Goals:** Production-grade security

| Task | Owner | Deliverable |
|---|---|---|
| Rate limiting (tower-governor) | กุ้งจ่อม | 5 login/min, 100 API/min |
| Security headers middleware | กุ้งจ่อม | CSP, HSTS, etc. |
| CORS (strict) | กุ้งจ่อม | Specific origins only |
| Account lockout | กุ้งจ่อม | After 5 failed |
| Verify all headers | กุ้งจ่อม | `curl -I` test |

**Acceptance criteria:**
- [ ] 6th login attempt in 1 min → 429
- [ ] All responses include CSP, HSTS, X-Frame-Options
- [ ] 5 failed logins → lockout 15 min

#### Day 19 — Docker + docker-compose

**Goals:** Containerized deploy

| Task | Owner | Deliverable |
|---|---|---|
| Multi-stage Dockerfile | กุ้งจ่อม | `Dockerfile` |
| docker-compose.yml + Caddy | กุ้งจ่อม | `docker-compose.yml` |
| Caddyfile (auto-TLS) | กุ้งจ่อม | `Caddyfile` |
| .env.example | กุ้งจ่อม | Template |
| Build + test | กุ้งจ่อม | `docker compose up` works |
| Domain name configurable | กุ้งจ่อม | `MOXUI_DOMAIN` env var |

**Acceptance criteria:**
- [ ] `docker compose up` starts MoxUI + Caddy
- [ ] Access via HTTPS (configured domain)
- [ ] TLS cert auto-issued by Let's Encrypt

#### Day 20-21 — MVP release

**Goals:** v0.1.0-alpha released

| Task | Owner | Deliverable |
|---|---|---|
| `cargo audit` + fix | กุ้งจ่อม | No advisories |
| `cargo clippy -- -D warnings` | กุ้งจ่อม | Clean |
| Test coverage > 70% | กุ้งจ่อม | |
| CHANGELOG → v0.1.0-alpha | กุ้งจ่อม | |
| Build + push image | กุ้งจ่อม | `ghcr.io/kungjom26/moxui:v0.1.0-alpha` |
| Tag v0.1.0-alpha | กุ้งจ่อม | |
| 🎉 **MVP RELEASE** | Both | Homelab deployment |

### Phase 3 — Multi-cluster + OIDC (Week 4)

#### Day 22-23 — Multi-cluster

| Task | Owner | Deliverable |
|---|---|---|
| Refactor config for multi-cluster | กุ้งจ่อม | `src/config.rs` |
| One `ProxmoxClient` per cluster | กุ้งจ่อม | |
| Per-cluster poller | กุ้งจ่อม | |
| Aggregate dashboard endpoint | กุ้งจ่อม | `/api/v1/dashboard` |
| Cross-cluster search | กุ้งจ่อม | |
| Per-cluster permission check | กุ้งจ่อม | |

#### Day 24 — OIDC SSO (Google)

| Task | Owner | Deliverable |
|---|---|---|
| `auth::oauth.rs` | กุ้งจ่อม | OAuth2 + PKCE |
| `/api/v1/auth/oidc/login` | กุ้งจ่อม | |
| `/api/v1/auth/oidc/callback` | กุ้งจ่อม | |
| Auto-create user | กุ้งจ่อม | On first login |
| Config: OIDC providers | กุ้งจ่อม | |
| UI: "Login with Google" button | กุ้งจ่อม | |

#### Day 25-26 — WebAuthn

| Task | Owner | Deliverable |
|---|---|---|
| `auth::webauthn.rs` | กุ้งจ่อม | Passkey support |
| `/api/v1/auth/webauthn/register` | กุ้งจ่อม | |
| `/api/v1/auth/webauthn/authenticate` | กุ้งจ่อม | |
| Credential storage | กุ้งจ่อม | |
| UI: Register/manage passkeys | กุ้งจ่อม | |

#### Day 27-28 — Observability

| Task | Owner | Deliverable |
|---|---|---|
| Prometheus metrics | กุ้งจ่อม | `/metrics` endpoint |
| OpenTelemetry tracing | กุ้งจ่อม | OTLP export |
| Structured JSON logs | กุ้งจ่อม | |
| Grafana dashboard JSON | กุ้งจ่อม | |

### Phase 4 — Production hardening (Week 5)

#### Day 29 — LDAP

| Task | Owner | Deliverable |
|---|---|---|
| `auth::ldap.rs` | กุ้งจ่อม | LDAP bind + search |
| `ldap_configs` table | กุ้งจ่อม | |
| AD group → role mapping | กุ้งจ่อม | |
| TLS connection (LDAPS/StartTLS) | กุ้งจ่อม | |

#### Day 30 — LXC + Snapshots + Backup

| Task | Owner | Deliverable |
|---|---|---|
| LXC console (xterm.js) | กุ้งจ่อม | |
| VM snapshot CRUD | กุ้งจ่อม | |
| VM backup trigger + list | กุ้งจ่อม | |

#### Day 31 — Storage + ISO

| Task | Owner | Deliverable |
|---|---|---|
| Storage pool UI | กุ้งจ่อม | |
| ISO library (list/upload/delete) | กุ้งจ่อม | |
| Network bridge UI | กุ้งจ่อม | |

#### Day 32-33 — Helm chart + Backup

| Task | Owner | Deliverable |
|---|---|---|
| Helm chart | กุ้งจ่อม | `deploy/k8s/` |
| Backup scripts | กุ้งจ่อม | `scripts/backup.sh`, `restore.sh` |
| K8s CronJob for backup | กุ้งจ่อม | |

### Phase 5 — Polish + v1.0.0 release (Week 6)

#### Day 34 — Security audit

| Task | Owner | Deliverable |
|---|---|---|
| `cargo audit` | กุ้งจ่อม | Clean |
| `cargo deny` | กุ้งจ่อม | License + bans |
| Manual security review | กุ้งจ่อม | All endpoints |
| SBOM generation | กุ้งจ่อม | CycloneDX |
| Sign release binary | กุ้งจ่อม | minisign |

#### Day 35 — Final docs + release

| Task | Owner | Deliverable |
|---|---|---|
| `docs/installation.md` | กุ้งจ่อม | |
| `docs/configuration.md` | กุ้งจ่อม | |
| `docs/deployment.md` | กุ้งจ่อม | |
| `docs/authentication.md` | กุ้งจ่อม | |
| `README.md` polish | กุ้งจ่อม | |
| 🎉 **v1.0.0 RELEASE** | Both | Public release |

---

## 4. STAGE 2: IMPLEMENTATION (Week 7)

**Purpose:** Internal dogfooding — deploy on homelab, exercise all features, fix bugs, establish baseline

### Day 36 — First-boot setup

| Task | Owner | Deliverable |
|---|---|---|
| Pull `ghcr.io/kungjom26/moxui:v0.1.0-alpha` | พี่เสือ | Local image |
| Configure `docker-compose.yml` | พี่เสือ | Domain, admin password |
| Start container | พี่เสือ | MoxUI up |
| First login as admin | พี่เสือ | Force 2FA setup |
| Setup TOTP | พี่เสือ | Google Authenticator |
| Backup codes saved | พี่เสือ | 8 codes written down |

**Acceptance criteria:**
- [ ] MoxUI accessible via configured domain
- [ ] TLS working (Let's Encrypt cert)
- [ ] Admin login + 2FA setup successful
- [ ] Backup codes accessible

### Day 37 — Configure Proxmox clusters

| Task | Owner | Deliverable |
|---|---|---|
| Login to MoxUI | พี่เสือ | Authenticated |
| Settings → Clusters → Add | พี่เสือ | |
| Connect pve11 (root@pam) | พี่เสือ | Cluster added |
| Connect pve12 | พี่เสือ | |
| Connect pve13 | พี่เสือ | |
| Verify all 3 show as reachable | พี่เสือ | Green status |

**Acceptance criteria:**
- [ ] All 3 homelab clusters connected
- [ ] All show "reachable" status
- [ ] VM counts visible on dashboard

### Day 38 — User setup + RBAC

| Task | Owner | Deliverable |
|---|---|---|
| Create user "operator1" | พี่เสือ | role=operator |
| Create user "viewer1" | พี่เสือ | role=viewer |
| Setup 2FA for both | พี่เสือ | |
| Login as operator1 | พี่เสือ | Should work |
| Try to delete VM as operator1 | พี่เสือ | Should get 403 |
| Login as viewer1 | พี่เสือ | Read-only |
| Try to start VM as viewer1 | พี่เสือ | Should get 403 |

**Acceptance criteria:**
- [ ] All 3 users can login
- [ ] RBAC enforced correctly
- [ ] Audit log captures all attempts

### Day 39 — Smoke test all features

| Feature | Action | Expected |
|---|---|---|
| **VM List** | View all VMs | See all VMs from 3 clusters |
| **VM Detail** | Click any VM | See details, tabs work |
| **VM Start** | Start a stopped VM | VM transitions to running |
| **VM Stop** | Stop a running VM | VM transitions to stopped |
| **VM Reboot** | Reboot a VM | VM restarts |
| **VM Delete** | Delete a test VM | VM removed |
| **VNC Console** | Open console | See desktop |
| **VM Stats** | View CPU/RAM | Charts update |
| **VM Snapshot** | Create snapshot | Snapshot listed |
| **VM Backup** | Trigger backup | Backup job runs |
| **VM Search** | Search by name | Results appear |
| **VM Filter** | Filter by tag | Results filtered |
| **Audit Log** | View audit log | All actions recorded |
| **Theme Toggle** | Switch theme | UI updates |
| **Logout** | Logout | Redirected to login |

**Acceptance criteria:**
- [ ] All features work as expected
- [ ] No crashes
- [ ] No data inconsistencies

### Day 40 — Edge cases + bug fixes

| Scenario | Test |
|---|---|
| Login with wrong password 5x | Account locks |
| Login with expired JWT | Re-auth required |
| Try to start already-running VM | Idempotent (200 OK) |
| Try to delete VM with snapshots | Confirm required |
| Open console for stopped VM | Error shown |
| Network blip during API call | Retry + circuit breaker |
| DB corruption test | Graceful error |
| Container restart | State preserved |
| Multiple browsers logged in | All work independently |
| Long VM name (63 chars) | Accepts, displays truncated |

**Acceptance criteria:**
- [ ] All edge cases handled gracefully
- [ ] No crashes
- [ ] Error messages user-friendly

### Day 41 — Performance baseline

| Metric | Measurement | Target |
|---|---|---|
| Container memory at idle | `docker stats` | < 100 MB |
| Container CPU at idle | `docker stats` | < 1% |
| API p50 latency (cached) | `wrk -t4 -c100 -d30s` | < 50ms |
| API p99 latency (cached) | same | < 200ms |
| API p99 latency (uncached) | same | < 1s |
| VM list load (100 VMs) | manual | < 500ms |
| Console latency | manual | < 100ms |
| Login flow | manual | < 1s |
| 100 concurrent users | `wrk` | No errors |

**Acceptance criteria:**
- [ ] All metrics within target
- [ ] Documented in `docs/performance-baseline.md`

### Day 42 — Implementation sign-off

| Task | Owner | Deliverable |
|---|---|---|
| Review all findings | Both | Bug list |
| File bugs | กุ้งจ่อม | GitHub issues |
| Sign-off on MVP | พี่เสือ | Ready for UAT |

**Deliverable:** MVP ready for UAT

---

## 5. STAGE 3: UAT (Week 8)

**Purpose:** User Acceptance Testing — validate all 54 MUST features, security, performance

### Day 43 — Test plan + setup

| Task | Owner | Deliverable |
|---|---|---|
| Write test cases (54 MUST features) | กุ้งจ่อม | `docs/UAT/test-cases.md` |
| Setup test environment | พี่เสือ | UAT instance |
| Setup test users | พี่เสือ | 3 users (admin/operator/viewer) |
| Setup test VMs | พี่เสือ | 10 VMs across clusters |
| Configure test cluster | พี่เสือ | Connect homelab |

**Acceptance criteria:**
- [ ] All 54 MUST features have test cases
- [ ] Test environment ready

### Day 44-45 — Functional testing

**Run through all 54 MUST features systematically:**

#### Auth (14 features)
- [ ] A-001 Login (email + password)
- [ ] A-002 JWT RS256
- [ ] A-003 Refresh token rotation
- [ ] A-004 Logout
- [ ] A-005 Password change
- [ ] A-007 Bootstrap admin
- [ ] A-101 TOTP
- [ ] A-102 2FA required for admin
- [ ] A-104 WebAuthn / Passkey
- [ ] A-201 OIDC SSO (Google)
- [ ] A-204 LDAP / Active Directory
- [ ] A-301/302/303 Roles
- [ ] A-304 Per-cluster permission
- [ ] A-101x Backup codes

#### VM (10 features)
- [ ] V-001 List VMs cross-cluster
- [ ] V-002 State badge
- [ ] V-003 VM detail page
- [ ] V-101 Start
- [ ] V-102 Stop (graceful)
- [ ] V-103 Stop (force)
- [ ] V-104 Reboot
- [ ] V-107 Delete
- [ ] V-201 VNC console
- [ ] V-301/302 CPU/RAM charts

#### Dashboard + Audit + Deploy + UI + Observability + DevEx
- [ ] D-001/002 Cluster summary + VM table
- [ ] AU-001 Audit log capture
- [ ] DP-001/002 Docker + compose
- [ ] DP-201/202 Graceful shutdown + auto-restart
- [ ] UX-001/003/004/007/008 Responsive/theme/loading/confirm/toasts
- [ ] O-001 Health endpoint
- [ ] DX-001/002 CI + release

**Acceptance criteria:**
- [ ] All 54 MUST features pass
- [ ] No P0/P1 bugs

### Day 46 — Security testing

| Test | Owner | Expected |
|---|---|---|
| **Brute force** | กุ้งจ่อม | 6th attempt → 429 |
| **XSS** | กุ้งจ่อม | All inputs sanitized |
| **SQL injection** | กุ้งจ่อม | Parameterized queries |
| **CSRF** | กุ้งจ่อม | JWT in header (not cookie) |
| **Auth bypass** | กุ้งจ่อม | RBAC enforced |
| **JWT forgery** | กุ้งจ่อม | RS256 signature required |
| **Token replay** | กุ้งจ่อม | Refresh tokens single-use |
| **Audit log tampering** | กุ้งจ่อม | Triggers prevent UPDATE/DELETE |
| **Secret in logs** | กุ้งจ่อม | No secrets logged |
| **TLS downgrade** | กุ้งจ่อม | TLS 1.3 enforced |
| **Container escape** | กุ้งจ่อม | Non-root, read-only rootfs |
| **Dependency CVE** | กุ้งจ่อม | `cargo audit` clean |

**Acceptance criteria:**
- [ ] No critical/high security issues
- [ ] All mitigations work as designed

### Day 47 — Load testing

**Test scenario:** Simulate production load

```bash
# wrk script
wrk -t12 -c500 -d5m -H "Authorization: Bearer ***" \
    http://moxui.test/api/v1/vms
```

| Metric | Target | Actual |
|---|---|---|
| Concurrent users | 500 | TBD |
| Request rate | 1000 req/s | TBD |
| p50 latency | < 100ms | TBD |
| p99 latency | < 500ms | TBD |
| Error rate | < 0.1% | TBD |
| Memory at load | < 500 MB | TBD |
| CPU at load | < 80% | TBD |

**Acceptance criteria:**
- [ ] All metrics meet targets
- [ ] No crashes under load
- [ ] Memory leak check (run 1 hour sustained load)

### Day 48 — Bug bash + fixes

| Task | Owner | Deliverable |
|---|---|---|
| Triage all bugs found | พี่เสือ | Prioritized list |
| Fix all P0 (blockers) | กุ้งจ่อม | Hotfix release v0.9.9 |
| Fix all P1 (critical) | กุ้งจ่อม | |
| Document P2/P3 (nice-to-have) | กุ้งจ่อม | GitHub issues |

**Acceptance criteria:**
- [ ] Zero P0 bugs
- [ ] Zero P1 bugs
- [ ] All P2/P3 documented

### Day 49 — UAT sign-off

| Task | Owner | Deliverable |
|---|---|---|
| Final review | Both | |
| UAT report | กุ้งจ่อม | `docs/UAT/UAT-report.md` |
| Sign-off | พี่เสือ | Ready for deploy |

**Deliverable:** UAT passed, ready for production deploy

---

## 6. STAGE 4: DEPLOY (Week 9)

**Purpose:** Production rollout — homelab first (existing usage), then production cluster

### Day 50 — Pre-deploy checklist

| Check | Owner | Status |
|---|---|---|
| All tests passing | กุ้งจ่อม | ✅/❌ |
| `cargo audit` clean | กุ้งจ่อม | |
| Container image signed | กุ้งจ่อม | |
| Backup of current MoxUI (if upgrading) | พี่เสือ | |
| Rollback plan ready | Both | |
| On-call rotation | พี่เสือ | |
| DNS ready (homelab domain) | พี่เสือ | |
| TLS cert ready (Let's Encrypt) | พี่เสือ | |
| Smoke test in staging | Both | |
| Documentation complete | กุ้งจ่อม | |

**Acceptance criteria:**
- [ ] All checks green
- [ ] Rollback tested

### Day 51 — Deploy to staging

| Task | Owner | Deliverable |
|---|---|---|
| Deploy v1.0.0 to staging env | พี่เสือ | Container running |
| Smoke test all critical paths | Both | Pass/fail |
| Monitor for 2 hours | พี่เสือ | Logs + metrics |

**Acceptance criteria:**
- [ ] All smoke tests pass
- [ ] No errors in logs
- [ ] Metrics within normal range

### Day 52 — Deploy to homelab (real usage)

| Task | Owner | Deliverable |
|---|---|---|
| Deploy v1.0.0 to homelab cluster | พี่เสือ | pve11/12/13 connected |
| DNS cutover | พี่เสือ | moxui.homelab.example.com |
| Announce to team (if any) | พี่เสือ | "MoxUI is live" |
| Monitor for 4 hours | พี่เสือ | Logs + metrics |

**Acceptance criteria:**
- [ ] Homelab fully operational
- [ ] Users can access via browser
- [ ] All VMs visible across pve11/12/13

### Day 53 — Monitor + tune

| Task | Owner | Deliverable |
|---|---|---|
| Review logs (errors, warnings) | กุ้งจ่อม | Issue list |
| Review metrics (latency, memory, CPU) | พี่เสือ | Baseline |
| Apply config tuning | กุ้งจ่อม | Cache TTL, rate limits |
| Apply hotfixes if needed | กุ้งจ่อม | Patch release |

**Acceptance criteria:**
- [ ] No P0 issues from homelab
- [ ] Performance within targets

### Day 54 — Deploy to production cluster

| Task | Owner | Deliverable |
|---|---|---|
| Backup production MoxUI config | พี่เสือ | |
| Deploy v1.0.0 to production cluster | พี่เสือ | Container running |
| Configure production Proxmox cluster | พี่เสือ | Connected |
| Test critical workflows | Both | |
| Monitor for 2 hours | พี่เสือ | |

**Acceptance criteria:**
- [ ] Production MoxUI operational
- [ ] All production VMs visible
- [ ] No disruption to production workloads

### Day 55 — DNS cutover + announcement

| Task | Owner | Deliverable |
|---|---|---|
| Update DNS records | พี่เสือ | moxui.example.com → MoxUI |
| Wait for TTL | — | 1 hour |
| Verify cutover | Both | All traffic to new instance |
| Send announcement | พี่เสือ | Team + users |
| Monitor for 4 hours | พี่เสือ | |

**Acceptance criteria:**
- [ ] DNS cutover successful
- [ ] All traffic routed to new MoxUI
- [ ] No accessibility issues

### Day 56 — Post-deploy review

| Task | Owner | Deliverable |
|---|---|---|
| 7-day soak test results | พี่เสือ | Report |
| Final metrics | Both | Performance baseline |
| Issues encountered | Both | Lessons learned |
| Update docs | กุ้งจ่อม | If needed |
| Plan v1.1 features | Both | Backlog |
| 🎉 **v1.0.0 LIVE** | Both | Production deployment complete |

**Acceptance criteria:**
- [ ] 7 days uptime with no critical issues
- [ ] v1.1 roadmap defined
- [ ] All docs accurate

---

## 7. Cross-stage Concerns

### 7.1 Continuous activities (during all 4 stages)

| Activity | Frequency | Owner |
|---|---|---|
| Code review | Every PR | Both |
| `cargo clippy` | Pre-commit | กุ้งจ่อม |
| `cargo test` | Every PR | กุ้งจ่อม |
| `cargo audit` | Weekly | กุ้งจ่อม |
| GitHub Actions CI | Every push | Automatic |
| Backup | Daily | Automatic |
| Security review | Weekly | Both |
| Update CHANGELOG | Every release | กุ้งจ่อม |

### 7.2 Quality gates

| Gate | Criteria | When |
|---|---|---|
| **Code quality** | clippy clean, fmt clean, coverage > 70% | Every PR |
| **Security** | `cargo audit` clean, no secrets in code | Every PR |
| **Build** | Docker build succeeds, image size < 100 MB | Every push |
| **Test** | All tests pass, no regressions | Every PR |
| **Docs** | Public APIs documented, CHANGELOG updated | Every release |

### 7.3 Rollback strategy

| Stage | Rollback method | RTO |
|---|---|---|
| Phase 0-5 (coding) | Revert git commit, rebuild | 5 min |
| Stage 2 (implementation) | Stop container, restart old version | 1 min |
| Stage 3 (UAT) | Re-deploy previous version | 5 min |
| Stage 4 (deploy) | DNS revert + redeploy old version | 10 min |

**Always keep previous version available** for quick rollback

---

## 8. Risk Register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Rust learning curve too steep | Medium | High | Daily practice, start with POC |
| Proxmox API breaking changes | Low | High | Pin to v8.x, abstract client |
| OIDC provider changes | Low | Medium | Adapter pattern, fallback to local |
| LDAP schema differences | High | Medium | Configurable search base + filter |
| Container image too large | Low | Low | Multi-stage build, distroless |
| Load test fails targets | Medium | High | Profile early, optimize cache |
| Security vulnerability found in UAT | Medium | High | Fix immediately, hotfix release |
| DNS cutover issues | Low | Medium | Low TTL before cutover, gradual |
| User adoption slow | Medium | Low | Documentation, demo video |
| Backup restore fails in disaster | Low | High | Test restore quarterly |

---

## 9. Success Metrics

### 9.1 Coding stage success

- [ ] 100% of MUST features implemented
- [ ] Unit test coverage > 70%
- [ ] Integration test coverage > 60%
- [ ] `cargo clippy -- -D warnings` clean
- [ ] `cargo audit` clean
- [ ] Docker image < 100 MB
- [ ] Binary size < 20 MB
- [ ] Cold build < 10 min, warm build < 1 min

### 9.2 Implementation stage success

- [ ] Homelab deployment stable for 7 days
- [ ] No critical bugs
- [ ] Performance baseline established
- [ ] All 3 users can use MoxUI daily

### 9.3 UAT stage success

- [ ] All 54 MUST features pass UAT
- [ ] Load test: 500 concurrent users, p99 < 500ms
- [ ] Security audit: zero critical/high issues
- [ ] Zero P0/P1 bugs

### 9.4 Deploy stage success

- [ ] 7-day soak test: uptime > 99.9%
- [ ] Zero data loss
- [ ] Zero security incidents
- [ ] User satisfaction > 80% (if surveyed)

---

## 10. Communication Plan

### 10.1 Daily (during coding)

- Morning: กุ้งจ่อม reports yesterday's progress + today's plan
- Evening: กุ้งจ่อม reports what got done, blockers, next steps

### 10.2 Weekly

- พี่เสือ reviews week's progress
- Adjust plan if needed
- Plan next week's priorities

### 10.3 Milestones

- v0.1.0-alpha (end Phase 2): MVP demo + go/no-go for UAT
- v0.9.9 (after UAT): UAT demo + go/no-go for production deploy
- v1.0.0 (after Deploy): Production launch announcement

### 10.4 Channels

- **Telegram** (primary) — daily updates, quick questions
- **GitHub Issues** — bugs, feature requests
- **GitHub Discussions** — design discussions, RFCs
- **GitHub Releases** — release notes, changelog

---

## 📎 Appendix — Quick Reference

### File locations

```
~/projects/moxui/
├── src/                          # Rust source
├── ui/                           # Frontend assets
├── migrations/                   # SQL migrations
├── tests/                        # Integration tests
├── docs/                         # Documentation
├── deploy/                       # Deployment configs
├── scripts/                      # Backup/restore
├── Dockerfile
├── docker-compose.yml
├── Cargo.toml
└── README.md
```

### Command cheat sheet

```bash
# Development
cargo run                         # run server
cargo test                        # run tests
cargo clippy                      # lint
cargo fmt                         # format
cargo audit                       # security check

# Build
cargo build --release             # build binary
docker build -t moxui:latest .    # build image
docker compose up                 # run full stack

# Deploy
docker pull ghcr.io/kungjom26/moxui:latest
docker compose -f docker-compose.prod.yml up -d

# Backup
./scripts/backup.sh               # backup data

# Restore
./scripts/restore.sh /backups/moxui-20260620.tar.gz
```

### URLs (configurable)

| Stage | URL |
|---|---|
| Dev | `http://localhost:8080` |
| Homelab | `https://moxui.homelab.example.com` |
| Production | `https://moxui.example.com` |

### Domain name configuration

Set via env var at deploy time:

```bash
# In docker-compose.yml
environment:
  - MOXUI_DOMAIN=moxui.example.com
  - MOXUI_ADMIN_PASSWORD=***
```

Or via `.env` file:

```bash
MOXUI_DOMAIN=moxui.example.com
```

---

**Total timeline:** 9 weeks (Coding 6 + Implementation 1 + UAT 1 + Deploy 1)
**Total effort:** ~270 hours (30 hrs/week × 9 weeks)
**Risk level:** Medium (Rust learning + Proxmox API integration + security)
**Confidence:** High (clear plan, achievable scope, good tooling)

---

**Last updated:** 2026-06-20
**Status:** Plan complete — awaiting Phase 0 Day 1 start
**Next action:** Say "go" to start coding 🦀