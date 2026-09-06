// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Offline package installation via chroot, reusing the bind-mount
//! machinery `grub_repair` already uses to run tools inside a mounted
//! guest root.
//!
//! chroot alone shares the host's network namespace, so a package
//! manager *can* reach real repositories — the usual blocker is a stale
//! guest `/etc/resolv.conf` (a build-time leftover that doesn't resolve
//! from this host). `network: true` temporarily swaps in the host's
//! resolver for the duration of the install and restores the guest's
//! original file afterward, regardless of outcome.

use crate::core::{Error, Result};
use crate::guestfs::grub_repair::{chroot_cmd, mount_binds, unmount_binds};
use std::fs;
use std::path::Path;

/// Outcome of an offline package-install attempt.
#[derive(Debug, Clone, Default)]
pub struct PackageInstallReport {
    pub package_manager: String,
    pub packages: Vec<String>,
    pub network_used: bool,
    pub notes: Vec<String>,
}

fn validate_package_name(name: &str) -> Result<()> {
    let first_ok = name
        .chars()
        .next()
        .map(|c| c.is_ascii_alphanumeric())
        .unwrap_or(false);
    let rest_ok = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-' | '_' | ':'));
    if !first_ok || !rest_ok {
        return Err(Error::InvalidFormat(format!(
            "invalid package name: {name:?}"
        )));
    }
    Ok(())
}

fn install_argv(package_format: &str, packages: &[String]) -> Result<String> {
    let pkgs = packages.join(" ");
    Ok(match package_format {
        "deb" => format!(
            "export DEBIAN_FRONTEND=noninteractive; apt-get update -qq && apt-get install -y -qq {pkgs}"
        ),
        "rpm" => format!(
            "(command -v dnf >/dev/null 2>&1 && dnf install -y {pkgs}) || yum install -y {pkgs}"
        ),
        "apk" => format!("apk update -q && apk add -q {pkgs}"),
        "pacman" => format!("pacman -Sy --noconfirm {pkgs}"),
        other => {
            return Err(Error::InvalidOperation(format!(
                "package installation not supported for package format: {other}"
            )));
        }
    })
}

/// Install `packages` into a mounted guest root via chroot.
///
/// `root_mount` must already have its filesystems mounted read-write
/// (matching the same precondition as `grub_repair::repair_grub`).
pub fn install_packages(
    root_mount: &Path,
    packages: &[String],
    package_format: &str,
    network: bool,
    verbose: bool,
) -> Result<PackageInstallReport> {
    if packages.is_empty() {
        return Err(Error::InvalidFormat("no packages specified".into()));
    }
    for p in packages {
        validate_package_name(p)?;
    }
    if !root_mount.is_dir() {
        return Err(Error::InvalidOperation(format!(
            "package-install root is not a directory: {}",
            root_mount.display()
        )));
    }

    let install_cmd = install_argv(package_format, packages)?;

    let mut report = PackageInstallReport {
        package_manager: package_format.to_string(),
        packages: packages.to_vec(),
        network_used: network,
        ..Default::default()
    };

    let resolv_guest = root_mount.join("etc/resolv.conf");
    let resolv_backup = root_mount.join("etc/resolv.conf.pre-guestkit");
    let mut resolv_swapped = false;
    if network {
        if resolv_guest.is_file() && fs::copy(&resolv_guest, &resolv_backup).is_ok() {
            report.notes.push("backed up guest /etc/resolv.conf".into());
        }
        match fs::copy("/etc/resolv.conf", &resolv_guest) {
            Ok(_) => {
                resolv_swapped = true;
                report
                    .notes
                    .push("using host /etc/resolv.conf for DNS during install".into());
            }
            Err(e) => report
                .notes
                .push(format!("could not stage host resolv.conf: {e}")),
        }
    }

    mount_binds(root_mount, verbose)?;
    let result = chroot_cmd(root_mount, &["/bin/sh", "-c", &install_cmd]);
    let _ = unmount_binds(root_mount);

    if resolv_swapped {
        if resolv_backup.is_file() {
            let _ = fs::rename(&resolv_backup, &resolv_guest);
        } else {
            let _ = fs::remove_file(&resolv_guest);
        }
    }

    let output = result?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let msg = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        return Err(Error::CommandFailed(format!(
            "package install ({package_format}) failed: {msg}"
        )));
    }

    report.notes.push(format!(
        "installed via {package_format}: {}",
        packages.join(", ")
    ));
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_package_list() {
        let dir = tempfile::tempdir().unwrap();
        assert!(install_packages(dir.path(), &[], "deb", false, false).is_err());
    }

    #[test]
    fn rejects_shell_metacharacters_in_package_name() {
        assert!(validate_package_name("curl; rm -rf /").is_err());
        assert!(validate_package_name("curl&&true").is_err());
        assert!(validate_package_name("$(whoami)").is_err());
        assert!(validate_package_name("curl").is_ok());
        assert!(validate_package_name("libssl1.1").is_ok());
        assert!(validate_package_name("python3-pip").is_ok());
    }

    #[test]
    fn install_argv_rejects_unknown_package_format() {
        assert!(install_argv("unknown", &["curl".to_string()]).is_err());
    }

    #[test]
    fn install_argv_builds_expected_commands() {
        let pkgs = vec!["curl".to_string(), "tcpdump".to_string()];
        assert!(install_argv("deb", &pkgs)
            .unwrap()
            .contains("apt-get install -y -qq curl tcpdump"));
        assert!(install_argv("rpm", &pkgs)
            .unwrap()
            .contains("dnf install -y curl tcpdump"));
        assert!(install_argv("apk", &pkgs)
            .unwrap()
            .contains("apk add -q curl tcpdump"));
        assert!(install_argv("pacman", &pkgs)
            .unwrap()
            .contains("pacman -Sy --noconfirm curl tcpdump"));
    }

    #[test]
    fn rejects_missing_root_dir() {
        assert!(install_packages(
            Path::new("/nonexistent/guestkit-package-install-test"),
            &["curl".to_string()],
            "deb",
            false,
            false
        )
        .is_err());
    }
}
