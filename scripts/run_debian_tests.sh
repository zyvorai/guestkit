#!/bin/bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Run Debian realistic disk image tests
#
# This script tests guestkit with production-quality Debian images
# that include both MBR and EFI partitioning, LVM layout, systemd units,
# and complete metadata.

set -e

echo "==================================================================="
echo "  Debian Realistic Disk Image Tests"
echo "==================================================================="
echo ""

# Check for required tools
echo "Checking for required tools..."
for tool in qemu-nbd parted sgdisk; do
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
NEEDED=1000000  # 1GB in KB (512MB per image but allow for overhead)

if [ $AVAILABLE -lt $NEEDED ]; then
    echo "⚠  Warning: Low disk space in /tmp"
    echo "   Available: ${AVAILABLE}KB"
    echo "   Recommended: ${NEEDED}KB"
    echo ""
fi

# Clean up any existing test images
echo "Cleaning up old test images..."
rm -f /tmp/debian-test.img
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

# Test 1: Debian 11 (Bullseye) - MBR/BIOS
echo "Test 1: Debian 11 (Bullseye) - MBR/BIOS Layout"
echo "-------------------------------------------------------------------"
if cargo test --test debian_realistic test_debian_11_mbr -- --nocapture --test-threads=1; then
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
rm -f /tmp/debian-test.img
sleep 1

# Test 2: Debian 12 (Bookworm) - EFI/GPT
echo "Test 2: Debian 12 (Bookworm) - EFI/GPT Layout"
echo "-------------------------------------------------------------------"
if cargo test --test debian_realistic test_debian_12_efi -- --nocapture --test-threads=1; then
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
rm -f /tmp/debian-test.img
sleep 1

# Test 3: Debian 13 (Trixie) - EFI/GPT
echo "Test 3: Debian 13 (Trixie) - EFI/GPT Layout"
echo "-------------------------------------------------------------------"
if cargo test --test debian_realistic test_debian_13_efi -- --nocapture --test-threads=1; then
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
rm -f /tmp/debian-test.img
sleep 1

# Test 4: OS Inspection
echo "Test 4: Debian OS Inspection"
echo "-------------------------------------------------------------------"
if cargo test --test debian_realistic test_debian_inspection -- --nocapture --test-threads=1; then
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
rm -f /tmp/debian-test.img
sleep 1

# Test 5: LVM Layout Validation
echo "Test 5: LVM Layout Validation"
echo "-------------------------------------------------------------------"
if cargo test --test debian_realistic test_debian_lvm_layout -- --nocapture --test-threads=1; then
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
rm -f /tmp/debian-test.img
echo "  ✓ Test artifacts removed"
echo ""

echo "==================================================================="
echo "  ✓ ALL DEBIAN TESTS PASSED!"
echo "==================================================================="
echo ""
echo "Summary:"
echo "  • Debian 11 (Bullseye) - MBR/BIOS layout with LVM"
echo "  • Debian 12 (Bookworm) - EFI/GPT layout with LVM"
echo "  • Debian 13 (Trixie) - EFI/GPT layout with LVM"
echo "  • OS Inspection validation"
echo "  • LVM layout validation"
echo ""
echo "Features Tested:"
echo "  • Both MBR and GPT partitioning"
echo "  • EFI System Partition (GPT only)"
echo "  • LVM with multiple logical volumes (root, usr, var, home)"
echo "  • Multiple filesystem types (ext2, VFAT)"
echo "  • Systemd units (ssh, networking, journald)"
echo "  • dpkg package database (7 packages)"
echo "  • APT sources configuration"
echo "  • GRUB configuration (BIOS and EFI)"
echo "  • Debian metadata files"
echo "  • Network configuration"
echo "  • User accounts (root, debian with sudo)"
echo "  • Phase 3 APIs (stat, lstat, rm, rm_rf)"
echo "  • OS inspection APIs"
echo "  • LVM APIs (vgs, lvs)"
echo ""
echo "All Debian tests completed successfully!"
