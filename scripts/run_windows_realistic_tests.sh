#!/bin/bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Run Windows realistic disk image tests
#
# This script tests guestkit with production-quality Windows images
# that include NTFS filesystem, registry simulation, and complete
# Windows directory structures for Windows 10, 11, and Server 2022.

set -e

echo "==================================================================="
echo "  Windows Realistic Disk Image Tests"
echo "==================================================================="
echo ""

# Check for required tools
echo "Checking for required tools..."
for tool in qemu-nbd parted sgdisk mkfs.ntfs; do
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
rm -f /tmp/windows-test.img
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

# Test 1: Windows 10 - MBR/BIOS
echo "Test 1: Windows 10 Pro - MBR/BIOS Layout"
echo "-------------------------------------------------------------------"
if cargo test --test windows_realistic test_windows_10_mbr -- --nocapture --test-threads=1; then
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
rm -f /tmp/windows-test.img
sleep 1

# Test 2: Windows 11 - EFI/GPT
echo "Test 2: Windows 11 Pro - EFI/GPT Layout"
echo "-------------------------------------------------------------------"
if cargo test --test windows_realistic test_windows_11_efi -- --nocapture --test-threads=1; then
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
rm -f /tmp/windows-test.img
sleep 1

# Test 3: Windows Server 2022 - EFI/GPT
echo "Test 3: Windows Server 2022 - EFI/GPT Layout"
echo "-------------------------------------------------------------------"
if cargo test --test windows_realistic test_windows_server_2022_efi -- --nocapture --test-threads=1; then
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
rm -f /tmp/windows-test.img
sleep 1

# Test 4: OS Inspection
echo "Test 4: Windows OS Inspection"
echo "-------------------------------------------------------------------"
if cargo test --test windows_realistic test_windows_inspection -- --nocapture --test-threads=1; then
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
rm -f /tmp/windows-test.img
sleep 1

# Test 5: NTFS Features
echo "Test 5: NTFS Feature Validation"
echo "-------------------------------------------------------------------"
if cargo test --test windows_realistic test_windows_ntfs_features -- --nocapture --test-threads=1; then
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
rm -f /tmp/windows-test.img
echo "  ✓ Test artifacts removed"
echo ""

echo "==================================================================="
echo "  ✓ ALL WINDOWS TESTS PASSED!"
echo "==================================================================="
echo ""
echo "Summary:"
echo "  • Windows 10 Pro - MBR/BIOS layout with NTFS"
echo "  • Windows 11 Pro - EFI/GPT layout with NTFS"
echo "  • Windows Server 2022 - EFI/GPT layout with NTFS"
echo "  • OS Inspection validation"
echo "  • NTFS feature validation"
echo ""
echo "Features Tested:"
echo "  • Both MBR and GPT partitioning"
echo "  • EFI System Partition + Microsoft Reserved Partition (GPT)"
echo "  • NTFS filesystem"
echo "  • Complete Windows directory structure"
echo "  • Windows registry simulation (SYSTEM, SOFTWARE, SAM, etc.)"
echo "  • Windows services configuration"
echo "  • Boot Configuration Data (BCD)"
echo "  • Event logs"
echo "  • User profiles"
echo "  • Windows Update metadata"
echo "  • Phase 3 APIs (stat, lstat, rm, rm_rf)"
echo "  • OS inspection APIs"
echo ""
echo "All Windows tests completed successfully!"
