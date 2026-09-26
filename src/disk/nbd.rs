// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! NBD (Network Block Device) support using qemu-nbd
//!
//! This module provides NBD device management for mounting disk images
//! as block devices. This allows filesystem access without implementing
//! full filesystem parsers.
//!
//! Device selection and `qemu-nbd -c` are serialized with a cross-process
//! `flock` so concurrent mounts never claim the same `/dev/nbdN`.

use crate::core::{DiskFormat, Error, Result};
use crate::disk::reader::DiskReader;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;

/// ioctl constant for getting block device size (Linux x86/x86_64)
#[cfg(target_os = "linux")]
const BLKGETSIZE64: libc::c_ulong = 0x80081272;

/// Check if debug mode is enabled
fn is_debug_enabled() -> bool {
    std::env::var("GUESTKIT_DEBUG").is_ok()
}

/// Cross-process exclusive lock held for the whole choose-and-connect window.
///
/// Prefer `/run/lock/guestkit-nbd.lock`; fall back to `/tmp` when `/run/lock`
/// is missing or not writable (unprivileged / container hosts).
struct NbdAllocLock {
    _file: File,
}

impl NbdAllocLock {
    fn acquire() -> Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;

            let candidates = [
                Path::new("/run/lock/guestkit-nbd.lock"),
                Path::new("/tmp/guestkit-nbd.lock"),
            ];
            let mut last_err: Option<String> = None;

            for path in candidates {
                if let Some(parent) = path.parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        last_err = Some(format!("create {}: {e}", parent.display()));
                        continue;
                    }
                }
                match OpenOptions::new()
                    .create(true)
                    .truncate(false)
                    .read(true)
                    .write(true)
                    .open(path)
                {
                    Ok(file) => {
                        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
                        if rc != 0 {
                            let err = std::io::Error::last_os_error();
                            last_err = Some(format!("flock {}: {err}", path.display()));
                            continue;
                        }
                        if is_debug_enabled() {
                            eprintln!("[DEBUG NBD] acquired alloc lock at {}", path.display());
                        }
                        return Ok(Self { _file: file });
                    }
                    Err(e) => {
                        last_err = Some(format!("open {}: {e}", path.display()));
                    }
                }
            }

            Err(Error::CommandFailed(format!(
                "Failed to acquire NBD allocation lock: {}",
                last_err.unwrap_or_else(|| "no candidates".into())
            )))
        }

        #[cfg(not(unix))]
        {
            // No flock on non-Unix; allocate without cross-process exclusion.
            let path = std::env::temp_dir().join("guestkit-nbd.lock");
            let file = OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(&path)
                .map_err(|e| {
                    Error::CommandFailed(format!(
                        "Failed to create NBD alloc lock placeholder: {e}"
                    ))
                })?;
            Ok(Self { _file: file })
        }
    }
}

impl Drop for NbdAllocLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let _ = unsafe { libc::flock(self._file.as_raw_fd(), libc::LOCK_UN) };
        }
    }
}

/// NBD device manager
pub struct NbdDevice {
    /// NBD device path (e.g., /dev/nbd0). Empty until [`Self::connect`].
    device_path: PathBuf,
    /// Image path being exported
    image_path: PathBuf,
    /// Whether device is connected
    connected: bool,
    /// qemu-nbd process handle
    qemu_nbd_process: Option<Child>,
}

impl NbdDevice {
    /// Create a new NBD device manager.
    ///
    /// Ensures the NBD module/devices are available but does **not** claim a
    /// `/dev/nbdN` index — allocation happens under flock inside [`Self::connect`].
    pub fn new() -> Result<Self> {
        Self::ensure_nbd_module()?;
        if !(0..16).any(|i| PathBuf::from(format!("/dev/nbd{i}")).exists()) {
            return Err(Error::NotFound(
                "No /dev/nbd* devices found. Try: sudo modprobe nbd max_part=16".to_string(),
            ));
        }

        Ok(NbdDevice {
            device_path: PathBuf::new(),
            image_path: PathBuf::new(),
            connected: false,
            qemu_nbd_process: None,
        })
    }

    /// Check if NBD module is loaded
    fn is_nbd_module_loaded() -> bool {
        if let Ok(output) = Command::new("lsmod").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            return stdout.lines().any(|line| line.starts_with("nbd "));
        }
        false
    }

    /// Load the NBD module if needed and wait for `/dev/nbd*` to appear.
    fn ensure_nbd_module() -> Result<()> {
        if Self::is_nbd_module_loaded() {
            return Ok(());
        }
        eprintln!("NBD kernel module not loaded. Attempting to load...");
        Self::load_nbd_module()?;
        eprintln!("NBD module loaded successfully.");
        Ok(())
    }

    /// Try to load NBD kernel module
    fn load_nbd_module() -> Result<()> {
        let need_sudo = crate::guestfs::mount::need_sudo();

        let mut cmd = if need_sudo {
            let mut sudo_cmd = Command::new("sudo");
            sudo_cmd.arg("modprobe");
            sudo_cmd
        } else {
            Command::new("modprobe")
        };

        let output = cmd
            .arg("nbd")
            .arg("max_part=16")
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute modprobe: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!(
                "Failed to load NBD module: {}. Try manually: sudo modprobe nbd max_part=16",
                stderr
            )));
        }

        // Wait a bit for devices to appear
        thread::sleep(Duration::from_millis(500));

        Ok(())
    }

    /// Check if NBD device is in use by checking if it's connected
    fn is_nbd_device_in_use(device_path: &Path) -> bool {
        // Extract device number from path (e.g., /dev/nbd0 -> 0)
        let device_name = device_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // More reliable check: see if device has non-zero size (means it's connected to an image)
        // This works even if the qemu-nbd process has died
        let size_path = format!("/sys/block/{}/size", device_name);
        if let Ok(size_str) = std::fs::read_to_string(&size_path) {
            if let Ok(size) = size_str.trim().parse::<u64>() {
                if size > 0 {
                    return true;
                }
            }
        }

        // Fallback: Check /sys/block/nbdX/pid for active connection
        let pid_path = format!("/sys/block/{}/pid", device_name);
        if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                if pid > 0 {
                    // Check if the process is actually running
                    let proc_path = format!("/proc/{}", pid);
                    return std::path::Path::new(&proc_path).exists();
                }
            }
        }

        false
    }

    /// Detect qemu-nbd `-f` format from magic bytes, then qemu-img, then extension.
    ///
    /// `.img` is not assumed raw — Cirros/Ubuntu cloud images are often qcow2.
    fn detect_image_format(path: &Path) -> String {
        if let Ok(fmt) = DiskReader::detect_image_format(path) {
            if let Some(nbd_fmt) = Self::disk_format_to_nbd(fmt) {
                if is_debug_enabled() {
                    eprintln!(
                        "[DEBUG NBD] Detected format '{nbd_fmt}' via magic bytes for {}",
                        path.display()
                    );
                }
                return nbd_fmt;
            }
        }

        if let Some(fmt) = Self::probe_qemu_img_format(path) {
            return fmt;
        }

        // Extension hint for unambiguous types only (never .img → raw)
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if let Some(fmt) = match ext.to_lowercase().as_str() {
                "qcow2" => Some("qcow2"),
                "vmdk" => Some("vmdk"),
                "vdi" => Some("vdi"),
                "vhd" | "vpc" => Some("vpc"),
                "vhdx" => Some("vhdx"),
                "raw" => Some("raw"),
                _ => None,
            } {
                return fmt.to_string();
            }
        }

        "raw".to_string()
    }

    fn disk_format_to_nbd(fmt: DiskFormat) -> Option<String> {
        Some(
            match fmt {
                DiskFormat::Qcow2 => "qcow2",
                DiskFormat::Vmdk => "vmdk",
                DiskFormat::Vhd => "vpc",
                DiskFormat::Vhdx => "vhdx",
                DiskFormat::Vdi => "vdi",
                DiskFormat::Raw => "raw",
                DiskFormat::Unknown => return None,
            }
            .to_string(),
        )
    }

    fn probe_qemu_img_format(path: &Path) -> Option<String> {
        let output = Command::new("qemu-img")
            .arg("info")
            .arg("--output=json")
            .arg(path)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let json = serde_json::from_slice::<serde_json::Value>(&output.stdout).ok()?;
        let fmt = json.get("format")?.as_str()?;
        if fmt == "file" {
            return None;
        }
        if is_debug_enabled() {
            eprintln!(
                "[DEBUG NBD] Detected format '{fmt}' via qemu-img for {}",
                path.display()
            );
        }
        Some(fmt.to_string())
    }

    /// List free `/dev/nbd*` indices. Caller must hold [`NbdAllocLock`].
    fn free_nbd_devices() -> Result<Vec<PathBuf>> {
        Self::ensure_nbd_module()?;

        let mut free = Vec::new();
        for i in 0..16 {
            let device = PathBuf::from(format!("/dev/nbd{i}"));
            if device.exists() && !Self::is_nbd_device_in_use(&device) {
                free.push(device);
            }
        }

        if free.is_empty() {
            return Err(Error::NotFound(
                "No available NBD devices found. All 16 NBD devices are in use. Try disconnecting unused devices with: for i in {0..15}; do sudo qemu-nbd --disconnect /dev/nbd$i; done".to_string()
            ));
        }
        Ok(free)
    }

    /// Connect disk image to an NBD device.
    ///
    /// Allocates a free `/dev/nbdN` and runs `qemu-nbd -c` under a process-wide
    /// flock so concurrent callers never share the same device index
    /// (fluxvm#104 / Keep concurrent sandbox create).
    ///
    /// # Arguments
    ///
    /// * `image_path` - Path to disk image (qcow2, raw, vmdk, etc.)
    /// * `read_only` - Whether to connect in read-only mode
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use guestkit::disk::nbd::NbdDevice;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut nbd = NbdDevice::new()?;
    /// nbd.connect("/path/to/disk.qcow2", true)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn connect<P: AsRef<Path>>(&mut self, image_path: P, read_only: bool) -> Result<()> {
        if self.connected {
            return Err(Error::InvalidState(
                "NBD device already connected".to_string(),
            ));
        }

        let image_path = image_path.as_ref();
        if !image_path.exists() {
            return Err(Error::NotFound(format!(
                "Image file does not exist: {}",
                image_path.display()
            )));
        }

        let format = Self::detect_image_format(image_path);
        let _lock = NbdAllocLock::acquire()?;

        let candidates = Self::free_nbd_devices()?;
        let mut last_err: Option<Error> = None;

        for device in candidates {
            self.device_path = device;
            match self.connect_one(image_path, read_only, &format) {
                Ok(()) => {
                    self.image_path = image_path.to_path_buf();
                    self.connected = true;
                    return Ok(());
                }
                Err(e) => {
                    if is_debug_enabled() {
                        eprintln!(
                            "[DEBUG NBD] connect to {} failed: {e}; trying next device",
                            self.device_path.display()
                        );
                    }
                    self.cleanup_failed_connect();
                    last_err = Some(e);
                }
            }
        }

        self.device_path = PathBuf::new();
        Err(last_err.unwrap_or_else(|| {
            Error::NotFound("No available NBD devices could be connected".to_string())
        }))
    }

    /// Spawn qemu-nbd against `self.device_path` and wait until the device is ready.
    /// Caller must hold [`NbdAllocLock`] and have set `self.device_path` to a free device.
    fn connect_one(&mut self, image_path: &Path, read_only: bool, format: &str) -> Result<()> {
        // Under the alloc lock we only select devices that looked free. If one
        // flipped to in-use (external qemu-nbd, not guestkit), skip — do not
        // qemu-nbd --disconnect another owner's device.
        if Self::is_nbd_device_in_use(&self.device_path) {
            return Err(Error::InvalidState(format!(
                "NBD device {} became busy before connect",
                self.device_path.display()
            )));
        }

        let need_sudo = crate::guestfs::mount::need_sudo();
        if is_debug_enabled() {
            eprintln!(
                "[DEBUG NBD] need_sudo={} device={}",
                need_sudo,
                self.device_path.display()
            );
        }

        let mut cmd = if need_sudo {
            let mut sudo_cmd = Command::new("sudo");
            sudo_cmd.arg("qemu-nbd");
            sudo_cmd
        } else {
            Command::new("qemu-nbd")
        };

        // Use short flags: -c instead of --connect, -f instead of --format
        // This is important! Long flags cause qemu-nbd to exit immediately
        cmd.arg("-c")
            .arg(&self.device_path)
            .arg("-f")
            .arg(format);

        // CRITICAL: Use -r (read-only) flag to prevent file locking issues
        // This allows multiple qemu-nbd processes to access the same file
        // (important when lazy unmount leaves a previous connection alive)
        if read_only {
            cmd.arg("-r");
        }

        cmd.arg(image_path);

        if is_debug_enabled() {
            eprintln!("[DEBUG NBD] Command: {:?}", cmd);
        }

        let mut child = cmd.spawn().map_err(|e| {
            Error::CommandFailed(format!(
                "Failed to spawn qemu-nbd: {}. Is qemu-nbd installed?",
                e
            ))
        })?;

        if is_debug_enabled() {
            eprintln!("[DEBUG NBD] Spawned process PID: {}", child.id());
        }
        thread::sleep(Duration::from_millis(500));

        // Note: qemu-nbd may exit after successfully connecting (daemonizes),
        // so we need to check if the device is actually connected, not just if the process is running
        let process_exited = match child.try_wait() {
            Ok(Some(status)) => {
                if is_debug_enabled() {
                    eprintln!(
                        "[DEBUG NBD] qemu-nbd process exited with status: {}",
                        status
                    );
                }
                true
            }
            Ok(None) => {
                if is_debug_enabled() {
                    eprintln!("[DEBUG NBD] Process still running");
                }
                false
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::CommandFailed(format!(
                    "Failed to check qemu-nbd process status: {}",
                    e
                )));
            }
        };

        self.qemu_nbd_process = Some(child);

        if let Err(e) = self.wait_for_device() {
            self.cleanup_failed_connect();

            if process_exited {
                return Err(Error::CommandFailed(format!(
                    "qemu-nbd exited and device did not become ready. \
                     Device {} may already be in use or the image may be corrupted. \
                     Original error: {}. Try: sudo qemu-nbd --disconnect {}",
                    self.device_path.display(),
                    e,
                    self.device_path.display()
                )));
            }
            return Err(Error::CommandFailed(format!(
                "Device did not become ready: {}. Try: sudo qemu-nbd --disconnect {}",
                e,
                self.device_path.display()
            )));
        }

        Ok(())
    }

    fn cleanup_failed_connect(&mut self) {
        if let Some(mut proc) = self.qemu_nbd_process.take() {
            let _ = proc.kill();
            let _ = proc.wait();
        }
        // Best-effort disconnect of the device we just tried (we hold the alloc lock).
        if !self.device_path.as_os_str().is_empty() {
            let need_sudo = crate::guestfs::mount::need_sudo();
            let mut cmd = if need_sudo {
                let mut sudo_cmd = Command::new("sudo");
                sudo_cmd.arg("qemu-nbd");
                sudo_cmd
            } else {
                Command::new("qemu-nbd")
            };
            let _ = cmd.arg("--disconnect").arg(&self.device_path).output();
            thread::sleep(Duration::from_millis(100));
        }
    }

    /// Wait for NBD device to become available
    fn wait_for_device(&self) -> Result<()> {
        for _i in 0..50 {
            // Try to read from device
            if std::fs::metadata(&self.device_path).is_ok() {
                // Check if device is actually readable and has non-zero size
                if let Ok(file) = std::fs::File::open(&self.device_path) {
                    #[cfg(target_os = "linux")]
                    {
                        use std::os::unix::io::AsRawFd;
                        let mut size_bytes: u64 = 0;
                        let result = unsafe {
                            libc::ioctl(
                                file.as_raw_fd(),
                                BLKGETSIZE64 as _,
                                &mut size_bytes as *mut u64,
                            )
                        };
                        if result == 0 && size_bytes > 0 {
                            return Ok(());
                        }
                    }
                    #[cfg(not(target_os = "linux"))]
                    {
                        let _ = file;
                        return Ok(());
                    }
                }
            }
            thread::sleep(Duration::from_millis(200));
        }

        Err(Error::CommandFailed(format!(
            "NBD device {} did not become ready in time (no data or zero size)",
            self.device_path.display()
        )))
    }

    /// Disconnect NBD device
    pub fn disconnect(&mut self) -> Result<()> {
        if !self.connected {
            return Ok(());
        }

        // Check if we need to use sudo
        let need_sudo = crate::guestfs::mount::need_sudo();

        // Disconnect using qemu-nbd
        let mut cmd = if need_sudo {
            let mut sudo_cmd = Command::new("sudo");
            sudo_cmd.arg("qemu-nbd");
            sudo_cmd
        } else {
            Command::new("qemu-nbd")
        };

        let output = cmd
            .arg("--disconnect")
            .arg(&self.device_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to disconnect NBD: {}", e)))?;

        if !output.status.success() {
            eprintln!(
                "Warning: qemu-nbd disconnect failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Wait for device to be fully disconnected (verify it's no longer connected)
        for i in 0..20 {
            if !Self::is_nbd_device_in_use(&self.device_path) {
                if is_debug_enabled() {
                    eprintln!(
                        "[DEBUG NBD] Device {} disconnected after {} attempts",
                        self.device_path.display(),
                        i + 1
                    );
                }
                if let Some(mut child) = self.qemu_nbd_process.take() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                self.connected = false;
                return Ok(());
            }
            thread::sleep(Duration::from_millis(100));
        }

        // Device still appears connected after retries - force disconnect one more time
        if is_debug_enabled() {
            eprintln!("[DEBUG NBD] Device still connected after 2s, attempting force disconnect");
        }

        // Try one more disconnect with -d flag (detach)
        let mut cmd = if need_sudo {
            let mut sudo_cmd = Command::new("sudo");
            sudo_cmd.arg("qemu-nbd");
            sudo_cmd
        } else {
            Command::new("qemu-nbd")
        };

        let _ = cmd.arg("-d").arg(&self.device_path).output();

        // Final check
        thread::sleep(Duration::from_millis(500));
        if !Self::is_nbd_device_in_use(&self.device_path) {
            if is_debug_enabled() {
                eprintln!(
                    "[DEBUG NBD] Device {} disconnected after force disconnect",
                    self.device_path.display()
                );
            }
            if let Some(mut child) = self.qemu_nbd_process.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
            self.connected = false;
            return Ok(());
        }

        // Device is still connected - this is a real failure, don't mark as disconnected
        eprintln!(
            "Warning: NBD device {} may still be connected after disconnect attempts",
            self.device_path.display()
        );

        // Kill and wait on the qemu-nbd child process if we still hold it
        if let Some(mut child) = self.qemu_nbd_process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        // Still mark as disconnected so Drop doesn't retry infinitely, but this is a real error
        self.connected = false;

        // Return an error to notify caller that cleanup failed
        Err(Error::CommandFailed(format!(
            "Failed to fully disconnect NBD device {}. Manual cleanup may be required: \
             sudo qemu-nbd --disconnect {}",
            self.device_path.display(),
            self.device_path.display()
        )))
    }

    /// Get NBD device path
    pub fn device_path(&self) -> &Path {
        &self.device_path
    }

    /// Get partition device path
    ///
    /// # Arguments
    ///
    /// * `partition_num` - Partition number (1-based)
    pub fn partition_path(&self, partition_num: u32) -> PathBuf {
        PathBuf::from(format!("{}p{}", self.device_path.display(), partition_num))
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// List partitions on NBD device
    pub fn list_partitions(&self) -> Result<Vec<PathBuf>> {
        if !self.connected {
            return Err(Error::InvalidState("NBD device not connected".to_string()));
        }

        let mut partitions = Vec::new();

        // Check for partitions (p1, p2, etc.)
        for i in 1..=16 {
            let part_path = self.partition_path(i);
            if part_path.exists() {
                partitions.push(part_path);
            } else {
                break;
            }
        }

        // If no partitions, return the main device
        if partitions.is_empty() {
            partitions.push(self.device_path.clone());
        }

        Ok(partitions)
    }
}

impl Drop for NbdDevice {
    fn drop(&mut self) {
        if let Err(e) = self.disconnect() {
            eprintln!(
                "Warning: Failed to disconnect NBD device {}: {}",
                self.device_path.display(),
                e
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    #[test]
    fn test_nbd_device_creation() {
        // Just test that we can create the struct
        // Actual connection requires root and qemu-nbd
        let result = NbdDevice::new();

        // May fail if no NBD devices available, which is fine
        match result {
            Ok(nbd) => {
                assert!(!nbd.is_connected());
                // Device index is claimed only in connect()
                assert!(nbd.device_path().as_os_str().is_empty());
            }
            Err(e) => {
                // Expected if NBD module not loaded
                eprintln!("NBD device creation failed (expected): {}", e);
            }
        }
    }

    #[test]
    fn test_nbd_alloc_lock_serializes() {
        let (ready_tx, ready_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();

        let holder = thread::spawn(move || {
            let lock = NbdAllocLock::acquire().expect("holder acquire");
            ready_tx.send(()).unwrap();
            // Hold long enough that the waiter must block
            thread::sleep(Duration::from_millis(250));
            drop(lock);
            done_tx.send(()).unwrap();
        });

        ready_rx.recv().expect("holder ready");
        let start = Instant::now();
        let _lock = NbdAllocLock::acquire().expect("waiter acquire");
        let waited = start.elapsed();
        // Must have blocked until the holder released (~250ms)
        assert!(
            waited >= Duration::from_millis(150),
            "expected flock to block ~250ms, waited {waited:?}"
        );
        let _ = done_rx.recv_timeout(Duration::from_secs(2));
        holder.join().unwrap();
    }

    /// Concurrent allocate+connect must never hand two workers the same `/dev/nbdN`
    /// (fluxvm#104). Needs a working `qemu-nbd` (self-hosted `nbd` runner); skipped
    /// on GitHub-hosted runners where NBD attach fails.
    #[test]
    #[ignore = "needs working qemu-nbd (run: scripts/test-nbd-concurrent.sh)"]
    fn test_concurrent_nbd_allocate_connect() {
        use std::collections::HashSet;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::{Arc, Mutex};

        let work = tempfile::tempdir().expect("tempdir");
        let image = work.path().join("blank.raw");
        let status = Command::new("qemu-img")
            .args(["create", "-f", "raw"])
            .arg(&image)
            .arg("64M")
            .status()
            .expect("spawn qemu-img");
        assert!(status.success(), "qemu-img create failed: {status}");

        let concurrency = 4usize;
        let runs = 20usize;
        let next = Arc::new(AtomicUsize::new(0));
        let fail = Arc::new(AtomicUsize::new(0));
        let ok = Arc::new(AtomicUsize::new(0));
        let held = Arc::new(Mutex::new(HashSet::<String>::new()));
        let collisions = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..concurrency {
            let image = image.clone();
            let next = next.clone();
            let fail = fail.clone();
            let ok = ok.clone();
            let held = held.clone();
            let collisions = collisions.clone();
            handles.push(thread::spawn(move || {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    if i >= runs {
                        break;
                    }
                    let res = (|| -> std::result::Result<(), String> {
                        let mut nbd = NbdDevice::new().map_err(|e| e.to_string())?;
                        nbd.connect(&image, true).map_err(|e| e.to_string())?;
                        let path = nbd.device_path().display().to_string();
                        {
                            let mut h = held.lock().unwrap();
                            if !h.insert(path.clone()) {
                                collisions.fetch_add(1, Ordering::SeqCst);
                                return Err(format!("COLLISION: {path} already held"));
                            }
                        }
                        thread::sleep(Duration::from_millis(100));
                        held.lock().unwrap().remove(&path);
                        drop(nbd);
                        Ok(())
                    })();
                    match res {
                        Ok(()) => {
                            ok.fetch_add(1, Ordering::SeqCst);
                        }
                        Err(e) => {
                            eprintln!("FAIL {i}: {e}");
                            fail.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                }
            }));
        }
        for h in handles {
            h.join().expect("worker panicked");
        }

        let ok_n = ok.load(Ordering::SeqCst);
        let fail_n = fail.load(Ordering::SeqCst);
        let col_n = collisions.load(Ordering::SeqCst);
        assert_eq!(fail_n, 0, "{fail_n} concurrent NBD connects failed");
        assert_eq!(col_n, 0, "{col_n} device collisions (fluxvm#104 race)");
        assert_eq!(ok_n, runs, "expected {runs} ok, got {ok_n}");
    }
}
