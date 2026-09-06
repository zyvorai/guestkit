#!/bin/bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Performance tracking script for guestkit
# Runs benchmarks and tracks performance metrics over time

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
PERF_LOG="$PROJECT_ROOT/performance-log.txt"
BENCH_RESULTS="$PROJECT_ROOT/bench-results.txt"

echo "=== GuestCtl Performance Tracking ==="
echo "Date: $(date '+%Y-%m-%d %H:%M:%S')"
echo ""

# Run benchmarks
echo "Running performance benchmarks..."
cd "$PROJECT_ROOT"
cargo bench --bench performance 2>&1 | tee "$BENCH_RESULTS"

echo ""
echo "=== Performance Summary ==="

# Extract key metrics (these are examples, adjust based on actual benchmark output)
if grep -q "appliance/launch_and_shutdown" "$BENCH_RESULTS"; then
    LAUNCH_TIME=$(grep "appliance/launch_and_shutdown" "$BENCH_RESULTS" | awk '{print $4}' | sed 's/ms//')
    echo "Appliance Launch: ${LAUNCH_TIME}ms"
fi

if grep -q "cache/bincode_serialize" "$BENCH_RESULTS"; then
    BINCODE_SER=$(grep "cache/bincode_serialize" "$BENCH_RESULTS" | awk '{print $4}')
    echo "Bincode Serialize: $BINCODE_SER"
fi

if grep -q "cache/json_serialize" "$BENCH_RESULTS"; then
    JSON_SER=$(grep "cache/json_serialize" "$BENCH_RESULTS" | awk '{print $4}')
    echo "JSON Serialize: $JSON_SER"
fi

if grep -q "parallel/parallel" "$BENCH_RESULTS"; then
    PARALLEL_TIME=$(grep "parallel/parallel" "$BENCH_RESULTS" | awk '{print $4}')
    echo "Parallel Processing: $PARALLEL_TIME"
fi

# Append to log
echo "" >> "$PERF_LOG"
echo "=== $(date '+%Y-%m-%d %H:%M:%S') ===" >> "$PERF_LOG"
echo "Launch: ${LAUNCH_TIME:-N/A}ms" >> "$PERF_LOG"
echo "Bincode: ${BINCODE_SER:-N/A}" >> "$PERF_LOG"
echo "JSON: ${JSON_SER:-N/A}" >> "$PERF_LOG"
echo "Parallel: ${PARALLEL_TIME:-N/A}" >> "$PERF_LOG"

echo ""
echo "Performance log updated: $PERF_LOG"
echo "Full results: $BENCH_RESULTS"
echo "HTML report: $PROJECT_ROOT/target/criterion/report/index.html"

# Check if targets are met (20% improvement)
echo ""
echo "=== Target Validation ==="

# Baseline: 2500ms, Target: 2000ms
if [ -n "$LAUNCH_TIME" ]; then
    if (( $(echo "$LAUNCH_TIME < 2000" | bc -l 2>/dev/null || echo 0) )); then
        echo "✅ Launch time target MET (${LAUNCH_TIME}ms < 2000ms)"
    else
        echo "❌ Launch time target NOT MET (${LAUNCH_TIME}ms >= 2000ms)"
    fi
fi

echo ""
echo "Run 'open target/criterion/report/index.html' to view detailed reports"
