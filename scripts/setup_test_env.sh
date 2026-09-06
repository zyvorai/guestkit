#!/bin/bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Setup test environment for Phase 3 comprehensive tests
#
# This script configures the system to allow non-root users to run
# guestkit tests that require NBD device access.

set -e

echo "==================================================================="
echo "  GuestKit Test Environment Setup"
echo "==================================================================="
echo ""

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "❌ ERROR: This script must be run as root (use sudo)"
    echo ""
    echo "Usage:"
    echo "  sudo ./scripts/setup_test_env.sh"
    exit 1
fi

# Load NBD kernel module
echo "[1/4] Loading NBD kernel module..."
if ! lsmod | grep -q '^nbd'; then
    modprobe nbd max_part=8
    echo "  ✓ NBD module loaded with max_part=8"
else
    echo "  ✓ NBD module already loaded"
fi

# Set NBD device permissions
echo ""
echo "[2/4] Setting NBD device permissions..."
chmod 666 /dev/nbd* 2>/dev/null || true
echo "  ✓ NBD devices readable/writable by all users"

# Create qemu-nbd lock directory with proper permissions
echo ""
echo "[3/4] Configuring qemu-nbd lock directory..."
if [ -d /run/lock ]; then
    # Make /run/lock writable by all (for qemu-nbd locks)
    chmod 1777 /run/lock
    echo "  ✓ /run/lock configured (sticky bit set)"
else
    echo "  ⚠  Warning: /run/lock not found"
fi

# Verify required tools
echo ""
echo "[4/4] Verifying required tools..."
MISSING_TOOLS=0

for tool in qemu-nbd parted sgdisk cpio; do
    if command -v $tool &> /dev/null; then
        echo "  ✓ $tool found"
    else
        echo "  ✗ $tool NOT FOUND"
        MISSING_TOOLS=1
    fi
done

echo ""
if [ $MISSING_TOOLS -eq 0 ]; then
    echo "==================================================================="
    echo "  ✅ Test environment setup complete!"
    echo "==================================================================="
    echo ""
    echo "You can now run tests as a regular user:"
    echo "  ./scripts/run_all_phase3_tests.sh"
    echo ""
    echo "Note: NBD device permissions reset on reboot. Re-run this script"
    echo "      after reboot to restore test environment."
    echo ""
else
    echo "==================================================================="
    echo "  ⚠️  Setup incomplete - missing tools"
    echo "==================================================================="
    echo ""
    echo "Install missing tools:"
    echo "  # Fedora/RHEL"
    echo "  sudo dnf install qemu-img parted gdisk cpio"
    echo ""
    echo "  # Ubuntu/Debian"
    echo "  sudo apt-get install qemu-utils parted gdisk cpio"
    echo ""
    exit 1
fi
