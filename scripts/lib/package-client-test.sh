#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
set -uo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"
export PKG_INSTALL_ROOT="${ROOT}"
# shellcheck source=/dev/null
[[ -f "${ROOT}/.package-lib/package-ui.sh" ]] && source "${ROOT}/.package-lib/package-ui.sh"

[[ "${1:-}" == "-h" || "${1:-}" == "--help" ]] && {
  pkg_script_help "test-package.sh"
  exit 0
}

_PKG_SESSION_START=${SECONDS}
pkg_counters_reset
pkg_banner "GuestKit package test" "CLI · qemu/nbd · optional selftest"

[[ -x ./guestkit ]] && { ./guestkit --version; pkg_ok "guestkit --version"; } || pkg_fail "guestkit"
[[ -x ./test-host.sh ]] && { ./test-host.sh || pkg_warn "test-host.sh"; }
[[ -x ./test-selftest.sh ]] && { ./test-selftest.sh --quick && pkg_ok "test-selftest.sh --quick" || pkg_warn "test-selftest.sh"; }

pkg_summary "Package test"
[[ "${_PKG_COUNTERS_FAIL}" -eq 0 ]]
