#!/usr/bin/env bash
# ===========================================================================
# MoxUI — Backup Script
#
# Exports the SQLite database and configuration archive to a timestamped
# tarball. Designed to be run as a cron job.
#
# Usage:
#   ./scripts/backup.sh [options]
#
# Options:
#   -d, --data-dir DIR    MoxUI data directory (default: /var/lib/moxui/data)
#   -c, --config-dir DIR  MoxUI config directory (default: /etc/moxui)
#   -o, --output-dir DIR  Backup output directory (default: /var/backups/moxui)
#   -n, --name PREFIX     Backup filename prefix (default: moxui-backup)
#   -k, --keep N          Number of recent backups to keep (default: 7)
#   -h, --help            Show this help message
#
# Examples:
#   ./scripts/backup.sh                                         # defaults
#   ./scripts/backup.sh -d /home/moxui/data -o /tmp/backups     # custom paths
#   ./scripts/backup.sh -k 14                                   # keep 14 days
# ===========================================================================
set -euo pipefail

# ── Defaults ─────────────────────────────────────────────────────────────
DATA_DIR="/var/lib/moxui/data"
CONFIG_DIR="/etc/moxui"
OUTPUT_DIR="/var/backups/moxui"
PREFIX="moxui-backup"
KEEP=7
TIMESTAMP=$(date -u '+%Y%m%dT%H%M%SZ')

# ── Parse options ────────────────────────────────────────────────────────
usage() {
  sed -n 's/^# \{0,2\}//p' "$0" | sed -n '2,/^$/p'
  exit 0
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -d|--data-dir)     DATA_DIR="$2";    shift 2 ;;
    -c|--config-dir)   CONFIG_DIR="$2";  shift 2 ;;
    -o|--output-dir)   OUTPUT_DIR="$2";  shift 2 ;;
    -n|--name)         PREFIX="$2";      shift 2 ;;
    -k|--keep)         KEEP="$2";        shift 2 ;;
    -h|--help)         usage ;;
    *) echo "Unknown option: $1" >&2; usage ;;
  esac
done

# ── Validate paths ───────────────────────────────────────────────────────
if [[ ! -d "$DATA_DIR" ]]; then
  echo "[ERROR] Data directory does not exist: $DATA_DIR" >&2
  exit 1
fi

if [[ ! -d "$CONFIG_DIR" ]]; then
  echo "[WARN] Config directory does not exist: $CONFIG_DIR — excluding from backup" >&2
  CONFIG_DIR=""
fi

# ── Create output directory ──────────────────────────────────────────────
mkdir -p "$OUTPUT_DIR"

# ── Build archive filename ───────────────────────────────────────────────
ARCHIVE="${OUTPUT_DIR}/${PREFIX}-${TIMESTAMP}.tar.gz"
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

echo "[INFO] Starting MoxUI backup..."
echo "       Data dir:   ${DATA_DIR}"
echo "       Config dir: ${CONFIG_DIR:-"(skipped)"}"
echo "       Output:     ${ARCHIVE}"

# ── Stage database ───────────────────────────────────────────────────────
DB_FILE="${DATA_DIR}/moxui.db"
if [[ -f "$DB_FILE" ]]; then
  # Use sqlite3 to create a consistent snapshot (avoids corruption during write)
  SNAPSHOT="${TMP_DIR}/moxui.db"
  if command -v sqlite3 &>/dev/null; then
    sqlite3 "$DB_FILE" ".backup '${SNAPSHOT}'"
    echo "[OK]   Database snapshot created via sqlite3 .backup"
  else
    # Fallback: copy the file (best-effort, may be inconsistent)
    cp "$DB_FILE" "$SNAPSHOT"
    echo "[WARN] sqlite3 not found — database copied without snapshot (may be inconsistent)"
  fi
else
  echo "[WARN] No database file found at ${DB_FILE}"
fi

# ── Stage additional DB files (WAL, audit) ───────────────────────────────
for extra in moxui.db-wal moxui.db-shm moxui.db.audit; do
  src="${DATA_DIR}/${extra}"
  if [[ -f "$src" ]]; then
    cp "$src" "${TMP_DIR}/${extra}"
    echo "[OK]   Copied ${extra}"
  fi
done

# ── Stage config files ───────────────────────────────────────────────────
if [[ -n "$CONFIG_DIR" ]]; then
  CONFIG_TMP="${TMP_DIR}/config"
  mkdir -p "$CONFIG_TMP"
  # Copy config files (exclude .pem keys for security — those should be
  # managed separately via Kubernetes Secrets or a secret manager)
  shopt -s nullglob
  for f in "$CONFIG_DIR"/*.yaml "$CONFIG_DIR"/*.yml "$CONFIG_DIR"/*.toml "$CONFIG_DIR"/*.json; do
    cp "$f" "$CONFIG_TMP/"
    echo "[OK]   Copied config: $(basename "$f")"
  done
  # Also copy .env if present
  if [[ -f "$CONFIG_DIR/.env" ]]; then
    cp "$CONFIG_DIR/.env" "$CONFIG_TMP/"
    echo "[OK]   Copied .env"
  fi
  shopt -u nullglob
fi

# ── Create archive ───────────────────────────────────────────────────────
tar -czf "$ARCHIVE" -C "$TMP_DIR" .
echo "[OK]   Backup archive created: ${ARCHIVE}"
echo "       Size: $(du -h "$ARCHIVE" | cut -f1)"

# ── Verify archive integrity ─────────────────────────────────────────────
if command -v gzip &>/dev/null; then
  if gzip -t "$ARCHIVE" 2>/dev/null; then
    echo "[OK]   Archive integrity check passed"
  else
    echo "[ERROR] Archive integrity check FAILED" >&2
    exit 1
  fi
fi

# ── Rotate old backups ───────────────────────────────────────────────────
find "$OUTPUT_DIR" -maxdepth 1 -name "${PREFIX}-*.tar.gz" -type f \
  | sort \
  | head -n -"${KEEP}" \
  | while read -r old; do
      rm -f "$old"
      echo "[OK]   Removed old backup: $(basename "$old")"
    done

echo "[INFO] Backup completed successfully."
