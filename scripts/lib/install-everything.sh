#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Zyvor — one-shot customer install (run inside extracted tarball directory).
# Does everything ./install.sh does, plus host tests and production setup when bundled.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "${ROOT}"

export PKG_INSTALL_ROOT="${ROOT}"
# shellcheck source=/dev/null
[[ -f "${ROOT}/.package-lib/package-ui.sh" ]] && source "${ROOT}/.package-lib/package-ui.sh"

pkg_parse_install_args "$@"

if [[ -f "${ROOT}/.package-lib/product.meta" ]]; then
  # shellcheck source=/dev/null
  source "${ROOT}/.package-lib/product.meta"
fi

PRODUCT_NAME="${PRODUCT_NAME:-$(basename "${ROOT}" | sed 's/-linux-amd64$//')}"
_PKG_SESSION_START=${SECONDS}

pkg_counters_reset
pkg_customer_hero "${PRODUCT_NAME}"
pkg_detail "Full automatic install — dependencies, config, tests, production setup when bundled"
pkg_detail "zyvor.dev · HyperSDK · © 2026"
echo ""

if [[ ! -x ./install.sh ]]; then
  pkg_fail "install.sh missing — extract the full tarball first"
  pkg_install_failed_hint
  exit 1
fi

pkg_bundle_sanity_check || pkg_warn "Some bundle files missing — install may still work"

pkg_phase "Core install (dependencies, config, binaries)"
./install.sh "$@"

if [[ -x ./test-host.sh ]]; then
  pkg_phase "Host preflight (libvirt / KVM / tools)"
  ./test-host.sh || pkg_warn "test-host.sh reported issues — see HOST_SETUP.txt"
fi

# install-full is also run from install.sh when AUTO_FULL_INSTALL=1; safe to skip if already done
if [[ -x ./install-full.sh ]] && [[ "${AUTO_FULL_INSTALL:-0}" != "1" ]]; then
  pkg_phase "Production host setup"
  if [[ "${ZYVOR_NONINTERACTIVE:-0}" == "1" ]] && ! sudo -n true 2>/dev/null; then
    pkg_skip "install-full.sh (non-interactive; no passwordless sudo)"
    pkg_detail "After install: sudo ./install-full.sh --open-firewall"
  elif pkg_sudo true 2>/dev/null; then
    pkg_sudo ./install-full.sh --open-firewall || pkg_warn "install-full.sh had issues"
  else
    pkg_warn "Skipped install-full.sh (need sudo). Run: sudo ./install-full.sh --open-firewall"
  fi
fi

if [[ -x ./test-package.sh ]]; then
  pkg_phase "Package smoke test"
  ./test-package.sh || pkg_warn "test-package.sh reported issues"
fi

if [[ -n "${ACCESS_SCHEME:-}" && -n "${ACCESS_PORT:-}" ]]; then
  _finish_extras=()
  [[ -n "${FINISH_EXTRA_1:-}" ]] && _finish_extras+=("${FINISH_EXTRA_1}")
  [[ -n "${FINISH_EXTRA_2:-}" ]] && _finish_extras+=("${FINISH_EXTRA_2}")
  [[ -n "${FINISH_EXTRA_3:-}" ]] && _finish_extras+=("${FINISH_EXTRA_3}")
  # shellcheck disable=SC2086
  pkg_install_finish "${PRODUCT_NAME}" "${ACCESS_SCHEME}" "${ACCESS_PORT}" "${ACCESS_PATH:-}" ${_finish_extras[@]+"${_finish_extras[@]}"}
else
  pkg_summary "${PRODUCT_NAME} — install finished"
  pkg_next_steps \
    "zyvor.dev · HyperSDK · © 2026" \
    "Help: cat HELP.txt · START_HERE.txt" \
    "See README.txt and QUICKSTART.txt in this folder"
fi
