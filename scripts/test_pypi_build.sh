#!/bin/bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Test script for PyPI package building and installation
# This script verifies that the package can be built and installed correctly

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "🔧 GuestKit PyPI Build Test"
echo "============================"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Helper functions
print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

# Check prerequisites
echo "📋 Checking prerequisites..."

if ! command -v python3 &> /dev/null; then
    print_error "python3 is not installed"
    exit 1
fi
print_success "Python 3 found: $(python3 --version)"

if ! command -v cargo &> /dev/null; then
    print_error "cargo is not installed"
    exit 1
fi
print_success "Cargo found: $(cargo --version)"

if ! python3 -m pip show maturin &> /dev/null; then
    print_warning "maturin not found, installing..."
    python3 -m pip install maturin
fi
print_success "Maturin found: $(python3 -m maturin --version)"

echo ""

# Set environment variable for Python 3.14+ compatibility
export PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1
print_info "Set PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1"

echo ""
echo "🔨 Building wheel..."
cd "$PROJECT_ROOT"

# Clean previous builds
rm -rf target/wheels/*.whl 2>/dev/null || true
rm -rf dist/*.whl 2>/dev/null || true

# Build wheel
if ! maturin build --release --features python-bindings; then
    print_error "Build failed"
    exit 1
fi

print_success "Build completed"

# Find the wheel
WHEEL=$(find target/wheels -name "guestkit-*.whl" | head -n 1)
if [ -z "$WHEEL" ]; then
    print_error "No wheel found in target/wheels/"
    exit 1
fi

print_success "Wheel created: $(basename "$WHEEL")"
print_info "Size: $(du -h "$WHEEL" | cut -f1)"

echo ""
echo "🧪 Testing installation..."

# Create temporary virtual environment
VENV_DIR=$(mktemp -d)
print_info "Creating test environment in $VENV_DIR"

python3 -m venv "$VENV_DIR"
source "$VENV_DIR/bin/activate"

# Install the wheel
print_info "Installing wheel..."
if ! pip install "$WHEEL"; then
    print_error "Installation failed"
    rm -rf "$VENV_DIR"
    exit 1
fi

print_success "Installation successful"

echo ""
echo "✅ Running import tests..."

# Test 1: Import Guestfs
if python3 -c "from guestkit import Guestfs" 2>/dev/null; then
    print_success "Guestfs import successful"
else
    print_error "Failed to import Guestfs"
    deactivate
    rm -rf "$VENV_DIR"
    exit 1
fi

# Test 2: Import DiskConverter
if python3 -c "from guestkit import DiskConverter" 2>/dev/null; then
    print_success "DiskConverter import successful"
else
    print_error "Failed to import DiskConverter"
    deactivate
    rm -rf "$VENV_DIR"
    exit 1
fi

# Test 3: Instantiate Guestfs
if python3 -c "from guestkit import Guestfs; g = Guestfs(); print('OK')" 2>/dev/null | grep -q "OK"; then
    print_success "Guestfs instantiation successful"
else
    print_error "Failed to instantiate Guestfs"
    deactivate
    rm -rf "$VENV_DIR"
    exit 1
fi

# Test 4: Context manager
if python3 -c "from guestkit import Guestfs; g = Guestfs(); g.__enter__(); print('OK')" 2>/dev/null | grep -q "OK"; then
    print_success "Context manager __enter__ works"
else
    print_error "Context manager failed"
    deactivate
    rm -rf "$VENV_DIR"
    exit 1
fi

# Test 5: Check type hints are included
if [ -f "$VENV_DIR/lib/python"*/site-packages/guestkit.pyi ]; then
    print_success "Type hints file (guestkit.pyi) included"
else
    print_warning "Type hints file not found (non-fatal)"
fi

# Test 6: Verify package metadata
echo ""
echo "📦 Package information:"
pip show guestkit | while IFS= read -r line; do
    echo "   $line"
done

# Cleanup
deactivate
rm -rf "$VENV_DIR"

echo ""
echo "🎉 All tests passed!"
echo ""
echo "📋 Summary:"
print_success "Wheel built successfully"
print_success "Installation works"
print_success "All imports work"
print_success "Context manager works"
echo ""
echo "✨ Ready for PyPI publication!"
echo ""
echo "Next steps:"
echo "  1. Test with TestPyPI:"
echo "     $ twine upload --repository testpypi target/wheels/*"
echo ""
echo "  2. Test installation from TestPyPI:"
echo "     $ pip install --index-url https://test.pypi.org/simple/ guestkit"
echo ""
echo "  3. If all looks good, publish to PyPI:"
echo "     $ git tag v0.3.0"
echo "     $ git push origin v0.3.0"
echo ""
echo "  Or see: docs/development/CONTRIBUTING.md"
