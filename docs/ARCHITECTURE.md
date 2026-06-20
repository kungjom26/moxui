# 🏛️ MoxUI — Architecture

> **Purpose:** High-level architecture + sequence diagrams + technology map
>
> **Audience:** Engineers, architects, reviewers, new contributors
>
> **Last updated:** 2026-06-20
>
> **See also:** [`docs/diagrams/`](./diagrams/) for detailed diagrams (state machines, ER, request flows, security boundaries)

---

## 📋 Table of Contents

1. [Overview](#1-overview)
2. [Deployment Architecture](#2-deployment-architecture)
3. [Container Architecture](#3-container-architecture)
4. [Module Architecture](#4-module-architecture)
5. [Sequence Diagrams](#5-sequence-diagrams)
6. [Data Flow](#6-data-flow)
7. [Technology Map](#7-technology-map)
8. [Security Boundaries](#8-security-boundaries)
9. [Scaling & Performance](#9-scaling--performance)
10. [Failure Modes](#10-failure-modes)

---

## 1. Overview

MoxUI is a **single-binary Rust application** deployed as a **container**, providing a modern web UI for Proxmox VE. It proxies Proxmox API calls, aggregates multi-cluster data, and adds features Proxmox lacks (modern UX, multi-cluster dashboard, audit log).

### 1.1 One-liner

> **MoxUI = Modern UI layer for Proxmox** — no Proxmox code modified, no kernel changes, no agent on nodes.

### 1.2 Key properties

- 🦀 **Rust binary** (~15 MB) → container image ~80 MB
- 🌐 **Multi-cluster** — connects to 1+ Proxmox clusters
- 🔐 **Auth complete** — local + TOTP + WebAuthn + OIDC + LDAP
- 📊 **Aggregate dashboard** — VMs across clusters in one view
- 🛡️ **Security-first** — TLS 1.3, JWT RS256, audit log immutable
- ⚡ **Fast** — p99 < 200ms cached, < 1s uncached

---

## 2. Deployment Architecture

### 2.1 Single-host deployment (Docker)

```
┌─────────────────────────────────────────────────────────────────┐
│  Internet                                                       │
│      │                                                          │
│      │ HTTPS (TLS 1.3)                                         │
│      ▼                                                          │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  Caddy (reverse proxy + TLS termination)                   │  │
│  │  - Auto Let's Encrypt                                     │  │
│  │  - Rate limit (5 login/min, 100 API/min)                   │  │
│  │  - Security headers (CSP, HSTS, X-Frame-Options)           │  │
│  │  - HTTP/3                                                  │  │
│  │  Listen: 0.0.0.0:443                                       │  │
│  └─────────────────────────┬─────────────────────────────────┘  │
│                            │ HTTP (internal)                     │
│  ┌─────────────────────────▼─────────────────────────────────┐  │
│  │  Docker host                                               │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │  MoxUI Container                                     │  │  │
│  │  │  - rust:1.78-slim runtime                            │  │  │
│  │  │  - User: moxui (UID 1000)                            │  │  │
│  │  │  - Read-only rootfs                                   │  │  │
│  │  │  - Cap-drop ALL                                       │  │  │
│  │  │  - Listen: 0.0.0.0:8080                               │  │  │
│  │  │  ┌───────────────────────────────────────────────┐  │  │  │
│  │  │  │  Single Rust binary                            │  │  │  │
│  │  │  │  - axum HTTP server                            │  │  │  │
│  │  │  │  - tokio runtime                              │  │  │  │
│  │  │  │  - embedded UI (Alpine.js + Tailwind)        │  │  │  │
│  │  │  │  - SQLite (WAL mode)                          │  │  │  │
│  │  │  │  - moka cache                                 │  │  │  │
│  │  │  │  - background poller                          │  │  │  │
│  │  │  │  - reqwest Proxmox client (per cluster)        │  │  │  │
│  │  │  └───────────────────────────────────────────────┘  │  │  │
│  │  └─────────────────────────┬─────────────────────────────┘  │  │
│  │                            │                                  │  │
│  │  ┌─────────────────────────▼─────────────────────────────┐  │  │
│  │  │  Docker volume: moxui-data                            │  │  │
│  │  │  - /home/moxui/data/moxui.db (SQLite + WAL)          │  │  │
│  │  │  - /home/moxui/data/backups/                          │  │  │
│  │  │  - /home/moxui/data/logs/                             │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  └────────────────────────────────────────────────────────────┘  │
│                            │                                     │
│                            │ HTTPS (private network, VLAN)       │
│                            ▼                                     │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │  Proxmox VE Cluster                                        │  │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐                    │  │
│  │  │  pve11   │ │  pve12   │ │  pve13   │                    │  │
│  │  │ :8006    │ │ :8006    │ │ :8006    │                    │  │
│  │  └──────────┘ └──────────┘ └──────────┘                    │  │
│  │  - Proxmox REST API (JSON-RPC)                              │  │
│  │  - Ticket-based auth (root@pam)                             │  │
│  │  - Self-signed certs (CA pinned to MoxUI)                  │  │
│  └────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 Production deployment (Kubernetes)

```
┌─────────────────────────────────────────────────────────────────┐
│  Internet                                                       │
│      │                                                          │
│      │ HTTPS                                                    │
│      ▼                                                          │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  CloudFlare / CDN (optional, future)                       │  │
│  │  - DDoS protection                                          │  │
│  │  - WAF rules                                                │  │
│  └─────────────────────────┬─────────────────────────────────┘  │
│                            │                                     │
│                            ▼                                     │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  K8s Cluster                                                │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │  Ingress: Traefik / nginx-ingress                   │  │  │
│  │  │  - TLS via cert-manager                             │  │  │
│  │  │  - Rate limiting                                    │  │  │
│  │  └─────────────────────────┬───────────────────────────┘  │  │
│  │                            │                                │  │
│  │  ┌─────────────────────────▼───────────────────────────┐  │  │
│  │  │  Service: moxui (ClusterIP, port 8080)               │  │  │
│  │  └─────────────────────────┬───────────────────────────┘  │  │
│  │                            │                                │  │
│  │  ┌─────────────────────────▼───────────────────────────┐  │  │
│  │  │  Deployment: moxui (2+ replicas)                     │  │  │
│  │  │  ┌──────────────┐ ┌──────────────┐                  │  │  │
│  │  │  │ Pod 1        │ │ Pod 2        │                  │  │  │
│  │  │  │ moxui:1.0.0  │ │ moxui:1.0.0  │                  │  │  │
│  │  │  │ RWO volume   │ │ RWO volume   │ (separate!)     │  │  │
│  │  │  └──────────────┘ └──────────────┘                  │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  │                                                            │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │  CronJob: moxui-backup (daily 02:00)                │  │  │
│  │  │  - SQLite .backup                                  │  │  │
│  │  │  - Tar + age encrypt                               │  │  │
│  │  │  - Upload to S3 / NFS                              │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  │                                                            │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │  HPA: 2-10 replicas (CPU 70%)                       │  │  │
│  │  │  PDB: minAvailable: 1                                │  │  │
│  │  │  NetworkPolicy: allow only ingress + Proxmox       │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  └────────────────────────────────────────────────────────────┘  │
│                            │                                     │
│                            ▼                                     │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │  Proxmox VE Clusters (homelab + production)                │  │
│  └────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### 2.3 Multi-cluster topology

```
┌─────────────────────────────────────────────────────────────┐
│  MoxUI                                                      │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  ProxmoxClient registry                               │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │  HashMap<String, Arc<ProxmoxClient>>             │  │  │
│  │  │                                                   │  │  │
│  │  │  "homelab"  → Arc<ProxmoxClient> → pve11/pve12   │  │  │
│  │  │  "prod"     → Arc<ProxmoxClient> → corp-prod-1   │  │  │
│  │  │  "staging"  → Arc<ProxmoxClient> → corp-stg-1    │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────┘  │
│           │           │           │                          │
│           ▼           ▼           ▼                          │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐                │
│  │  homelab   │ │   prod     │ │  staging   │                │
│  │  pve11     │ │ corp-prod-1│ │ corp-stg-1 │                │
│  │  pve12     │ │ corp-prod-2│ │ corp-stg-2 │                │
│  │  pve13     │ │ corp-prod-3│ │            │                │
│  └────────────┘ └────────────┘ └────────────┘                │
└─────────────────────────────────────────────────────────────┘
```

---

## 3. Container Architecture

### 3.1 What's inside the container

```
┌─────────────────────────────────────────────────────────────┐
│  Container: ghcr.io/kungjom26/moxui:v1.0.0                  │
│  Base: debian:bookworm-slim (~75 MB)                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  /usr/local/bin/moxui          # Single Rust binary (15 MB) │
│  /etc/moxui/                   # Default config              │
│                                                             │
│  ┌──────────────────────────┐                                │
│  │  Rust binary structure   │                                │
│  │  ┌────────────────────┐  │                                │
│  │  │ main.rs           │  │  ← entry point                  │
│  │  │  - load config    │  │                                │
│  │  │  - init tracing   │  │                                │
│  │  │  - run migrations │  │                                │
│  │  │  - start tokio    │  │                                │
│  │  │  - start axum     │  │                                │
│  │  └────────────────────┘  │                                │
│  │                          │                                │
│  │  Embedded UI (rust-embed)│                                │
│  │  ┌────────────────────┐  │                                │
│  │  │ ui/index.html     │  │  Single HTML (~50 KB)           │
│  │  │ ui/app.js         │  │  Alpine.js components          │
│  │  │ ui/app.css        │  │  Custom styles                  │
│  │  │ ui/vendor/        │  │                                │
│  │  │  ├── alpine.js    │  │  15 KB                          │
│  │  │  ├── tailwind.css │  │  50 KB (pre-compiled)           │
│  │  │  ├── uPlot.js     │  │  45 KB                          │
│  │  │  ├── mousetrap.js │  │  5 KB                           │
│  │  │  └── novnc/       │  │  150 KB                         │
│  │  └────────────────────┘  │                                │
│  └──────────────────────────┘                                │
│                                                             │
│  /home/moxui/data/            # Persistent volume            │
│  ├── moxui.db                 # SQLite (main)                │
│  ├── moxui.db-wal             # WAL file                     │
│  ├── moxui.db-shm             # Shared memory                │
│  ├── backups/                 # Auto-backup output            │
│  └── logs/                    # Application logs (optional)   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 Process model (tokio runtime)

```
┌─────────────────────────────────────────────────────────────┐
│  tokio runtime (multi-thread, N workers = CPU count)         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────────┐                                        │
│  │  Main task      │  ← axum server (accept loop)            │
│  │  - bind :8080   │                                        │
│  │  - serve HTTP   │                                        │
│  └─────────────────┘                                        │
│                                                             │
│  ┌─────────────────┐  ┌─────────────────┐ ┌───────────────┐  │
│  │  Poller task    │  │  Poller task    │ │  Poller task  │  │
│  │  cluster 1      │  │  cluster 2      │ │  cluster N    │  │
│  │  (every 5s)     │  │  (every 5s)     │ │  (every 5s)   │  │
│  └─────────────────┘  └─────────────────┘ └───────────────┘  │
│                                                             │
│  ┌─────────────────┐  ┌─────────────────┐                   │
│  │  Retention job  │  │  Backup job     │                   │
│  │  (daily 03:00)  │  │  (daily 02:00)  │                   │
│  └─────────────────┘  └─────────────────┘                   │
│                                                             │
│  ┌─────────────────┐  ┌─────────────────┐                   │
│  │  VNC proxy WS   │  │  VNC proxy WS   │  (1 per active   │
│  │  connection #1  │  │  connection #N  │   console)       │
│  └─────────────────┘  └─────────────────┘                   │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  Spawn-blocking pool (for bcrypt, SQLite queries)   │    │
│  │  - separate thread pool                            │    │
│  │  - doesn't block async runtime                      │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 4. Module Architecture

### 4.1 Code structure

```
src/
├── main.rs              # entry: load config → init tracing → run migrations → spawn tasks → serve
├── lib.rs               # library exports (for integration tests)
│
├── config.rs            # figment config loader (env + yaml)
├── error.rs             # AppError enum + IntoResponse
├── state.rs             # AppState (Arc<Config>, Arc<Db>, clients, cache)
├── telemetry.rs         # tracing init + Prometheus metrics
│
├── auth/                # Authentication module
│   ├── mod.rs
│   ├── password.rs      # bcrypt hash/verify
│   ├── jwt.rs           # encode/decode JWT (RS256)
│   ├── refresh.rs       # refresh token rotation
│   ├── totp.rs          # TOTP 2FA
│   ├── webauthn.rs      # Passkey / WebAuthn
│   ├── oauth.rs         # OIDC SSO
│   ├── ldap.rs          # LDAP / Active Directory
│   └── middleware.rs    # extract JWT → User
│
├── proxmox/             # Proxmox API client
│   ├── mod.rs           # ProxmoxClient struct
│   ├── auth.rs          # ticket lifecycle
│   ├── circuit_breaker.rs
│   ├── retry.rs         # exponential backoff
│   ├── cluster.rs       # cluster + node info
│   ├── vms.rs           # QEMU VMs
│   ├── lxc.rs           # LXC containers
│   ├── storage.rs       # storage pools + ISO
│   ├── network.rs       # bridges + VLAN
│   ├── snapshot.rs      # snapshots
│   ├── backup.rs        # vzdump
│   ├── tasks.rs         # task tracking (UPID)
│   ├── console.rs       # VNC proxy
│   └── types.rs         # strongly-typed responses
│
├── api/                 # HTTP API handlers (axum)
│   ├── mod.rs           # router assembly
│   ├── middleware.rs    # RequestID, Trace, RateLimit, Timeout
│   ├── auth.rs          # /api/v1/auth/*
│   ├── dashboard.rs     # /api/v1/dashboard (multi-cluster)
│   ├── clusters.rs      # /api/v1/clusters/*
│   ├── vms.rs           # /api/v1/vms/*
│   ├── lxc.rs           # /api/v1/lxc/*
│   ├── storage.rs       # /api/v1/storage/*
│   ├── network.rs       # /api/v1/network/*
│   ├── tasks.rs         # /api/v1/tasks/*
│   ├── console.rs       # WebSocket
│   ├── users.rs         # /api/v1/users/* (admin)
│   ├── audit.rs         # /api/v1/audit (admin)
│   ├── settings.rs      # /api/v1/settings/*
│   ├── metrics.rs       # /metrics (Prometheus)
│   ├── health.rs        # /health, /livez, /readyz
│   └── ui.rs            # serve embedded UI
│
├── cache/               # In-memory cache
│   ├── mod.rs           # moka wrapper (TTL + LRU)
│   ├── poller.rs        # background task (every 5s per cluster)
│   └── invalidation.rs  # cache invalidation on writes
│
├── db/                  # Database (rusqlite)
│   ├── mod.rs           # connection pool + migrations runner
│   ├── migrations.rs    # refinery runner
│   ├── users.rs         # User CRUD
│   ├── audit.rs         # AuditLogEntry + builder
│   ├── refresh_tokens.rs
│   ├── api_keys.rs
│   ├── webauthn.rs
│   ├── totp.rs
│   ├── clusters.rs
│   ├── cluster_permissions.rs
│   ├── oauth_state.rs
│   ├── ldap_configs.rs
│   └── system_config.rs
│
├── security/            # Security middleware
│   ├── mod.rs
│   ├── rate_limit.rs    # tower-governor
│   ├── headers.rs       # CSP, HSTS, X-Frame-Options
│   ├── validation.rs    # input validators
│   └── encryption.rs    # ChaCha20-Poly1305 (cluster passwords)
│
├── observability/       # Metrics + tracing
│   ├── mod.rs
│   ├── metrics.rs       # Prometheus collectors
│   └── tracing.rs       # OpenTelemetry init
│
├── ui/                  # Embedded UI server
│   ├── mod.rs
│   └── assets.rs        # rust-embed + template replacement
│
└── jobs/                # Background jobs
    ├── mod.rs
    ├── retention.rs     # cleanup old tokens, audit logs
    └── backup.rs        # scheduled backup
```

### 4.2 Module dependencies

```
        main
          │
          ├─→ config
          ├─→ error
          ├─→ state ──→ all modules
          ├─→ telemetry
          │
          ├─→ api ──→ auth, proxmox, db, cache, security
          │    │
          │    └─→ observability
          │
          ├─→ auth ──→ db, security (jwt, bcrypt)
          ├─→ proxmox ──→ security (encryption)
          ├─→ db ──→ (depends on rusqlite, refinery)
          ├─→ cache ──→ proxmox
          ├─→ ui ──→ (depends on rust-embed)
          └─→ jobs ──→ db, proxmox
```

**Layer rules:**
- `api` is the topmost layer (no upward dependencies)
- `auth` and `proxmox` are mid-level (depend on `db`, `security`)
- `db` and `security` are foundational (no dependencies on other modules)

---

## 5. Sequence Diagrams

### 5.1 User login + dashboard load

```
User                 Browser              Caddy             MoxUI              Proxmox
 │                     │                    │                  │                   │
 │ 1. Open browser     │                    │                  │                   │
 │────────────────────→│                    │                  │                   │
 │                     │ 2. GET /           │                  │                   │
 │                     │───────────────────→│                  │                   │
 │                     │                    │ 3. GET /         │                   │
 │                     │                    │─────────────────→│                   │
 │                     │                    │                  │                   │
 │                     │                    │                  │ 4. Check session  │
 │                     │                    │                  │ (no JWT)          │
 │                     │                    │                  │                   │
 │                     │                    │ 5. HTML + JS     │                   │
 │                     │←────────────────────│←─────────────────│                   │
 │ 6. Render UI        │                    │                  │                   │
 │←────────────────────│                    │                  │                   │
 │                     │                    │                  │                   │
 │ 7. Enter creds      │                    │                  │                   │
 │────────────────────→│                    │                  │                   │
 │                     │ 8. POST /api/v1/auth/login             │                   │
 │                     │───────────────────→│─────────────────→│                   │
 │                     │                    │                  │                   │
 │                     │                    │                  │ 9. Verify bcrypt  │
 │                     │                    │                  │    (spawn_block)  │
 │                     │                    │                  │ 10. Check lockout │
 │                     │                    │                  │ 11. Check 2FA     │
 │                     │                    │                  │                   │
 │                     │                    │ 12. Set-Cookie: JWT │               │
 │                     │←────────────────────│←─────────────────│                   │
 │ 13. Render TOTP input                    │                  │                   │
 │←────────────────────│                    │                  │                   │
 │                     │                    │                  │                   │
 │ 14. Enter TOTP      │                    │                  │                   │
 │────────────────────→│                    │                  │                   │
 │                     │ 15. POST /api/v1/auth/2fa/verify        │                   │
 │                     │───────────────────→│─────────────────→│                   │
 │                     │                    │                  │ 16. Verify TOTP   │
 │                     │                    │                  │ 17. Issue JWT (15min)│
 │                     │                    │                  │ 18. Issue refresh (7d)│
 │                     │                    │                  │ 19. Audit log     │
 │                     │                    │ 20. 200 OK + JWT │                   │
 │                     │←────────────────────│←─────────────────│                   │
 │                     │                    │                  │                   │
 │                     │ 21. GET /api/v1/dashboard               │                   │
 │                     │───────────────────→│─────────────────→│                   │
 │                     │                    │                  │ 22. Check cache   │
 │                     │                    │                  │    (HIT)          │
 │                     │                    │ 23. JSON (cached)│                   │
 │                     │←────────────────────│←─────────────────│                   │
 │ 24. Render dashboard                    │                  │                   │
 │←────────────────────│                    │                  │                   │
```

### 5.2 VM start (write operation)

```
User           Browser         MoxUI                DB              Proxmox
 │               │               │                    │                 │
 │ 1. Click Start│               │                    │                 │
 │──────────────→│               │                    │                 │
 │               │ 2. POST /api/v1/vms/103/start       │                 │
 │               │──────────────→│                    │                 │
 │               │               │                    │                 │
 │               │               │ 3. Auth middleware │                 │
 │               │               │    (extract JWT)  │                 │
 │               │               │ 4. RBAC check      │                 │
 │               │               │    (operator ok)  │                 │
 │               │               │ 5. Rate limit ok   │                 │
 │               │               │ 6. Validate input │                 │
 │               │               │                    │                 │
 │               │               │ 7. Audit log       │                 │
 │               │               │───────────────────→│                 │
 │               │               │                    │                 │
 │               │               │ 8. Get ProxmoxClient (cached)        │
 │               │               │ 9. ensure_ticket()│                 │
 │               │               │10. POST .../status/start             │
 │               │               │───────────────────────────────────────→│
 │               │               │                    │                 │
 │               │               │                    │    11. UPID      │
 │               │               │←───────────────────────────────────────│
 │               │               │                    │                 │
 │               │               │12. Invalidate cache (VM list)        │
 │               │               │                    │                 │
 │               │               │ 13. 202 Accepted + UPID              │
 │               │←──────────────│                    │                 │
 │ 14. Toast "VM starting..."    │                    │                 │
 │←──────────────│               │                    │                 │
 │               │               │                    │                 │
 │               │ 15. Poll /api/v1/tasks/{upid}/status│                │
 │               │──────────────→│                    │                 │
 │               │               │16. Get task status│                 │
 │               │               │───────────────────────────────────────→│
 │               │               │17. "running"      │                 │
 │               │               │←───────────────────────────────────────│
 │               │ 18. {status: "OK"}│                  │                 │
 │               │←──────────────│                    │                 │
 │ 19. Toast "VM started"        │                    │                 │
 │←──────────────│               │                    │                 │
```

### 5.3 VNC console (WebSocket proxy)

```
User Browser      Caddy         MoxUI WebSocket       Proxmox VNC
 │  noVNC client    │              │                       │
 │                  │              │                       │
 │ 1. Open console │              │                       │
 │─────────────────→│              │                       │
 │                  │ 2. WS /api/v1/console/pve11/103│      │
 │                  │─────────────→│                       │
 │                  │              │                       │
 │                  │              │ 3. Auth check (JWT)  │
 │                  │              │ 4. RBAC check        │
 │                  │              │ 5. Get Proxmox ticket│
 │                  │              │ 6. POST /vncproxy    │
 │                  │              │──────────────────────→│
 │                  │              │                       │
 │                  │              │ 7. {port: 5900, ticket: "..."}│
 │                  │              │←──────────────────────│
 │                  │              │                       │
 │                  │              │ 8. TCP connect pve11:5900       │
 │                  │              │─────────────→│       │
 │                  │              │              │       │
 │ 9. WS message (keyboard)       │              │       │
 │←────────────────│←─────────────│              │       │
 │                  │              │              │       │
 │                  │              │ 10. Send to VNC      │
 │                  │              │─────────────→│       │
 │                  │              │              │       │
 │                  │              │ 11. VNC response     │
 │                  │              │←─────────────│       │
 │                  │              │              │       │
 │ 12. WS message (frame)         │              │       │
 │─────────────────→│─────────────→│              │       │
 │                  │              │              │       │
 │                  │              │ 13. Decode RFB, encode WS │    │
 │                  │              │ 14. Send to browser  │       │
 │                  │←─────────────│              │       │
 │ 15. Render canvas               │              │       │
 │←────────────────│              │              │       │
```

### 5.4 Multi-cluster aggregate dashboard

```
User                MoxUI                    Background Pollers              Proxmox
 │                    │                              │                         │
 │ 1. GET /dashboard │                              │                         │
 │───────────────────→│                              │                         │
 │                    │                              │                         │
 │                    │ 2. Check cache              │                         │
 │                    │    (cache TTL = 5s)         │                         │
 │                    │                              │                         │
 │                    ├─── if HIT: return cached ────┤                         │
 │                    │                              │                         │
 │                    ├─── if MISS: spawn all pollers                              │
 │                    │                              │                         │
 │                    │ 3. Poller cluster 1 (every 5s)│                         │
 │                    │    ────────────────────────────────────────────────→│   │
 │                    │    4. Update cache cluster 1  │                         │
 │                    │                              │                         │
 │                    │ 5. Poller cluster 2 (every 5s)│                         │
 │                    │    ────────────────────────────────────────────────→│   │
 │                    │    6. Update cache cluster 2  │                         │
 │                    │                              │                         │
 │                    │ 7. Aggregate all caches      │                         │
 │                    │ 8. Sort, filter, paginate    │                         │
 │                    │                              │                         │
 │                    │ 9. Return aggregate JSON     │                         │
 │←───────────────────│                              │                         │
 │                    │                              │                         │
 │ Note: Pollers run in background even when no user is logged in    │
```

---

## 6. Data Flow

### 6.1 Read path (cache-first)

```
Client → Caddy → MoxUI handler
                    ↓
                Auth middleware (extract User)
                    ↓
                RBAC check
                    ↓
                Rate limit
                    ↓
                Timeout (10s)
                    ↓
                Handler logic
                    ↓
                Check cache (moka)
                    ├─ HIT  → return cached (5ms)
                    │
                    └─ MISS → call Proxmox API
                              ↓
                              ProxmoxClient (with circuit breaker)
                              ↓
                              reqwest → Caddy → Proxmox
                              ↓
                              Cache result (TTL 5s)
                              ↓
                              Return to client
```

### 6.2 Write path (with audit + cache invalidation)

```
Client → Caddy → MoxUI handler
                    ↓
                Auth middleware
                    ↓
                RBAC check (write requires higher role)
                    ↓
                Validate input
                    ↓
                ┌─ BEGIN transaction ─┐
                ↓                       ↓
                Audit log INSERT    Action call
                ↓                       ↓
                Execute action       Proxmox API call
                ↓                       ↓
                Update DB           Cache invalidate
                ↓                       ↓
                └─ COMMIT transaction ┘
                    ↓
                Return 202 Accepted + UPID (task ID for long ops)
                    ↓
                Frontend polls task status
```

### 6.3 Token refresh flow

```
Client sends expired access token
                    ↓
                Auth middleware detects expired
                    ↓
                Returns 401 + WWW-Authenticate: Bearer
                    ↓
                Frontend catches 401
                    ↓
                POST /api/v1/auth/refresh (with refresh token cookie)
                    ↓
                Verify refresh token hash in DB
                    ↓
                Mark old token as used
                    ↓
                Issue new refresh token (rotation)
                    ↓
                Issue new access token (15 min)
                    ↓
                Set cookies
                    ↓
                Frontend retries original request
```

---

## 7. Technology Map

### 7.1 Backend stack

| Layer | Tech | Version | Purpose |
|---|---|---|---|
| **Language** | Rust | 1.78+ | Memory safety, performance |
| **Async runtime** | tokio | 1.x | Foundation for all async I/O |
| **Web framework** | axum | 0.7 | HTTP server + routing |
| **Middleware** | tower / tower-http | 0.4 / 0.5 | CORS, compression, trace |
| **HTTP client** | reqwest | 0.12 | Proxmox API calls |
| **Database** | rusqlite | 0.31 | SQLite driver (sync) |
| **DB pool** | r2d2 + r2d2_sqlite | latest | Connection pooling |
| **Migrations** | refinery | latest | Versioned SQL migrations |
| **Serialization** | serde + serde_json | 1.x | JSON encoding/decoding |
| **JWT** | jsonwebtoken | 9 | RS256 encode/decode |
| **Password hash** | bcrypt | 0.15 | Bcrypt cost 12 |
| **2FA (TOTP)** | totp-rs | 5 | RFC 6238 |
| **WebAuthn** | webauthn-rs | 0.5 | Passkey / Yubikey |
| **OIDC** | oauth2 | 4 | Google SSO + PKCE |
| **LDAP** | ldap3 | 0.11 | Active Directory |
| **TLS** | rustls | 0.23 | Pure Rust TLS |
| **Encryption** | chacha20poly1305 + age | latest | Cluster password encryption |
| **Cache** | moka | 0.12 | Async LRU + TTL |
| **Metrics** | prometheus | 0.13 | `/metrics` endpoint |
| **Tracing** | tracing + tracing-opentelemetry | 0.1 / 0.25 | Structured logs + OTLP |
| **Config** | figment | 0.10 | env + yaml + toml |
| **CLI** | clap | 4 | Argument parsing |
| **Errors** | thiserror + anyhow | 1 | Error enum + context |
| **Embedded UI** | rust-embed | 8 | Bundle HTML/CSS/JS |
| **WebSocket** | tokio-tungstenite | 0.21 | VNC proxy |
| **Rate limit** | tower-governor | latest | Rate limiting |
| **Mocks** | wiremock | 0.6 | Proxmox mock for tests |
| **Mocks** | mockall | 0.13 | Mock traits for tests |
| **Benchmarks** | criterion | 0.5 | Micro-benchmarks |

### 7.2 Frontend stack

| Library | Version | Size | Purpose |
|---|---|---|---|
| Alpine.js | 3.x | 15 KB | Reactivity (no build) |
| Tailwind CSS | 3.x JIT | 50 KB | Utility CSS (compiled) |
| uPlot | 1.x | 45 KB | Time-series charts |
| noVNC | 1.x | 150 KB | VNC client |
| Mousetrap | 1.x | 5 KB | Keyboard shortcuts |
| **Total** | | **~265 KB** | Plus HTML/CSS ≈ 400 KB |

### 7.3 Infrastructure

| Component | Tech |
|---|---|
| **Container registry** | ghcr.io |
| **CI/CD** | GitHub Actions |
| **TLS** | Let's Encrypt via Caddy / cert-manager |
| **Reverse proxy** | Caddy (homelab) / Traefik (K8s) |
| **Backup** | age-encrypted tar → S3/NFS |
| **Monitoring** | Prometheus + Grafana |
| **Log aggregation** | Loki (optional) |
| **Tracing** | OpenTelemetry → Jaeger / Tempo (optional) |

---

## 8. Security Boundaries

See [diagrams/security-boundaries.md](./diagrams/security-boundaries.md) for detailed security architecture.

### 8.1 Trust zones

```
┌─────────────────────────────────────────────────────────────────┐
│  Zone 0: Internet (untrusted)                                   │
│  - Random users, attackers                                      │
└─────────────────────────────┬───────────────────────────────────┘
                              │ TLS 1.3 + JWT
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Zone 1: Public (Caddy)                                         │
│  - TLS termination                                              │
│  - Rate limiting                                                 │
│  - Security headers                                             │
└─────────────────────────────┬───────────────────────────────────┘
                              │ HTTP (internal)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Zone 2: MoxUI (trusted app)                                    │
│  - JWT verification                                             │
│  - RBAC enforcement                                             │
│  - Audit log                                                    │
│  - Encryption/decryption                                        │
└─────────────────────────────┬───────────────────────────────────┘
                              │ HTTPS (private VLAN)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Zone 3: Proxmox API (trusted network)                          │
│  - Ticket auth                                                  │
│  - CA cert verification                                         │
│  - Rate limiting (Proxmox native)                               │
└─────────────────────────────┬───────────────────────────────────┘
                              │ local
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Zone 4: Proxmox nodes (full trust)                             │
│  - KVM/QEMU                                                     │
│  - Storage                                                      │
│  - VM consoles                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 8.2 Security headers (every response)

```
Strict-Transport-Security: max-age=63072000; includeSubDomains; preload
Content-Security-Policy: default-src 'self'; script-src 'self'; object-src 'none'; frame-ancestors 'none'
X-Frame-Options: DENY
X-Content-Type-Options: nosniff
Referrer-Policy: strict-origin-when-cross-origin
Permissions-Policy: camera=(), microphone=(), geolocation=()
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Resource-Policy: same-origin
```

---

## 9. Scaling & Performance

### 9.1 Horizontal scaling (multi-instance)

```
                    Load Balancer (sticky session for WebSocket)
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
        ┌─────────┐     ┌─────────┐     ┌─────────┐
        │ MoxUI 1 │     │ MoxUI 2 │     │ MoxUI 3 │
        └────┬────┘     └────┬────┘     └────┬────┘
             │               │               │
             └───────────────┴───────────────┘
                              │
                              ▼
                    ┌──────────────────┐
                    │  Proxmox API     │
                    └──────────────────┘
```

**Note:** SQLite is per-instance (RWO volume) — HA requires replication (future, v2.0)

### 9.2 Vertical scaling

| Metric | 1 vCPU / 1 GB | 2 vCPU / 2 GB | 4 vCPU / 8 GB |
|---|---|---|---|
| Concurrent users | ~50 | ~200 | ~500+ |
| Polling clients (2s) | ~25 | ~100 | ~250+ |
| Memory at idle | 50 MB | 50 MB | 50 MB |
| Memory at load | 200 MB | 500 MB | 800 MB |
| CPU at load | 30% | 60% | 80% |

### 9.3 Performance hot paths

```
Read (VM list):
  Cache HIT  → 5ms (p99)
  Cache MISS → 50-200ms (Proxmox API call)

Write (VM start):
  Audit log INSERT → 5ms
  Proxmox API POST → 100-500ms
  Cache invalidate → <1ms

VNC console:
  First frame → 100ms (after WS connect)
  Subsequent frames → 30-60 fps
```

### 9.4 Caching strategy

| Data | TTL | Invalidation |
|---|---|---|
| VM list (per cluster) | 5s | On any VM create/delete |
| VM detail | 5s | On any VM update |
| Cluster stats | 10s | On any cluster update |
| User list | 60s | On user create/delete |
| Audit log (recent) | 30s | Append-only (no invalidation) |

---

## 10. Failure Modes

### 10.1 Single cluster down

**Symptom:** Proxmox client for cluster X returns errors

**Behavior:**
- Circuit breaker opens after 5 failures
- Dashboard shows "unreachable" badge for cluster X
- Other clusters continue working
- Auto-retry after 30s (half-open)
- User sees error toast only for cluster X actions

**Recovery:** Automatic (no manual intervention)

### 10.2 Database corruption

**Symptom:** SQLite integrity check fails

**Behavior:**
- Container fails to start (refuses to serve with corrupt DB)
- Logs error: "Database integrity check failed"
- On-call alert (Prometheus)
- Restore from latest backup (`scripts/restore.sh`)

**Recovery:** Manual (restore from backup)

### 10.3 Out of memory

**Symptom:** Container OOM killed

**Behavior:**
- Kubernetes / Docker restarts container automatically
- Graceful shutdown triggered before OOM (if possible)
- New instance loads cache cold, poller refills

**Recovery:** Automatic restart + manual investigation

### 10.4 Disk full

**Symptom:** SQLite write fails (no space)

**Behavior:**
- Backend returns 500 error
- Logs error
- Audit log can't be written (warn only)
- WAL file grows, eventually fails

**Recovery:** Manual (cleanup disk, increase volume size)

### 10.5 Proxmox API rate limit

**Symptom:** Proxmox returns 429

**Behavior:**
- Circuit breaker engages
- Poller backs off (no immediate retry)
- User actions get rate limit response (429)
- After 30s cooldown, resume

**Recovery:** Automatic (backoff)

### 10.6 TLS cert expiry

**Symptom:** Caddy / cert-manager fails to renew cert

**Behavior:**
- Container continues running
- Browser shows cert warning
- Users can bypass with manual exception (not recommended)

**Recovery:** Manual (renew cert via cert-manager)

---

## 📎 See also

- [`docs/diagrams/`](./diagrams/) — detailed diagrams
  - [`state-machines.md`](./diagrams/state-machines.md) — VM/LXC state transitions
  - [`request-flows.md`](./diagrams/request-flows.md) — detailed request flows
  - [`security-boundaries.md`](./diagrams/security-boundaries.md) — security architecture
  - [`data-flow.md`](./diagrams/data-flow.md) — data movement
- [`docs/DATA_MODEL.md`](./DATA_MODEL.md) — DB schema + ER diagrams
- [`docs/FEATURE_SCOPE.md`](./FEATURE_SCOPE.md) — feature list
- [`PROPOSAL.md`](../PROPOSAL.md) — high-level proposal

---

**Last updated:** 2026-06-20
**Status:** Architecture design complete
**Next:** Implementation (Phase 0 Day 1)