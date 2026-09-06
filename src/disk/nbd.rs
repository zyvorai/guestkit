// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! NBD (Network Block Device) support using qemu-nbd
//!
//! This module provides NBD device management for mounting disk images
//! as block devices. This allows filesystem access without implementing
//! full filesystem parsers.

use crate::core::{DiskFormat, Error, Result};
use crate::disk::reader::DiskReader;
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

/// NBD device manager
pub struct NbdDevice {
    /// NBD device path (e.g., /dev/nbd0)
    device_path: PathBuf,
    /// Image path being exported
    image_path: PathBuf,
    /// Whether device is connected
    connected: bool,
    /// qemu-nbd process handle
    qemu_nbd_process: Option<Child>,
}

impl NbdDevice {
    /// Create a new NBD device manager
    ///
    /// This finds an available /dev/nbd* device
    pub fn new() -> Result<Self> {
        let device_path = Self::find_available_device()?;

        Ok(NbdDevice {
            device_path,
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

    /// Find an available NBD device
    ///
    /// KNOWN RACE (not fixed here — no NBD-capable test infra to verify a
    /// fix against): this checks a device's availability, then the caller
    /// connects to it, as two separate, unlocked steps — no flock or other
    /// cross-process coordination. Two processes on the same host racing
    /// this (e.g. two independent API requests each mounting a different
    /// disk image at the same moment) can both pick the same device index;
    /// one side's connect then fails outright rather than retrying a
    /// different device. Seen in practice: `deploy/scripts/e2e-smoke.sh`
    /// hit this via an unawaited async migrate-plan job racing a
    /// synchronous provision call against the same image — worked around
    /// there by serializing the two calls, not fixed at the source.
    fn find_available_device() -> Result<PathBuf> {
        // First, check if NBD module is loaded
        if !Self::is_nbd_module_loaded() {
            eprintln!("NBD kernel module not loaded. Attempting to load...");
            Self::load_nbd_module()?;
            eprintln!("NBD module loaded successfully.");
        }

        // Try /dev/nbd0 through /dev/nbd15
        for i in 0..16 {
            let device = PathBuf::from(format!("/dev/nbd{}", i));
            if device.exists() {
                // Check if device is actually connected
                if !Self::is_nbd_device_in_use(&device) {
                    return Ok(device);
                }
            }
        }

        Err(Error::NotFound(
            "No available NBD devices found. All 16 NBD devices are in use. Try disconnecting unused devices with: for i in {0..15}; do sudo qemu-nbd --disconnect /dev/nbd$i; done".to_string()
        ))
    }

    /// Connect disk image to NBD device
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

        // Check if the device is already in use (stale connection from previous run)
        if Self::is_nbd_device_in_use(&self.device_path) {
            eprintln!(
                "Warning: NBD device {} is already in use. Attempting to disconnect...",
                self.device_path.display()
            );

            // Try to disconnect the stale connection
            let need_sudo = crate::guestfs::mount::need_sudo();
            let mut cmd = if need_sudo {
                let mut sudo_cmd = Command::new("sudo");
                sudo_cmd.arg("qemu-nbd");
                sudo_cmd
            } else {
                Command::new("qemu-nbd")
            };

            if let Err(e) = cmd.arg("--disconnect").arg(&self.device_path).output() {
                log::warn!(
                    "NBD disconnect failed for {}: {}",
                    self.device_path.display(),
                    e
                );
            }

            // Wait for disconnect to complete
            thread::sleep(Duration::from_millis(500));

            // Check again
            if Self::is_nbd_device_in_use(&self.device_path) {
                return Err(Error::InvalidState(format!(
                    "NBD device {} is still in use after disconnect attempt. \
                     Try manually: sudo qemu-nbd --disconnect {}",
                    self.device_path.display(),
                    self.device_path.display()
                )));
            }

            eprintln!("Successfully disconnected stale NBD connection.");
        }

        // Check if we need to use sudo (qemu-nbd --connect requires root)
        let need_sudo = crate::guestfs::mount::need_sudo();
        if is_debug_enabled() {
            eprintln!("[DEBUG NBD] need_sudo={}", need_sudo);
        }

        // Build qemu-nbd command
        let mut cmd = if need_sudo {
            let mut sudo_cmd = Command::new("sudo");
            sudo_cmd.arg("qemu-nbd");
            sudo_cmd
        } else {
            Command::new("qemu-nbd")
        };

        // Detect image format: try extension first, then fall back to qemu-img info
        let format = Self::detect_image_format(image_path);

        // Use short flags: -c instead of --connect, -f instead of --format
        // This is important! Long flags cause qemu-nbd to exit immediately
        cmd.arg("-c").arg(&self.device_path).arg("-f").arg(format);

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

        // Don't redirect stdio - qemu-nbd needs it to stay alive
        // cmd.stdin(Stdio::null())
        //     .stdout(Stdio::null())
        //     .stderr(Stdio::null());

        // Spawn the process and keep it alive
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

        // Check if process is still alive
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
                // Process exited - could be normal (daemonized) or an error
                // We'll check if device is connected below
                true
            }
            Ok(None) => {
                if is_debug_enabled() {
                    eprintln!("[DEBUG NBD] Process still running");
                }
                false
            }
            Err(e) => {
                return Err(Error::CommandFailed(format!(
                    "Failed to check qemu-nbd process status: {}",
                    e
                )));
            }
        };

        // Store the child process handle first (will be used or dropped)
        self.qemu_nbd_process = Some(child);

        // Wait for device to be ready - this verifies the connection actually worked
        if let Err(e) = self.wait_for_device() {
            // Device not ready - connection failed, cleanup the process
            if let Some(mut proc) = self.qemu_nbd_process.take() {
                let _ = proc.kill();
                let _ = proc.wait();
            }

            if process_exited {
                return Err(Error::CommandFailed(format!(
                    "qemu-nbd exited and device did not become ready. \
                     Device {} may already be in use or the image may be corrupted. \
                     Original error: {}. Try: sudo qemu-nbd --disconnect {}",
                    self.device_path.display(),
                    e,
                    self.device_path.display()
                )));
            } else {
                return Err(Error::CommandFailed(format!(
                    "Device did not become ready: {}. Try: sudo qemu-nbd --disconnect {}",
                    e,
                    self.device_path.display()
                )));
            }
        }

        // Device is ready - connection successful
        self.image_path = image_path.to_path_buf();
        self.connected = true;
        Ok(())
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

    #[test]
    fn test_nbd_device_creation() {
        // Just test that we can create the struct
        // Actual connection requires root and qemu-nbd
        let result = NbdDevice::new();

        // May fail if no NBD devices available, which is fine
        match result {
            Ok(nbd) => {
                assert!(!nbd.is_connected());
            }
            Err(e) => {
                // Expected if NBD module not loaded
                eprintln!("NBD device creation failed (expected): {}", e);
            }
        }
    }
}
