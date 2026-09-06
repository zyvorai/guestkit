#!/usr/bin/env python3
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
"""
Integration tests for guestkit wrapper

Tests the Python wrapper against the guestkit Rust binary
"""

import sys
import os
import unittest
import tempfile
from pathlib import Path

# Add parent directory to path
sys.path.insert(0, str(Path(__file__).parent.parent / "python"))

from guestkit_wrapper import GuestkitWrapper, ConversionResult


class TestGuestkitIntegration(unittest.TestCase):
    """Integration tests for guestkit wrapper"""

    @classmethod
    def setUpClass(cls):
        """Set up test fixtures"""
        # Find guestkit binary
        guestkit_paths = [
            "../../target/debug/guestkit",
            "../../target/release/guestkit",
            "guestkit",  # In PATH
        ]

        cls.guestkit_path = None
        for path in guestkit_paths:
            if Path(path).exists() or path == "guestkit":
                cls.guestkit_path = path
                break

        if not cls.guestkit_path:
            raise RuntimeError(
                "guestkit binary not found. "
                "Run 'cargo build' first."
            )

        cls.wrapper = GuestkitWrapper(guestkit_path=cls.guestkit_path)

    def test_wrapper_initialization(self):
        """Test wrapper can be initialized"""
        self.assertIsNotNone(self.wrapper)
        self.assertEqual(self.wrapper.guestkit_path, self.guestkit_path)

    def test_version_command(self):
        """Test guestkit version command works"""
        import subprocess

        result = subprocess.run(
            [self.guestkit_path, "version"],
            capture_output=True,
            text=True
        )

        self.assertEqual(result.returncode, 0)
        self.assertIn("guestkit", result.stdout)

    def test_help_command(self):
        """Test guestkit help command works"""
        import subprocess

        result = subprocess.run(
            [self.guestkit_path, "--help"],
            capture_output=True,
            text=True
        )

        self.assertEqual(result.returncode, 0)
        self.assertIn("convert", result.stdout)
        self.assertIn("detect", result.stdout)
        self.assertIn("info", result.stdout)

    # Note: The following tests require actual disk images
    # Uncomment when you have test images available

    # def test_detect_format(self):
    #     """Test format detection"""
    #     test_image = "/path/to/test.qcow2"
    #
    #     if not Path(test_image).exists():
    #         self.skipTest(f"Test image not found: {test_image}")
    #
    #     format_type = self.wrapper.detect_format(test_image)
    #     self.assertEqual(format_type, "qcow2")

    # def test_get_info(self):
    #     """Test getting disk information"""
    #     test_image = "/path/to/test.qcow2"
    #
    #     if not Path(test_image).exists():
    #         self.skipTest(f"Test image not found: {test_image}")
    #
    #     info = self.wrapper.get_info(test_image)
    #     self.assertIn("format", info)
    #     self.assertEqual(info["format"], "qcow2")

    # def test_conversion(self):
    #     """Test disk conversion"""
    #     source = "/path/to/source.vmdk"
    #
    #     if not Path(source).exists():
    #         self.skipTest(f"Source image not found: {source}")
    #
    #     with tempfile.NamedTemporaryFile(suffix=".qcow2") as tmp:
    #         result = self.wrapper.convert(
    #             source_path=source,
    #             output_path=tmp.name,
    #             compress=True
    #         )
    #
    #         self.assertTrue(result.success)
    #         self.assertGreater(result.output_size, 0)
    #         self.assertEqual(result.output_format, "qcow2")


class TestConversionResult(unittest.TestCase):
    """Test ConversionResult dataclass"""

    def test_conversion_result_creation(self):
        """Test creating ConversionResult"""
        result = ConversionResult(
            source_path="/source.vmdk",
            output_path="/output.qcow2",
            source_format="vmdk",
            output_format="qcow2",
            output_size=1024000,
            duration_secs=1.5,
            success=True
        )

        self.assertEqual(result.source_format, "vmdk")
        self.assertEqual(result.output_format, "qcow2")
        self.assertTrue(result.success)
        self.assertIsNone(result.error)

    def test_conversion_result_with_error(self):
        """Test ConversionResult with error"""
        result = ConversionResult(
            source_path="/source.vmdk",
            output_path="/output.qcow2",
            source_format="vmdk",
            output_format="qcow2",
            output_size=0,
            duration_secs=0.0,
            success=False,
            error="Conversion failed"
        )

        self.assertFalse(result.success)
        self.assertEqual(result.error, "Conversion failed")


def main():
    """Run integration tests"""
    # Set up test environment
    os.environ["RUST_LOG"] = "info"

    # Run tests
    unittest.main(verbosity=2)


if __name__ == "__main__":
    main()
