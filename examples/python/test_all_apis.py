#!/usr/bin/env python3
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
"""
Comprehensive GuestKit API Testing
Tests all major APIs against various disk image formats
"""

import subprocess
import json
import sys
from pathlib import Path

# Test images
TEST_IMAGES = [
    {
        "name": "Photon OS 5.0 (QCOW2)",
        "path": "/home/ssahani/by-path/out/work/working-flattened-20260123-193807.qcow2",
        "os_type": "linux",
    },
    {
        "name": "Windows 11 (VMDK)",
        "path": "/home/ssahani/tt/hyper2kvm/win11/win11.vmdk",
        "os_type": "windows",
    },
    {
        "name": "Ubuntu 24.04 (QCOW2)",
        "path": "/var/lib/libvirt/images/photon.qcow2",
        "os_type": "linux",
    },
    {
        "name": "RHEL 10 Beta (QCOW2)",
        "path": "/var/lib/libvirt/images/rhel10.qcow2",
        "os_type": "linux",
    },
    {
        "name": "Arch Linux (VMDK)",
        "path": "/home/ssahani/Downloads/VMs/Arch Linux 20240601.vmdk",
        "os_type": "linux",
    },
    {
        "name": "Ubuntu Server 25.04 (VDI)",
        "path": "/home/ssahani/Downloads/VMs/Ubuntu Server 25.04.vdi",
        "os_type": "linux",
    },
    {
        "name": "Photon Azure (VHD)",
        "path": "/home/ssahani/Downloads/VMs/photon-azure-5.0.vhd",
        "os_type": "linux",
    },
]

# API tests to run
TESTS = [
    {
        "name": "OS Inspection",
        "cmd": ["cargo", "run", "--bin", "guestkit", "--", "inspect"],
    },
    {
        "name": "List Filesystems",
        "cmd": ["cargo", "run", "--bin", "guestkit", "--", "filesystems"],
    },
    {
        "name": "List Packages",
        "cmd": ["cargo", "run", "--bin", "guestkit", "--", "packages"],
    },
    {
        "name": "List Root Directory",
        "cmd": ["sudo", "/home/ssahani/tt/guestkit/target/release/guestkit", "ls"],
    },
    {
        "name": "Read /etc/os-release",
        "cmd": ["sudo", "/home/ssahani/tt/guestkit/target/release/guestkit", "cat"],
    },
]

def run_test(image_path, test_cmd, test_name, timeout=60):
    """Run a single test command"""
    cmd = test_cmd + [image_path]

    # Add path argument for ls and cat commands
    if "List Root Directory" in test_name:
        cmd.append("/")
    elif "Read /etc/os-release" in test_name:
        cmd.append("/etc/os-release")
    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout,
            cwd="/home/ssahani/tt/guestkit"
        )
        return {
            "success": result.returncode == 0,
            "stdout": result.stdout,
            "stderr": result.stderr,
            "returncode": result.returncode,
        }
    except subprocess.TimeoutExpired:
        return {
            "success": False,
            "stdout": "",
            "stderr": "Timeout expired",
            "returncode": -1,
        }
    except Exception as e:
        return {
            "success": False,
            "stdout": "",
            "stderr": str(e),
            "returncode": -2,
        }

def print_separator(char="=", length=80):
    print(char * length)

def main():
    print_separator("=")
    print("GUESTKIT COMPREHENSIVE API TEST SUITE")
    print_separator("=")
    print()

    total_tests = 0
    passed_tests = 0
    failed_tests = 0

    results_summary = []

    for image in TEST_IMAGES:
        image_path = Path(image["path"])

        # Check if image exists
        if not image_path.exists():
            print(f"⚠️  Skipping {image['name']} - File not found")
            print()
            continue

        print_separator("-")
        print(f"📀 Testing: {image['name']}")
        print(f"   Path: {image['path']}")
        print(f"   OS Type: {image['os_type']}")
        print_separator("-")
        print()

        image_results = {
            "image": image["name"],
            "path": image["path"],
            "tests": []
        }

        for test in TESTS:
            total_tests += 1
            test_name = test["name"]

            print(f"  🧪 {test_name}...", end=" ", flush=True)

            result = run_test(image["path"], test["cmd"], test_name)

            if result["success"]:
                print("✅ PASSED")
                passed_tests += 1
                status = "PASSED"
            else:
                print("❌ FAILED")
                failed_tests += 1
                status = "FAILED"

            # Show output preview (first 3 lines)
            if result["stdout"]:
                lines = result["stdout"].strip().split("\n")[:3]
                for line in lines:
                    print(f"       {line}")
                if len(result["stdout"].strip().split("\n")) > 3:
                    print(f"       ... ({len(result['stdout'].strip().split('\n')) - 3} more lines)")

            if not result["success"] and result["stderr"]:
                stderr_lines = result["stderr"].strip().split("\n")[:2]
                for line in stderr_lines:
                    print(f"       ERROR: {line}")

            print()

            image_results["tests"].append({
                "name": test_name,
                "status": status,
                "returncode": result["returncode"]
            })

        results_summary.append(image_results)
        print()

    # Final summary
    print_separator("=")
    print("TEST SUMMARY")
    print_separator("=")
    print()
    print(f"Total Tests:  {total_tests}")
    print(f"Passed:       {passed_tests} ✅")
    print(f"Failed:       {failed_tests} ❌")
    print(f"Success Rate: {(passed_tests/total_tests*100) if total_tests > 0 else 0:.1f}%")
    print()

    # Per-image summary
    print_separator("-")
    print("PER-IMAGE RESULTS")
    print_separator("-")
    for img_result in results_summary:
        passed = sum(1 for t in img_result["tests"] if t["status"] == "PASSED")
        total = len(img_result["tests"])
        print(f"{img_result['image']}: {passed}/{total} tests passed")

    print_separator("=")

    # Exit code
    sys.exit(0 if failed_tests == 0 else 1)

if __name__ == "__main__":
    main()
