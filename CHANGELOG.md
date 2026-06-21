# Changelog

All notable changes to moxui are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-06-21

### Phase 0: Read-only MVP

First usable release. MoxUI can authenticate users, list VMs / LXC
containers / storage pools, and start/stop/shutdown/reboot QEMU VMs.
It enforces HTTPS, RBAC, and writes every state-changing request to a
tamper-evident audit log.

#### Added

- **Auth (Day 4)**: moxui-side auth middleware (RS256 JWT). `POST
  /api/v1/auth/login` exchanges username + password for a token.
  `GET /api/v1/auth/me` echoes the current claim set. Three roles
  (`admin`, `operator`, `viewer`) with a privilege hierarchy.
- **VM endpoints (Day 3)**: `GET /api/v1/vms` (cross-cluster
  aggregation), `GET /api/v1/vms/:cluster/:vmid` (single-VM detail),
  `POST /api/v1/vms/:cluster/:node/:vmid/:action` (start / stop /
  shutdown / reboot — returns the Proxmox UPID for completion polling).
- **LXC endpoints (Day 5)**: `GET /api/v1/lxcs`,
  `GET /api/v1/lxcs/:cluster/:node/:vmid`.
- **Storage endpoints (Day 5)**: `GET /api/v1/storages`,
  `GET /api/v1/storages/:cluster/:node/:storage/content` (ISO images,
  container templates, etc.).
- **Audit log (Day 2.4)**: Every state-changing request is recorded to
  a SQLite database (`<db_path>.audit`) with the user ID (extracted
  from the JWT `sub` claim), method, path, status, and timestamp.
- **Health + readiness (Day 2.3)**: `GET /health` (detailed JSON
  status), `GET /livez` (k8s liveness, 200), `GET /readyz` (k8s
  readiness — pings every configured Proxmox cluster with a 10s TTL
  cache).
- **Secret hygiene (Day 3)**: All Proxmox credentials wrapped in
  `SecretString` (zeroed on drop). Tickets redacted in `Debug`.
- **Config (Day 2)**: TOML/YAML config with figment, fail-closed on
  missing/invalid fields.
- **HTTPS + security headers (Day 6)**: Optional TLS termination
  (axum-server + rustls). When `server.tls` is configured, the server
  listens with HTTPS only; otherwise it logs a startup warning and
  serves plaintext (dev mode). Every response gets
  HSTS / X-Content-Type-Options / X-Frame-Options / Referrer-Policy /
  CSP headers.
- **Packaging (Day 7)**: `make build-release` produces a stripped
  binary with LTO + single-codegen-unit + abort-on-panic. `make
  package-deb` builds a Debian package (systemd unit, moxui user,
  hardened `ProtectSystem=strict` / `NoNewPrivileges` etc.).

#### Security

- HTTPS-only in production (configurable; default = dev plaintext)
- HSTS with `max-age=31536000; includeSubDomains`
- Content-Security-Policy: `default-src 'self'`
- X-Frame-Options: DENY
- X-Content-Type-Options: nosniff
- All auth passwords bcrypt-hashed on the wire-side
- JWT keys must be 2048-bit RSA minimum (fail-closed on missing keys)
- Audit log captures every write, indexed by user
- Production systemd unit runs as `moxui:moxui` with `ProtectSystem=strict`,
  `NoNewPrivileges`, `RestrictNamespaces`, `MemoryDenyWriteExecute`

#### Known limitations

- **No write endpoints for LXC or storage** — Phase 1.
- **No WebSocket / no live console** — Phase 2.
- **Plaintext HTTP dev mode** — must be replaced with TLS for production.
- **No cluster-level `/cluster/*` endpoints** (e.g. HA, replication) —
  Phase 3.
- **Self-signed / in-cluster certs require `insecure_skip_verify: true`
  unless you ship a CA** — by design, but requires operator action.

#### Statistics

| Metric | Value |
|---|---|
| Source lines (lib + bin) | ~3500 |
| Test count | 73 |
| Dependency count (direct) | 35 |
| CI gates | fmt + clippy + test + audit |

[0.1.0]: https://github.com/kungjom26/moxui/releases/tag/v0.1.0
