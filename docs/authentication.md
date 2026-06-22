# Authentication & Authorization

MoxUI provides a comprehensive authentication system supporting multiple identity providers, multi-factor authentication, and role-based access control (RBAC).

---

## Architecture

```
                    ┌──────────────────────┐
                    │    Browser / Client   │
                    └──────┬───────────────┘
                           │ Request (Bearer JWT / X-API-Key)
                           ▼
              ┌────────────────────────┐
              │  Security Middleware   │
              │  ┌──────────────────┐  │
              │  │ Rate Limiter     │  │  tower-governor
              │  ├──────────────────┤  │
              │  │ CORS             │  │  tower-http
              │  ├──────────────────┤  │
              │  │ API Key Check    │  │  X-API-Key header
              │  ├──────────────────┤  │
              │  │ JWT Validation   │  │  RS256 Bearer token
              │  ├──────────────────┤  │
              │  │ RBAC Check       │  │  admin/operator/viewer
              │  └──────────────────┘  │
              └────────────────────────┘
```

---

## 1. Local Login (Username + Password)

### Login Flow

```
POST /api/v1/auth/login
Content-Type: application/json

{
    "username": "admin",
    "password": "correct-horse-battery-staple"
}
```

**Success response (no 2FA):**
```json
{
    "token": "eyJhbGciOiJSUzI1NiIs...",
    "expires_in": 3600,
    "token_type": "Bearer",
    "user": {
        "id": "u-admin",
        "username": "admin",
        "display_name": "Admin User",
        "email": "admin@example.com",
        "role": "admin"
    },
    "refresh_token": "a1b2c3d4e5f6..."
}
```

**If 2FA is enabled:**
```json
{
    "status": "2fa_required",
    "preauth_token": "preauth-xxxxx"
}
```

Then complete with:
```
POST /api/v1/auth/2fa/complete
Content-Type: application/json

{
    "preauth_token": "preauth-xxxxx",
    "code": "123456"
}
```

### Password Storage

- Passwords are bcrypt-hashed (cost factor 12)
- Plaintext passwords are never stored — only bcrypt hashes
- In production configs, use `password_hash` (not `password`)
- Login attempts are rate-limited (5 req/sec per IP by default)

### Token Refresh

```
POST /api/v1/auth/refresh
Content-Type: application/json

{
    "refresh_token": "a1b2c3d4e5f6..."
}
```

Response returns a new JWT + new refresh token (rotation). Old refresh token is revoked.

**Security properties:**
- Refresh tokens are 32-byte random values (256-bit entropy)
- Stored as SHA-256 hashes only (plaintext never persisted)
- Token rotation: each refresh invalidates the previous token
- Family revocation: replaying a revoked token revokes ALL tokens for that user
- 7-day TTL on refresh tokens
- Logout revokes the refresh token (always returns 200)

### Logout

```
POST /api/v1/auth/logout
Content-Type: application/json

{
    "refresh_token": "a1b2c3d4e5f6..."
}
```
Always returns `200 {"ok": true}` to avoid leaking token validity.

---

## 2. Two-Factor Authentication (TOTP)

MoxUI supports RFC 6238 TOTP (Time-based One-Time Password) as a second factor.

### Setup

```
POST /api/v1/auth/2fa/setup
Authorization: Bearer <jwt>
```

**Response:**
```json
{
    "secret": "JBSWY3DPEHPK3PXP",
    "url": "otpauth://totp/MoxUI:admin?secret=...&issuer=MoxUI",
    "backup_codes": [
        "ABCD-EFGH-IJKL-MNOP",
        "QRST-UVWX-YZ12-3456",
        ...
    ]
}
```

Scan the QR URL with an authenticator app (Google Authenticator, Authy, 1Password, etc.), then verify:

```
POST /api/v1/auth/2fa/verify
Authorization: Bearer <jwt>
Content-Type: application/json

{
    "secret": "JBSWY3DPEHPK3PXP",
    "code": "123456"
}
```

### Disable

```
POST /api/v1/auth/2fa/disable
Authorization: Bearer <jwt>
Content-Type: application/json

{
    "password": "current-password"
}
```

### Backup Codes

- 8 backup codes generated on setup (8-character alphanumeric, hyphen-separated)
- Each backup code can be used **once**
- Accepted via the same `POST /api/v1/auth/2fa/complete` endpoint using the `code` field
- Store backup codes securely — they're the only way to recover if you lose your authenticator device

---

## 3. WebAuthn / Passkeys

MoxUI supports WebAuthn (FIDO2) for passwordless authentication using platform authenticators (Touch ID, Windows Hello, YubiKey, etc.).

### Registration

```
POST /api/v1/auth/webauthn/register/start
Authorization: Bearer <jwt>
```

**Response:** Public key credential creation options (pass to `navigator.credentials.create()`)

```
POST /api/v1/auth/webauthn/register/complete
Authorization: Bearer <jwt>
Content-Type: application/json

{
    "credential": { ... }  // From navigator.credentials.create()
}
```

### Authentication

```
POST /api/v1/auth/webauthn/login/start
Content-Type: application/json

{
    "username": "admin"
}
```

**Response:** Assertion options (pass to `navigator.credentials.get()`)

```
POST /api/v1/auth/webauthn/login/complete
Content-Type: application/json

{
    "username": "admin",
    "credential": { ... }  // From navigator.credentials.get()
}
```

**Response:** JWT + refresh token (same shape as password login)

### Configuration

```yaml
auth:
  webauthn:
    enabled: true
    rp_id: "moxui.example.com"       # Domain
    rp_origin: "https://moxui.example.com"  # Must match browser URL
    rp_name: "MoxUI"                 # Display name in browser prompt
```

---

## 4. OIDC / OAuth2 SSO

MoxUI supports single sign-on via Google (OpenID Connect) and GitHub (OAuth2).

### Configuration

```yaml
auth:
  oidc:
    enabled: true
    providers:
      - name: "google"
        client_id: "1234567890-xxxxx.apps.googleusercontent.com"
        client_secret: "${MOXUI_OIDC_GOOGLE_SECRET}"
        redirect_url: "https://moxui.example.com/api/v1/auth/oidc/callback"
      - name: "github"
        client_id: "Iv1.xxxxxxxxxxxx"
        client_secret: "${MOXUI_OIDC_GITHUB_SECRET}"
        redirect_url: "https://moxui.example.com/api/v1/auth/oidc/callback"
```

### Flow

1. **Initiate:** `POST /api/v1/auth/oidc/login` with provider name → returns auth URL
2. **Redirect:** User is redirected to the provider (Google/GitHub)
3. **Callback:** Provider redirects back with authorization code
4. **Complete:** `POST /api/v1/auth/oidc/callback` exchanges code → returns JWT

**Security:**
- PKCE (Proof Key for Code Exchange) for all flows
- CSRF state tokens validated on callback
- 5-minute TTL on pending authorization states
- Nonce validation for Google OIDC ID tokens

---

## 5. API Key Authentication

For automation and CI/CD pipelines, MoxUI supports API key-based authentication.

```yaml
auth:
  api_key:
    enabled: true
    key: "${MOXUI_API_KEY}"
```

**Usage:**
```bash
curl -H "X-API-Key: your-api-key" http://localhost:8080/api/v1/vms
```

**Characteristics:**
- Flat shared secret (not user-specific)
- Best suited for read-only monitoring/automation
- Can coexist with JWT Bearer auth
- The API key layer marks the request as authenticated via a request extension, which downstream JWT middleware also accepts

---

## 6. Role-Based Access Control (RBAC)

### Roles

| Role | Privileges | VMs | LXCs | Storage | Networks | Audit | Users | VNC |
|---|---|---|---|---|---|---|---|---|
| **admin** | Full access | R/W | R/W | R | R | R | R/W | ✓ |
| **operator** | Operational | R/W | R/W | R | R | R | - | ✓ |
| **viewer** | Read-only | R | R | R | R | - | - | - |

### Per-Cluster Permissions

Users can be restricted to specific clusters:

```yaml
auth:
  users:
    - id: "u-ops"
      username: "operator1"
      role: "operator"
      allowed_clusters:
        - "homelab"        # Can only see/operate on "homelab"
        - "staging"        # And "staging"
    - id: "u-admin"
      username: "admin1"
      role: "admin"
      allowed_clusters: []  # Empty = all clusters
```

### Auth Middleware Stack

1. **Rate Limiter** (tower-governor) — global rate limit per IP
2. **CORS** (tower-http) — origin validation
3. **API Key Check** — marks request if `X-API-Key` matches
4. **JWT Validation** — validates Bearer token, extracts claims
5. **Cluster Access** — checks user's `allowed_clusters` against `:cluster` path param
6. **Role Check** — endpoint-level: operator+ for VM writes, admin+ for user management

---

## API Endpoints Summary

### Public (No Auth)

| Method | Path | Description |
|---|---|---|
| GET | `/health` | Health check |
| GET | `/livez` | Liveness probe |
| GET | `/readyz` | Readiness probe |
| GET | `/metrics` | Prometheus metrics |
| POST | `/api/v1/auth/login` | Login (username + password) |
| POST | `/api/v1/auth/refresh` | Refresh token |
| POST | `/api/v1/auth/logout` | Logout |
| POST | `/api/v1/auth/2fa/complete` | Complete 2FA login |
| POST | `/api/v1/auth/oidc/login` | Start OIDC login |
| POST | `/api/v1/auth/oidc/callback` | Complete OIDC login |
| POST | `/api/v1/auth/webauthn/login/start` | Start passkey login |
| POST | `/api/v1/auth/webauthn/login/complete` | Complete passkey login |

### Authenticated (Bearer Token Required)

| Method | Path | Min Role |
|---|---|---|
| GET | `/api/v1/auth/me` | viewer |
| GET | `/api/v1/dashboard` | viewer |
| GET | `/api/v1/audit` | viewer |
| GET | `/api/v1/vms` | viewer |
| GET | `/api/v1/vms/:cluster/:vmid` | viewer |
| POST | `/api/v1/vms/:cluster/:node/:vmid/:action` | operator |
| GET | `/api/v1/lxcs` | viewer |
| GET | `/api/v1/lxcs/:cluster/:node/:vmid` | viewer |
| GET | `/api/v1/storages` | viewer |
| GET | `/api/v1/storages/:cluster/:node/:storage/content` | viewer |
| GET | `/api/v1/networks` | viewer |
| GET | `/api/v1/networks/:cluster/:node` | viewer |
| GET | `/api/v1/tasks/:cluster/:node/:upid` | viewer |
| POST | `/api/v1/vms/:cluster/:node/:vmid/vnc/ticket` | operator |
| GET | `/api/v1/vms/:cluster/:node/:vmid/vnc/ws` | operator |
| POST | `/api/v1/auth/2fa/setup` | viewer |
| POST | `/api/v1/auth/2fa/verify` | viewer |
| POST | `/api/v1/auth/2fa/disable` | viewer |
| POST | `/api/v1/auth/webauthn/register/start` | viewer |
| POST | `/api/v1/auth/webauthn/register/complete` | viewer |
