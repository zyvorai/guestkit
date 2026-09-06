#!/usr/bin/env python3
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
"""
Package Inspection

This example demonstrates inspecting installed packages on different
Linux distributions using the GuestKit Python bindings.

Supports:
- Debian/Ubuntu (dpkg)
- Red Hat/Fedora/CentOS (RPM)
- Arch Linux (pacman)

Usage:
    python package_inspection.py <disk-image>

Example:
    python package_inspection.py /path/to/ubuntu.qcow2
"""

import sys
import re
from guestkit import Guestfs

def inspect_dpkg_packages(g, root):
    """Inspect packages on Debian/Ubuntu systems."""
    print("\n=== Debian/Ubuntu Package Inspection (dpkg) ===")

    try:
        # Get list of installed applications
        apps = g.inspect_list_applications(root)

        if not apps:
            print("No packages found via inspect_list_applications")
            return

        print(f"\nFound {len(apps)} installed packages\n")

        # Show first 20 packages
        print("Package Name                     Version                 Description")
        print("-" * 100)

        for app in apps[:20]:
            name = app.get('app_name', 'unknown')[:30]
            version = app.get('app_version', '')[:20]
            description = app.get('app_summary', '')[:40]

            print(f"{name:<32} {version:<23} {description}")

        if len(apps) > 20:
            print(f"\n... and {len(apps) - 20} more packages")

        # Show some statistics
        print(f"\n--- Package Statistics ---")

        # Count packages by prefix (common package categories)
        lib_count = sum(1 for app in apps if app.get('app_name', '').startswith('lib'))
        python_count = sum(1 for app in apps if 'python' in app.get('app_name', '').lower())
        kernel_count = sum(1 for app in apps if 'linux-' in app.get('app_name', ''))

        print(f"Total packages: {len(apps)}")
        print(f"Library packages (lib*): {lib_count}")
        print(f"Python-related packages: {python_count}")
        print(f"Kernel packages: {kernel_count}")

    except Exception as e:
        print(f"Error inspecting dpkg packages: {e}")

def inspect_rpm_packages(g, root):
    """Inspect packages on Red Hat/Fedora/CentOS systems."""
    print("\n=== Red Hat/Fedora Package Inspection (RPM) ===")

    try:
        # Get list of installed applications
        apps = g.inspect_list_applications(root)

        if not apps:
            print("No packages found via inspect_list_applications")
            return

        print(f"\nFound {len(apps)} installed packages\n")

        # Show first 20 packages
        print("Package Name                     Version                 Release")
        print("-" * 90)

        for app in apps[:20]:
            name = app.get('app_name', 'unknown')[:30]
            version = app.get('app_version', '')[:20]
            release = app.get('app_release', '')[:20]

            print(f"{name:<32} {version:<23} {release}")

        if len(apps) > 20:
            print(f"\n... and {len(apps) - 20} more packages")

        # Show some statistics
        print(f"\n--- Package Statistics ---")

        # Count packages by category
        systemd_count = sum(1 for app in apps if 'systemd' in app.get('app_name', '').lower())
        python_count = sum(1 for app in apps if 'python' in app.get('app_name', '').lower())
        kernel_count = sum(1 for app in apps if 'kernel' in app.get('app_name', ''))

        print(f"Total packages: {len(apps)}")
        print(f"Systemd-related packages: {systemd_count}")
        print(f"Python-related packages: {python_count}")
        print(f"Kernel packages: {kernel_count}")

    except Exception as e:
        print(f"Error inspecting RPM packages: {e}")

def inspect_pacman_packages(g):
    """Inspect packages on Arch Linux systems (manual parsing)."""
    print("\n=== Arch Linux Package Inspection (pacman) ===")

    pacman_db = "/var/lib/pacman/local"

    try:
        if not g.is_dir(pacman_db):
            print(f"Pacman database not found at {pacman_db}")
            return

        # List package directories
        packages = g.ls(pacman_db)

        # Filter out non-package entries
        package_list = [p for p in packages if p not in ['.', '..']]

        print(f"\nFound {len(package_list)} installed packages\n")

        # Show first 20 packages
        print("Package Name                     Version")
        print("-" * 70)

        for pkg in package_list[:20]:
            # Package format is typically: name-version-release
            parts = pkg.rsplit('-', 2)
            if len(parts) >= 2:
                name = parts[0][:30]
                version = '-'.join(parts[1:])[:30]
                print(f"{name:<32} {version}")
            else:
                print(f"{pkg[:60]}")

        if len(package_list) > 20:
            print(f"\n... and {len(package_list) - 20} more packages")

        print(f"\nTotal packages: {len(package_list)}")

    except Exception as e:
        print(f"Error inspecting pacman packages: {e}")

def show_package_managers(g, root):
    """Show available package managers."""
    print("\n--- Package Management Information ---")

    try:
        pkg_format = g.inspect_get_package_format(root)
        print(f"Package format: {pkg_format}")
    except Exception:
        pass

    try:
        pkg_mgmt = g.inspect_get_package_management(root)
        print(f"Package management: {pkg_mgmt}")
    except Exception:
        pass

def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <disk-image>")
        print(f"Example: {sys.argv[0]} /path/to/ubuntu.qcow2")
        sys.exit(1)

    disk_path = sys.argv[1]

    print("=== Package Inspection ===")
    print(f"Image: {disk_path}\n")

    g = Guestfs()
    g.add_drive_ro(disk_path)
    g.launch()

    # Inspect operating systems
    print("[1/3] Detecting operating systems...")
    roots = g.inspect_os()

    if not roots:
        print("No operating systems detected.")
        g.shutdown()
        return

    root = roots[0]
    print(f"Root device: {root}")

    # Get OS info
    try:
        os_type = g.inspect_get_type(root)
        distro = g.inspect_get_distro(root)
        print(f"OS: {distro} ({os_type})")
    except Exception as e:
        print(f"Error getting OS info: {e}")

    # Show package manager info
    show_package_managers(g, root)

    # Mount filesystems
    print("\n[2/3] Mounting filesystems...")
    try:
        mountpoints = g.inspect_get_mountpoints(root)
        mountpoints_sorted = sorted(mountpoints, key=lambda x: len(x[0]))

        for mount_path, device in mountpoints_sorted:
            try:
                g.mount_ro(device, mount_path)
                print(f"  Mounted {device} at {mount_path}")
            except Exception as e:
                print(f"  Failed to mount {device}: {e}")

    except Exception as e:
        print(f"Error mounting filesystems: {e}")

    # Inspect packages based on distribution
    print("\n[3/3] Inspecting installed packages...")

    try:
        distro = g.inspect_get_distro(root)

        if distro in ['ubuntu', 'debian', 'linuxmint', 'kali']:
            inspect_dpkg_packages(g, root)
        elif distro in ['fedora', 'rhel', 'centos', 'rocky', 'alma', 'scientificlinux']:
            inspect_rpm_packages(g, root)
        elif distro in ['archlinux', 'manjaro']:
            inspect_pacman_packages(g)
        else:
            print(f"Package inspection not implemented for distribution: {distro}")
            # Try generic inspection anyway
            try:
                apps = g.inspect_list_applications(root)
                print(f"Found {len(apps)} packages via generic inspection")
            except Exception as e:
                print(f"Generic inspection failed: {e}")

    except Exception as e:
        print(f"Error: {e}")

    # Cleanup
    g.umount_all()
    g.shutdown()
    print("\n✓ Package inspection complete!")

if __name__ == "__main__":
    main()
