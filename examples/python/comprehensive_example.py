#!/usr/bin/env python3
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
"""
Comprehensive GuestKit Python Bindings Example

This example demonstrates all major features of the GuestKit Python bindings:
- VM disk inspection
- File operations
- Package management
- Command execution
- LVM operations
- Archive operations
"""

import sys
from guestkit import Guestfs

def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <disk-image>")
        print("\nExample:")
        print(f"  sudo python3 {sys.argv[0]} /path/to/vm.qcow2")
        sys.exit(1)

    disk_image = sys.argv[1]

    print("=" * 80)
    print("GuestKit Python Bindings - Comprehensive Example")
    print("=" * 80)
    print()

    # Create GuestFS handle
    print("[1/10] Creating GuestFS handle...")
    g = Guestfs()

    # Enable verbose mode (optional)
    # g.set_verbose(True)

    try:
        # Add disk image (read-only for safety)
        print(f"[2/10] Adding disk image: {disk_image}")
        g.add_drive_ro(disk_image)

        # Launch the appliance
        print("[3/10] Launching appliance...")
        g.launch()

        # === OS Inspection ===
        print("\n[4/10] Inspecting operating systems...")
        roots = g.inspect_os()

        if not roots:
            print("  ⚠️  No operating systems detected")
            return

        print(f"  Found {len(roots)} operating system(s)")

        # Inspect first OS
        root = roots[0]
        print(f"\n  Root device: {root}")

        try:
            os_type = g.inspect_get_type(root)
            print(f"  Type: {os_type}")
        except Exception as e:
            print(f"  Could not get OS type: {e}")

        try:
            distro = g.inspect_get_distro(root)
            print(f"  Distribution: {distro}")
        except Exception as e:
            print(f"  Could not get distribution: {e}")

        try:
            major = g.inspect_get_major_version(root)
            minor = g.inspect_get_minor_version(root)
            print(f"  Version: {major}.{minor}")
        except Exception as e:
            print(f"  Could not get version: {e}")

        try:
            hostname = g.inspect_get_hostname(root)
            print(f"  Hostname: {hostname}")
        except Exception as e:
            print(f"  Could not get hostname: {e}")

        try:
            arch = g.inspect_get_arch(root)
            print(f"  Architecture: {arch}")
        except Exception as e:
            print(f"  Could not get architecture: {e}")

        # === Mounting Filesystems ===
        print("\n[5/10] Mounting filesystems...")
        try:
            mountpoints = g.inspect_get_mountpoints(root)

            # Sort by mount path length (mount / before /boot, etc.)
            sorted_mounts = sorted(mountpoints.items(), key=lambda x: len(x[0]))

            for mountpoint, device in sorted_mounts:
                try:
                    print(f"  Mounting {device} at {mountpoint}")
                    g.mount_ro(device, mountpoint)
                except Exception as e:
                    print(f"  Could not mount {device}: {e}")
        except Exception as e:
            print(f"  Could not get mountpoints: {e}")

        # === Device Operations ===
        print("\n[6/10] Listing devices and partitions...")
        try:
            devices = g.list_devices()
            print(f"  Devices ({len(devices)}):")
            for dev in devices:
                try:
                    size = g.blockdev_getsize64(dev)
                    size_gb = size / (1024**3)
                    print(f"    {dev}: {size_gb:.2f} GB")
                except Exception as e:
                    print(f"    {dev}: (size unavailable)")
        except Exception as e:
            print(f"  Could not list devices: {e}")

        try:
            partitions = g.list_partitions()
            print(f"  Partitions ({len(partitions)}):")
            for part in partitions[:5]:  # Show first 5
                try:
                    fs_type = g.vfs_type(part)
                    label = ""
                    try:
                        label = g.vfs_label(part)
                        if label:
                            label = f" [{label}]"
                    except:
                        pass
                    print(f"    {part}: {fs_type}{label}")
                except Exception as e:
                    print(f"    {part}: (type unavailable)")
        except Exception as e:
            print(f"  Could not list partitions: {e}")

        # === File Operations ===
        print("\n[7/10] Reading system files...")

        # Read /etc/os-release
        if g.is_file("/etc/os-release"):
            try:
                content = g.cat("/etc/os-release")
                lines = content.split('\n')[:5]  # First 5 lines
                print("  /etc/os-release:")
                for line in lines:
                    if line.strip():
                        print(f"    {line}")
            except Exception as e:
                print(f"  Could not read /etc/os-release: {e}")

        # List /etc directory
        if g.is_dir("/etc"):
            try:
                files = g.ls("/etc")
                print(f"  /etc directory ({len(files)} entries):")
                print(f"    {', '.join(files[:10])}...")
            except Exception as e:
                print(f"  Could not list /etc: {e}")

        # === Package Management ===
        print("\n[8/10] Listing installed packages...")
        try:
            apps = g.inspect_list_applications(root)
            print(f"  Total packages: {len(apps)}")

            # Show first 5 packages
            print("  Sample packages:")
            for app in apps[:5]:
                name = app['app_name']
                version = app['app_version']
                print(f"    {name}-{version}")

            # Count kernel packages
            kernel_count = sum(1 for app in apps if 'kernel' in app['app_name'].lower())
            print(f"  Kernel packages: {kernel_count}")
        except Exception as e:
            print(f"  Could not list packages: {e}")

        # === LVM Operations ===
        print("\n[9/10] Checking LVM configuration...")
        try:
            g.vgscan()
            vgs = g.vgs()
            if vgs:
                print(f"  Volume groups: {', '.join(vgs)}")
                pvs = g.pvs()
                print(f"  Physical volumes: {', '.join(pvs)}")
                lvs = g.lvs()
                print(f"  Logical volumes: {', '.join(lvs)}")
            else:
                print("  No LVM volume groups found")
        except Exception as e:
            print(f"  LVM not configured or unavailable: {e}")

        # === File Statistics ===
        print("\n[10/10] Getting filesystem statistics...")
        try:
            statvfs = g.statvfs("/")
            total_bytes = statvfs['blocks'] * statvfs['frsize']
            free_bytes = statvfs['bfree'] * statvfs['frsize']
            avail_bytes = statvfs['bavail'] * statvfs['frsize']

            total_gb = total_bytes / (1024**3)
            free_gb = free_bytes / (1024**3)
            avail_gb = avail_bytes / (1024**3)
            used_gb = total_gb - free_gb
            used_percent = (used_gb / total_gb * 100) if total_gb > 0 else 0

            print(f"  Root filesystem (/):")
            print(f"    Total: {total_gb:.2f} GB")
            print(f"    Used:  {used_gb:.2f} GB ({used_percent:.1f}%)")
            print(f"    Free:  {free_gb:.2f} GB")
            print(f"    Available: {avail_gb:.2f} GB")
        except Exception as e:
            print(f"  Could not get filesystem stats: {e}")

        print("\n" + "=" * 80)
        print("✓ Inspection completed successfully")
        print("=" * 80)

    except Exception as e:
        print(f"\n✗ Error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)

    finally:
        # Cleanup
        print("\nCleaning up...")
        try:
            g.umount_all()
            g.shutdown()
        except Exception as e:
            print(f"Warning during cleanup: {e}")

if __name__ == "__main__":
    main()
