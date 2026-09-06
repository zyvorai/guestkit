#!/usr/bin/env python3
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
"""
GuestKit Python Bindings Test Script

This script tests all major functionality of the GuestKit Python bindings.
It's designed to work with any Linux VM disk image.

Usage:
    sudo python3 test_bindings.py <disk-image>

Example:
    sudo python3 test_bindings.py /path/to/ubuntu.qcow2
"""

import sys
import os
import tempfile

def test_module_import():
    """Test 1: Module Import"""
    print("=" * 70)
    print("TEST 1: Module Import")
    print("=" * 70)

    try:
        import guestkit
        print(f"✓ GuestKit module imported successfully")
        print(f"  Version: {guestkit.__version__}")
        return True
    except ImportError as e:
        print(f"✗ Failed to import guestkit: {e}")
        print("\n  Installation steps:")
        print("    pip install maturin")
        print("    PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin develop --features python-bindings")
        return False

def test_guestfs_creation():
    """Test 2: Guestfs Handle Creation"""
    print("\n" + "=" * 70)
    print("TEST 2: Guestfs Handle Creation")
    print("=" * 70)

    try:
        from guestkit import Guestfs
        g = Guestfs()
        print("✓ Guestfs handle created successfully")
        g.shutdown()
        return True
    except Exception as e:
        print(f"✗ Failed to create Guestfs handle: {e}")
        return False

def test_disk_inspection(disk_path):
    """Test 3: Disk Inspection"""
    print("\n" + "=" * 70)
    print("TEST 3: Disk Inspection")
    print("=" * 70)

    from guestkit import Guestfs

    g = Guestfs()
    success = True

    try:
        # Add drive
        print(f"  Adding disk: {disk_path}")
        g.add_drive_ro(disk_path)
        print("  ✓ Disk added")

        # Launch
        print("  Launching appliance...")
        g.launch()
        print("  ✓ Appliance launched")

        # Inspect OS
        print("  Detecting operating systems...")
        roots = g.inspect_os()
        print(f"  ✓ Found {len(roots)} operating system(s)")

        if roots:
            root = roots[0]
            print(f"\n  OS Details:")
            print(f"    Root device: {root}")

            try:
                os_type = g.inspect_get_type(root)
                print(f"    Type: {os_type}")
            except Exception as e:
                print(f"    Type: (error: {e})")

            try:
                distro = g.inspect_get_distro(root)
                print(f"    Distribution: {distro}")
            except Exception as e:
                print(f"    Distribution: (error: {e})")

            try:
                major = g.inspect_get_major_version(root)
                minor = g.inspect_get_minor_version(root)
                print(f"    Version: {major}.{minor}")
            except Exception as e:
                print(f"    Version: (error: {e})")

            try:
                hostname = g.inspect_get_hostname(root)
                print(f"    Hostname: {hostname}")
            except Exception as e:
                print(f"    Hostname: (error: {e})")

            try:
                arch = g.inspect_get_arch(root)
                print(f"    Architecture: {arch}")
            except Exception as e:
                print(f"    Architecture: (error: {e})")

    except Exception as e:
        print(f"  ✗ Inspection failed: {e}")
        success = False

    finally:
        try:
            g.shutdown()
        except:
            pass

    return success

def test_device_operations(disk_path):
    """Test 4: Device Operations"""
    print("\n" + "=" * 70)
    print("TEST 4: Device Operations")
    print("=" * 70)

    from guestkit import Guestfs

    g = Guestfs()
    success = True

    try:
        g.add_drive_ro(disk_path)
        g.launch()

        # List devices
        print("  Listing devices...")
        devices = g.list_devices()
        print(f"  ✓ Found {len(devices)} device(s):")
        for dev in devices:
            try:
                size = g.blockdev_getsize64(dev)
                size_gb = size / (1024**3)
                print(f"    {dev}: {size_gb:.2f} GB")
            except Exception as e:
                print(f"    {dev}: (error getting size: {e})")

        # List partitions
        print("\n  Listing partitions...")
        partitions = g.list_partitions()
        print(f"  ✓ Found {len(partitions)} partition(s):")
        for part in partitions[:5]:  # First 5
            try:
                fs_type = g.vfs_type(part)
                print(f"    {part}: {fs_type}")
            except Exception as e:
                print(f"    {part}: (error: {e})")

    except Exception as e:
        print(f"  ✗ Device operations failed: {e}")
        success = False

    finally:
        try:
            g.shutdown()
        except:
            pass

    return success

def test_mount_and_files(disk_path):
    """Test 5: Mount and File Operations"""
    print("\n" + "=" * 70)
    print("TEST 5: Mount and File Operations")
    print("=" * 70)

    from guestkit import Guestfs

    g = Guestfs()
    success = True

    try:
        g.add_drive_ro(disk_path)
        g.launch()

        roots = g.inspect_os()
        if not roots:
            print("  ⚠ No OS detected, skipping mount test")
            return True

        root = roots[0]

        # Mount filesystems
        print("  Mounting filesystems...")
        mountpoints = g.inspect_get_mountpoints(root)
        for mp, dev in sorted(mountpoints.items(), key=lambda x: len(x[0])):
            try:
                g.mount_ro(dev, mp)
                print(f"    ✓ Mounted {dev} at {mp}")
            except Exception as e:
                print(f"    ✗ Failed to mount {dev}: {e}")

        # Test file operations
        print("\n  Testing file operations...")

        test_files = [
            "/etc/hostname",
            "/etc/os-release",
            "/etc/fstab",
        ]

        for file_path in test_files:
            try:
                if g.exists(file_path):
                    print(f"    ✓ {file_path} exists")

                    if g.is_file(file_path):
                        content = g.cat(file_path)
                        lines = content.split('\n')[:2]
                        print(f"      First line: {lines[0] if lines else '(empty)'}")
                else:
                    print(f"    - {file_path} does not exist")
            except Exception as e:
                print(f"    ✗ Error with {file_path}: {e}")

        # Test directory listing
        print("\n  Testing directory listing...")
        try:
            if g.is_dir("/etc"):
                files = g.ls("/etc")
                print(f"    ✓ /etc contains {len(files)} entries")
                print(f"      Sample: {', '.join(files[:5])}...")
        except Exception as e:
            print(f"    ✗ Error listing /etc: {e}")

    except Exception as e:
        print(f"  ✗ Mount/file operations failed: {e}")
        success = False

    finally:
        try:
            g.umount_all()
            g.shutdown()
        except:
            pass

    return success

def test_package_listing(disk_path):
    """Test 6: Package Listing"""
    print("\n" + "=" * 70)
    print("TEST 6: Package Listing")
    print("=" * 70)

    from guestkit import Guestfs

    g = Guestfs()
    success = True

    try:
        g.add_drive_ro(disk_path)
        g.launch()

        roots = g.inspect_os()
        if not roots:
            print("  ⚠ No OS detected, skipping package test")
            return True

        root = roots[0]

        # Mount filesystems
        mountpoints = g.inspect_get_mountpoints(root)
        for mp, dev in sorted(mountpoints.items(), key=lambda x: len(x[0])):
            try:
                g.mount_ro(dev, mp)
            except:
                pass

        # List packages
        print("  Listing installed packages...")
        try:
            apps = g.inspect_list_applications(root)
            print(f"  ✓ Found {len(apps)} packages")

            if apps:
                print(f"\n  Sample packages (first 5):")
                for app in apps[:5]:
                    name = app.get('app_name', 'unknown')
                    version = app.get('app_version', '')
                    print(f"    {name}-{version}")

                # Count kernels
                kernels = [a for a in apps if 'kernel' in a.get('app_name', '').lower()]
                print(f"\n  Kernel packages: {len(kernels)}")

        except Exception as e:
            print(f"  ✗ Package listing failed: {e}")
            success = False

    except Exception as e:
        print(f"  ✗ Package test failed: {e}")
        success = False

    finally:
        try:
            g.umount_all()
            g.shutdown()
        except:
            pass

    return success

def test_filesystem_stats(disk_path):
    """Test 7: Filesystem Statistics"""
    print("\n" + "=" * 70)
    print("TEST 7: Filesystem Statistics")
    print("=" * 70)

    from guestkit import Guestfs

    g = Guestfs()
    success = True

    try:
        g.add_drive_ro(disk_path)
        g.launch()

        roots = g.inspect_os()
        if not roots:
            print("  ⚠ No OS detected, skipping stats test")
            return True

        root = roots[0]

        # Mount
        mountpoints = g.inspect_get_mountpoints(root)
        for mp, dev in sorted(mountpoints.items(), key=lambda x: len(x[0])):
            try:
                g.mount_ro(dev, mp)
            except:
                pass

        # Get filesystem stats
        print("  Getting filesystem statistics...")
        try:
            statvfs = g.statvfs("/")
            total_bytes = statvfs['blocks'] * statvfs['frsize']
            free_bytes = statvfs['bfree'] * statvfs['frsize']

            total_gb = total_bytes / (1024**3)
            free_gb = free_bytes / (1024**3)
            used_gb = total_gb - free_gb
            used_percent = (used_gb / total_gb * 100) if total_gb > 0 else 0

            print(f"  ✓ Root filesystem (/):")
            print(f"    Total: {total_gb:.2f} GB")
            print(f"    Used:  {used_gb:.2f} GB ({used_percent:.1f}%)")
            print(f"    Free:  {free_gb:.2f} GB")

        except Exception as e:
            print(f"  ✗ Filesystem stats failed: {e}")
            success = False

        # Get file stats
        print("\n  Testing file stat...")
        try:
            if g.is_file("/etc/passwd"):
                stat = g.stat("/etc/passwd")
                print(f"  ✓ /etc/passwd:")
                print(f"    Size: {stat['size']} bytes")
                print(f"    UID: {stat['uid']}, GID: {stat['gid']}")
                print(f"    Mode: {oct(stat['mode'])}")
        except Exception as e:
            print(f"  ✗ File stat failed: {e}")

    except Exception as e:
        print(f"  ✗ Stats test failed: {e}")
        success = False

    finally:
        try:
            g.umount_all()
            g.shutdown()
        except:
            pass

    return success

def test_lvm_operations(disk_path):
    """Test 8: LVM Operations"""
    print("\n" + "=" * 70)
    print("TEST 8: LVM Operations")
    print("=" * 70)

    from guestkit import Guestfs

    g = Guestfs()
    success = True

    try:
        g.add_drive_ro(disk_path)
        g.launch()

        print("  Scanning for LVM volumes...")
        try:
            g.vgscan()
            vgs = g.vgs()

            if vgs:
                print(f"  ✓ Volume groups: {', '.join(vgs)}")

                pvs = g.pvs()
                print(f"  ✓ Physical volumes: {', '.join(pvs)}")

                lvs = g.lvs()
                print(f"  ✓ Logical volumes: {', '.join(lvs)}")
            else:
                print("  - No LVM configuration found")

        except Exception as e:
            print(f"  - LVM not available: {e}")

    except Exception as e:
        print(f"  ✗ LVM test failed: {e}")
        success = False

    finally:
        try:
            g.shutdown()
        except:
            pass

    return success

def test_checksum(disk_path):
    """Test 9: Checksum Operations"""
    print("\n" + "=" * 70)
    print("TEST 9: Checksum Operations")
    print("=" * 70)

    from guestkit import Guestfs

    g = Guestfs()
    success = True

    try:
        g.add_drive_ro(disk_path)
        g.launch()

        roots = g.inspect_os()
        if not roots:
            print("  ⚠ No OS detected, skipping checksum test")
            return True

        # Mount
        mountpoints = g.inspect_get_mountpoints(roots[0])
        for mp, dev in sorted(mountpoints.items(), key=lambda x: len(x[0])):
            try:
                g.mount_ro(dev, mp)
            except:
                pass

        # Test checksums
        print("  Testing checksum operations...")
        if g.is_file("/etc/passwd"):
            try:
                md5 = g.checksum("md5", "/etc/passwd")
                sha256 = g.checksum("sha256", "/etc/passwd")

                print(f"  ✓ /etc/passwd checksums:")
                print(f"    MD5:    {md5}")
                print(f"    SHA256: {sha256}")
            except Exception as e:
                print(f"  ✗ Checksum failed: {e}")
                success = False

    except Exception as e:
        print(f"  ✗ Checksum test failed: {e}")
        success = False

    finally:
        try:
            g.umount_all()
            g.shutdown()
        except:
            pass

    return success

def test_disk_converter():
    """Test 10: Disk Converter"""
    print("\n" + "=" * 70)
    print("TEST 10: Disk Converter")
    print("=" * 70)

    try:
        from guestkit import DiskConverter
        converter = DiskConverter()
        print("  ✓ DiskConverter created successfully")
        return True
    except Exception as e:
        print(f"  ✗ DiskConverter test failed: {e}")
        return False

def main():
    print("\n" + "=" * 70)
    print("GUESTKIT PYTHON BINDINGS TEST SUITE")
    print("=" * 70)
    print()

    # Check for disk image argument
    if len(sys.argv) < 2:
        print("Usage: sudo python3 test_bindings.py <disk-image>")
        print("\nExample:")
        print("  sudo python3 test_bindings.py /path/to/ubuntu.qcow2")
        print("\nNote: Most tests require a Linux VM disk image.")
        sys.exit(1)

    disk_path = sys.argv[1]

    # Verify disk exists
    if not os.path.exists(disk_path):
        print(f"Error: Disk image not found: {disk_path}")
        sys.exit(1)

    print(f"Disk image: {disk_path}")
    print()

    # Run tests
    results = []

    results.append(("Module Import", test_module_import()))

    if not results[0][1]:
        print("\n" + "=" * 70)
        print("ABORTED: Module import failed")
        print("=" * 70)
        sys.exit(1)

    results.append(("Handle Creation", test_guestfs_creation()))
    results.append(("Disk Inspection", test_disk_inspection(disk_path)))
    results.append(("Device Operations", test_device_operations(disk_path)))
    results.append(("Mount & Files", test_mount_and_files(disk_path)))
    results.append(("Package Listing", test_package_listing(disk_path)))
    results.append(("Filesystem Stats", test_filesystem_stats(disk_path)))
    results.append(("LVM Operations", test_lvm_operations(disk_path)))
    results.append(("Checksum", test_checksum(disk_path)))
    results.append(("Disk Converter", test_disk_converter()))

    # Summary
    print("\n" + "=" * 70)
    print("TEST SUMMARY")
    print("=" * 70)

    passed = sum(1 for _, success in results if success)
    total = len(results)

    for test_name, success in results:
        status = "✓ PASS" if success else "✗ FAIL"
        print(f"  {status:8} {test_name}")

    print()
    print(f"Results: {passed}/{total} tests passed")

    if passed == total:
        print("\n✓ All tests passed!")
        sys.exit(0)
    else:
        print(f"\n✗ {total - passed} test(s) failed")
        sys.exit(1)

if __name__ == "__main__":
    main()
