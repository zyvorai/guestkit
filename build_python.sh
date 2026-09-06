#!/bin/bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Build script for GuestKit Python bindings
#
# This script helps build and install the Python bindings for GuestKit.
# It handles the PyO3 version compatibility automatically.

set -e

echo "========================================"
echo "GuestKit Python Bindings Builder"
echo "========================================"
echo

# Check for maturin
if ! command -v maturin &> /dev/null; then
    echo "Error: maturin not found"
    echo "Please install maturin:"
    echo "  pip install maturin"
    exit 1
fi

# Check for virtual environment
if [ -z "$VIRTUAL_ENV" ] && [ "$1" != "--system" ]; then
    echo "No virtual environment detected."
    echo
    echo "Options:"
    echo "  1. Create and use a virtual environment (recommended)"
    echo "  2. Install system-wide (requires --system flag)"
    echo
    read -p "Create virtual environment in .venv? [Y/n] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Nn]$ ]]; then
        echo "Creating virtual environment..."
        python3 -m venv .venv
        source .venv/bin/activate
        echo "✓ Virtual environment created and activated"
    else
        echo "Aborted. Either activate a virtual environment or run with --system"
        exit 1
    fi
fi

# Determine build mode
BUILD_MODE="develop"
if [ "$1" = "--release" ] || [ "$2" = "--release" ]; then
    BUILD_MODE="build --release"
    echo "Build mode: Release (optimized)"
else
    echo "Build mode: Development (debug)"
fi

# Build with PyO3 compatibility flag
echo
echo "Building Python bindings..."
echo "This may take a few minutes..."
echo

export PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1

if [ "$BUILD_MODE" = "develop" ]; then
    maturin develop --features python-bindings
else
    maturin build --release --features python-bindings

    # Install the wheel if in virtualenv
    if [ -n "$VIRTUAL_ENV" ]; then
        echo
        echo "Installing wheel..."
        WHEEL=$(ls -t target/wheels/guestkit-*.whl | head -1)
        pip install --force-reinstall "$WHEEL"
    fi
fi

echo
echo "========================================"
echo "✓ Build completed successfully!"
echo "========================================"
echo

# Test the installation
echo "Testing installation..."
python3 -c "import guestkit; print(f'GuestKit version: {guestkit.__version__}')" && \
python3 -c "from guestkit import Guestfs, DiskConverter; print('✓ All imports successful')"

echo
echo "Python bindings are ready to use!"
echo
echo "To test the bindings, try:"
echo "  cd examples/python"
echo "  sudo python3 test_bindings.py /path/to/disk.img"
echo
