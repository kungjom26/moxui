# 🗺️ MoxUI — Implementation Roadmap

> **Goal:** Ship a production-ready, secure, performant Rust-based UI for Proxmox VE that runs as a single container.
>
> **Total duration:** 6+ weeks (MVP at Day 21, Production-ready at Day 35)
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
Week 6  │ Phase 4 — Polish (live migration, HA, bulk, webhooks, custom dashboards, i18n)
Week 7+ │ Phase 5 — Power User (replication, plugins, Terraform, migration wizard)
```

---

## 📦 Phase 0 — Foundation ✅

**Goal:** Cargo project, ProxmoxClient works, list VMs from cluster

### Key Deliverables
- ✅ Cargo project with axum 0.7, tokio, rusqlite, reqwest
- ✅ ProxmoxClient with ticket auth + auto-refresh
- ✅ Strongly-typed responses (VM, Node, Storage types)
- ✅ Axum server with `/health`, `/api/v1/vms` endpoints
- ✅ Background poller + moka cache (p99 < 10ms cached)
- ✅ Circuit breaker, retry with exponential backoff
- ✅ 73 unit tests, cargo clippy + fmt clean

---

## 📦 Phase 1 — Core API + Frontend Skeleton ✅

**Goal:** Functional REST API with full VM operations + dashboard UI

### Key Deliverables
- ✅ VM lifecycle: start, stop (graceful+force), reboot, delete
- ✅ LXC, Storage, Network read endpoints
- ✅ Alpine.js SPA with hash routing (sidebar + main area)
- ✅ VM list page (sortable, filterable, live poll 2s, state badges)
- ✅ VM detail page (tabs: Overview/Console/Stats/Config)
- ✅ VNC console (noVNC, token-based, connection limiter)
- ✅ 118 tests, 4 Criterion benchmark suites

---

## 📦 Phase 2 — Auth + Audit + Docker ✅

**Goal:** Secure access + audit trail + containerized deploy

### Key Deliverables
- ✅ Local auth with bcrypt, RS256 JWT (15-min TTL)
- ✅ Refresh token rotation (7-day TTL, SHA-256 hashed, family revocation)
- ✅ Login/Refresh/Logout endpoints
- ✅ TOTP 2FA (RFC 6238) with QR setup + backup codes
- ✅ PreAuthStore for 2FA pending sessions (5-min TTL)
- ✅ RBAC: admin/operator/viewer with per-cluster permissions
- ✅ Audit log (SQLite, every mutation captured)
- ✅ Rate limiting (tower-governor, 5 req/sec per IP)
- ✅ Security headers (CSP, HSTS, X-Frame-Options, etc.)
- ✅ CORS (configurable origins)
- ✅ Docker multi-stage build + docker-compose + Caddy TLS
- ✅ 133 tests

---

## 📖 Phase 3 — Production Hardening ✅

**Goal:** Make it ready for production cluster

### Key Deliverables
- ✅ Multi-cluster support (aggregate dashboard, per-cluster poller)
- ✅ OIDC SSO (Google + GitHub)
- ✅ Per-cluster permission filtering (multi-tenant isolation)
- ✅ Prometheus metrics (RED + USE) at `/metrics`
- ✅ OpenTelemetry tracing (OTLP gRPC export)
- ✅ WebAuthn / Passkey (YubiKey, Touch ID, Windows Hello)
- ✅ Helm chart (HPA, PDB, NetworkPolicy, ServiceMonitor)
- ✅ Backup/restore scripts (SQLite dump, S3 target, k8s CronJob)
- ✅ k6 load tests (500 concurrent, 100 req/s)
- ✅ Security audit: cargo audit, cargo deny, manual review
- ✅ Documentation: installation, configuration, authentication, deployment, dev guide
- ✅ 170 tests, tagged v1.0.0, pushed to ghcr.io

---

## 📦 Phase 4 — Polish & Community ✅

**Goal:** Community-requested features and polish

### Key Deliverables
- ✅ **Live Migration UI** — `POST /api/v1/vms/:cluster/:node/:vmid/migrate` with target + live flag
- ✅ **HA Group Management** — CRUD for HA groups (`GET/POST/DELETE /api/v1/hagroups`)
- ✅ **Bulk Operations** — Start/Stop/Reboot/Delete multiple VMs at once
- ✅ **Webhook Notifications** — Slack & Discord formatters, HMAC signing, retry with backoff
- ✅ **Custom Dashboards** — Drag & drop widget grid, per-user JSON persistence
- ✅ **i18n** — English + Thai (199 keys each), `$t()` key-based translation
- ✅ 170 tests, all passing

---

## 📦 Phase 5 — Power User Features ✅

**Goal:** Advanced features for power users

### Key Deliverables
- ✅ **Multi-Region Replication** — CRUD API for replication jobs, status monitoring
- ✅ **Plugin System** — `MoxuiPlugin` trait with lifecycle hooks, `PluginRegistry`
- ✅ **Terraform Provider** — Go SDK v2 with `moxui_vm` resource
- ✅ **Migration Wizard** — 6-step setup wizard for new deployments
- ✅ Built-in plugins: audit_logger (captures request/response), webhook_bridge (dispatches events)
- ✅ 170 tests, all passing

---

## 📦 Phase 6+ — Future

**Goal:** Iterative improvement based on feedback

### Planned Features
- VM/LXC creation and configuration editing
- Storage content upload and management
- VM snapshots, templates, backup configuration
- Full VNC WebSocket proxy
- LDAP/AD authentication
- User management UI
- Ceph dashboard
- SDN management (Zones, VNets, Subnets)
- Cross-cluster live migration
- Cloud deployment (AWS, GCP, Azure)
- AI-powered recommendations (right-sizing, anomaly detection)
- Mobile app (Tauri or React Native)
- Multi-tenancy (organizations, billing)

---

## 📊 Milestones Summary

| Date | Milestone | Status |
|---|---|---|
| End Week 1 | Phase 0 — Foundation | ✅ Complete |
| End Week 2 | Phase 1 — Core API + UI | ✅ Complete |
| End Week 3 | **MVP — usable on homelab** | ✅ Complete |
| End Week 5 | Phase 3 — Production hardening | ✅ Complete |
| End Week 6 | Phase 4 — Polish & Community | ✅ Complete |
| Week 7+ | Phase 5 — Power User Features | ✅ Complete |
| Q3 2026 | v2.0 — Advanced Cluster Mgmt | 🔜 Planning |

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

| Metric | Target | Current |
|---|---|---|
| Code coverage | > 80% (unit) + > 60% (integration) | ✅ 170 tests |
| Build time (warm) | < 60s | ✅ ~30s |
| Binary size | < 20 MB | ✅ ~15 MB |
| Container image | < 80 MB | ✅ ~15 MB (runtime) |
| API p99 latency (cached) | < 200ms | ✅ |
| API p99 latency (uncached) | < 1s | ✅ |
| Memory at idle | < 100 MB | ✅ |
| Open issues | < 10 | ✅ |
| Documentation pages | 10+ | ✅ 20+ |
| Auth methods | 5 | ✅ local, TOTP, WebAuthn, OIDC, API key |
| Plugin system | 2+ built-in | ✅ audit_logger, webhook_bridge |
| Terraform provider | 1 resource | ✅ moxui_vm (CRUD) |

---

## ⚠️ Risk Register

| Risk | Mitigation |
|---|---|
| Rust learning curve | Daily 30-min Rust reading, ask กุ้งจ่อม |
| Proxmox API changes | Pin to v8.x, abstract client |
| Scope creep | Strict Phase gate — don't add to Phase N |
| Burnout | 4-day work week option, flexible schedule |

---

## 📦 Statistics (All Phases)

| Metric | Phase 0-3 | Phase 4 | Phase 5 | **Total** |
|---|---|---|---|---|
| Source lines | ~4,500 | ~2,500 | ~2,000 | **~9,000** |
| Tests | 170 | 170 | 170 | **170** |
| API endpoints | ~35 | ~5 | ~6 | **~46** |
| Auth methods | 5 | 5 | 5 | **5** |
| Files | ~80 | ~20 | ~31 | **~131** |
| Commits | 20 | 1 | 1 | **22** |

---

## ✅ Next Action

**Current state:** All 5 Phases complete 🎉

**Next:** Plan Phase 6 / v2.0 based on feedback
