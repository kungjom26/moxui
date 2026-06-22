# Changelog

All notable changes to MoxUI are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.2.0] — 2026-06-22

### Phase 5 — Power User Features

Added multi-region replication, plugin system, Terraform provider, and migration wizard.

#### Multi-Region Replication

- **`src/api/replication.rs`** — CRUD API for replication jobs across clusters
- **`GET /api/v1/replication`** — List all replication jobs
- **`POST /api/v1/replication/:cluster/:vmid`** — Create a new replication job
- **`DELETE /api/v1/replication/:cluster/:vmid/delete`** — Delete a replication job
- **`GET /api/v1/replication/:cluster/:vmid/status`** — Get replication job status
- **`ProxmoxClient` methods** — `list_replication()`, `create_replication()`, `delete_replication()`, `get_replication_status()`
- **`src/proxmox/types.rs`** — `ReplicationJob`, `ReplicationStatus`, `ReplicationSchedule` types

#### Plugin System

- **`src/plugin/mod.rs`** — `MoxuiPlugin` trait with lifecycle hooks (`on_register`, `before_request`, `after_request`, `on_shutdown`)
- **`src/plugin/audit_logger.rs`** — Built-in plugin: captures every request/response to audit log
- **`src/plugin/webhook_bridge.rs`** — Built-in plugin: dispatches webhook events via `WebhookDispatcher`
- **`PluginRegistry`** — Thread-safe registry loading plugins from config
- **Config integration** — `plugins` section with enable/disable, per-plugin settings

#### Terraform Provider

- **`deploy/terraform/`** — Full Terraform provider scaffold
  - **`provider/provider.go`** — Provider + `moxui_vm` resource CRUD (Create/Read/Update/Delete)
  - **`provider/main.go`** — Provider entrypoint
  - **`provider/resources/resource_vm.go`** — VM resource: `name`, `cluster`, `node`, `vmid`, `memory`, `cores`, `disk_size`, `disk_storage`, `network_bridge`, `iso`, `start_on_create`
  - **`provider/resources/resource_vm_test.go`** — Acceptance tests
  - **`provider/go.mod`** — Go module with `terraform-plugin-sdk/v2`
  - **`deploy/terraform/main.tf`** — Example configuration
  - **`deploy/terraform/variables.tf`** — Variable definitions
  - **`deploy/terraform/outputs.tf`** — Output definitions
  - **`deploy/terraform/README.md`** — Getting started guide
  - **`deploy/terraform/examples/basic/README.md`** — Basic example docs
  - **`deploy/terraform/provider/docs/index.md`** — Provider documentation

#### Migration Wizard

- **`ui/index.html`** — 6-step setup wizard UI with step indicators:
  1. Welcome & connection check
  2. Proxmox cluster configuration (name, URL, credentials)
  3. Proxmox data import (VMs, storage, networks summary)
  4. Admin user creation + role assignment
  5. Feature selection (auth, audit, monitoring, alerts, i18n)
  6. Apply + deployment summary
- **`ui/static/app.js`** — Wizard state machine, form validation, API calls
- **`src/config.rs`** — Config validation for wizard settings

#### Statistics

| Metric | Value |
|---|---|
| Source lines (lib + bin) | ~8,000 |
| Test count | 170 |
| Files added | 31 |
| Terraform provider | Go SDK v2, 1 resource |
| Plugin system | 2 built-in plugins |
| Replication endpoints | 4 |

---

## [1.1.0] — 2026-06-22

### Phase 4 — Polish & Community (7 features)

Added live migration, HA group management, bulk operations, webhooks, custom dashboards, and i18n.

#### Live Migration UI

- **`POST /api/v1/vms/:cluster/:node/:vmid/migrate`** — Trigger live migration with target node + live flag
- **`ProxmoxClient::migrate_vm()`** — Client method calling Proxmox migration endpoint
- **Frontend** — Migration modal with target node dropdown + live/offline toggle

#### HA Group Management

- **`src/api/hagroups.rs`** — Full CRUD for HA groups
- **`GET /api/v1/hagroups`** — List all HA groups
- **`POST /api/v1/hagroups/:cluster/:group`** — Create a new HA group
- **`DELETE /api/v1/hagroups/:cluster/:group`** — Delete an HA group
- **`ProxmoxClient` methods** — `list_ha_groups()`, `create_ha_group()`, `delete_ha_group()`
- **Frontend** — HA Groups page with create/edit/delete

#### Bulk Operations

- **`POST /api/v1/vms/bulk/start`** — Start multiple VMs (JSON array of `{cluster, node, vmid}`)
- **`POST /api/v1/vms/bulk/stop`** — Stop multiple VMs
- **`POST /api/v1/vms/bulk/reboot`** — Reboot multiple VMs
- **`POST /api/v1/vms/bulk/delete`** — Delete multiple VMs
- **Frontend** — Checkbox selection + "Select All" + action toolbar with batch confirmations

#### Webhook Notifications

- **`src/webhook/mod.rs`** — `WebhookDispatcher` trait + registry for webhook delivery
- **`src/webhook/dispatcher.rs`** — Slack/Discord formatters, HMAC signing, retry with backoff
- **`WebhookDispatcher` trait** — `dispatch(event, payload)` with thread-safe async dispatch
- **Config integration** — `notifications.webhooks[]` with channel type, URL, secret, enabled flag
- **Two formatters** — Slack (`blocks` API) and Discord (`embeds` API)

#### Custom Dashboards

- **`src/dashboard_custom/mod.rs`** — Widget configuration (type, position, size, settings) with JSON persistence
- **`GET /api/v1/dashboard/custom`** — Get user's saved dashboard layout
- **`POST /api/v1/dashboard/custom`** — Save dashboard layout
- **`GET /api/v1/dashboard/custom/widget-types`** — List available widget types
- **Frontend** — Drag & drop widget grid, add/remove/rearrange widgets

#### Internationalization (i18n)

- **`ui/locales/en.json`** — English translations (199 keys)
- **`ui/locales/th.json`** — Thai translations (199 keys)
- **Frontend** — `$t()` function for key-based translation, language switcher dropdown
- **Keys organized** — sidebar, vms, storage, network, dashboard, auth, audit, settings, common, notifications

#### Statistics

| Metric | Value |
|---|---|
| Source lines (lib + bin) | ~7,000 |
| Test count | 170 |
| Files added | 20 |
| Webhook formatters | 2 (Slack + Discord) |
| i18n locales | 2 (EN + TH) |
| Locale keys | 199 |

---

## [1.0.0] — 2026-06-22

### Production Release — v1.0.0 MVP

v1.0.0 consolidates all Phase 0–3 features into a production-ready MVP.

#### Phase 3 — WebAuthn / Passkey Support (Day 21)

Added passwordless authentication via WebAuthn / passkeys (platform authenticators,
YubiKeys, Touch ID, Windows Hello).

##### Added

- **`auth::webauthn::WebauthnState`** — Manages WebAuthn registration + authentication
  state using the `webauthn-rs` crate (core-level, pre-0.5 preview types).
- **`POST /api/v1/auth/webauthn/register/start`** — Returns a passkey creation challenge
  for the authenticated user. Auth required (admin/operator).
- **`POST /api/v1/auth/webauthn/register/complete`** — Stores the registered credential
  after browser `navigator.credentials.create()`.
- **`POST /api/v1/auth/webauthn/login/start`** — Returns an assertion challenge for a
  given username. Public endpoint.
- **`POST /api/v1/auth/webauthn/login/complete`** — Verifies the passkey assertion and
  issues a JWT + refresh token (same flow as password login).
- **WebAuthn config** — `auth.webauthn` section with `enabled`, `rp_id`, `rp_origin`,
  `rp_name` fields. Fail-closed when `enabled: true` but config is invalid.
- **WebAuthn state stored** in `AppState.webauthn` (`Option<Arc<WebauthnState>>`).
  Disabled by default — opt-in via `auth.webauthn.enabled: true`.

#### Phase 3 — Rate limiting, CORS, API keys, Audit UI, TOTP 2FA (Days 16–18)

##### Added

- **Rate limiting** — `tower-governor` middleware (5 req/sec per IP, burst 10).
  Configurable via `auth.rate_limit`. Applied globally to all routes.
- **CORS** — Configurable allowed origins (`auth.cors.allowed_origins`).
  Empty = permissive mode (development). Max-age headers for preflight caching.
- **API key authentication** — `X-API-Key` header support via `auth.api_key`.
  Coexists with Bearer JWT auth. Ideal for automation/CI pipelines.
- **Audit API endpoint** — `GET /api/v1/audit` with pagination, filtering (method,
  path, status, timestamp, request_id), and sort direction. Returns paginated
  results with total count and page info.
- **TOTP 2FA (RFC 6238)** — `POST /api/v1/auth/2fa/complete` completes a 2FA login
  with a 6-digit TOTP code or 8-digit backup code.
- **`POST /api/v1/auth/2fa/setup`** — Generates a new TOTP secret and QR URL.
- **`POST /api/v1/auth/2fa/verify`** — Verifies a TOTP code to enable 2FA.
- **`POST /api/v1/auth/2fa/disable`** — Disables 2FA (requires current password).
- **PreAuthStore** — In-memory store for 2FA pending sessions (5-min TTL).
- **Backup codes** — 8-digit single-use backup codes generated with 2FA setup.

#### Phase 2 — Refresh tokens, logout, polish (Days 14–15)

##### Added

- **`auth::refresh` module** — `RefreshStore` with `issue` / `verify` / `revoke` /
  `revoke_all_for_user` / `rotate`. Tokens are 32-byte random values (256-bit
  entropy), stored as SHA-256 hashes only.
- **Family revocation** — Replaying a revoked token revokes all tokens for that user.
- **`POST /api/v1/auth/refresh`** — Exchange a refresh token for a new JWT + new
  refresh token (rotation). Invalid/expired/replayed tokens return 401.
- **`POST /api/v1/auth/logout`** — Revoke a refresh token. Always returns 200
  to avoid leaking token validity.
- **Login response** — Now includes `refresh_token` field alongside the JWT.
- **`UserStore::get_by_id`** — Look up a user by their `id` field.
- **End-to-end integration tests** — Auth login (valid, wrong password, unknown user,
  disabled account), auth me (valid token, missing token, expired token),
  VM config, VNC ticket (disabled, viewer rejected, unauthenticated),
  storage auth checks, task status, empty cluster list.
- **Criterion benchmarks** — `jwt_encode` (~535µs), `jwt_decode` (~20µs),
  `bcrypt_hash` (~195ms), `bcrypt_verify` (~194ms).

#### Phase 1 — Core read + VM control (Days 5–13)

##### Added

- **VNC console backend** — `POST .../vnc/ticket` mints short-lived HMAC-SHA256
  VNC tokens (5-min TTL, bound to cluster/node/vmid). Operator+ role required.
  Proxmox `vncproxy` ticket never leaves the process.
- **`VncConnectionLimiter`** — Atomic concurrency limiter (max 5 per VM).
- **VM detail page** — Frontend tabs (summary, config, console) with action buttons
  and confirmation dialogs.
- **VM list enhancements** — Search, sort, filter, auto-polling, stale indicators,
  error states.
- **Frontend skeleton** — Alpine.js SPA served via `rust-embed`. Responsive layout
  with dark/light theme support.
- **Network read endpoints** — `GET /api/v1/networks` (cross-cluster aggregate)
  and `GET /api/v1/networks/:cluster/:node` (per-node). Bridges, bonds, VLANs,
  physical NICs, Linux aliases.
- **VM delete** — `DELETE /api/v1/vms/:cluster/:node/:vmid` with purge/force/skiplock
  options. Operator+ required.
- **VM config endpoint** — `GET /api/v1/vms/:cluster/:node/:vmid/config`.

#### Phase 0 — Foundation (Days 1–4)

##### Added

- **Cargo project** — Rust 2021 edition, axum 0.7 web framework, tokio async runtime,
  rusqlite database, reqwest HTTP client, figment config loading.
- **Module skeleton** — api, auth, proxmox, audit, security, config, state, cache,
  observability modules.
- **LXC read endpoints** — `GET /api/v1/lxcs` and `GET /api/v1/lxcs/:cluster/:node/:vmid`.
- **Storage read endpoints** — `GET /api/v1/storages` and
  `GET /api/v1/storages/:cluster/:node/:storage/content`
  (ISO images, container templates).
- **Auth (JWT + RBAC)** — `POST /api/v1/auth/login` exchanges username + password
  for an RS256 JWT. `GET /api/v1/auth/me` echoes the current claim set. Three roles:
  `admin`, `operator`, `viewer` with a privilege hierarchy.
- **VM write endpoints** — `POST /api/v1/vms/:cluster/:node/:vmid/:action`
  (start / stop / shutdown / reboot — returns the Proxmox UPID).
- **Audit log** — Every state-changing request recorded to SQLite with user ID,
  method, path, status, and timestamp.
- **Health + readiness** — `GET /health` (detailed JSON), `GET /livez` (k8s liveness),
  `GET /readyz` (k8s readiness with 10s TTL cache).
- **Secret hygiene** — All Proxmox credentials wrapped in `SecretString` (zeroed on
  drop). Tickets redacted in `Debug`.
- **Config** — TOML/YAML config with figment, fail-closed on missing/invalid fields.
  Environment variable overrides via `MOXUI_*` prefix.
- **TLS + security headers** — Optional HTTPS (axum-server + rustls). HSTS,
  X-Content-Type-Options, X-Frame-Options, CSP, Referrer-Policy on every response.
- **Packaging** — `make build-release` (LTO + strip + abort-on-panic),
  `make package-deb` (Debian package with systemd unit, hardened `ProtectSystem=strict`).
- **OpenTelemetry tracing** — OTLP gRPC exporter (Jaeger, Tempo, SigNoz).
  Configurable via `tracing` config section.
- **Prometheus metrics** — `/metrics` endpoint with configurable registry.
- **CI gates** — fmt, clippy, test, audit, bench.

#### Security

- HTTPS-only in production (configurable; dev mode warns on plaintext)
- HSTS with `max-age=31536000; includeSubDomains`
- Content-Security-Policy: `default-src 'self'`
- X-Frame-Options: DENY
- X-Content-Type-Options: nosniff
- Referrer-Policy: no-referrer
- All auth passwords bcrypt-hashed
- JWT keys must be 2048-bit RSA minimum (fail-closed on missing keys)
- Audit log captures every write, indexed by user
- SHA-256 hashed refresh tokens (plaintext never persisted)
- Refresh token rotation invalidates old token on each use
- Family revocation: replayed tokens trigger full user token invalidation
- Rate limiting on login (5 req/sec per IP)
- CORS restricted to configured origins in production
- Production systemd unit with `ProtectSystem=strict`, `NoNewPrivileges`,
  `RestrictNamespaces`, `MemoryDenyWriteExecute`
- VNC token secret minimum 32 bytes enforced
- Fail-closed: missing JWT keys or VNC secret prevents startup
- `#![forbid(unsafe_code)]` at crate level

#### Known Limitations (v1.0.0)

- **No LXC write endpoints** — start/stop/shutdown/reboot for containers (v1.1)
- **No VM/LXC creation or config editing** — v1.1 feature
- **VNC WebSocket proxy** — Ticket minting works end-to-end; WS proxy returns
  501 (Phase 2 follow-up requires `tokio-tungstenite` with rustls)
- **No storage write endpoints** — upload, download URL, delete (v1.1)
- **No cluster-level endpoints** — HA status, replication, firewall (v1.1+)
- **No LDAP/AD authentication** — v1.1 feature
- **No user management UI** — users seeded from config file only

#### Statistics

| Metric | Value |
|---|---|
| Source lines (lib + bin) | ~4,500 |
| Test count | 140+ |
| Dependency count (direct) | ~45 |
| Benchmark suites | 4 |
| CI gates | fmt + clippy + test + audit + bench |
| Docker image size | ~15 MB (runtime) |
| Roles | admin, operator, viewer |
| Auth methods | local, TOTP, WebAuthn, OIDC, API key |

---

## [0.2.0] — 2026-06-22

### Phase 2 — Refresh token + logout (Day 15)

*See [1.0.0] Phase 2 for full details.*

Key additions that were later incorporated into v1.0.0:
- `auth::refresh` module with SHA-256 hashed refresh tokens
- Refresh token rotation with family revocation
- `POST /api/v1/auth/refresh` and `POST /api/v1/auth/logout`

#### Statistics

| Metric | Value |
|---|---|
| Source lines (lib + bin) | ~3,700 |
| Test count | 133 |
| Refresh token entropy | 256 bits |
| Refresh token TTL | 7 days |

---

## [0.1.1] — 2026-06-22

### Phase 1 polish (Day 14)

*See [1.0.0] Phase 2 for full details.*

Key additions that were later incorporated into v1.0.0:
- End-to-end integration tests for every unprotected endpoint
- Criterion benchmarks for auth-critical paths

#### Statistics

| Metric | Value |
|---|---|
| Source lines (lib + bin) | ~3,500 |
| Test count | 118 |
| Benchmark suites | 4 |

---

## [0.1.0] — 2026-06-21

### Phase 0: Read-only MVP

*See [1.0.0] Phase 0 for full details.*

First usable release. MoxUI can authenticate users, list VMs / LXC containers /
storage pools, and start/stop/shutdown/reboot QEMU VMs. It enforces HTTPS, RBAC,
and writes every state-changing request to a tamper-evident audit log.

#### Statistics

| Metric | Value |
|---|---|
| Source lines (lib + bin) | ~3,500 |
| Test count | 73 |
| Dependency count (direct) | 35 |
| CI gates | fmt + clippy + test + audit |

---

## [0.0.0] — 2026-06-20

### Pre-release — Design Phase

- Project proposal, roadmap, feature scope ([PROPOSAL.md](./PROPOSAL.md))
- Architecture diagrams ([docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md))
- Data model specification ([DATA_MODEL.md](./DATA_MODEL.md))
- Implementation plan with 3-role team structure
- 168 features defined with tier + acceptance criteria

---

[1.2.0]: https://github.com/kungjom26/moxui/releases/tag/v1.2.0
[1.1.0]: https://github.com/kungjom26/moxui/releases/tag/v1.1.0
[1.0.0]: https://github.com/kungjom26/moxui/releases/tag/v1.0.0
[0.2.0]: https://github.com/kungjom26/moxui/releases/tag/v0.2.0
[0.1.1]: https://github.com/kungjom26/moxui/releases/tag/v0.1.1
[0.1.0]: https://github.com/kungjom26/moxui/releases/tag/v0.1.0
