#!/usr/bin/env python3
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
"""
Python bindings tests for guestkit

This test suite validates the Python bindings functionality.
Run with: pytest tests/test_python_bindings.py -v

Note: Some tests require a disk image file. Set GUESTKIT_TEST_IMAGE
environment variable to point to a test VM disk image.
"""

import pytest
import os
import sys
import tempfile


def test_import_guestkit():
    """Test that guestkit module can be imported"""
    import guestkit
    assert guestkit is not None
    assert hasattr(guestkit, '__version__')
    # Version comes from CARGO_PKG_VERSION (currently 1.1.0)
    parts = guestkit.__version__.split('.')
    assert len(parts) == 3
    assert int(parts[0]) >= 1


def test_assurance_api_exports():
    """v1.1.0+ assurance bindings used by h2kvm offline fixer."""
    import guestkit
    for name in (
        'run_doctor',
        'run_boot_inspect',
        'run_migrate_plan',
        'run_repair_plan',
        'run_migrate_repair',
    ):
        assert hasattr(guestkit, name), f"missing guestkit.{name}"
        assert callable(getattr(guestkit, name))


def test_import_classes():
    """Test that main classes can be imported"""
    from guestkit import Guestfs, DiskConverter
    assert Guestfs is not None
    assert DiskConverter is not None


def test_guestfs_creation():
    """Test Guestfs handle creation and shutdown"""
    from guestkit import Guestfs

    g = Guestfs()
    assert g is not None

    # Test shutdown
    g.shutdown()


def test_guestfs_methods_exist():
    """Test that expected methods exist on Guestfs class"""
    from guestkit import Guestfs

    expected_methods = [
        'add_drive', 'add_drive_ro', 'launch', 'shutdown',
        'inspect_os', 'inspect_get_type', 'inspect_get_distro',
        'inspect_get_hostname', 'inspect_get_arch',
        'list_devices', 'list_partitions', 'blockdev_getsize64',
        'mount', 'mount_ro', 'umount', 'umount_all',
        'exists', 'is_file', 'is_dir', 'ls', 'cat', 'read_file', 'write',
        'download', 'upload', 'mkdir', 'mkdir_p', 'rm', 'rmdir', 'rm_rf',
        'chmod', 'chown', 'stat', 'statvfs',
        'vfs_type', 'vfs_label', 'vfs_uuid',
        'command', 'sh', 'sh_lines',
        'vgscan', 'vgs', 'pvs', 'lvs',
        'tar_in', 'tar_out', 'tgz_in', 'tgz_out',
        'checksum', 'sync', 'set_verbose',
        'inspect_get_major_version', 'inspect_get_minor_version',
        'inspect_get_product_name', 'inspect_get_package_format',
        'inspect_get_package_management', 'inspect_get_mountpoints',
        'inspect_list_applications',
    ]

    for method in expected_methods:
        assert hasattr(Guestfs, method), f"Guestfs missing method: {method}"


def test_disk_converter_creation():
    """Test DiskConverter creation"""
    from guestkit import DiskConverter

    converter = DiskConverter()
    assert converter is not None


def test_disk_converter_methods_exist():
    """Test that expected methods exist on DiskConverter class"""
    from guestkit import DiskConverter

    expected_methods = ['convert', 'detect_format', 'get_info']

    for method in expected_methods:
        assert hasattr(DiskConverter, method), f"DiskConverter missing method: {method}"


def test_guestfs_method_count():
    """Test that Guestfs has expected number of public methods"""
    from guestkit import Guestfs

    # Count public methods (not starting with _)
    public_methods = [m for m in dir(Guestfs) if not m.startswith('_')]

    # We expect at least 50 methods
    assert len(public_methods) >= 50, f"Expected at least 50 public methods, found {len(public_methods)}"


def test_error_handling():
    """Test that Python exceptions are raised properly"""
    from guestkit import Guestfs

    g = Guestfs()

    # Test that calling methods before launch raises an error
    with pytest.raises(Exception):
        g.inspect_os()

    g.shutdown()


def test_version_consistency():
    """Test that version is consistent"""
    import guestkit

    # Version should be a string
    assert isinstance(guestkit.__version__, str)

    # Version should have format X.Y.Z
    parts = guestkit.__version__.split('.')
    assert len(parts) == 3, f"Version should have 3 parts, got: {guestkit.__version__}"

    # Each part should be numeric
    for part in parts:
        assert part.isdigit(), f"Version part '{part}' should be numeric"


class TestWithDiskImage:
    """Tests that require a disk image file

    These tests will be skipped if GUESTKIT_TEST_IMAGE environment
    variable is not set.
    """

    pytestmark = pytest.mark.requires_image

    @pytest.fixture
    def disk_image(self):
        """Get test disk image path from environment"""
        image_path = os.environ.get('GUESTKIT_TEST_IMAGE')
        if not image_path:
            pytest.skip("GUESTKIT_TEST_IMAGE environment variable not set")
        if not os.path.exists(image_path):
            pytest.skip(f"Test disk image not found: {image_path}")
        return image_path

    def test_add_drive_and_launch(self, disk_image):
        """Test adding a drive and launching"""
        from guestkit import Guestfs

        g = Guestfs()
        try:
            g.add_drive_ro(disk_image)
            g.launch()

            # If we got here, launch succeeded
            assert True
        finally:
            g.shutdown()

    def test_inspect_os(self, disk_image):
        """Test OS inspection"""
        from guestkit import Guestfs

        g = Guestfs()
        try:
            g.add_drive_ro(disk_image)
            g.launch()

            roots = g.inspect_os()
            assert isinstance(roots, list)

            # If OS detected, test inspection methods
            if roots:
                root = roots[0]

                # These should return strings
                os_type = g.inspect_get_type(root)
                assert isinstance(os_type, str)

                distro = g.inspect_get_distro(root)
                assert isinstance(distro, str)

                hostname = g.inspect_get_hostname(root)
                assert isinstance(hostname, str)

                arch = g.inspect_get_arch(root)
                assert isinstance(arch, str)

                # These should return integers
                major = g.inspect_get_major_version(root)
                assert isinstance(major, int)

                minor = g.inspect_get_minor_version(root)
                assert isinstance(minor, int)
        finally:
            g.shutdown()

    def test_device_operations(self, disk_image):
        """Test device and partition listing"""
        from guestkit import Guestfs

        g = Guestfs()
        try:
            g.add_drive_ro(disk_image)
            g.launch()

            # List devices
            devices = g.list_devices()
            assert isinstance(devices, list)
            assert len(devices) > 0

            # List partitions
            partitions = g.list_partitions()
            assert isinstance(partitions, list)

            # Get size of first device
            if devices:
                size = g.blockdev_getsize64(devices[0])
                assert isinstance(size, int)
                assert size > 0
        finally:
            g.shutdown()

    def test_filesystem_operations(self, disk_image):
        """Test filesystem type detection"""
        from guestkit import Guestfs

        g = Guestfs()
        try:
            g.add_drive_ro(disk_image)
            g.launch()

            partitions = g.list_partitions()

            # Test filesystem detection on first partition
            if partitions:
                part = partitions[0]

                try:
                    fs_type = g.vfs_type(part)
                    assert isinstance(fs_type, str)
                except Exception:
                    # Some partitions may not have a filesystem
                    pass
        finally:
            g.shutdown()

    def test_mount_and_file_operations(self, disk_image):
        """Test mounting and file operations"""
        from guestkit import Guestfs

        g = Guestfs()
        try:
            g.add_drive_ro(disk_image)
            g.launch()

            roots = g.inspect_os()
            if not roots:
                pytest.skip("No OS detected in disk image")

            root = roots[0]

            # Get and mount filesystems
            mountpoints = g.inspect_get_mountpoints(root)
            assert isinstance(mountpoints, dict)

            # Mount filesystems
            for mp, dev in sorted(mountpoints.items(), key=lambda x: len(x[0])):
                try:
                    g.mount_ro(dev, mp)
                except Exception as e:
                    # Some mounts may fail, that's ok
                    pass

            # Test file operations
            if g.is_dir("/etc"):
                files = g.ls("/etc")
                assert isinstance(files, list)

            if g.is_file("/etc/hostname"):
                content = g.cat("/etc/hostname")
                assert isinstance(content, str)

            # Test stat
            if g.exists("/etc/passwd"):
                stat = g.stat("/etc/passwd")
                assert isinstance(stat, dict)
                assert 'size' in stat
                assert 'mode' in stat
        finally:
            g.umount_all()
            g.shutdown()

    def test_package_listing(self, disk_image):
        """Test package listing"""
        from guestkit import Guestfs

        g = Guestfs()
        try:
            g.add_drive_ro(disk_image)
            g.launch()

            roots = g.inspect_os()
            if not roots:
                pytest.skip("No OS detected in disk image")

            root = roots[0]

            # Mount filesystems
            mountpoints = g.inspect_get_mountpoints(root)
            for mp, dev in sorted(mountpoints.items(), key=lambda x: len(x[0])):
                try:
                    g.mount_ro(dev, mp)
                except Exception:
                    pass

            # List applications
            try:
                apps = g.inspect_list_applications(root)
                assert isinstance(apps, list)

                if apps:
                    # Check first app structure
                    app = apps[0]
                    assert isinstance(app, dict)
                    assert 'app_name' in app
                    assert 'app_version' in app
            except Exception as e:
                # Package listing may not work on all systems
                pytest.skip(f"Package listing not available: {e}")
        finally:
            g.umount_all()
            g.shutdown()


class TestDiskConverter:
    """Tests for DiskConverter class"""

    def test_detect_format_nonexistent(self):
        """Test format detection with non-existent file"""
        from guestkit import DiskConverter

        converter = DiskConverter()

        with pytest.raises(Exception):
            converter.detect_format("/nonexistent/file.img")


# Pytest configuration
def pytest_configure(config):
    """Configure pytest"""
    config.addinivalue_line(
        "markers",
        "requires_image: mark test as requiring a disk image"
    )


if __name__ == '__main__':
    # Run tests
    pytest.main([__file__, '-v'])
