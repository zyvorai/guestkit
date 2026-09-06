#!/bin/bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Run Arch Linux realistic disk image tests
#
# This script tests guestkit with production-quality Arch Linux images
# that include BTRFS subvolumes, systemd-boot, and complete metadata.

set -e

echo "==================================================================="
echo "  Arch Linux Realistic Disk Image Tests"
echo "==================================================================="
echo ""

# Check for required tools
echo "Checking for required tools..."
for tool in qemu-nbd parted sgdisk mkfs.btrfs; do
    if ! command -v $tool &> /dev/null; then
        echo "⚠  Warning: $tool not found (tests may fail)"
    else
        echo "  ✓ $tool found"
    fi
done
echo ""

# Check disk space
echo "Checking available disk space..."
AVAILABLE=$(df /tmp | tail -1 | awk '{print $4}')
NEEDED=1500000  # 1.5GB in KB

if [ $AVAILABLE -lt $NEEDED ]; then
    echo "⚠  Warning: Low disk space in /tmp"
    echo "   Available: ${AVAILABLE}KB"
    echo "   Recommended: ${NEEDED}KB"
    echo ""
fi

# Clean up any existing test images
echo "Cleaning up old test images..."
rm -f /tmp/arch-test.img
for i in {0..15}; do
    sudo qemu-nbd --disconnect /dev/nbd$i 2>/dev/null || true
done
echo "  ✓ Cleanup complete"
echo ""

# Run the tests
echo "==================================================================="
echo "  Running Tests"
echo "==================================================================="
echo ""

# Run with verbose output
export RUST_BACKTRACE=1

# Test 1: Arch Linux with BTRFS
echo "Test 1: Arch Linux (Rolling Release) - BTRFS with Subvolumes"
echo "-------------------------------------------------------------------"
if cargo test --test arch_realistic test_arch_realistic -- --nocapture --test-threads=1; then
    echo "  ✓ PASSED"
else
    echo "  ✗ FAILED"
    exit 1
fi
echo ""

# Cleanup between tests
for i in {0..15}; do
    sudo qemu-nbd --disconnect /dev/nbd$i 2>/dev/null || true
done
rm -f /tmp/arch-test.img
sleep 1

# Test 2: OS Inspection
echo "Test 2: Arch Linux OS Inspection"
echo "-------------------------------------------------------------------"
if cargo test --test arch_realistic test_arch_inspection -- --nocapture --test-threads=1; then
    echo "  ✓ PASSED"
else
    echo "  ✗ FAILED"
    exit 1
fi
echo ""

# Cleanup between tests
for i in {0..15}; do
    sudo qemu-nbd --disconnect /dev/nbd$i 2>/dev/null || true
done
rm -f /tmp/arch-test.img
sleep 1

# Test 3: BTRFS Subvolume Validation
echo "Test 3: BTRFS Subvolume Validation"
echo "-------------------------------------------------------------------"
if cargo test --test arch_realistic test_arch_btrfs_subvolumes -- --nocapture --test-threads=1; then
    echo "  ✓ PASSED"
else
    echo "  ✗ FAILED"
    exit 1
fi
echo ""

# Final cleanup
echo "==================================================================="
echo "  Cleanup"
echo "==================================================================="
for i in {0..15}; do
    sudo qemu-nbd --disconnect /dev/nbd$i 2>/dev/null || true
done
rm -f /tmp/arch-test.img
echo "  ✓ Test artifacts removed"
echo ""

echo "==================================================================="
echo "  ✓ ALL ARCH LINUX TESTS PASSED!"
echo "==================================================================="
echo ""
echo "Summary:"
echo "  • Arch Linux (Rolling Release) - BTRFS filesystem"
echo "  • OS Inspection validation"
echo "  • BTRFS subvolume validation"
echo ""
echo "Features Tested:"
echo "  • GPT partitioning with EFI System Partition"
echo "  • BTRFS filesystem with subvolumes (@, @home, @var, @snapshots)"
echo "  • systemd-boot bootloader"
echo "  • Systemd units (sshd, NetworkManager, journald)"
echo "  • pacman package manager configuration"
echo "  • Arch Linux metadata files"
echo "  • Phase 3 APIs (stat, lstat, rm, rm_rf)"
echo "  • BTRFS APIs (btrfs_subvolume_create, btrfs_subvolume_list)"
echo "  • OS inspection APIs"
echo ""
echo "All Arch Linux tests completed successfully!"
