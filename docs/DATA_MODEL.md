# 🗄️ MoxUI — Data Model & Storage Architecture

> **Purpose:** Single source of truth สำหรับ data model ของ MoxUI v1.0.0
> **Audience:** Rust developers, DB admins, security reviewers
> **Storage:** SQLite (WAL mode) — embedded, zero-dep, fast
> **Last updated:** 2026-06-20

---

## 📋 Table of Contents

1. [Overview & Design Principles](#1-overview--design-principles)
2. [ER Diagram (Entity-Relationship)](#2-er-diagram)
3. [Schema (All Tables)](#3-schema)
4. [Indexes & Performance](#4-indexes--performance)
5. [Migrations](#5-migrations)
6. [Seed Data](#6-seed-data)
7. [Sample Queries](#7-sample-queries)
8. [Rust Struct Mapping](#8-rust-struct-mapping)
9. [Retention & Archival Policy](#9-retention--archival-policy)
10. [Backup & Restore Strategy](#10-backup--restore)
11. [Security Considerations](#11-security-considerations)

---

## 1. Overview & Design Principles

### 1.1 Storage choice — Why SQLite?

| Criterion | SQLite | PostgreSQL | MySQL |
|---|---|---|---|
| **Embedded (zero ops)** | ✅ | ❌ | ❌ |
| **Backup = file copy** | ✅ | ❌ | ❌ |
| **Performance for our scale** | ✅ (< 100K req/s) | ✅ | ✅ |
| **Concurrent reads** | ✅ (WAL) | ✅ | ✅ |
| **Concurrent writes** | ⚠️ (1 at a time) | ✅ | ✅ |
| **Container-friendly** | ✅ | ⚠️ | ⚠️ |
| **No service to manage** | ✅ | ❌ | ❌ |
| **License** | Public Domain | PostgreSQL | GPLv2 |

**Decision:** SQLite — สำหรับ MoxUI scale (10-100 concurrent users, ~10K audit entries/day) SQLite เกินพอ Container-friendly ไม่ต้องมี service แยก backup = copy 1 file

### 1.2 Design principles

1. **Immutable audit log** — append-only, ไม่มี UPDATE/DELETE (compliance)
2. **Soft delete where possible** — `deleted_at` column แทนการลบจริง (recovery)
3. **Hash everything sensitive** — passwords (bcrypt), refresh tokens (SHA-256), API keys (SHA-256)
4. **Foreign keys enforced** — `PRAGMA foreign_keys = ON`
5. **Type-safe enums** — ใช้ TEXT + CHECK constraint (SQLite ไม่มี native enum)
6. **Unix timestamps** — INTEGER (epoch seconds) เสมอ เพื่อ sort/index ง่าย
7. **UTF-8 everywhere** — TEXT เป็น UTF-8 (default)
8. **No JSON blobs ยกเว้น config** — normalize เป็น rows (query/index ง่ายกว่า)
9. **UTC always** — convert to local ใน UI

### 1.3 PRAGMAs (set at every connection)

```sql
PRAGMA journal_mode = WAL;          -- Write-Ahead Logging (concurrent reads)
PRAGMA synchronous = NORMAL;       -- 2-3x faster, still crash-safe with WAL
PRAGMA foreign_keys = ON;          -- enforce FK
PRAGMA temp_store = MEMORY;        -- temp tables in memory
PRAGMA cache_size = -64000;        -- 64 MB cache
PRAGMA busy_timeout = 5000;        -- 5s wait if locked
PRAGMA mmap_size = 268435456;      -- 256 MB mmap
```

---

## 2. ER Diagram

```
┌──────────────────────────────┐
│          users               │ ◄─────────────────────────────────┐
│──────────────────────────────│                                   │
│ PK  id                       │                                   │
│     username (UNIQUE)        │                                   │
│     email (UNIQUE)           │                                   │
│     password_hash            │                                   │
│     display_name             │                                   │
│     role                     │                                   │
│     is_active                │                                   │
│     is_locked                │                                   │
│     failed_login_count       │                                   │
│     locked_until             │                                   │
│     last_login_at            │                                   │
│     created_at, updated_at   │                                   │
│     deleted_at               │                                   │
└──────────────────────────────┘                                   │
       │                                                            │
       │ 1:N                                                        │
       ├──────────────────┐                                         │
       │                  │                                         │
       ▼                  ▼                                         │
┌─────────────────┐  ┌─────────────────────────┐                   │
│ totp_secrets    │  │ refresh_tokens          │                   │
│─────────────────│  │─────────────────────────│                   │
│ PK,FK user_id   │  │ PK id                   │                   │
│    secret       │  │    hash (token SHA-256) │                   │
│    is_confirmed │  │    user_id (FK)         │                   │
│    backup_codes │  │    expires_at           │                   │
│    created_at   │  │    used_at              │                   │
└─────────────────┘  │    revoked_at           │                   │
                     │    user_agent, ip       │                   │
                     │    created_at           │                   │
                     └─────────────────────────┘                   │
                                                                    │
       ┌────────────────────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────┐         ┌──────────────────────────┐
│  webauthn_credentials   │         │      api_keys            │
│──────────────────────────│         │──────────────────────────│
│ PK id                    │         │ PK id                    │
│ FK user_id               │         │    hash (key SHA-256)    │
│    credential_id (UNIQUE)│         │    user_id (FK)          │
│    public_key            │         │    name                  │
│    counter               │         │    scopes                │
│    transports            │         │    expires_at            │
│    name                  │         │    last_used_at          │
│    last_used_at          │         │    revoked_at            │
│    created_at            │         │    created_at            │
└──────────────────────────┘         └──────────────────────────┘

┌──────────────────────────────┐
│         clusters             │ ◄────────────────────────────┐
│──────────────────────────────│                              │
│ PK id                        │                              │
│    name (UNIQUE)             │                              │
│    url                       │                              │
│    username                  │                              │
│    password (encrypted)      │                              │
│    realm                     │                              │
│    verify_tls                │                              │
│    is_active                 │                              │
│    created_at, updated_at    │                              │
└──────────────────────────────┘                              │
       │                                                       │
       │ 1:N                                                   │
       ├────────────────────┐                                  │
       │                    │                                  │
       ▼                    ▼                                  │
┌─────────────────┐  ┌────────────────────────────┐            │
│ cluster_perms   │  │ cluster_status_cache       │            │
│─────────────────│  │────────────────────────────│            │
│ PK (user_id,    │  │ PK cluster_id              │            │
│     cluster_id) │  │    reachable               │            │
│    permission   │  │    last_poll_at            │            │
│    created_at   │  │    last_error              │            │
└─────────────────┘  └────────────────────────────┘            │
       ▲                                                       │
       │ N:M (user-cluster)                                     │
       └───────────────────────────────────────────────────────┘

┌──────────────────────────────┐
│       audit_log              │ ◄── append-only (no UPDATE/DELETE)
│──────────────────────────────│
│ PK id                        │
│ FK user_id (nullable)        │
│    action (enum)             │
│    target_type               │
│    target_id                 │
│    cluster_id (nullable)     │
│    ip_address                │
│    user_agent                │
│    request_id                │
│    result (enum)             │
│    details (JSON)            │
│    created_at                │
└──────────────────────────────┘

┌──────────────────────────────┐
│   oauth_state                │  (transient, expires 10min)
│──────────────────────────────│
│ PK state                     │
│    provider                  │
│    redirect_uri              │
│    pkce_verifier             │
│    created_at, expires_at    │
└──────────────────────────────┘

┌──────────────────────────────┐
│   ldap_configs               │
│──────────────────────────────│
│ PK id                        │
│    name (UNIQUE)             │
│    url                       │
│    bind_dn                   │
│    bind_password (encrypted) │
│    user_search_base          │
│    user_search_filter        │
│    group_search_base         │
│    group_search_filter       │
│    group_attribute           │
│    tls_mode (enum)           │
│    is_active                 │
│    created_at, updated_at    │
└──────────────────────────────┘

┌──────────────────────────────┐
│   system_config              │  (singleton — key/value)
│──────────────────────────────│
│ PK key                       │
│    value (JSON)              │
│    updated_at                │
└──────────────────────────────┘
```

---

## 3. Schema

### 3.1 `users`

```sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE COLLATE NOCASE,
    email TEXT UNIQUE COLLATE NOCASE,
    password_hash TEXT,                    -- bcrypt; NULL if SSO-only user
    display_name TEXT,
    role TEXT NOT NULL DEFAULT 'viewer'
        CHECK (role IN ('admin', 'operator', 'viewer')),
    auth_source TEXT NOT NULL DEFAULT 'local'
        CHECK (auth_source IN ('local', 'oidc', 'ldap')),
    is_active INTEGER NOT NULL DEFAULT 1
        CHECK (is_active IN (0, 1)),
    is_locked INTEGER NOT NULL DEFAULT 0
        CHECK (is_locked IN (0, 1)),
    failed_login_count INTEGER NOT NULL DEFAULT 0,
    locked_until INTEGER,                 -- unix timestamp
    last_login_at INTEGER,
    last_login_ip TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
    deleted_at INTEGER,                    -- soft delete
    CONSTRAINT chk_password_for_local
        CHECK ((auth_source = 'local' AND password_hash IS NOT NULL)
            OR auth_source != 'local')
);

CREATE INDEX idx_users_username ON users(username) WHERE deleted_at IS NULL;
CREATE INDEX idx_users_email ON users(email) WHERE deleted_at IS NULL;
CREATE INDEX idx_users_role ON users(role) WHERE is_active = 1 AND deleted_at IS NULL;
```

**Notes:**
- `password_hash` NULL สำหรับ SSO-only user (OIDC/LDAP) ไม่ต้องมี password
- `failed_login_count` reset to 0 on successful login
- `locked_until` ใช้ lockout 15 นาทีหลัง 5 failed attempts
- Soft delete — `deleted_at IS NULL` filter ในทุก query

### 3.2 `refresh_tokens`

```sql
CREATE TABLE refresh_tokens (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- hash ของ token (SHA-256) — ไม่เก็บ plaintext
    token_hash TEXT NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    used_at INTEGER,                       -- single-use rotation
    revoked_at INTEGER,                    -- logout / revoke
    replaced_by INTEGER,                   -- new token id (rotation chain)
    user_agent TEXT,
    ip_address TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (replaced_by) REFERENCES refresh_tokens(id) ON DELETE SET NULL
);

CREATE INDEX idx_refresh_tokens_user ON refresh_tokens(user_id);
CREATE INDEX idx_refresh_tokens_hash ON refresh_tokens(token_hash);
CREATE INDEX idx_refresh_tokens_expires ON refresh_tokens(expires_at);
```

**Notes:**
- **Hash only** — ถ้า DB leak, token ยังใช้ไม่ได้ (ต้อง hash ก่อน)
- **Single-use** — เมื่อใช้แล้ว `used_at` set, ต้อง rotate ใหม่
- **Rotation chain** — `replaced_by` link ไปยัง token ใหม่ (detect reuse)

### 3.3 `totp_secrets`

```sql
CREATE TABLE totp_secrets (
    user_id INTEGER PRIMARY KEY,
    secret TEXT NOT NULL,                  -- base32-encoded TOTP secret
    is_confirmed INTEGER NOT NULL DEFAULT 0
        CHECK (is_confirmed IN (0, 1)),
    backup_codes TEXT NOT NULL,            -- JSON array of 8 hashed codes
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    last_used_at INTEGER,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
```

**Notes:**
- `secret` base32-encoded (Google Authenticator compatible)
- `backup_codes` JSON array — เก็บ **hashed** (bcrypt) เพื่อ single-use
- `is_confirmed = 0` ระหว่าง setup (ยังไม่ activate)

### 3.4 `webauthn_credentials`

```sql
CREATE TABLE webauthn_credentials (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    credential_id BLOB NOT NULL UNIQUE,    -- raw credential ID from authenticator
    public_key BLOB NOT NULL,              -- COSE-encoded public key
    counter INTEGER NOT NULL DEFAULT 0,    -- signature counter (replay protection)
    transports TEXT,                       -- JSON: ["usb", "nfc", "ble", "internal"]
    aaguid BLOB,                           -- Authenticator Attestation GUID
    name TEXT NOT NULL,                    -- user-friendly name (e.g., "Yubikey 5")
    last_used_at INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX idx_webauthn_user ON webauthn_credentials(user_id);
CREATE INDEX idx_webauthn_cred_id ON webauthn_credentials(credential_id);
```

**Notes:**
- `counter` เพิ่มทุกครั้งที่ authenticate — ถ้า counter ไม่เพิ่ม = cloned key (reject)
- `public_key` COSE format (CBOR)
- รองรับ multi-key per user (Yubikey + Touch ID + Windows Hello)

### 3.5 `api_keys`

```sql
CREATE TABLE api_keys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- SHA-256 hash ของ key (เช่นเดียวกับ refresh token)
    key_hash TEXT NOT NULL UNIQUE,
    -- prefix สำหรับ identify (e.g., "mox_pk_abc12345...")
    key_prefix TEXT NOT NULL,
    user_id INTEGER NOT NULL,
    name TEXT NOT NULL,                    -- "CI/CD pipeline", "Terraform"
    scopes TEXT NOT NULL,                  -- JSON: ["vms:read", "vms:start"]
    expires_at INTEGER,                    -- NULL = never expire
    last_used_at INTEGER,
    revoked_at INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX idx_api_keys_hash ON api_keys(key_hash);
CREATE INDEX idx_api_keys_user ON api_keys(user_id);
CREATE INDEX idx_api_keys_expires ON api_keys(expires_at) WHERE revoked_at IS NULL;
```

**Notes:**
- `key_prefix` ช่วยให้ user identify key ได้โดยไม่ expose full key (เช่น `mox_pk_abc12345...`)
- `scopes` เป็น JSON array — restrict ได้แบบ granular
- `expires_at` NULL = ไม่ expire (ใช้สำหรับ CI/CD)

### 3.6 `clusters`

```sql
CREATE TABLE clusters (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    url TEXT NOT NULL,                     -- https://pve11.local:8006
    username TEXT NOT NULL,                -- e.g., "proxui@pve"
    -- Proxmox password encrypted (see §11.3)
    password_encrypted BLOB NOT NULL,
    realm TEXT NOT NULL DEFAULT 'pam'
        CHECK (realm IN ('pam', 'pve', 'openid')),
    verify_tls INTEGER NOT NULL DEFAULT 1
        CHECK (verify_tls IN (0, 1)),
    ca_cert_pem TEXT,                      -- optional CA cert (for self-signed PVE)
    is_active INTEGER NOT NULL DEFAULT 1
        CHECK (is_active IN (0, 1)),
    last_connected_at INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);
```

**Notes:**
- `password_encrypted` — encrypted ด้วย age/ChaCha20-Poly1305 (key จาก env var)
- `ca_cert_pem` สำหรับ self-signed Proxmox cert (verified แทน `verify_tls=0`)

### 3.7 `cluster_permissions`

```sql
CREATE TABLE cluster_permissions (
    user_id INTEGER NOT NULL,
    cluster_id INTEGER NOT NULL,
    permission TEXT NOT NULL DEFAULT 'user'
        CHECK (permission IN ('admin', 'operator', 'viewer', 'denied')),
    granted_by INTEGER,                    -- user_id ของคน grant
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (user_id, cluster_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (cluster_id) REFERENCES clusters(id) ON DELETE CASCADE,
    FOREIGN KEY (granted_by) REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX idx_cluster_perms_cluster ON cluster_permissions(cluster_id);
```

**Notes:**
- **Per-cluster permission override** — user.role = 'admin' แต่ถ้ามี row ในนี้ด้วย permission='viewer' → ใช้ row นี้ (specificity wins)
- `denied` สำหรับ explicit block — admin ก็ block ได้
- ถ้าไม่มี row → use user.role เป็น default

### 3.8 `cluster_status_cache`

```sql
CREATE TABLE cluster_status_cache (
    cluster_id INTEGER PRIMARY KEY,
    reachable INTEGER NOT NULL DEFAULT 0
        CHECK (reachable IN (0, 1)),
    last_poll_at INTEGER,
    last_success_at INTEGER,
    last_error TEXT,
    version TEXT,                          -- Proxmox version, e.g., "8.2.4"
    nodes_count INTEGER,
    vms_count INTEGER,
    FOREIGN KEY (cluster_id) REFERENCES clusters(id) ON DELETE CASCADE
);
```

**Notes:**
- Update ทุก 10s จาก background poller
- ใช้ใน dashboard — ไม่ต้อง query Proxmox ทุก UI request

### 3.9 `audit_log` ⭐ (append-only)

```sql
CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- user อาจ NULL ถ้าเป็น anonymous action (login fail)
    user_id INTEGER,
    username TEXT,                         -- denormalized for forensics (user อาจถูกลบ)
    action TEXT NOT NULL,                  -- e.g., "vm.start", "user.create"
    target_type TEXT,                      -- "vm", "user", "cluster"
    target_id TEXT,                        -- e.g., "pve11/103"
    cluster_id INTEGER,                    -- FK (nullable)
    ip_address TEXT,                       -- client IP
    user_agent TEXT,
    request_id TEXT,                       -- UUID ของ request (trace ข้าม log)
    result TEXT NOT NULL DEFAULT 'success'
        CHECK (result IN ('success', 'failure', 'denied')),
    error_message TEXT,                    -- ถ้า result != 'success'
    -- structured details (เช่น {"vmid": 103, "node": "pve11", "duration_ms": 250})
    details TEXT,                          -- JSON
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (cluster_id) REFERENCES clusters(id) ON DELETE SET NULL
);

CREATE INDEX idx_audit_user ON audit_log(user_id, created_at DESC);
CREATE INDEX idx_audit_action ON audit_log(action, created_at DESC);
CREATE INDEX idx_audit_cluster ON audit_log(cluster_id, created_at DESC);
CREATE INDEX idx_audit_target ON audit_log(target_type, target_id, created_at DESC);
CREATE INDEX idx_audit_result ON audit_log(result, created_at DESC);
CREATE INDEX idx_audit_created ON audit_log(created_at DESC);
CREATE INDEX idx_audit_request ON audit_log(request_id);
```

**Notes:**
- ⭐ **APPEND-ONLY** — ห้าม UPDATE/DELETE (enforce ด้วย trigger)
- `username` denormalized — user ถูกลบไปแล้วยังเห็นได้ว่าใครทำ
- `request_id` ช่วย trace ข้าม log entries (หนึ่ง request อาจมีหลาย audit entries)
- Composite indexes สำหรับ common queries (audit log viewer)

**Trigger to prevent UPDATE/DELETE:**
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

### 3.10 `oauth_state`

```sql
CREATE TABLE oauth_state (
    state TEXT PRIMARY KEY,                -- CSRF state token (random 32 bytes)
    provider TEXT NOT NULL,                -- "google", "github", etc.
    redirect_uri TEXT NOT NULL,
    pkce_verifier TEXT,                    -- PKCE code_verifier
    expires_at INTEGER NOT NULL,           -- 10 minutes
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX idx_oauth_state_expires ON oauth_state(expires_at);
```

**Notes:**
- Transient — cleanup job ลบทุกชั่วโมง
- `state` = CSRF protection (verify ตอน callback)
- `pkce_verifier` = PKCE OAuth2 flow

### 3.11 `ldap_configs`

```sql
CREATE TABLE ldap_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,             -- "corporate AD", "internal LDAP"
    url TEXT NOT NULL,                     -- ldap://corp.example.com:389
    bind_dn TEXT NOT NULL,                 -- CN=moxui,OU=Service,DC=...
    bind_password_encrypted BLOB NOT NULL,
    user_search_base TEXT NOT NULL,        -- OU=Users,DC=corp,DC=example,DC=com
    user_search_filter TEXT NOT NULL DEFAULT '(sAMAccountName={username})',
    user_attr_username TEXT NOT NULL DEFAULT 'sAMAccountName',
    user_attr_email TEXT DEFAULT 'mail',
    user_attr_display_name TEXT DEFAULT 'displayName',
    group_search_base TEXT,                -- OU=Groups,DC=...
    group_search_filter TEXT DEFAULT '(member={user_dn})',
    group_attribute TEXT DEFAULT 'cn',
    role_mapping TEXT,                     -- JSON: {"Domain Admins": "admin", ...}
    tls_mode TEXT NOT NULL DEFAULT 'starttls'
        CHECK (tls_mode IN ('none', 'starttls', 'ldaps')),
    is_active INTEGER NOT NULL DEFAULT 1,
    last_sync_at INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);
```

**Notes:**
- รองรับ multi-LDAP (ถ้ามีหลาย forest/domain)
- `role_mapping` map AD group → MoxUI role

### 3.12 `system_config`

```sql
CREATE TABLE system_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,                   -- JSON-encoded
    description TEXT,                      -- for admin UI
    updated_by INTEGER,
    updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (updated_by) REFERENCES users(id) ON DELETE SET NULL
);
```

**Default keys:**
- `auth.session_timeout_minutes` → 60
- `auth.lockout_threshold` → 5
- `auth.lockout_duration_minutes` → 15
- `audit.retention_days` → 90
- `cache.vm_list_ttl_seconds` → 5
- `cache.cluster_stats_ttl_seconds` → 10
- `security.max_login_attempts_per_ip` → 10
- `security.api_rate_limit_per_minute` → 100

**Notes:**
- Singleton-style — ไม่มี FK chain
- Update ผ่าน settings UI

---

## 4. Indexes & Performance

### 4.1 Index strategy

| Query pattern | Index used |
|---|---|
| Login by username | `idx_users_username` (UNIQUE) |
| Find user by email | `idx_users_email` (UNIQUE) |
| List active users by role | `idx_users_role` |
| Validate refresh token | `idx_refresh_tokens_hash` (UNIQUE) |
| List user's tokens | `idx_refresh_tokens_user` |
| Cleanup expired tokens | `idx_refresh_tokens_expires` |
| Validate API key | `idx_api_keys_hash` (UNIQUE) |
| Find user's API keys | `idx_api_keys_user` |
| Audit log: filter by user | `idx_audit_user` (user_id, created_at DESC) |
| Audit log: filter by action | `idx_audit_action` (action, created_at DESC) |
| Audit log: filter by cluster | `idx_audit_cluster` (cluster_id, created_at DESC) |
| Audit log: latest N | `idx_audit_created` (created_at DESC) |
| Trace by request_id | `idx_audit_request` |

### 4.2 Estimated sizes (1 year)

| Table | Rows | Size |
|---|---|---|
| users | ~50 | < 100 KB |
| refresh_tokens | ~500 (active) + ~50K/year (rotated) | ~10 MB/year |
| webauthn_credentials | ~50 | < 50 KB |
| api_keys | ~20 | < 10 KB |
| clusters | ~10 | < 50 KB |
| cluster_permissions | ~200 | < 50 KB |
| **audit_log** | ~500K/year | ~200 MB/year |
| oauth_state | < 100 (transient) | < 50 KB |
| system_config | ~20 | < 10 KB |
| **Total** | | **~220 MB/year** |

### 4.3 Performance tuning

- **WAL mode** — concurrent reads, 1 writer (sufficient for our load)
- **Cache size 64 MB** — keep hot indexes in memory
- **mmap 256 MB** — let SQLite use mmap for large reads
- **Prepared statements** — cache via `r2d2_sqlite` pool (Phase 3)
- **Bulk inserts** — for audit log batches (`INSERT ... VALUES (?,?), ...`)

---

## 5. Migrations

### 5.1 Migration framework

ใช้ **refinery** (Rust crate) — เก็บ migrations เป็น SQL files versioned

```
migrations/
├── V001__initial_schema.sql
├── V002__add_webauthn.sql
├── V003__add_ldap.sql
├── V004__add_audit_triggers.sql
├── V005__add_system_config.sql
└── ...
```

### 5.2 V001 — initial schema

```sql
-- migrations/V001__initial_schema.sql

-- Users
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE COLLATE NOCASE,
    email TEXT UNIQUE COLLATE NOCASE,
    password_hash TEXT,
    display_name TEXT,
    role TEXT NOT NULL DEFAULT 'viewer'
        CHECK (role IN ('admin', 'operator', 'viewer')),
    auth_source TEXT NOT NULL DEFAULT 'local'
        CHECK (auth_source IN ('local', 'oidc', 'ldap')),
    is_active INTEGER NOT NULL DEFAULT 1,
    is_locked INTEGER NOT NULL DEFAULT 0,
    failed_login_count INTEGER NOT NULL DEFAULT 0,
    locked_until INTEGER,
    last_login_at INTEGER,
    last_login_ip TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
    deleted_at INTEGER,
    CONSTRAINT chk_password_for_local
        CHECK ((auth_source = 'local' AND password_hash IS NOT NULL)
            OR auth_source != 'local')
);
CREATE INDEX idx_users_username ON users(username) WHERE deleted_at IS NULL;
CREATE INDEX idx_users_email ON users(email) WHERE deleted_at IS NULL;
CREATE INDEX idx_users_role ON users(role) WHERE is_active = 1 AND deleted_at IS NULL;

-- Refresh tokens
CREATE TABLE refresh_tokens (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    token_hash TEXT NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    used_at INTEGER,
    revoked_at INTEGER,
    replaced_by INTEGER,
    user_agent TEXT,
    ip_address TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (replaced_by) REFERENCES refresh_tokens(id) ON DELETE SET NULL
);
CREATE INDEX idx_refresh_tokens_user ON refresh_tokens(user_id);
CREATE INDEX idx_refresh_tokens_hash ON refresh_tokens(token_hash);
CREATE INDEX idx_refresh_tokens_expires ON refresh_tokens(expires_at);

-- Audit log
CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER,
    username TEXT,
    action TEXT NOT NULL,
    target_type TEXT,
    target_id TEXT,
    cluster_id INTEGER,
    ip_address TEXT,
    user_agent TEXT,
    request_id TEXT,
    result TEXT NOT NULL DEFAULT 'success'
        CHECK (result IN ('success', 'failure', 'denied')),
    error_message TEXT,
    details TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (cluster_id) REFERENCES clusters(id) ON DELETE SET NULL
);
CREATE INDEX idx_audit_user ON audit_log(user_id, created_at DESC);
CREATE INDEX idx_audit_action ON audit_log(action, created_at DESC);
CREATE INDEX idx_audit_cluster ON audit_log(cluster_id, created_at DESC);
CREATE INDEX idx_audit_target ON audit_log(target_type, target_id, created_at DESC);
CREATE INDEX idx_audit_result ON audit_log(result, created_at DESC);
CREATE INDEX idx_audit_created ON audit_log(created_at DESC);
CREATE INDEX idx_audit_request ON audit_log(request_id);
```

### 5.3 V002 — add WebAuthn

```sql
-- migrations/V002__add_webauthn.sql
CREATE TABLE webauthn_credentials (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    credential_id BLOB NOT NULL UNIQUE,
    public_key BLOB NOT NULL,
    counter INTEGER NOT NULL DEFAULT 0,
    transports TEXT,
    aaguid BLOB,
    name TEXT NOT NULL,
    last_used_at INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
CREATE INDEX idx_webauthn_user ON webauthn_credentials(user_id);
CREATE INDEX idx_webauthn_cred_id ON webauthn_credentials(credential_id);
```

### 5.4 V003 — add LDAP + API keys + TOTP + OAuth state

```sql
-- migrations/V003__add_auth_features.sql

CREATE TABLE totp_secrets (
    user_id INTEGER PRIMARY KEY,
    secret TEXT NOT NULL,
    is_confirmed INTEGER NOT NULL DEFAULT 0,
    backup_codes TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    last_used_at INTEGER,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE api_keys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key_hash TEXT NOT NULL UNIQUE,
    key_prefix TEXT NOT NULL,
    user_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    scopes TEXT NOT NULL,
    expires_at INTEGER,
    last_used_at INTEGER,
    revoked_at INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
CREATE INDEX idx_api_keys_hash ON api_keys(key_hash);
CREATE INDEX idx_api_keys_user ON api_keys(user_id);
CREATE INDEX idx_api_keys_expires ON api_keys(expires_at) WHERE revoked_at IS NULL;

CREATE TABLE oauth_state (
    state TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    redirect_uri TEXT NOT NULL,
    pkce_verifier TEXT,
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX idx_oauth_state_expires ON oauth_state(expires_at);

CREATE TABLE ldap_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    url TEXT NOT NULL,
    bind_dn TEXT NOT NULL,
    bind_password_encrypted BLOB NOT NULL,
    user_search_base TEXT NOT NULL,
    user_search_filter TEXT NOT NULL DEFAULT '(sAMAccountName={username})',
    user_attr_username TEXT NOT NULL DEFAULT 'sAMAccountName',
    user_attr_email TEXT DEFAULT 'mail',
    user_attr_display_name TEXT DEFAULT 'displayName',
    group_search_base TEXT,
    group_search_filter TEXT DEFAULT '(member={user_dn})',
    group_attribute TEXT DEFAULT 'cn',
    role_mapping TEXT,
    tls_mode TEXT NOT NULL DEFAULT 'starttls',
    is_active INTEGER NOT NULL DEFAULT 1,
    last_sync_at INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);
```

### 5.5 V004 — add audit triggers + clusters

```sql
-- migrations/V004__add_clusters_and_audit_triggers.sql

CREATE TABLE clusters (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    url TEXT NOT NULL,
    username TEXT NOT NULL,
    password_encrypted BLOB NOT NULL,
    realm TEXT NOT NULL DEFAULT 'pam',
    verify_tls INTEGER NOT NULL DEFAULT 1,
    ca_cert_pem TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    last_connected_at INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE cluster_permissions (
    user_id INTEGER NOT NULL,
    cluster_id INTEGER NOT NULL,
    permission TEXT NOT NULL DEFAULT 'user'
        CHECK (permission IN ('admin', 'operator', 'viewer', 'denied')),
    granted_by INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (user_id, cluster_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (cluster_id) REFERENCES clusters(id) ON DELETE CASCADE,
    FOREIGN KEY (granted_by) REFERENCES users(id) ON DELETE SET NULL
);
CREATE INDEX idx_cluster_perms_cluster ON cluster_permissions(cluster_id);

CREATE TABLE cluster_status_cache (
    cluster_id INTEGER PRIMARY KEY,
    reachable INTEGER NOT NULL DEFAULT 0,
    last_poll_at INTEGER,
    last_success_at INTEGER,
    last_error TEXT,
    version TEXT,
    nodes_count INTEGER,
    vms_count INTEGER,
    FOREIGN KEY (cluster_id) REFERENCES clusters(id) ON DELETE CASCADE
);

-- Audit log immutability triggers
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

### 5.6 Migration execution

```rust
// Rust migration runner (using refinery)
use refinery::embed_migrations;

embed_migrations!("migrations/");

fn run_migrations(db: &SqliteConnection) -> Result<()> {
    let migration = EmbeddedMigration::max_version().unwrap();
    let report = migrations::runner()
        .run_async(&mut *db)
        .await?;
    
    if !report.applied_migrations().is_empty() {
        tracing::info!("applied {} migrations", report.applied_migrations().len());
    }
    Ok(())
}
```

---

## 6. Seed Data

### 6.1 Bootstrap admin

เมื่อ first run (ไม่มี user ใน DB) สร้าง admin จาก env vars:

```rust
// First boot logic
if users::count_active(&db).await? == 0 {
    let username = env::var("MOXUI_ADMIN_USERNAME").unwrap_or_else(|_| "admin".into());
    let password = env::var("MOXUI_ADMIN_PASSWORD")
        .expect("MOXUI_ADMIN_PASSWORD required on first boot");
    
    users::create(&db, NewUser {
        username: &username,
        email: None,
        password_hash: Some(bcrypt::hash(&password, 12)?),
        role: Role::Admin,
        auth_source: AuthSource::Local,
        display_name: Some("Administrator"),
        ..Default::default()
    }).await?;
    
    audit::log(&db, AuditEntry {
        action: "user.bootstrap",
        target_type: "user",
        target_id: &username,
        result: AuditResult::Success,
        ..Default::default()
    }).await?;
    
    warn!("Bootstrap admin created. Force password change on first login.");
}
```

### 6.2 Default system_config values

```sql
INSERT INTO system_config (key, value, description) VALUES
    ('auth.session_timeout_minutes', '60', 'JWT access token TTL'),
    ('auth.refresh_token_ttl_days', '7', 'Refresh token TTL'),
    ('auth.lockout_threshold', '5', 'Failed attempts before lockout'),
    ('auth.lockout_duration_minutes', '15', 'Lockout duration'),
    ('audit.retention_days', '90', 'Audit log retention (0=forever)'),
    ('cache.vm_list_ttl_seconds', '5', 'VM list cache TTL'),
    ('cache.cluster_stats_ttl_seconds', '10', 'Cluster stats cache TTL'),
    ('security.max_login_attempts_per_ip', '10', 'Rate limit per IP per minute'),
    ('security.api_rate_limit_per_minute', '100', 'API rate limit per user per minute');
```

---

## 7. Sample Queries

### 7.1 Login flow (local + TOTP)

```sql
-- Step 1: Find user by username
SELECT id, username, password_hash, role, is_active, is_locked,
       failed_login_count, locked_until
FROM users
WHERE username = ? COLLATE NOCASE AND deleted_at IS NULL;

-- Step 2: Verify password (bcrypt verify in Rust, not SQL)

-- Step 3: If user requires TOTP, get secret
SELECT secret, is_confirmed FROM totp_secrets WHERE user_id = ?;

-- Step 4: On success, reset failed_login_count + update last_login_at
UPDATE users
SET failed_login_count = 0,
    locked_until = NULL,
    last_login_at = unixepoch(),
    last_login_ip = ?,
    updated_at = unixepoch()
WHERE id = ?;

-- Step 5: Create refresh token (hashed)
INSERT INTO refresh_tokens (token_hash, user_id, expires_at, user_agent, ip_address)
VALUES (?, ?, unixepoch() + 604800, ?, ?);

-- Step 6: Audit log
INSERT INTO audit_log (user_id, username, action, target_type, target_id,
                       ip_address, user_agent, result, request_id)
VALUES (?, ?, 'user.login', 'user', ?, ?, ?, 'success', ?);
```

### 7.2 RBAC check (user permission for cluster)

```sql
-- Check per-cluster permission (specificity wins)
SELECT
    u.role AS user_role,
    cp.permission AS cluster_permission
FROM users u
LEFT JOIN cluster_permissions cp
    ON cp.user_id = u.id
    AND cp.cluster_id = ?
WHERE u.id = ? AND u.deleted_at IS NULL AND u.is_active = 1;

-- In Rust:
-- 1. If no cp row → use user.role
-- 2. If cp.permission = 'denied' → deny
-- 3. If cp.permission exists → use it (overrides user.role)
-- 4. Check if permission >= required permission (admin > operator > viewer)
```

### 7.3 Audit log search (with filters)

```sql
-- Search by user, action, cluster, date range, with pagination
SELECT id, username, action, target_type, target_id, cluster_id,
       ip_address, result, details, created_at
FROM audit_log
WHERE (? IS NULL OR user_id = ?)
  AND (? IS NULL OR action LIKE ?)
  AND (? IS NULL OR cluster_id = ?)
  AND (? IS NULL OR result = ?)
  AND (? IS NULL OR created_at >= ?)
  AND (? IS NULL OR created_at <= ?)
ORDER BY created_at DESC
LIMIT ? OFFSET ?;
```

### 7.4 Cluster status overview

```sql
-- Dashboard cards (one row per cluster)
SELECT
    c.id, c.name, c.url,
    csc.reachable, csc.last_poll_at, csc.last_error,
    csc.version, csc.nodes_count, csc.vms_count
FROM clusters c
LEFT JOIN cluster_status_cache csc ON csc.cluster_id = c.id
WHERE c.is_active = 1
ORDER BY c.name;
```

### 7.5 Token cleanup (background job)

```sql
-- Delete expired refresh tokens (older than 7 days past expiry)
DELETE FROM refresh_tokens
WHERE expires_at < unixepoch() - 604800;

-- Delete used+expired tokens (already rotated)
DELETE FROM refresh_tokens
WHERE used_at IS NOT NULL
  AND used_at < unixepoch() - 86400;  -- 1 day grace

-- Delete revoked tokens (older than 30 days)
DELETE FROM refresh_tokens
WHERE revoked_at IS NOT NULL
  AND revoked_at < unixepoch() - 2592000;
```

### 7.6 Audit log retention

```sql
-- Move old audit entries to archive table or delete
-- (run monthly as background job)
DELETE FROM audit_log
WHERE created_at < unixepoch() - (90 * 86400);  -- 90 days

-- Alternative: move to archive
-- INSERT INTO audit_log_archive SELECT * FROM audit_log WHERE ...;
```

---

## 8. Rust Struct Mapping

### 8.1 User struct

```rust
// src/db/users.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: Option<String>,
    #[serde(skip_serializing)]  // never serialize password hash
    pub password_hash: Option<String>,
    pub display_name: Option<String>,
    pub role: Role,
    pub auth_source: AuthSource,
    pub is_active: bool,
    pub is_locked: bool,
    pub failed_login_count: i64,
    pub locked_until: Option<DateTime<Utc>>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub last_login_ip: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Operator,
    Viewer,
}

impl Role {
    pub fn can(&self, action: Action) -> bool {
        match (self, action) {
            (Role::Admin, _) => true,
            (Role::Operator, Action::Read) => true,
            (Role::Operator, Action::StartStopVm) => true,
            (Role::Operator, Action::CreateVm) => true,
            (Role::Operator, Action::DeleteVm) => false,  // admin only
            (Role::Viewer, Action::Read) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuthSource {
    Local,
    Oidc,
    Ldap,
}

// rusqlite FromRow implementation
impl From<&rusqlite::Row<'_>> for User {
    fn from(row: &rusqlite::Row) -> Self {
        User {
            id: row.get("id"),
            username: row.get("username"),
            email: row.get("email"),
            password_hash: row.get("password_hash"),
            display_name: row.get("display_name"),
            role: match row.get::<_, String>("role").as_str() {
                "admin" => Role::Admin,
                "operator" => Role::Operator,
                "viewer" => Role::Viewer,
                _ => unreachable!(),
            },
            auth_source: match row.get::<_, String>("auth_source").as_str() {
                "local" => AuthSource::Local,
                "oidc" => AuthSource::Oidc,
                "ldap" => AuthSource::Ldap,
                _ => unreachable!(),
            },
            is_active: row.get::<_, i64>("is_active") != 0,
            is_locked: row.get::<_, i64>("is_locked") != 0,
            failed_login_count: row.get("failed_login_count"),
            locked_until: row.get::<_, Option<i64>>("locked_until")
                .map(|ts| DateTime::from_timestamp(ts, 0).unwrap()),
            last_login_at: row.get::<_, Option<i64>>("last_login_at")
                .map(|ts| DateTime::from_timestamp(ts, 0).unwrap()),
            last_login_ip: row.get("last_login_ip"),
            created_at: DateTime::from_timestamp(row.get("created_at"), 0).unwrap(),
            updated_at: DateTime::from_timestamp(row.get("updated_at"), 0).unwrap(),
            deleted_at: row.get::<_, Option<i64>>("deleted_at")
                .map(|ts| DateTime::from_timestamp(ts, 0).unwrap()),
        }
    }
}
```

### 8.2 AuditLogEntry struct

```rust
// src/db/audit.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: i64,
    pub user_id: Option<i64>,
    pub username: Option<String>,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub cluster_id: Option<i64>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub request_id: Option<String>,
    pub result: AuditResult,
    pub error_message: Option<String>,
    pub details: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuditResult {
    Success,
    Failure,
    Denied,
}

// Helper to create audit entry from request context
pub struct AuditEntryBuilder<'a> {
    user_id: Option<i64>,
    username: Option<&'a str>,
    action: &'a str,
    target_type: Option<&'a str>,
    target_id: Option<&'a str>,
    cluster_id: Option<i64>,
    request_id: Option<&'a str>,
}

impl<'a> AuditEntryBuilder<'a> {
    pub fn new(action: &'a str) -> Self {
        Self {
            user_id: None,
            username: None,
            action,
            target_type: None,
            target_id: None,
            cluster_id: None,
            request_id: None,
        }
    }
    
    pub fn user(mut self, user: &User) -> Self {
        self.user_id = Some(user.id);
        self.username = Some(&user.username);
        self
    }
    
    pub fn target(mut self, target_type: &'a str, target_id: &'a str) -> Self {
        self.target_type = Some(target_type);
        self.target_id = Some(target_id);
        self
    }
    
    pub fn cluster(mut self, cluster_id: i64) -> Self {
        self.cluster_id = Some(cluster_id);
        self
    }
    
    pub async fn log(self, db: &SqlitePool, ip: Option<&str>, ua: Option<&str>, result: AuditResult) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO audit_log (
                user_id, username, action, target_type, target_id,
                cluster_id, ip_address, user_agent, request_id, result
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(self.user_id)
        .bind(self.username)
        .bind(self.action)
        .bind(self.target_type)
        .bind(self.target_id)
        .bind(self.cluster_id)
        .bind(ip)
        .bind(ua)
        .bind(self.request_id)
        .bind(match result {
            AuditResult::Success => "success",
            AuditResult::Failure => "failure",
            AuditResult::Denied => "denied",
        })
        .execute(db)
        .await?;
        Ok(())
    }
}
```

### 8.3 Cluster struct

```rust
// src/db/clusters.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cluster {
    pub id: i64,
    pub name: String,
    pub url: String,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_encrypted: Vec<u8>,  // encrypted bytes
    pub realm: String,
    pub verify_tls: bool,
    pub ca_cert_pem: Option<String>,
    pub is_active: bool,
    pub last_connected_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStatus {
    pub cluster_id: i64,
    pub reachable: bool,
    pub last_poll_at: Option<i64>,
    pub last_success_at: Option<i64>,
    pub last_error: Option<String>,
    pub version: Option<String>,
    pub nodes_count: Option<i32>,
    pub vms_count: Option<i32>,
}
```

---

## 9. Retention & Archival Policy

### 9.1 Retention periods

| Data | Retention | Method |
|---|---|---|
| `users` (active) | Forever | — |
| `users` (deleted) | 90 days | Hard delete |
| `refresh_tokens` (active) | Until expiry + 24h grace | Auto-cleanup |
| `refresh_tokens` (revoked) | 30 days | Auto-cleanup |
| `api_keys` (active) | Until expiry + 30d grace | Auto-cleanup |
| `api_keys` (revoked) | 90 days | Auto-cleanup |
| `webauthn_credentials` | Until user deletion | Cascade |
| `totp_secrets` | Until user deletion | Cascade |
| `oauth_state` | 10 minutes | Auto-cleanup |
| **`audit_log`** | **90 days** (configurable) | Move to archive, then delete |
| `cluster_status_cache` | Always latest | UPSERT |

### 9.2 Retention job (cron)

```rust
// src/jobs/retention.rs
pub async fn run_retention_job(db: &SqlitePool) -> Result<RetentionReport> {
    let report = RetentionReport::default();
    
    // 1. Refresh tokens
    let n = sqlx::query(
        "DELETE FROM refresh_tokens
         WHERE (expires_at < unixepoch() - 604800)
            OR (used_at IS NOT NULL AND used_at < unixepoch() - 86400)
            OR (revoked_at IS NOT NULL AND revoked_at < unixepoch() - 2592000)"
    ).execute(db).await?.rows_affected();
    report.refresh_tokens_deleted = n;
    
    // 2. API keys (similar)
    // ...
    
    // 3. OAuth state
    let n = sqlx::query("DELETE FROM oauth_state WHERE expires_at < unixepoch()")
        .execute(db).await?.rows_affected();
    report.oauth_state_deleted = n;
    
    // 4. Audit log (archive or delete)
    let retention_days: i64 = system_config::get(db, "audit.retention_days")
        .await?
        .parse()?;
    if retention_days > 0 {
        // Archive first
        sqlx::query(
            "INSERT INTO audit_log_archive
             SELECT * FROM audit_log
             WHERE created_at < unixepoch() - (? * 86400)"
        ).bind(retention_days).execute(db).await?;
        
        // Then delete
        let n = sqlx::query(
            "DELETE FROM audit_log
             WHERE created_at < unixepoch() - (? * 86400)"
        ).bind(retention_days).execute(db).await?.rows_affected();
        report.audit_log_deleted = n;
    }
    
    Ok(report)
}
```

### 9.3 Schedule

Run via `tokio::spawn` loop with `croner` crate:

```rust
// Run daily at 03:00 UTC
let schedule = Schedule::new("0 3 * * *").await?;
schedule.run_forever(|| async {
    match run_retention_job(&db).await {
        Ok(r) => tracing::info!("retention job completed: {:?}", r),
        Err(e) => tracing::error!("retention job failed: {:?}", e),
    }
}).await;
```

---

## 10. Backup & Restore Strategy

### 10.1 Backup script

```bash
#!/bin/bash
# scripts/backup.sh
set -euo pipefail

DATA_DIR="${MOXUI_DATA_DIR:-/home/moxui/data}"
BACKUP_DIR="${MOXUI_BACKUP_DIR:-/backups/moxui}"
RETENTION_DAYS="${MOXUI_BACKUP_RETENTION:-30}"
TIMESTAMP=$(date -u +%Y%m%dT%H%M%SZ)
BACKUP_FILE="${BACKUP_DIR}/moxui-${TIMESTAMP}.tar.gz"

mkdir -p "${BACKUP_DIR}"

# 1. SQLite hot backup using .backup command (safe with WAL)
sqlite3 "${DATA_DIR}/moxui.db" ".backup '${DATA_DIR}/moxui-backup.db'"

# 2. Tar with config (excluding secrets)
tar -czf "${BACKUP_FILE}" \
    -C "${DATA_DIR}" \
    moxui-backup.db \
    moxui.db-wal moxui.db-shm \
    --transform 's|^moxui-backup.db$|moxui.db|' \
    || { echo "Backup failed"; exit 1; }

# 3. Encrypt with age (optional but recommended)
if [ -n "${MOXUI_BACKUP_AGE_KEY:-}" ]; then
    age -r "${MOXUI_BACKUP_AGE_KEY}" < "${BACKUP_FILE}" > "${BACKUP_FILE}.age"
    rm "${BACKUP_FILE}"
    BACKUP_FILE="${BACKUP_FILE}.age"
fi

# 4. Verify integrity
if [ -f "${BACKUP_FILE}" ]; then
    SIZE=$(stat -c%s "${BACKUP_FILE}")
    echo "Backup complete: ${BACKUP_FILE} (${SIZE} bytes)"
fi

# 5. Upload to remote (optional)
if [ -n "${MOXUI_BACKUP_REMOTE:-}" ]; then
    rsync -avz "${BACKUP_FILE}" "${MOXUI_BACKUP_REMOTE}"
fi

# 6. Cleanup old backups
find "${BACKUP_DIR}" -name "moxui-*.tar.gz*" -mtime +${RETENTION_DAYS} -delete
```

### 10.2 Restore script

```bash
#!/bin/bash
# scripts/restore.sh
set -euo pipefail

BACKUP_FILE="$1"
DATA_DIR="${MOXUI_DATA_DIR:-/home/moxui/data}"

if [ ! -f "${BACKUP_FILE}" ]; then
    echo "Usage: $0 <backup-file>"
    exit 1
fi

# 1. Stop MoxUI
docker stop moxui || systemctl stop moxui || true

# 2. Backup current data (just in case)
mv "${DATA_DIR}/moxui.db" "${DATA_DIR}/moxui.db.bak.$(date +%s)"

# 3. Decrypt if needed
if [[ "${BACKUP_FILE}" == *.age ]]; then
    TMP=$(mktemp)
    age -d -i "${MOXUI_BACKUP_AGE_KEY}" < "${BACKUP_FILE}" > "${TMP}.tar.gz"
    BACKUP_FILE="${TMP}.tar.gz"
fi

# 4. Extract
tar -xzf "${BACKUP_FILE}" -C "${DATA_DIR}"

# 5. Verify
sqlite3 "${DATA_DIR}/moxui.db" "PRAGMA integrity_check;"

# 6. Start MoxUI
docker start moxui || systemctl start moxui

echo "Restore complete"
```

### 10.3 Backup schedule (K8s CronJob)

```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: moxui-backup
spec:
  schedule: "0 2 * * *"  # daily at 02:00
  jobTemplate:
    spec:
      template:
        spec:
          containers:
          - name: backup
            image: ghcr.io/kungjom26/moxui:latest
            command: ["/usr/local/bin/moxui-backup"]
            env:
            - name: MOXUI_DATA_DIR
              value: /data
            - name: MOXUI_BACKUP_REMOTE
              value: s3://my-bucket/moxui-backups/
            - name: AWS_ACCESS_KEY_ID
              valueFrom:
                secretKeyRef:
                  name: moxui-backup-secrets
                  key: aws-key
            - name: AWS_SECRET_ACCESS_KEY
              valueFrom:
                secretKeyRef:
                  name: moxui-backup-secrets
                  key: aws-secret
            volumeMounts:
            - name: data
              mountPath: /data
          restartPolicy: OnFailure
          volumes:
          - name: data
            persistentVolumeClaim:
              claimName: moxui-data
```

### 10.4 Disaster recovery targets

| RPO (data loss tolerance) | RTO (downtime tolerance) | Method |
|---|---|---|
| 1 hour | 30 min | Backup every hour, restore from latest |
| 15 min | 5 min | Streaming replication (Litestream) |
| 0 (zero loss) | < 1 min | HA cluster (2+ instances, but complex) |

**MoxUI default:** RPO 24h, RTO 30 min — daily backup sufficient for admin tool

---

## 11. Security Considerations

### 11.1 Encryption at rest

| Data | Encrypted? | How |
|---|---|---|
| User passwords | ✅ bcrypt | One-way hash (cannot decrypt) |
| Refresh tokens (DB) | ✅ SHA-256 hash | Cannot retrieve from DB |
| API keys (DB) | ✅ SHA-256 hash | Cannot retrieve from DB |
| Cluster passwords | ✅ ChaCha20-Poly1305 | Symmetric, key from env var |
| LDAP bind password | ✅ ChaCha20-Poly1305 | Same |
| **Audit log** | ❌ plaintext | Append-only = integrity, not confidentiality |
| Session cookies | ✅ JWT signed | RS256 private key |

### 11.2 Encryption key management

**Key derivation:**
```rust
// src/crypto.rs
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use chacha20poly1305::aead::{Aead, NewAead};

pub fn derive_key(master_key: &[u8], purpose: &[u8]) -> [u8; 32] {
    // HKDF-SHA256
    let hk = Hkdf::<Sha256>::new(None, master_key);
    let mut okm = [0u8; 32];
    hk.expand(purpose, &mut okm).unwrap();
    okm
}

pub fn encrypt(master_key: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    let key = derive_key(master_key, b"moxui-encryption-v1");
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
    let nonce = Nonce::from_slice(b"unique12byte");  // 12 bytes — should be random!
    // NOTE: production uses random nonce, prefix to ciphertext
    Ok(cipher.encrypt(nonce, plaintext)?)
}
```

**Master key** มาจาก env var: `MOXUI_MASTER_KEY` (32 bytes, base64-encoded)
- Rotate โดย decrypt ทั้งหมด → re-encrypt ด้วย key ใหม่
- Documented ใน `docs/key-rotation.md`

### 11.3 Threat model recap

| Threat | Mitigation |
|---|---|
| DB file leak | Sensitive fields encrypted (passwords hashed, cluster creds encrypted) |
| SQL injection | Parameterized queries only (rusqlite prepared statements) |
| Audit log tampering | Triggers prevent UPDATE/DELETE |
| Privilege escalation | RBAC enforced at API + UI |
| Session hijacking | Short JWT TTL + refresh token rotation + IP tracking |
| Brute force | Account lockout + 2FA required for admin |
| Insider threat | Audit log immutable + RBAC |

### 11.4 Compliance

- **GDPR** — audit log เก็บ 90 days, user delete = soft delete 30 days, hard delete 90 days
- **SOC 2** — audit log immutable, access control, encryption
- **HIPAA** (ถ้าจำเป็น) — encryption at rest + in transit, access logging

---

## 📎 Appendix — Quick Reference

### File locations

```
~/projects/moxui/
├── src/
│   ├── db/
│   │   ├── mod.rs              # connection pool + migrations runner
│   │   ├── users.rs            # User struct + queries
│   │   ├── audit.rs            # AuditLogEntry + AuditEntryBuilder
│   │   ├── refresh_tokens.rs
│   │   ├── api_keys.rs
│   │   ├── clusters.rs
│   │   └── ...
│   ├── crypto.rs               # encryption helpers
│   └── jobs/
│       └── retention.rs        # cleanup job
├── migrations/
│   ├── V001__initial_schema.sql
│   ├── V002__add_webauthn.sql
│   ├── V003__add_auth_features.sql
│   └── V004__add_clusters_and_audit_triggers.sql
└── scripts/
    ├── backup.sh
    └── restore.sh
```

### Common operations

```rust
// Initialize DB
let db = SqlitePoolOptions::new()
    .max_connections(8)
    .connect_with(connect_with_url(&url)?)
    .await?;
run_migrations(&db).await?;

// Open connection with PRAGMAs
async fn open_connection(url: &str) -> Result<SqlitePool> {
    SqlitePoolOptions::new()
        .max_connections(8)
        .after_connect(|conn| Box::pin(async move {
            sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await?;
            sqlx::query("PRAGMA journal_mode = WAL").execute(&mut *conn).await?;
            // ... other PRAGMAs
            Ok(())
        }))
        .connect(url)
        .await
}
```

### Useful indexes cheat sheet

| Table | Most-used indexes |
|---|---|
| `users` | `username` (UNIQUE), `email` (UNIQUE), `role` (partial) |
| `refresh_tokens` | `token_hash` (UNIQUE), `user_id`, `expires_at` |
| `api_keys` | `key_hash` (UNIQUE), `user_id` |
| `audit_log` | `(user_id, created_at DESC)`, `(action, created_at DESC)`, `(cluster_id, created_at DESC)`, `created_at DESC` |
| `cluster_permissions` | `(user_id, cluster_id)` PK, `cluster_id` |

---

**Last updated:** 2026-06-20
**Status:** Design complete — implementation starts Phase 0 Day 1
**Next review:** After Phase 2 (MVP) — validate with real workload