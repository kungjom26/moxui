# MoxUI — Backup & Disaster Recovery Runbook

This directory contains scripts for backing up and restoring MoxUI data (SQLite
database, configuration files, and audit logs) and for syncing backups to
S3-compatible object storage.

## Overview

```
scripts/
├── backup.sh       # Create a timestamped local backup tarball
├── restore.sh      # Restore MoxUI from a backup tarball
├── s3-backup.sh    # Upload backups to S3-compatible storage (AWS S3, MinIO, etc.)
└── README.md       # This file — DR runbook
```

## Backup Architecture

```
┌─────────────┐     ┌──────────────┐     ┌─────────────────┐
│  MoxUI App  │     │ backup.sh   │     │  S3 storage     │
│  (SQLite DB)│────>│ (cron job)  │────>│  (AWS/MinIO)    │
│  /var/lib/  │     │ /var/backups/│    │ s3://bucket/    │
│  moxui/data │     │ moxui-*.gz  │     │ moxui/backup.gz │
└─────────────┘     └──────────────┘     └─────────────────┘
                           │
                    ┌──────┘
                    ▼
              ┌──────────────┐
              │ restore.sh   │  (manual DR)
              │ restore from │
              │ backup       │
              └──────────────┘
```

## Quick Start

### Create a backup

```bash
# Default paths (requires root or appropriate permissions)
sudo ./scripts/backup.sh

# Custom paths (run as moxui user)
./scripts/backup.sh \
  -d /var/lib/moxui/data \
  -c /etc/moxui \
  -o /home/moxui/backups \
  --keep 14

# Output: /home/moxui/backups/moxui-backup-20250622T120000Z.tar.gz
```

### Restore from a backup

```bash
# Stop service, restore, restart
sudo ./scripts/restore.sh /var/backups/moxui/moxui-backup-20250622T120000Z.tar.gz

# Force restore without confirmation
sudo ./scripts/restore.sh /tmp/backup.tar.gz -f

# Restore to a different location, skip service management
./scripts/restore.sh /tmp/backup.tar.gz \
  -d /tmp/moxui-data \
  -c /tmp/moxui-config \
  -s ""
```

### Upload backup to S3

```bash
# Export credentials (or configure ~/.aws/credentials)
export AWS_ACCESS_KEY_ID="your-access-key"
export AWS_SECRET_ACCESS_KEY="your-secret-key"
export S3_ENDPOINT="https://s3.eu-west-1.amazonaws.com"  # or MinIO URL

# Upload the most recent backup
./scripts/s3-backup.sh

# Upload with options
./scripts/s3-backup.sh \
  -b moxui-backups \
  -e https://play.min.io \
  -d /var/backups/moxui \
  --cleanup-remote 60 \
  --dry-run

# Upload a specific file
./scripts/s3-backup.sh -f /tmp/emergency-backup.tar.gz
```

### Cron job setup

```bash
# Add to crontab (runs daily at 2 AM, then syncs to S3)
0 2 * * * /opt/moxui/scripts/backup.sh && /opt/moxui/scripts/s3-backup.sh

# Log to syslog for monitoring
0 2 * * * /opt/moxui/scripts/backup.sh >> /var/log/moxui-backup.log 2>&1
```

## Disaster Recovery Scenarios

### Scenario 1: Database corruption

**Symptom:** MoxUI returns 500 errors or fails to start.

**Recovery:**
```bash
# 1. Find the most recent backup
ls -lt /var/backups/moxui/

# 2. Restore from backup
sudo ./scripts/restore.sh /var/backups/moxui/moxui-backup-20250622T120000Z.tar.gz

# 3. Verify
curl http://localhost:8080/health
curl http://localhost:8080/api/v1/vms  # should return valid JSON
```

### Scenario 2: Complete node failure

**Symptom:** Server is unreachable; data is lost.

**Recovery (new node):**
```bash
# 1. Provision a new server and install MoxUI
# 2. Copy backup from S3
aws s3 cp s3://moxui-backups/moxui/moxui-backup-20250622T120000Z.tar.gz /tmp/

# 3. Restore (install scripts first)
tar -xzf /tmp/backup.tar.gz -C /tmp/restore/
sudo ./scripts/restore.sh /tmp/backup.tar.gz

# 4. Recreate secrets (JWT keys, cluster passwords)
#    These are NOT included in the backup for security reasons.
#    Refer to your secret management system (Vault, K8s Secrets, etc.)
```

### Scenario 3: Misconfiguration

**Symptom:** MoxUI starts but behaves incorrectly (wrong clusters, auth issues).

**Recovery:**
```bash
# 1. Identify the last working config backup
# 2. Restore config only
sudo ./scripts/restore.sh /var/backups/moxui/moxui-backup-YYYYMMDDTHHMMSSZ.tar.gz \
  -d /dev/null  # skip data restore
  # Note: edit restore.sh to skip data dir if needed — currently restores both
```

### Scenario 4: Accidental data deletion

**Symptom:** VMs, users, or other records missing from MoxUI.

**Recovery:**
```bash
# 1. Immediately stop the service to prevent writes
sudo systemctl stop moxui

# 2. Find the most recent backup BEFORE the deletion occurred
# 3. Check the pre-restore snapshot that restore.sh creates automatically
ls -lt /tmp/moxui-pre-restore-*.tar.gz

# 4. Restore
sudo ./scripts/restore.sh /var/backups/moxui/backup-before-deletion.tar.gz
```

## Backup Contents

A backup tarball contains:

```
moxui.db            # SQLite database (consistent snapshot via .backup)
moxui.db-wal        # WAL file (if using WAL mode)
moxui.db-shm        # Shared memory file (if using WAL mode)
moxui.db.audit      # Audit log database
config/
├── config.yaml     # Main configuration
├── config.local.yaml  # Local overrides
└── .env            # Environment variables (secrets excluded)
```

**NOT included in backups (security):**
- JWT private/public keys (.pem files)
- Cluster passwords
- SSL/TLS certificates
- OIDC client secrets

These should be managed via your secret management system (Kubernetes Secrets,
HashiCorp Vault, Sealed Secrets, etc.).

## Dependencies

| Script | Dependencies |
|---|---|
| `backup.sh` | `bash`, `sqlite3` (recommended), `tar`, `gzip` |
| `restore.sh` | `bash`, `tar`, `gzip`, `systemctl` (optional) |
| `s3-backup.sh` | `bash`, `aws-cli` **OR** `mc` (MinIO Client) |

## Testing Backup Integrity

```bash
# Verify archive
gzip -t /var/backups/moxui/moxui-backup-*.tar.gz

# List contents without extracting
tar -tzf /var/backups/moxui/moxui-backup-*.tar.gz

# Extract and verify the database
tar -xzf backup.tar.gz -C /tmp/check/ moxui.db
sqlite3 /tmp/check/moxui.db "PRAGMA integrity_check;"
```
