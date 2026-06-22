#!/usr/bin/env bash
# ===========================================================================
# MoxUI — Binary Release Signing Script
#
# Signs a binary with minisign for release verification.
# Creates a .minisig file alongside the binary.
#
# Usage:
#   ./scripts/sign-release.sh path/to/binary
#
# Environment:
#   MOXUI_SIGN_KEY   Path to the minisign secret key
#                     (default: ~/.moxui/signing.key)
#
# Verification by users:
#   minisign -Vm <binary> -P "$(cat ~/.moxui/signing.pub)"
# ===========================================================================
set -euo pipefail

# ── Config ─────────────────────────────────────────────────────────────────
SIGN_KEY="${MOXUI_SIGN_KEY:-${HOME}/.moxui/signing.key}"
SIGN_PUB="${HOME}/.moxui/signing.pub"

# ── Validate arguments ────────────────────────────────────────────────────
if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <binary-path>" >&2
  exit 1
fi

BINARY="$1"

if [[ ! -f "$BINARY" ]]; then
  echo "[ERROR] Binary not found: ${BINARY}" >&2
  exit 1
fi

if [[ ! -f "$SIGN_KEY" ]]; then
  echo "[ERROR] Signing key not found: ${SIGN_KEY}" >&2
  echo "        Generate one with: minisign -G -p ${SIGN_PUB} -s ${SIGN_KEY}" >&2
  exit 1
fi

# ── Sign ───────────────────────────────────────────────────────────────────
echo "[INFO] Signing: ${BINARY}"
echo "[INFO] Using key: ${SIGN_KEY}"

minisign -Sm "$BINARY" -s "$SIGN_KEY" -W

SIG_FILE="${BINARY}.minisig"
if [[ -f "$SIG_FILE" ]]; then
  echo "[OK]   Signature: ${SIG_FILE}"
  echo "[OK]   Public key: ${SIGN_PUB}"
  echo ""
  echo "Verification command:"
  echo "  minisign -Vm \"${BINARY}\" -P \"$(cat "$SIGN_PUB")\""
else
  echo "[ERROR] Signature file was not created." >&2
  exit 1
fi
