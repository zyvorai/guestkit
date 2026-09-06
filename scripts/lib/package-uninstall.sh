#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=/dev/null
[[ -f "${ROOT}/.package-lib/package-ui.sh" ]] && source "${ROOT}/.package-lib/package-ui.sh"
# shellcheck source=/dev/null
source "${ROOT}/.package-lib/package-uninstall-lib.sh"

PRODUCT="GuestKit"
BINARIES=(guestkit guestctl)
LOCAL_CONFIGS=(guestkit.env)
SYSTEM_PATHS=("${HOME}/.cache/guestkit")

package_uninstall_main "${PRODUCT}" "${ROOT}" "$@"
