#!/usr/bin/env python3
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
"""
Mount and Explore Filesystems

This example demonstrates mounting filesystems and exploring their contents
using the GuestKit Python bindings.

Usage:
    python mount_and_explore.py <disk-image>

Example:
    python mount_and_explore.py /path/to/ubuntu.qcow2
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

def explore_directory(g, path, indent=0):
    """Recursively explore directory contents."""
    prefix = "  " * indent

    try:
        entries = g.ls(path)
        for entry in entries[:20]:  # Limit to first 20 entries
            full_path = f"{path}/{entry}" if path != "/" else f"/{entry}"

            try:
                stat = g.statns(full_path)

                # Determine file type
                if stat['st_mode'] & 0o040000:  # Directory
                    print(f"{prefix}📁 {entry}/")
                elif stat['st_mode'] & 0o100000:  # Regular file
                    size = format_size(stat['st_size'])
                    print(f"{prefix}📄 {entry} ({size})")
                elif stat['st_mode'] & 0o120000:  # Symbolic link
                    try:
                        target = g.readlink(full_path)
                        print(f"{prefix}🔗 {entry} -> {target}")
                    except Exception:
                        print(f"{prefix}🔗 {entry}")
                else:
                    print(f"{prefix}⚙️  {entry}")

            except Exception:
                print(f"{prefix}❓ {entry}")

    except Exception as e:
        print(f"{prefix}Error reading directory: {e}")

def read_file_sample(g, path, max_lines=10):
    """Read first few lines of a text file."""
    try:
        content = g.read_file(path)
        if isinstance(content, bytes):
            content = content.decode('utf-8', errors='replace')

        lines = content.split('\n')
        result = '\n'.join(lines[:max_lines])

        if len(lines) > max_lines:
            result += f"\n... ({len(lines) - max_lines} more lines)"

        return result
    except Exception as e:
        return f"Error reading file: {e}"

def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <disk-image>")
        print(f"Example: {sys.argv[0]} /path/to/ubuntu.qcow2")
        sys.exit(1)

    disk_path = sys.argv[1]

    print("=== Mount and Explore Filesystems ===")
    print(f"Image: {disk_path}\n")

    g = Guestfs()
    g.add_drive_ro(disk_path)
    g.launch()

    # Inspect operating systems
    print("[1/5] Detecting operating systems...")
    roots = g.inspect_os()

    if not roots:
        print("No operating systems detected.")
        g.shutdown()
        return

    root = roots[0]  # Use first OS
    print(f"Root device: {root}")

    # Get OS info
    try:
        os_type = g.inspect_get_type(root)
        print(f"OS Type: {os_type}")
    except Exception:
        pass

    try:
        distro = g.inspect_get_distro(root)
        print(f"Distribution: {distro}")
    except Exception:
        pass

    # Mount filesystems
    print("\n[2/5] Mounting filesystems...")
    try:
        mountpoints = g.inspect_get_mountpoints(root)

        # Sort by mount path length (mount / before /usr, etc.)
        mountpoints_sorted = sorted(mountpoints, key=lambda x: len(x[0]))

        for mount_path, device in mountpoints_sorted:
            try:
                g.mount_ro(device, mount_path)
                print(f"  Mounted {device} at {mount_path}")
            except Exception as e:
                print(f"  Failed to mount {device}: {e}")

    except Exception as e:
        print(f"Error getting mountpoints: {e}")
        g.shutdown()
        return

    # Explore root directory
    print("\n[3/5] Exploring root filesystem...")
    explore_directory(g, "/", indent=0)

    # Read interesting files
    print("\n[4/5] Reading system files...")

    interesting_files = [
        ("/etc/os-release", "OS Release Information"),
        ("/etc/hostname", "Hostname"),
        ("/etc/fstab", "Filesystem Table"),
        ("/etc/hosts", "Hosts File"),
    ]

    for file_path, description in interesting_files:
        try:
            if g.is_file(file_path):
                print(f"\n--- {description} ({file_path}) ---")
                content = read_file_sample(g, file_path, max_lines=15)
                print(content)
        except Exception:
            pass

    # Disk usage information
    print("\n[5/5] Disk usage information...")
    try:
        statvfs = g.statvfs("/")

        total_bytes = statvfs['blocks'] * statvfs['bsize']
        free_bytes = statvfs['bfree'] * statvfs['bsize']
        used_bytes = total_bytes - free_bytes

        print(f"Total space: {format_size(total_bytes)}")
        print(f"Used space:  {format_size(used_bytes)}")
        print(f"Free space:  {format_size(free_bytes)}")

        if total_bytes > 0:
            used_percent = (used_bytes / total_bytes) * 100
            print(f"Used:        {used_percent:.1f}%")

    except Exception as e:
        print(f"Could not get disk usage: {e}")

    # Cleanup
    g.umount_all()
    g.shutdown()
    print("\n✓ Exploration complete!")

if __name__ == "__main__":
    main()
