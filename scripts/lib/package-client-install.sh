#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail
# shellcheck source=/dev/null
[[ -f "$(dirname "$0")/.package-lib/package-ui.sh" ]] && source "$(dirname "$0")/.package-lib/package-ui.sh"

pkg_banner "GuestKit host dependencies" "qemu-img · nbd · libhivex (pure Rust — not legacy appliance tooling)"
SUDO=""
[[ "$(id -u)" -ne 0 ]] && command -v sudo &>/dev/null && SUDO=sudo
pkg_info "Installing qemu-img, nbd, lvm2, parted, libhivex…"
# Delegate to existing logic inline (same as before).
# libhivex0 provides libhivex.so.0 — the glibc build links it for offline
# Windows registry writes (guestkit plan apply RegistryEdit).
if command -v apt-get &>/dev/null; then
  $SUDO apt-get update -qq
  $SUDO apt-get install -y -qq qemu-utils nbd-client lvm2 parted libhivex0 2>&1 | tail -5 || true
elif command -v dnf &>/dev/null; then
  $SUDO dnf install -y qemu-img nbd lvm2 parted e2fsprogs 2>&1 | tail -5 || true
  # Runtime hivex library — package name varies across Fedora/EL releases.
  $SUDO dnf install -y hivex-libs 2>&1 | tail -3 \
    || $SUDO dnf install -y hivex 2>&1 | tail -3 || true
fi
$SUDO modprobe nbd max_part=16 2>/dev/null || true
pkg_ok "Host packages configured"
pkg_summary "Dependencies"
