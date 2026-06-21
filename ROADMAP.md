# 🗺️ MoxUI — Implementation Roadmap

> **Goal:** Ship a production-ready, secure, performant Rust-based UI for Proxmox VE that runs as a single container.
>
> **Total duration:** 6 weeks (MVP at Week 4, Production-ready at Week 6)
>
> **Daily commitment:** ~3-4 hours focused work (with help from กุ้งจ่อม)
>
> **Strategy:** Ship early, ship often — every phase ends with a working binary you can run.

---

## 📅 Overview

```
Week 1  │ Phase 0 — Foundation + Cargo project + ProxmoxClient
Week 2  │ Phase 1 — Core API (VM list + CRUD + console) + Dashboard
Week 3  │ Phase 2 — Auth + Audit + Frontend integration + Docker
Week 4  │ 🎉 MVP — usable on homelab cluster
Week 5  │ Phase 3 — Production hardening (tests + metrics + OIDC)
Week 6  │ Phase 4 — Polish + Helm chart + docs + release
Week 6+ │ Phase 5+ — Feature additions, multi-region, etc.
```

---

## 📦 Phase 0 — Foundation (Week 1)

**Goal:** Cargo project, ProxmoxClient works, list VMs from pve11

### Day 1 — Setup + Rust warm-up

**Tasks:**
- [ ] Install Rust toolchain (`rustup`, `cargo`, `clippy`, `rustfmt`)
- [ ] Create Cargo project at `~/projects/moxui/`
- [ ] Add dependencies to `Cargo.toml` (see PROPOSAL.md)
- [ ] Setup `tracing` + JSON logging
- [ ] Setup `figment` config loader
- [ ] Smoke test: `cargo run` prints "Hello, MoxUI"

**Deliverable:** `cargo build` works, binary starts and exits cleanly

### Day 2 — Proxmox auth + client

**Tasks:**
- [ ] Implement `ProxmoxClient` struct with ticket auth
- [ ] Auto-refresh ticket (refresh 5 min before expiry)
- [ ] Connection pool (reqwest built-in keep-alive)
- [ ] Implement `get()` and `post()` helpers
- [ ] Add `circuit_breaker.rs` (use `failsafe` crate)
- [ ] Add `retry.rs` with exponential backoff
- [ ] Smoke test: login to pve11, call `/api2/json/version`, print

**Deliverable:** `cargo run` → "Connected to pve11, API version 8.2.4"

### Day 3 — Strongly-typed responses + VM list

**Tasks:**
- [ ] Define `VmListEntry`, `NodeInfo`, `ClusterInfo` structs in `proxmox/types.rs`
- [ ] Implement `proxmox::vms::list_all()` using `cluster/resources?type=vm`
- [ ] Implement `proxmox::cluster::get_status()` for node stats
- [ ] Write unit tests with sample JSON
- [ ] Smoke test: list all VMs on pve11, print JSON

**Deliverable:** Cargo CLI prints all VMs on homelab cluster

### Day 4 — Axum server + first endpoint

**Tasks:**
- [ ] Implement `axum::serve` with tokio runtime
- [ ] Add `/health` endpoint (returns 200 OK with version + uptime)
- [ ] Add `/api/v1/cluster/info` endpoint
- [ ] Add `/api/v1/vms` endpoint (lists VMs from Proxmox)
- [ ] Add error type (`AppError`) + `IntoResponse` impl
- [ ] Add `RequestID` middleware
- [ ] Smoke test: `curl http://localhost:8080/api/v1/vms` returns JSON

**Deliverable:** Working API server with health + cluster info + VM list

### Day 5 — Background poller + cache

**Tasks:**
- [ ] Add `moka` cache (TTL + LRU)
- [ ] Implement `cache::poller.rs` — background task poll Proxmox every 5s
- [ ] Refactor `/api/v1/vms` to read from cache
- [ ] Add `/api/v1/cluster/stats` (host stats)
- [ ] Add cache hit/miss metrics (Prometheus later, simple counters now)
- [ ] Smoke test: hammer `/api/v1/vms` 1000x, verify cache works

**Deliverable:** Fast API (p99 < 10ms for cached), no Proxmox overload

### Day 6-7 — Refactor + docs

**Tasks:**
- [ ] Refactor modules, ensure clean separation
- [ ] Add module-level docs (`///` comments)
- [ ] Write unit tests (target 70% coverage)
- [ ] Run `cargo clippy` + fix warnings
- [ ] Run `cargo fmt`
- [ ] Update CHANGELOG.md

**Deliverable:** Clean, documented codebase, ready for Phase 1

---

## 📦 Phase 1 — Core API + Frontend Skeleton (Week 2)

**Goal:** Functional REST API with full VM operations + dashboard UI

### Day 8 — VM operations (write)

**Tasks:**
- [ ] Implement `proxmox::vms::start()`, `stop()`, `reboot()`, `delete()`
- [ ] Add `/api/v1/vms/{vmid}/start` etc. endpoints
- [ ] Add request validation (vmid range, name length, etc.)
- [ ] Add audit log (basic — write to SQLite for now)
- [ ] Add task tracking (long-running operations like stop)
- [ ] Smoke test: start/stop VM via API, verify state changes

**Deliverable:** Full VM lifecycle via REST API

### Day 9 — LXC + Storage + Network (read)

**Tasks:**
- [ ] Implement `proxmox::lxc::list_all()`, `proxmox::lxc::start()`, etc.
- [ ] Implement `proxmox::storage::list_pools()`, `list_iso()`
- [ ] Implement `proxmox::network::list_bridges()`, `list_vlan()`
- [ ] Add `/api/v1/lxc`, `/api/v1/storage`, `/api/v1/network` endpoints
- [ ] Write integration tests with mock Proxmox server

**Deliverable:** Read-only endpoints for LXC, storage, network

### Day 10 — Frontend skeleton

**Tasks:**
- [ ] Set up `ui/` directory
- [ ] Vendor Alpine.js + Tailwind (compiled CSS) + Mousetrap
- [ ] Build minimal HTML with sidebar + blank main area
- [ ] Hash routing (`#/`, `#/vms`, `#/lxc`, `#/network`, `#/storage`, `#/settings`)
- [ ] Theme toggle (dark/light) with CSS variables
- [ ] Embed UI via `rust-embed` + `include_str!`
- [ ] Add `/` and `/static/*` endpoints
- [ ] Smoke test: open `http://localhost:8080`, see blank UI

**Deliverable:** Working SPA shell, navigates between routes

### Day 11 — VM list page (frontend)

**Tasks:**
- [ ] VM list table component (sortable, filterable)
- [ ] Fetch from `/api/v1/vms` + render rows
- [ ] Polling every 2s (with conditional GET / ETag)
- [ ] Search + filter + tag-based filter
- [ ] Row click → navigate to `#/vms/:vmid`
- [ ] Bulk select + bulk start/stop actions
- [ ] State badge colors (running/stopped/paused)
- [ ] Smoke test: see VM list, click row, see detail

**Deliverable:** Working VM list page with live data

### Day 12 — VM detail page

**Tasks:**
- [ ] VM detail layout (header + tabs)
- [ ] Tabs: Overview / Console / Stats / Snapshots / Config
- [ ] Overview: resources, network, events
- [ ] Stats: uPlot chart (CPU/Mem over time)
- [ ] Action buttons: Start/Stop/Reboot/Delete
- [ ] Confirmation modals for destructive actions
- [ ] Toasts for success/error notifications

**Deliverable:** Full VM detail page

### Day 13 — Console (noVNC WebSocket proxy)

**Tasks:**
- [ ] Implement `proxmox::console::proxy_websocket()` in Rust
- [ ] Add `/api/v1/console/:node/:vmid` WebSocket endpoint
- [ ] Frontend: embed noVNC, connect via WebSocket
- [ ] Clipboard sync (optional — start without it)
- [ ] Test: open console for a running VM, see desktop

**Deliverable:** Working VNC console in browser

### Day 14 — Polish + integration tests

**Tasks:**
- [ ] Write integration tests for key API endpoints
- [ ] Add `cargo bench` for critical paths
- [ ] Fix any bugs found
- [ ] Update documentation

**Deliverable:** Phase 1 complete, ready for Phase 2 (auth + audit)

---

## 📦 Phase 2 — Auth + Audit + Docker (Week 3)

**Goal:** Secure access + audit trail + containerized deploy

### Day 15 — Local auth + JWT

**Tasks:**
- [ ] Implement `auth::password.rs` (bcrypt hash/verify)
- [ ] Implement `auth::jwt.rs` (RS256 encode/decode, 15-min TTL)
- [ ] Implement `auth::refresh.rs` (rotation, 7-day TTL)
- [ ] Add `users` table + migrations
- [ ] Add `refresh_tokens` table (with revoke flag)
- [ ] Implement `/api/v1/auth/login` + `/refresh` + `/logout`
- [ ] Add `Auth` middleware (extract JWT → User)
- [ ] Smoke test: login → get token → use token → refresh

**Deliverable:** Working JWT auth flow

### Day 16 — 2FA (TOTP)

**Tasks:**
- [ ] Implement `auth::totp.rs` (RFC 6238)
- [ ] Add `users.totp_secret` column
- [ ] Add `/api/v1/auth/2fa/setup` (returns QR code)
- [ ] Add `/api/v1/auth/2fa/verify` (accepts TOTP code)
- [ ] Modify `/login` to require 2FA if enabled
- [ ] Add backup codes (8 single-use codes per user)
- [ ] Smoke test: setup 2FA, login with TOTP code

**Deliverable:** Working 2FA

### Day 17 — RBAC + audit log

**Tasks:**
- [ ] Add `roles` table: `admin`, `operator`, `viewer`
- [ ] Implement permission checks (middleware)
- [ ] Add `audit_log` table (user, action, target, timestamp, IP, result)
- [ ] Implement `audit::log()` helper, call from every mutating handler
- [ ] Add `/api/v1/audit` endpoint (admin only)
- [ ] Frontend: audit log page (sortable, filterable)
- [ ] Smoke test: try to delete VM as viewer → 403, audit logged

**Deliverable:** Working RBAC + audit trail

### Day 18 — Rate limiting + security headers

**Tasks:**
- [ ] Add `tower-governor` for rate limiting
- [ ] Configure: 5 login/min, 100 API/min per user/IP
- [ ] Add security headers middleware (CSP, HSTS, X-Frame-Options, etc.)
- [ ] Add CORS (strict, specific origins only)
- [ ] Verify all headers with `curl -I`
- [ ] Smoke test: 6 logins in 1 min → 6th gets 429

**Deliverable:** Production-grade security headers + rate limiting

### Day 19 — Docker + docker-compose

**Tasks:**
- [ ] Write multi-stage `Dockerfile` (rust:bookworm → debian-slim)
- [ ] Use `rust:1.78-bookworm` as builder, `debian:bookworm-slim` as runtime
- [ ] Use musl for static binary (optional, smaller image)
- [ ] Write `docker-compose.yml` with Caddy reverse proxy
- [ ] Write `Caddyfile` with auto-TLS (Let's Encrypt)
- [ ] Write `.env.example` (no secrets)
- [ ] Build + test: `docker compose up`, access via HTTPS
- [ ] Smoke test: access `https://localhost`, get working UI

**Deliverable:** One-command deploy

### Day 20-21 — Audit + ship MVP

**Tasks:**
- [ ] Run `cargo audit` + fix any advisories
- [ ] Run `cargo clippy` + fix warnings
- [ ] Run `cargo fmt`
- [ ] Update test coverage (target 80%+)
- [ ] Write integration tests (mock Proxmox)
- [ ] Update CHANGELOG.md → **v0.1.0-alpha**
- [ ] 🎉 **MVP RELEASE** — deploy to homelab

**Deliverable:** MVP live on homelab, usable for daily operations

---

## 🎉 MVP Checkpoint (End of Week 3)

**What works:**
- ✅ View all VMs on pve11 in one dashboard
- ✅ Start/stop/reboot/delete VMs via UI
- ✅ VNC console works
- ✅ Local auth + 2FA
- ✅ RBAC enforced
- ✅ Audit log captured
- ✅ Docker-compose deploy (single host)
- ✅ TLS via Caddy

**What's next (Phase 3-4):**
- Multi-cluster
- OIDC SSO
- Prometheus metrics
- Helm chart
- Load testing

---

## 📦 Phase 3 — Production Hardening (Week 4-5)

**Goal:** Make it ready for production cluster

### Week 4 — Multi-cluster + OIDC

### Day 22 — Multi-cluster support

**Tasks:**
- [ ] Refactor config to support multiple Proxmox clusters
- [ ] One `ProxmoxClient` per cluster
- [ ] Background poller per cluster
- [ ] Aggregate dashboard endpoint (`/api/v1/dashboard`)
- [ ] Per-cluster health status (reachable? auth OK?)
- [ ] Cross-cluster search
- [ ] Smoke test: connect to pve11 + production cluster, see both in dashboard

**Deliverable:** Multi-cluster working

### Day 23 — OIDC SSO (basic)

**Tasks:**
- [ ] Add `oauth2` crate
- [ ] Implement `/api/v1/auth/oidc/login` (redirect to provider)
- [ ] Implement `/api/v1/auth/oidc/callback`
- [ ] Map OIDC user → MoxUI user (auto-create on first login)
- [ ] Support Google + GitHub as initial providers
- [ ] Add config: `oidc.providers[]`
- [ ] Smoke test: login with Google → MoxUI

**Deliverable:** Working OIDC SSO

### Day 24 — Per-cluster permissions

**Tasks:**
- [ ] Add `cluster_permissions` table (user → allowed clusters)
- [ ] Modify dashboard to filter by user's allowed clusters
- [ ] Modify cluster endpoints to check permission
- [ ] UI: admin can assign cluster access per user
- [ ] Smoke test: user A sees only cluster X, not Y

**Deliverable:** Multi-tenant isolation

### Day 25 — Prometheus metrics

**Tasks:**
- [ ] Add `prometheus` crate
- [ ] Implement RED metrics (Rate, Errors, Duration) per endpoint
- [ ] Implement USE metrics (Utilization, Saturation, Errors) for Proxmox clients
- [ ] Add cache hit/miss counters
- [ ] Add audit log counter
- [ ] Add `/metrics` endpoint
- [ ] Write Grafana dashboard JSON
- [ ] Smoke test: `curl /metrics | grep proxui_`

**Deliverable:** Production observability

### Day 26 — OpenTelemetry tracing

**Tasks:**
- [ ] Add `opentelemetry` + `tracing-opentelemetry` crates
- [ ] Initialize OTLP exporter (Jaeger/Tempo compatible)
- [ ] Add spans for every handler
- [ ] Propagate trace context through to Proxmox API calls
- [ ] Smoke test: see traces in Jaeger UI

**Deliverable:** Distributed tracing

### Day 27-28 — Load testing

**Tasks:**
- [ ] Write `k6` load test script
- [ ] Test: 500 concurrent clients, 100 req/s for 10 min
- [ ] Verify p99 < 1s, no memory leaks, no crashes
- [ ] Profile with `cargo flamegraph` + `tokio-console`
- [ ] Fix bottlenecks found
- [ ] Document performance numbers

**Deliverable:** Load-tested, performance baseline documented

### Week 5 — WebAuthn + Helm + Disaster Recovery

### Day 29 — WebAuthn / Passkey

**Tasks:**
- [ ] Add `webauthn-rs` crate
- [ ] Implement `/api/v1/auth/webauthn/register`
- [ ] Implement `/api/v1/auth/webauthn/authenticate`
- [ ] Store credential ID per user
- [ ] Add UI: register passkey, login with passkey
- [ ] Smoke test: login with Touch ID / Yubikey

**Deliverable:** WebAuthn working

### Day 30 — Helm chart

**Tasks:**
- [ ] Create `deploy/k8s/` directory
- [ ] Write Helm chart (Deployment, Service, Ingress, ConfigMap, Secret)
- [ ] HPA (Horizontal Pod Autoscaler) 2-10 replicas
- [ ] PDB (Pod Disruption Budget)
- [ ] NetworkPolicy
- [ ] ServiceAccount + RBAC
- [ ] Test on k3s/kind
- [ ] Smoke test: `helm install moxui ./chart`, access via Ingress

**Deliverable:** Helm chart

### Day 31 — Backup + disaster recovery

**Tasks:**
- [ ] Implement `scripts/backup.sh` (SQLite dump + config archive)
- [ ] Implement `scripts/restore.sh`
- [ ] Document disaster recovery runbook
- [ ] Test: backup → wipe → restore → verify data intact
- [ ] Implement scheduled backups (cron / k8s CronJob)
- [ ] Off-cluster backup (S3 / NFS)

**Deliverable:** DR runbook + tested restore

### Day 32 — Security audit

**Tasks:**
- [ ] Run `cargo audit` + fix
- [ ] Run `cargo deny` + fix
- [ ] Manual security review of all endpoints
- [ ] Verify no secrets in logs
- [ ] Verify no SQL injection possible
- [ ] Verify rate limiting works
- [ ] Verify CSP blocks inline scripts
- [ ] Generate SBOM (CycloneDX)
- [ ] Sign release with minisign

**Deliverable:** Security-hardened release

### Day 33-34 — Documentation

**Tasks:**
- [ ] Write `docs/installation.md`
- [ ] Write `docs/configuration.md`
- [ ] Write `docs/authentication.md`
- [ ] Write `docs/deployment.md`
- [ ] Write `docs/development.md`
- [ ] Write `docs/proxmox-api-coverage.md` (what's supported)
- [ ] Update README.md with quickstart
- [ ] Write CHANGELOG.md → **v1.0.0**

**Deliverable:** Production-ready v1.0.0

### Day 35 — Final release

**Tasks:**
- [ ] Tag release `v1.0.0`
- [ ] Build + push container image to ghcr.io
- [ ] Publish Helm chart
- [ ] Announce on social media / blog
- [ ] 🎉 **v1.0.0 RELEASE**

**Deliverable:** MoxUI v1.0.0 production release

---

## 📦 Phase 4+ — Future (Week 6+)

**Goal:** Iterative improvement based on feedback

### Phase 4 (Week 6-8)

- [ ] Live migration UI
- [ ] HA group management
- [ ] Ceph dashboard
- [ ] Bulk operations
- [ ] Webhook notifications (Slack, Discord)
- [ ] Custom dashboards (user-defined widgets)
- [ ] Multi-language support (i18n)

### Phase 5 (Week 9-12)

- [ ] Multi-region replication
- [ ] Cloud Proxmox (subscription) support
- [ ] Migration from native Proxmox UI wizard
- [ ] Plugin system for custom integrations
- [ ] Mobile app (Tauri or React Native)
- [ ] Terraform provider

### Phase 6 (Long-term)

- [ ] Multi-tenancy (organizations, billing)
- [ ] Cloud deployment (AWS, GCP, Azure)
- [ ] AI-powered recommendations (right-sizing, anomaly detection)
- [ ] Self-hosted Proxmox Cloud (distributed cluster UI)

---

## 📊 Milestones Summary

| Date | Milestone | Status |
|---|---|---|
| End Week 1 | Cargo project + ProxmoxClient + cache | Pending |
| End Week 2 | API + dashboard UI + console | Pending |
| End Week 3 | **MVP — usable on homelab** | Pending |
| End Week 5 | Multi-cluster + OIDC + metrics + Helm | Pending |
| End Week 6 | **v1.0.0 production release** | Pending |
| Week 8+ | Feature additions | Pending |

---

## 🛠️ Daily Workflow

```
Morning (1 hour):
  - Pull latest changes
  - Review open issues
  - Run tests locally
  - Plan day's tasks with กุ้งจ่อม

Build (2 hours):
  - Implement 1-2 features from roadmap
  - Write tests for new code
  - Update docs

Review (30 min):
  - Run cargo clippy + fmt
  - Run cargo audit
  - Run test suite
  - Commit with conventional commits

Evening (30 min):
  - Deploy to dev cluster
  - Smoke test in browser
  - Update CHANGELOG.md
  - Report progress
```

---

## 📈 Success Metrics (tracked weekly)

| Metric | Target |
|---|---|
| Code coverage | > 80% (unit) + > 60% (integration) |
| Build time (warm) | < 60s |
| Binary size | < 20 MB |
| Container image | < 80 MB |
| API p99 latency (cached) | < 200ms |
| API p99 latency (uncached) | < 1s |
| Memory at idle | < 100 MB |
| Open issues | < 10 |
| Documentation pages | 10+ |

---

## ⚠️ Risk Register (tracked weekly)

| Risk | Mitigation |
|---|---|
| Rust learning curve | Daily 30-min Rust reading, ask กุ้งจ่อม |
| Proxmox API changes | Pin to v8.x, abstract client |
| Scope creep | Strict Phase gate — don't add to Phase N |
| Burnout | 4-day work week option, flexible schedule |

---

## 🤝 Help from กุ้งจ่อม

ผม (กุ้งจ่อม) ช่วยได้:

| Task | ช่วยได้ |
|---|---|
| ✅ Write code (Rust, HTML, JS) | เต็มที่ |
| ✅ Write tests | เต็มที่ |
| ✅ Research (Proxmox API, libraries) | เต็มที่ |
| ✅ Review code (PR review style) | เต็มที่ |
| ✅ Run cargo commands (build, test, clippy) | เต็มที่ |
| ✅ Deploy to homelab via API/SSH | ต้อง consent |
| ✅ Generate test data, mock servers | เต็มที่ |
| ⚠️ Make architectural decisions | แนะนำ แต่ moxui ตัดสินใจ |
| ❌ Manage production clusters | destructive — ต้อง explicit approval |

---

## ✅ Next Action (Today)

**ตอนนี้:** PROPOSAL.md + ROADMAP.md เสร็จแล้ว

**Next:** รอ moxui approve proposal → เริ่ม Phase 0 Day 1 (Cargo project setup)

**ถ้า approve ตอนนี้** กุ้งจ่อมจะ:
1. `cd ~/projects/moxui/`
2. `cargo init --name moxui`
3. เพิ่ม dependencies ทั้งหมดใน Cargo.toml
4. สร้าง module skeleton (auth/, proxmox/, api/, cache/, db/, observability/, security/)
5. ทำ `cargo check` ให้ผ่าน
6. Commit + push

**Total:** ~30 นาที

---

**Status:** Awaiting approval to proceed to Phase 0 🦐