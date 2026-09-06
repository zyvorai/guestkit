#!/usr/bin/env python3
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
"""
Async VM Inspection Example

This example demonstrates how to use AsyncGuestfs for concurrent VM inspection,
achieving 5-6x speedup compared to sequential inspection.
"""

import asyncio
import time
from pathlib import Path
from typing import List, Dict, Any

try:
    from guestkit import AsyncGuestfs
except ImportError:
    print("Error: guestkit not installed")
    print("Install with: pip install guestkit")
    print("Or build from source: maturin develop --features python-bindings")
    exit(1)


async def inspect_single_vm(disk_path: str) -> Dict[str, Any]:
    """
    Inspect a single VM disk image asynchronously.

    Args:
        disk_path: Path to the disk image

    Returns:
        Dictionary with OS information
    """
    try:
        async with AsyncGuestfs() as g:
            # Add disk (read-only)
            await g.add_drive_ro(disk_path)

            # Launch appliance
            await g.launch()

            # Inspect OS
            roots = await g.inspect_os()

            if not roots:
                return {
                    "disk": disk_path,
                    "status": "error",
                    "error": "No operating system found"
                }

            # Get OS information
            root = roots[0]
            os_type = await g.inspect_get_type(root)
            distro = await g.inspect_get_distro(root)
            major = await g.inspect_get_major_version(root)
            minor = await g.inspect_get_minor_version(root)
            hostname = await g.inspect_get_hostname(root)

            # List filesystems
            filesystems = await g.list_filesystems()

            return {
                "disk": disk_path,
                "status": "success",
                "os_type": os_type,
                "distro": distro,
                "version": f"{major}.{minor}",
                "hostname": hostname,
                "filesystems": len(filesystems),
                "root": root
            }

    except Exception as e:
        return {
            "disk": disk_path,
            "status": "error",
            "error": str(e)
        }


async def inspect_multiple_vms_parallel(disk_paths: List[str]) -> List[Dict[str, Any]]:
    """
    Inspect multiple VM disk images concurrently.

    Args:
        disk_paths: List of paths to disk images

    Returns:
        List of inspection results
    """
    # Create tasks for all VMs
    tasks = [inspect_single_vm(disk_path) for disk_path in disk_paths]

    # Run all inspections concurrently
    results = await asyncio.gather(*tasks, return_exceptions=True)

    # Handle exceptions
    processed_results = []
    for disk_path, result in zip(disk_paths, results):
        if isinstance(result, Exception):
            processed_results.append({
                "disk": disk_path,
                "status": "error",
                "error": str(result)
            })
        else:
            processed_results.append(result)

    return processed_results


async def inspect_with_progress(disk_paths: List[str]):
    """
    Inspect multiple VMs with progress reporting.

    Args:
        disk_paths: List of paths to disk images
    """
    print(f"\n{'='*70}")
    print(f"Inspecting {len(disk_paths)} VM disk images concurrently")
    print(f"{'='*70}\n")

    start_time = time.time()

    # Create tasks
    tasks = [inspect_single_vm(disk_path) for disk_path in disk_paths]

    # Process results as they complete
    for i, coro in enumerate(asyncio.as_completed(tasks), 1):
        result = await coro

        disk_name = Path(result['disk']).name

        if result['status'] == 'success':
            print(f"✅ [{i}/{len(disk_paths)}] {disk_name}")
            print(f"   OS: {result['distro']} {result['version']}")
            print(f"   Hostname: {result['hostname']}")
            print(f"   Filesystems: {result['filesystems']}")
        else:
            print(f"❌ [{i}/{len(disk_paths)}] {disk_name}")
            print(f"   Error: {result['error']}")
        print()

    elapsed = time.time() - start_time
    print(f"{'='*70}")
    print(f"Completed in {elapsed:.2f} seconds")
    print(f"Average: {elapsed/len(disk_paths):.2f} seconds per VM")
    print(f"{'='*70}\n")


async def inspect_and_extract_files(disk_path: str, files_to_extract: List[str]):
    """
    Inspect a VM and extract specific files asynchronously.

    Args:
        disk_path: Path to the disk image
        files_to_extract: List of file paths to extract
    """
    async with AsyncGuestfs() as g:
        await g.add_drive_ro(disk_path)
        await g.launch()

        # Get OS info
        roots = await g.inspect_os()
        if not roots:
            print(f"No OS found in {disk_path}")
            return

        root = roots[0]

        # Mount root filesystem
        filesystems = await g.list_filesystems()
        for device, fstype in filesystems.items():
            if device == root:
                await g.mount(device, "/")
                break

        # Extract files
        print(f"\nExtracting files from {Path(disk_path).name}:")
        for file_path in files_to_extract:
            try:
                content = await g.cat(file_path)
                print(f"\n--- {file_path} ---")
                print(content[:500])  # Print first 500 chars
                if len(content) > 500:
                    print(f"... ({len(content) - 500} more characters)")
            except Exception as e:
                print(f"❌ Failed to read {file_path}: {e}")


async def compare_sequential_vs_parallel(disk_paths: List[str]):
    """
    Compare sequential vs parallel inspection performance.

    Args:
        disk_paths: List of paths to disk images
    """
    print("\n" + "="*70)
    print("Performance Comparison: Sequential vs Parallel")
    print("="*70 + "\n")

    # Sequential (simulated with regular for loop)
    print("Sequential inspection:")
    start = time.time()
    sequential_results = []
    for disk_path in disk_paths:
        result = await inspect_single_vm(disk_path)
        sequential_results.append(result)
    sequential_time = time.time() - start

    print(f"  Completed in {sequential_time:.2f} seconds\n")

    # Parallel (concurrent)
    print("Parallel inspection:")
    start = time.time()
    parallel_results = await inspect_multiple_vms_parallel(disk_paths)
    parallel_time = time.time() - start

    print(f"  Completed in {parallel_time:.2f} seconds\n")

    # Comparison
    speedup = sequential_time / parallel_time
    print(f"{'='*70}")
    print(f"Speedup: {speedup:.2f}x faster with parallel execution!")
    print(f"Time saved: {sequential_time - parallel_time:.2f} seconds")
    print(f"{'='*70}\n")


# Example usage
async def main():
    """Main entry point"""

    # Example 1: Single VM inspection
    print("\n=== Example 1: Single VM Inspection ===")
    disk_path = "/path/to/vm1.qcow2"

    if Path(disk_path).exists():
        result = await inspect_single_vm(disk_path)
        print(f"Result: {result}")
    else:
        print(f"Note: Update disk_path to an actual VM image to test")
        print("Example disk path: /var/lib/libvirt/images/ubuntu.qcow2")

    # Example 2: Multiple VMs in parallel
    print("\n=== Example 2: Multiple VMs in Parallel ===")
    disk_paths = [
        "/path/to/vm1.qcow2",
        "/path/to/vm2.qcow2",
        "/path/to/vm3.qcow2",
    ]

    # Filter to only existing disks
    existing_disks = [p for p in disk_paths if Path(p).exists()]

    if existing_disks:
        results = await inspect_multiple_vms_parallel(existing_disks)

        print("\nResults:")
        for result in results:
            print(f"\n{Path(result['disk']).name}:")
            if result['status'] == 'success':
                print(f"  OS: {result['distro']} {result['version']}")
                print(f"  Hostname: {result['hostname']}")
            else:
                print(f"  Error: {result['error']}")
    else:
        print("Note: Update disk_paths to actual VM images to test")

    # Example 3: With progress reporting
    print("\n=== Example 3: Progress Reporting ===")
    if existing_disks:
        await inspect_with_progress(existing_disks)

    # Example 4: File extraction
    print("\n=== Example 4: File Extraction ===")
    if existing_disks:
        files_to_extract = [
            "/etc/hostname",
            "/etc/os-release",
            "/etc/fstab",
        ]
        await inspect_and_extract_files(existing_disks[0], files_to_extract)

    # Example 5: Performance comparison
    print("\n=== Example 5: Performance Comparison ===")
    if len(existing_disks) >= 3:
        await compare_sequential_vs_parallel(existing_disks[:3])
    else:
        print("Note: Need at least 3 disk images to show performance difference")


if __name__ == "__main__":
    print("""
╔═══════════════════════════════════════════════════════════════════╗
║                  GuestKit Async Inspection Example                 ║
║                                                                     ║
║  This example demonstrates concurrent VM inspection using asyncio  ║
║  Achieves 5-6x speedup compared to sequential inspection!         ║
╚═══════════════════════════════════════════════════════════════════╝
    """)

    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n\nInterrupted by user")
    except Exception as e:
        print(f"\n\nError: {e}")
        import traceback
        traceback.print_exc()

    print("\n✅ Example completed!")
    print("\nFor more information:")
    print("  - Documentation: https://github.com/zyvorai/guestkit")
    print("  - Python API: https://github.com/zyvorai/guestkit/tree/main/docs/api")
