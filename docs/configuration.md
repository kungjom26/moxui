# Configuration Reference

MoxUI is configured via a YAML file (default: `/etc/moxui/config.yaml`) with environment variable overrides (`MOXUI_*`).

**Precedence** (highest → lowest):
1. Environment variables (`MOXUI_*`)
2. YAML/TOML config file
3. Hardcoded defaults

---

## Top-Level Structure

```yaml
server:
  bind: "0.0.0.0:8080"
  workers: 0
  tls: ~

database:
  path: "/var/lib/moxui/data/moxui.db"
  max_connections: 8
  run_migrations: true

logging:
  level: "info"
  format: "pretty"   # "pretty"|"json"

clusters:
  - name: "homelab"
    url: "https://192.168.1.11:8006"
    username: "root@pam"
    password: "${MOXUI_PROXMOX_HOMELAB_PASSWORD}"
    realm: "pam"
    insecure_skip_verify: true
    ca_cert_pem: ~

auth:
  jwt_issuer: "moxui"
  jwt_audience: "moxui-clients"
  jwt_lifetime_secs: 3600
  jwt_private_key_pem_path: "/etc/moxui/jwt_priv.pem"
  jwt_public_key_pem_path: "/etc/moxui/jwt_pub.pem"
  vnc_token_secret_pem_path: ~
  users: []
  rate_limit:
    requests_per_second: 5
    burst_size: 10
  cors:
    allowed_origins: []
    max_age_secs: 86400
  api_key:
    enabled: false
    key: ~
  webauthn:
    enabled: false
    rp_id: "localhost"
    rp_origin: "http://localhost:8080"
    rp_name: "MoxUI"
  oidc:
    enabled: false
    providers: []

tracing:
  enabled: false
  otlp_endpoint: "http://localhost:4317"
  service_name: "moxui"
```

---

## `server` — HTTP Server

| Field | Type | Default | Description |
|---|---|---|---|
| `bind` | `string` | `"0.0.0.0:8080"` | Address and port to listen on |
| `workers` | `usize` | `0` | Number of tokio worker threads (`0` = num CPUs) |
| `tls` | `TlsConfig \| null` | `null` | TLS configuration. When `null`, plaintext HTTP is used (dev mode with startup warning) |

### `server.tls`

| Field | Type | Description |
|---|---|---|
| `cert_pem_path` | `string` | Path to PEM-encoded certificate (full chain: leaf + intermediates) |
| `key_pem_path` | `string` | Path to PEM-encoded private key (unencrypted PKCS#8 or RSA PEM) |

When TLS is configured, the server enforces **HTTPS-only** — plaintext HTTP connections are refused at the TLS layer.

---

## `database` — SQLite Database

| Field | Type | Default | Description |
|---|---|---|---|
| `path` | `string` | `"moxui.db"` | SQLite database file path |
| `max_connections` | `u32` | `8` | Connection pool size (r2d2 pool) |
| `run_migrations` | `bool` | `true` | Run schema migrations on startup |

The audit log is stored at `{path}.audit` (e.g., `moxui.db.audit`).

---

## `logging` — Logging

| Field | Type | Default | Description |
|---|---|---|---|
| `level` | `string` | `"info"` | Log level: `trace`, `debug`, `info`, `warn`, `error` |
| `format` | `string` | `"pretty"` | Output format: `pretty` (colored, human-readable) or `json` (structured, for production) |

In release mode, the default format is `json`. In debug mode, the default is `pretty`.

---

## `clusters[]` — Proxmox Cluster Connections

Each entry configures one Proxmox VE cluster:

| Field | Type | Default | Description |
|---|---|---|---|
| `name` | `string` | **required** | Unique cluster identifier (used in routes and user permissions) |
| `url` | `string` | **required** | Proxmox API URL (e.g., `https://pve11.local:8006`) |
| `username` | `string` | **required** | Proxmox API user (e.g., `root@pam`, `proxui@pve`) |
| `password` | `SecretString` | **required** | Proxmox password. Use env var references (`${VAR}`) in production |
| `realm` | `string` | `"pam"` | Auth realm: `pam`, `pve`, or `openid` |
| `insecure_skip_verify` | `bool` | `false` | Skip TLS certificate verification. ⚠️ Use only for self-signed certs in development |
| `ca_cert_pem` | `string \| null` | `null` | PEM-encoded CA certificate for TLS verification (alternative to `insecure_skip_verify`) |

**Security:** Passwords are wrapped in `SecretString` (from the `secrecy` crate):
- Never printed in `Debug` output
- Zeroed on drop
- Require explicit `.expose_secret()` calls

---

## `auth` — Authentication

### `auth.jwt` — JWT Settings

| Field | Type | Default | Description |
|---|---|---|---|
| `jwt_issuer` | `string` | `"moxui"` | JWT `iss` claim |
| `jwt_audience` | `string` | `"moxui-clients"` | JWT `aud` claim |
| `jwt_lifetime_secs` | `i64` | `3600` | Token lifetime in seconds (default 1 hour) |
| `jwt_private_key_pem_path` | `string \| null` | `null` | **Required.** Path to RS256 private key in PEM format |
| `jwt_public_key_pem_path` | `string \| null` | `null` | **Required.** Path to RS256 public key in PEM format |

If JWT keys are missing, the server **refuses to start** (fail-closed).

### `auth.users[]` — Seeded Users

| Field | Type | Default | Description |
|---|---|---|---|
| `id` | `string` | **required** | Unique user identifier |
| `username` | `string` | **required** | Login username |
| `display_name` | `string` | `""` | Human-readable display name |
| `email` | `string \| null` | `null` | Email address |
| `role` | `string` | **required** | RBAC role: `admin`, `operator`, or `viewer` |
| `password_hash` | `string \| null` | `null` | bcrypt hash of password (preferred for production) |
| `password` | `string \| null` | `null` | Plaintext password (dev/first-boot only — ignored if `password_hash` is set) |
| `enabled` | `bool` | `true` | Whether the account is active |
| `allowed_clusters` | `string[]` | `[]` | Cluster names this user can access. Empty = all clusters |

**Role Hierarchy:**
- `admin` — Full access: read, write, delete, manage users, view audit logs
- `operator` — Read + write VMs/LXCs (start, stop, reboot, VNC console). Cannot delete or manage users
- `viewer` — Read-only (view VMs, LXCs, storage, networks, dashboard)

### `auth.vnc_token_secret_pem_path`

Path to a file containing the HMAC secret for VNC console tokens. The file is read as raw bytes (not parsed as PEM). Minimum 32 bytes recommended.

When absent, VNC endpoints return 404 (disabled by design).

### `auth.rate_limit` — Rate Limiting

| Field | Type | Default | Description |
|---|---|---|---|
| `requests_per_second` | `u64` | `5` | Max requests per second per IP |
| `burst_size` | `u32` | `10` | Burst allowance before rate limiting applies |

Uses `tower-governor` with GCRA algorithm.

### `auth.cors` — CORS

| Field | Type | Default | Description |
|---|---|---|---|
| `allowed_origins` | `string[]` | `[]` | Allowed CORS origins. Empty = `*` (permissive) |
| `max_age_secs` | `u64` | `86400` | Preflight cache max-age (24h) |

### `auth.api_key` — API Key Authentication

| Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | `bool` | `false` | Enable API key authentication |
| `key` | `string \| null` | `null` | The shared API key secret |

API keys provide an alternative to JWT Bearer tokens. Clients send `X-API-Key: <key>` in request headers. API keys are flat shared secrets — they don't identify individual users. Best for automation/CI pipelines.

### `auth.webauthn` — WebAuthn / Passkey

| Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | `bool` | `false` | Enable WebAuthn passkey authentication |
| `rp_id` | `string` | `"localhost"` | Relying Party ID (domain, e.g. `moxui.example.com`) |
| `rp_origin` | `string` | `"http://localhost:8080"` | Relying Party origin (must match browser URL) |
| `rp_name` | `string` | `"MoxUI"` | Relying Party display name shown in browser prompt |

### `auth.oidc` — OIDC / OAuth2 SSO

| Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | `bool` | `false` | Enable OIDC/OAuth2 SSO |
| `providers[]` | `OidcProvider[]` | `[]` | List of SSO providers |

Supported providers: `google` (full OpenID Connect), `github` (OAuth2 only).

#### `auth.oidc.providers[]`

| Field | Type | Description |
|---|---|---|
| `name` | `string` | Provider name: `"google"` or `"github"` |
| `client_id` | `string` | OAuth2 client ID from the provider |
| `client_secret` | `string` | OAuth2 client secret |
| `redirect_url` | `string` | Registered redirect URL (e.g., `http://localhost:8080/api/v1/auth/oidc/callback`) |

---

## `tracing` — OpenTelemetry

| Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | `bool` | `false` | Enable OTLP tracing export |
| `otlp_endpoint` | `string` | `"http://localhost:4317"` | OTLP gRPC endpoint |
| `service_name` | `string` | `"moxui"` | Service name reported to tracing backend |

When enabled, spans are exported via OTLP gRPC using batch exporter. The init is best-effort — failures log a warning but don't prevent startup.

---

## Environment Variable Reference

All YAML fields can be overridden with `MOXUI_*` environment variables. Use `__` (double underscore) for nested keys:

```bash
# Flat fields
MOXUI_SERVER__BIND="0.0.0.0:9090"
MOXUI_LOGGING__LEVEL="debug"
MOXUI_DATABASE__PATH="/custom/path/moxui.db"

# Nested fields
MOXUI_AUTH__JWT_ISSUER="my-moxui"
MOXUI_AUTH__RATE_LIMIT__REQUESTS_PER_SECOND="10"
MOXUI_AUTH__CORS__ALLOWED_ORIGINS='["https://app.example.com"]'

# Array-style access (figment convention — some arrays may not work via env)
# For array configs (clusters, users, providers), use the YAML file
```

**CLI flags** also available:

```bash
moxui --help
# Usage: moxui [OPTIONS]
#
# Options:
#   -c, --config <CONFIG>          Config file path [default: /etc/moxui/config.yaml]
#   -l, --log-level <LOG_LEVEL>    Log level [env: MOXUI_LOG_LEVEL] [default: info]
#   -v, --verbose                  Enable verbose (debug) output
#   -h, --help                     Print help
#   -V, --version                  Print version
```
