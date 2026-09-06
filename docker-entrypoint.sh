#!/bin/bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
set -e

# Load kernel modules if not already loaded
if [ -w /dev ]; then
    # Check if nbd module is loaded
    if ! lsmod | grep -q nbd; then
        echo "Loading NBD kernel module..."
        modprobe nbd max_part=8 || echo "Warning: Could not load nbd module"
    fi

    # Check if loop module is loaded
    if ! lsmod | grep -q loop; then
        echo "Loading loop kernel module..."
        modprobe loop || echo "Warning: Could not load loop module"
    fi
fi

# Execute guestkit with provided arguments
exec guestkit "$@"
