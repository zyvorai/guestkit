#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# From your laptop: sync test script, optionally rebuild one product, run remote E2E.
# Same script in every Zyvor product repo — paths resolve via sibling checkout under tt/.
#
# Usage:
#   ./scripts/test-user-e2e-remote.sh HOST USER
#   ./scripts/test-user-e2e-remote.sh HOST USER --quick          # skip rebuild
#   ./scripts/test-user-e2e-remote.sh HOST USER --product machina # rebuild one tarball
#
# Examples:
#   ./scripts/test-user-e2e-remote.sh 212.8.252.194 sus
#   ZYVOR_E2E_SKIP=VMRogue,v9s ./scripts/test-user-e2e-remote.sh 212.8.252.194 sus --quick
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
TT="$(cd "${ROOT}/.." && pwd)"
VMROGUE="${TT}/VMRogue"

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'
  exit 0
fi

HOST="${1:?usage: $0 HOST USER [--quick] [--product NAME]}"
USER="${2:?usage: $0 HOST USER [--quick] [--product NAME]}"
shift 2 || true

QUICK=0
PRODUCT=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --quick) QUICK=1 ;;
    --product) PRODUCT="${2:?}"; shift ;;
    -h|--help)
      sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) echo "Unknown arg: $1" >&2; exit 1 ;;
  esac
  shift
done

REMOTE="${USER}@${HOST}"
SSH_OPTS=(-o ConnectTimeout=15 -o BatchMode=yes -o StrictHostKeyChecking=accept-new)
TEST_SCRIPT="${SCRIPT_DIR}/test-packages-remote-only.sh"
[[ -f "${TEST_SCRIPT}" ]] || TEST_SCRIPT="${VMROGUE}/scripts/test-packages-remote-only.sh"

echo "==> Remote user E2E → ${REMOTE}"
echo ""

scp "${SSH_OPTS[@]}" "${TEST_SCRIPT}" "${REMOTE}:~/test-packages-remote-only.sh"

package_script_for() {
  local key="${1,,}"
  case "${key}" in
    vmrogue) echo "${VMROGUE}/scripts/package-binary-remote.sh" ;;
    machina) echo "${TT}/machina/scripts/package-binary-remote.sh" ;;
    v9s) echo "${TT}/v9s/scripts/package-binary-remote.sh" ;;
    guestkit) echo "${TT}/guestkit/scripts/package-binary-remote.sh" ;;
    hypersdk|hypersdk-) echo "${TT}/hypersdk-/scripts/package-binary-remote.sh" ;;
    hyper2kvm|hyper2kvm-) echo "${TT}/hyper2kvm-/scripts/package-binary-remote.sh" ;;
    packetwolf) echo "${TT}/packetwolf/scripts/package-binary-remote.sh" ;;
    ragnarok) echo "${TT}/ragnarok/scripts/package-binary-remote.sh" ;;
    aether) echo "${TT}/Aether/scripts/package-binary-remote.sh" ;;
    ironwolf) echo "${TT}/IronWolf/scripts/package-binary-remote.sh" ;;
    forge) echo "${TT}/forge/scripts/package-binary-remote.sh" ;;
    *) return 1 ;;
  esac
}

if [[ "${QUICK}" -eq 0 ]]; then
  if [[ -n "${PRODUCT}" ]]; then
    pkg=$(package_script_for "${PRODUCT}") || {
      echo "Unknown --product ${PRODUCT}" >&2
      exit 1
    }
    [[ -x "${pkg}" ]] || { echo "Missing ${pkg}" >&2; exit 1; }
    "${pkg}" "${HOST}" "${USER}" --fetch
  else
    echo "==> Using existing tarballs on remote (pass --product NAME to rebuild one first)"
  fi
fi

echo ""
echo "==> Running remote install/uninstall tests (install-everything when bundled)..."
ssh "${SSH_OPTS[@]}" "${REMOTE}" 'chmod +x ~/test-packages-remote-only.sh && bash ~/test-packages-remote-only.sh'
echo ""
echo "Done. Fetch log: scp ${REMOTE}:~/package-tests/results-*.log ."
