#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"
export PKG_INSTALL_ROOT="${ROOT}"
# shellcheck source=/dev/null
[[ -f "${ROOT}/.package-lib/package-ui.sh" ]] && source "${ROOT}/.package-lib/package-ui.sh"

pkg_parse_install_args "$@"

_PKG_SESSION_START=${SECONDS}
pkg_install_welcome "GuestKit"
pkg_banner "GuestKit" "Offline VM disk inspection · client bundle"
pkg_step_init 4

pkg_step "Host dependencies"
pkg_sudo ./install-client-deps.sh 2>/dev/null || ./install-client-deps.sh || pkg_warn "deps issues"
pkg_step_done

pkg_step "Configuration"
if [[ -f guestkit.env.example ]]; then
  pkg_env_bootstrap guestkit.env.example guestkit.env
  if declare -F pkg_env_bootstrap_auth_for_file >/dev/null 2>&1; then
    pkg_env_bootstrap_auth_for_file guestkit.env 2>/dev/null || true
  fi
fi
pkg_step_done

pkg_step "Verify binary"
[[ -x ./guestkit ]] && ./guestkit --version && pkg_ok "guestkit" || { pkg_fail "guestkit"; exit 1; }
[[ -x ./guestctl ]] && ./guestctl --version >/dev/null && pkg_ok "guestctl" || pkg_warn "guestctl symlink missing"
pkg_step_done

pkg_step "Smoke test"
[[ -x ./test-package.sh ]] && ./test-package.sh || pkg_warn "test-package.sh"
[[ -x ./test-host.sh ]] && ./test-host.sh || true
pkg_step_done

pkg_summary "GuestKit — ready"
pkg_next_steps \
  "zyvor.dev · HyperSDK · © 2026" \
  "Help: cat HELP.txt · ./install.sh --help" \
  "Try: ./guestkit inspect /path/to/disk.qcow2" \
  "Alias: ./guestctl (same CLI) · shorthand: ./guestctl disk.qcow2" \
  "./test-selftest.sh --quick (if bundled)" \
  "Docs: HOST_SETUP.txt · PREREQUISITES.txt" \
  "Remove: ./uninstall.sh --yes [--remove-dir]"
