# State Machines — VM & LXC Lifecycle

> **Last updated:** 2026-06-20

---

## 1. VM State Machine (QEMU)

```
┌─────────┐
│         │
│  (new)  │   ← created via API (v1.1+) or Proxmox UI
│         │
└────┬────┘
     │
     ▼
┌─────────┐    start     ┌─────────┐
│         │ ────────────→ │         │
│ stopped │               │ running │ ─┐
│         │ ←──────────── │         │  │ shutdown (graceful)
└─────────┘    stop       └─────────┘  │
     ▲                       │  │     │
     │                       │  │     ▼
     │                       │  │  ┌─────────┐
     │                       │  │  │         │
     │                       │  └──│shutdown │ ─┐
     │                       │     │         │  │ timeout
     │                       │     └─────────┘  ▼
     │                       │                ┌─────────┐
     │                       │                │         │
     │                       │                │ stopped │
     │                       │                │         │
     │                       │                └─────────┘
     │                       │
     │   pause               │   resume
     │   ┌─────────┐         │
     └───┤         │←────────┘
         │ paused  │
         │         │ ───────────→ running
         └─────────┘
         
         
         ╔═════════╗
         ║ ERROR   ║ ← crashes, hardware failure
         ║ states  ║
         ╚═════════╝
```

### State definitions

| State | Proxmox | MoxUI action |
|---|---|---|
| **stopped** | `stopped` | Allow start, delete |
| **running** | `running` | Allow stop, pause, shutdown, reboot, snapshot |
| **paused** | `paused` | Allow resume, stop (force) |
| **shutdown** | `shutdown` | Wait for timeout → becomes `stopped` |
| **migrating** | `running` (with flag) | Wait for completion |
| **error** | (no state) | Show error UI, suggest manual intervention |

### Allowed transitions (RBAC enforced)

| From | To | Who can do it |
|---|---|---|
| stopped | running | operator, admin |
| running | stopped | operator, admin |
| running | paused | operator, admin |
| paused | running | operator, admin |
| running | shutdown | operator, admin |
| shutdown | running | (auto — power on after ACPI) |
| any | deleted | admin only |

### State change sequence (running → stopped)

```
User clicks "Stop"
       ↓
POST /api/v1/vms/103/stop
       ↓
MoxUI sends POST /nodes/pve11/qemu/103/status/stop
       ↓
Proxmox sends ACPI shutdown signal
       ↓
VM receives signal, OS shuts down gracefully
       ↓
VM stops (or timeout → force stop)
       ↓
Proxmox returns task UPID
       ↓
MoxUI polls task status until "stopped"
       ↓
MoxUI invalidates cache
       ↓
Frontend receives update, shows VM as stopped
```

---

## 2. LXC State Machine (Container)

```
┌─────────┐
│         │
│ stopped │
│         │
└────┬────┘
     │
     ▼ (start)
┌─────────┐
│         │ ─── shutdown ───→ (timeout 60s)
│ running │                       │
│         │                       ▼
└────┬────┘                  ┌─────────┐
     │                       │         │
     │ (pause — not          │ stopped │
     │  supported by LXC)    │         │
     │                       └─────────┘
     │
     ▼ (stop)
┌─────────┐
│         │
│ stopped │
│         │
└─────────┘
```

**Note:** LXC containers do NOT support pause/resume — only start/stop/shutdown/reboot

---

## 3. Task State Machine (Long-running Operations)

```
┌─────────┐
│         │
│  new    │   ← created
│         │
└────┬────┘
     │
     ▼
┌─────────┐
│         │ ←── running ────→ stopping (on cancel, future)
│ running │                    │
│         │                    │
└────┬────┘                    │
     │                         │
     ├──→ OK ─→ stopped        │
     │                         │
     ├──→ ERROR ──→ stopped    │
     │                         │
     └──→ TIMEOUT ─→ stopped   │
                               │
                               ▼
                          (terminate)
```

### Task states (from Proxmox)

| Status | Description |
|---|---|
| `running` | Task is in progress |
| `stopped` | Task completed (check exitstatus) |
| `unknown` | Lost track (rare) |

### Exit statuses

| Exitstatus | Meaning |
|---|---|
| `OK` | Success |
| `WARNINGS` | Completed but with warnings |
| `ERROR` | Failed |

### Task lifecycle in MoxUI

```
1. Coder submits action (e.g., POST .../start)
2. Proxmox returns UPID
3. MoxUI returns 202 Accepted + UPID to frontend
4. Frontend shows "VM starting..." toast + spinner on VM
5. Frontend polls GET /api/v1/tasks/{upid} every 1s
6. Backend polls Proxmox GET /nodes/{node}/tasks/{upid}/status
7. When status = "stopped" → check exitstatus
8. If OK → invalidate cache, frontend updates to "running"
9. If ERROR → frontend shows error toast
```

---

## 4. Backup Job State Machine

```
┌─────────┐
│         │
│ queued  │
│         │
└────┬────┘
     │
     ▼
┌─────────┐
│         │
│  init   │ ← Proxmox creating snapshot
│         │
└────┬────┘
     │
     ▼
┌─────────┐
│         │
│ running │ ← backup in progress
│         │
└────┬────┘
     │
     ▼
┌─────────┐
│         │
│ verify  │ ← verify checksum
│         │
└────┬────┘
     │
     ├──→ OK ─────→ ┌─────────┐
     │               │         │
     │               │  done   │
     │               │         │
     │               └─────────┘
     │
     └──→ ERROR ──→ ┌─────────┐
                     │         │
                     │ failed  │
                     │         │
                     └─────────┘
```

### Backup modes (Proxmox vzdump)

| Mode | Description | Allowed states |
|---|---|---|
| `snapshot` | Create snapshot, backup from snapshot | running |
| `suspend` | Suspend VM, backup, resume | running |
| `stop` | Stop VM, backup, restart | running |
| `none` | No consistency (raw disk) | stopped |

---

## 5. Auth Flow State Machine (Login)

```
┌─────────┐
│         │
│ unknown │
│         │
└────┬────┘
     │
     ▼ (POST /auth/login with username + password)
┌─────────┐
│         │
│ creds  │ ← verify password
│  ok?   │
└────┬───┘
     │
     ├─ NO ──────→ ┌─────────┐
     │             │         │
     │             │ failed  │ ──→ (increment counter, lock if >= 5)
     │             │         │
     │             └─────────┘
     │
     ▼ YES
┌─────────┐
│         │
│ check   │ ← is 2FA enabled?
│  2FA    │
└────┬───┘
     │
     ├─ NO ──────→ ┌─────────┐ ──→ issue JWT + refresh
     │             │         │
     │             │ success │
     │             │         │
     │             └─────────┘
     │
     ▼ YES
┌─────────┐
│         │
│ need    │ ← ask user for TOTP / WebAuthn
│  2FA    │
└────┬───┘
     │
     ▼ (POST /auth/2fa/verify)
┌─────────┐
│         │
│verify   │ ← verify code
│  2FA    │
└────┬───┘
     │
     ├─ NO ──────→ (back to "need 2FA")
     │
     ▼ YES ──→ ┌─────────┐
                │ success │
                └─────────┘
```

### Account states

| State | Description |
|---|---|
| `active` | Normal user |
| `locked` | Failed login attempts exceeded threshold |
| `inactive` | Disabled by admin |
| `deleted` | Soft-deleted (recoverable 30 days) |

---

## 6. VM Snapshot State Machine

```
       ┌──────────┐
       │          │
       │  parent  │ (VM at point T0)
       │          │
       └────┬─────┘
            │ (snapshot create)
            ▼
       ┌──────────┐
       │          │
       │ snap-001 │ (snapshot at T1)
       │          │
       └────┬─────┘
            │ (snapshot create)
            ▼
       ┌──────────┐
       │          │
       │ snap-002 │ (snapshot at T2)
       │          │
       └────┬─────┘
            │ (rollback to snap-001)
            ▼
       ┌──────────┐
       │          │
       │ snap-001 │ ← VM is now at T1 state
       │ (active) │    snap-002 still exists but not current
       │          │
       └────┬─────┘
            │ (delete snap-002)
            ▼
       (gone)
```

### Snapshot operations

| Op | Effect | Notes |
|---|---|---|
| **create** | Add new snapshot | Optional RAM state |
| **rollback** | Restore VM to snapshot state | VM must be stopped first |
| **delete** | Remove snapshot | May merge children |

---

## 7. Cache State Machine

```
┌─────────┐
│         │
│  fresh  │ ← just inserted
│         │
└────┬────┘
     │
     ▼ (TTL = 5s)
┌─────────┐
│         │
│  valid  │ ← within TTL, accessible
│         │
└────┬────┘
     │
     ▼ (TTL expired)
┌─────────┐
│         │
│ stale   │ ← will refresh on next access
│         │
└────┬────┘
     │
     ▼ (next read)
┌─────────┐
│         │
│ refresh │ ← re-fetch from Proxmox
│         │
└────┬────┘
     │
     ▼
   fresh
```

### Cache invalidation triggers

| Trigger | What to invalidate |
|---|---|
| VM start/stop/reboot/delete | VM list cache for that cluster |
| VM config change | VM detail cache |
| Node status change | Cluster stats cache |
| Storage change | Storage list cache |

---

**See also:**
- [`ARCHITECTURE.md`](../ARCHITECTURE.md) — high-level
- [`request-flows.md`](./request-flows.md) — detailed flows