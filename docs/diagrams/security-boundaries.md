# Security Boundaries & Trust Zones

> **Last updated:** 2026-06-20

---

## 1. Trust Zones

```
┌─────────────────────────────────────────────────────────────────┐
│  Zone 0: INTERNET (UNTRUSTED)                                  │
│  ================                                               │
│  • Random users                                                 │
│  • Attackers                                                    │
│  • Bots, scanners                                               │
│  Trust: NONE                                                    │
└─────────────────────────────┬───────────────────────────────────┘
                              │ HTTPS (TLS 1.3)
                              │ Must authenticate to enter Zone 1
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Zone 1: PUBLIC EDGE (Caddy / Traefik)                         │
│  ========================                                       │
│  • TLS termination                                              │
│  • Rate limiting (5 login/min, 100 API/min)                    │
│  • Security headers injection                                  │
│  • Optional: WAF rules                                         │
│  • HTTP/3                                                      │
│  Trust: LOW (semi-trusted, exposed to internet)                │
└─────────────────────────────┬───────────────────────────────────┘
                              │ HTTP (internal, no TLS termination needed)
                              │ Source IP allowed (firewall)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Zone 2: MOXUI (TRUSTED APP)                                    │
│  ======================                                         │
│  • JWT verification                                             │
│  • RBAC enforcement                                             │
│  • Audit logging                                                │
│  • Input validation                                            │
│  • Rate limiting (per-user)                                    │
│  • Secret decryption                                            │
│  Trust: MEDIUM (trusted to enforce security, but not full     │
│         trust — assume container could be compromised)         │
└─────────────────────────────┬───────────────────────────────────┘
                              │ HTTPS (private VLAN/VPN)
                              │ Proxmox CA cert required
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Zone 3: PROXMOX API (TRUSTED NETWORK)                         │
│  ============================                                   │
│  • Ticket auth (root@pam)                                      │
│  • Self-signed cert (CA pinned)                                │
│  • Proxmox native rate limiting                                │
│  Trust: MEDIUM (trusted to provide correct data, but apply     │
│         principle of least privilege — limit what MoxUI can do)│
└─────────────────────────────┬───────────────────────────────────┘
                              │ Local network (intra-cluster)
                              │ Full network access within cluster
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Zone 4: PROXMOX NODES (FULL TRUST)                             │
│  ============================                                   │
│  • KVM/QEMU hypervisor                                         │
│  • VM storage                                                   │
│  • VM network bridges                                          │
│  • Console access                                              │
│  Trust: FULL (MoxUI has admin access to VMs)                   │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. Authentication Chain

### Multi-layer defense

```
Zone 0 → Zone 1:  TLS 1.3 (encryption in transit)
Zone 1 → Zone 2:  HTTP (trusted internal)
Zone 2 internal:   JWT (RS256, 15min) — every request authenticated
Zone 2 → Zone 3:  HTTPS + ticket auth (Proxmox session)
Zone 3 → Zone 4:  Internal (no auth needed within cluster)
```

### Required credentials per zone

| Zone | Required credential | TTL | Storage |
|---|---|---|---|
| Zone 0 → 1 | None (public) | — | — |
| Zone 1 → 2 | JWT (Bearer token) | 15 min | Client (memory) |
| Zone 1 → 2 | Refresh token (cookie) | 7 days | DB (SHA-256 hashed) |
| Zone 2 → 3 | Proxmox ticket | 2 hours | Memory (per ProxmoxClient) |
| Zone 3 → 4 | (root within cluster) | — | — |

---

## 3. Data Classification

### Sensitivity levels

| Level | Examples | Protection |
|---|---|---|
| **CRITICAL** | User passwords, JWT private key, cluster passwords, refresh tokens, API keys | bcrypt / encrypted at rest / hashed in DB |
| **HIGH** | Audit log, user PII (email, name), backup files | Encrypted at rest (optional), TLS in transit, access controlled |
| **MEDIUM** | VM metadata (names, IPs, tags), cluster config | TLS in transit, access controlled |
| **LOW** | Public docs, version numbers | No special protection |

### Encryption at rest

| Data | Encryption | Key |
|---|---|---|
| User passwords | bcrypt (one-way) | N/A |
| Refresh tokens | SHA-256 hash | N/A |
| API keys | SHA-256 hash | N/A |
| Cluster passwords | ChaCha20-Poly1305 (symmetric) | `MOXUI_MASTER_KEY` env var |
| LDAP bind passwords | ChaCha20-Poly1305 | `MOXUI_MASTER_KEY` |
| TOTP secrets | ChaCha20-Poly1305 (optional) | `MOXUI_MASTER_KEY` |
| Audit log | (none — append-only for integrity) | — |

### Encryption in transit

| Path | Encryption |
|---|---|
| User browser → Caddy | TLS 1.3 |
| Caddy → MoxUI | HTTP (trusted internal) |
| MoxUI → Proxmox API | TLS (with CA pinning for self-signed) |
| VNC console (browser ↔ Proxmox) | TLS via WebSocket → Proxmox native |

---

## 4. Authorization Boundaries

### Role hierarchy

```
admin (highest)
  │
  ├── operator
  │     │
  │     └── viewer (lowest)
```

### Permission matrix

| Action | admin | operator | viewer |
|---|---|---|---|
| View VMs/LXC | ✅ | ✅ | ✅ |
| Start/stop/reboot VM | ✅ | ✅ | ❌ |
| Shutdown VM | ✅ | ✅ | ❌ |
| Delete VM | ✅ | ❌ | ❌ |
| Create VM | ✅ | ❌ | ❌ |
| Snapshot | ✅ | ✅ | ❌ |
| Backup | ✅ | ✅ | ❌ |
| View console | ✅ | ✅ | ❌ |
| View stats | ✅ | ✅ | ✅ |
| Cluster config | ✅ | ❌ | ❌ |
| User management | ✅ | ❌ | ❌ |
| View audit log | ✅ | ❌ | ❌ |
| System settings | ✅ | ❌ | ❌ |

### Per-cluster overrides

```
user.role = 'admin' (global)
            +
cluster_permissions row: { user: X, cluster: Y, permission: 'viewer' }
            =
Effective permission on cluster Y: 'viewer' (specificity wins)

user.role = 'viewer' (global)
            +
cluster_permissions row: { user: X, cluster: Y, permission: 'denied' }
            =
Effective permission on cluster Y: 'denied' (explicit block)
```

---

## 5. Attack Surface Analysis

### External attack surface (Zone 0/1)

| Endpoint | Method | Risk | Mitigation |
|---|---|---|---|
| `/` (login page) | GET | XSS, CSRF | CSP, JWT (not cookie) |
| `/api/v1/auth/login` | POST | Brute force | Rate limit (5/min), lockout, 2FA |
| `/api/v1/auth/refresh` | POST | Token replay | Rotation, hashed storage |
| `/api/v1/vms/...` | GET | IDOR | RBAC check, cluster filter |
| `/api/v1/vms/{id}/start` | POST | Unauthorized action | RBAC check |
| `/api/v1/console/...` | WS | Connection hijack | JWT in subprotocol, expiry |
| `/.well-known/...` | GET | Information leak | (no info leak) |
| `/metrics` | GET | Information leak | (network-restricted) |

### Internal attack surface (Zone 2/3)

| Vector | Risk | Mitigation |
|---|---|---|
| Compromised MoxUI container → Proxmox | High | Use dedicated service account (future), limit to required scopes |
| SQL injection via API | Critical | Parameterized queries only, no string concat |
| Command injection via user input | Critical | No shell exec, validate input |
| Path traversal | High | Canonicalize paths, prefix check |
| XSS via dashboard | Medium | Alpine.js x-text (escape), CSP |
| Container escape | High | Non-root, read-only rootfs, cap-drop ALL |

---

## 6. Defense in Depth Layers

```
Layer 1: Network
  ↓ TLS 1.3, private VLAN, firewall rules
Layer 2: Reverse proxy (Caddy/Traefik)
  ↓ Rate limiting, security headers, WAF (optional)
Layer 3: Container
  ↓ Non-root, read-only rootfs, cap-drop ALL, distroless
Layer 4: Application — Auth
  ↓ JWT verification, RBAC check, rate limit
Layer 5: Application — Validation
  ↓ Input validation, length limits, charset whitelist
Layer 6: Application — Database
  ↓ Parameterized queries, audit log, transactions
Layer 7: External API
  ↓ Circuit breaker, retry with backoff, CA cert pinning
Layer 8: OS / Proxmox
  ↓ Latest security patches, firewall, least privilege
```

**Each layer fails independently — no single failure compromises system**

---

## 7. Secret Management

### Secret classification

| Secret | Criticality | Storage | Rotation |
|---|---|---|---|
| `MOXUI_JWT_PRIVATE_KEY` | CRITICAL | env var | Annually |
| `MOXUI_MASTER_KEY` (for encryption) | CRITICAL | env var | Quarterly |
| `MOXUI_PROXMOX_HOMELAB_PASSWORD` | CRITICAL | env var + DB (encrypted) | Quarterly |
| `MOXUI_ADMIN_PASSWORD` | CRITICAL | DB (bcrypt) | On compromise |
| User passwords | HIGH (per user) | DB (bcrypt cost 12) | User-initiated |
| Refresh tokens | HIGH (per user) | DB (SHA-256 hash) | Per use (rotation) |
| API keys | HIGH (per user) | DB (SHA-256 hash) | User-initiated |
| OIDC client secrets | HIGH | env var | Per provider |

### Secret rotation

```
JWT key rotation:
  1. Generate new keypair (RS256)
  2. Deploy with BOTH old + new public keys (verify accepts both)
  3. Wait for old tokens to expire (15 min for access, 7 days for refresh)
  4. Remove old public key
  5. Force refresh token rotation

Master key rotation (ChaCha20-Poly1305):
  1. Generate new master key
  2. Decrypt all encrypted fields with old key
  3. Re-encrypt with new key
  4. Update env var
  5. Restart MoxUI
  6. Verify decryption works
```

### Secret leak response

```
1. Detect leak (audit log anomaly, secret scanner, manual report)
2. Rotate affected secret IMMEDIATELY
3. Revoke all sessions for affected users (if applicable)
4. Audit log review for unauthorized access
5. Notify affected users (if PII involved)
6. Post-mortem: how did it leak? How to prevent?
7. Update secret management process
```

---

## 8. Network Segmentation

### Recommended topology

```
┌────────────────────────────────────────────────────────────┐
│  Public subnet (10.0.0.0/24)                              │
│  - Internet gateway                                         │
│  - Caddy / reverse proxy                                    │
└────────────────┬───────────────────────────────────────────┘
                 │ firewall rule: allow 443 from internet
                 │
┌────────────────▼───────────────────────────────────────────┐
│  DMZ subnet (10.0.1.0/24)                                  │
│  - MoxUI containers (2+)                                    │
│  - Caddy (if not in public)                                │
└────────────────┬───────────────────────────────────────────┘
                 │ firewall rule: allow 8006 to Proxmox subnet only
                 │
┌────────────────▼───────────────────────────────────────────┐
│  Proxmox management subnet (10.0.10.0/24)                 │
│  - Proxmox nodes (pve11/12/13)                             │
│  - Proxmox cluster network                                 │
└────────────────┬───────────────────────────────────────────┘
                 │ (no access from MoxUI — VMs isolated)
                 │
┌────────────────▼───────────────────────────────────────────┐
│  VM traffic subnet (10.0.20.0/24)                          │
│  - VM internal network                                       │
│  - VM external network (if routed)                         │
└────────────────────────────────────────────────────────────┘
```

### Firewall rules

| Source | Destination | Port | Protocol | Action |
|---|---|---|---|---|
| Internet | Caddy | 443 | TCP | ALLOW |
| Caddy | MoxUI | 8080 | TCP | ALLOW |
| MoxUI | Proxmox | 8006 | TCP | ALLOW |
| MoxUI | DNS | 53 | UDP | ALLOW (for OIDC discovery) |
| Proxmox | Internet | * | * | DENY (no outbound from Proxmox) |
| MoxUI | Internet | * | * | DENY (no outbound except DNS + Proxmox) |

---

## 9. Container Security

### Hardening checklist

```
✓ Non-root user (UID 1000)
✓ Read-only rootfs (--read-only flag)
✓ tmpfs for /tmp (--tmpfs /tmp:rw,noexec,nosuid)
✓ Cap-drop ALL (--cap-drop=ALL)
✓ Cap-add NET_BIND_SERVICE only (if needed)
✓ No new privileges (--security-opt=no-new-privileges:true)
✓ Resource limits (--memory=512m, --cpus=2)
✓ Distroless base image (debian-slim)
✓ Image scanning (trivy, grype)
✓ Image signing (cosign)
✓ SBOM generated (syft)
✓ No secrets baked in layers
✓ Health check configured
✓ Restart policy (unless-stopped)
```

### Runtime detection

```
/proc/1/cgroup    # check escape attempts
/proc/1/mounts    # check mount points
/proc/self/status # check capabilities
getcap -r / 2>/dev/null  # check file capabilities
```

If any anomalies → log + alert + stop container

---

## 10. Security Headers

### Every response

```
Strict-Transport-Security: max-age=63072000; includeSubDomains; preload
Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; connect-src 'self' wss:; object-src 'none'; frame-ancestors 'none'; form-action 'self'; base-uri 'self'
X-Frame-Options: DENY
X-Content-Type-Options: nosniff
Referrer-Policy: strict-origin-when-cross-origin
Permissions-Policy: camera=(), microphone=(), geolocation=(), payment=(), usb=(), magnetometer=(), gyroscope=(), accelerometer=()
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
Cross-Origin-Resource-Policy: same-origin
```

### CSP details

- `default-src 'self'` — only same-origin by default
- `script-src 'self'` — no inline scripts (Alpine.js bundled)
- `style-src 'self' 'unsafe-inline'` — Tailwind + dynamic styles (minimal)
- `img-src 'self' data:` — local + base64 (for icons)
- `connect-src 'self' wss:` — REST + WebSocket (for VNC console)
- `object-src 'none'` — no Flash, no Java
- `frame-ancestors 'none'` — no embedding in iframe
- `form-action 'self'` — forms only post to same origin
- `base-uri 'self'` — no <base> tag hijacking

---

## 11. Audit Log Immutability

### Trigger enforcement

```sql
CREATE TRIGGER audit_log_no_update
    BEFORE UPDATE ON audit_log
BEGIN
    SELECT RAISE(ABORT, 'audit_log is append-only');
END;

CREATE TRIGGER audit_log_no_delete
    BEFORE DELETE ON audit_log
BEGIN
    SELECT RAISE(ABORT, 'audit_log is append-only — use retention job instead');
END;
```

### Tamper detection (future)

```
- Sign each entry with HMAC-SHA256 (master key)
- Store HMAC in audit_log_entry.hmac column
- Daily job verifies chain (each entry's HMAC includes prev entry's HMAC)
- Alert if chain broken
```

---

**See also:**
- [`ARCHITECTURE.md`](../ARCHITECTURE.md) — high-level
- [`../DATA_MODEL.md`](../DATA_MODEL.md) — DB schema
- [`moxui-security-audit`](../../../../.hermes/profiles/moxui-reviewer/skills/moxui-security-audit/SKILL.md) — audit checklist (LOCAL-ONLY)