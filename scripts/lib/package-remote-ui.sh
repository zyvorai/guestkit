#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# shellcheck shell=bash
# UI helpers for package-binary-remote.sh (sourced, not executed).
_PKG_REMOTE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=package-ui.sh
source "${_PKG_REMOTE_DIR}/package-ui.sh"

PKG_REMOTE_START=${SECONDS}

pkg_remote_banner() {
    local product="$1" version="$2" host="$3" arch="${4:-linux-amd64}"
    pkg_banner "${product} — remote package build" "v${version} · ${arch} · ${host}"
    pkg_info "Build output will include install scripts, tests, and user docs"
}

pkg_remote_phase() {
    local phase="$1"
    echo ""
    printf "%s━━ %s ━━%s\n" "${PKG_C_BOLD}${PKG_C_BLUE}" "${phase}" "${PKG_C_RESET}"
}

# Prefer this over "Sync source" — user tarball has no source tree.
pkg_remote_sync_build_host() {
    pkg_remote_phase "Sync to build host"
}

pkg_remote_kv() {
    pkg_detail "$1: $2"
}

pkg_remote_done() {
    local product="$1" tarball="$2" checksum="${3:-}"
    local elapsed=$((SECONDS - PKG_REMOTE_START))
    echo ""
    pkg_divider "═"
    printf "%s%s  Package complete%s\n" "${PKG_C_BOLD}${PKG_C_GREEN}" "${product}" "${PKG_C_RESET}"
    pkg_divider "═"
    echo ""
    pkg_remote_kv "Archive" "${tarball}"
    [[ -n "${checksum}" ]] && pkg_remote_kv "Checksum" "${checksum}"
    pkg_remote_kv "Total time" "${elapsed}s"
    echo ""
    pkg_next_steps \
        "Hand off the .tar.gz + .sha256 to the user" \
        "User: tar xzf → cd → ./install.sh" \
        "Docs inside tarball: README.txt · QUICKSTART.txt"
    echo ""
}
