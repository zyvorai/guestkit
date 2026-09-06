#!/bin/bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Generate flamegraph profiles for guestkit operations
# Requires: cargo-flamegraph (install with: cargo install flamegraph)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
PROFILE_DIR="$PROJECT_ROOT/flamegraphs"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo "🔥 GuestCtl Flamegraph Profiler"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Check if flamegraph is installed
if ! command -v flamegraph &> /dev/null; then
    echo -e "${YELLOW}⚠${NC}  cargo-flamegraph not found"
    echo ""
    echo "Install with:"
    echo "  cargo install flamegraph"
    echo ""
    echo "You may also need to install perf:"
    echo "  sudo dnf install perf  # Fedora"
    echo "  sudo apt install linux-tools-common  # Ubuntu"
    echo ""
    exit 1
fi

# Create profile directory
mkdir -p "$PROFILE_DIR"

echo -e "${BLUE}📂${NC} Profile directory: $PROFILE_DIR"
echo ""

# Profile different operations
OPERATIONS=(
    "inspect"
    "list"
    "packages"
)

echo "Select operation to profile:"
echo ""
echo "  1) inspect  - VM disk inspection"
echo "  2) list     - File listing"
echo "  3) packages - Package enumeration"
echo "  4) all      - Profile all operations"
echo ""
read -p "Choice [1-4]: " choice

case $choice in
    1)
        SELECTED="inspect"
        ;;
    2)
        SELECTED="list"
        ;;
    3)
        SELECTED="packages"
        ;;
    4)
        SELECTED="all"
        ;;
    *)
        echo "Invalid choice"
        exit 1
        ;;
esac

cd "$PROJECT_ROOT"

profile_operation() {
    local op=$1
    local output="$PROFILE_DIR/flamegraph-$op-$(date +%Y%m%d-%H%M%S).svg"

    echo ""
    echo -e "${BLUE}🔥${NC} Profiling: $op"
    echo ""

    case $op in
        inspect)
            echo "Note: Update this command with actual disk image path"
            echo "Example: cargo flamegraph --output=$output -- inspect /path/to/disk.qcow2"
            ;;
        list)
            echo "Note: Update this command with actual disk image path"
            echo "Example: cargo flamegraph --output=$output -- list /path/to/disk.qcow2 /"
            ;;
        packages)
            echo "Note: Update this command with actual disk image path"
            echo "Example: cargo flamegraph --output=$output -- packages /path/to/disk.qcow2"
            ;;
    esac

    echo ""
    echo -e "${GREEN}✓${NC} Flamegraph would be saved to: $output"
}

if [ "$SELECTED" = "all" ]; then
    for op in "${OPERATIONS[@]}"; do
        profile_operation "$op"
    done
else
    profile_operation "$SELECTED"
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📊 Flamegraph Profiling Complete"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "To profile with actual VM image:"
echo "  cargo flamegraph --output=flamegraph.svg -- inspect /path/to/vm.qcow2"
echo ""
echo "View flamegraph:"
echo "  firefox $PROFILE_DIR/flamegraph-*.svg"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
