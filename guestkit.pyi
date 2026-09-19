"""Type stubs for guestkit Python bindings

This file provides type hints for better IDE support and type checking.
"""

from typing import List, Dict, Any, Optional
from types import TracebackType

__version__: str

class Guestfs:
    """Main class for VM disk image inspection and manipulation"""

    def __init__(self) -> None:
        """Create a new Guestfs handle"""
        ...

    def __enter__(self) -> 'Guestfs':
        """Enter context manager"""
        ...

    def __exit__(
        self,
        exc_type: Optional[type[BaseException]],
        exc_value: Optional[BaseException],
        traceback: Optional[TracebackType]
    ) -> bool:
        """Exit context manager and cleanup"""
        ...

    # Drive operations
    def add_drive(self, filename: str) -> None:
        """Add a disk image (read-write)"""
        ...

    def add_drive_ro(self, filename: str) -> None:
        """Add a disk image (read-only)"""
        ...

    def launch(self) -> None:
        """Launch the appliance"""
        ...

    def shutdown(self) -> None:
        """Shutdown the appliance"""
        ...

    def set_verbose(self, verbose: bool) -> None:
        """Enable/disable verbose output"""
        ...

    # Inspection API
    def inspect_os(self) -> List[str]:
        """Inspect operating systems in the disk image"""
        ...

    def inspect_get_type(self, root: str) -> str:
        """Get OS type (e.g., 'linux', 'windows')"""
        ...

    def inspect_get_distro(self, root: str) -> str:
        """Get distribution name (e.g., 'ubuntu', 'fedora')"""
        ...

    def inspect_get_major_version(self, root: str) -> int:
        """Get major version number"""
        ...

    def inspect_get_minor_version(self, root: str) -> int:
        """Get minor version number"""
        ...

    def inspect_get_hostname(self, root: str) -> str:
        """Get hostname"""
        ...

    def inspect_get_arch(self, root: str) -> str:
        """Get architecture (e.g., 'x86_64', 'aarch64')"""
        ...

    def inspect_get_product_name(self, root: str) -> str:
        """Get product name"""
        ...

    def inspect_get_package_format(self, root: str) -> str:
        """Get package format (e.g., 'rpm', 'deb')"""
        ...

    def inspect_get_package_management(self, root: str) -> str:
        """Get package management tool (e.g., 'apt', 'dnf')"""
        ...

    def inspect_get_mountpoints(self, root: str) -> Dict[str, str]:
        """Get filesystem mountpoints"""
        ...

    def inspect_list_applications(self, root: str) -> List[Dict[str, Any]]:
        """List installed packages"""
        ...

    # Device operations
    def list_devices(self) -> List[str]:
        """List all block devices"""
        ...

    def list_partitions(self) -> List[str]:
        """List all partitions"""
        ...

    def blockdev_getsize64(self, device: str) -> int:
        """Get device size in bytes"""
        ...

    # Filesystem operations
    def vfs_type(self, device: str) -> str:
        """Get filesystem type"""
        ...

    def vfs_label(self, device: str) -> str:
        """Get filesystem label"""
        ...

    def vfs_uuid(self, device: str) -> str:
        """Get filesystem UUID"""
        ...

    def mount(self, device: str, mountpoint: str) -> None:
        """Mount a filesystem (read-write)"""
        ...

    def mount_ro(self, device: str, mountpoint: str) -> None:
        """Mount a filesystem (read-only)"""
        ...

    def umount(self, mountpoint: str) -> None:
        """Unmount a filesystem"""
        ...

    def umount_all(self) -> None:
        """Unmount all filesystems"""
        ...

    def sync(self) -> None:
        """Synchronize filesystem"""
        ...

    # File operations
    def read_file(self, path: str) -> bytes:
        """Read file contents as bytes"""
        ...

    def cat(self, path: str) -> str:
        """Read file contents as string"""
        ...

    def write(self, path: str, content: bytes) -> None:
        """Write bytes to file"""
        ...

    def exists(self, path: str) -> bool:
        """Check if path exists"""
        ...

    def is_file(self, path: str) -> bool:
        """Check if path is a regular file"""
        ...

    def is_dir(self, path: str) -> bool:
        """Check if path is a directory"""
        ...

    def ls(self, directory: str) -> List[str]:
        """List directory contents"""
        ...

    def download(self, remotefilename: str, filename: str) -> None:
        """Download file from guest to host"""
        ...

    def upload(self, filename: str, remotefilename: str) -> None:
        """Upload file from host to guest"""
        ...

    # Directory operations
    def mkdir(self, path: str) -> None:
        """Create directory"""
        ...

    def mkdir_p(self, path: str) -> None:
        """Create directory with parents"""
        ...

    def rm(self, path: str) -> None:
        """Remove file"""
        ...

    def rmdir(self, path: str) -> None:
        """Remove empty directory"""
        ...

    def rm_rf(self, path: str) -> None:
        """Remove directory recursively"""
        ...

    # Permissions
    def chmod(self, mode: int, path: str) -> None:
        """Change file permissions"""
        ...

    def chown(self, owner: int, group: int, path: str) -> None:
        """Change file owner and group"""
        ...

    # Stat
    def stat(self, path: str) -> Dict[str, int]:
        """Get file stat information"""
        ...

    def statvfs(self, path: str) -> Dict[str, int]:
        """Get filesystem statistics"""
        ...

    # Command execution
    def command(self, arguments: List[str]) -> str:
        """Execute a command in the guest"""
        ...

    def sh(self, command: str) -> str:
        """Execute shell command"""
        ...

    def sh_lines(self, command: str) -> List[str]:
        """Execute shell command and return lines"""
        ...

    # LVM operations
    def vgscan(self) -> None:
        """Scan for LVM volume groups"""
        ...

    def vgs(self) -> List[str]:
        """List volume groups"""
        ...

    def pvs(self) -> List[str]:
        """List physical volumes"""
        ...

    def lvs(self) -> List[str]:
        """List logical volumes"""
        ...

    # Archive operations
    def tar_in(self, tarfile: str, directory: str) -> None:
        """Extract tar archive into guest directory"""
        ...

    def tar_out(self, directory: str, tarfile: str) -> None:
        """Create tar archive from guest directory"""
        ...

    def tgz_in(self, tarfile: str, directory: str) -> None:
        """Extract compressed tar archive into guest directory"""
        ...

    def tgz_out(self, directory: str, tarfile: str) -> None:
        """Create compressed tar archive from guest directory"""
        ...

    # Checksum operations
    def checksum(self, csumtype: str, path: str) -> str:
        """Calculate file checksum"""
        ...

    # Additional filesystem/device/LUKS operations
    def glob_expand(self, pattern: str) -> List[str]: ...
    def lvm_scan(self, activate: bool) -> None: ...
    def command_quiet(self, arguments: List[str]) -> str: ...
    def command_with_mounts(self, arguments: List[str]) -> str: ...
    def mount_local(self, mountpoint: str) -> None: ...
    def mount_local_run(self) -> None: ...
    def umount_local(self) -> None: ...
    def mount_options(self, options: str, mountable: str, mountpoint: str) -> None: ...
    def part_to_dev(self, partition: str) -> str: ...
    def luks_open(self, device: str, key: str, mapname: str) -> None: ...
    def luks_close(self, device: str) -> None: ...
    def cryptsetup_open(self, device: str, key: str, mapname: str) -> None:
        """Alias of luks_open (libguestfs name)."""
        ...
    def add_drive_opts(self, filename: str, readonly: bool = False, format: Optional[str] = None) -> None: ...
    def list_filesystems(self) -> Dict[str, str]: ...
    def available_all_groups(self) -> List[str]: ...
    def device_index(self, device: str) -> int: ...
    def is_link(self, path: str) -> bool:
        """Check if path is a symbolic link (alias of is_symlink)."""
        ...
    def vgchange_activate_all(self, activate: bool) -> None:
        """Activate (or deactivate) all LVM volume groups."""
        ...
    def md_stat(self, devices: List[str]) -> Dict[str, int]:
        """Assemble/activate MD RAID arrays (all discoverable if devices is
        empty), returning aggregate stats for whatever is active afterward."""
        ...
    def blkid(self, device: str) -> Dict[str, str]:
        """Get all blkid tags for a device (TYPE, UUID, LABEL, PARTUUID, ...)."""
        ...
    def findfs_uuid(self, uuid: str) -> str:
        """Find the device with the given filesystem UUID."""
        ...
    def findfs_label(self, label: str) -> str:
        """Find the device with the given filesystem label."""
        ...
    def list_dm_devices(self) -> List[str]:
        """List candidate LUKS-encrypted devices (suitable for luks_open)."""
        ...
    def rename(self, src: str, dst: str) -> None:
        """Rename (move) a file or directory, overwriting the destination."""
        ...
    def rm_f(self, path: str) -> None:
        """Remove a file, but don't error if it doesn't already exist."""
        ...
    def statns(self, path: str) -> Dict[str, int]:
        """Get file stat information with nanosecond-resolution timestamps."""
        ...
    def get_backend_info(self) -> Dict[str, str]:
        """Diagnostic info about this backend instance (implementation,
        version, attach mode, readonly)."""
        ...
    def get_performance_metrics(self) -> Dict[str, float]:
        """Measured performance counters for the most recent launch() call."""
        ...

    # Hivex (Windows registry)
    def hivex_open(self, filename: str, write: bool = False) -> int: ...
    def hivex_close(self, handle: int) -> None: ...
    def hivex_root(self, handle: int) -> int: ...
    def hivex_node_name(self, handle: int, node: int) -> str: ...
    def hivex_node_children(self, handle: int, node: int) -> List[int]: ...
    def hivex_node_get_child(self, handle: int, node: int, name: str) -> int: ...
    def hivex_node_values(self, handle: int, node: int) -> List[int]: ...
    def hivex_node_get_value(self, handle: int, node: int, key: str) -> int: ...
    def hivex_value_key(self, handle: int, value: int) -> str: ...
    def hivex_value_type(self, handle: int, value: int) -> int: ...
    def hivex_value_string(self, handle: int, value: int) -> str: ...
    def hivex_value_dword(self, handle: int, value: int) -> int: ...
    def hivex_value_uint32(self, handle: int, value: int) -> int: ...
    def hivex_value_integer(self, handle: int, value: int) -> int: ...
    def hivex_value_value(self, handle: int, value: int) -> bytes: ...
    def hivex_commit(self, handle: int, filename: Optional[str] = None) -> None: ...
    def hivex_node_add_child(self, handle: int, parent: int, name: str) -> int: ...


class DiskConverter:
    """Class for converting disk image formats"""

    def __init__(self) -> None:
        """Create a new disk converter instance"""
        ...

    def convert(
        self,
        source: str,
        output: str,
        format: str = "qcow2",
        compress: bool = False,
        flatten: bool = True
    ) -> Dict[str, Any]:
        """Convert disk image format

        Returns:
            Dictionary with conversion results including:
            - source_path: Source file path
            - output_path: Output file path
            - source_format: Detected source format
            - output_format: Output format
            - output_size: Output file size in bytes
            - duration_secs: Conversion duration
            - success: True if successful
            - error: Error message (if failed)
        """
        ...

    def detect_format(self, image: str) -> str:
        """Detect disk image format

        Returns:
            Format string (e.g., 'qcow2', 'raw', 'vmdk')
        """
        ...

    def get_info(self, image: str) -> Dict[str, Any]:
        """Get disk image metadata

        Returns:
            Dictionary with image information
        """
        ...


# TODO: AsyncGuestfs - Waiting for pyo3-asyncio PyO3 0.22+ support
# Planned for future release once pyo3-asyncio is updated
'''
class AsyncGuestfs:
    """Async version of Guestfs for non-blocking operations

    Use this class with asyncio for concurrent VM inspection.

    Example:
        import asyncio
        from guestkit import AsyncGuestfs

        async def inspect_vm(disk_path: str):
            async with AsyncGuestfs() as g:
                await g.add_drive_ro(disk_path)
                await g.launch()
                roots = await g.inspect_os()
                return roots

        asyncio.run(inspect_vm("/path/to/disk.qcow2"))
    """

    def __init__(self) -> None:
        """Create a new AsyncGuestfs handle"""
        ...

    async def __aenter__(self) -> 'AsyncGuestfs':
        """Enter async context manager"""
        ...

    async def __aexit__(
        self,
        exc_type: Optional[type[BaseException]],
        exc_value: Optional[BaseException],
        traceback: Optional[TracebackType]
    ) -> bool:
        """Exit async context manager and cleanup"""
        ...

    # Drive operations (async)
    async def add_drive(self, filename: str) -> None:
        """Add a disk image (read-write) - async version"""
        ...

    async def add_drive_ro(self, filename: str) -> None:
        """Add a disk image (read-only) - async version"""
        ...

    async def launch(self) -> None:
        """Launch the appliance - async version"""
        ...

    async def shutdown(self) -> None:
        """Shutdown the appliance - async version"""
        ...

    # Inspection operations (async)
    async def inspect_os(self) -> List[str]:
        """Inspect operating systems - async version

        Returns:
            List of root devices
        """
        ...

    async def inspect_get_type(self, root: str) -> str:
        """Get OS type - async version

        Returns:
            OS type (e.g., 'linux', 'windows')
        """
        ...

    async def inspect_get_distro(self, root: str) -> str:
        """Get distribution name - async version

        Returns:
            Distribution name (e.g., 'ubuntu', 'fedora')
        """
        ...

    async def inspect_get_major_version(self, root: str) -> int:
        """Get major version number - async version"""
        ...

    async def inspect_get_minor_version(self, root: str) -> int:
        """Get minor version number - async version"""
        ...

    async def inspect_get_hostname(self, root: str) -> str:
        """Get hostname - async version"""
        ...

    # Filesystem operations (async)
    async def list_filesystems(self) -> Dict[str, str]:
        """List filesystems - async version

        Returns:
            Dictionary mapping devices to filesystem types
        """
        ...

    async def mount(self, device: str, mountpoint: str) -> None:
        """Mount a filesystem - async version"""
        ...

    async def ls(self, directory: str) -> List[str]:
        """List directory contents - async version

        Returns:
            List of filenames
        """
        ...

    async def cat(self, path: str) -> str:
        """Read file contents - async version

        Returns:
            File contents as string
        """
        ...
'''

def run_doctor(
    image: str,
    target: str = "kvm",
    explain: bool = False,
    verbose: bool = False,
) -> Dict[str, Any]: ...

def run_boot_inspect(
    image: str,
    target: str = "kvm",
    verbose: bool = False,
) -> Dict[str, Any]: ...

def run_migrate_plan(
    image: str,
    target: str = "kvm",
    explain: bool = False,
    verbose: bool = False,
    export_fix_plan: bool = False,
) -> Dict[str, Any]: ...

def run_repair_plan(
    image: str,
    dry_run: bool = True,
    verbose: bool = False,
    fix_cloud_init_network: bool = False,
    validate_fstab: bool = False,
) -> Dict[str, Any]: ...

def run_migrate_repair(
    image: str,
    target: str = "kvm",
    apply: bool = False,
    include_destructive: bool = False,
    virtio_win: Optional[str] = None,
    verbose: bool = False,
    inject_json: Optional[str] = None,
) -> Dict[str, Any]:
    """Hypervisor-aware offline repair.

    ``inject_json`` is a JSON object string (hostname, network files, users,
    services, first-boot, cloud-init, AD rejoin, KMS, RDP). Omitted, empty,
    or ``"null"`` adds nothing. Invalid JSON raises ``ValueError``.
    """
    ...

def live_fix_commands(
    update_grub: bool = True,
    regen_initramfs: bool = True,
    remove_vmware_tools: bool = False,
) -> List[str]:
    """Shell commands to run on a booted guest (initramfs, GRUB, open-vm-tools)."""
    ...

def run_live_plan(commands: List[str], dry_run: bool = False) -> Dict[str, Any]:
    """Run ``commands`` on this machine. Does not SSH. ``dry_run=False`` executes."""
    ...
