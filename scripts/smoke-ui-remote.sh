#!/usr/bin/env bash
# Copyright 2026 Zyvor
# SPDX-License-Identifier: Apache-2.0
# ============================================================================
# smoke-ui-remote.sh — Verify a running GuestKit UI instance (HTTPS)
# ============================================================================
# Usage:
#   GUESTKIT_UI_URL=https://212.8.248.187:27173 ./scripts/smoke-ui-remote.sh
#   ./scripts/smoke-ui-remote.sh --port 27173
#   ./scripts/smoke-ui-remote.sh   # uses HOST:PORT from .deploy-ui-last
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PORT_FROM_CLI=""
while [ $# -gt 0 ]; do
  case "$1" in
    --port) [ $# -ge 2 ] || { echo "--port requires a value" >&2; exit 2; }; PORT_FROM_CLI="$2"; shift 2 ;;
    --port=*) PORT_FROM_CLI="${1#*=}"; shift ;;
    --help|-h)
      sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
done

BASE="${GUESTKIT_UI_URL:-}"
HOST_FROM_LAST=""
PORT_FROM_LAST=""
if [ -f "$ROOT/.deploy-ui-last" ]; then
  # shellcheck disable=SC1091
  source "$ROOT/.deploy-ui-last"
  HOST_FROM_LAST="${HOST:-}"
  PORT_FROM_LAST="${PORT:-}"
fi

if [ -z "$BASE" ]; then
  PORT_RESOLVED="${PORT_FROM_CLI:-${GUESTKIT_UI_PORT:-$PORT_FROM_LAST}}"
  HOST_RESOLVED="${GUESTKIT_UI_HOST:-$HOST_FROM_LAST}"
  if [ -n "$HOST_RESOLVED" ] && [ -n "$PORT_RESOLVED" ]; then
    BASE="https://${HOST_RESOLVED}:${PORT_RESOLVED}"
  fi
fi
[ -n "$BASE" ] || {
  echo "Set GUESTKIT_UI_URL=https://host:port, or --port / GUESTKIT_UI_PORT with host from .deploy-ui-last" >&2
  exit 2
}
BASE="${BASE%/}"
# Lab certs are self-signed
CURL=(curl -ksS)
TMP="${TMPDIR:-/tmp}"

pass() { printf '  ✅ %s\n' "$*"; }
fail() { printf '  ❌ %s\n' "$*" >&2; exit 1; }

echo "GuestKit UI smoke → ${BASE}"

code="$("${CURL[@]}" -o "${TMP}/gk-ui.html" -w '%{http_code}' "${BASE}/")"
[ "$code" = "200" ] || fail "index HTTP ${code}"
grep -qi 'GuestKit\|zyvor' "${TMP}/gk-ui.html" || fail "index body unexpected"
grep -q 'zyvor-ux.css\|zyvor-ux.js' "${TMP}/gk-ui.html" || fail "Zyvor GA UX assets not linked"
grep -qi 'Built by Zyvor' "${TMP}/gk-ui.html" || fail "footer missing Built by Zyvor"
grep -qi 'HyperSDK' "${TMP}/gk-ui.html" && fail "HyperSDK still present in UI" || true
pass "index + Zyvor GA UX"

code="$("${CURL[@]}" -o "${TMP}/gk-login.html" -w '%{http_code}' "${BASE}/login.html")"
[ "$code" = "200" ] || fail "login HTTP ${code}"
grep -q 'zyvor-ux.css\|zyvor-ux.js' "${TMP}/gk-login.html" || fail "login missing Zyvor GA UX"
grep -qi 'HyperSDK' "${TMP}/gk-login.html" && fail "HyperSDK still present on login" || true
pass "login"

code="$("${CURL[@]}" -o /dev/null -w '%{http_code}' "${BASE}/zyvor-ux.css")"
[ "$code" = "200" ] || fail "zyvor-ux.css HTTP ${code}"
pass "zyvor-ux.css"

code="$("${CURL[@]}" -o /dev/null -w '%{http_code}' "${BASE}/zyvor-ux.js")"
[ "$code" = "200" ] || fail "zyvor-ux.js HTTP ${code}"
pass "zyvor-ux.js"

echo "  ✨ smoke OK"
