#!/bin/bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Run Ubuntu realistic disk image tests
#
# This script tests guestkit with production-quality Ubuntu images
# that include EFI partitioning, systemd units, and complete metadata.

set -e

echo "==================================================================="
echo "  Ubuntu Realistic Disk Image Tests"
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
NEEDED=2000000  # 2GB in KB

if [ $AVAILABLE -lt $NEEDED ]; then
    echo "⚠  Warning: Low disk space in /tmp"
    echo "   Available: ${AVAILABLE}KB"
    echo "   Recommended: ${NEEDED}KB"
    echo ""
fi

# Clean up any existing test images
echo "Cleaning up old test images..."
rm -f /tmp/ubuntu-efi-test.img
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

# Test 1: Ubuntu 22.04 LTS
echo "Test 1: Ubuntu 22.04 LTS (Jammy Jellyfish)"
echo "-------------------------------------------------------------------"
if cargo test --test ubuntu_realistic test_ubuntu_2204_realistic -- --nocapture --test-threads=1; then
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
rm -f /tmp/ubuntu-efi-test.img
sleep 1

# Test 2: Ubuntu 20.04 LTS
echo "Test 2: Ubuntu 20.04 LTS (Focal Fossa)"
echo "-------------------------------------------------------------------"
if cargo test --test ubuntu_realistic test_ubuntu_2004_realistic -- --nocapture --test-threads=1; then
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
rm -f /tmp/ubuntu-efi-test.img
sleep 1

# Test 3: Ubuntu 24.04 LTS
echo "Test 3: Ubuntu 24.04 LTS (Noble Numbat)"
echo "-------------------------------------------------------------------"
if cargo test --test ubuntu_realistic test_ubuntu_2404_realistic -- --nocapture --test-threads=1; then
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
rm -f /tmp/ubuntu-efi-test.img
sleep 1

# Test 4: OS Inspection
echo "Test 4: Ubuntu OS Inspection"
echo "-------------------------------------------------------------------"
if cargo test --test ubuntu_realistic test_ubuntu_inspection -- --nocapture --test-threads=1; then
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
rm -f /tmp/ubuntu-efi-test.img
echo "  ✓ Test artifacts removed"
echo ""

echo "==================================================================="
echo "  ✓ ALL UBUNTU TESTS PASSED!"
echo "==================================================================="
echo ""
echo "Summary:"
echo "  • Ubuntu 22.04 LTS (Jammy) - ext4 filesystem"
echo "  • Ubuntu 20.04 LTS (Focal) - ext4 filesystem"
echo "  • Ubuntu 24.04 LTS (Noble) - XFS filesystem"
echo "  • OS Inspection validation"
echo ""
echo "Features Tested:"
echo "  • GPT partitioning with EFI System Partition"
echo "  • Multiple filesystem types (ext4, XFS, VFAT)"
echo "  • Systemd units (ssh, networking, journald)"
echo "  • dpkg package database"
echo "  • APT sources configuration"
echo "  • GRUB configuration"
echo "  • Ubuntu metadata files"
echo "  • Phase 3 APIs (stat, lstat, rm, rm_rf)"
echo "  • OS inspection APIs"
echo ""
echo "All Ubuntu tests completed successfully!"
