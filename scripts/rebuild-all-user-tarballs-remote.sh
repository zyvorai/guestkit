#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Rebuild user tarballs on a remote Linux host (fresh HELP.txt, install-everything, UX).
# Same script in every Zyvor product repo — paths resolve via sibling checkout under tt/.
#
# Usage:
#   ./scripts/rebuild-all-user-tarballs-remote.sh HOST USER
#   ./scripts/rebuild-all-user-tarballs-remote.sh HOST USER --reuse-build
#
# Then verify:
#   ./scripts/test-user-e2e-remote.sh HOST USER --quick
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
TT="$(cd "${ROOT}/.." && pwd)"
VMROGUE="${TT}/VMRogue"

HOST="${1:?HOST}"
USER="${2:?USER}"
shift 2 || true

EXTRA=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --reuse-build) EXTRA+=(--reuse-build) ;;
    -h|--help)
      sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) echo "Unknown: $1" >&2; exit 1 ;;
  esac
  shift
done

declare -a PRODUCTS=(
  "VMRogue:${VMROGUE}/scripts/package-binary-remote.sh"
  "machina:${TT}/machina/scripts/package-binary-remote.sh"
  "v9s:${TT}/v9s/scripts/package-binary-remote.sh"
  "guestkit:${TT}/guestkit/scripts/package-binary-remote.sh"
  "hypersdk-:${TT}/hypersdk-/scripts/package-binary-remote.sh"
  "hyper2kvm-:${TT}/hyper2kvm-/scripts/package-binary-remote.sh"
  "packetwolf:${TT}/packetwolf/scripts/package-binary-remote.sh"
  "ragnarok:${TT}/ragnarok/scripts/package-binary-remote.sh"
  "Aether:${TT}/Aether/scripts/package-binary-remote.sh"
  "IronWolf:${TT}/IronWolf/scripts/package-binary-remote.sh"
  "forge:${TT}/forge/scripts/package-binary-remote.sh"
)

echo "==> Rebuilding user tarballs on ${USER}@${HOST}"
echo "    Workspace: ${TT}"
for entry in "${PRODUCTS[@]}"; do
  name="${entry%%:*}"
  script="${entry#*:}"
  if [[ ! -x "${script}" ]]; then
    echo "SKIP ${name} (no ${script})"
    continue
  fi
  echo ""
  echo "━━ ${name} ━━"
  "${script}" "${HOST}" "${USER}" "${EXTRA[@]+"${EXTRA[@]}"}" || {
    echo "WARN: ${name} build failed — continuing"
  }
done

echo ""
echo "==> Done. Run E2E: ${SCRIPT_DIR}/test-user-e2e-remote.sh ${HOST} ${USER} --quick"
