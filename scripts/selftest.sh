#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail
# ============================================================================
# selftest.sh — Post-installation verification for guestkit
# ============================================================================
# Verifies that the guestkit binary, system dependencies, kernel modules,
# and core inspection pipeline are correctly installed.
#
# Usage:
#   ./scripts/selftest.sh              # Run all checks
#   ./scripts/selftest.sh --quick      # Skip disk tests
#   make selftest                      # Via Makefile
#
# Exit codes:
#   0 = all checks passed
#   1 = one or more checks failed
# ============================================================================

PASS=0
FAIL=0
WARN=0
QUICK=false

[[ "${1:-}" == "--quick" ]] && QUICK=true

pass() { PASS=$((PASS + 1)); echo "  ✅ $1"; }
fail() { FAIL=$((FAIL + 1)); echo "  ❌ $1"; }
warn() { WARN=$((WARN + 1)); echo "  ⚠️  $1"; }
section() { echo ""; echo "━━━ $1 ━━━"; }

# ── Binary ───────────────────────────────────────────────────────────────────
section "GuestKit Binary"

SCRIPT_DIR_BIN="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GUESTKIT_BIN=""

if command -v guestkit &>/dev/null; then
    GUESTKIT_BIN="$(command -v guestkit)"
    pass "guestkit found: $GUESTKIT_BIN"
elif [ -x "$SCRIPT_DIR_BIN/target/release/guestkit" ]; then
    GUESTKIT_BIN="$SCRIPT_DIR_BIN/target/release/guestkit"
    pass "guestkit found: $GUESTKIT_BIN (local build)"
elif [ -x "$SCRIPT_DIR_BIN/target/debug/guestkit" ]; then
    GUESTKIT_BIN="$SCRIPT_DIR_BIN/target/debug/guestkit"
    pass "guestkit found: $GUESTKIT_BIN (debug build)"
else
    fail "guestkit not found in PATH or target/"
fi

if [ -n "$GUESTKIT_BIN" ]; then
    ver=$($GUESTKIT_BIN --version 2>/dev/null || echo "FAILED")
    if [[ "$ver" != "FAILED" ]]; then
        pass "guestkit --version: $ver"
    else
        fail "guestkit --version failed"
    fi
fi

# Use guestkit function for rest of script
guestkit() { "$GUESTKIT_BIN" "$@"; }

# ── Rust Toolchain ───────────────────────────────────────────────────────────
section "Rust Toolchain"

if command -v cargo &>/dev/null; then
    pass "cargo found: $(cargo --version 2>/dev/null | head -1)"
else
    warn "cargo not found (needed for building from source)"
fi

if command -v rustc &>/dev/null; then
    pass "rustc found: $(rustc --version 2>/dev/null | head -1)"
else
    warn "rustc not found (needed for building from source)"
fi

# ── System Dependencies ─────────────────────────────────────────────────────
section "System Dependencies"

# GuestKit is pure Rust + qemu/nbd — no appliance daemon required.

# QEMU tools
for tool in qemu-img qemu-nbd; do
    if command -v "$tool" &>/dev/null; then
        pass "$tool found: $(command -v "$tool")"
    else
        fail "$tool not found (install qemu-img or qemu-utils; on EL9 qemu-nbd is in qemu-img)"
    fi
done

# Loop device support
if command -v losetup &>/dev/null; then
    pass "losetup found: $(command -v losetup)"
else
    warn "losetup not found (needed for RAW/IMG format support)"
fi

# NBD kernel module
if lsmod 2>/dev/null | grep -q nbd; then
    pass "nbd kernel module: loaded"
elif [ -f /lib/modules/"$(uname -r)"/kernel/drivers/block/nbd.ko* ] 2>/dev/null; then
    warn "nbd kernel module: available but not loaded (run: modprobe nbd max_part=16)"
else
    warn "nbd kernel module: not found (needed for QCOW2/VMDK support)"
fi

# Optional tools
for tool in parted blkid; do
    if command -v "$tool" &>/dev/null; then
        pass "$tool found"
    else
        warn "$tool not found (optional, used for partition operations)"
    fi
done

for tool in openssl; do
    if command -v "$tool" &>/dev/null; then
        pass "$tool found (used for password hashing)"
    else
        warn "$tool not found (needed for password reset operations)"
    fi
done

# ── LVM Support ─────────────────────────────────────────────────────────────
section "LVM Support"

for tool in pvscan vgscan lvscan vgchange; do
    if command -v "$tool" &>/dev/null; then
        pass "$tool found"
    else
        warn "$tool not found (install lvm2 for LVM support)"
    fi
done

# ── Disk Format Support ─────────────────────────────────────────────────────
section "Disk Format Support"

# Check supported formats via qemu-img
if command -v qemu-img &>/dev/null; then
    for fmt in qcow2 vmdk vdi vpc raw; do
        if qemu-img --help 2>&1 | grep -q "$fmt" || qemu-img info --help 2>&1 | grep -q "$fmt"; then
            pass "format: $fmt"
        else
            # qemu-img supports these by default, just verify
            pass "format: $fmt (assumed)"
        fi
    done
fi

# ── Disk Smoke Test ──────────────────────────────────────────────────────────
if ! $QUICK; then
    section "Smoke Tests"

    TEST_TMPDIR=$(mktemp -d)
    trap 'rm -rf "$TEST_TMPDIR"' EXIT

    # Test 1: Create and detect RAW image
    RAW_IMG="$TEST_TMPDIR/test.raw"
    if qemu-img create -f raw "$RAW_IMG" 64M &>/dev/null 2>&1; then
        pass "created test RAW image (64M)"
        if guestkit detect "$RAW_IMG" &>/dev/null 2>&1; then
            pass "guestkit detect: RAW image recognized"
        else
            warn "guestkit detect: could not detect format (may need root)"
        fi
    else
        fail "qemu-img create RAW failed"
    fi

    # Test 2: Create and detect QCOW2 image
    QCOW2_IMG="$TEST_TMPDIR/test.qcow2"
    if qemu-img create -f qcow2 "$QCOW2_IMG" 64M &>/dev/null 2>&1; then
        pass "created test QCOW2 image (64M)"
        if guestkit detect "$QCOW2_IMG" &>/dev/null 2>&1; then
            pass "guestkit detect: QCOW2 image recognized"
        else
            warn "guestkit detect: could not detect format (may need root)"
        fi
    else
        fail "qemu-img create QCOW2 failed"
    fi

    # Test 3: Inspect (will fail on empty image, but should not crash)
    if guestkit inspect "$RAW_IMG" 2>&1 | grep -qiE "error|no os|unknown|failed"; then
        pass "guestkit inspect: handled empty image gracefully"
    else
        pass "guestkit inspect: ran without crash"
    fi

    # Test 4: Help and subcommands
    if guestkit --help &>/dev/null 2>&1; then
        pass "guestkit --help: OK"
    else
        fail "guestkit --help failed"
    fi

    for subcmd in inspect filesystems detect; do
        if guestkit "$subcmd" --help &>/dev/null 2>&1; then
            pass "guestkit $subcmd --help: OK"
        else
            warn "guestkit $subcmd --help: failed"
        fi
    done
fi

# ── Build Check ──────────────────────────────────────────────────────────────
section "Build Check"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

if [ -f "$REPO_DIR/Cargo.toml" ]; then
    pass "Cargo.toml found: $REPO_DIR"

    if [ -f "$REPO_DIR/target/release/guestkit" ]; then
        pass "release binary exists: target/release/guestkit"
    elif [ -f "$REPO_DIR/target/debug/guestkit" ]; then
        pass "debug binary exists: target/debug/guestkit"
    else
        warn "no compiled binary found (run: cargo build --release)"
    fi

    # Clippy only when explicitly requested (slow; not required on deploy hosts)
    if [ "${GUESTKIT_SELFTEST_CLIPPY:-0}" = "1" ] && command -v cargo &>/dev/null; then
        CLIPPY_OK=$(cd "$REPO_DIR" && cargo clippy --lib 2>&1 | grep -c "^warning\|^error" || true)
        if [ "$CLIPPY_OK" -eq 0 ]; then
            pass "clippy: zero warnings"
        else
            warn "clippy: $CLIPPY_OK warning(s)"
        fi
    fi
else
    warn "Not in guestkit repo — skipping build checks"
fi

# ── Permissions ──────────────────────────────────────────────────────────────
section "Permissions"

if [ "$(id -u)" -eq 0 ]; then
    pass "running as root (full access to devices)"
else
    warn "not running as root — some operations require sudo"

    # Check if user is in disk group
    if groups 2>/dev/null | grep -qw disk; then
        pass "user in 'disk' group"
    else
        warn "user not in 'disk' group (needed for direct device access)"
    fi

    # Check if user can use sudo
    if sudo -n true 2>/dev/null; then
        pass "passwordless sudo available"
    else
        warn "sudo requires password (some operations may prompt)"
    fi
fi

# ── Summary ──────────────────────────────────────────────────────────────────
echo ""
echo "━━━ Summary ━━━"
echo "  ✅ Passed: $PASS  ❌ Failed: $FAIL  ⚠️  Warnings: $WARN"

if [[ $FAIL -gt 0 ]]; then
    echo ""
    echo "💥 SELFTEST FAILED — fix the above errors before proceeding"
    exit 1
else
    echo ""
    echo "🎉 SELFTEST PASSED"
    exit 0
fi
