#!/usr/bin/env python3
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
"""
List Filesystems and Partitions

This example shows how to list and inspect partitions and filesystems
in a disk image using Python bindings.

Usage:
    python list_filesystems.py <disk-image>
"""

import sys
from guestkit import Guestfs

def format_size(bytes_val):
    """Format bytes into human-readable size."""
    for unit in ['B', 'KB', 'MB', 'GB', 'TB']:
        if bytes_val < 1024.0:
            return f"{bytes_val:.2f} {unit}"
        bytes_val /= 1024.0
    return f"{bytes_val:.2f} PB"

def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <disk-image>")
        sys.exit(1)

    disk_path = sys.argv[1]

    print("=== Filesystem and Partition Inspection ===")
    print(f"Image: {disk_path}\n")

    g = Guestfs()
    g.add_drive_ro(disk_path)
    g.launch()

    # List devices
    print("--- Devices ---")
    devices = g.list_devices()
    for device in devices:
        print(f"Device: {device}")

        # Get device size
        try:
            size = g.blockdev_getsize64(device)
            print(f"  Size: {format_size(size)}")
        except Exception:
            pass

        # Check if it's a partition table
        try:
            parttype = g.part_get_parttype(device)
            print(f"  Partition table: {parttype}")
        except Exception:
            pass

        print()

    # List partitions
    print("--- Partitions ---")
    partitions = g.list_partitions()
    for partition in partitions:
        print(f"Partition: {partition}")

        # Get partition size
        try:
            size = g.blockdev_getsize64(partition)
            print(f"  Size: {format_size(size)}")
        except Exception:
            pass

        # Get partition number
        try:
            partnum = g.part_to_partnum(partition)
            print(f"  Partition number: {partnum}")
        except Exception:
            pass

        # Get filesystem type
        try:
            fstype = g.vfs_type(partition)
            print(f"  Filesystem: {fstype}")
        except Exception:
            print("  Filesystem: (unknown)")

        # Get filesystem label
        try:
            label = g.vfs_label(partition)
            if label:
                print(f"  Label: {label}")
        except Exception:
            pass

        # Get filesystem UUID
        try:
            uuid = g.vfs_uuid(partition)
            if uuid:
                print(f"  UUID: {uuid}")
        except Exception:
            pass

        print()

    # List all filesystems
    print("--- All Filesystems ---")
    filesystems = g.list_filesystems()
    for device, fstype in filesystems.items():
        print(f"{device}: {fstype}")

    # List LVM information if available
    print("\n--- LVM Information ---")
    try:
        vgs = g.vgs()
        if vgs:
            print("Volume Groups:")
            for vg in vgs:
                print(f"  {vg}")
        else:
            print("No volume groups found")
    except Exception as e:
        print(f"LVM not available: {e}")

    try:
        lvs = g.lvs()
        if lvs:
            print("\nLogical Volumes:")
            for lv in lvs:
                print(f"  {lv}")
        else:
            print("No logical volumes found")
    except Exception:
        pass

    # Cleanup
    g.shutdown()
    print("\n✓ Complete!")

if __name__ == "__main__":
    main()
