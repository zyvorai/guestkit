// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Builder pattern for creating Guestfs handles with fluent API

use super::handle::DriveConfig;
use super::Guestfs;
use crate::core::Result;
use std::path::Path;

/// Builder for creating Guestfs handles with a fluent, type-safe API
///
/// # Examples
///
/// ```no_run
/// use guestkit::Guestfs;
///
/// let mut guest = Guestfs::builder()
///     .add_drive("disk.img")
///     .add_drive_ro("template.img")
///     .verbose(true)
///     .autosync(true)
///     .build()?;
///
/// guest.launch()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct GuestfsBuilder {
    verbose: bool,
    trace: bool,
    readonly: bool,
    drives: Vec<DriveConfig>,
    autosync: bool,
    selinux: bool,
    identifier: Option<String>,
}

impl Default for GuestfsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl GuestfsBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            verbose: false,
            trace: false,
            readonly: false,
            drives: Vec::new(),
            autosync: true,
            selinux: false,
            identifier: None,
        }
    }

    /// Enable verbose output
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use guestkit::Guestfs;
    /// let guest = Guestfs::builder()
    ///     .verbose(true)
    ///     .build()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn verbose(mut self, enabled: bool) -> Self {
        self.verbose = enabled;
        self
    }

    /// Enable tracing
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use guestkit::Guestfs;
    /// let guest = Guestfs::builder()
    ///     .trace(true)
    ///     .build()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn trace(mut self, enabled: bool) -> Self {
        self.trace = enabled;
        self
    }

    /// Set read-only mode for all drives by default
    pub fn readonly(mut self, readonly: bool) -> Self {
        self.readonly = readonly;
        self
    }

    /// Enable or disable automatic sync
    pub fn autosync(mut self, enabled: bool) -> Self {
        self.autosync = enabled;
        self
    }

    /// Enable SELinux support
    pub fn selinux(mut self, enabled: bool) -> Self {
        self.selinux = enabled;
        self
    }

    /// Set an identifier for this handle
    pub fn identifier<S: Into<String>>(mut self, id: S) -> Self {
        self.identifier = Some(id.into());
        self
    }

    /// Add a drive in read-write mode
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use guestkit::Guestfs;
    /// let guest = Guestfs::builder()
    ///     .add_drive("disk.img")
    ///     .build()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn add_drive<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.drives.push(DriveConfig {
            path: path.as_ref().to_path_buf(),
            readonly: false,
            format: None,
        });
        self
    }

    /// Add a drive in read-only mode
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use guestkit::Guestfs;
    /// let guest = Guestfs::builder()
    ///     .add_drive_ro("template.img")
    ///     .build()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn add_drive_ro<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.drives.push(DriveConfig {
            path: path.as_ref().to_path_buf(),
            readonly: true,
            format: None,
        });
        self
    }

    /// Add a drive with explicit format
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use guestkit::Guestfs;
    /// let guest = Guestfs::builder()
    ///     .add_drive_with_format("disk.qcow2", "qcow2")
    ///     .build()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn add_drive_with_format<P: AsRef<Path>, S: Into<String>>(
        mut self,
        path: P,
        format: S,
    ) -> Self {
        self.drives.push(DriveConfig {
            path: path.as_ref().to_path_buf(),
            readonly: false,
            format: Some(format.into()),
        });
        self
    }

    /// Add multiple drives at once
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use guestkit::Guestfs;
    /// let guest = Guestfs::builder()
    ///     .add_drives(&["disk1.img", "disk2.img"])
    ///     .build()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn add_drives<P: AsRef<Path>, I: IntoIterator<Item = P>>(mut self, paths: I) -> Self {
        for path in paths {
            self.drives.push(DriveConfig {
                path: path.as_ref().to_path_buf(),
                readonly: false,
                format: None,
            });
        }
        self
    }

    /// Build the Guestfs handle
    ///
    /// # Errors
    ///
    /// Returns an error if the handle cannot be created.
    pub fn build(self) -> Result<Guestfs> {
        let mut guestfs = Guestfs::new()?;

        guestfs.verbose = self.verbose;
        guestfs.trace = self.trace;
        guestfs.readonly = self.readonly;
        guestfs.autosync = self.autosync;
        guestfs.selinux = self.selinux;
        guestfs.identifier = self.identifier;

        // Add all configured drives
        for drive in self.drives {
            guestfs.add_drive_opts(drive.path, drive.readonly, drive.format.as_deref())?;
        }

        Ok(guestfs)
    }

    /// Build and launch the handle in one step
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use guestkit::Guestfs;
    /// let mut guest = Guestfs::builder()
    ///     .add_drive("disk.img")
    ///     .build_and_launch()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn build_and_launch(self) -> Result<Guestfs> {
        let mut guestfs = self.build()?;
        guestfs.launch()?;
        Ok(guestfs)
    }
}

impl Guestfs {
    /// Create a new builder for constructing a Guestfs handle
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use guestkit::Guestfs;
    ///
    /// let mut guest = Guestfs::builder()
    ///     .add_drive("disk.img")
    ///     .verbose(true)
    ///     .build()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn builder() -> GuestfsBuilder {
        GuestfsBuilder::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let builder = GuestfsBuilder::new()
            .verbose(true)
            .trace(true)
            .autosync(false);

        assert!(builder.verbose);
        assert!(builder.trace);
        assert!(!builder.autosync);
    }

    #[test]
    fn test_builder_drives() {
        let builder = GuestfsBuilder::new()
            .add_drive("/tmp/disk1.img")
            .add_drive_ro("/tmp/disk2.img");

        assert_eq!(builder.drives.len(), 2);
        assert!(!builder.drives[0].readonly);
        assert!(builder.drives[1].readonly);
    }

    #[test]
    fn test_builder_fluent() {
        let builder = Guestfs::builder()
            .verbose(true)
            .add_drive("/tmp/test.img")
            .identifier("test-guest");

        assert!(builder.verbose);
        assert_eq!(builder.drives.len(), 1);
        assert_eq!(builder.identifier, Some("test-guest".to_string()));
    }
}
