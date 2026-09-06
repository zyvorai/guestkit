// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Main GuestFS handle implementation

use crate::core::{Error, Result};
use crate::disk::{DiskReader, LoopDevice, NbdDevice, PartitionTable};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// GuestFS handle state
#[derive(Debug, PartialEq)]
pub enum GuestfsState {
    /// Initial state after creation
    Config,
    /// Between Config and Ready during launch
    Launching,
    /// After launch() called successfully
    Ready,
    /// Error state — launch or other critical operation failed
    Error,
    /// After shutdown() called
    Closed,
}

/// UTF-8 handling policy
#[derive(Debug, Clone, PartialEq)]
pub enum Utf8Policy {
    /// Return error on invalid UTF-8
    Strict,
    /// Replace invalid UTF-8 with U+FFFD (default for backward compatibility)
    Lossy,
}

/// Resource limits for safe operation
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum file size in bytes (default: 100MB)
    pub max_file_size: Option<u64>,
    /// Operation timeout in seconds (default: 300s)
    pub operation_timeout: Option<Duration>,
    /// Maximum path length (default: 4096)
    pub max_path_length: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_file_size: Some(100 * 1024 * 1024),            // 100MB
            operation_timeout: Some(Duration::from_secs(300)), // 5 minutes
            max_path_length: 4096,
        }
    }
}

/// Main GuestFS handle
pub struct Guestfs {
    pub(crate) state: GuestfsState,
    pub(crate) verbose: bool,
    pub(crate) trace: bool,
    pub(crate) debug: bool,
    pub(crate) readonly: bool,
    pub(crate) drives: Vec<DriveConfig>,
    pub(crate) reader: Option<DiskReader>,
    pub(crate) partition_table: Option<PartitionTable>,
    pub(crate) nbd_device: Option<NbdDevice>,
    pub(crate) loop_device: Option<LoopDevice>,
    pub(crate) mounted: HashMap<String, String>, // device -> mountpoint
    pub(crate) mount_root: Option<PathBuf>,      // Temporary mount directory
    pub(crate) lazy_unmount_used: bool,          // Track if lazy unmount was needed
    pub(crate) activated_vgs: Vec<String>,       // Track activated LVM volume groups for cleanup
    pub(crate) identifier: Option<String>,
    pub(crate) autosync: bool,
    pub(crate) selinux: bool,
    pub(crate) utf8_policy: Utf8Policy,
    pub(crate) resource_limits: ResourceLimits,
    pub(crate) windows_version_cache: HashMap<String, (String, String, String)>, // Cache for Windows registry data (root -> (product, version, edition))
    pub(crate) open_hives: HashMap<i64, PathBuf>, // Tracks open registry hive files (handle -> host path)
    /// Node handle → key path from hive root (empty path = root). Keyed by (hive_handle, node).
    pub(crate) hive_nodes: HashMap<(i64, i64), Vec<String>>,
    /// Value handle → (node path, value name). Keyed by (hive_handle, value).
    pub(crate) hive_values: HashMap<(i64, i64), (Vec<String>, String)>,
    /// Per-hive next allocatable (node_id, value_id); node 0 is always root.
    pub(crate) hive_next_ids: HashMap<i64, (i64, i64)>,
    /// Bind-mount target for mount_local / umount_local.
    pub(crate) local_mountpoint: Option<String>,
    pub(crate) ova_temp: Vec<tempfile::TempDir>, // Keeps VMDKs extracted from OVA archives alive for the handle's lifetime
    /// Wall-clock time spent in the most recent `launch()` call, success or failure.
    pub(crate) launch_duration: Option<Duration>,
}

/// Drive configuration
#[derive(Debug, Clone)]
pub struct DriveConfig {
    pub path: PathBuf,
    pub readonly: bool,
    pub format: Option<String>,
}

impl Guestfs {
    /// Create a new GuestFS handle
    ///
    /// # Examples
    ///
    /// ```
    /// use guestkit::guestfs::Guestfs;
    ///
    /// let g = Guestfs::new().unwrap();
    /// ```
    pub fn new() -> Result<Self> {
        Ok(Self {
            state: GuestfsState::Config,
            verbose: false,
            trace: false,
            debug: false,
            readonly: false,
            drives: Vec::new(),
            reader: None,
            partition_table: None,
            nbd_device: None,
            loop_device: None,
            mounted: HashMap::new(),
            mount_root: None,
            lazy_unmount_used: false,
            activated_vgs: Vec::new(),
            identifier: None,
            autosync: true,
            selinux: false,
            utf8_policy: Utf8Policy::Lossy,
            resource_limits: ResourceLimits::default(),
            windows_version_cache: HashMap::new(),
            open_hives: HashMap::new(),
            hive_nodes: HashMap::new(),
            hive_values: HashMap::new(),
            hive_next_ids: HashMap::new(),
            local_mountpoint: None,
            ova_temp: Vec::new(),
            launch_duration: None,
        })
    }

    /// Create a new GuestFS handle
    ///
    pub fn create() -> Result<Self> {
        Self::new()
    }

    /// Add a drive in read-write mode
    ///
    pub fn add_drive<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        self.add_drive_opts(path, false, None)
    }

    /// Add a drive in read-only mode
    ///
    pub fn add_drive_ro<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        self.add_drive_opts(path, true, None)
    }

    /// Add a drive with options
    ///
    pub fn add_drive_opts<P: AsRef<Path>>(
        &mut self,
        path: P,
        readonly: bool,
        format: Option<&str>,
    ) -> Result<()> {
        if self.state != GuestfsState::Config {
            return Err(Error::InvalidState(
                "Cannot add drives after launch".to_string(),
            ));
        }

        // OVA/OVF bundles are tar archives, not disks — transparently extract the
        // embedded disk and add that instead (extension is unreliable, sniff too).
        let resolved = self.maybe_extract_ova(path.as_ref())?;

        self.drives.push(DriveConfig {
            path: resolved,
            readonly,
            format: format.map(|s| s.to_string()),
        });

        Ok(())
    }

    /// If `path` is an OVA (a tar archive containing a disk image), extract the
    /// primary disk to a temp dir owned by this handle and return its path.
    /// Otherwise return `path` unchanged.
    #[cfg(feature = "guest-inspect")]
    fn maybe_extract_ova(&mut self, path: &Path) -> Result<PathBuf> {
        let looks_ova = path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("ova"))
            .unwrap_or(false)
            || Self::is_tar_archive(path);
        if !looks_ova {
            return Ok(path.to_path_buf());
        }

        let file = std::fs::File::open(path).map_err(Error::Io)?;
        let mut archive = tar::Archive::new(file);
        let dir = tempfile::tempdir().map_err(Error::Io)?;

        // Pick the largest disk-shaped entry (OVAs list the boot disk first, but
        // choosing by size is robust for multi-file bundles).
        const DISK_EXTS: [&str; 7] = ["vmdk", "img", "raw", "qcow2", "vhd", "vhdx", "vdi"];
        let mut best: Option<(PathBuf, u64)> = None;
        for entry in archive.entries().map_err(Error::Io)? {
            let mut entry = entry.map_err(Error::Io)?;
            let ent_path = entry.path().map_err(Error::Io)?.to_path_buf();
            let is_disk = ent_path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| DISK_EXTS.contains(&e.to_ascii_lowercase().as_str()))
                .unwrap_or(false);
            if !is_disk {
                continue;
            }
            let size = entry.header().size().unwrap_or(0);
            let name = ent_path
                .file_name()
                .map(|n| n.to_os_string())
                .unwrap_or_else(|| std::ffi::OsString::from("disk.img"));
            let out = dir.path().join(name);
            entry.unpack(&out).map_err(Error::Io)?;
            match &best {
                Some((_, bs)) if *bs >= size => {}
                _ => best = Some((out, size)),
            }
        }

        let disk = best.map(|(p, _)| p).ok_or_else(|| {
            Error::InvalidOperation(format!(
                "OVA archive {} contains no disk image (.vmdk/.img/.raw/.qcow2)",
                path.display()
            ))
        })?;

        if self.verbose {
            eprintln!("guestfs: extracted OVA disk -> {}", disk.display());
        }
        self.ova_temp.push(dir);
        Ok(disk)
    }

    #[cfg(not(feature = "guest-inspect"))]
    fn maybe_extract_ova(&mut self, path: &Path) -> Result<PathBuf> {
        Ok(path.to_path_buf())
    }

    /// Sniff the POSIX tar `ustar` magic at offset 257 (covers .ova without the
    /// `.ova` extension, e.g. a server-vault UUID filename).
    #[cfg(feature = "guest-inspect")]
    fn is_tar_archive(path: &Path) -> bool {
        use std::io::{Read, Seek, SeekFrom};
        let mut f = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(_) => return false,
        };
        if f.seek(SeekFrom::Start(257)).is_err() {
            return false;
        }
        let mut magic = [0u8; 5];
        f.read_exact(&mut magic).is_ok() && &magic == b"ustar"
    }

    /// Choose loop (raw) vs NBD (qcow2/vmdk/…) based on on-disk format, not extension.
    fn should_use_loop_device(path: &Path) -> Result<bool> {
        use crate::core::DiskFormat;

        match DiskReader::detect_image_format(path) {
            Ok(DiskFormat::Raw) => Ok(true),
            Ok(
                DiskFormat::Qcow2
                | DiskFormat::Vmdk
                | DiskFormat::Vhd
                | DiskFormat::Vhdx
                | DiskFormat::Vdi,
            ) => Ok(false),
            Ok(DiskFormat::Unknown) | Err(_) => Ok(LoopDevice::is_format_supported(path)),
        }
    }

    /// Launch the guestfs handle (prepare for operations)
    pub fn launch(&mut self) -> Result<()> {
        if self.state != GuestfsState::Config {
            return Err(Error::InvalidState(format!(
                "Cannot launch from state: {:?}",
                self.state
            )));
        }

        if self.drives.is_empty() {
            self.state = GuestfsState::Error;
            return Err(Error::InvalidState("No drives added".to_string()));
        }

        // Transition to Launching state
        self.state = GuestfsState::Launching;
        let launch_started = std::time::Instant::now();

        // Open the first drive (multi-drive not yet supported)
        let drive = &self.drives[0];

        // Attempt to launch - if any error occurs, move to Error state
        let result: Result<()> = (|| {
            // Strategy: loop for true raw images; NBD for qcow2/vmdk/etc.
            // Extension alone is unreliable (.img is often qcow2, e.g. Cirros cloud images).
            let use_loop_device = Self::should_use_loop_device(&drive.path)?;
            if self.debug {
                eprintln!(
                    "[DEBUG] File: {}, use_loop_device: {}",
                    drive.path.display(),
                    use_loop_device
                );
            }

            if use_loop_device {
                // Use loop device for RAW/IMG/ISO formats (built into Linux kernel)
                if self.trace {
                    eprintln!("guestfs: using loop device for raw disk format");
                }

                let mut loop_dev = LoopDevice::new()?;
                loop_dev.connect(&drive.path, drive.readonly)?;

                let device_path = loop_dev
                    .device_path()
                    .ok_or_else(|| Error::InvalidState("Loop device not connected".to_string()))?;

                // Read partitions from the loop device (open once, reuse for partition parsing)
                let mut reader = DiskReader::open(device_path)?;
                let partition_table = PartitionTable::parse(&mut reader)?;

                self.reader = Some(reader);
                self.partition_table = Some(partition_table);
                self.loop_device = Some(loop_dev);
            } else {
                // Use NBD for QCOW2/VMDK/VDI/VHD formats
                if self.trace {
                    eprintln!("guestfs: using NBD for qcow2/vmdk/vdi/vhd disk format");
                }

                if self.debug {
                    eprintln!("[DEBUG] Creating NBD device...");
                }
                let mut nbd = NbdDevice::new()?;
                if self.debug {
                    eprintln!(
                        "[DEBUG] NBD device created: {}",
                        nbd.device_path().display()
                    );
                    eprintln!("[DEBUG] Connecting NBD to image: {}", drive.path.display());
                }
                nbd.connect(&drive.path, drive.readonly)?;
                if self.debug {
                    eprintln!("[DEBUG] NBD connected successfully");
                    eprintln!(
                        "[DEBUG] Opening DiskReader for NBD device: {}",
                        nbd.device_path().display()
                    );
                }
                let mut reader = DiskReader::open(nbd.device_path())?;
                if self.debug {
                    eprintln!("[DEBUG] DiskReader opened successfully");
                }
                let partition_table = PartitionTable::parse(&mut reader)?;

                self.reader = Some(reader);
                self.partition_table = Some(partition_table);
                self.nbd_device = Some(nbd);
            }

            Ok(())
        })();

        self.launch_duration = Some(launch_started.elapsed());

        match result {
            Ok(_) => {
                self.state = GuestfsState::Ready;

                if self.trace {
                    eprintln!("guestfs: launched with {} drive(s)", self.drives.len());
                }

                Ok(())
            }
            Err(e) => {
                self.state = GuestfsState::Error;
                Err(e)
            }
        }
    }

    /// Shutdown the guestfs handle
    pub fn shutdown(&mut self) -> Result<()> {
        if self.state == GuestfsState::Closed {
            return Ok(());
        }

        if self.trace {
            eprintln!("guestfs: shutdown - starting cleanup");
        }

        // Step 0: Close readers FIRST (they hold file descriptors to devices)
        if self.trace {
            eprintln!("guestfs: closing disk readers");
        }
        self.reader = None;
        self.partition_table = None;

        // Ensure all file descriptors are flushed and closed
        // CRITICAL: Wait for kernel to release all filesystem references
        // This is essential to avoid EBUSY during unmount
        let _ = std::process::Command::new("sync").output();
        if self.trace {
            eprintln!("guestfs: waiting for kernel to release filesystem references...");
        }
        // Poll for readiness instead of sleeping a fixed 2 seconds
        for _ in 0..10 {
            std::thread::sleep(std::time::Duration::from_millis(200));
            if self.mounted.is_empty() {
                break;
            }
            // Check if mounts are still busy
            if let Some(ref mount_root) = self.mount_root {
                let check = std::process::Command::new("fuser")
                    .arg("-m")
                    .arg(mount_root)
                    .output();
                if let Ok(output) = check {
                    if !output.status.success() || output.stdout.is_empty() {
                        break; // No processes using the mount
                    }
                } else {
                    break; // fuser not available, proceed
                }
            } else {
                break;
            }
        }

        // Step 1: Unmount all filesystems (before disconnecting devices)
        let had_mounts = !self.mounted.is_empty();
        if had_mounts {
            if self.trace {
                eprintln!("guestfs: unmounting {} filesystem(s)", self.mounted.len());
            }

            match self.umount_all() {
                Ok(_) => {
                    if self.trace {
                        eprintln!("guestfs: all filesystems unmounted successfully");
                    }
                }
                Err(e) => {
                    eprintln!(
                        "Warning: umount_all failed: {}. Attempting lazy unmount.",
                        e
                    );
                    // Attempt lazy unmount as fallback to avoid leaving mounts behind
                    if let Some(ref mount_root) = self.mount_root {
                        let need_sudo = crate::guestfs::mount::need_sudo();
                        let mut lazy_cmd = if need_sudo {
                            let mut c = std::process::Command::new("sudo");
                            c.arg("umount");
                            c
                        } else {
                            std::process::Command::new("umount")
                        };
                        let _ = lazy_cmd.arg("-l").arg(mount_root).output();
                        self.lazy_unmount_used = true;
                    }
                }
            }

            // CRITICAL: Verify unmount actually worked
            // umount command can return success before kernel fully processes the unmount
            if let Some(mount_root) = &self.mount_root {
                for attempt in 1..=10 {
                    let check = std::process::Command::new("findmnt")
                        .arg("-R")
                        .arg(mount_root)
                        .output();

                    if let Ok(output) = check {
                        if output.stdout.is_empty() || !output.status.success() {
                            // No mounts found - unmount completed
                            if self.trace && attempt > 1 {
                                eprintln!("guestfs: unmount verified after {} attempts", attempt);
                            }
                            break;
                        }
                    }

                    if attempt < 10 {
                        // Still mounted - wait and retry
                        if self.trace {
                            eprintln!(
                                "guestfs: waiting for unmount to complete (attempt {})",
                                attempt
                            );
                        }
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    } else {
                        eprintln!("Error: Unmount verification failed after {} attempts. Filesystem may still be active.", attempt);
                        return Err(Error::CommandFailed(format!(
                            "Unmount verification failed after {} attempts. Filesystem under {} may still be active. \
                             Check with: findmnt -R {}",
                            attempt,
                            mount_root.display(),
                            mount_root.display()
                        )));
                    }
                }
            }
        }

        // Step 1.5: Deactivate LVM volume groups
        // This must happen after unmount but before NBD/loop disconnect
        if !self.activated_vgs.is_empty() {
            if self.trace {
                eprintln!(
                    "guestfs: deactivating {} LVM volume group(s)",
                    self.activated_vgs.len()
                );
            }

            // Build device filter for LVM cleanup (same as activation)
            // Escape all regex metacharacters, not just forward slashes
            let cleanup_filter = if let Some(ref nbd) = self.nbd_device {
                let path = Self::escape_lvm_regex(&nbd.device_path().display().to_string());
                format!(
                    r#"devices {{ filter=["a|^{}|","r|.*|"] }} global {{ locking_type=0 }}"#,
                    path
                )
            } else if let Some(ref loop_dev) = self.loop_device {
                let path = loop_dev
                    .device_path()
                    .map(|p| Self::escape_lvm_regex(&p.display().to_string()))
                    .unwrap_or_else(|| r"\/dev\/loop".to_string());
                format!(
                    r#"devices {{ filter=["a|^{}|","r|.*|"] }} global {{ locking_type=0 }}"#,
                    path
                )
            } else {
                String::new()
            };

            for vg in &self.activated_vgs {
                if self.trace {
                    eprintln!("guestfs: deactivating volume group {}", vg);
                }

                let mut cmd = std::process::Command::new("vgchange");
                cmd.arg("-an").arg(vg);
                if !cleanup_filter.is_empty() {
                    cmd.arg("--config").arg(&cleanup_filter);
                }
                let output = cmd.output();

                match output {
                    Ok(out) if out.status.success() => {
                        if self.trace {
                            eprintln!("guestfs: volume group {} deactivated", vg);
                        }
                    }
                    Ok(out) => {
                        eprintln!(
                            "Warning: failed to deactivate volume group {}: {}",
                            vg,
                            String::from_utf8_lossy(&out.stderr)
                        );
                    }
                    Err(e) => {
                        eprintln!("Warning: failed to run vgchange for {}: {}", vg, e);
                    }
                }
            }

            self.activated_vgs.clear();
        }

        // Step 2: Disconnect loop device
        if let Some(mut loop_dev) = self.loop_device.take() {
            if self.trace {
                eprintln!("guestfs: disconnecting loop device");
            }
            match loop_dev.disconnect() {
                Ok(_) => {
                    if self.trace {
                        eprintln!("guestfs: loop device disconnected");
                    }
                }
                Err(e) => {
                    eprintln!("Warning: loop device disconnect failed: {}", e);
                }
            }
        }

        // Step 3: Disconnect NBD device
        // CRITICAL: Do NOT disconnect NBD if lazy unmount was used!
        // Lazy unmount means the filesystem is still active in the kernel,
        // and disconnecting NBD will cause I/O errors when the kernel tries to access it.
        if self.lazy_unmount_used {
            if self.trace {
                eprintln!("guestfs: skipping NBD disconnect because lazy unmount was used");
            }

            // Take the NBD device and intentionally leak it to prevent Drop from running.
            // SAFETY: We intentionally leak the NbdDevice because a lazy unmount is in progress.
            // The kernel holds mount references and will clean up the device when all references
            // are released. Calling Drop here would try to disconnect the NBD device while it's
            // still in use, which would fail or cause data corruption.
            if let Some(nbd) = self.nbd_device.take() {
                let device_path = nbd.device_path().to_path_buf();

                // Write a cleanup script instead of leaking the NbdDevice via mem::forget.
                // This lets the kernel handle lazy unmount while providing a deterministic
                // cleanup mechanism. Drop will attempt disconnect; if it fails because the
                // device is still busy, the warning below tells the user how to clean up.
                eprintln!(
                    "Warning: NBD device {} cleanup deferred due to lazy unmount.",
                    device_path.display()
                );
                eprintln!("The device will be freed automatically when the kernel releases all mount references.");
                eprintln!(
                    "To check status: findmnt -R {} && lsblk {}",
                    self.mount_root
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default(),
                    device_path.display()
                );
                eprintln!(
                    "To force cleanup after refs are gone: sudo qemu-nbd --disconnect {}",
                    device_path.display()
                );
                // Allow Drop to run — it will attempt disconnect and log a warning if it fails.
                // This is safer than mem::forget which permanently leaks the device.
                drop(nbd);
            }
        } else if let Some(mut nbd) = self.nbd_device.take() {
            if self.trace {
                eprintln!("guestfs: disconnecting NBD device");
            }

            // Add diagnostic checks before disconnect
            let device_path = nbd.device_path().to_path_buf();

            // Check if device is actually still mounted
            if let Some(mount_root) = &self.mount_root {
                let findmnt_check = std::process::Command::new("findmnt")
                    .arg("-R")
                    .arg(mount_root)
                    .output();

                if let Ok(output) = findmnt_check {
                    if !output.stdout.is_empty() && output.status.success() {
                        eprintln!(
                            "Warning: Device {} still has active mounts:",
                            device_path.display()
                        );
                        eprintln!("{}", String::from_utf8_lossy(&output.stdout));
                        eprintln!("Skipping disconnect to avoid I/O errors.");
                        // Allow Drop to run — it will attempt disconnect and warn if it fails.
                        drop(nbd);
                        self.state = GuestfsState::Closed;
                        return Ok(());
                    }
                }
            }

            match nbd.disconnect() {
                Ok(_) => {
                    if self.trace {
                        eprintln!("guestfs: NBD device disconnected");
                    }
                }
                Err(e) => {
                    eprintln!("Warning: NBD device disconnect failed: {}", e);
                }
            }
        }

        // Step 4: Clean up mount root directory
        if let Some(mount_root) = self.mount_root.take() {
            // If lazy unmount was used, skip directory cleanup - it will be cleaned up
            // automatically when the lazy unmount completes
            if self.lazy_unmount_used {
                eprintln!(
                    "Note: Lazy unmount was used. Directory {} will be cleaned up automatically.",
                    mount_root.display()
                );
                eprintln!(
                    "If you want to clean it up manually later, run: sudo rm -rf {}",
                    mount_root.display()
                );
                self.state = GuestfsState::Closed;
                return Ok(());
            }

            if self.trace {
                eprintln!("guestfs: removing mount root: {}", mount_root.display());
            }

            // Ensure all filesystem operations are complete before trying to remove
            if had_mounts {
                let _ = std::process::Command::new("sync").output();
                std::thread::sleep(std::time::Duration::from_secs(1));
            }

            // Verify nothing is mounted under mount_root
            if let Ok(output) = std::process::Command::new("mount").output() {
                let mount_output = String::from_utf8_lossy(&output.stdout);
                let mount_root_str = mount_root.to_string_lossy();
                // Check if any mount line has our mount root as the "on <path>" field
                // Mount output format: "<device> on <path> type <fstype> (<options>)"
                let still_mounted = mount_output.lines().any(|line| {
                    line.split_whitespace()
                        .nth(2) // The mount path is the 3rd field (index 2)
                        .is_some_and(|field| {
                            field == mount_root_str.as_ref()
                                || field.starts_with(&format!("{}/", mount_root_str))
                        })
                });
                if still_mounted {
                    eprintln!(
                        "Warning: mount_root {} still has active mounts (likely from lazy unmount)",
                        mount_root.display()
                    );
                    eprintln!("Note: Lazy unmount was used. Directory {} will be cleaned up automatically.", mount_root.display());
                    eprintln!(
                        "If you want to clean it up manually later, run: sudo rm -rf {}",
                        mount_root.display()
                    );
                    return Ok(());
                }
            }

            // First try without sudo
            match std::fs::remove_dir_all(&mount_root) {
                Ok(_) => {
                    if self.trace {
                        eprintln!("guestfs: mount root removed");
                    }
                }
                Err(e) => {
                    // If permission denied or read-only, try with sudo
                    if self.trace {
                        eprintln!("guestfs: normal removal failed ({}), trying with sudo", e);
                    }

                    let need_sudo = crate::guestfs::mount::need_sudo();
                    let mut cmd = if need_sudo {
                        let mut sudo_cmd = std::process::Command::new("sudo");
                        sudo_cmd.arg("rm");
                        sudo_cmd
                    } else {
                        std::process::Command::new("rm")
                    };

                    match cmd.arg("-rf").arg(&mount_root).output() {
                        Ok(output) if output.status.success() => {
                            if self.trace {
                                eprintln!("guestfs: mount root removed with sudo");
                            }
                        }
                        Ok(output) => {
                            eprintln!(
                                "Warning: failed to remove mount root {} with sudo: {}",
                                mount_root.display(),
                                String::from_utf8_lossy(&output.stderr)
                            );
                        }
                        Err(e2) => {
                            eprintln!(
                                "Warning: failed to remove mount root {}: {} (sudo also failed: {})",
                                mount_root.display(),
                                e,
                                e2
                            );
                        }
                    }
                }
            }
        }

        // Step 5: Final state transition
        self.state = GuestfsState::Closed;

        if self.trace {
            eprintln!("guestfs: shutdown complete");
        }

        Ok(())
    }

    /// Close the handle (same as shutdown)
    pub fn close(&mut self) -> Result<()> {
        self.shutdown()
    }

    /// Set verbose mode
    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
    }

    /// Get verbose mode
    pub fn get_verbose(&self) -> bool {
        self.verbose
    }

    /// Set trace mode
    pub fn set_trace(&mut self, trace: bool) {
        self.trace = trace;
    }

    /// Get trace mode
    pub fn get_trace(&self) -> bool {
        self.trace
    }

    /// Set debug mode
    pub fn set_debug(&mut self, debug: bool) {
        self.debug = debug;
    }

    /// Get debug mode
    pub fn get_debug(&self) -> bool {
        self.debug
    }

    /// Get current state
    pub fn state(&self) -> &GuestfsState {
        &self.state
    }

    /// Get reader reference (internal — used by consumers of the crate API)
    #[allow(dead_code)]
    pub(crate) fn reader_mut(&mut self) -> Result<&mut DiskReader> {
        self.reader
            .as_mut()
            .ok_or_else(|| Error::InvalidState("Not launched".to_string()))
    }

    /// Get partition table reference (internal)
    pub(crate) fn partition_table(&self) -> Result<&PartitionTable> {
        self.partition_table
            .as_ref()
            .ok_or_else(|| Error::InvalidState("Not launched".to_string()))
    }

    /// Get NBD device reference safely (internal)
    pub(crate) fn nbd_device(&self) -> Result<&NbdDevice> {
        self.nbd_device.as_ref().ok_or_else(|| {
            Error::InvalidState(
                "NBD device not initialized. Call setup_nbd_if_needed() first.".to_string(),
            )
        })
    }

    /// Get mutable NBD device reference safely (internal — used by consumers of the crate API)
    #[allow(dead_code)]
    pub(crate) fn nbd_device_mut(&mut self) -> Result<&mut NbdDevice> {
        self.nbd_device.as_mut().ok_or_else(|| {
            Error::InvalidState(
                "NBD device not initialized. Call setup_nbd_if_needed() first.".to_string(),
            )
        })
    }

    /// Convert path to string safely (internal — used by consumers of the crate API)
    #[allow(dead_code)]
    pub(crate) fn path_to_string(path: &Path) -> Result<String> {
        path.to_str()
            .ok_or_else(|| {
                Error::InvalidFormat(format!("Path contains invalid Unicode: {:?}", path))
            })
            .map(|s| s.to_string())
    }

    /// Set UTF-8 handling policy
    pub fn set_utf8_policy(&mut self, policy: Utf8Policy) {
        self.utf8_policy = policy;
    }

    /// Get current UTF-8 handling policy
    pub fn get_utf8_policy(&self) -> &Utf8Policy {
        &self.utf8_policy
    }

    /// Decode bytes to UTF-8 string according to policy (internal — used by consumers of the crate API)
    #[allow(dead_code)]
    pub(crate) fn decode_utf8(&self, bytes: &[u8]) -> Result<String> {
        match self.utf8_policy {
            Utf8Policy::Strict => String::from_utf8(bytes.to_vec())
                .map_err(|e| Error::InvalidFormat(format!("Invalid UTF-8: {}", e))),
            Utf8Policy::Lossy => {
                let result = String::from_utf8_lossy(bytes);
                if result.contains('\u{FFFD}') {
                    log::warn!(
                        "Invalid UTF-8 bytes replaced with U+FFFD replacement character. \
                               Use set_utf8_policy(Utf8Policy::Strict) to treat this as an error."
                    );
                }
                Ok(result.to_string())
            }
        }
    }

    /// Set resource limits
    pub fn set_resource_limits(&mut self, limits: ResourceLimits) {
        self.resource_limits = limits;
    }

    /// Get current resource limits
    pub fn get_resource_limits(&self) -> &ResourceLimits {
        &self.resource_limits
    }

    /// Check if file size is within limits (internal — used by consumers of the crate API)
    #[allow(dead_code)]
    pub(crate) fn check_file_size_limit(&self, size: u64) -> Result<()> {
        if let Some(max) = self.resource_limits.max_file_size {
            if size > max {
                return Err(Error::InvalidOperation(format!(
                    "File size {} exceeds limit {} bytes",
                    size, max
                )));
            }
        }
        Ok(())
    }

    /// Check if path length is within limits (internal — used by consumers of the crate API)
    #[allow(dead_code)]
    pub(crate) fn check_path_length_limit(&self, path: &str) -> Result<()> {
        if path.len() > self.resource_limits.max_path_length {
            return Err(Error::InvalidOperation(format!(
                "Path length {} exceeds limit {}",
                path.len(),
                self.resource_limits.max_path_length
            )));
        }
        Ok(())
    }

    /// Escape regex metacharacters for LVM device filter patterns
    pub(crate) fn escape_lvm_regex(path: &str) -> String {
        let mut escaped = String::with_capacity(path.len() * 2);
        for c in path.chars() {
            match c {
                '/' | '.' | '[' | ']' | '*' | '+' | '?' | '^' | '$' | '|' | '(' | ')' | '{'
                | '}' | '\\' => {
                    escaped.push('\\');
                    escaped.push(c);
                }
                _ => escaped.push(c),
            }
        }
        escaped
    }

    /// Check if ready for operations
    pub(crate) fn ensure_ready(&self) -> Result<()> {
        if self.state != GuestfsState::Ready {
            return Err(Error::InvalidState(
                "Handle not ready (call launch first)".to_string(),
            ));
        }
        Ok(())
    }

    /// Parse device name to partition number
    ///
    /// Supports multiple device patterns:
    /// - /dev/sda, /dev/sda1, /dev/sda2, ...
    /// - /dev/vda, /dev/vda1, /dev/vda2, ...
    /// - /dev/hda, /dev/hda1, /dev/hda2, ...
    /// - /dev/xvda, /dev/xvda1, /dev/xvda2, ...
    /// - /dev/nvme0n1p1, /dev/nvme0n1p2, ...
    pub(crate) fn parse_device_name(&self, device: &str) -> Result<u32> {
        // Validate device path length
        if device.len() > 255 || !device.starts_with("/dev/") {
            return Err(Error::InvalidFormat(format!("Invalid device: {}", device)));
        }

        // Handle LVM logical volumes (/dev/mapper/*, /dev/vg_name/lv_name)
        if device.starts_with("/dev/mapper/")
            || (device.contains('/') && device.matches('/').count() >= 3)
        {
            // LVM devices don't have partition numbers - they are complete volumes
            return Ok(0);
        }

        // Support multiple device patterns
        let patterns = [
            "/dev/sda",
            "/dev/vda",
            "/dev/hda",
            "/dev/xvda",
            "/dev/nvme0n1p",
        ];

        for prefix in &patterns {
            if let Some(num_str) = device.strip_prefix(prefix) {
                if num_str.is_empty() {
                    return Ok(0); // Whole device (no partition number)
                }
                return num_str
                    .parse::<u32>()
                    .map_err(|_| Error::InvalidFormat(format!("Invalid device: {}", device)));
            }
        }

        Err(Error::InvalidFormat(format!(
            "Unsupported device pattern: {}. Supported: /dev/{{sd,vd,hd,xvd}}a*, /dev/nvme0n1p*, /dev/mapper/*",
            device
        )))
    }

    /// Set up NBD device if not already set up (internal helper)
    pub(crate) fn setup_nbd_if_needed(&mut self) -> Result<()> {
        if self.nbd_device.is_some() {
            return Ok(());
        }

        // Get first drive
        let drive = self
            .drives
            .first()
            .ok_or_else(|| Error::InvalidState("No drives added".to_string()))?;

        // Create and connect NBD device
        let mut nbd = NbdDevice::new()?;
        nbd.connect(&drive.path, drive.readonly)?;

        self.nbd_device = Some(nbd);

        if self.verbose {
            eprintln!("guestfs: NBD device connected");
        }

        Ok(())
    }
}

impl Drop for Guestfs {
    fn drop(&mut self) {
        if let Err(e) = self.shutdown() {
            eprintln!("Warning: guestfs shutdown failed during drop: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guestfs_creation() {
        let g = Guestfs::new().unwrap();
        assert_eq!(g.state(), &GuestfsState::Config);
    }

    #[test]
    fn test_guestfs_verbose() {
        let mut g = Guestfs::new().unwrap();
        assert!(!g.get_verbose());
        g.set_verbose(true);
        assert!(g.get_verbose());
    }

    #[test]
    fn test_guestfs_trace() {
        let mut g = Guestfs::new().unwrap();
        assert!(!g.get_trace());
        g.set_trace(true);
        assert!(g.get_trace());
    }
}
