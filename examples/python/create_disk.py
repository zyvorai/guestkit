#!/usr/bin/env python3
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
"""
Create and Configure Disk Image

This example demonstrates creating a new disk image with partitions
and filesystems using the GuestKit Python bindings.

Usage:
    python create_disk.py <output-image> [size-in-mb]

Example:
    python create_disk.py /tmp/test-disk.img 1024
"""

import sys
import os
from guestkit import Guestfs

def create_basic_disk(output_path, size_mb=1024):
    """Create a basic disk with GPT partition table and ext4 filesystem."""

    print("=== Creating Basic Disk Image ===")
    print(f"Output: {output_path}")
    print(f"Size: {size_mb} MB\n")

    # Create empty disk image
    print("[1/7] Creating empty disk image...")
    g = Guestfs()
    g.disk_create(output_path, "raw", size_mb * 1024 * 1024)
    print(f"✓ Created {output_path}")

    # Add the disk and launch
    print("\n[2/7] Adding disk and launching appliance...")
    g.add_drive(output_path)
    g.launch()
    print("✓ Appliance launched")

    # Create GPT partition table
    print("\n[3/7] Creating GPT partition table...")
    device = "/dev/sda"
    g.part_init(device, "gpt")
    print(f"✓ Created GPT partition table on {device}")

    # Create partitions
    print("\n[4/7] Creating partitions...")

    # EFI System Partition (512 MB)
    g.part_add(device, "primary", 2048, 1050623)  # ~512 MB
    efi_partition = "/dev/sda1"
    print(f"✓ Created EFI partition: {efi_partition}")

    # Root partition (remaining space)
    g.part_add(device, "primary", 1050624, -2048)
    root_partition = "/dev/sda2"
    print(f"✓ Created root partition: {root_partition}")

    # Set partition types
    g.part_set_gpt_type(efi_partition, "C12A7328-F81F-11D2-BA4B-00A0C93EC93B")  # EFI System
    g.part_set_gpt_type(root_partition, "0FC63DAF-8483-4772-8E79-3D69D8477DE4")  # Linux filesystem

    # Create filesystems
    print("\n[5/7] Creating filesystems...")

    # VFAT for EFI partition
    g.mkfs("vfat", efi_partition)
    g.set_label(efi_partition, "EFI")
    print(f"✓ Created VFAT filesystem on {efi_partition} with label 'EFI'")

    # ext4 for root partition
    g.mkfs("ext4", root_partition)
    g.set_label(root_partition, "rootfs")
    print(f"✓ Created ext4 filesystem on {root_partition} with label 'rootfs'")

    # Mount and create basic directory structure
    print("\n[6/7] Creating directory structure...")

    g.mount(root_partition, "/")
    g.mkdir("/boot")
    g.mount(efi_partition, "/boot")

    # Create standard Linux directories
    directories = [
        "/etc", "/home", "/root", "/var", "/tmp", "/usr",
        "/usr/bin", "/usr/lib", "/opt", "/srv", "/mnt"
    ]

    for directory in directories:
        g.mkdir(directory)
        print(f"  Created {directory}")

    # Create a simple file
    g.write("/etc/hostname", b"test-system\n")
    print("  Created /etc/hostname")

    # Create fstab
    fstab_content = f"""# /etc/fstab: static file system information
#
# <file system>  <mount point>  <type>  <options>         <dump>  <pass>
LABEL=rootfs     /              ext4    defaults          0       1
LABEL=EFI        /boot          vfat    defaults          0       2
"""
    g.write("/etc/fstab", fstab_content.encode())
    print("  Created /etc/fstab")

    # Sync and unmount
    print("\n[7/7] Finalizing...")
    g.sync()
    g.umount_all()
    g.shutdown()

    print(f"\n✓ Disk image created successfully: {output_path}")
    print(f"  Size: {os.path.getsize(output_path) / (1024*1024):.2f} MB")


def create_advanced_disk(output_path, size_mb=2048):
    """Create an advanced disk with multiple partitions and BTRFS."""

    print("=== Creating Advanced Disk Image (BTRFS with subvolumes) ===")
    print(f"Output: {output_path}")
    print(f"Size: {size_mb} MB\n")

    # Create empty disk image
    print("[1/8] Creating empty disk image...")
    g = Guestfs()
    g.disk_create(output_path, "raw", size_mb * 1024 * 1024)
    print(f"✓ Created {output_path}")

    # Add the disk and launch
    print("\n[2/8] Adding disk and launching appliance...")
    g.add_drive(output_path)
    g.launch()
    print("✓ Appliance launched")

    # Create GPT partition table
    print("\n[3/8] Creating GPT partition table...")
    device = "/dev/sda"
    g.part_init(device, "gpt")
    print(f"✓ Created GPT partition table on {device}")

    # Create partitions
    print("\n[4/8] Creating partitions...")

    # EFI System Partition (512 MB)
    g.part_add(device, "primary", 2048, 1050623)
    efi_partition = "/dev/sda1"

    # Swap partition (512 MB)
    g.part_add(device, "primary", 1050624, 2099199)
    swap_partition = "/dev/sda2"

    # Root partition (remaining space)
    g.part_add(device, "primary", 2099200, -2048)
    root_partition = "/dev/sda3"

    print(f"✓ Created EFI partition: {efi_partition}")
    print(f"✓ Created swap partition: {swap_partition}")
    print(f"✓ Created root partition: {root_partition}")

    # Set partition types
    g.part_set_gpt_type(efi_partition, "C12A7328-F81F-11D2-BA4B-00A0C93EC93B")  # EFI System
    g.part_set_gpt_type(swap_partition, "0657FD6D-A4AB-43C4-84E5-0933C84B4F4F")  # Linux swap
    g.part_set_gpt_type(root_partition, "0FC63DAF-8483-4772-8E79-3D69D8477DE4")  # Linux filesystem

    # Create filesystems
    print("\n[5/8] Creating filesystems...")

    # VFAT for EFI
    g.mkfs("vfat", efi_partition)
    g.set_label(efi_partition, "EFI")
    print(f"✓ Created VFAT on {efi_partition}")

    # Swap
    g.mkswap(swap_partition)
    g.set_label(swap_partition, "swap")
    print(f"✓ Created swap on {swap_partition}")

    # BTRFS for root
    g.mkfs("btrfs", root_partition)
    g.set_label(root_partition, "rootfs")
    print(f"✓ Created BTRFS on {root_partition}")

    # Create BTRFS subvolumes
    print("\n[6/8] Creating BTRFS subvolumes...")

    g.mount(root_partition, "/")

    # Create subvolumes
    subvolumes = ["@", "@home", "@var", "@snapshots"]
    for subvol in subvolumes:
        g.btrfs_subvolume_create(f"/{subvol}")
        print(f"  Created subvolume: {subvol}")

    g.umount_all()

    # Mount with subvolumes
    print("\n[7/8] Setting up directory structure...")

    # Mount @ as root
    g.mount_options("subvol=@,compress=zstd", root_partition, "/")

    # Create mount points
    g.mkdir("/boot")
    g.mkdir("/home")
    g.mkdir("/var")
    g.mkdir("/.snapshots")

    # Mount EFI
    g.mount(efi_partition, "/boot")

    # Mount other subvolumes
    g.mount_options("subvol=@home,compress=zstd", root_partition, "/home")
    g.mount_options("subvol=@var,compress=zstd", root_partition, "/var")
    g.mount_options("subvol=@snapshots", root_partition, "/.snapshots")

    # Create standard directories
    directories = [
        "/etc", "/root", "/tmp", "/usr", "/usr/bin",
        "/usr/lib", "/opt", "/srv", "/mnt"
    ]

    for directory in directories:
        g.mkdir(directory)

    # Create fstab with BTRFS subvolumes
    fstab_content = f"""# /etc/fstab: static file system information
#
# <file system>              <mount point>  <type>  <options>                    <dump>  <pass>
LABEL=rootfs                 /              btrfs   subvol=@,compress=zstd       0       0
LABEL=rootfs                 /home          btrfs   subvol=@home,compress=zstd   0       0
LABEL=rootfs                 /var           btrfs   subvol=@var,compress=zstd    0       0
LABEL=rootfs                 /.snapshots    btrfs   subvol=@snapshots            0       0
LABEL=EFI                    /boot          vfat    defaults                     0       2
LABEL=swap                   none           swap    defaults                     0       0
"""
    g.write("/etc/fstab", fstab_content.encode())
    g.write("/etc/hostname", b"btrfs-system\n")

    print("  Created directory structure and configuration files")

    # Sync and unmount
    print("\n[8/8] Finalizing...")
    g.sync()
    g.umount_all()
    g.shutdown()

    print(f"\n✓ Advanced disk image created successfully: {output_path}")
    print(f"  Size: {os.path.getsize(output_path) / (1024*1024):.2f} MB")


def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <output-image> [size-in-mb] [--advanced]")
        print(f"\nExamples:")
        print(f"  {sys.argv[0]} /tmp/basic-disk.img 1024")
        print(f"  {sys.argv[0]} /tmp/advanced-disk.img 2048 --advanced")
        sys.exit(1)

    output_path = sys.argv[1]

    # Parse size
    size_mb = 1024
    if len(sys.argv) >= 3 and sys.argv[2].isdigit():
        size_mb = int(sys.argv[2])

    # Check if advanced mode
    advanced = "--advanced" in sys.argv

    # Verify output path doesn't exist
    if os.path.exists(output_path):
        response = input(f"Warning: {output_path} already exists. Overwrite? [y/N] ")
        if response.lower() != 'y':
            print("Aborted.")
            sys.exit(1)
        os.remove(output_path)

    try:
        if advanced:
            create_advanced_disk(output_path, size_mb)
        else:
            create_basic_disk(output_path, size_mb)

    except Exception as e:
        print(f"\n✗ Error: {e}")
        # Clean up partial file
        if os.path.exists(output_path):
            os.remove(output_path)
        sys.exit(1)

if __name__ == "__main__":
    main()
