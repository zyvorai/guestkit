#!/usr/bin/env python3
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
"""
Basic OS Inspection with GuestKit Python Bindings

This example demonstrates the core OS inspection capabilities
of GuestKit from Python.

Usage:
    python basic_inspection.py <disk-image>

Example:
    python basic_inspection.py /path/to/ubuntu.qcow2
"""

import sys
from guestkit import Guestfs

def main():
    if len(sys.argv) < 2:
        print("Usage: {} <disk-image>".format(sys.argv[0]))
        print("Example: {} /path/to/vm.qcow2".format(sys.argv[0]))
        sys.exit(1)

    disk_path = sys.argv[1]

    print("=== GuestKit Python Bindings - OS Inspection ===")
    print(f"Image: {disk_path}\n")

    # Create GuestFS handle
    g = Guestfs()

    # Add disk image (read-only for safety)
    print("[1/4] Adding disk image...")
    g.add_drive_ro(disk_path)

    # Launch the appliance
    print("[2/4] Launching appliance...")
    g.launch()

    # Inspect operating systems
    print("[3/4] Detecting operating systems...\n")
    roots = g.inspect_os()

    if not roots:
        print("No operating systems detected.")
        return

    print(f"Found {len(roots)} operating system(s):\n")

    for i, root in enumerate(roots, 1):
        print(f"=== OS #{i} ===")
        print(f"Root device: {root}")

        # Get OS type
        try:
            os_type = g.inspect_get_type(root)
            print(f"Type: {os_type}")
        except Exception as e:
            print(f"Type: Error - {e}")

        # Get distribution (for Linux)
        try:
            distro = g.inspect_get_distro(root)
            print(f"Distribution: {distro}")
        except Exception:
            pass

        # Get version
        try:
            major = g.inspect_get_major_version(root)
            minor = g.inspect_get_minor_version(root)
            print(f"Version: {major}.{minor}")
        except Exception:
            pass

        # Get product name
        try:
            product = g.inspect_get_product_name(root)
            print(f"Product: {product}")
        except Exception:
            pass

        # Get architecture
        try:
            arch = g.inspect_get_arch(root)
            print(f"Architecture: {arch}")
        except Exception:
            pass

        # Get hostname
        try:
            hostname = g.inspect_get_hostname(root)
            print(f"Hostname: {hostname}")
        except Exception:
            pass

        # Get package format
        try:
            pkg_format = g.inspect_get_package_format(root)
            print(f"Package format: {pkg_format}")
        except Exception:
            pass

        # Get package management
        try:
            pkg_mgmt = g.inspect_get_package_management(root)
            print(f"Package management: {pkg_mgmt}")
        except Exception:
            pass

        # List mountpoints
        try:
            mountpoints = g.inspect_get_mountpoints(root)
            if mountpoints:
                print("\nMountpoints:")
                for mount, device in mountpoints:
                    print(f"  {mount} -> {device}")
        except Exception:
            pass

        print()

    # Cleanup
    print("[4/4] Cleaning up...")
    g.shutdown()

    print("✓ Inspection complete!")

if __name__ == "__main__":
    main()
