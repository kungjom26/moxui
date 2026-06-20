# Data Flow Architecture

> **Last updated:** 2026-06-20

---

## 1. Data at Rest

### Storage locations

```
┌────────────────────────────────────────────────────────────┐
│  Container filesystem (read-only after init)               │
│  ┌─────────────────────────────────────────────────────┐  │
│  │  /usr/local/bin/moxui          # Binary (15 MB)     │  │
│  │  /etc/moxui/config.yaml       # Default config      │  │
│  │  /usr/local/share/moxui/ui/   # Embedded UI         │  │
│  └─────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────┐
│  Persistent volume (read-write)                            │
│  ┌─────────────────────────────────────────────────────┐  │
│  │  /home/moxui/data/moxui.db       # SQLite (main)   │  │
│  │  /home/moxui/data/moxui.db-wal   # WAL file          │  │
│  │  /home/moxui/data/moxui.db-shm   # Shared memory     │  │
│  │  /home/moxui/data/backups/       # Auto-backup       │  │
│  │  /home/moxui/data/logs/          # App logs (optional)│  │
│  └─────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────┐
│  In-memory (lost on restart)                                │
│  ┌─────────────────────────────────────────────────────┐  │
│  │  moka cache (TTL + LRU)                              │  │
│  │  Proxmox tickets (per cluster)                       │  │
│  │  JWT verification cache                              │  │
│  │  Rate limit counters                                 │  │
│  └─────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────┘
```

### SQLite WAL mode

```
┌────────────────────────────────────────────────────────────┐
│  Process 1 (MoxUI)                                           │
│  ────write─────→ moxui.db-wal (append-only)                 │
│                                                              │
│  Process 2 (Reader)                                          │
│  ────read──────→ moxui.db (snapshot at checkpoint)          │
│                                                              │
│  Every N seconds → checkpoint: copy WAL → main DB, truncate WAL│
└────────────────────────────────────────────────────────────┘

Benefits:
  • Concurrent readers (don't block writers)
  • Crash-safe (replay WAL on startup)
  • Faster writes (sequential append)

Backup strategy:
  • Use sqlite3 .backup command (atomic snapshot)
  • Includes WAL + main DB
  • Safe to backup while running
```

---

## 2. Data Flow: Read Path (VM List)

```
┌──────┐    ┌──────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐
│Client│    │Caddy │    │  MoxUI  │    │ Cache   │    │Proxmox  │
└──┬───┘    └──┬───┘    └────┬────┘    └────┬────┘    └────┬────┘
   │           │             │             │             │
   │ 1. GET /vms           │             │             │
   │─────────→│             │             │             │
   │          │ 2. Forward  │             │             │
   │          │────────────→│             │             │
   │          │             │             │             │
   │          │             │ 3. Auth     │             │
   │          │             │ (JWT verify)│             │
   │          │             │             │             │
   │          │             │ 4. RBAC    │             │
   │          │             │ (filter by │             │
   │          │             │  allowed   │             │
   │          │             │  clusters) │             │
   │          │             │             │             │
   │          │             │ 5. Cache lookup         │
   │          │             │────────────→│            │
   │          │             │            │             │
   │          │             │ 6. Cache HIT/MISS       │
   │          │             │←────────────│            │
   │          │             │            │             │
   │          │             ├── HIT ────┤             │
   │          │             │ 7. Return cached        │
   │          │             │  (5ms)     │             │
   │          │             │            │             │
   │          │             ├── MISS ───┤             │
   │          │             │ 8. Check poller status  │
   │          │             │            │             │
   │          │             ├── if poller fresh ─────│
   │          │             │   Use poller's data      │
   │          │             │            │             │
   │          │             ├── if stale ─────────────│
   │          │             │   9. Proxmox API call   │
   │          │             │─────────────────────────→│
   │          │             │            │             │
   │          │             │   10. Response (VM list) │
   │          │             │←────────────────────────│
   │          │             │            │             │
   │          │             │ 11. Update cache        │
   │          │             │────────────→            │
   │          │             │            │             │
   │          │             │ 12. Return VMs to client│
   │          │←────────────│            │             │
   │←─────────│             │             │             │
   │          │             │             │             │
   │ 13. Render table        │             │             │
```

---

## 3. Data Flow: Write Path (VM Start)

```
┌──────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐
│Client│    │  MoxUI  │    │   DB    │    │ Audit   │    │Proxmox  │
└──┬───┘    └────┬────┘    └────┬────┘    └────┬────┘    └────┬────┘
   │             │              │             │             │
   │ 1. POST /vms/103/start   │             │             │
   │────────────→│             │             │             │
   │             │             │             │             │
   │             │ 2. Auth + RBAC + Validate              │
   │             │             │             │             │
   │             │ ┌─ BEGIN transaction ─┐                │
   │             │ │                     │                │
   │             │ 3. INSERT audit_log   │                │
   │             │─────────────────────→│                │
   │             │  (pending)            │                │
   │             │                       │                │
   │             │ 4. (DB updates if needed)              │
   │             │                       │                │
   │             │ └─ COMMIT ──────────────────────────┘│
   │             │             │             │             │
   │             │ 5. Get ProxmoxClient   │             │
   │             │ 6. ensure_ticket()     │             │
   │             │             │             │             │
   │             │ 7. POST .../status/start              │
   │             │──────────────────────────────────────→│
   │             │             │             │             │
   │             │ 8. UPID returned        │             │
   │             │←──────────────────────────────────────│
   │             │             │             │             │
   │             │ 9. UPDATE audit_log (result: success)  │
   │             │─────────────────────→│                │
   │             │                       │                │
   │             │ 10. Invalidate cache (VM list)          │
   │             │             │             │             │
   │             │ 11. Return 202 + UPID   │             │
   │←────────────│             │             │             │
   │             │             │             │             │
   │ 12. Poll task status (every 1s)                     │
   │             │             │             │             │
   │             │ 13. GET /tasks/{upid}/status            │
   │             │──────────────────────────────────────→│
   │             │             │             │             │
   │             │ 14. Status response                     │
   │             │←──────────────────────────────────────│
   │             │             │             │             │
   │ 15. Return status          │             │             │
   │←────────────│             │             │             │
   │             │             │             │             │
   │ 16. Update UI (spinner → checkmark)  │             │
```

---

## 4. Data Flow: Multi-Cluster Aggregation

### On dashboard request

```
                              ┌──────────┐
                              │   Cache  │
                              │ (moka)  │
                              └────┬─────┘
                                   │
        ┌──────────────────────────┼──────────────────────────┐
        │                          │                          │
   ┌────▼─────┐              ┌──────▼─────┐              ┌──────▼─────┐
   │ homelab  │              │   prod     │              │  staging   │
   │  cache   │              │   cache    │              │   cache    │
   └────┬─────┘              └──────┬─────┘              └──────┬─────┘
        │                          │                          │
        │ ┌─ if MISS ─┐            │ ┌─ if MISS ─┐            │ ┌─ if MISS ─┐
        │ ↓            ↓            │ ↓            ↓            │ ↓            ↓
   ┌────▼──────┐  ┌────▼──────┐ ┌───▼────────┐ ┌───▼────────┐ ┌──▼─────────┐ ┌──▼─────────┐
   │Poller #1  │  │Proxmox    │ │Poller #2   │ │Proxmox     │ │Poller #3  │ │Proxmox     │
   │(every 5s) │  │API call   │ │(every 5s)  │ │API call    │ │(every 5s) │ │API call    │
   └───────────┘  └───────────┘ └────────────┘ └────────────┘ └───────────┘ └───────────┘
        │                          │                          │
        └──────────────┬───────────┴──────────────┬───────────┘
                       ↓                          ↓
                  ┌─────────────────────────────────┐
                  │   Aggregate + Filter + Sort   │
                  │   (in Rust, no extra calls)    │
                  └────────────────┬────────────────┘
                                   ↓
                              ┌─────────┐
                              │ Client  │
                              └─────────┘
```

### Background poller (runs continuously)

```
┌──────────────────────────────────────────────────────────────┐
│  Tokio task (spawned at startup, runs forever)               │
│                                                              │
│  loop {                                                      │
│    for each cluster in config.clusters {                     │
│      if circuit_breaker.is_open() {                          │
│        skip;                                                 │
│        continue;                                             │
│      }                                                       │
│                                                              │
│      match proxmox.get("/cluster/resources?type=vm") {       │
│        Ok(vms) => {                                          │
│          cache.put(format!("vms:{}", cluster_name), vms);     │
│          circuit_breaker.record_success();                   │
│        }                                                     │
│        Err(e) => {                                           │
│          circuit_breaker.record_failure();                   │
│          tracing::warn!("poller failed: {}", e);             │
│        }                                                     │
│      }                                                       │
│    }                                                         │
│                                                              │
│    sleep(Duration::from_secs(5)).await;                     │
│  }                                                           │
└──────────────────────────────────────────────────────────────┘
```

---

## 5. Data Flow: Cache Invalidation

### On any VM mutation (start/stop/reboot/delete)

```
Mutation handler
   │
   ├─→ 1. Execute mutation via Proxmox API
   │
   ├─→ 2. Get affected cluster name
   │
   ├─→ 3. Invalidate cache entries:
   │     cache.invalidate(format!("vms:{}", cluster_name))
   │     cache.invalidate(format!("cluster:{}", cluster_name))
   │     cache.invalidate(format!("vm:{}:{}", cluster_name, vmid))
   │
   ├─→ 4. Audit log (success)
   │
   └─→ 5. Return response
```

### TTL-based invalidation (no explicit invalidation)

```
On read:
   │
   ├─→ cache.get(key)
   │
   ├─→ If exists and age < TTL:
   │     return cached
   │
   └─→ Else:
         fetch from Proxmox
         cache.put(key, value, TTL)
         return value
```

---

## 6. Data Flow: Authentication (Token Storage)

```
┌─────────────────────────────────────────────────────────────┐
│  Client (browser)                                            │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  Access token (JWT)                                   │   │
│  │  - Stored in JavaScript memory (not localStorage)     │   │
│  │  - Added to Authorization header on every request       │   │
│  │  - Lost on page refresh (must re-login or refresh)    │   │
│  │                                                        │   │
│  │  Refresh token                                         │   │
│  │  - Stored in HttpOnly cookie                            │   │
│  │  - Sent automatically on /auth/refresh                  │   │
│  │  - Persists across page refreshes                       │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ HTTPS
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  MoxUI server                                                │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  Access token                                          │   │
│  │  - Verified on every request via middleware           │   │
│  │  - Public key (RS256) used to verify signature       │   │
│  │  - NOT stored (stateless)                             │   │
│  │                                                        │   │
│  │  Refresh token                                         │   │
│  │  - Stored in SQLite as SHA-256 hash                   │   │
│  │  - Single-use (marked used_at after use)              │   │
│  │  - Rotation chain via replaced_by FK                  │   │
│  │  - Revocable via revoked_at column                    │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### Token lifecycle

```
Login
  ├─→ Issue access_token (15min) + refresh_token (7d)
  │
  │ Time passes...
  │
  ├─→ Access token expires (after 15min)
  │     Client gets 401
  │     Client POST /auth/refresh (with refresh cookie)
  │     Server verifies refresh token
  │     Issue new access_token + new refresh_token (rotation)
  │     Mark old refresh as used
  │
  │ Time passes...
  │
  ├─→ User clicks logout
  │     Client POST /auth/logout
  │     Server marks refresh as revoked
  │     Clear cookie
  │
  │ OR
  │
  └─→ Refresh token expires (after 7d)
        User must login again
```

---

## 7. Data Flow: Audit Log Capture

### Every mutating handler

```
Handler
  │
  ├─→ (action starts)
  │
  ├─→ Build AuditLogEntry {
  │     user_id: extension_user.id,
  │     username: extension_user.username,
  │     action: "vm.start",
  │     target_type: "vm",
  │     target_id: format!("{}/{}", node, vmid),
  │     cluster_id: cluster.id,
  │     ip_address: connection_info.real_ip,
  │     user_agent: headers.user_agent,
  │     request_id: tracing::Span::field("request_id"),
  │     result: "success",  // or "failure" / "denied"
  │     details: Some(json!({"upid": upid})),
  │     created_at: now(),
  │   }
  │
  ├─→ audit_repo.insert(entry).await
  │
  └─→ (continue handler)
```

### Audit log query

```
GET /api/v1/audit?user_id=5&action=vm.start&from=2026-06-01&to=2026-06-30&limit=100
  │
  ├─→ Parse filters (with bounds checking)
  │
  ├─→ Build parameterized query:
  │     SELECT id, user_id, username, action, target_type, target_id,
  │            cluster_id, ip_address, user_agent, result,
  │            error_message, details, created_at
  │     FROM audit_log
  │     WHERE (? IS NULL OR user_id = ?)
  │       AND (? IS NULL OR action = ?)
  │       AND (? IS NULL OR cluster_id = ?)
  │       AND (? IS NULL OR created_at >= ?)
  │       AND (? IS NULL OR created_at <= ?)
  │       AND (? IS NULL OR result = ?)
  │     ORDER BY created_at DESC
  │     LIMIT ? OFFSET ?
  │
  ├─→ Execute query
  │
  ├─→ Count total (for pagination)
  │
  └─→ Return { data: [...], total: N, page, limit }
```

---

## 8. Data Flow: Database Backup

```
CronJob (daily 02:00 UTC)
  │
  ├─→ Run scripts/backup.sh
  │
  ├─→ sqlite3 /data/moxui.db ".backup '/tmp/moxui-backup.db'"
  │     (atomic snapshot, includes WAL)
  │
  ├─→ tar -czf backup-{date}.tar.gz
  │     --exclude='*.db-journal' --exclude='*.db-shm'
  │     moxui.db moxui.db-wal config/
  │
  ├─→ (optional) Encrypt with age:
  │     age -r age1xxx... < backup.tar.gz > backup.tar.gz.age
  │
  ├─→ Verify integrity:
  │     tar -tzf backup.tar.gz > /dev/null
  │     sqlite3 backup.db "PRAGMA integrity_check;"
  │
  ├─→ Upload to remote:
  │     rsync /tmp/backup.tar.gz.age backup-server:/backups/moxui/
  │     OR aws s3 cp backup.tar.gz.age s3://my-bucket/moxui/
  │
  ├─→ Cleanup local backups > 30 days
  │
  └─→ Log: "backup complete: {size} bytes uploaded to {destination}"
```

---

## 9. Data Flow: Real-Time Updates (Polling)

### Client polls /api/v1/vms every 2s

```
Browser                              MoxUI
   │                                   │
   │ 1. GET /api/v1/vms (every 2s)    │
   │──────────────────────────────────→│
   │                                   │
   │                                   │ 2. Check cache
   │                                   │    (5s TTL)
   │                                   │
   │                                   ├── HIT ──→ 3. Return cached
   │                                   │           (5ms)
   │                                   │
   │ 4. JSON response                 │
   │←──────────────────────────────────│
   │                                   │
   │ 5. Diff with previous state      │
   │ 6. Update only changed rows      │
   │                                   │
   │ (repeat every 2s)
```

**Optimization:** Send `ETag` header based on response hash → server returns 304 Not Modified if no change → zero data transfer

---

## 10. Data Flow: VNC Console (Binary Stream)

```
noVNC (browser)               MoxUI                     Proxmox VNC (port 5900)
      │                          │                              │
      │ 1. WS upgrade            │                              │
      │ ws://.../console/...    │                              │
      │─────────────────────────→│                              │
      │                          │ 2. Auth (JWT)                │
      │                          │ 3. Get VNC ticket            │
      │                          │    POST /vncproxy            │
      │                          │─────────────────────────────→│
      │                          │ 4. {port: 5900, ticket: ...}│
      │                          │←─────────────────────────────│
      │                          │                              │
      │                          │ 5. TCP connect pve11:5900   │
      │                          │─────────────────────────────→│
      │                          │ 6. RFB handshake             │
      │                          │←─────────────────────────────→
      │                          │                              │
      │                          │ 7. Open WebSocket to browser │
      │ 8. WS open               │                              │
      │←─────────────────────────│                              │
      │                          │                              │
      │ 9. RFB frame (canvas)    │                              │
      │←─────────────────────────│                              │
      │                          │                              │
      │                          │ 10. RFB frame update         │
      │                          │←─────────────────────────────│
      │                          │ 11. Encode as WS binary      │
      │ 12. WS msg (frame)       │                              │
      │←─────────────────────────│                              │
      │                          │                              │
      │ 13. Render to canvas     │                              │
      │                          │                              │
      │                          │ (loop for keyboard/mouse/    │
      │                          │  resize/clipboard)           │
```

---

## 11. Data Movement Summary

### Network traffic

| Source | Destination | Protocol | Data size | Frequency |
|---|---|---|---|---|
| Browser → Caddy | HTTPS (443) | TLS 1.3 | ~5 KB avg | Per user action |
| Caddy → MoxUI | HTTP (8080) | Plain | ~5 KB avg | Per user action |
| MoxUI → Proxmox | HTTPS (8006) | TLS (CA pinned) | ~2 KB avg | Per API call |
| MoxUI → Proxmox (poll) | HTTPS (8006) | TLS | ~50 KB | Every 5s per cluster |
| Browser ↔ MoxUI (console) | WebSocket | WSS | ~30 fps | Continuous during console use |
| MoxUI → S3 (backup) | HTTPS (443) | TLS | ~50 MB | Daily 02:00 |

### Data persistence

| Data | Persistent? | Where | Backup? |
|---|---|---|---|
| User passwords (bcrypt) | ✅ | SQLite | Yes (encrypted backup) |
| Refresh tokens (SHA-256 hash) | ✅ | SQLite | Yes |
| Audit log | ✅ | SQLite (immutable) | Yes |
| Cluster passwords (encrypted) | ✅ | SQLite | Yes |
| Proxmox tickets | ❌ | Memory | No (auto-refresh) |
| Cache (VM lists) | ❌ | Memory (moka) | No |
| Proxmox VM data | ❌ (read-only) | Proxmox itself | Backed up by Proxmox |
| Session JWT | ❌ | Client + signature | N/A (stateless) |

---

**See also:**
- [`ARCHITECTURE.md`](../ARCHITECTURE.md) — high-level
- [`request-flows.md`](./request-flows.md) — request details
- [`../DATA_MODEL.md`](../DATA_MODEL.md) — schema details