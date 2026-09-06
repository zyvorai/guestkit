#!/usr/bin/env python3
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
"""
Python wrapper for guestkit - For use with hyper2kvm

This module provides a Python interface to the guestkit Rust binary,
allowing hyper2kvm to use high-performance disk operations.
"""

import json
import subprocess
import logging
from pathlib import Path
from typing import Optional, Dict, Any
from dataclasses import dataclass


@dataclass
class ConversionResult:
    """Result from disk conversion"""
    source_path: str
    output_path: str
    source_format: str
    output_format: str
    output_size: int
    duration_secs: float
    success: bool
    error: Optional[str] = None


class GuestkitWrapper:
    """Python wrapper for guestkit binary"""

    def __init__(self, guestkit_path: str = "guestkit", logger: Optional[logging.Logger] = None):
        self.guestkit_path = guestkit_path
        self.logger = logger or logging.getLogger(__name__)

    def convert(
        self,
        source_path: str,
        output_path: str,
        output_format: str = "qcow2",
        compress: bool = False,
        flatten: bool = True,
        verbose: bool = False
    ) -> ConversionResult:
        """
        Convert disk image format using guestkit.

        Args:
            source_path: Input disk image path
            output_path: Output disk image path
            output_format: Target format (qcow2, raw, vmdk, vdi)
            compress: Enable compression (qcow2 only)
            flatten: Flatten snapshot chains
            verbose: Enable verbose logging

        Returns:
            ConversionResult with conversion details

        Example:
            >>> wrapper = GuestkitWrapper()
            >>> result = wrapper.convert(
            ...     "/path/to/vm.vmdk",
            ...     "/path/to/vm.qcow2",
            ...     compress=True
            ... )
            >>> if result.success:
            ...     print(f"Converted to {result.output_format}")
        """
        self.logger.info(f"Converting {source_path} -> {output_path}")

        # Build command
        cmd = [
            self.guestkit_path,
            "convert",
            "--source", source_path,
            "--output", output_path,
            "--format", output_format,
        ]

        if compress:
            cmd.append("--compress")
        if flatten:
            cmd.append("--flatten")
        if verbose:
            cmd.insert(1, "-v")

        # Execute conversion
        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                check=True
            )

            # Parse output
            output_size = Path(output_path).stat().st_size if Path(output_path).exists() else 0

            # Extract format info from stdout
            source_format = self._extract_format_from_output(result.stdout, "Source:")
            detected_output_format = self._extract_format_from_output(result.stdout, "Output:")

            # Extract duration
            duration = self._extract_duration(result.stdout)

            self.logger.info(f"Conversion successful: {output_size:,} bytes")

            return ConversionResult(
                source_path=source_path,
                output_path=output_path,
                source_format=source_format or "unknown",
                output_format=detected_output_format or output_format,
                output_size=output_size,
                duration_secs=duration,
                success=True
            )

        except subprocess.CalledProcessError as e:
            self.logger.error(f"Conversion failed: {e.stderr}")
            return ConversionResult(
                source_path=source_path,
                output_path=output_path,
                source_format="unknown",
                output_format=output_format,
                output_size=0,
                duration_secs=0.0,
                success=False,
                error=e.stderr
            )

    def detect_format(self, image_path: str) -> str:
        """
        Detect disk image format.

        Args:
            image_path: Path to disk image

        Returns:
            Detected format (qcow2, raw, vmdk, etc.)

        Example:
            >>> wrapper = GuestkitWrapper()
            >>> format = wrapper.detect_format("/path/to/disk.img")
            >>> print(f"Format: {format}")
        """
        cmd = [self.guestkit_path, "detect", "--image", image_path]

        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                check=True
            )

            # Parse "Detected format: qcow2" output
            for line in result.stdout.split('\n'):
                if "Detected format:" in line:
                    return line.split(":")[-1].strip()

            return "unknown"

        except subprocess.CalledProcessError as e:
            self.logger.error(f"Format detection failed: {e.stderr}")
            return "unknown"

    def get_info(self, image_path: str) -> Dict[str, Any]:
        """
        Get detailed disk image information.

        Args:
            image_path: Path to disk image

        Returns:
            Dictionary with disk image metadata

        Example:
            >>> wrapper = GuestkitWrapper()
            >>> info = wrapper.get_info("/path/to/disk.img")
            >>> print(f"Format: {info.get('format')}")
            >>> print(f"Size: {info.get('virtual-size')}")
        """
        cmd = [self.guestkit_path, "info", "--image", image_path]

        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                check=True
            )

            # Parse JSON output
            return json.loads(result.stdout)

        except subprocess.CalledProcessError as e:
            self.logger.error(f"Info retrieval failed: {e.stderr}")
            return {}
        except json.JSONDecodeError as e:
            self.logger.error(f"JSON parse error: {e}")
            return {}

    def doctor(
        self,
        image_path: str,
        target: str = "kvm",
        output_format: str = "json",
    ) -> Dict[str, Any]:
        """Run guestkit doctor on a disk image."""
        cmd = [
            self.guestkit_path,
            "doctor",
            image_path,
            "--target",
            target,
            "-f",
            output_format,
        ]
        try:
            result = subprocess.run(cmd, capture_output=True, text=True, check=True)
            return json.loads(result.stdout) if output_format == "json" else {"raw": result.stdout}
        except subprocess.CalledProcessError as e:
            self.logger.error(f"doctor failed: {e.stderr}")
            return {"success": False, "error": e.stderr}
        except json.JSONDecodeError as e:
            return {"success": False, "error": str(e)}

    def migrate_plan(
        self,
        image_path: str,
        target: str = "kvm",
        export_path: Optional[str] = None,
        output_format: str = "json",
    ) -> Dict[str, Any]:
        """Run guestkit migrate-plan."""
        cmd = [
            self.guestkit_path,
            "migrate-plan",
            image_path,
            "--target",
            target,
            "-f",
            output_format,
        ]
        if export_path:
            cmd.extend(["--export", export_path])
        try:
            result = subprocess.run(cmd, capture_output=True, text=True, check=True)
            if output_format == "json" and result.stdout.strip():
                return json.loads(result.stdout)
            return {"raw": result.stdout, "export": export_path}
        except subprocess.CalledProcessError as e:
            self.logger.error(f"migrate-plan failed: {e.stderr}")
            return {"success": False, "error": e.stderr}

    def repair_inject_agent(
        self,
        image_path: str,
        agent_binary: Optional[str] = None,
        agent_unit: Optional[str] = None,
        dry_run: bool = False,
    ) -> Dict[str, Any]:
        """Run guestkit repair --fix boot --inject-agent."""
        cmd = [
            self.guestkit_path,
            "repair",
            image_path,
            "--fix",
            "boot",
            "--inject-agent",
        ]
        if agent_binary:
            cmd.extend(["--agent-binary", agent_binary])
        if agent_unit:
            cmd.extend(["--agent-unit", agent_unit])
        if dry_run:
            cmd.append("--dry-run")
        try:
            result = subprocess.run(cmd, capture_output=True, text=True, check=True)
            return {"success": True, "stdout": result.stdout}
        except subprocess.CalledProcessError as e:
            self.logger.error(f"repair --inject-agent failed: {e.stderr}")
            return {"success": False, "error": e.stderr}

    def _extract_format_from_output(self, output: str, prefix: str) -> Optional[str]:
        """Extract format from conversion output"""
        for line in output.split('\n'):
            if prefix in line and '(' in line and ')' in line:
                # Extract format from "Source: /path/to/file (vmdk)"
                start = line.find('(')
                end = line.find(')')
                if start != -1 and end != -1:
                    return line[start+1:end]
        return None

    def _extract_duration(self, output: str) -> float:
        """Extract duration from conversion output"""
        for line in output.split('\n'):
            if "Time:" in line:
                try:
                    # Extract "1.23s" from "Time: 1.23s"
                    time_str = line.split("Time:")[-1].strip().rstrip('s')
                    return float(time_str)
                except (ValueError, IndexError):
                    pass
        return 0.0


# Example usage for hyper2kvm integration
if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO)

    # Initialize wrapper
    wrapper = GuestkitWrapper(guestkit_path="../../target/debug/guestkit")

    # Example: Detect format
    print("Example 1: Format Detection")
    print("=" * 50)
    # format_type = wrapper.detect_format("/path/to/disk.img")
    # print(f"Detected format: {format_type}\n")

    # Example: Get info
    print("Example 2: Disk Information")
    print("=" * 50)
    # info = wrapper.get_info("/path/to/disk.img")
    # print(json.dumps(info, indent=2))
    # print()

    # Example: Convert
    print("Example 3: Disk Conversion")
    print("=" * 50)
    # result = wrapper.convert(
    #     "/path/to/source.vmdk",
    #     "/path/to/output.qcow2",
    #     compress=True
    # )
    # if result.success:
    #     print(f"✓ Conversion successful!")
    #     print(f"  Format: {result.source_format} -> {result.output_format}")
    #     print(f"  Size:   {result.output_size:,} bytes")
    #     print(f"  Time:   {result.duration_secs:.2f}s")
    # else:
    #     print(f"✗ Conversion failed: {result.error}")

    print("\nTo use this in hyper2kvm:")
    print("1. Build guestkit: cd ~/tt/guestkit && cargo build --release")
    print("2. Import wrapper: from guestkit_wrapper import GuestkitWrapper")
    print("3. Use in pipeline: wrapper.convert(source, output, compress=True)")
