#!/usr/bin/env bash
# ===========================================================================
# MoxUI — S3 Backup Upload Script
#
# Uploads MoxUI backup tarballs to S3-compatible storage (AWS S3, MinIO,
# DigitalOcean Spaces, Cloudflare R2, etc.).
#
# Can be used standalone or piped after backup.sh:
#   ./scripts/backup.sh && ./scripts/s3-backup.sh
#
# Usage:
#   ./scripts/s3-backup.sh [options]
#
# Options:
#   -b, --bucket BUCKET   S3 bucket name (default: moxui-backups)
#   -e, --endpoint URL    S3-compatible endpoint URL (default: from env)
#   -d, --backup-dir DIR  Local backup directory (default: /var/backups/moxui)
#   -r, --region REGION   AWS region (default: us-east-1)
#   -p, --prefix PREFIX   Object key prefix (default: moxui/)
#   -k, --keep N          Number of recent backups to keep on local disk
#                            after upload (default: 7)
#   -f, --file FILE       Upload a specific file instead of the newest
#   --cleanup-remote N    Remove remote backups older than N days (default: 30)
#   --dry-run             Show what would be uploaded without uploading
#   -h, --help            Show this help message
#
# Required environment variables (or ~/.aws/credentials):
#   AWS_ACCESS_KEY_ID       or  MC_HOST_moxui (MinIO client)
#   AWS_SECRET_ACCESS_KEY
#
# Dependencies:
#   - aws-cli (v2)
#   OR
#   - mc (MinIO client) for non-AWS S3 endpoints
#
# Examples:
#   ./scripts/s3-backup.sh
#   ./scripts/s3-backup.sh -b my-backups -e https://s3.eu-west-1.amazonaws.com
#   ./scripts/s3-backup.sh --dry-run
#   ./scripts/s3-backup.sh -f /tmp/backups/moxui-backup-20250622.tar.gz
#   # MinIO example:
#   S3_ENDPOINT=https://play.min.io S3_BUCKET=moxui ./scripts/s3-backup.sh
# ===========================================================================
set -euo pipefail

# ── Defaults ─────────────────────────────────────────────────────────────
BUCKET="${S3_BUCKET:-moxui-backups}"
ENDPOINT="${S3_ENDPOINT:-}"
BACKUP_DIR="/var/backups/moxui"
REGION="${AWS_REGION:-us-east-1}"
PREFIX="moxui/"
KEEP=7
CLEANUP_REMOTE_DAYS=30
DRY_RUN=false
SPECIFIC_FILE=""

# ── Parse options ────────────────────────────────────────────────────────
usage() {
  sed -n 's/^# \{0,2\}//p' "$0" | sed -n '2,/^$/p'
  exit 0
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -b|--bucket)         BUCKET="$2";          shift 2 ;;
    -e|--endpoint)       ENDPOINT="$2";        shift 2 ;;
    -d|--backup-dir)     BACKUP_DIR="$2";      shift 2 ;;
    -r|--region)         REGION="$2";          shift 2 ;;
    -p|--prefix)         PREFIX="$2";          shift 2 ;;
    -k|--keep)           KEEP="$2";            shift 2 ;;
    -f|--file)           SPECIFIC_FILE="$2";   shift 2 ;;
    --cleanup-remote)    CLEANUP_REMOTE_DAYS="$2"; shift 2 ;;
    --dry-run)           DRY_RUN=true;         shift ;;
    -h|--help)           usage ;;
    *) echo "Unknown option: $1" >&2; usage ;;
  esac
done

# ── Detect S3 client ─────────────────────────────────────────────────────
USE_AWS=false
USE_MC=false
AWS_CMD=""
MC_CMD=""

if command -v aws &>/dev/null; then
  USE_AWS=true
  AWS_CMD="aws"
  echo "[INFO] Using AWS CLI v$(aws --version 2>&1 | cut -d' ' -f1 | cut -d'/' -f2)"
elif command -v mc &>/dev/null; then
  USE_MC=true
  MC_CMD="mc"
  echo "[INFO] Using MinIO Client (mc)"
else
  echo "[ERROR] Neither 'aws' nor 'mc' CLI found." >&2
  echo "       Install one of:" >&2
  echo "         aws:  pip install awscli  OR  https://aws.amazon.com/cli/" >&2
  echo "         mc:   https://min.io/docs/minio/linux/reference/minio-mc.html" >&2
  exit 1
fi

# ── Find backup file ─────────────────────────────────────────────────────
if [[ -n "$SPECIFIC_FILE" ]]; then
  BACKUP_FILE="$SPECIFIC_FILE"
  if [[ ! -f "$BACKUP_FILE" ]]; then
    echo "[ERROR] Specified file not found: ${BACKUP_FILE}" >&2
    exit 1
  fi
else
  if [[ ! -d "$BACKUP_DIR" ]]; then
    echo "[ERROR] Backup directory not found: ${BACKUP_DIR}" >&2
    exit 1
  fi
  # Get the newest backup
  BACKUP_FILE=$(find "$BACKUP_DIR" -maxdepth 1 -name 'moxui-backup-*.tar.gz' -type f -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2-)
  if [[ -z "$BACKUP_FILE" ]]; then
    echo "[ERROR] No backup files found in ${BACKUP_DIR}" >&2
    exit 1
  fi
fi

echo "[INFO] Backup file: ${BACKUP_FILE}"
echo "       Size: $(du -h "$BACKUP_FILE" | cut -f1)"

# ── Build remote path ────────────────────────────────────────────────────
REMOTE_KEY="${PREFIX}$(basename "$BACKUP_FILE")"

# ── Upload ───────────────────────────────────────────────────────────────
echo "[INFO] Uploading to s3://${BUCKET}/${REMOTE_KEY}"

if [[ "$DRY_RUN" == "true" ]]; then
  echo "[DRY-RUN] Would upload: ${BACKUP_FILE} → s3://${BUCKET}/${REMOTE_KEY}"
  echo "[DRY-RUN] Skipping upload."
else
  if [[ "$USE_AWS" == "true" ]]; then
    # Build AWS CLI args
    AWS_ARGS=(--region "$REGION")
    if [[ -n "$ENDPOINT" ]]; then
      AWS_ARGS+=(--endpoint-url "$ENDPOINT")
    fi
    aws s3 cp "$BACKUP_FILE" "s3://${BUCKET}/${REMOTE_KEY}" "${AWS_ARGS[@]}"
  elif [[ "$USE_MC" == "true" ]]; then
    if [[ -n "$ENDPOINT" ]]; then
      # mc requires an alias; use the endpoint directly
      mc cp "$BACKUP_FILE" "${ENDPOINT}/${BUCKET}/${REMOTE_KEY}"
    else
      echo "[ERROR] MinIO client requires --endpoint for S3-compatible storage" >&2
      exit 1
    fi
  fi
  echo "[OK]   Upload completed"
fi

# ── Cleanup old remote backups ───────────────────────────────────────────
if [[ "$DRY_RUN" != "true" && "$CLEANUP_REMOTE_DAYS" -gt 0 ]]; then
  echo "[INFO] Cleaning up remote backups older than ${CLEANUP_REMOTE_DAYS} days..."
  CUTOFF=$(date -u -d "${CLEANUP_REMOTE_DAYS} days ago" '+%Y%m%dT%H%M%SZ' 2>/dev/null || date -u -v-${CLEANUP_REMOTE_DAYS}d '+%Y%m%dT%H%M%SZ')

  if [[ "$USE_AWS" == "true" ]]; then
    AWS_ARGS=(--region "$REGION")
    if [[ -n "$ENDPOINT" ]]; then
      AWS_ARGS+=(--endpoint-url "$ENDPOINT")
    fi

    aws s3 ls "s3://${BUCKET}/${PREFIX}" "${AWS_ARGS[@]}" 2>/dev/null \
      | while read -r line; do
          key=$(echo "$line" | awk '{print $4}')
          date_str=$(echo "$key" | grep -oP '\d{8}T\d{6}Z' || true)
          if [[ -n "$date_str" && "$date_str" < "$CUTOFF" ]]; then
            echo "[INFO] Removing remote: ${key}"
            aws s3 rm "s3://${BUCKET}/${key}" "${AWS_ARGS[@]}"
          fi
        done
  fi
fi

# ── Local rotation ───────────────────────────────────────────────────────
if [[ "$KEEP" -gt 0 ]]; then
  find "$BACKUP_DIR" -maxdepth 1 -name 'moxui-backup-*.tar.gz' -type f \
    | sort \
    | head -n -"${KEEP}" \
    | while read -r old; do
        echo "[INFO] Removing old local backup: $(basename "$old")"
        rm -f "$old"
      done
fi

echo "[INFO] S3 backup sync completed successfully."
