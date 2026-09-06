// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! PyO3 Python bindings for guestkit
//!
//! Build with: cargo build --release --features python-bindings

#[cfg(feature = "python-bindings")]
use pyo3::prelude::*;

#[cfg(feature = "python-bindings")]
use crate::converters::DiskConverter as RustDiskConverter;
#[cfg(feature = "python-bindings")]
use std::path::{Path, PathBuf};

/// Convert a guestkit error to an appropriate Python exception type
#[cfg(feature = "python-bindings")]
fn to_pyerr(err: crate::core::Error) -> PyErr {
    let msg = err.to_string();
    match &err {
        crate::core::Error::Io(_) => PyErr::new::<pyo3::exceptions::PyIOError, _>(msg),
        crate::core::Error::NotFound(_) => {
            PyErr::new::<pyo3::exceptions::PyFileNotFoundError, _>(msg)
        }
        crate::core::Error::PermissionDenied(_) => {
            PyErr::new::<pyo3::exceptions::PyPermissionError, _>(msg)
        }
        crate::core::Error::InvalidFormat(_) | crate::core::Error::InputValidation(_) => {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(msg)
        }
        crate::core::Error::Unsupported(_) => {
            PyErr::new::<pyo3::exceptions::PyNotImplementedError, _>(msg)
        }
        _ => PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(msg),
    }
}

/// Convert any Display error to a PyRuntimeError (for non-guestkit errors)
#[cfg(feature = "python-bindings")]
fn to_pyerr_generic(err: impl std::fmt::Display) -> PyErr {
    PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(err.to_string())
}

/// Python wrapper for disk conversion
#[cfg(feature = "python-bindings")]
#[pyclass]
struct DiskConverter {
    converter: RustDiskConverter,
}

#[cfg(feature = "python-bindings")]
#[pymethods]
impl DiskConverter {
    #[new]
    fn new() -> Self {
        Self {
            converter: RustDiskConverter::new(),
        }
    }

    /// Convert disk image format
    ///
    /// # Arguments
    ///
    /// * `source` - Source disk image path
    /// * `output` - Output disk image path
    /// * `format` - Output format (qcow2, raw, vmdk, vdi)
    /// * `compress` - Enable compression (default: false)
    /// * `flatten` - Flatten snapshot chains (default: true)
    ///
    /// # Returns
    ///
    /// Dictionary with conversion results
    ///
    /// # Examples
    ///
    /// ```python
    /// from guestkit import DiskConverter
    ///
    /// converter = DiskConverter()
    /// result = converter.convert(
    ///     "/path/to/source.vmdk",
    ///     "/path/to/output.qcow2",
    ///     "qcow2",
    ///     compress=True
    /// )
    ///
    /// if result["success"]:
    ///     print(f"Converted: {result['output_size']} bytes")
    /// ```
    #[pyo3(signature = (source, output, format="qcow2", compress=false, flatten=true))]
    fn convert(
        &self,
        source: String,
        output: String,
        format: &str,
        compress: bool,
        flatten: bool,
    ) -> PyResult<Py<PyAny>> {
        let result = self
            .converter
            .convert(
                Path::new(&source),
                Path::new(&output),
                format,
                compress,
                flatten,
            )
            .map_err(to_pyerr)?;

        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("source_path", result.source_path.to_str())?;
            dict.set_item("output_path", result.output_path.to_str())?;
            dict.set_item("source_format", result.source_format.as_str())?;
            dict.set_item("output_format", result.output_format.as_str())?;
            dict.set_item("output_size", result.output_size)?;
            dict.set_item("duration_secs", result.duration_secs)?;
            dict.set_item("success", result.success)?;
            dict.set_item("error", result.error)?;
            Ok(dict.into())
        })
    }

    /// Detect disk image format
    ///
    /// # Arguments
    ///
    /// * `image` - Disk image path
    ///
    /// # Returns
    ///
    /// Format string (qcow2, raw, vmdk, etc.)
    fn detect_format(&self, image: String) -> PyResult<String> {
        let format = self
            .converter
            .detect_format(Path::new(&image))
            .map_err(to_pyerr)?;

        Ok(format.as_str().to_string())
    }

    /// Get disk image information
    ///
    /// # Arguments
    ///
    /// * `image` - Disk image path
    ///
    /// # Returns
    ///
    /// Dictionary with disk image metadata
    fn get_info(&self, image: String) -> PyResult<Py<PyAny>> {
        let info = self
            .converter
            .get_info(Path::new(&image))
            .map_err(to_pyerr)?;

        Python::attach(|py| {
            let json_str = serde_json::to_string(&info).map_err(to_pyerr_generic)?;

            let json_module = py.import("json")?;
            let loads = json_module.getattr("loads")?;
            let result = loads.call1((json_str,))?;
            Ok(result.into())
        })
    }
}

/// Python wrapper for Guestfs handle
#[cfg(feature = "python-bindings")]
#[pyclass]
struct Guestfs {
    handle: crate::guestfs::Guestfs,
}

#[cfg(feature = "python-bindings")]
#[pymethods]
impl Guestfs {
    /// Create a new Guestfs handle
    ///
    /// # Examples
    ///
    /// ```python
    /// from guestkit import Guestfs
    ///
    /// g = Guestfs()
    /// g.add_drive_ro("/path/to/disk.qcow2")
    /// g.launch()
    /// roots = g.inspect_os()
    /// for root in roots:
    ///     print(f"Found OS: {g.inspect_get_distro(root)}")
    /// g.shutdown()
    /// ```
    #[new]
    fn new() -> PyResult<Self> {
        let handle = crate::guestfs::Guestfs::new().map_err(to_pyerr)?;

        Ok(Self { handle })
    }

    /// Add a disk image (read-only)
    ///
    /// # Arguments
    ///
    /// * `filename` - Path to disk image
    fn add_drive_ro(&mut self, filename: String) -> PyResult<()> {
        self.handle
            .add_drive_ro(&filename)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Add a disk image (read-write)
    ///
    /// # Arguments
    ///
    /// * `filename` - Path to disk image
    fn add_drive(&mut self, filename: String) -> PyResult<()> {
        self.handle
            .add_drive(&filename)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Launch the backend (analyze disk)
    fn launch(&mut self) -> PyResult<()> {
        self.handle
            .launch()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Shutdown the backend
    fn shutdown(&mut self) -> PyResult<()> {
        self.handle
            .shutdown()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Enable/disable verbose output
    ///
    /// # Arguments
    ///
    /// * `verbose` - Enable verbose mode
    fn set_verbose(&mut self, verbose: bool) {
        self.handle.set_verbose(verbose);
    }

    // === Inspection API ===

    /// Inspect operating systems in the disk image
    ///
    /// # Returns
    ///
    /// List of root devices (e.g., ["/dev/sda1"])
    fn inspect_os(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .inspect_os()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get OS type
    ///
    /// # Arguments
    ///
    /// * `root` - Root device from inspect_os()
    ///
    /// # Returns
    ///
    /// OS type (e.g., "linux", "windows")
    fn inspect_get_type(&mut self, root: String) -> PyResult<String> {
        self.handle
            .inspect_get_type(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get distribution name
    ///
    /// # Arguments
    ///
    /// * `root` - Root device from inspect_os()
    ///
    /// # Returns
    ///
    /// Distribution name (e.g., "fedora", "ubuntu")
    fn inspect_get_distro(&mut self, root: String) -> PyResult<String> {
        self.handle
            .inspect_get_distro(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get major version number
    ///
    /// # Arguments
    ///
    /// * `root` - Root device from inspect_os()
    fn inspect_get_major_version(&mut self, root: String) -> PyResult<i32> {
        self.handle
            .inspect_get_major_version(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get minor version number
    ///
    /// # Arguments
    ///
    /// * `root` - Root device from inspect_os()
    fn inspect_get_minor_version(&mut self, root: String) -> PyResult<i32> {
        self.handle
            .inspect_get_minor_version(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get hostname
    ///
    /// # Arguments
    ///
    /// * `root` - Root device from inspect_os()
    fn inspect_get_hostname(&mut self, root: String) -> PyResult<String> {
        self.handle
            .inspect_get_hostname(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get architecture
    ///
    /// # Arguments
    ///
    /// * `root` - Root device from inspect_os()
    fn inspect_get_arch(&mut self, root: String) -> PyResult<String> {
        self.handle
            .inspect_get_arch(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get product name
    ///
    /// # Arguments
    ///
    /// * `root` - Root device from inspect_os()
    fn inspect_get_product_name(&mut self, root: String) -> PyResult<String> {
        self.handle
            .inspect_get_product_name(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get package format
    ///
    /// # Arguments
    ///
    /// * `root` - Root device from inspect_os()
    ///
    /// # Returns
    ///
    /// Package format (e.g., "rpm", "deb")
    fn inspect_get_package_format(&mut self, root: String) -> PyResult<String> {
        self.handle
            .inspect_get_package_format(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get package management tool
    ///
    /// # Arguments
    ///
    /// * `root` - Root device from inspect_os()
    fn inspect_get_package_management(&mut self, root: String) -> PyResult<String> {
        self.handle
            .inspect_get_package_management(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get mountpoints
    ///
    /// # Arguments
    ///
    /// * `root` - Root device from inspect_os()
    ///
    /// # Returns
    ///
    /// Dictionary of mountpoint -> device mappings
    fn inspect_get_mountpoints(&mut self, root: String) -> PyResult<Py<PyAny>> {
        let mountpoints = self
            .handle
            .inspect_get_mountpoints(&root)
            .map_err(to_pyerr)?;

        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            for (mountpoint, device) in mountpoints {
                dict.set_item(mountpoint, device)?;
            }
            Ok(dict.into())
        })
    }

    // === Device Operations ===

    /// List all devices
    ///
    /// # Returns
    ///
    /// List of device names (e.g., ["/dev/sda", "/dev/sdb"])
    fn list_devices(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .list_devices()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List all partitions
    ///
    /// # Returns
    ///
    /// List of partition names (e.g., ["/dev/sda1", "/dev/sda2"])
    fn list_partitions(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .list_partitions()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get device size
    ///
    /// # Arguments
    ///
    /// * `device` - Device name
    ///
    /// # Returns
    ///
    /// Size in bytes
    fn blockdev_getsize64(&mut self, device: String) -> PyResult<i64> {
        self.handle
            .blockdev_getsize64(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Filesystem Operations ===

    /// Get filesystem type
    ///
    /// # Arguments
    ///
    /// * `device` - Device name
    fn vfs_type(&mut self, device: String) -> PyResult<String> {
        self.handle
            .vfs_type(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get filesystem label
    ///
    /// # Arguments
    ///
    /// * `device` - Device name
    fn vfs_label(&mut self, device: String) -> PyResult<String> {
        self.handle
            .vfs_label(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get filesystem UUID
    ///
    /// # Arguments
    ///
    /// * `device` - Device name
    fn vfs_uuid(&mut self, device: String) -> PyResult<String> {
        self.handle
            .vfs_uuid(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Mount filesystem
    ///
    /// # Arguments
    ///
    /// * `device` - Device to mount
    /// * `mountpoint` - Mount point path
    fn mount(&mut self, device: String, mountpoint: String) -> PyResult<()> {
        self.handle
            .mount(&device, &mountpoint)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Mount filesystem read-only
    ///
    /// # Arguments
    ///
    /// * `device` - Device to mount
    /// * `mountpoint` - Mount point path
    fn mount_ro(&mut self, device: String, mountpoint: String) -> PyResult<()> {
        self.handle
            .mount_ro(&device, &mountpoint)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Unmount filesystem
    ///
    /// # Arguments
    ///
    /// * `mountpoint` - Mount point path
    fn umount(&mut self, mountpoint: String) -> PyResult<()> {
        self.handle
            .umount(&mountpoint)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === File Operations ===

    /// Read file contents
    ///
    /// # Arguments
    ///
    /// * `path` - File path in guest
    ///
    /// # Returns
    ///
    /// File contents as bytes
    fn read_file(&mut self, path: String) -> PyResult<Vec<u8>> {
        self.handle
            .read_file(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Write file contents
    ///
    /// # Arguments
    ///
    /// * `path` - File path in guest
    /// * `content` - Content to write
    fn write(&mut self, path: String, content: Vec<u8>) -> PyResult<()> {
        self.handle
            .write(&path, &content)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Check if path exists
    ///
    /// # Arguments
    ///
    /// * `path` - Path in guest
    fn exists(&mut self, path: String) -> PyResult<bool> {
        self.handle
            .exists(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Check if path is a file
    ///
    /// # Arguments
    ///
    /// * `path` - Path in guest
    fn is_file(&mut self, path: String) -> PyResult<bool> {
        self.handle
            .is_file(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Check if path is a directory
    ///
    /// # Arguments
    ///
    /// * `path` - Path in guest
    fn is_dir(&mut self, path: String) -> PyResult<bool> {
        self.handle
            .is_dir(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List directory contents
    ///
    /// # Arguments
    ///
    /// * `directory` - Directory path in guest
    fn ls(&mut self, directory: String) -> PyResult<Vec<String>> {
        self.handle
            .ls(&directory)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Download file from guest
    ///
    /// # Arguments
    ///
    /// * `remotefilename` - File path in guest
    /// * `filename` - Local file path
    fn download(&mut self, remotefilename: String, filename: String) -> PyResult<()> {
        self.handle
            .download(&remotefilename, &filename)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Upload file to guest
    ///
    /// # Arguments
    ///
    /// * `filename` - Local file path
    /// * `remotefilename` - File path in guest
    fn upload(&mut self, filename: String, remotefilename: String) -> PyResult<()> {
        self.handle
            .upload(&filename, &remotefilename)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Package Management ===

    /// List installed packages
    ///
    /// # Arguments
    ///
    /// * `root` - Root device from inspect_os()
    ///
    /// # Returns
    ///
    /// List of installed packages
    fn inspect_list_applications(&mut self, root: String) -> PyResult<Py<PyAny>> {
        let apps = self
            .handle
            .inspect_list_applications(&root)
            .map_err(to_pyerr)?;

        Python::attach(|py| {
            let list = pyo3::types::PyList::empty(py);
            for app in apps {
                let dict = pyo3::types::PyDict::new(py);
                dict.set_item("app_name", &app.name)?;
                dict.set_item("app_display_name", &app.display_name)?;
                dict.set_item("app_epoch", app.epoch)?;
                dict.set_item("app_version", &app.version)?;
                dict.set_item("app_release", &app.release)?;
                dict.set_item("app_install_path", &app.install_path)?;
                dict.set_item("app_publisher", &app.publisher)?;
                dict.set_item("app_url", &app.url)?;
                dict.set_item("app_description", &app.description)?;
                list.append(dict)?;
            }
            Ok(list.into())
        })
    }

    // === Command Execution ===

    /// Execute a command in the guest
    ///
    /// # Arguments
    ///
    /// * `arguments` - List of command arguments (first is command name)
    ///
    /// # Returns
    ///
    /// Command output as string
    fn command(&mut self, arguments: Vec<String>) -> PyResult<String> {
        let args: Vec<&str> = arguments.iter().map(|s| s.as_str()).collect();
        self.handle
            .command(&args)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Execute shell command lines
    ///
    /// # Arguments
    ///
    /// * `command` - Shell command string
    ///
    /// # Returns
    ///
    /// List of output lines
    fn sh_lines(&mut self, command: String) -> PyResult<Vec<String>> {
        self.handle
            .sh_lines(&command)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Execute shell command
    ///
    /// # Arguments
    ///
    /// * `command` - Shell command string
    ///
    /// # Returns
    ///
    /// Command output as string
    fn sh(&mut self, command: String) -> PyResult<String> {
        self.handle
            .sh(&command)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === LVM Operations ===

    /// Scan for LVM volume groups
    fn vgscan(&mut self) -> PyResult<()> {
        self.handle
            .vgscan()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List LVM volume groups
    ///
    /// # Returns
    ///
    /// List of volume group names
    fn vgs(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .vgs()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List LVM physical volumes
    ///
    /// # Returns
    ///
    /// List of physical volume names
    fn pvs(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .pvs()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List LVM logical volumes
    ///
    /// # Returns
    ///
    /// List of logical volume names
    fn lvs(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .lvs()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Archive Operations ===

    /// Extract tar archive into guest directory
    ///
    /// # Arguments
    ///
    /// * `tarfile` - Path to tar file on host
    /// * `directory` - Directory in guest to extract to
    fn tar_in(&mut self, tarfile: String, directory: String) -> PyResult<()> {
        self.handle
            .tar_in(&tarfile, &directory)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Create tar archive from guest directory
    ///
    /// # Arguments
    ///
    /// * `directory` - Directory in guest to archive
    /// * `tarfile` - Path to tar file on host
    fn tar_out(&mut self, directory: String, tarfile: String) -> PyResult<()> {
        self.handle
            .tar_out(&directory, &tarfile)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Extract compressed tar archive into guest directory
    ///
    /// # Arguments
    ///
    /// * `tarfile` - Path to tar.gz file on host
    /// * `directory` - Directory in guest to extract to
    fn tgz_in(&mut self, tarfile: String, directory: String) -> PyResult<()> {
        self.handle
            .tgz_in(&tarfile, &directory)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Create compressed tar archive from guest directory
    ///
    /// # Arguments
    ///
    /// * `directory` - Directory in guest to archive
    /// * `tarfile` - Path to tar.gz file on host
    fn tgz_out(&mut self, directory: String, tarfile: String) -> PyResult<()> {
        self.handle
            .tgz_out(&directory, &tarfile)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Additional File Operations ===

    /// Read entire file as string
    ///
    /// # Arguments
    ///
    /// * `path` - File path in guest
    ///
    /// # Returns
    ///
    /// File contents as string
    fn cat(&mut self, path: String) -> PyResult<String> {
        self.handle
            .cat(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Create directory
    ///
    /// # Arguments
    ///
    /// * `path` - Directory path in guest
    fn mkdir(&mut self, path: String) -> PyResult<()> {
        self.handle
            .mkdir(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Create directory with parents
    ///
    /// # Arguments
    ///
    /// * `path` - Directory path in guest
    fn mkdir_p(&mut self, path: String) -> PyResult<()> {
        self.handle
            .mkdir_p(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Remove file
    ///
    /// # Arguments
    ///
    /// * `path` - File path in guest
    fn rm(&mut self, path: String) -> PyResult<()> {
        self.handle
            .rm(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Remove directory
    ///
    /// # Arguments
    ///
    /// * `path` - Directory path in guest
    fn rmdir(&mut self, path: String) -> PyResult<()> {
        self.handle
            .rmdir(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Remove directory recursively
    ///
    /// # Arguments
    ///
    /// * `path` - Directory path in guest
    fn rm_rf(&mut self, path: String) -> PyResult<()> {
        self.handle
            .rm_rf(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Change file permissions
    ///
    /// # Arguments
    ///
    /// * `mode` - Permission mode (octal)
    /// * `path` - File path in guest
    fn chmod(&mut self, mode: i32, path: String) -> PyResult<()> {
        self.handle
            .chmod(mode, &path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Change file owner
    ///
    /// # Arguments
    ///
    /// * `owner` - New owner UID
    /// * `group` - New group GID
    /// * `path` - File path in guest
    fn chown(&mut self, owner: i32, group: i32, path: String) -> PyResult<()> {
        self.handle
            .chown(owner, group, &path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get file stat information
    ///
    /// # Arguments
    ///
    /// * `path` - File path in guest
    ///
    /// # Returns
    ///
    /// Dictionary with stat information
    fn stat(&mut self, path: String) -> PyResult<Py<PyAny>> {
        let stat = self.handle.stat(&path).map_err(to_pyerr)?;

        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("dev", stat.dev)?;
            dict.set_item("ino", stat.ino)?;
            dict.set_item("mode", stat.mode)?;
            dict.set_item("nlink", stat.nlink)?;
            dict.set_item("uid", stat.uid)?;
            dict.set_item("gid", stat.gid)?;
            dict.set_item("rdev", stat.rdev)?;
            dict.set_item("size", stat.size)?;
            dict.set_item("blksize", stat.blksize)?;
            dict.set_item("blocks", stat.blocks)?;
            dict.set_item("atime", stat.atime)?;
            dict.set_item("mtime", stat.mtime)?;
            dict.set_item("ctime", stat.ctime)?;
            Ok(dict.into())
        })
    }

    /// Get filesystem statistics
    ///
    /// # Arguments
    ///
    /// * `path` - Path in guest filesystem
    ///
    /// # Returns
    ///
    /// Dictionary with filesystem statistics
    fn statvfs(&mut self, path: String) -> PyResult<Py<PyAny>> {
        let statvfs = self.handle.statvfs(&path).map_err(to_pyerr)?;

        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            for (key, value) in statvfs {
                dict.set_item(key, value)?;
            }
            Ok(dict.into())
        })
    }

    // === Checksum Operations ===

    /// Calculate file checksum
    ///
    /// # Arguments
    ///
    /// * `csumtype` - Checksum type (md5, sha1, sha256, etc.)
    /// * `path` - File path in guest
    ///
    /// # Returns
    ///
    /// Checksum as hex string
    fn checksum(&mut self, csumtype: String, path: String) -> PyResult<String> {
        self.handle
            .checksum(&csumtype, &path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Unmount Operations ===

    /// Unmount all filesystems
    fn umount_all(&mut self) -> PyResult<()> {
        self.handle
            .umount_all()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Synchronize filesystem
    fn sync(&mut self) -> PyResult<()> {
        self.handle
            .sync()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Inspection Extended ===

    /// Get image format
    fn inspect_get_format(&mut self, root: String) -> PyResult<String> {
        self.handle
            .inspect_get_format(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get init system type from inspection
    fn inspect_get_init_system(&mut self, root: String) -> PyResult<String> {
        self.handle
            .inspect_get_init_system(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get product variant
    fn inspect_get_product_variant(&mut self, root: String) -> PyResult<String> {
        self.handle
            .inspect_get_product_variant(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get osinfo ID
    fn inspect_get_osinfo_id(&mut self, root: String) -> PyResult<String> {
        self.handle
            .inspect_get_osinfo_id(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Check if disk is multipart
    fn inspect_is_multipart(&mut self, root: String) -> PyResult<bool> {
        self.handle
            .inspect_is_multipart(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Check if disk is a netinst image
    fn inspect_is_netinst(&mut self, root: String) -> PyResult<bool> {
        self.handle
            .inspect_is_netinst(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List applications (version 2, returns tuples of name, version, release)
    fn inspect_list_applications2(&mut self, root: String) -> PyResult<Py<PyAny>> {
        let apps = self
            .handle
            .inspect_list_applications2(&root)
            .map_err(to_pyerr)?;
        Python::attach(|py| {
            let list = pyo3::types::PyList::empty(py);
            for (name, version, release) in apps {
                let tuple = pyo3::types::PyTuple::new(py, &[name, version, release])?;
                list.append(tuple)?;
            }
            Ok(list.into())
        })
    }

    /// Get OS icon data
    fn inspect_get_os_icon(&mut self, root: String) -> PyResult<Vec<u8>> {
        self.handle
            .inspect_get_os_icon(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get Windows current control set key
    fn inspect_get_windows_current_control_set_key(&mut self, root: String) -> PyResult<String> {
        self.handle
            .inspect_get_windows_current_control_set_key(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Check if disk is a live image
    fn inspect_is_live(&mut self, root: String) -> PyResult<bool> {
        self.handle
            .inspect_is_live(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Enhanced Inspection ===

    /// Inspect boot configuration
    fn inspect_boot_config(&mut self, root: String) -> PyResult<Py<PyAny>> {
        let config = self.handle.inspect_boot_config(&root).map_err(to_pyerr)?;
        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("bootloader", config.bootloader)?;
            dict.set_item("default_entry", config.default_entry)?;
            dict.set_item("timeout", config.timeout)?;
            dict.set_item("kernel_cmdline", config.kernel_cmdline)?;
            Ok(dict.into())
        })
    }

    /// Inspect SSL/TLS certificates
    fn inspect_certificates(&mut self, root: String) -> PyResult<Py<PyAny>> {
        let certs = self.handle.inspect_certificates(&root).map_err(to_pyerr)?;
        Python::attach(|py| {
            let list = pyo3::types::PyList::empty(py);
            for cert in certs {
                let dict = pyo3::types::PyDict::new(py);
                dict.set_item("path", cert.path)?;
                dict.set_item("subject", cert.subject)?;
                dict.set_item("issuer", cert.issuer)?;
                dict.set_item("expiry", cert.expiry)?;
                list.append(dict)?;
            }
            Ok(list.into())
        })
    }

    /// Check if cloud-init is present
    fn inspect_cloud_init(&mut self, root: String) -> PyResult<bool> {
        self.handle
            .inspect_cloud_init(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Inspect container runtimes
    fn inspect_container_runtimes(&mut self, root: String) -> PyResult<Vec<String>> {
        self.handle
            .inspect_container_runtimes(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Inspect cron jobs
    fn inspect_cron(&mut self, root: String) -> PyResult<Vec<String>> {
        self.handle
            .inspect_cron(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Inspect databases
    fn inspect_databases(&mut self, root: String) -> PyResult<Py<PyAny>> {
        let dbs = self.handle.inspect_databases(&root).map_err(to_pyerr)?;
        Python::attach(|py| {
            let list = pyo3::types::PyList::empty(py);
            for db in dbs {
                let dict = pyo3::types::PyDict::new(py);
                dict.set_item("name", db.name)?;
                dict.set_item("data_dir", db.data_dir)?;
                dict.set_item("config_path", db.config_path)?;
                list.append(dict)?;
            }
            Ok(list.into())
        })
    }

    /// Inspect DNS configuration
    fn inspect_dns(&mut self, root: String) -> PyResult<Vec<String>> {
        self.handle
            .inspect_dns(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Inspect firewall configuration
    fn inspect_firewall(&mut self, root: String) -> PyResult<Py<PyAny>> {
        let fw = self.handle.inspect_firewall(&root).map_err(to_pyerr)?;
        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("firewall_type", fw.firewall_type)?;
            dict.set_item("enabled", fw.enabled)?;
            dict.set_item("rules_count", fw.rules_count)?;
            dict.set_item("zones", fw.zones)?;
            Ok(dict.into())
        })
    }

    /// Inspect fstab entries (returns list of [device, mountpoint, fstype])
    fn inspect_fstab(&mut self, root: String) -> PyResult<Py<PyAny>> {
        let entries = self.handle.inspect_fstab(&root).map_err(to_pyerr)?;
        Python::attach(|py| {
            let list = pyo3::types::PyList::empty(py);
            for (dev, mp, fs) in entries {
                let tuple = pyo3::types::PyTuple::new(py, &[dev, mp, fs])?;
                list.append(tuple)?;
            }
            Ok(list.into())
        })
    }

    /// Inspect /etc/hosts entries
    fn inspect_hosts(&mut self, root: String) -> PyResult<Py<PyAny>> {
        let entries = self.handle.inspect_hosts(&root).map_err(to_pyerr)?;
        Python::attach(|py| {
            let list = pyo3::types::PyList::empty(py);
            for entry in entries {
                let dict = pyo3::types::PyDict::new(py);
                dict.set_item("ip", entry.ip)?;
                dict.set_item("hostnames", entry.hostnames)?;
                list.append(dict)?;
            }
            Ok(list.into())
        })
    }

    /// Inspect init system
    fn inspect_init_system(&mut self, root: String) -> PyResult<String> {
        self.handle
            .inspect_init_system(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Inspect kernel modules
    fn inspect_kernel_modules(&mut self, root: String) -> PyResult<Vec<String>> {
        self.handle
            .inspect_kernel_modules(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Inspect kernel parameters
    fn inspect_kernel_params(&mut self, root: String) -> PyResult<Py<PyAny>> {
        let params = self.handle.inspect_kernel_params(&root).map_err(to_pyerr)?;
        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            for (k, v) in params {
                dict.set_item(k, v)?;
            }
            Ok(dict.into())
        })
    }

    /// Inspect locale
    fn inspect_locale(&mut self, root: String) -> PyResult<String> {
        self.handle
            .inspect_locale(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Inspect LVM configuration
    fn inspect_lvm(&mut self, root: String) -> PyResult<Py<PyAny>> {
        let info = self.handle.inspect_lvm(&root).map_err(to_pyerr)?;
        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("physical_volumes", &info.physical_volumes)?;
            let vgs = pyo3::types::PyList::empty(py);
            for vg in &info.volume_groups {
                let d = pyo3::types::PyDict::new(py);
                d.set_item("name", &vg.name)?;
                d.set_item("pv_count", vg.pv_count)?;
                d.set_item("lv_count", vg.lv_count)?;
                d.set_item("size", &vg.size)?;
                vgs.append(d)?;
            }
            dict.set_item("volume_groups", vgs)?;
            let lvs = pyo3::types::PyList::empty(py);
            for lv in &info.logical_volumes {
                let d = pyo3::types::PyDict::new(py);
                d.set_item("name", &lv.name)?;
                d.set_item("vg_name", &lv.vg_name)?;
                d.set_item("size", &lv.size)?;
                d.set_item("path", &lv.path)?;
                lvs.append(d)?;
            }
            dict.set_item("logical_volumes", lvs)?;
            Ok(dict.into())
        })
    }

    /// Inspect network interfaces
    fn inspect_network(&mut self, root: String) -> PyResult<Py<PyAny>> {
        let ifaces = self.handle.inspect_network(&root).map_err(to_pyerr)?;
        Python::attach(|py| {
            let list = pyo3::types::PyList::empty(py);
            for iface in ifaces {
                let dict = pyo3::types::PyDict::new(py);
                dict.set_item("name", iface.name)?;
                dict.set_item("ip_address", iface.ip_address)?;
                dict.set_item("mac_address", iface.mac_address)?;
                dict.set_item("dhcp", iface.dhcp)?;
                dict.set_item("dns_servers", iface.dns_servers)?;
                list.append(dict)?;
            }
            Ok(list.into())
        })
    }

    /// Inspect packages
    fn inspect_packages(&mut self, root: String) -> PyResult<Py<PyAny>> {
        let info = self.handle.inspect_packages(&root).map_err(to_pyerr)?;
        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("manager", &info.manager)?;
            dict.set_item("package_count", info.package_count)?;
            let pkgs = pyo3::types::PyList::empty(py);
            for pkg in &info.packages {
                let d = pyo3::types::PyDict::new(py);
                d.set_item("name", &pkg.name)?;
                d.set_item("version", &pkg.version)?;
                d.set_item("manager", &pkg.manager)?;
                pkgs.append(d)?;
            }
            dict.set_item("packages", pkgs)?;
            Ok(dict.into())
        })
    }

    /// Inspect RAID arrays
    fn inspect_raid(&mut self, root: String) -> PyResult<Py<PyAny>> {
        let arrays = self.handle.inspect_raid(&root).map_err(to_pyerr)?;
        Python::attach(|py| {
            let list = pyo3::types::PyList::empty(py);
            for arr in arrays {
                let dict = pyo3::types::PyDict::new(py);
                dict.set_item("device", arr.device)?;
                dict.set_item("level", arr.level)?;
                dict.set_item("status", arr.status)?;
                dict.set_item("devices", arr.devices)?;
                dict.set_item("active_devices", arr.active_devices)?;
                dict.set_item("total_devices", arr.total_devices)?;
                list.append(dict)?;
            }
            Ok(list.into())
        })
    }

    /// Inspect runtimes (Python, Ruby, Node, etc.)
    fn inspect_runtimes(&mut self, root: String) -> PyResult<Py<PyAny>> {
        let runtimes = self.handle.inspect_runtimes(&root).map_err(to_pyerr)?;
        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            for (k, v) in runtimes {
                dict.set_item(k, v)?;
            }
            Ok(dict.into())
        })
    }

    /// Inspect security configuration
    fn inspect_security(&mut self, root: String) -> PyResult<Py<PyAny>> {
        let info = self.handle.inspect_security(&root).map_err(to_pyerr)?;
        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("selinux", info.selinux)?;
            dict.set_item("apparmor", info.apparmor)?;
            dict.set_item("fail2ban", info.fail2ban)?;
            dict.set_item("aide", info.aide)?;
            dict.set_item("auditd", info.auditd)?;
            let keys = pyo3::types::PyList::empty(py);
            for (path, count) in info.ssh_keys {
                let t = pyo3::types::PyTuple::new(py, &[path, count.to_string()])?;
                keys.append(t)?;
            }
            dict.set_item("ssh_keys", keys)?;
            Ok(dict.into())
        })
    }

    /// Inspect SELinux status
    fn inspect_selinux(&mut self, root: String) -> PyResult<String> {
        self.handle
            .inspect_selinux(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Inspect SSH configuration
    fn inspect_ssh_config(&mut self, root: String) -> PyResult<Py<PyAny>> {
        let config = self.handle.inspect_ssh_config(&root).map_err(to_pyerr)?;
        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            for (k, v) in config {
                dict.set_item(k, v)?;
            }
            Ok(dict.into())
        })
    }

    /// Inspect swap partitions
    fn inspect_swap(&mut self, root: String) -> PyResult<Vec<String>> {
        self.handle
            .inspect_swap(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Inspect systemd services
    fn inspect_systemd_services(&mut self, root: String) -> PyResult<Py<PyAny>> {
        let svcs = self
            .handle
            .inspect_systemd_services(&root)
            .map_err(to_pyerr)?;
        Python::attach(|py| {
            let list = pyo3::types::PyList::empty(py);
            for svc in svcs {
                let dict = pyo3::types::PyDict::new(py);
                dict.set_item("name", svc.name)?;
                dict.set_item("enabled", svc.enabled)?;
                dict.set_item("state", svc.state)?;
                list.append(dict)?;
            }
            Ok(list.into())
        })
    }

    /// Inspect systemd timers
    fn inspect_systemd_timers(&mut self, root: String) -> PyResult<Vec<String>> {
        self.handle
            .inspect_systemd_timers(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Inspect timezone
    fn inspect_timezone(&mut self, root: String) -> PyResult<String> {
        self.handle
            .inspect_timezone(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Inspect user accounts
    fn inspect_users(&mut self, root: String) -> PyResult<Py<PyAny>> {
        let users = self.handle.inspect_users(&root).map_err(to_pyerr)?;
        Python::attach(|py| {
            let list = pyo3::types::PyList::empty(py);
            for user in users {
                let dict = pyo3::types::PyDict::new(py);
                dict.set_item("username", user.username)?;
                dict.set_item("uid", user.uid)?;
                dict.set_item("gid", user.gid)?;
                dict.set_item("home", user.home)?;
                dict.set_item("shell", user.shell)?;
                list.append(dict)?;
            }
            Ok(list.into())
        })
    }

    /// Inspect VM tools installed
    fn inspect_vm_tools(&mut self, root: String) -> PyResult<Vec<String>> {
        self.handle
            .inspect_vm_tools(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Inspect web servers
    fn inspect_web_servers(&mut self, root: String) -> PyResult<Py<PyAny>> {
        let servers = self.handle.inspect_web_servers(&root).map_err(to_pyerr)?;
        Python::attach(|py| {
            let list = pyo3::types::PyList::empty(py);
            for srv in servers {
                let dict = pyo3::types::PyDict::new(py);
                dict.set_item("name", srv.name)?;
                dict.set_item("version", srv.version)?;
                dict.set_item("config_path", srv.config_path)?;
                dict.set_item("enabled", srv.enabled)?;
                list.append(dict)?;
            }
            Ok(list.into())
        })
    }

    // === Block Device Operations ===

    /// Set device read-only
    fn blockdev_setro(&mut self, device: String) -> PyResult<()> {
        self.handle
            .blockdev_setro(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Set device read-write
    fn blockdev_setrw(&mut self, device: String) -> PyResult<()> {
        self.handle
            .blockdev_setrw(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get device read-only flag
    fn blockdev_getro(&mut self, device: String) -> PyResult<bool> {
        self.handle
            .blockdev_getro(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Flush device buffers
    fn blockdev_flushbufs(&mut self, device: String) -> PyResult<()> {
        self.handle
            .blockdev_flushbufs(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Re-read partition table
    fn blockdev_rereadpt(&mut self, device: String) -> PyResult<()> {
        self.handle
            .blockdev_rereadpt(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get device block size
    fn blockdev_getbsz(&mut self, device: String) -> PyResult<i32> {
        self.handle
            .blockdev_getbsz(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Set device block size
    fn blockdev_setbsz(&mut self, device: String, blocksize: i32) -> PyResult<()> {
        self.handle
            .blockdev_setbsz(&device, blocksize)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get total device sectors
    fn blockdev_getsectors(&mut self, device: String) -> PyResult<i64> {
        self.handle
            .blockdev_getsectors(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get device sector size
    fn blockdev_getss(&mut self, device: String) -> PyResult<i32> {
        self.handle
            .blockdev_getss(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get device size in 512-byte sectors
    fn blockdev_getsz(&self, device: String) -> PyResult<i64> {
        self.handle
            .blockdev_getsz(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Partition Operations ===

    /// List partitions on a device
    fn part_list(&self, device: String) -> PyResult<Py<PyAny>> {
        let parts = self.handle.part_list(&device).map_err(to_pyerr)?;
        Python::attach(|py| {
            let list = pyo3::types::PyList::empty(py);
            for p in parts {
                let dict = pyo3::types::PyDict::new(py);
                dict.set_item("part_num", p.part_num)?;
                dict.set_item("part_start", p.part_start)?;
                dict.set_item("part_end", p.part_end)?;
                dict.set_item("part_size", p.part_size)?;
                list.append(dict)?;
            }
            Ok(list.into())
        })
    }

    /// Add a partition
    fn part_add(
        &mut self,
        device: String,
        prlogex: String,
        startsect: i64,
        endsect: i64,
    ) -> PyResult<()> {
        self.handle
            .part_add(&device, &prlogex, startsect, endsect)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Delete a partition
    fn part_del(&mut self, device: String, partnum: i32) -> PyResult<()> {
        self.handle
            .part_del(&device, partnum)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Initialize partition table
    fn part_init(&mut self, device: String, parttype: String) -> PyResult<()> {
        self.handle
            .part_init(&device, &parttype)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Resize a partition
    fn part_resize(&mut self, device: String, partnum: i32, endsect: i64) -> PyResult<()> {
        self.handle
            .part_resize(&device, partnum, endsect)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get bootable flag
    fn part_get_bootable(&self, device: String, partnum: i32) -> PyResult<bool> {
        self.handle
            .part_get_bootable(&device, partnum)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Set bootable flag
    fn part_set_bootable(&mut self, device: String, partnum: i32, bootable: bool) -> PyResult<()> {
        self.handle
            .part_set_bootable(&device, partnum, bootable)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get MBR partition type ID
    fn part_get_mbr_id(&self, device: String, partnum: i32) -> PyResult<i32> {
        self.handle
            .part_get_mbr_id(&device, partnum)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Set MBR partition type ID
    fn part_set_mbr_id(&mut self, device: String, partnum: i32, idbyte: i32) -> PyResult<()> {
        self.handle
            .part_set_mbr_id(&device, partnum, idbyte)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get partition name
    fn part_get_name(&mut self, device: String, partnum: i32) -> PyResult<String> {
        self.handle
            .part_get_name(&device, partnum)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Set partition name
    fn part_set_name(&mut self, device: String, partnum: i32, name: String) -> PyResult<()> {
        self.handle
            .part_set_name(&device, partnum, &name)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get partition table type
    fn part_get_parttype(&self, device: String) -> PyResult<String> {
        self.handle
            .part_get_parttype(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Set partition table type
    fn part_set_parttype(&mut self, device: String, parttype: String) -> PyResult<()> {
        self.handle
            .part_set_parttype(&device, &parttype)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get device name from partition
    fn part_to_dev(&self, partition: String) -> PyResult<String> {
        self.handle
            .part_to_dev(&partition)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get partition number
    fn part_to_partnum(&self, partition: String) -> PyResult<i32> {
        self.handle
            .part_to_partnum(&partition)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get GPT partition type GUID
    fn part_get_gpt_type(&mut self, device: String, partnum: i32) -> PyResult<String> {
        self.handle
            .part_get_gpt_type(&device, partnum)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Set GPT partition type GUID
    fn part_set_gpt_type(&mut self, device: String, partnum: i32, guid: String) -> PyResult<()> {
        self.handle
            .part_set_gpt_type(&device, partnum, &guid)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get GPT partition GUID
    fn part_get_gpt_guid(&mut self, device: String, partnum: i32) -> PyResult<String> {
        self.handle
            .part_get_gpt_guid(&device, partnum)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get disk GUID
    fn part_get_disk_guid(&mut self, device: String) -> PyResult<String> {
        self.handle
            .part_get_disk_guid(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Filesystem Operations (extended) ===

    /// Create a filesystem
    fn mkfs(&mut self, fstype: String, device: String) -> PyResult<()> {
        self.handle
            .mkfs(&fstype, &device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Create a filesystem with options
    #[pyo3(signature = (fstype, device, blocksize=None, features=None, label=None))]
    fn mkfs_opts(
        &mut self,
        fstype: String,
        device: String,
        blocksize: Option<i32>,
        features: Option<String>,
        label: Option<String>,
    ) -> PyResult<()> {
        self.handle
            .mkfs_opts(
                &fstype,
                &device,
                blocksize,
                features.as_deref(),
                label.as_deref(),
            )
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Check filesystem
    fn fsck(&mut self, fstype: String, device: String) -> PyResult<i32> {
        self.handle
            .fsck(&fstype, &device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Check ext2/3/4 filesystem
    fn e2fsck(&mut self, device: String, correct: bool, forceall: bool) -> PyResult<()> {
        self.handle
            .e2fsck(&device, correct, forceall)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Tune ext2/3/4 filesystem
    #[pyo3(signature = (device, force=false, maxmountcount=None, label=None))]
    fn tune2fs(
        &mut self,
        device: String,
        force: bool,
        maxmountcount: Option<i32>,
        label: Option<String>,
    ) -> PyResult<()> {
        self.handle
            .tune2fs(&device, force, maxmountcount, label.as_deref())
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Repair XFS filesystem
    #[pyo3(signature = (device, forcelogzero=false, nomodify=false))]
    fn xfs_repair(&mut self, device: String, forcelogzero: bool, nomodify: bool) -> PyResult<i32> {
        self.handle
            .xfs_repair(&device, forcelogzero, nomodify)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get XFS filesystem info
    fn xfs_info(&mut self, pathordevice: String) -> PyResult<String> {
        self.handle
            .xfs_info(&pathordevice)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Create ext2/3/4 filesystem
    fn mke2fs(
        &mut self,
        device: String,
        blockscount: i64,
        blocksize: i64,
        fragsize: i64,
        reserved: i64,
        inode: i64,
    ) -> PyResult<()> {
        self.handle
            .mke2fs(&device, blockscount, blocksize, fragsize, reserved, inode)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Zero free blocks on ext2/3/4
    fn zerofree(&mut self, device: String) -> PyResult<()> {
        self.handle
            .zerofree(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Trim filesystem
    fn fstrim(&mut self, mountpoint: String) -> PyResult<()> {
        self.handle
            .fstrim(&mountpoint)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get filesystem label
    fn get_label(&mut self, mountable: String) -> PyResult<String> {
        self.handle
            .get_label(&mountable)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Set filesystem label
    fn set_label(&mut self, mountable: String, label: String) -> PyResult<()> {
        self.handle
            .set_label(&mountable, &label)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get filesystem UUID
    fn get_uuid(&mut self, device: String) -> PyResult<String> {
        self.handle
            .get_uuid(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Set filesystem UUID
    fn set_uuid(&mut self, device: String, uuid: String) -> PyResult<()> {
        self.handle
            .set_uuid(&device, &uuid)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Set random filesystem UUID
    fn set_uuid_random(&mut self, device: String) -> PyResult<()> {
        self.handle
            .set_uuid_random(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get disk usage report
    fn df(&mut self) -> PyResult<String> {
        self.handle
            .df()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get human-readable disk usage report
    fn df_h(&mut self) -> PyResult<String> {
        self.handle
            .df_h()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === File Operations (extended) ===

    /// Copy file
    fn cp(&mut self, src: String, dest: String) -> PyResult<()> {
        self.handle
            .cp(&src, &dest)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Copy file preserving attributes
    fn cp_a(&mut self, src: String, dest: String) -> PyResult<()> {
        self.handle
            .cp_a(&src, &dest)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Copy file recursively
    fn cp_r(&mut self, src: String, dest: String) -> PyResult<()> {
        self.handle
            .cp_r(&src, &dest)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Move file
    fn mv(&mut self, src: String, dest: String) -> PyResult<()> {
        self.handle
            .mv(&src, &dest)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Touch (create/update timestamp) file
    fn touch(&mut self, path: String) -> PyResult<()> {
        self.handle
            .touch(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Find files recursively
    fn find(&mut self, directory: String) -> PyResult<Vec<String>> {
        self.handle
            .find(&directory)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Find files and write null-separated list
    fn find0(&mut self, directory: String, files: String) -> PyResult<()> {
        self.handle
            .find0(&directory, &files)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Grep file contents
    fn grep(&mut self, regex: String, path: String) -> PyResult<Vec<String>> {
        self.handle
            .grep(&regex, &path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Extended grep
    fn egrep(&mut self, regex: String, path: String) -> PyResult<Vec<String>> {
        self.handle
            .egrep(&regex, &path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Fixed-string grep
    fn fgrep(&mut self, pattern: String, path: String) -> PyResult<Vec<String>> {
        self.handle
            .fgrep(&pattern, &path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Read first 10 lines of file
    fn head(&mut self, path: String) -> PyResult<Vec<String>> {
        self.handle
            .head(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Read first N lines of file
    fn head_n(&mut self, nrlines: i32, path: String) -> PyResult<Vec<String>> {
        self.handle
            .head_n(nrlines, &path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Read last 10 lines of file
    fn tail(&mut self, path: String) -> PyResult<Vec<String>> {
        self.handle
            .tail(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Read last N lines of file
    fn tail_n(&mut self, nrlines: i32, path: String) -> PyResult<Vec<String>> {
        self.handle
            .tail_n(nrlines, &path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get disk usage of path
    fn du(&mut self, path: String) -> PyResult<i64> {
        self.handle
            .du(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get file size
    fn filesize(&mut self, file: String) -> PyResult<i64> {
        self.handle
            .filesize(&file)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Resolve path to absolute
    fn realpath(&mut self, path: String) -> PyResult<String> {
        self.handle
            .realpath(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Read file lines
    fn read_lines(&mut self, path: String) -> PyResult<Vec<String>> {
        self.handle
            .read_lines(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Append data to file
    fn write_append(&mut self, path: String, content: Vec<u8>) -> PyResult<()> {
        self.handle
            .write_append(&path, &content)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Long listing of directory
    fn ll(&mut self, directory: String) -> PyResult<String> {
        self.handle
            .ll(&directory)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Create hard link
    fn ln(&mut self, target: String, linkname: String) -> PyResult<()> {
        self.handle
            .ln(&target, &linkname)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Create hard link (force)
    fn ln_f(&mut self, target: String, linkname: String) -> PyResult<()> {
        self.handle
            .ln_f(&target, &linkname)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Create symbolic link
    fn ln_s(&mut self, target: String, linkname: String) -> PyResult<()> {
        self.handle
            .ln_s(&target, &linkname)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Create symbolic link (force)
    fn ln_sf(&mut self, target: String, linkname: String) -> PyResult<()> {
        self.handle
            .ln_sf(&target, &linkname)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Read symbolic link target
    fn readlink(&mut self, path: String) -> PyResult<String> {
        self.handle
            .readlink(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Truncate file to zero size
    fn truncate(&mut self, path: String) -> PyResult<()> {
        self.handle
            .truncate(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Truncate file to given size
    fn truncate_size(&mut self, path: String, size: i64) -> PyResult<()> {
        self.handle
            .truncate_size(&path, size)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Metadata & Permissions ===

    /// Get file stat (without following symlinks)
    fn lstat(&mut self, path: String) -> PyResult<Py<PyAny>> {
        let stat = self.handle.lstat(&path).map_err(to_pyerr)?;
        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("dev", stat.dev)?;
            dict.set_item("ino", stat.ino)?;
            dict.set_item("mode", stat.mode)?;
            dict.set_item("nlink", stat.nlink)?;
            dict.set_item("uid", stat.uid)?;
            dict.set_item("gid", stat.gid)?;
            dict.set_item("rdev", stat.rdev)?;
            dict.set_item("size", stat.size)?;
            dict.set_item("blksize", stat.blksize)?;
            dict.set_item("blocks", stat.blocks)?;
            dict.set_item("atime", stat.atime)?;
            dict.set_item("mtime", stat.mtime)?;
            dict.set_item("ctime", stat.ctime)?;
            Ok(dict.into())
        })
    }

    /// Get file mode
    fn get_mode(&mut self, path: String) -> PyResult<u32> {
        self.handle
            .get_mode(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get file UID
    fn get_uid(&mut self, path: String) -> PyResult<u32> {
        self.handle
            .get_uid(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get file GID
    fn get_gid(&mut self, path: String) -> PyResult<u32> {
        self.handle
            .get_gid(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get file access time
    fn get_atime(&mut self, path: String) -> PyResult<i64> {
        self.handle
            .get_atime(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get file modification time
    fn get_mtime(&mut self, path: String) -> PyResult<i64> {
        self.handle
            .get_mtime(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get file change time
    fn get_ctime(&mut self, path: String) -> PyResult<i64> {
        self.handle
            .get_ctime(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get file size
    fn get_size(&mut self, path: String) -> PyResult<i64> {
        self.handle
            .get_size(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get number of hard links
    fn get_nlink(&mut self, path: String) -> PyResult<u64> {
        self.handle
            .get_nlink(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Check if path is a symbolic link
    fn is_symlink(&mut self, path: String) -> PyResult<bool> {
        self.handle
            .is_symlink(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Check if path is a block device
    fn is_blockdev(&mut self, path: String) -> PyResult<bool> {
        self.handle
            .is_blockdev(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Check if path is a character device
    fn is_chardev(&mut self, path: String) -> PyResult<bool> {
        self.handle
            .is_chardev(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Check if path is a FIFO
    fn is_fifo(&mut self, path: String) -> PyResult<bool> {
        self.handle
            .is_fifo(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Check if path is a socket
    fn is_socket(&mut self, path: String) -> PyResult<bool> {
        self.handle
            .is_socket(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Recursively change permissions
    fn chmod_recursive(&mut self, mode: i32, path: String) -> PyResult<()> {
        self.handle
            .chmod_recursive(mode, &path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Change owner by name
    fn chown_by_name(&mut self, username: String, groupname: String, path: String) -> PyResult<()> {
        self.handle
            .chown_by_name(&username, &groupname, &path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Recursively change owner
    fn chown_recursive(&mut self, owner: i32, group: i32, path: String) -> PyResult<()> {
        self.handle
            .chown_recursive(owner, group, &path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get file owner UID
    fn file_owner(&mut self, path: String) -> PyResult<u32> {
        self.handle
            .file_owner(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get file group GID
    fn file_group(&mut self, path: String) -> PyResult<u32> {
        self.handle
            .file_group(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get file permissions mode
    fn file_mode(&mut self, path: String) -> PyResult<u32> {
        self.handle
            .file_mode(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Extended Attributes ===

    /// Get extended attribute value
    fn getxattr(&mut self, path: String, name: String) -> PyResult<Vec<u8>> {
        self.handle
            .getxattr(&path, &name)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Set extended attribute
    fn setxattr(&mut self, xattr: String, val: String, vallen: i32, path: String) -> PyResult<()> {
        self.handle
            .setxattr(&xattr, &val, vallen, &path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Remove extended attribute
    fn removexattr(&mut self, xattr: String, path: String) -> PyResult<()> {
        self.handle
            .removexattr(&xattr, &path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List extended attributes
    fn listxattrs(&mut self, path: String) -> PyResult<Vec<String>> {
        self.handle
            .listxattrs(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === ACL Operations ===

    /// Get file ACL
    fn acl_get_file(&mut self, path: String, acltype: String) -> PyResult<String> {
        self.handle
            .acl_get_file(&path, &acltype)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Set file ACL
    fn acl_set_file(&mut self, path: String, acltype: String, acl: String) -> PyResult<()> {
        self.handle
            .acl_set_file(&path, &acltype, &acl)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Delete default ACL
    fn acl_delete_def_file(&mut self, path: String) -> PyResult<()> {
        self.handle
            .acl_delete_def_file(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get POSIX ACL as text
    fn getfacl(&mut self, path: String) -> PyResult<String> {
        self.handle
            .getfacl(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Set POSIX ACL from text
    fn setfacl(&mut self, mode: String, path: String, acl: String) -> PyResult<()> {
        self.handle
            .setfacl(&mode, &path, &acl)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === LVM Extended ===

    /// Create logical volume
    fn lvcreate(&mut self, logvol: String, volgroup: String, mbytes: i32) -> PyResult<()> {
        self.handle
            .lvcreate(&logvol, &volgroup, mbytes)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Remove logical volume
    fn lvremove(&mut self, device: String) -> PyResult<()> {
        self.handle
            .lvremove(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List logical volumes with full details
    fn lvs_full(&self) -> PyResult<Py<PyAny>> {
        let lvs = self.handle.lvs_full().map_err(to_pyerr)?;
        Python::attach(|py| {
            let list = pyo3::types::PyList::empty(py);
            for lv in lvs {
                let dict = pyo3::types::PyDict::new(py);
                dict.set_item("lv_name", &lv.lv_name)?;
                dict.set_item("lv_uuid", &lv.lv_uuid)?;
                dict.set_item("lv_attr", &lv.lv_attr)?;
                dict.set_item("lv_size", lv.lv_size)?;
                dict.set_item("origin", &lv.origin)?;
                list.append(dict)?;
            }
            Ok(list.into())
        })
    }

    /// Activate volume groups
    fn vg_activate(&mut self, activate: bool, volgroups: Vec<String>) -> PyResult<()> {
        let refs: Vec<&str> = volgroups.iter().map(|s| s.as_str()).collect();
        self.handle
            .vg_activate(activate, &refs)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Activate all volume groups
    fn vg_activate_all(&mut self, activate: bool) -> PyResult<()> {
        self.handle
            .vg_activate_all(activate)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === LUKS Operations ===

    /// Format device with LUKS encryption
    fn luks_format(&mut self, device: String, key: String, keyslot: i32) -> PyResult<()> {
        self.handle
            .luks_format(&device, &key, keyslot)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Open LUKS device
    fn luks_open(&mut self, device: String, key: String, mapname: String) -> PyResult<()> {
        self.handle
            .luks_open(&device, &key, &mapname)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Open LUKS device read-only
    fn luks_open_ro(&mut self, device: String, key: String, mapname: String) -> PyResult<()> {
        self.handle
            .luks_open_ro(&device, &key, &mapname)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Close LUKS device
    fn luks_close(&mut self, device: String) -> PyResult<()> {
        self.handle
            .luks_close(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Additional filesystem/device/LUKS operations ===

    /// Expand a guest path glob.
    fn glob_expand(&mut self, pattern: String) -> PyResult<Vec<String>> {
        self.handle
            .glob_expand(&pattern)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// LVM scan + optional activate.
    fn lvm_scan(&mut self, activate: bool) -> PyResult<()> {
        self.handle
            .lvm_scan(activate)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// blkid key/value map for a device.
    fn blkid(&mut self, device: String) -> PyResult<Py<PyAny>> {
        let map = self
            .handle
            .blkid(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            for (k, v) in map {
                dict.set_item(k, v)?;
            }
            Ok(dict.into())
        })
    }

    fn findfs_uuid(&mut self, uuid: String) -> PyResult<String> {
        self.handle
            .findfs_uuid(&uuid)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn findfs_label(&mut self, label: String) -> PyResult<String> {
        self.handle
            .findfs_label(&label)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn list_dm_devices(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .list_dm_devices()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn rm_f(&mut self, path: String) -> PyResult<()> {
        self.handle
            .rm_f(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn rename(&mut self, src: String, dest: String) -> PyResult<()> {
        self.handle
            .rename(&src, &dest)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn vgchange_activate_all(&mut self, activate: bool) -> PyResult<()> {
        self.handle
            .vgchange_activate_all(activate)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// libguestfs-compatible alias for luks_open(device, key, mapname).
    fn cryptsetup_open(&mut self, device: String, key: String, mapname: String) -> PyResult<()> {
        self.handle
            .cryptsetup_open(&device, &key, &mapname)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn available_all_groups(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .available_all_groups()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn device_index(&self, device: String) -> PyResult<i32> {
        self.handle
            .device_index(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Assemble/activate MD RAID arrays (all discoverable if `devices` is
    /// empty), returning aggregate stats for whatever is active afterward.
    fn md_stat(&mut self, devices: Vec<String>) -> PyResult<Py<PyAny>> {
        let stats = self
            .handle
            .md_activate_all(&devices)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            for (key, value) in stats {
                dict.set_item(key, value)?;
            }
            Ok(dict.into())
        })
    }

    fn is_link(&mut self, path: String) -> PyResult<bool> {
        self.handle
            .is_link(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get file stat information with nanosecond-resolution timestamps.
    fn statns(&mut self, path: String) -> PyResult<Py<PyAny>> {
        let stat = self.handle.statns(&path).map_err(to_pyerr)?;

        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("dev", stat.dev)?;
            dict.set_item("ino", stat.ino)?;
            dict.set_item("mode", stat.mode)?;
            dict.set_item("nlink", stat.nlink)?;
            dict.set_item("uid", stat.uid)?;
            dict.set_item("gid", stat.gid)?;
            dict.set_item("rdev", stat.rdev)?;
            dict.set_item("size", stat.size)?;
            dict.set_item("blksize", stat.blksize)?;
            dict.set_item("blocks", stat.blocks)?;
            dict.set_item("atime", stat.atime)?;
            dict.set_item("mtime", stat.mtime)?;
            dict.set_item("ctime", stat.ctime)?;
            dict.set_item("atime_nsec", stat.atime_nsec)?;
            dict.set_item("mtime_nsec", stat.mtime_nsec)?;
            dict.set_item("ctime_nsec", stat.ctime_nsec)?;
            Ok(dict.into())
        })
    }

    /// Command wrapper (quiet variant).
    fn command_quiet(&mut self, arguments: Vec<String>) -> PyResult<String> {
        let args: Vec<&str> = arguments.iter().map(|s| s.as_str()).collect();
        self.handle
            .command_quiet(&args)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Command with guest mounts available.
    fn command_with_mounts(&mut self, arguments: Vec<String>) -> PyResult<String> {
        let args: Vec<&str> = arguments.iter().map(|s| s.as_str()).collect();
        self.handle
            .command_with_mounts(&args)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Bind-mount guest tree to a host path.
    fn mount_local(&mut self, mountpoint: String) -> PyResult<()> {
        self.handle
            .mount_local(&mountpoint)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn mount_local_run(&mut self) -> PyResult<()> {
        self.handle
            .mount_local_run()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn umount_local(&mut self) -> PyResult<()> {
        self.handle
            .umount_local()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // --- Hivex (Windows registry) ---

    #[pyo3(signature = (filename, write=false))]
    fn hivex_open(&mut self, filename: String, write: bool) -> PyResult<i64> {
        self.handle
            .hivex_open(&filename, write)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn hivex_close(&mut self, handle: i64) -> PyResult<()> {
        self.handle
            .hivex_close(handle)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn hivex_root(&mut self, handle: i64) -> PyResult<i64> {
        self.handle
            .hivex_root(handle)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn hivex_node_name(&mut self, handle: i64, node: i64) -> PyResult<String> {
        self.handle
            .hivex_node_name(handle, node)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn hivex_node_children(&mut self, handle: i64, node: i64) -> PyResult<Vec<i64>> {
        self.handle
            .hivex_node_children(handle, node)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn hivex_node_get_child(&mut self, handle: i64, node: i64, name: String) -> PyResult<i64> {
        self.handle
            .hivex_node_get_child(handle, node, &name)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn hivex_node_values(&mut self, handle: i64, node: i64) -> PyResult<Vec<i64>> {
        self.handle
            .hivex_node_values(handle, node)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn hivex_node_get_value(&mut self, handle: i64, node: i64, key: String) -> PyResult<i64> {
        self.handle
            .hivex_node_get_value(handle, node, &key)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn hivex_value_key(&mut self, handle: i64, value: i64) -> PyResult<String> {
        self.handle
            .hivex_value_key(handle, value)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn hivex_value_type(&mut self, handle: i64, value: i64) -> PyResult<i64> {
        self.handle
            .hivex_value_type(handle, value)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn hivex_value_string(&mut self, handle: i64, value: i64) -> PyResult<String> {
        self.handle
            .hivex_value_string(handle, value)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn hivex_value_dword(&mut self, handle: i64, value: i64) -> PyResult<i32> {
        self.handle
            .hivex_value_dword(handle, value)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn hivex_value_uint32(&mut self, handle: i64, value: i64) -> PyResult<u32> {
        self.handle
            .hivex_value_uint32(handle, value)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn hivex_value_integer(&mut self, handle: i64, value: i64) -> PyResult<i64> {
        self.handle
            .hivex_value_integer(handle, value)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn hivex_value_value(&mut self, handle: i64, value: i64) -> PyResult<Vec<u8>> {
        self.handle
            .hivex_value_value(handle, value)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    #[pyo3(signature = (handle, filename=None))]
    fn hivex_commit(&mut self, handle: i64, filename: Option<String>) -> PyResult<()> {
        self.handle
            .hivex_commit(handle, filename.as_deref())
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn hivex_node_add_child(&mut self, handle: i64, parent: i64, name: String) -> PyResult<i64> {
        self.handle
            .hivex_node_add_child(handle, parent, &name)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Add LUKS key
    fn luks_add_key(
        &mut self,
        device: String,
        key: String,
        newkey: String,
        keyslot: i32,
    ) -> PyResult<()> {
        self.handle
            .luks_add_key(&device, &key, &newkey, keyslot)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get LUKS UUID
    fn luks_uuid(&mut self, device: String) -> PyResult<String> {
        self.handle
            .luks_uuid(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Boot & GRUB ===

    /// Get bootloader type
    fn get_bootloader(&mut self) -> PyResult<String> {
        self.handle
            .get_bootloader()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get kernel command line
    fn get_cmdline(&mut self) -> PyResult<String> {
        self.handle
            .get_cmdline()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get default kernel path
    fn get_default_kernel(&mut self) -> PyResult<String> {
        self.handle
            .get_default_kernel()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get initrd path for a kernel
    fn get_initrd(&mut self, kernel: String) -> PyResult<String> {
        self.handle
            .get_initrd(&kernel)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Check if system uses UEFI
    fn is_uefi(&mut self) -> PyResult<bool> {
        self.handle
            .is_uefi()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List available kernels
    fn list_kernels(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .list_kernels()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List fstab entries as strings
    fn list_fstab(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .list_fstab()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Read fstab content
    fn read_fstab(&mut self) -> PyResult<String> {
        self.handle
            .read_fstab()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Install GRUB bootloader
    fn grub_install(&mut self, root: String, device: String) -> PyResult<()> {
        self.handle
            .grub_install(&root, &device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Read GRUB configuration
    fn grub_read_config(&mut self, path: String) -> PyResult<String> {
        self.handle
            .grub_read_config(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List GRUB boot entries
    fn grub_list_entries(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .grub_list_entries()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Network Operations ===

    /// Get hostname
    fn get_hostname(&mut self) -> PyResult<String> {
        self.handle
            .get_hostname()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Set hostname
    fn set_hostname(&mut self, hostname: String) -> PyResult<()> {
        self.handle
            .set_hostname(&hostname)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get DNS servers
    fn get_dns(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .get_dns()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get network configuration for an interface
    fn get_network_config(&mut self, interface: String) -> PyResult<String> {
        self.handle
            .get_network_config(&interface)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List network interfaces
    fn list_network_interfaces(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .list_network_interfaces()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Read /etc/hosts content
    fn read_etc_hosts(&mut self) -> PyResult<String> {
        self.handle
            .read_etc_hosts()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Service & System ===

    /// Get init system type
    fn get_init_system(&mut self) -> PyResult<String> {
        self.handle
            .get_init_system()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List all services
    fn list_services(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .list_services()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List enabled services
    fn list_enabled_services(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .list_enabled_services()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List disabled services
    fn list_disabled_services(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .list_disabled_services()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Check if service is enabled
    fn is_service_enabled(&mut self, service: String) -> PyResult<bool> {
        self.handle
            .is_service_enabled(&service)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get service status
    fn get_service_status(&mut self, service: String) -> PyResult<String> {
        self.handle
            .get_service_status(&service)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get kernel version
    fn get_kernel_version(&mut self) -> PyResult<String> {
        self.handle
            .get_kernel_version()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get locale
    fn get_locale(&mut self) -> PyResult<String> {
        self.handle
            .get_locale()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get timezone
    fn get_timezone(&mut self) -> PyResult<String> {
        self.handle
            .get_timezone()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Set locale
    fn set_locale(&mut self, locale: String) -> PyResult<()> {
        self.handle
            .set_locale(&locale)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Set timezone
    fn set_timezone(&mut self, timezone: String) -> PyResult<()> {
        self.handle
            .set_timezone(&timezone)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get machine ID
    fn get_machine_id(&mut self) -> PyResult<String> {
        self.handle
            .get_machine_id()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List users
    fn list_users(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .list_users()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List groups
    fn list_groups(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .list_groups()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List systemd units
    fn list_systemd_units(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .list_systemd_units()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get environment variables
    fn get_environment(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .get_environment()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Security & SELinux ===

    /// Get SELinux context
    fn getcon(&mut self) -> PyResult<String> {
        self.handle
            .getcon()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Set SELinux context
    fn setcon(&mut self, context: String) -> PyResult<()> {
        self.handle
            .setcon(&context)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Relabel SELinux contexts
    fn selinux_relabel(&mut self, specfile: String, path: String, force: bool) -> PyResult<()> {
        self.handle
            .selinux_relabel(&specfile, &path, force)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Restore SELinux contexts
    fn restorecon(&mut self, path: String, recursive: bool) -> PyResult<()> {
        self.handle
            .restorecon(&path, recursive)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get file capabilities
    fn getcap(&mut self, path: String) -> PyResult<String> {
        self.handle
            .getcap(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Set file capabilities
    fn setcap(&mut self, cap: String, path: String) -> PyResult<()> {
        self.handle
            .setcap(&cap, &path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Sysprep Operations ===

    /// Run all sysprep operations
    fn sysprep_all(&mut self) -> PyResult<()> {
        self.handle
            .sysprep_all()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Clear bash history
    fn sysprep_bash_history(&mut self) -> PyResult<()> {
        self.handle
            .sysprep_bash_history()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Remove SSH host keys
    fn sysprep_ssh_hostkeys(&mut self) -> PyResult<()> {
        self.handle
            .sysprep_ssh_hostkeys()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Reset machine ID
    fn sysprep_machine_id(&mut self) -> PyResult<()> {
        self.handle
            .sysprep_machine_id()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Clear log files
    fn sysprep_logfiles(&mut self) -> PyResult<()> {
        self.handle
            .sysprep_logfiles()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Clear temporary files
    fn sysprep_tmp_files(&mut self) -> PyResult<()> {
        self.handle
            .sysprep_tmp_files()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Remove network hardware addresses
    fn sysprep_net_hwaddr(&mut self) -> PyResult<()> {
        self.handle
            .sysprep_net_hwaddr()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Clear package manager cache
    fn sysprep_package_cache(&mut self) -> PyResult<()> {
        self.handle
            .sysprep_package_cache()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Windows Operations ===

    /// Get Windows system root
    fn inspect_get_windows_systemroot(&mut self, root: String) -> PyResult<String> {
        self.handle
            .inspect_get_windows_systemroot(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get Windows version (major, minor)
    fn inspect_get_windows_version(&mut self, root: String) -> PyResult<(i32, i32)> {
        self.handle
            .inspect_get_windows_version(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get path to Windows SOFTWARE hive
    fn inspect_get_windows_software_hive(&mut self, root: String) -> PyResult<String> {
        self.handle
            .inspect_get_windows_software_hive(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get path to Windows SYSTEM hive
    fn inspect_get_windows_system_hive(&mut self, root: String) -> PyResult<String> {
        self.handle
            .inspect_get_windows_system_hive(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get Windows drive letter mappings
    fn inspect_get_drive_mappings(&mut self, root: String) -> PyResult<Py<PyAny>> {
        let mappings = self
            .handle
            .inspect_get_drive_mappings(&root)
            .map_err(to_pyerr)?;
        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            for (k, v) in mappings {
                dict.set_item(k, v)?;
            }
            Ok(dict.into())
        })
    }

    /// List Windows drivers
    fn inspect_list_windows_drivers(&mut self, root: String) -> PyResult<Vec<String>> {
        self.handle
            .inspect_list_windows_drivers(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Check if Windows is hibernated
    fn is_windows_hibernated(&mut self) -> PyResult<bool> {
        self.handle
            .is_windows_hibernated()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Download a registry hive
    fn download_hive(&mut self, hive_path: String, local_path: String) -> PyResult<()> {
        self.handle
            .download_hive(&hive_path, &local_path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Upload a registry hive
    fn upload_hive(&mut self, local_path: String, hive_path: String) -> PyResult<()> {
        self.handle
            .upload_hive(&local_path, &hive_path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get OS icon
    fn inspect_get_icon(&mut self, root: String) -> PyResult<Vec<u8>> {
        self.handle
            .inspect_get_icon(&root)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Disk Management ===

    /// Create a disk image
    fn disk_create(&mut self, filename: String, format: String, size: i64) -> PyResult<()> {
        self.handle
            .disk_create(&filename, &format, size)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Detect disk image format
    fn disk_format(&mut self, filename: String) -> PyResult<String> {
        self.handle
            .disk_format(&filename)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get virtual disk size
    fn disk_virtual_size(&mut self, filename: String) -> PyResult<i64> {
        self.handle
            .disk_virtual_size(&filename)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Check if disk has backing file
    fn disk_has_backing_file(&mut self, filename: String) -> PyResult<bool> {
        self.handle
            .disk_has_backing_file(&filename)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Resize disk image
    fn disk_resize(&mut self, filename: String, size: i64) -> PyResult<()> {
        self.handle
            .disk_resize(&filename, size)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Zero free space on filesystem
    fn zero_free_space(&mut self, directory: String) -> PyResult<()> {
        self.handle
            .zero_free_space(&directory)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Sparsify a disk image
    fn sparsify(&mut self, input: String, output: String) -> PyResult<()> {
        self.handle
            .sparsify(&input, &output)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Archive & Compression (extended) ===

    /// Extract tar archive with options
    #[pyo3(signature = (tarfile, directory, compress=None, xattrs=false, selinux=false, acls=false))]
    fn tar_in_opts(
        &mut self,
        tarfile: String,
        directory: String,
        compress: Option<String>,
        xattrs: bool,
        selinux: bool,
        acls: bool,
    ) -> PyResult<()> {
        self.handle
            .tar_in_opts(
                &tarfile,
                &directory,
                compress.as_deref(),
                xattrs,
                selinux,
                acls,
            )
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Create tar archive with options
    #[pyo3(signature = (directory, tarfile, compress=None, numericowner=false, xattrs=false, selinux=false, acls=false))]
    #[allow(clippy::too_many_arguments)]
    fn tar_out_opts(
        &mut self,
        directory: String,
        tarfile: String,
        compress: Option<String>,
        numericowner: bool,
        xattrs: bool,
        selinux: bool,
        acls: bool,
    ) -> PyResult<()> {
        self.handle
            .tar_out_opts(
                &directory,
                &tarfile,
                compress.as_deref(),
                numericowner,
                xattrs,
                selinux,
                acls,
            )
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Extract CPIO archive
    fn cpio_in(&mut self, cpiofile: String, directory: String) -> PyResult<()> {
        self.handle
            .cpio_in(&cpiofile, &directory)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Create CPIO archive
    fn cpio_out(&mut self, directory: String, cpiofile: String, format: String) -> PyResult<()> {
        self.handle
            .cpio_out(&directory, &cpiofile, &format)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Compress a file
    fn compress_out(&mut self, ctype: String, file: String, output: String) -> PyResult<()> {
        self.handle
            .compress_out(&ctype, &file, &output)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Decompress a file
    fn decompress_file(&mut self, src: String, dest: String, ctype: String) -> PyResult<()> {
        self.handle
            .decompress_file(&src, &dest, &ctype)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Base64 decode file into guest
    fn base64_in(&mut self, base64file: String, filename: String) -> PyResult<()> {
        self.handle
            .base64_in(&base64file, &filename)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Base64 encode file from guest
    fn base64_out(&mut self, filename: String, base64file: String) -> PyResult<()> {
        self.handle
            .base64_out(&filename, &base64file)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === DD & Raw I/O ===

    /// Copy data between files/devices
    fn dd(&mut self, src: String, dest: String) -> PyResult<()> {
        self.handle
            .dd(&src, &dest)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Read bytes from file at offset
    fn pread(&mut self, path: String, count: i32, offset: i64) -> PyResult<Vec<u8>> {
        self.handle
            .pread(&path, count, offset)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Read bytes from device at offset
    fn pread_device(&mut self, device: String, count: i32, offset: i64) -> PyResult<Vec<u8>> {
        self.handle
            .pread_device(&device, count, offset)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Write bytes to file at offset
    fn pwrite(&mut self, path: String, content: Vec<u8>, offset: i64) -> PyResult<i32> {
        self.handle
            .pwrite(&path, &content, offset)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Write bytes to device at offset
    fn pwrite_device(&mut self, device: String, content: Vec<u8>, offset: i64) -> PyResult<i32> {
        self.handle
            .pwrite_device(&device, &content, offset)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Zero a device
    fn zero(&mut self, device: String) -> PyResult<()> {
        self.handle
            .zero(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Zero an entire device
    fn zero_device(&mut self, device: String) -> PyResult<()> {
        self.handle
            .zero_device(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Copy file to file with offsets
    fn copy_file_to_file(
        &mut self,
        src: String,
        dest: String,
        srcoffset: i64,
        destoffset: i64,
        size: i64,
    ) -> PyResult<()> {
        self.handle
            .copy_file_to_file(&src, &dest, srcoffset, destoffset, size)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Copy device to device with offsets
    fn copy_device_to_device(
        &mut self,
        src: String,
        dest: String,
        srcoffset: i64,
        destoffset: i64,
        size: i64,
    ) -> PyResult<()> {
        self.handle
            .copy_device_to_device(&src, &dest, srcoffset, destoffset, size)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Download file with offset
    fn download_offset(
        &mut self,
        remote: String,
        local: String,
        offset: i64,
        size: i64,
    ) -> PyResult<()> {
        self.handle
            .download_offset(&remote, &local, offset, size)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Upload file with offset
    fn upload_offset(&mut self, local: String, remote: String, offset: i64) -> PyResult<()> {
        self.handle
            .upload_offset(&local, &remote, offset)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Package Management ===

    /// List RPM packages
    fn rpm_list(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .rpm_list()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List DPKG packages
    fn dpkg_list(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .dpkg_list()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get package info
    fn get_package_info(&mut self, package: String) -> PyResult<String> {
        self.handle
            .get_package_info(&package)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Check if package is installed
    fn is_package_installed(&mut self, package: String) -> PyResult<bool> {
        self.handle
            .is_package_installed(&package)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List files owned by package
    fn package_files(&mut self, package: String) -> PyResult<Vec<String>> {
        self.handle
            .package_files(&package)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === SSH Operations ===

    /// Get SSH host keys
    fn get_ssh_host_keys(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .get_ssh_host_keys()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get SSH authorized keys for user
    fn get_ssh_authorized_keys(&mut self, user: String) -> PyResult<Vec<String>> {
        self.handle
            .get_ssh_authorized_keys(&user)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Set SSH authorized keys for user
    fn set_ssh_authorized_keys(&mut self, user: String, keys: Vec<String>) -> PyResult<()> {
        let refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
        self.handle
            .set_ssh_authorized_keys(&user, &refs)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get sshd configuration
    fn get_sshd_config(&mut self) -> PyResult<String> {
        self.handle
            .get_sshd_config()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List user SSH keys
    fn list_user_ssh_keys(&mut self, user: String) -> PyResult<Vec<String>> {
        self.handle
            .list_user_ssh_keys(&user)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List SSL certificates
    fn list_ssl_certificates(&mut self) -> PyResult<Vec<String>> {
        self.handle
            .list_ssl_certificates()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Miscellaneous ===

    /// Get guestkit version
    fn version(&self) -> PyResult<(i64, i64, i64)> {
        self.handle
            .version()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Check if feature group is available
    fn available(&mut self, groups: Vec<String>) -> PyResult<bool> {
        let refs: Vec<&str> = groups.iter().map(|s| s.as_str()).collect();
        self.handle
            .available(&refs)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get file type description
    fn file_type(&mut self, path: String) -> PyResult<String> {
        self.handle
            .file_type(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get file architecture
    fn file_architecture(&mut self, path: String) -> PyResult<String> {
        self.handle
            .file_architecture(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get umask
    fn get_umask(&mut self) -> PyResult<i32> {
        self.handle
            .get_umask()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get memory info
    fn get_meminfo(&mut self) -> PyResult<String> {
        self.handle
            .get_meminfo()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get disk usage of path
    fn disk_usage(&mut self, path: String) -> PyResult<i64> {
        self.handle
            .disk_usage(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Create a swap device
    #[pyo3(signature = (device, label=None, uuid=None))]
    fn mkswap(
        &mut self,
        device: String,
        label: Option<String>,
        uuid: Option<String>,
    ) -> PyResult<()> {
        self.handle
            .mkswap(&device, label.as_deref(), uuid.as_deref())
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Hexdump file content
    fn hexdump(&mut self, path: String) -> PyResult<String> {
        self.handle
            .hexdump(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get printable strings from file
    fn strings(&mut self, path: String) -> PyResult<Vec<String>> {
        self.handle
            .strings(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Fill file with byte pattern
    fn fill(&mut self, c: i32, len: i32, path: String) -> PyResult<()> {
        self.handle
            .fill(c, len, &path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === NTFS Operations ===

    /// Fix NTFS filesystem
    fn ntfsfix(&mut self, device: String, clearbadsectors: bool) -> PyResult<()> {
        self.handle
            .ntfsfix(&device, clearbadsectors)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Restore NTFS from backup
    fn ntfsclone_in(&mut self, backupfile: String, device: String) -> PyResult<()> {
        self.handle
            .ntfsclone_in(&backupfile, &device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Backup NTFS to file
    fn ntfsclone_out(
        &mut self,
        device: String,
        backupfile: String,
        metadataonly: bool,
    ) -> PyResult<()> {
        self.handle
            .ntfsclone_out(&device, &backupfile, metadataonly)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Probe NTFS volume
    fn ntfs_3g_probe(&mut self, rw: bool, device: String) -> PyResult<i32> {
        self.handle
            .ntfs_3g_probe(rw, &device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Btrfs Operations ===

    /// Create btrfs subvolume
    fn btrfs_subvolume_create(&mut self, dest: String) -> PyResult<()> {
        self.handle
            .btrfs_subvolume_create(&dest)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Delete btrfs subvolume
    fn btrfs_subvolume_delete(&mut self, subvolume: String) -> PyResult<()> {
        self.handle
            .btrfs_subvolume_delete(&subvolume)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List btrfs subvolumes
    fn btrfs_subvolume_list(&mut self, fs: String) -> PyResult<Vec<String>> {
        self.handle
            .btrfs_subvolume_list(&fs)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Snapshot btrfs subvolume
    fn btrfs_subvolume_snapshot(&mut self, source: String, dest: String, ro: bool) -> PyResult<()> {
        self.handle
            .btrfs_subvolume_snapshot(&source, &dest, ro)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Balance btrfs filesystem
    fn btrfs_balance(&mut self, fs: String) -> PyResult<()> {
        self.handle
            .btrfs_balance(&fs)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Scrub btrfs filesystem
    fn btrfs_scrub(&mut self, fs: String) -> PyResult<()> {
        self.handle
            .btrfs_scrub(&fs)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Show btrfs filesystem info
    fn btrfs_filesystem_show(&mut self, device: String) -> PyResult<String> {
        self.handle
            .btrfs_filesystem_show(&device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === XFS Operations (extended) ===

    /// XFS admin operations
    #[pyo3(signature = (device, extunwritten=false, imgfile=false, v2log=false, projid32bit=false, lazycounter=false, label=None, uuid=None))]
    #[allow(clippy::too_many_arguments)]
    fn xfs_admin(
        &mut self,
        device: String,
        extunwritten: bool,
        imgfile: bool,
        v2log: bool,
        projid32bit: bool,
        lazycounter: bool,
        label: Option<String>,
        uuid: Option<String>,
    ) -> PyResult<i32> {
        self.handle
            .xfs_admin(
                &device,
                extunwritten,
                imgfile,
                v2log,
                projid32bit,
                lazycounter,
                label.as_deref(),
                uuid.as_deref(),
            )
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // === Mount Operations (extended) ===

    /// Mount with options
    fn mount_options(
        &mut self,
        options: String,
        mountable: String,
        mountpoint: String,
    ) -> PyResult<()> {
        self.handle
            .mount_options(&options, &mountable, &mountpoint)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Mount with virtual filesystem type
    fn mount_vfs(
        &mut self,
        options: String,
        vfstype: String,
        mountable: String,
        mountpoint: String,
    ) -> PyResult<()> {
        self.handle
            .mount_vfs(&options, &vfstype, &mountable, &mountpoint)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// List current mounts
    fn mounts(&self) -> PyResult<Vec<String>> {
        self.handle
            .mounts()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get current mountpoints as dictionary
    fn mountpoints(&self) -> PyResult<Py<PyAny>> {
        let mps = self.handle.mountpoints().map_err(to_pyerr)?;
        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            for (k, v) in mps.iter() {
                dict.set_item(k, v)?;
            }
            Ok(dict.into())
        })
    }

    /// List filesystems
    fn list_filesystems(&mut self) -> PyResult<Py<PyAny>> {
        let fss = self.handle.list_filesystems().map_err(to_pyerr)?;
        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            for (k, v) in fss {
                dict.set_item(k, v)?;
            }
            Ok(dict.into())
        })
    }

    /// Get device file description
    fn file(&mut self, path: String) -> PyResult<String> {
        self.handle
            .file(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get checksum of a device
    fn checksum_device(&mut self, csumtype: String, device: String) -> PyResult<String> {
        self.handle
            .checksum_device(&csumtype, &device)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Add drive with options
    #[pyo3(signature = (filename, readonly=false, format=None))]
    fn add_drive_opts(
        &mut self,
        filename: String,
        readonly: bool,
        format: Option<String>,
    ) -> PyResult<()> {
        self.handle
            .add_drive_opts(&filename, readonly, format.as_deref())
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get verbose mode
    fn get_verbose(&self) -> bool {
        self.handle.get_verbose()
    }

    /// Set trace mode
    fn set_trace(&mut self, trace: bool) {
        self.handle.set_trace(trace);
    }

    /// Get trace mode
    fn get_trace(&self) -> bool {
        self.handle.get_trace()
    }

    // === Additional compatibility operations ===

    /// Diagnostic info about this backend instance (implementation, version,
    /// attach mode, readonly)
    fn get_backend_info(&self) -> PyResult<Py<PyAny>> {
        let info = self.handle.get_backend_info().map_err(to_pyerr)?;

        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            for (key, value) in info {
                dict.set_item(key, value)?;
            }
            Ok(dict.into())
        })
    }

    /// Measured performance counters for the most recent launch() call
    fn get_performance_metrics(&self) -> PyResult<Py<PyAny>> {
        let metrics = self.handle.get_performance_metrics().map_err(to_pyerr)?;

        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            for (key, value) in metrics {
                dict.set_item(key, value)?;
            }
            Ok(dict.into())
        })
    }

    /// Close the handle
    fn close(&mut self) -> PyResult<()> {
        self.handle
            .close()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Ping the daemon
    fn ping_daemon(&self) -> PyResult<bool> {
        self.handle
            .ping_daemon()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    // Context manager support
    /// Enter context manager
    ///
    /// # Examples
    ///
    /// ```python
    /// from guestkit import Guestfs
    ///
    /// with Guestfs() as g:
    ///     g.add_drive_ro("/path/to/disk.qcow2")
    ///     g.launch()
    ///     roots = g.inspect_os()
    ///     # ... operations
    ///     # Automatic cleanup on exit
    /// ```
    fn __enter__(slf: PyRef<Self>) -> PyRef<Self> {
        slf
    }

    /// Exit context manager
    #[pyo3(signature = (_exc_type=None, _exc_value=None, _traceback=None))]
    fn __exit__(
        &mut self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        self.shutdown()?;
        Ok(false)
    }
}

/// Python wrapper for LVM clone operations
#[cfg(feature = "python-bindings")]
#[pyclass]
struct LvmCloner {}

#[cfg(feature = "python-bindings")]
#[pymethods]
impl LvmCloner {
    #[new]
    fn new() -> Self {
        Self {}
    }

    /// Clone a logical volume
    #[pyo3(signature = (source_vg, source_lv, clone_lv_name, target_vg=None, regenerate_uuids=true, update_fstab=true, update_bootloader=true, update_crypttab=true, hostname=None, dry_run=false, snapshot_size=None, regenerate_initramfs=false, verify_security=true, regenerate_grub=false, verify_boot=false, verbose=false))]
    #[allow(clippy::too_many_arguments)]
    fn clone(
        &self,
        source_vg: String,
        source_lv: String,
        clone_lv_name: String,
        target_vg: Option<String>,
        regenerate_uuids: bool,
        update_fstab: bool,
        update_bootloader: bool,
        update_crypttab: bool,
        hostname: Option<String>,
        dry_run: bool,
        snapshot_size: Option<String>,
        regenerate_initramfs: bool,
        verify_security: bool,
        regenerate_grub: bool,
        verify_boot: bool,
        verbose: bool,
    ) -> PyResult<Py<PyAny>> {
        let config = crate::guestfs::lvm_clone::LvmCloneConfig {
            source_vg,
            source_lv,
            clone_lv_name,
            target_vg,
            regenerate_uuids,
            update_fstab,
            update_bootloader,
            update_crypttab,
            hostname,
            dry_run,
            snapshot_size,
            regenerate_initramfs,
            isolation_level: crate::guestfs::lvm_clone::IsolationLevel::MountOnly,
            verify_security,
            regenerate_grub,
            verify_boot,
            container_image: None,
        };

        let result = crate::guestfs::lvm_clone::lvm_clone(&config, verbose).map_err(to_pyerr)?;

        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("source_path", &result.source_path)?;
            dict.set_item("clone_path", &result.clone_path)?;
            dict.set_item("timestamp", &result.timestamp)?;
            dict.set_item("fstab_updated", result.fstab_updated)?;
            dict.set_item("bootloader_updated", result.bootloader_updated)?;
            dict.set_item("crypttab_updated", result.crypttab_updated)?;
            dict.set_item("initramfs_regenerated", result.initramfs_regenerated)?;
            dict.set_item("namespace_isolated", result.namespace_isolated)?;
            dict.set_item("grub_regenerated", result.grub_regenerated)?;
            dict.set_item("boot_verified", result.boot_verified)?;
            dict.set_item("kernel_version", result.kernel_version)?;
            dict.set_item("backup_files", &result.backup_files)?;

            let mappings = pyo3::types::PyList::empty(py);
            for m in &result.uuid_mappings {
                let d = pyo3::types::PyDict::new(py);
                d.set_item("device", &m.device)?;
                d.set_item("fs_type", &m.fs_type)?;
                d.set_item("old_uuid", &m.old_uuid)?;
                d.set_item("new_uuid", &m.new_uuid)?;
                mappings.append(d)?;
            }
            dict.set_item("uuid_mappings", mappings)?;

            let warnings = pyo3::types::PyList::empty(py);
            for w in &result.security_warnings {
                let d = pyo3::types::PyDict::new(py);
                d.set_item("category", &w.category)?;
                d.set_item("message", &w.message)?;
                warnings.append(d)?;
            }
            dict.set_item("security_warnings", warnings)?;

            Ok(dict.into())
        })
    }

    /// Clone a logical volume using Podman container isolation
    #[pyo3(signature = (source_vg, source_lv, clone_lv_name, target_vg=None, regenerate_uuids=true, update_fstab=true, update_bootloader=true, update_crypttab=true, hostname=None, dry_run=false, snapshot_size=None, regenerate_initramfs=false, verify_security=true, regenerate_grub=false, verify_boot=false, container_image=None, verbose=false))]
    #[allow(clippy::too_many_arguments)]
    fn clone_podman(
        &self,
        source_vg: String,
        source_lv: String,
        clone_lv_name: String,
        target_vg: Option<String>,
        regenerate_uuids: bool,
        update_fstab: bool,
        update_bootloader: bool,
        update_crypttab: bool,
        hostname: Option<String>,
        dry_run: bool,
        snapshot_size: Option<String>,
        regenerate_initramfs: bool,
        verify_security: bool,
        regenerate_grub: bool,
        verify_boot: bool,
        container_image: Option<String>,
        verbose: bool,
    ) -> PyResult<Py<PyAny>> {
        let config = crate::guestfs::lvm_clone::LvmCloneConfig {
            source_vg,
            source_lv,
            clone_lv_name,
            target_vg,
            regenerate_uuids,
            update_fstab,
            update_bootloader,
            update_crypttab,
            hostname,
            dry_run,
            snapshot_size,
            regenerate_initramfs,
            isolation_level: crate::guestfs::lvm_clone::IsolationLevel::None,
            verify_security,
            regenerate_grub,
            verify_boot,
            container_image,
        };

        let result =
            crate::guestfs::lvm_clone::lvm_clone_podman(&config, verbose).map_err(to_pyerr)?;

        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("source_path", &result.source_path)?;
            dict.set_item("clone_path", &result.clone_path)?;
            dict.set_item("timestamp", &result.timestamp)?;
            dict.set_item("fstab_updated", result.fstab_updated)?;
            dict.set_item("bootloader_updated", result.bootloader_updated)?;
            dict.set_item("crypttab_updated", result.crypttab_updated)?;
            dict.set_item("initramfs_regenerated", result.initramfs_regenerated)?;
            dict.set_item("namespace_isolated", result.namespace_isolated)?;
            dict.set_item("grub_regenerated", result.grub_regenerated)?;
            dict.set_item("boot_verified", result.boot_verified)?;
            dict.set_item("kernel_version", result.kernel_version)?;
            dict.set_item("backup_files", &result.backup_files)?;

            let mappings = pyo3::types::PyList::empty(py);
            for m in &result.uuid_mappings {
                let d = pyo3::types::PyDict::new(py);
                d.set_item("device", &m.device)?;
                d.set_item("fs_type", &m.fs_type)?;
                d.set_item("old_uuid", &m.old_uuid)?;
                d.set_item("new_uuid", &m.new_uuid)?;
                mappings.append(d)?;
            }
            dict.set_item("uuid_mappings", mappings)?;

            let warnings = pyo3::types::PyList::empty(py);
            for w in &result.security_warnings {
                let d = pyo3::types::PyDict::new(py);
                d.set_item("category", &w.category)?;
                d.set_item("message", &w.message)?;
                warnings.append(d)?;
            }
            dict.set_item("security_warnings", warnings)?;

            Ok(dict.into())
        })
    }

    /// Verify a cloned logical volume
    fn verify(&self, lv_path: String) -> PyResult<bool> {
        crate::guestfs::lvm_clone::lvm_clone_verify(&lv_path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Clone a logical volume to a disk image file
    #[pyo3(signature = (source_vg, source_lv, output_path, output_format=None, keep_raw=false, verbose=false))]
    fn clone_to_disk_image(
        &self,
        source_vg: String,
        source_lv: String,
        output_path: String,
        output_format: Option<String>,
        keep_raw: bool,
        verbose: bool,
    ) -> PyResult<Py<PyAny>> {
        let result = crate::guestfs::lvm_clone::clone_lv_to_disk_image(
            &source_vg,
            &source_lv,
            Path::new(&output_path),
            output_format.as_deref(),
            keep_raw,
            verbose,
        )
        .map_err(to_pyerr)?;

        Python::attach(|py| {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("source_path", &result.source_path)?;
            dict.set_item("image_path", &result.image_path)?;
            dict.set_item("image_format", &result.image_format)?;
            dict.set_item("image_size", result.image_size)?;
            dict.set_item("raw_copy", result.raw_copy)?;
            Ok(dict.into())
        })
    }

    /// Convert a disk image to another format
    fn convert_disk_image(
        &self,
        source: String,
        output: String,
        output_format: String,
        verbose: bool,
    ) -> PyResult<()> {
        crate::guestfs::lvm_clone::convert_disk_image(
            Path::new(&source),
            Path::new(&output),
            &output_format,
            verbose,
        )
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }
}

/* Async Python API - blocked on pyo3-asyncio PyO3 0.22+ support
 * See: https://github.com/awestlake87/pyo3-asyncio/issues
 *
/// Async Python wrapper for Guestfs handle
///
/// Provides non-blocking operations for concurrent VM inspection.
#[cfg(feature = "python-bindings")]
#[pyclass]
struct AsyncGuestfs {
    handle: std::sync::Arc<tokio::sync::Mutex<crate::guestfs::Guestfs>>,
}

#[cfg(feature = "python-bindings")]
#[pymethods]
impl AsyncGuestfs {
*/
/*
    /// Create a new AsyncGuestfs handle
    ///
    /// # Examples
    ///
    /// ```python
    /// import asyncio
    /// from guestkit import AsyncGuestfs
    ///
    /// async def main():
    ///     async with AsyncGuestfs() as g:
    ///         await g.add_drive_ro("/path/to/disk.qcow2")
    ///         await g.launch()
    ///         roots = await g.inspect_os()
    ///         for root in roots:
    ///             print(f"Found OS: {await g.inspect_get_distro(root)}")
    ///
    /// asyncio.run(main())
    /// ```
    #[new]
    fn new() -> PyResult<Self> {
        let handle = crate::guestfs::Guestfs::new()
            .map_err(to_pyerr)?;

        Ok(Self {
            handle: std::sync::Arc::new(tokio::sync::Mutex::new(handle)),
        })
    }

    /// Context manager entry
    fn __aenter__<'p>(slf: pyo3::Py<Self>, py: pyo3::Python<'p>) -> PyResult<pyo3::Bound<'p, pyo3::types::PyAny>> {
        pyo3_asyncio_0_21::tokio::future_into_py(py, async move {
            Ok(slf)
        })
    }

    /// Context manager exit
    fn __aexit__<'p>(
        slf: pyo3::Py<Self>,
        py: pyo3::Python<'p>,
        _exc_type: Option<&pyo3::Bound<'_, pyo3::types::PyAny>>,
        _exc_value: Option<&pyo3::Bound<'_, pyo3::types::PyAny>>,
        _traceback: Option<&pyo3::Bound<'_, pyo3::types::PyAny>>,
    ) -> PyResult<pyo3::Bound<'p, pyo3::types::PyAny>> {
        pyo3_asyncio_0_21::tokio::future_into_py(py, async move {
            Python::attach(|py| {
                let binding = slf.bind(py).borrow_mut();
                let handle = binding.handle.clone();
                drop(binding);

                tokio::spawn(async move {
                    let mut h = handle.lock().await;
                    let _ = h.shutdown();
                });

                Ok(false)
            })
        })
    }

    /// Add a disk image (read-only) - async version
    fn add_drive_ro<'p>(
        &self,
        py: pyo3::Python<'p>,
        filename: String,
    ) -> PyResult<pyo3::Bound<'p, pyo3::types::PyAny>> {
        let handle = self.handle.clone();
        pyo3_asyncio_0_21::tokio::future_into_py(py, async move {
            let mut h = handle.lock().await;
            h.add_drive_ro(&filename)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        })
    }

    /// Add a disk image (read-write) - async version
    fn add_drive<'p>(
        &self,
        py: pyo3::Python<'p>,
        filename: String,
    ) -> PyResult<pyo3::Bound<'p, pyo3::types::PyAny>> {
        let handle = self.handle.clone();
        pyo3_asyncio_0_21::tokio::future_into_py(py, async move {
            let mut h = handle.lock().await;
            h.add_drive(&filename)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        })
    }

    /// Launch the backend (analyze disk) - async version
    fn launch<'p>(&self, py: pyo3::Python<'p>) -> PyResult<pyo3::Bound<'p, pyo3::types::PyAny>> {
        let handle = self.handle.clone();
        pyo3_asyncio_0_21::tokio::future_into_py(py, async move {
            let mut h = handle.lock().await;
            h.launch()
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        })
    }

    /// Shutdown the backend - async version
    fn shutdown<'p>(&self, py: pyo3::Python<'p>) -> PyResult<pyo3::Bound<'p, pyo3::types::PyAny>> {
        let handle = self.handle.clone();
        pyo3_asyncio_0_21::tokio::future_into_py(py, async move {
            let mut h = handle.lock().await;
            h.shutdown()
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        })
    }

    /// Inspect operating systems in the disk image - async version
    fn inspect_os<'p>(&self, py: pyo3::Python<'p>) -> PyResult<pyo3::Bound<'p, pyo3::types::PyAny>> {
        let handle = self.handle.clone();
        pyo3_asyncio_0_21::tokio::future_into_py(py, async move {
            let mut h = handle.lock().await;
            h.inspect_os()
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        })
    }

    /// Get OS type - async version
    fn inspect_get_type<'p>(
        &self,
        py: pyo3::Python<'p>,
        root: String,
    ) -> PyResult<pyo3::Bound<'p, pyo3::types::PyAny>> {
        let handle = self.handle.clone();
        pyo3_asyncio_0_21::tokio::future_into_py(py, async move {
            let mut h = handle.lock().await;
            h.inspect_get_type(&root)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        })
    }

    /// Get distribution name - async version
    fn inspect_get_distro<'p>(
        &self,
        py: pyo3::Python<'p>,
        root: String,
    ) -> PyResult<pyo3::Bound<'p, pyo3::types::PyAny>> {
        let handle = self.handle.clone();
        pyo3_asyncio_0_21::tokio::future_into_py(py, async move {
            let mut h = handle.lock().await;
            h.inspect_get_distro(&root)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        })
    }

    /// Get major version - async version
    fn inspect_get_major_version<'p>(
        &self,
        py: pyo3::Python<'p>,
        root: String,
    ) -> PyResult<pyo3::Bound<'p, pyo3::types::PyAny>> {
        let handle = self.handle.clone();
        pyo3_asyncio_0_21::tokio::future_into_py(py, async move {
            let mut h = handle.lock().await;
            h.inspect_get_major_version(&root)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        })
    }

    /// Get minor version - async version
    fn inspect_get_minor_version<'p>(
        &self,
        py: pyo3::Python<'p>,
        root: String,
    ) -> PyResult<pyo3::Bound<'p, pyo3::types::PyAny>> {
        let handle = self.handle.clone();
        pyo3_asyncio_0_21::tokio::future_into_py(py, async move {
            let mut h = handle.lock().await;
            h.inspect_get_minor_version(&root)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        })
    }

    /// Get hostname - async version
    fn inspect_get_hostname<'p>(
        &self,
        py: pyo3::Python<'p>,
        root: String,
    ) -> PyResult<pyo3::Bound<'p, pyo3::types::PyAny>> {
        let handle = self.handle.clone();
        pyo3_asyncio_0_21::tokio::future_into_py(py, async move {
            let mut h = handle.lock().await;
            h.inspect_get_hostname(&root)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        })
    }

    /// List filesystems - async version
    fn list_filesystems<'p>(
        &self,
        py: pyo3::Python<'p>,
    ) -> PyResult<pyo3::Bound<'p, pyo3::types::PyAny>> {
        let handle = self.handle.clone();
        pyo3_asyncio_0_21::tokio::future_into_py(py, async move {
            let mut h = handle.lock().await;
            h.list_filesystems()
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        })
    }

    /// Mount a filesystem - async version
    fn mount<'p>(
        &self,
        py: pyo3::Python<'p>,
        device: String,
        mountpoint: String,
    ) -> PyResult<pyo3::Bound<'p, pyo3::types::PyAny>> {
        let handle = self.handle.clone();
        pyo3_asyncio_0_21::tokio::future_into_py(py, async move {
            let mut h = handle.lock().await;
            h.mount(&device, &mountpoint)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        })
    }

    /// List directory contents - async version
    fn ls<'p>(
        &self,
        py: pyo3::Python<'p>,
        directory: String,
    ) -> PyResult<pyo3::Bound<'p, pyo3::types::PyAny>> {
        let handle = self.handle.clone();
        pyo3_asyncio_0_21::tokio::future_into_py(py, async move {
            let mut h = handle.lock().await;
            h.ls(&directory)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        })
    }

    /// Read file contents - async version
    fn cat<'p>(
        &self,
        py: pyo3::Python<'p>,
        path: String,
    ) -> PyResult<pyo3::Bound<'p, pyo3::types::PyAny>> {
        let handle = self.handle.clone();
        pyo3_asyncio_0_21::tokio::future_into_py(py, async move {
            let mut h = handle.lock().await;
            h.cat(&path)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        })
    }
}
*/

/// Serialize a Rust value to a Python dict via JSON round-trip.
#[cfg(feature = "python-bindings")]
fn serde_to_py<T: serde::Serialize>(py: Python<'_>, value: &T) -> PyResult<Py<PyAny>> {
    let json_str = serde_json::to_string(value).map_err(to_pyerr_generic)?;
    let json_module = py.import("json")?;
    let loads = json_module.getattr("loads")?;
    Ok(loads.call1((json_str,))?.into())
}

/// Run bootability doctor on an offline disk image.
#[cfg(feature = "python-bindings")]
#[pyfunction]
#[pyo3(signature = (image, target = "kvm", explain = false, verbose = false))]
fn run_doctor(
    py: Python<'_>,
    image: String,
    target: &str,
    explain: bool,
    verbose: bool,
) -> PyResult<Py<PyAny>> {
    let result = crate::assurance::run_doctor(Path::new(&image), target, explain, verbose)
        .map_err(to_pyerr_generic)?;
    serde_to_py(py, &result)
}

/// Run boot inspect summary on an offline disk image.
#[cfg(feature = "python-bindings")]
#[pyfunction]
#[pyo3(signature = (image, target = "kvm", verbose = false))]
fn run_boot_inspect(
    py: Python<'_>,
    image: String,
    target: &str,
    verbose: bool,
) -> PyResult<Py<PyAny>> {
    let result = crate::assurance::run_boot_inspect(Path::new(&image), target, verbose)
        .map_err(to_pyerr_generic)?;
    serde_to_py(py, &result)
}

/// Generate a hypervisor-aware migration plan.
#[cfg(feature = "python-bindings")]
#[pyfunction]
#[pyo3(signature = (image, target = "kvm", explain = false, verbose = false, export_fix_plan = false))]
fn run_migrate_plan(
    py: Python<'_>,
    image: String,
    target: &str,
    explain: bool,
    verbose: bool,
    export_fix_plan: bool,
) -> PyResult<Py<PyAny>> {
    let options = crate::assurance::MigratePlanOptions {
        explain,
        verbose,
        export_fix_plan,
        inject_agent: false,
        #[cfg(feature = "agent")]
        agent_binary: None,
        #[cfg(feature = "agent")]
        agent_unit: None,
    };
    let result = crate::assurance::run_migrate_plan(Path::new(&image), target, &options)
        .map_err(to_pyerr_generic)?;
    serde_to_py(py, &result)
}

/// Generate or apply a boot repair plan.
#[cfg(feature = "python-bindings")]
#[pyfunction]
#[pyo3(signature = (image, dry_run = true, verbose = false, fix_cloud_init_network = false, validate_fstab = false))]
fn run_repair_plan(
    py: Python<'_>,
    image: String,
    dry_run: bool,
    verbose: bool,
    fix_cloud_init_network: bool,
    validate_fstab: bool,
) -> PyResult<Py<PyAny>> {
    let options = crate::assurance::RepairOptions {
        dry_run,
        verbose,
        inject_agent: false,
        inject_qga: false,
        enable_systemd: false,
        fix_cloud_init_network,
        validate_fstab,
        #[cfg(feature = "agent")]
        agent_binary: None,
        #[cfg(feature = "agent")]
        agent_unit: None,
    };
    let result =
        crate::assurance::run_repair_plan(Path::new(&image), &options).map_err(to_pyerr_generic)?;
    serde_to_py(py, &result)
}

/// Generate or apply a hypervisor-aware migration repair plan.
#[cfg(feature = "python-bindings")]
#[pyfunction]
#[pyo3(signature = (image, target = "kvm", apply = false, include_destructive = false, virtio_win = None, verbose = false))]
fn run_migrate_repair(
    py: Python<'_>,
    image: String,
    target: &str,
    apply: bool,
    include_destructive: bool,
    virtio_win: Option<String>,
    verbose: bool,
) -> PyResult<Py<PyAny>> {
    let options = crate::assurance::MigrateRepairOptions {
        apply,
        include_destructive,
        virtio_win_dir: virtio_win.map(PathBuf::from),
        verbose,
    };
    let result = crate::assurance::run_migrate_repair(Path::new(&image), target, &options)
        .map_err(to_pyerr_generic)?;
    serde_to_py(py, &result)
}

/// Python module definition
#[cfg(feature = "python-bindings")]
#[pymodule]
fn guestkit(m: &pyo3::Bound<'_, pyo3::types::PyModule>) -> PyResult<()> {
    m.add_class::<Guestfs>()?;
    // m.add_class::<AsyncGuestfs>()?;  // Blocked on pyo3-asyncio PyO3 0.22+ support
    m.add_class::<DiskConverter>()?;
    m.add_class::<LvmCloner>()?;
    m.add_function(wrap_pyfunction!(run_doctor, m)?)?;
    m.add_function(wrap_pyfunction!(run_boot_inspect, m)?)?;
    m.add_function(wrap_pyfunction!(run_migrate_plan, m)?)?;
    m.add_function(wrap_pyfunction!(run_repair_plan, m)?)?;
    m.add_function(wrap_pyfunction!(run_migrate_repair, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

// Stub when python-bindings feature is not enabled
#[cfg(not(feature = "python-bindings"))]
pub fn python_bindings_not_available() {
    eprintln!("Python bindings not compiled. Build with --features python-bindings");
}
