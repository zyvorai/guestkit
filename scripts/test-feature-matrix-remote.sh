#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# ─────────────────────────────────────────────────────────────────────────────
# test-feature-matrix-remote.sh — run scripts/test-feature-matrix.sh on a remote
# Linux host that has the system deps the matrix needs (libhivex, libsystemd,
# qemu, rust+clippy). The local macOS box can't build several features, so this
# is the canonical way to re-verify "does every feature still build" when a
# feature breaks.
#
# Usage:
#   scripts/test-feature-matrix-remote.sh <host> <user> [--setup] [--key KEYFILE]
#
#   --setup       apt/dnf-install the matrix deps on the remote first
#                 (libhivex-dev, libhivex-bin, libsystemd-dev, qemu, maturin)
#   --key FILE    ssh -i FILE
#
# Live-test rows run only when their provider/creds are reachable; the following
# env vars are forwarded to the remote matrix if set locally:
#   OLLAMA_HOST  OLLAMA_MODEL  GUESTKIT_AI_PROVIDER
#   OPENAI_API_KEY  ANTHROPIC_API_KEY  XAI_API_KEY
#   GK_TEST_DISK  GK_TEST_S3_URI  GK_TEST_AZURE_URI  GK_TEST_GCS_URI
#
# Exit code mirrors the remote matrix (non-zero if any compile row failed).
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

usage() { echo "usage: $0 <host> <user> [--setup] [--key KEYFILE]" >&2; exit 1; }
[ $# -ge 2 ] || usage
HOST="$1"; REMOTE_USER="$2"; shift 2

SETUP=false
SSH_OPTS=(-o BatchMode=yes -o ConnectTimeout=15 -o StrictHostKeyChecking=accept-new)
while [ $# -gt 0 ]; do
    case "$1" in
        --setup) SETUP=true ;;
        --key)   shift; SSH_OPTS+=(-i "${1:?--key needs a file}") ;;
        *) echo "unknown arg: $1" >&2; usage ;;
    esac
    shift
done

TARGET="${REMOTE_USER}@${HOST}"
REMOTE_DIR="/home/${REMOTE_USER}/.deployments/guestkit"
ssh_run() { ssh "${SSH_OPTS[@]}" "$TARGET" "$@"; }

echo "▶ syncing working tree → ${TARGET}:${REMOTE_DIR}"
rsync -az --delete \
    -e "ssh ${SSH_OPTS[*]}" \
    --exclude 'target/' --exclude '.git/' --exclude '*.qcow2' \
    --exclude '*.img' --exclude 'test-vms/' \
    "${REPO_DIR}/" "${TARGET}:${REMOTE_DIR}/"

if $SETUP; then
    echo "▶ installing feature-matrix deps on remote"
    ssh_run 'bash -s' <<'SETUP_REMOTE'
set -e
SUDO=""; [ "$(id -u)" -ne 0 ] && SUDO=sudo
if command -v apt-get >/dev/null 2>&1; then
    $SUDO apt-get update -qq
    $SUDO apt-get install -y -qq libhivex-dev libhivex-bin libsystemd-dev \
        qemu-utils pkg-config python3-pip pipx || true
elif command -v dnf >/dev/null 2>&1; then
    $SUDO dnf install -y hivex-devel hivex systemd-devel qemu-img pkgconf \
        python3-pip pipx || true
fi
# maturin — PEP668-safe install for the python-bindings row
if ! command -v maturin >/dev/null 2>&1 && [ ! -x "$HOME/.local/bin/maturin" ]; then
    pipx install maturin >/dev/null 2>&1 \
        || pip3 install --user --break-system-packages maturin >/dev/null 2>&1 || true
fi
echo "  hivexget=$(command -v hivexget || echo MISSING)"
echo "  maturin=$(command -v maturin || echo "$HOME/.local/bin/maturin")"
SETUP_REMOTE
fi

# Forward any live-test env that is set locally.
FWD=""
for v in OLLAMA_HOST OLLAMA_MODEL GUESTKIT_AI_PROVIDER \
         OPENAI_API_KEY ANTHROPIC_API_KEY XAI_API_KEY \
         GK_TEST_DISK GK_TEST_S3_URI GK_TEST_AZURE_URI GK_TEST_GCS_URI; do
    val="$(printenv "$v" || true)"
    [ -n "$val" ] && FWD+="$v=$(printf %q "$val") "
done

echo "▶ running feature matrix on remote  ${FWD:+(live env: ${FWD})}"
ssh_run "bash -lc 'cd ${REMOTE_DIR} \
    && source \$HOME/.cargo/env 2>/dev/null || true; \
    export PATH=\$HOME/.local/bin:\$PATH; \
    ${FWD} bash scripts/test-feature-matrix.sh'"
