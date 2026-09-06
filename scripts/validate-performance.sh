#!/bin/bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Performance validation script for guestkit
#
# This script runs performance validation tests and compares against baseline.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PERF_DIR="$PROJECT_ROOT/performance-results"
BASELINE_FILE="$PERF_DIR/baseline.json"
CURRENT_FILE="$PERF_DIR/current.json"
REPORT_FILE="$PERF_DIR/validation-report.md"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Create performance results directory
mkdir -p "$PERF_DIR"

echo "🚀 Performance Validation Framework"
echo "===================================="
echo ""

# Run performance validation tests
echo "📊 Running performance validation tests..."
cd "$PROJECT_ROOT"
cargo test --release --test performance_validation -- --nocapture --test-threads=1 > "$PERF_DIR/test-output.txt" 2>&1

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓${NC} Performance tests completed successfully"
else
    echo -e "${RED}✗${NC} Performance tests failed"
    cat "$PERF_DIR/test-output.txt"
    exit 1
fi

# Extract performance metrics
echo ""
echo "📈 Extracting performance metrics..."

# Parse test output for timing information
grep -E "test.*ok|Duration" "$PERF_DIR/test-output.txt" > "$CURRENT_FILE" || true

# Check if baseline exists
if [ -f "$BASELINE_FILE" ]; then
    echo ""
    echo "🔍 Comparing against baseline..."

    # Simple comparison (in production, this would be more sophisticated)
    if diff -q "$BASELINE_FILE" "$CURRENT_FILE" > /dev/null; then
        echo -e "${GREEN}✓${NC} Performance matches baseline"
    else
        echo -e "${YELLOW}⚠${NC}  Performance differs from baseline"
        echo "    See detailed report: $REPORT_FILE"
    fi
else
    echo ""
    echo -e "${YELLOW}ℹ${NC}  No baseline found. Creating baseline..."
    cp "$CURRENT_FILE" "$BASELINE_FILE"
    echo -e "${GREEN}✓${NC} Baseline created: $BASELINE_FILE"
fi

# Generate validation report
echo ""
echo "📝 Generating validation report..."

cat > "$REPORT_FILE" << 'EOF'
# Performance Validation Report

**Date:** $(date '+%Y-%m-%d %H:%M:%S')
**Commit:** $(git rev-parse --short HEAD 2>/dev/null || echo "N/A")

## Test Execution

All performance validation tests completed successfully.

## Results Summary

EOF

# Append test results
echo "### Test Results" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"
grep "test " "$PERF_DIR/test-output.txt" | head -20 >> "$REPORT_FILE" || echo "No detailed results available" >> "$REPORT_FILE"

# Add comparison section if baseline exists
if [ -f "$BASELINE_FILE" ]; then
    echo "" >> "$REPORT_FILE"
    echo "## Baseline Comparison" >> "$REPORT_FILE"
    echo "" >> "$REPORT_FILE"

    if diff -q "$BASELINE_FILE" "$CURRENT_FILE" > /dev/null; then
        echo "✅ **No performance regressions detected**" >> "$REPORT_FILE"
    else
        echo "⚠️  **Performance differences detected**" >> "$REPORT_FILE"
        echo "" >> "$REPORT_FILE"
        echo "See diff:" >> "$REPORT_FILE"
        echo '```' >> "$REPORT_FILE"
        diff "$BASELINE_FILE" "$CURRENT_FILE" >> "$REPORT_FILE" || true
        echo '```' >> "$REPORT_FILE"
    fi
fi

# Add system information
echo "" >> "$REPORT_FILE"
echo "## System Information" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"
echo "- **OS:** $(uname -s)" >> "$REPORT_FILE"
echo "- **Kernel:** $(uname -r)" >> "$REPORT_FILE"
echo "- **Architecture:** $(uname -m)" >> "$REPORT_FILE"
echo "- **CPU Cores:** $(nproc 2>/dev/null || echo "N/A")" >> "$REPORT_FILE"

echo -e "${GREEN}✓${NC} Validation report generated: $REPORT_FILE"

# Display summary
echo ""
echo "📊 Performance Validation Summary"
echo "=================================="
echo ""
echo "Test Output:    $PERF_DIR/test-output.txt"
echo "Current Results: $CURRENT_FILE"
echo "Baseline:        $BASELINE_FILE"
echo "Report:          $REPORT_FILE"
echo ""

# Check for regressions
if [ -f "$BASELINE_FILE" ] && ! diff -q "$BASELINE_FILE" "$CURRENT_FILE" > /dev/null; then
    echo -e "${YELLOW}⚠  Performance changes detected. Review the report for details.${NC}"
    exit 2
else
    echo -e "${GREEN}✓  All validation checks passed!${NC}"
    exit 0
fi
