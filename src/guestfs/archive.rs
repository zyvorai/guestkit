// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Archive operations (tar, tgz, cpio)
//!
//! This implementation uses system tar command with mounted filesystems.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::path::Path;
use std::process::Command;

impl Guestfs {
    /// Extract tar archive into directory
    ///
    ///
    /// # Arguments
    ///
    /// * `tarfile` - Path to tar archive on host
    /// * `directory` - Directory in guest to extract to
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use guestkit::guestfs::Guestfs;
    ///
    /// let mut g = Guestfs::new().unwrap();
    /// // ... setup and mount ...
    ///
    /// g.tar_in("/path/to/archive.tar", "/target/dir").unwrap();
    /// ```
    pub fn tar_in<P: AsRef<Path>>(&mut self, tarfile: P, directory: &str) -> Result<()> {
        self.ensure_ready()?;

        let tarfile = tarfile.as_ref();

        if self.verbose {
            eprintln!("guestfs: tar_in {} {}", tarfile.display(), directory);
        }

        // Verify tar file exists
        if !tarfile.exists() {
            return Err(Error::NotFound(format!(
                "Tar file not found: {}",
                tarfile.display()
            )));
        }

        // Get root mount point
        let root_mountpoint = self
            .mounted
            .get("/dev/sda1")
            .or_else(|| self.mounted.get("/dev/sda2"))
            .or_else(|| self.mounted.get("/dev/vda1"))
            .or_else(|| self.mounted.values().next())
            .ok_or_else(|| {
                Error::InvalidState("No filesystem mounted. Call mount_ro() first.".to_string())
            })?;

        // Build target directory path
        let directory_clean = directory.trim_start_matches('/');
        let target_path = std::path::PathBuf::from(root_mountpoint).join(directory_clean);

        // Ensure target directory exists
        std::fs::create_dir_all(&target_path).map_err(|e| {
            Error::CommandFailed(format!("Failed to create target directory: {}", e))
        })?;

        // Extract tar
        let output = Command::new("tar")
            .arg("-xf")
            .arg(tarfile)
            .arg("-C")
            .arg(&target_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to run tar: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!(
                "Tar extraction failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Create tar archive from directory
    ///
    pub fn tar_out<P: AsRef<Path>>(&mut self, directory: &str, tarfile: P) -> Result<()> {
        self.ensure_ready()?;

        let tarfile = tarfile.as_ref();

        if self.verbose {
            eprintln!("guestfs: tar_out {} {}", directory, tarfile.display());
        }

        // Get root mount point
        let root_mountpoint = self
            .mounted
            .get("/dev/sda1")
            .or_else(|| self.mounted.get("/dev/sda2"))
            .or_else(|| self.mounted.get("/dev/vda1"))
            .or_else(|| self.mounted.values().next())
            .ok_or_else(|| {
                Error::InvalidState("No filesystem mounted. Call mount_ro() first.".to_string())
            })?;

        // Build source directory path
        let directory_clean = directory.trim_start_matches('/');
        let source_path = std::path::PathBuf::from(root_mountpoint).join(directory_clean);

        // Verify source exists
        if !source_path.exists() {
            return Err(Error::NotFound(format!(
                "Directory not found: {}",
                directory
            )));
        }

        // Create tar archive
        let output = Command::new("tar")
            .arg("-cf")
            .arg(tarfile)
            .arg("-C")
            .arg(&source_path)
            .arg(".")
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to run tar: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!(
                "Tar creation failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Extract compressed tar archive
    ///
    pub fn tgz_in<P: AsRef<Path>>(&mut self, tarball: P, directory: &str) -> Result<()> {
        self.ensure_ready()?;

        let tarball = tarball.as_ref();

        if self.verbose {
            eprintln!("guestfs: tgz_in {} {}", tarball.display(), directory);
        }

        // Verify tar file exists
        if !tarball.exists() {
            return Err(Error::NotFound(format!(
                "Tar file not found: {}",
                tarball.display()
            )));
        }

        // Get root mount point
        let root_mountpoint = self
            .mounted
            .get("/dev/sda1")
            .or_else(|| self.mounted.get("/dev/sda2"))
            .or_else(|| self.mounted.get("/dev/vda1"))
            .or_else(|| self.mounted.values().next())
            .ok_or_else(|| {
                Error::InvalidState("No filesystem mounted. Call mount_ro() first.".to_string())
            })?;

        // Build target directory path
        let directory_clean = directory.trim_start_matches('/');
        let target_path = std::path::PathBuf::from(root_mountpoint).join(directory_clean);

        // Ensure target directory exists
        std::fs::create_dir_all(&target_path).map_err(|e| {
            Error::CommandFailed(format!("Failed to create target directory: {}", e))
        })?;

        // Extract compressed tar
        let output = Command::new("tar")
            .arg("-xzf")
            .arg(tarball)
            .arg("-C")
            .arg(&target_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to run tar: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!(
                "Tar extraction failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Create compressed tar archive
    ///
    pub fn tgz_out<P: AsRef<Path>>(&mut self, directory: &str, tarball: P) -> Result<()> {
        self.ensure_ready()?;

        let tarball = tarball.as_ref();

        if self.verbose {
            eprintln!("guestfs: tgz_out {} {}", directory, tarball.display());
        }

        // Get root mount point
        let root_mountpoint = self
            .mounted
            .get("/dev/sda1")
            .or_else(|| self.mounted.get("/dev/sda2"))
            .or_else(|| self.mounted.get("/dev/vda1"))
            .or_else(|| self.mounted.values().next())
            .ok_or_else(|| {
                Error::InvalidState("No filesystem mounted. Call mount_ro() first.".to_string())
            })?;

        // Build source directory path
        let directory_clean = directory.trim_start_matches('/');
        let source_path = std::path::PathBuf::from(root_mountpoint).join(directory_clean);

        // Verify source exists
        if !source_path.exists() {
            return Err(Error::NotFound(format!(
                "Directory not found: {}",
                directory
            )));
        }

        // Create compressed tar archive
        let output = Command::new("tar")
            .arg("-czf")
            .arg(tarball)
            .arg("-C")
            .arg(&source_path)
            .arg(".")
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to run tar: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!(
                "Tar creation failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Extract tar with options
    ///
    pub fn tar_in_opts<P: AsRef<Path>>(
        &mut self,
        tarfile: P,
        directory: &str,
        compress: Option<&str>,
        xattrs: bool,
        selinux: bool,
        acls: bool,
    ) -> Result<()> {
        self.ensure_ready()?;

        let tarfile = tarfile.as_ref();

        if self.verbose {
            eprintln!(
                "guestfs: tar_in_opts {} {} compress={:?} xattrs={} selinux={} acls={}",
                tarfile.display(),
                directory,
                compress,
                xattrs,
                selinux,
                acls
            );
        }

        // Get root mount point
        let root_mountpoint = self
            .mounted
            .get("/dev/sda1")
            .or_else(|| self.mounted.get("/dev/sda2"))
            .or_else(|| self.mounted.get("/dev/vda1"))
            .or_else(|| self.mounted.values().next())
            .ok_or_else(|| {
                Error::InvalidState("No filesystem mounted. Call mount_ro() first.".to_string())
            })?;

        // Build target directory path
        let directory_clean = directory.trim_start_matches('/');
        let target_path = std::path::PathBuf::from(root_mountpoint).join(directory_clean);

        // Build tar command with options
        let mut cmd = Command::new("tar");

        // Add compression flag if specified
        match compress {
            Some("gzip") | Some("gz") => {
                cmd.arg("-z");
            }
            Some("bzip2") | Some("bz2") => {
                cmd.arg("-j");
            }
            Some("xz") => {
                cmd.arg("-J");
            }
            Some("compress") => {
                cmd.arg("-Z");
            }
            _ => {}
        }

        cmd.arg("-xf").arg(tarfile);
        cmd.arg("-C").arg(&target_path);

        // Add extended attributes options
        if xattrs {
            cmd.arg("--xattrs");
        }
        if selinux {
            cmd.arg("--selinux");
        }
        if acls {
            cmd.arg("--acls");
        }

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to run tar: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!(
                "Tar extraction failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Create tar with options
    #[allow(clippy::too_many_arguments)]
    pub fn tar_out_opts<P: AsRef<Path>>(
        &mut self,
        directory: &str,
        tarfile: P,
        compress: Option<&str>,
        _numericowner: bool,
        xattrs: bool,
        selinux: bool,
        acls: bool,
    ) -> Result<()> {
        self.ensure_ready()?;

        let tarfile = tarfile.as_ref();

        if self.verbose {
            eprintln!(
                "guestfs: tar_out_opts {} {} compress={:?} xattrs={} selinux={} acls={}",
                directory,
                tarfile.display(),
                compress,
                xattrs,
                selinux,
                acls
            );
        }

        // Get root mount point
        let root_mountpoint = self
            .mounted
            .get("/dev/sda1")
            .or_else(|| self.mounted.get("/dev/sda2"))
            .or_else(|| self.mounted.get("/dev/vda1"))
            .or_else(|| self.mounted.values().next())
            .ok_or_else(|| {
                Error::InvalidState("No filesystem mounted. Call mount_ro() first.".to_string())
            })?;

        // Build source directory path
        let directory_clean = directory.trim_start_matches('/');
        let source_path = std::path::PathBuf::from(root_mountpoint).join(directory_clean);

        // Verify source exists
        if !source_path.exists() {
            return Err(Error::NotFound(format!(
                "Directory not found: {}",
                directory
            )));
        }

        // Build tar command with options
        let mut cmd = Command::new("tar");

        // Add compression flag if specified
        match compress {
            Some("gzip") | Some("gz") => {
                cmd.arg("-z");
            }
            Some("bzip2") | Some("bz2") => {
                cmd.arg("-j");
            }
            Some("xz") => {
                cmd.arg("-J");
            }
            Some("compress") => {
                cmd.arg("-Z");
            }
            _ => {}
        }

        cmd.arg("-cf").arg(tarfile);
        cmd.arg("-C").arg(&source_path);

        // Add extended attributes options
        if xattrs {
            cmd.arg("--xattrs");
        }
        if selinux {
            cmd.arg("--selinux");
        }
        if acls {
            cmd.arg("--acls");
        }

        cmd.arg(".");

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to run tar: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!(
                "Tar creation failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Extract CPIO archive into directory
    ///
    pub fn cpio_in<P: AsRef<Path>>(&mut self, cpiofile: P, directory: &str) -> Result<()> {
        self.ensure_ready()?;

        let cpiofile = cpiofile.as_ref();

        if self.verbose {
            eprintln!("guestfs: cpio_in {} {}", cpiofile.display(), directory);
        }

        // Verify CPIO file exists
        if !cpiofile.exists() {
            return Err(Error::NotFound(format!(
                "CPIO file not found: {}",
                cpiofile.display()
            )));
        }

        // Get root mount point
        let root_mountpoint = self
            .mounted
            .get("/dev/sda1")
            .or_else(|| self.mounted.get("/dev/sda2"))
            .or_else(|| self.mounted.get("/dev/vda1"))
            .or_else(|| self.mounted.values().next())
            .ok_or_else(|| {
                Error::InvalidState("No filesystem mounted. Call mount_ro() first.".to_string())
            })?;

        // Build target directory path
        let directory_clean = directory.trim_start_matches('/');
        let target_path = std::path::PathBuf::from(root_mountpoint).join(directory_clean);

        // Ensure target directory exists
        std::fs::create_dir_all(&target_path).map_err(|e| {
            Error::CommandFailed(format!("Failed to create target directory: {}", e))
        })?;

        // Extract CPIO archive
        // cpio -idm < archive.cpio
        let cpio_data = std::fs::read(cpiofile).map_err(Error::Io)?;

        let mut cmd = Command::new("cpio");
        cmd.arg("-idm")
            .arg("-D")
            .arg(&target_path)
            .stdin(std::process::Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| Error::CommandFailed(format!("Failed to spawn cpio: {}", e)))?;

        // Write CPIO data to cpio's stdin
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(&cpio_data).map_err(Error::Io)?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| Error::CommandFailed(format!("Failed to wait for cpio: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!(
                "CPIO extraction failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Create cpio archive
    ///
    pub fn cpio_out<P: AsRef<Path>>(
        &mut self,
        directory: &str,
        cpiofile: P,
        format: &str,
    ) -> Result<()> {
        self.ensure_ready()?;

        let cpiofile = cpiofile.as_ref();

        if self.verbose {
            eprintln!(
                "guestfs: cpio_out {} {} {}",
                directory,
                cpiofile.display(),
                format
            );
        }

        // Get root mount point
        let root_mountpoint = self
            .mounted
            .get("/dev/sda1")
            .or_else(|| self.mounted.get("/dev/sda2"))
            .or_else(|| self.mounted.get("/dev/vda1"))
            .or_else(|| self.mounted.values().next())
            .ok_or_else(|| {
                Error::InvalidState("No filesystem mounted. Call mount_ro() first.".to_string())
            })?;

        // Build source directory path
        let directory_clean = directory.trim_start_matches('/');
        let source_path = std::path::PathBuf::from(root_mountpoint).join(directory_clean);

        // Verify source exists
        if !source_path.exists() {
            return Err(Error::NotFound(format!(
                "Directory not found: {}",
                directory
            )));
        }

        // Build cpio format flag
        let format_flag = match format {
            "newc" => "--format=newc",
            "crc" => "--format=crc",
            "odc" => "--format=odc",
            "bin" => "--format=bin",
            "tar" => "--format=tar",
            _ => "--format=newc", // default
        };

        // Use find + cpio to create archive
        // find . -print | cpio -o --format=newc > archive.cpio
        let find_output = Command::new("find")
            .current_dir(&source_path)
            .arg(".")
            .arg("-print")
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to run find: {}", e)))?;

        if !find_output.status.success() {
            return Err(Error::CommandFailed(format!(
                "Find failed: {}",
                String::from_utf8_lossy(&find_output.stderr)
            )));
        }

        // Pass find output to cpio
        let mut cpio_cmd = Command::new("cpio");
        cpio_cmd
            .arg("-o")
            .arg(format_flag)
            .arg("-O")
            .arg(cpiofile)
            .stdin(std::process::Stdio::piped())
            .current_dir(&source_path);

        let mut cpio_process = cpio_cmd
            .spawn()
            .map_err(|e| Error::CommandFailed(format!("Failed to spawn cpio: {}", e)))?;

        // Write find output to cpio's stdin
        if let Some(mut stdin) = cpio_process.stdin.take() {
            use std::io::Write;
            stdin.write_all(&find_output.stdout).map_err(|e| {
                Error::CommandFailed(format!("Failed to write to cpio stdin: {}", e))
            })?;
        }

        let cpio_output = cpio_process
            .wait_with_output()
            .map_err(|e| Error::CommandFailed(format!("Failed to wait for cpio: {}", e)))?;

        if !cpio_output.status.success() {
            return Err(Error::CommandFailed(format!(
                "CPIO creation failed: {}",
                String::from_utf8_lossy(&cpio_output.stderr)
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archive_api_exists() {
        let g = Guestfs::new().unwrap();
        let _ = g;
    }
}
