#!/usr/bin/env bash
# ===========================================================================
# MoxUI — Restore Script
#
# Restores a MoxUI backup tarball to the specified data and config directories.
# Stops the moxui service before restoring and restarts it after.
#
# Usage:
#   ./scripts/restore.sh <backup-file> [options]
#
# Arguments:
#   backup-file           Path to the backup tarball (.tar.gz)
#
# Options:
#   -d, --data-dir DIR    MoxUI data directory (default: /var/lib/moxui/data)
#   -c, --config-dir DIR  MoxUI config directory (default: /etc/moxui)
#   -s, --service NAME    MoxUI systemd service name (default: moxui)
#                            Set to "" to skip service management
#   -f, --force           Skip confirmation prompt
#   -h, --help            Show this help message
#
# Examples:
#   ./scripts/restore.sh /var/backups/moxui/moxui-backup-20250622T120000Z.tar.gz
#   ./scripts/restore.sh /tmp/backups/backup.tar.gz -d /home/moxui/data -f
# ===========================================================================
set -euo pipefail

# ── Defaults ─────────────────────────────────────────────────────────────
DATA_DIR="/var/lib/moxui/data"
CONFIG_DIR="/etc/moxui"
SERVICE_NAME="moxui"
FORCE=false

# ── Parse options ────────────────────────────────────────────────────────
usage() {
  sed -n 's/^# \{0,2\}//p' "$0" | sed -n '2,/^$/p'
  exit 0
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -d|--data-dir)     DATA_DIR="$2";    shift 2 ;;
    -c|--config-dir)   CONFIG_DIR="$2";  shift 2 ;;
    -s|--service)      SERVICE_NAME="$2"; shift 2 ;;
    -f|--force)        FORCE=true;       shift ;;
    -h|--help)         usage ;;
    -*)
      echo "Unknown option: $1" >&2
      usage
      ;;
    *)
      if [[ -z "${BACKUP_FILE:-}" ]]; then
        BACKUP_FILE="$1"
        shift
      else
        echo "Unexpected argument: $1" >&2
        usage
      fi
      ;;
  esac
done

# ── Validate ─────────────────────────────────────────────────────────────
if [[ -z "${BACKUP_FILE:-}" ]]; then
  echo "[ERROR] No backup file specified" >&2
  usage
fi

if [[ ! -f "$BACKUP_FILE" ]]; then
  echo "[ERROR] Backup file not found: ${BACKUP_FILE}" >&2
  exit 1
fi

# ── Confirmation ─────────────────────────────────────────────────────────
echo "=== MoxUI Restore ==="
echo "  Backup file: ${BACKUP_FILE}"
echo "  Data dir:    ${DATA_DIR}"
echo "  Config dir:  ${CONFIG_DIR}"
echo "  Service:     ${SERVICE_NAME:-"(manual)"}"
echo ""
echo "[WARN] This will OVERWRITE existing data in ${DATA_DIR}!"
echo "       A backup of the current state will be created automatically."

if [[ "$FORCE" != "true" ]]; then
  read -rp "Continue? [y/N] " CONFIRM
  if [[ "$CONFIRM" != "y" && "$CONFIRM" != "Y" ]]; then
    echo "Restore cancelled."
    exit 0
  fi
fi

# ── Verify archive ───────────────────────────────────────────────────────
echo ""
echo "[INFO] Verifying backup archive..."
if command -v gzip &>/dev/null; then
  if ! gzip -t "$BACKUP_FILE" 2>/dev/null; then
    echo "[ERROR] Backup archive is corrupt or invalid" >&2
    exit 1
  fi
fi

# Check archive contents
ARCHIVE_FILES=$(tar -tzf "$BACKUP_FILE" 2>/dev/null) || {
  echo "[ERROR] Cannot read archive contents — file may be invalid" >&2
  exit 1
}
echo "[OK]   Archive verified — contents:"
echo "$ARCHIVE_FILES" | sed 's/^/       /'

# ── Stop service ─────────────────────────────────────────────────────────
if [[ -n "$SERVICE_NAME" ]] && command -v systemctl &>/dev/null; then
  if systemctl is-active --quiet "$SERVICE_NAME"; then
    echo "[INFO] Stopping service ${SERVICE_NAME}..."
    systemctl stop "$SERVICE_NAME"
    echo "[OK]   Service stopped"
  else
    echo "[WARN] Service ${SERVICE_NAME} is not running"
  fi
fi

# ── Create pre-restore snapshot ──────────────────────────────────────────
TEMP_BACKUP=$(mktemp -d)
trap 'rm -rf "$TEMP_BACKUP"' EXIT

if [[ -d "$DATA_DIR" ]]; then
  PRE_RESTORE_SNAPSHOT="/tmp/moxui-pre-restore-$(date -u '+%Y%m%dT%H%M%SZ').tar.gz"
  tar -czf "$PRE_RESTORE_SNAPSHOT" -C "$DATA_DIR" . 2>/dev/null || true
  echo "[INFO] Pre-restore snapshot saved: ${PRE_RESTORE_SNAPSHOT}"
fi

# ── Extract archive to temp dir ──────────────────────────────────────────
EXTRACT_DIR=$(mktemp -d)
tar -xzf "$BACKUP_FILE" -C "$EXTRACT_DIR"
echo "[OK]   Archive extracted to temporary directory"

# ── Restore data directory ───────────────────────────────────────────────
mkdir -p "$DATA_DIR"
# Clear existing data (skip hidden files like .git)
find "$DATA_DIR" -type f -not -path '*/\.*' -delete 2>/dev/null || true

# Copy .db and .db.* files
for f in "$EXTRACT_DIR"/moxui.db*; do
  if [[ -f "$f" ]]; then
    cp "$f" "${DATA_DIR}/$(basename "$f")"
    echo "[OK]   Restored data: $(basename "$f")"
  fi
done

# ── Restore config directory ─────────────────────────────────────────────
if [[ -d "$EXTRACT_DIR/config" ]]; then
  mkdir -p "$CONFIG_DIR"
  for f in "$EXTRACT_DIR/config"/*; do
    if [[ -f "$f" ]]; then
      cp "$f" "${CONFIG_DIR}/$(basename "$f")"
      echo "[OK]   Restored config: $(basename "$f")"
    fi
  done
else
  echo "[WARN] No config files found in backup — skipping config restore"
fi

# ── Clean up ─────────────────────────────────────────────────────────────
rm -rf "$EXTRACT_DIR"

# ── Restart service ──────────────────────────────────────────────────────
if [[ -n "$SERVICE_NAME" ]] && command -v systemctl &>/dev/null; then
  echo "[INFO] Starting service ${SERVICE_NAME}..."
  systemctl start "$SERVICE_NAME"

  # Wait for service to become healthy
  sleep 2
  if systemctl is-active --quiet "$SERVICE_NAME"; then
    echo "[OK]   Service started successfully"
  else
    echo "[ERROR] Service failed to start — check 'journalctl -u ${SERVICE_NAME}'" >&2
    exit 1
  fi
fi

echo ""
echo "[INFO] Restore completed successfully."
echo "       Pre-restore snapshot: ${PRE_RESTORE_SNAPSHOT:-"(none)"}"
