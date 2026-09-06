#!/bin/bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Run all Phase 3 tests individually with proper cleanup between runs
#
# This script runs each test function separately to avoid NBD device conflicts

set -e

echo "==================================================================="
echo "  Phase 3 Individual Test Execution"
echo "==================================================================="
echo ""

# Color codes
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counter
PASSED=0
FAILED=0
TOTAL=0

# Cleanup function
cleanup_nbd() {
    echo "  Cleaning up NBD devices..."
    for i in {0..15}; do
        sudo qemu-nbd --disconnect /dev/nbd$i 2>/dev/null || true
    done
    rm -f /tmp/*-test*.img 2>/dev/null || true
    sleep 1
}

# Run a single test
run_test() {
    local test_file=$1
    local test_name=$2
    local description=$3

    TOTAL=$((TOTAL + 1))
    echo ""
    echo "-------------------------------------------------------------------"
    echo "[$TOTAL] $description"
    echo "    Test: $test_name"
    echo "-------------------------------------------------------------------"

    # Cleanup before test
    cleanup_nbd

    # Run test
    cargo test --test "$test_file" "$test_name" -- --test-threads=1 2>&1 | tee /tmp/test_output_$test_name.log >/dev/null

    # Check result
    if grep -q "test result: ok" /tmp/test_output_$test_name.log; then
        echo -e "${GREEN}✓ PASSED${NC}"
        PASSED=$((PASSED + 1))
    else
        echo -e "${RED}✗ FAILED${NC}"
        FAILED=$((FAILED + 1))
        # Show error
        echo "    Error:"
        grep -E "Error:|FAILED|panicked" /tmp/test_output_$test_name.log | head -3 | sed 's/^/    /' || true
    fi
}

echo "==================================================================="
echo "  FEDORA TEST SUITE"
echo "==================================================================="

run_test "phase3_comprehensive" "test_create_vs_new" "API Alias: create() vs new()"
run_test "phase3_comprehensive" "test_stat_vs_lstat_behavior" "stat() vs lstat() on symlinks"
run_test "phase3_comprehensive" "test_rm_rm_rf_edge_cases" "rm() and rm_rf() edge cases"
run_test "phase3_comprehensive" "test_add_drive_vs_add_drive_ro" "add_drive() vs add_drive_ro()"
run_test "phase3_comprehensive" "test_phase3_comprehensive" "Comprehensive Fedora workflow"

echo ""
echo "==================================================================="
echo "  WINDOWS TEST SUITE"
echo "==================================================================="

run_test "phase3_windows" "test_windows_stat_vs_lstat" "stat() vs lstat() on Windows paths"
run_test "phase3_windows" "test_windows_rm_operations" "rm() on Windows directory structures"
run_test "phase3_windows" "test_windows_ntfs_features" "NTFS-specific features"
run_test "phase3_windows" "test_windows_long_paths" "Windows long path handling"
run_test "phase3_windows" "test_phase3_windows_comprehensive" "Comprehensive Windows workflow"

# Final cleanup
echo ""
echo "==================================================================="
echo "  CLEANUP"
echo "==================================================================="
cleanup_nbd
rm -f /tmp/test_output_*.log
echo "  ✓ Cleanup complete"
echo ""

# Summary
echo "==================================================================="
echo "  TEST SUMMARY"
echo "==================================================================="
echo ""
echo "Total Tests:  $TOTAL"
echo -e "Passed:       ${GREEN}$PASSED${NC}"
echo -e "Failed:       ${RED}$FAILED${NC}"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}✓ ALL TESTS PASSED!${NC}"
    echo ""
    echo "APIs Validated:"
    echo "  • Guestfs::create() - Handle creation alias"
    echo "  • add_drive() - Read-write drive mounting"
    echo "  • add_drive_ro() - Read-only drive mounting"
    echo "  • stat() - File metadata (follows symlinks)"
    echo "  • lstat() - File metadata (doesn't follow symlinks)"
    echo "  • rm() - Single file removal"
    echo "  • rm_rf() - Recursive force removal"
    echo "  • cpio_in() - CPIO archive extraction"
    echo "  • part_get_name() - Get partition label"
    echo "  • part_set_parttype() - Set partition table type"
    echo ""
    exit 0
else
    echo -e "${RED}✗ SOME TESTS FAILED${NC}"
    echo ""
    echo "Note: NBD-based tests may require:"
    echo "  1. sudo ./scripts/setup_test_env.sh"
    echo "  2. Running tests with: sudo -E <test command>"
    echo "  3. Manual NBD cleanup between runs"
    echo ""
    exit 1
fi
