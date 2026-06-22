# Development Guide

> MoxUI — Setting up the development environment, understanding the codebase, and contributing

---

## Table of Contents

- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Project Structure](#project-structure)
- [Code Architecture](#code-architecture)
- [Running Tests](#running-tests)
- [Benchmarking](#benchmarking)
- [CI Gates](#ci-gates)
- [Contributing](#contributing)

---

## Prerequisites

| Tool | Version | Installation |
|---|---|---|
| Rust | 1.78+ | [rustup.rs](https://rustup.rs/) |
| Cargo | latest (via rustup) | Included with Rust |
| OpenSSL dev libs | latest | `apt install pkg-config libssl-dev` |
| Docker (optional) | latest | [docker.com](https://docs.docker.com/get-docker/) |
| Helm (optional) | 3.x | [helm.sh](https://helm.sh/docs/intro/install/) |

---

## Quick Start

```bash
# Clone the repository
git clone https://github.com/kungjom26/moxui.git
cd moxui

# Generate test JWT keys (for development)
mkdir -p tests/fixtures
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out tests/fixtures/test_jwt_priv.pem
openssl pkey -in tests/fixtures/test_jwt_priv.pem -pubout -out tests/fixtures/test_jwt_pub.pem

# Generate test TLS certs (for development)
openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout tests/fixtures/test_tls_key.pem \
  -out tests/fixtures/test_tls_cert.pem \
  -days 365 -subj "/CN=localhost"

# Run in dev mode
cargo run
# Server starts at http://localhost:8080 (plaintext HTTP — dev mode warning)
```

### Dev Configuration

Create a `config.yaml` in the project root:

```yaml
server:
  bind: "0.0.0.0:8080"
  workers: 0

database:
  path: "moxui.db"

logging:
  level: "debug"
  format: "pretty"

clusters: []  # No Proxmox clusters needed for testing most features

auth:
  jwt_private_key_pem_path: "tests/fixtures/test_jwt_priv.pem"
  jwt_public_key_pem_path: "tests/fixtures/test_jwt_pub.pem"
  users:
    - id: "u-admin"
      username: "admin"
      display_name: "Admin User"
      password: "admin123"
      role: "admin"
```

Then run with:

```bash
MOXUI_CONFIG=./config.yaml cargo run
```

---

## Project Structure

```
moxui/
├── Cargo.toml              # Dependencies, build config, profiles
├── Makefile                # Build, test, package, install targets
├── Dockerfile              # Multi-stage Docker build
├── config.example.yaml     # Example configuration (committed)
│
├── src/
│   ├── main.rs             # Binary entry point — CLI, config load, server bootstrap
│   ├── lib.rs              # Library root — module declarations, build metadata
│   │
│   ├── api/                # HTTP API layer (axum handlers)
│   │   ├── mod.rs          # Router builder — wires all routes + middleware
│   │   ├── auth.rs         # Login, logout, refresh, 2FA, OIDC callback handlers
│   │   ├── audit.rs        # GET /api/v1/audit — paginated audit log
│   │   ├── dashboard.rs    # GET /api/v1/dashboard — aggregate cluster stats
│   │   ├── health.rs       # /health, /livez, /readyz, /metrics
│   │   ├── lxcs.rs         # LXC container list + detail
│   │   ├── networks.rs     # Network interface listing
│   │   ├── storages.rs     # Storage pool listing + content
│   │   ├── tasks.rs        # Proxmox task status polling
│   │   ├── vms.rs          # VM list, detail, action (start/stop/etc), config
│   │   ├── vnc.rs          # VNC console ticket + WebSocket proxy
│   │   └── webauthn.rs     # WebAuthn registration + login handlers
│   │
│   ├── auth/               # Authentication & authorization
│   │   ├── mod.rs          # Re-exports
│   │   ├── jwt.rs          # RS256 JWT encode/decode
│   │   ├── user.rs         # User model, Role enum, UserStore
│   │   ├── middleware.rs   # require_auth, require_cluster_access, require_role
│   │   ├── password.rs     # bcrypt hash + verify
│   │   ├── refresh.rs      # Refresh token store (SHA-256 hashed, with rotation)
│   │   ├── totp.rs         # TOTP (RFC 6238) + backup codes
│   │   ├── webauthn.rs     # WebAuthn state (webauthn-rs crate integration)
│   │   ├── oidc.rs         # OIDC/OAuth2 SSO (Google + GitHub)
│   │   └── vnc.rs          # VNC token mint + verify (HMAC-SHA256)
│   │
│   ├── proxmox/            # Proxmox VE API client
│   │   ├── mod.rs          # Re-exports
│   │   ├── client.rs       # ProxmoxClient — API HTTP client with auth, retry, cache
│   │   ├── auth.rs         # Proxmox ticket auth
│   │   ├── types.rs        # Proxmox API response types
│   │   ├── retry.rs        # Configurable retry with jitter
│   │   └── circuit_breaker.rs # Circuit breaker for upstream failures
│   │
│   ├── audit/              # Audit logging
│   │   ├── mod.rs          # Re-exports
│   │   ├── store.rs        # SQLite-based audit store
│   │   └── middleware.rs   # Axum middleware that logs every request
│   │
│   ├── security/           # Security middleware
│   │   ├── mod.rs          # CORS layer, security headers
│   │   ├── rate_limiter.rs # Rate limiting (tower-governor)
│   │   └── api_key.rs      # X-API-Key header authentication
│   │
│   ├── config.rs           # Configuration loading (figment: yaml + env + defaults)
│   ├── state.rs            # AppState — shared application state
│   ├── error.rs            # AppError / AppResult types
│   ├── db/                 # Database layer (SQLite migrations)
│   ├── cache/              # Response caching (moka)
│   ├── tls.rs              # TLS server bootstrap (axum-server + rustls)
│   ├── telemetry.rs        # tracing-subscriber initialization
│   ├── ui/                 # Embedded frontend (Alpine.js SPA via rust-embed)
│   └── observability/      # Metrics + OpenTelemetry
│       ├── mod.rs
│       ├── metrics.rs      # Prometheus metrics
│       └── tracing.rs      # OTLP tracing configuration
│
├── tests/
│   └── fixtures/           # Test certs, keys, sample data
│       ├── test_jwt_priv.pem
│       ├── test_jwt_pub.pem
│       ├── test_tls_cert.pem
│       └── test_tls_key.pem
│
├── benches/
│   └── auth_benchmarks.rs  # Criterion benchmarks (JWT, bcrypt)
│
├── ui/                     # Frontend source (Alpine.js, Tailwind CSS)
│   └── ...                 # Built assets embedded via rust-embed
│
├── deploy/
│   └── k8s/moxui/          # Helm chart
│       ├── Chart.yaml
│       ├── values.yaml
│       └── templates/      # Deployment, ConfigMap, Secret, Ingress, HPA, etc.
│
├── contrib/                # Packaging support files
│   ├── moxui.service       # systemd unit
│   └── moxui.yaml.example  # Example config for packaging
│
├── docs/                   # Documentation
│   ├── installation.md
│   ├── configuration.md
│   ├── authentication.md
│   ├── deployment.md
│   ├── development.md
│   └── proxmox-api-coverage.md
│
├── README.md
├── CHANGELOG.md
└── LICENSE                 # MIT
```

---

## Code Architecture

### Design Principles

- **No unsafe code** — `#![forbid(unsafe_code)]` enforced at the crate level
- **Fail-closed** — Missing JWT keys, VNC secrets, or config fields cause startup failure
- **Secret hygiene** — Passwords in `SecretString` (zeroed on drop, redacted in Debug)
- **Defense in depth** — TLS, CSP, HSTS, rate limiting, audit logging, RBAC
- **Testability** — In-process router tests with wiremock, moka cache for Proxmox responses

### Request Flow

```
Client → Security Headers Middleware → CORS Layer → Rate Limiter → Audit Middleware
    → Router
        ├── Public routes: /health, /livez, /readyz, /login, /refresh, /logout
        ├── Protected routes: require_auth → handler
        └── Cluster-scoped routes: require_auth → require_cluster_access → handler
    → Response (with X-Request-Id header)
```

### State Management

`AppState` holds all shared dependencies and is cloned for every handler:
- `Arc<Config>` — Immutable config
- `Arc<Vec<ProxmoxClient>>` — One client per cluster
- `Arc<AuditStore>` — Thread-safe SQLite audit store
- `Arc<JwtService>` — JWT encode/decode
- `Arc<UserStore>` — In-memory user accounts
- `Arc<RefreshStore>` — Refresh token store
- `Arc<PreAuthStore>` — 2FA pending sessions
- `Option<Arc<WebauthnState>>` — WebAuthn state (optional)
- `Option<Arc<OidcService>>` — OIDC SSO service (optional)
- `Option<MetricsService>` — Prometheus metrics (optional)

### Build Profiles

| Profile | Uses | Features |
|---|---|---|
| `dev` | `cargo build` / `cargo run` | Debug symbols, incremental compilation |
| `release` | `cargo build --release` | LTO, single codegen unit, strip, abort-on-panic |
| `release-debug` | Custom | Release with debug info for profiling |
| `ci` | Custom | Dev profile, no incremental (for CI) |

---

## Running Tests

```bash
# All tests
cargo test --all-features

# Specific test
cargo test test_router_audits_state_changing_or_non_2xx

# With test output
cargo test -- --nocapture

# Integration tests only
cargo test --test '*'
```

### Test Coverage Areas

- **Auth**: Login with valid/invalid credentials, 401 for disabled users, JWT encode/decode, refresh token rotation, logout
- **Authorization**: Role-based access (viewer can't write, operator can't delete), cluster-scoped permissions
- **Audit**: READ-2xx not audited, WRITE/non-2xx audited, pagination, filtering
- **VNC**: Token mint/verify, subject matching, concurrency limiter
- **API Key**: Correct key = authenticated, wrong key = no auth, disabled config
- **TLS**: Server starts with valid cert, errors on missing cert/key, security headers in HTTPS response
- **Health**: /livez returns 200, readiness cluster reachability
- **CORS**: Permissive when empty, restrictive with origins

---

## Benchmarking

Benchmarks use [Criterion.rs](https://github.com/bheisler/criterion.rs):

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench auth_benchmarks
```

### Current Benchmarks

| Benchmark | Metric | Typical Value |
|---|---|---|
| `jwt_encode` | Time to sign an RS256 JWT | ~535 µs |
| `jwt_decode` | Time to verify an RS256 JWT | ~20 µs |
| `bcrypt_hash` | Time to hash a password | ~195 ms |
| `bcrypt_verify` | Time to verify a password | ~194 ms |

---

## CI Gates

Every commit (or PR) should pass:

```bash
# Quick check (run before push)
make lint     # cargo fmt --check + clippy

# Full check
make check-all  # fmt + clippy + test + audit

# Security advisory check
make audit    # cargo audit
make deny     # cargo deny check (licenses + bans + advisories)
```

CI configuration is defined in `.github/workflows/ci.yml`:
1. `fmt` — `cargo fmt --check`
2. `clippy` — `cargo clippy --all-targets --all-features -- -D warnings`
3. `test` — `cargo test --all-features`
4. `audit` — `cargo audit`
5. `bench` — `cargo bench` (comparison only, no fail on regression)

---

## Adding a New API Endpoint

1. **Create the handler module** in `src/api/` (e.g., `my_feature.rs`)
2. **Add the route** in `src/api/mod.rs` — insert into `public`, `protected`, or `cluster_scoped`
3. **Implement the handler** — standard axum handler signature with `State`, `AuthContext`, etc.
4. **Add tests** — unit tests in the handler module + integration tests in the router
5. **Add audit coverage** — ensure write operations are captured by the audit middleware
6. **Document** — update the relevant docs and API coverage
7. **Run CI** — `make check-all` before committing

---

## Adding a New Auth Provider

1. **Add config** to `src/config.rs` (`AuthConfig` struct)
2. **Implement the provider** in `src/auth/` (e.g., `src/auth/ldap.rs`)
3. **Wire it** in `src/main.rs` — initialize from config, add to `AppState`
4. **Add handlers** in `src/api/auth.rs` for login/callback
5. **Route** in `src/api/mod.rs`
6. **Test** with mock server (wiremock or similar)

---

## Security Considerations

- **Never log secrets** — All password/token fields use `Display` redaction or are explicitly excluded from logging
- **Fail closed** — Missing security configuration is a startup error, not a runtime warning
- **Regular audits** — `cargo audit` runs in CI to catch vulnerable dependencies
- **Defense in depth** — Every endpoint is protected by middleware layers (auth → rate limit → audit)
- **No unsafe** — `#![forbid(unsafe_code)]` prevents unsafe Rust patterns
