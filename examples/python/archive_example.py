#!/usr/bin/env python3
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
"""
Archive Operations Example

This example demonstrates how to:
- Extract tar archives into a VM disk
- Create tar archives from VM directories
- Work with compressed archives (tar.gz)
"""

import sys
import os
import tempfile
from guestkit import Guestfs

def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <disk-image>")
        print("\nExample:")
        print(f"  sudo python3 {sys.argv[0]} /path/to/vm.qcow2")
        sys.exit(1)

    disk_image = sys.argv[1]

    print("=" * 60)
    print("GuestKit Archive Operations Example")
    print("=" * 60)
    print()

    g = Guestfs()

    try:
        # Add disk (writable for this example)
        print("[1/5] Launching appliance...")
        g.add_drive(disk_image)
        g.launch()

        # Inspect and mount
        print("[2/5] Inspecting and mounting...")
        roots = g.inspect_os()
        if not roots:
            print("  No OS detected")
            return

        root = roots[0]
        mountpoints = g.inspect_get_mountpoints(root)
        sorted_mounts = sorted(mountpoints.items(), key=lambda x: len(x[0]))

        for mountpoint, device in sorted_mounts:
            try:
                g.mount(device, mountpoint)
                print(f"  Mounted {device} at {mountpoint}")
            except Exception as e:
                print(f"  Could not mount {device}: {e}")

        # === Create archive from guest directory ===
        print("\n[3/5] Creating archive from /etc...")

        with tempfile.NamedTemporaryFile(suffix=".tar.gz", delete=False) as tmp:
            archive_path = tmp.name

        try:
            # Create compressed archive of /etc
            g.tgz_out("/etc", archive_path)

            # Check archive size
            size_mb = os.path.getsize(archive_path) / (1024 * 1024)
            print(f"  ✓ Created {archive_path} ({size_mb:.2f} MB)")

            # List archive contents (using tar command)
            import subprocess
            result = subprocess.run(
                ["tar", "-tzf", archive_path],
                capture_output=True,
                text=True
            )
            files = result.stdout.strip().split('\n')
            print(f"  Archive contains {len(files)} files")
            print(f"  Sample files: {', '.join(files[:5])}...")

        except Exception as e:
            print(f"  Error creating archive: {e}")

        # === Extract archive into guest ===
        print("\n[4/5] Testing archive extraction...")

        try:
            # Create test directory in guest
            test_dir = "/tmp/guestkit-test"
            if not g.is_dir(test_dir):
                g.mkdir_p(test_dir)
                print(f"  Created {test_dir}")

            # Extract archive
            g.tgz_in(archive_path, test_dir)
            print(f"  ✓ Extracted archive to {test_dir}")

            # Verify extraction
            extracted_files = g.ls(test_dir)
            print(f"  Extracted {len(extracted_files)} items")

            # Cleanup test directory
            g.rm_rf(test_dir)
            print(f"  Cleaned up {test_dir}")

        except Exception as e:
            print(f"  Error during extraction: {e}")

        # === Other archive operations ===
        print("\n[5/5] Additional archive examples...")

        # Create uncompressed tar archive
        with tempfile.NamedTemporaryFile(suffix=".tar", delete=False) as tmp:
            uncompressed_archive = tmp.name

        try:
            # Archive /var/log (if exists)
            if g.is_dir("/var/log"):
                g.tar_out("/var/log", uncompressed_archive)
                size_mb = os.path.getsize(uncompressed_archive) / (1024 * 1024)
                print(f"  ✓ Created uncompressed archive: {uncompressed_archive} ({size_mb:.2f} MB)")
        except Exception as e:
            print(f"  Note: /var/log archive creation skipped: {e}")

        print("\n" + "=" * 60)
        print("✓ Archive operations completed")
        print("=" * 60)
        print("\nArchive tips:")
        print("  - Use tgz_in/tgz_out for compressed archives")
        print("  - Use tar_in/tar_out for uncompressed archives")
        print("  - Archives are created on the host filesystem")
        print("  - Extraction happens inside the guest filesystem")

        # Cleanup temporary files
        try:
            os.unlink(archive_path)
            if os.path.exists(uncompressed_archive):
                os.unlink(uncompressed_archive)
        except:
            pass

    except Exception as e:
        print(f"\n✗ Error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)

    finally:
        try:
            g.sync()
            g.umount_all()
            g.shutdown()
        except:
            pass

if __name__ == "__main__":
    main()
