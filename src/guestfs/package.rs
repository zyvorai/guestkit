// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Package management operations for disk image manipulation
//!
//! This implementation provides package inspection capabilities.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;

impl Guestfs {
    /// List Debian packages
    ///
    pub fn dpkg_list(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: dpkg_list");
        }

        // Check if dpkg status file exists
        if !self.exists("/var/lib/dpkg/status")? {
            return Ok(Vec::new());
        }

        let status = self.cat("/var/lib/dpkg/status")?;
        let mut packages = crate::core::mem_optimize::vec_for_packages();
        let mut current_package = String::new();
        let mut current_status = String::new();

        for line in status.lines() {
            if line.starts_with("Package: ") {
                current_package = line.trim_start_matches("Package: ").to_string();
            } else if line.starts_with("Status: ") {
                current_status = line.trim_start_matches("Status: ").to_string();
            } else if line.is_empty() && !current_package.is_empty() {
                // Check if package is installed
                if current_status.contains("install ok installed") {
                    packages.push(current_package.clone());
                }
                current_package.clear();
                current_status.clear();
            }
        }

        Ok(packages)
    }

    /// List RPM packages
    ///
    pub fn rpm_list(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: rpm_list");
        }

        // Check if RPM database exists
        if !self.exists("/var/lib/rpm")? {
            return Ok(Vec::new());
        }

        // Use chroot to run rpm query
        match self.command(&["rpm", "-qa", "--queryformat", "%{NAME}\n"]) {
            Ok(output) => {
                let packages: Vec<String> = output
                    .lines()
                    .map(|s| s.to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                Ok(packages)
            }
            Err(_) => Ok(Vec::new()),
        }
    }

    /// Get package info
    ///
    pub fn get_package_info(&mut self, package: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_package_info {}", package);
        }

        // Try dpkg first
        if self.exists("/var/lib/dpkg/status")? {
            let status = self.cat("/var/lib/dpkg/status")?;
            let mut in_package = false;
            let mut info = String::new();

            for line in status.lines() {
                if line.starts_with("Package: ") && line.contains(package) {
                    in_package = true;
                }
                if in_package {
                    info.push_str(line);
                    info.push('\n');
                    if line.is_empty() {
                        break;
                    }
                }
            }

            if !info.is_empty() {
                return Ok(info);
            }
        }

        // Try RPM
        if self.exists("/var/lib/rpm")? {
            if let Ok(output) = self.command(&["rpm", "-qi", package]) {
                return Ok(output);
            }
        }

        Err(Error::NotFound(format!("Package {} not found", package)))
    }

    /// Check if package is installed
    ///
    pub fn is_package_installed(&mut self, package: &str) -> Result<bool> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: is_package_installed {}", package);
        }

        // Try dpkg
        if self.exists("/var/lib/dpkg/status")? {
            let dpkg_list = self.dpkg_list()?;
            if dpkg_list.contains(&package.to_string()) {
                return Ok(true);
            }
        }

        // Try RPM
        if self.exists("/var/lib/rpm")? && self.command(&["rpm", "-q", package]).is_ok() {
            return Ok(true);
        }

        Ok(false)
    }

    /// List package files
    ///
    pub fn package_files(&mut self, package: &str) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: package_files {}", package);
        }

        // Try dpkg
        if self.exists("/var/lib/dpkg/info")? {
            let list_file = format!("/var/lib/dpkg/info/{}.list", package);
            if self.exists(&list_file)? {
                let content = self.cat(&list_file)?;
                return Ok(content.lines().map(|s| s.to_string()).collect());
            }
        }

        // Try RPM
        if self.exists("/var/lib/rpm")? {
            if let Ok(output) = self.command(&["rpm", "-ql", package]) {
                return Ok(output.lines().map(|s| s.to_string()).collect());
            }
        }

        Err(Error::NotFound(format!("Package {} not found", package)))
    }

    /// Install packages into the mounted guest via chroot.
    ///
    /// Bind-mounts `/proc`, `/sys`, `/dev` for the duration of the install
    /// (same machinery as [`Guestfs::repair_grub`]). `network`, when true,
    /// temporarily swaps the guest's `/etc/resolv.conf` for the host's so
    /// the package manager can resolve real repositories — the guest's
    /// original file is restored afterward regardless of outcome.
    pub fn install_packages(
        &mut self,
        packages: &[String],
        network: bool,
    ) -> Result<crate::guestfs::package_install::PackageInstallReport> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!(
                "guestfs: install_packages {:?} network={}",
                packages, network
            );
        }

        let roots = self.inspect_os()?;
        let root_id = roots
            .first()
            .ok_or_else(|| Error::NotFound("No operating system found in image".into()))?
            .clone();
        let package_format = self.inspect_get_package_format(&root_id)?;
        let root_mount = std::path::PathBuf::from(self.find_root_mountpoint()?);

        crate::guestfs::package_install::install_packages(
            &root_mount,
            packages,
            &package_format,
            network,
            self.verbose,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
