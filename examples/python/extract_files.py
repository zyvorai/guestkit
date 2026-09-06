#!/usr/bin/env python3
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
"""
Extract files from VM disk image

This example shows how to extract specific files from a VM disk image
without booting the VM.
"""

import sys
import os
from guestkit import Guestfs

def extract_file(g, guest_path, local_path):
    """Extract a single file from guest to local filesystem"""
    try:
        if g.is_file(guest_path):
            g.download(guest_path, local_path)
            print(f"  ✓ {guest_path} → {local_path}")
            return True
        else:
            print(f"  ✗ {guest_path} not found or not a file")
            return False
    except Exception as e:
        print(f"  ✗ {guest_path}: {e}")
        return False

def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <disk-image> [output-dir]")
        print("\nExample:")
        print(f"  sudo python3 {sys.argv[0]} /path/to/vm.qcow2 ./extracted")
        sys.exit(1)

    disk_image = sys.argv[1]
    output_dir = sys.argv[2] if len(sys.argv) > 2 else "./extracted"

    # Create output directory
    os.makedirs(output_dir, exist_ok=True)

    print("=" * 60)
    print("GuestKit File Extraction Example")
    print("=" * 60)
    print(f"Disk image: {disk_image}")
    print(f"Output directory: {output_dir}")
    print()

    g = Guestfs()

    try:
        # Add and launch
        print("[1/4] Launching appliance...")
        g.add_drive_ro(disk_image)
        g.launch()

        # Inspect and mount
        print("[2/4] Inspecting OS...")
        roots = g.inspect_os()
        if not roots:
            print("  No OS detected")
            return

        root = roots[0]
        mountpoints = g.inspect_get_mountpoints(root)
        sorted_mounts = sorted(mountpoints.items(), key=lambda x: len(x[0]))

        print("[3/4] Mounting filesystems...")
        for mountpoint, device in sorted_mounts:
            try:
                g.mount_ro(device, mountpoint)
                print(f"  Mounted {device}")
            except Exception as e:
                print(f"  Warning: Could not mount {device}: {e}")

        # Extract files
        print("[4/4] Extracting files...")

        files_to_extract = [
            ("/etc/passwd", "passwd"),
            ("/etc/group", "group"),
            ("/etc/hostname", "hostname"),
            ("/etc/os-release", "os-release"),
            ("/etc/fstab", "fstab"),
            ("/etc/hosts", "hosts"),
            ("/etc/resolv.conf", "resolv.conf"),
        ]

        success_count = 0
        for guest_path, filename in files_to_extract:
            local_path = os.path.join(output_dir, filename)
            if extract_file(g, guest_path, local_path):
                success_count += 1

        print()
        print(f"✓ Extracted {success_count}/{len(files_to_extract)} files to {output_dir}")

        # Show extracted files
        print("\nExtracted files:")
        for f in os.listdir(output_dir):
            path = os.path.join(output_dir, f)
            size = os.path.getsize(path)
            print(f"  {f} ({size} bytes)")

    except Exception as e:
        print(f"\n✗ Error: {e}")
        sys.exit(1)

    finally:
        try:
            g.umount_all()
            g.shutdown()
        except:
            pass

if __name__ == "__main__":
    main()
