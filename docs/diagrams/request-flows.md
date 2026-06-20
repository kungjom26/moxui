# Detailed Request Flows

> **Last updated:** 2026-06-20

---

## 1. Login Flow (Full Details)

### Phase 1: Username + Password

```
┌────────┐         ┌─────────┐         ┌──────────┐         ┌─────────┐
│ Client │         │ MoxUI   │         │ DB       │         │ AuditLog│
└───┬────┘         └────┬────┘         └────┬─────┘         └────┬────┘
    │                   │                  │                    │
    │ 1. POST /login    │                  │                    │
    │ {user, pass}      │                  │                    │
    │──────────────────→│                  │                    │
    │                   │ 2. SELECT user   │                    │
    │                   │─────────────────→│                    │
    │                   │                  │                    │
    │                   │ 3. User row      │                    │
    │                   │←─────────────────│                    │
    │                   │                  │                    │
    │                   │ 4. Check is_active, is_locked, locked_until│
    │                   │                  │                    │
    │                   │ 5. Verify bcrypt │                    │
    │                   │   (spawn_block)  │                    │
    │                   │                  │                    │
    │                   ├─── INVALID ────→│                    │
    │                   │ 6. Increment failed_login_count           │
    │                   │──────────────────→                   │
    │                   │                  │                    │
    │                   │ 7. If count >= 5: lock account         │
    │                   │──────────────────→                   │
    │                   │                  │                    │
    │                   │ 8. INSERT audit_log (result: failure)  │
    │                   │──────────────────────────────────────→│
    │                   │                  │                    │
    │                   │ 9. Return 401 Unauthorized              │
    │←──────────────────│                  │                    │
    │                   │                  │                    │
    │                   ├─── VALID ──────→│                    │
    │                   │ 10. Reset failed_login_count            │
    │                   │──────────────────→                   │
    │                   │                  │                    │
    │                   │ 11. Update last_login_at, last_login_ip│
    │                   │──────────────────→                   │
    │                   │                  │                    │
    │                   │ 12. Check if 2FA required             │
    │                   │     (admin → always, else if totp_secrets│
    │                   │      row exists)                      │
    │                   │                  │                    │
    │                   ├─── NO 2FA ───────────────────────────→
    │                   │ 13. Issue JWT (15min)                │
    │                   │ 14. Issue refresh token (7d)          │
    │                   │ 15. INSERT refresh_tokens (hashed)   │
    │                   │──────────────────→                   │
    │                   │ 16. INSERT audit_log (result: success)│
    │                   │──────────────────────────────────────→│
    │                   │ 17. Set-Cookie: JWT, refresh         │
    │                   │ 18. 200 OK + {token, refresh}        │
    │←──────────────────│                  │                    │
    │                   │                  │                    │
    │                   ├─── 2FA REQUIRED ─────────────────→
    │                   │ 19. INSERT audit_log (result: partial)│
    │                   │──────────────────────────────────────→│
    │                   │ 20. Return 200 OK + {require_2fa: true}│
    │                   │ 21. Set temporary session token (5min) │
    │←──────────────────│                  │                    │
```

### Phase 2: 2FA Verification

```
┌────────┐         ┌─────────┐         ┌──────────┐         ┌─────────┐
│ Client │         │ MoxUI   │         │ DB       │         │ TOTP/WebAuthn│
└───┬────┘         └────┬────┘         └────┬─────┘         └────┬────┘
    │                   │                  │                    │
    │ 1. POST /2fa/verify                  │                    │
    │ {temp_token, code}│                  │                    │
    │──────────────────→│                  │                    │
    │                   │ 2. Verify temp_token (5min TTL)      │
    │                   │                  │                    │
    │                   │ 3. SELECT totp_secrets OR webauthn_creds│
    │                   │──────────────────→│                    │
    │                   │                  │                    │
    │                   ├─── TOTP ─────────────────────────────→
    │                   │ 4. Verify code (totp-rs, ±30s window)│
    │                   │ 5. Update last_used_at, reset counter  │
    │                   │──────────────────→                   │
    │                   │                  │                    │
    │                   ├─── WebAuthn ─────────────────────────→
    │                   │ 6. Verify signature (webauthn-rs)    │
    │                   │ 7. Increment counter (replay check)   │
    │                   │──────────────────→                   │
    │                   │                  │                    │
    │                   ├─── INVALID ────────────────────────→
    │                   │ 8. Return 401                       │
    │                   │ 9. Audit log (failure)               │
    │                   │ 10. If 3+ fails: lock account        │
    │                   │──────────────────→                   │
    │←──────────────────│                  │                    │
    │                   │                  │                    │
    │                   ├─── VALID ─────────────────────────→
    │                   │ 11. Issue JWT (15min)               │
    │                   │ 12. Issue refresh token (7d)         │
    │                   │ 13. INSERT refresh_tokens (hashed)  │
    │                   │──────────────────→                   │
    │                   │ 14. Audit log (success)              │
    │                   │ 15. Set-Cookie                       │
    │                   │ 16. 200 OK + {token, refresh}        │
    │←──────────────────│                  │                    │
```

---

## 2. Token Refresh Flow

```
┌────────┐         ┌─────────┐         ┌──────────┐
│ Client │         │ MoxUI   │         │ DB       │
└───┬────┘         └────┬────┘         └────┬─────┘
    │                   │                  │
    │ 1. POST /auth/refresh                  │
    │ Cookie: refresh_token=***             │
    │──────────────────→│                  │
    │                   │ 2. Hash token (SHA-256)│
    │                   │                  │
    │                   │ 3. SELECT refresh_tokens│
    │                   │──────────────────→│
    │                   │                  │
    │                   │ 4. Token row     │
    │                   │←─────────────────│
    │                   │                  │
    │                   ├─── CHECKS ──────→
    │                   │  • expires_at > now?
    │                   │  • used_at IS NULL?
    │                   │  • revoked_at IS NULL?
    │                   │                  │
    │                   ├─── ANY FAIL ───→
    │                   │ 5. Return 401 + clear cookie│
    │                   │ 6. Audit log (denied)│
    │                   │──────────────────→│
    │←──────────────────│                  │
    │                   │                  │
    │                   ├─── ALL PASS ────→
    │                   │ 7. INSERT new refresh_token│
    │                   │ 8. UPDATE old.used_at + replaced_by│
    │                   │──────────────────→│
    │                   │                  │
    │                   │ 9. Issue new JWT  │
    │                   │ 10. Set new cookies│
    │                   │ 11. Audit log (token.rotated)│
    │                   │──────────────────→│
    │                   │                  │
    │                   │ 12. 200 OK + new tokens│
    │←──────────────────│                  │
```

### Reuse detection (rotation chain)

If old token is reused (used_at IS NOT NULL):

```
1. Find user_id from token
2. Check token chain (replaced_by links)
3. If any token in chain was revoked → REUSE ATTACK
4. Revoke ALL tokens for this user (force logout everywhere)
5. Audit log (severity: high)
6. Alert admin via webhook
7. Return 401
```

---

## 3. VM Operation Flow

### 3.1 Start VM

```
Client → POST /api/v1/vms/{cluster}/{node}/{vmid}/start
   │
   ├─→ Auth middleware: extract JWT → User
   ├─→ RBAC check: user.role >= operator?
   ├─→ Rate limit: per-user, per-action
   ├─→ Validate: vmid is u32
   │
   ├─→ INSERT audit_log (action: "vm.start", result: pending)
   │
   ├─→ ProxmoxClient (per cluster)
   │     ├─→ ensure_ticket() (refresh if <5min to expiry)
   │     ├─→ POST /nodes/{node}/qemu/{vmid}/status/start
   │     │   Headers: Cookie + CSRFPreventionToken
   │     ├─→ Proxmox returns UPID
   │     └─→ Return UPID to MoxUI
   │
   ├─→ INSERT audit_log (action: "vm.start", details: {upid}, result: pending)
   │
   ├─→ Invalidate cache (VM list for this cluster)
   │
   ├─→ Return 202 Accepted + {upid}
   │
   └─→ Frontend polls /api/v1/tasks/{upid}
```

### 3.2 Polling task status

```
Client → GET /api/v1/tasks/{upid} (every 1s while pending)
   │
   ├─→ Auth middleware
   ├─→ Parse upid: "UPID:pve11:00001234:..."
   │
   ├─→ ProxmoxClient.get(/nodes/{node}/tasks/{upid}/status)
   │     ├─→ Circuit breaker check
   │     ├─→ Cache (5s TTL)
   │     └─→ Return task status JSON
   │
   ├─→ Return to frontend
   │
   └─→ When status == "stopped":
         ├─→ If exitstatus == "OK" → return success
         └─→ If exitstatus == "ERROR" → return error
```

### 3.3 Delete VM

```
Client → DELETE /api/v1/vms/{cluster}/{node}/{vmid}?purge=1
   │
   ├─→ Auth + RBAC: admin only
   │
   ├─→ Confirm dialog required (frontend)
   │
   ├─→ INSERT audit_log
   │
   ├─→ ProxmoxClient.delete(/nodes/{node}/qemu/{vmid}?purge=1)
   │     └─→ Proxmox stops VM (if running), deletes config, deletes disks
   │
   ├─→ Soft-delete in local DB (set deleted_at)
   │     (for future "undo" feature — v1.1)
   │
   ├─→ Audit log (success)
   │
   ├─→ Invalidate cache
   │
   └─→ Return 204 No Content
```

---

## 4. Multi-Cluster Aggregation Flow

### Dashboard request

```
Client → GET /api/v1/dashboard?cluster=all
   │
   ├─→ Auth + RBAC: filter by allowed clusters
   │
   ├─→ Read from cache (TTL 5s)
   │     ├─ HIT → return cached aggregate
   │     │
   │     └─ MISS → continue ↓
   │
   ├─→ For each allowed cluster (parallel via join_all):
   │     ├─→ Check cluster cache (TTL 5s)
   │     │     ├─ HIT → use cached
   │     │     └─ MISS → fetch from Proxmox
   │     │              ├─→ Circuit breaker check
   │     │              ├─→ If open: skip + log warning
   │     │              └─→ If closed: GET /cluster/resources?type=vm
   │     │
   │     └─→ Return cluster VMs
   │
   ├─→ Aggregate (sort, filter, paginate)
   │
   ├─→ Cache aggregate (TTL 5s)
   │
   ├─→ Return JSON
   │
   └─→ Frontend renders dashboard
```

### Background poller (runs continuously)

```
Every 5 seconds per cluster:
   │
   ├─→ Check if circuit breaker open
   │     └─ YES → skip this cluster
   │
   ├─→ GET /cluster/resources?type=vm
   │     ├─ Success → update cache
   │     └─ Failure → increment breaker failure count
   │
   ├─→ If failure count >= 5 → open circuit breaker
   │
   └─→ Sleep 5s (next iteration)
```

---

## 5. Audit Log Capture Flow

### Every mutating endpoint

```
Handler executes mutation
   │
   ├─→ (action already started)
   │
   ├─→ On success:
   │     ├─→ INSERT audit_log
   │     │     {
   │     │       user_id: Some(user.id),
   │     │       username: user.username,
   │     │       action: "vm.start",
   │     │       target_type: "vm",
   │     │       target_id: "pve11/103",
   │     │       cluster_id: Some(1),
   │     │       ip_address: req.ip,
   │     │       user_agent: req.headers["user-agent"],
   │     │       request_id: tracing::Span::field("request_id"),
   │     │       result: "success",
   │     │       details: Some({ "upid": "..." }),
   │     │       created_at: unixepoch()
   │     │     }
   │     │
   │     └─→ Return success response
   │
   ├─→ On failure:
   │     ├─→ INSERT audit_log (result: "failure", error_message: "...")
   │     │
   │     └─→ Return error response (4xx/5xx)
   │
   └─→ On permission denied:
         ├─→ INSERT audit_log (result: "denied")
         │
         └─→ Return 403 Forbidden
```

### Audit log query (admin)

```
Client → GET /api/v1/audit?user_id=5&action=vm.start&from=2026-06-01&to=2026-06-30&limit=100&offset=0
   │
   ├─→ Auth + RBAC: admin only
   │
   ├─→ Validate filters
   │
   ├─→ Build query (parameterized):
   │     SELECT * FROM audit_log
   │     WHERE (? IS NULL OR user_id = ?)
   │       AND (? IS NULL OR action LIKE ?)
   │       AND (? IS NULL OR cluster_id = ?)
   │       AND (? IS NULL OR created_at >= ?)
   │       AND (? IS NULL OR created_at <= ?)
   │       AND (? IS NULL OR result = ?)
   │     ORDER BY created_at DESC
   │     LIMIT ? OFFSET ?
   │
   ├─→ Return JSON + pagination metadata
   │
   └─→ Frontend renders table
```

---

## 6. WebSocket Console Flow

### Initial connection

```
Browser (noVNC client)
   │
   │ 1. WS upgrade: GET /api/v1/console/{cluster}/{node}/{vmid}
   │    Headers: Authorization: Bearer ***
   │    Cookie: refresh=***
   │
   ├─→ Auth middleware: verify JWT
   ├─→ RBAC check: can access this cluster?
   │
   ├─→ ProxmoxClient.post(/vncproxy)
   │     { websocket: 1 }
   │     → Returns { port: 5900, ticket: "..." }
   │
   ├─→ Establish TCP connection to pve11:5900
   │     (with ticket as password)
   │
   ├─→ Spawn 2 tasks:
   │     • Browser → MoxUI (decode RFB, encode WS)
   │     • MoxUI → Proxmox (encode RFB, decode WS)
   │
   └─→ Return 101 Switching Protocols (WebSocket open)
```

### Bi-directional data flow

```
Browser                MoxUI                    Proxmox VNC
   │                      │                          │
   │ WS msg: keypress     │                          │
   │─────────────────────→│                          │
   │                      │ 1. Decode WS frame       │
   │                      │ 2. Encode RFB KeyEvent   │
   │                      │─────────────────────────→│
   │                      │                          │
   │                      │ 3. RFB FrameUpdate       │
   │                      │←─────────────────────────│
   │                      │ 4. Decode RFB            │
   │                      │ 5. Encode WS binary      │
   │ WS msg: frame        │                          │
   │←─────────────────────│                          │
   │                      │                          │
   │ 6. Render to canvas  │                          │
   │                      │                          │
```

### Disconnection handling

```
Client disconnects (network, navigation, refresh)
   │
   ├─→ WebSocket close frame received
   ├─→ Close TCP connection to Proxmox
   ├─→ Cancel both tokio tasks
   ├─→ Log: "Console session ended: {duration}"
   ├─→ Audit log (action: "console.disconnect")
   │
   └─→ Cleanup resources
```

---

## 7. Backup Flow

```
Client → POST /api/v1/vms/{cluster}/{node}/{vmid}/backup
   Body: { mode: "snapshot", storage: "local-lvm", compress: "zstd" }
   │
   ├─→ Auth + RBAC: operator+
   ├─→ Validate inputs
   ├─→ Audit log (pending)
   │
   ├─→ ProxmoxClient.post(/nodes/{node}/vzdump)
   │     Form: {
   │       vmid: 103,
   │       mode: "snapshot",
   │       storage: "local-lvm",
   │       compress: "zstd",
   │       remove: 0,    // keep old backups
   │       maxfiles: 0   // unlimited
   │     }
   │     → Proxmox returns UPID
   │
   ├─→ Audit log (success: {upid, mode, storage})
   │
   ├─→ Return 202 + UPID
   │
   └─→ Frontend polls task status
```

### Backup list query

```
Client → GET /api/v1/backups?cluster=homelab&vmid=103
   │
   ├─→ Auth + RBAC
   │
   ├─→ For each storage in cluster:
   │     ProxmoxClient.get(/nodes/{node}/storage/{storage}/content?content=backup)
   │     → Returns list of .vma.zst files
   │
   ├─→ Aggregate + filter
   │
   ├─→ Return JSON
```

---

## 8. Rate Limiting Flow

```
Client sends request
   │
   ├─→ Rate limit middleware (tower-governor)
   │
   ├─→ Check key (IP for login, user_id for API)
   │
   ├─→ Check rate (5/min for login, 100/min for API)
   │
   ├─→ Within limit → continue to handler
   │
   └─→ Exceeded → return 429 Too Many Requests
                  Headers: Retry-After: 60, X-RateLimit-Remaining: 0
```

---

## 9. Search & Filter Flow

```
Client → GET /api/v1/vms?search=web&tag=production&state=running&sort=cpu&order=desc&page=1&limit=50
   │
   ├─→ Parse query params
   │
   ├─→ Get VMs from cache (TTL 5s)
   │
   ├─→ Apply filters (in Rust):
   │     • search: lower(name).contains(lower(search))
   │     • tag: vm.tags.contains(&"production")
   │     • state: vm.status == "running"
   │
   ├─→ Sort:
   │     • sort=cpu → by vm.cpu desc
   │     • sort=name → by vm.name asc
   │
   ├─→ Paginate: skip(50 * (page-1)).take(50)
   │
   ├─→ Return { data: [...], total: N, page, limit }
   │
   └─→ Frontend renders filtered table
```

---

**See also:**
- [`ARCHITECTURE.md`](../ARCHITECTURE.md) — high-level
- [`state-machines.md`](./state-machines.md) — state diagrams
- [`security-boundaries.md`](./security-boundaries.md) — security architecture