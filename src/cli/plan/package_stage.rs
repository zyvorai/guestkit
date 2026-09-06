// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Offline PackageInstall staging into the guest for first-boot install.
//!
//! Live `dnf`/`apt` still requires a running guest. When matching `.rpm` / `.deb`
//! files exist under `GUESTKIT_PACKAGE_CACHE` (or `PackageInstall.host_cache`),
//! offline apply copies them into the guest and enables a oneshot systemd unit
//! that installs on first boot.
//!
//! With `GUESTKIT_PACKAGE_FETCH=1`, missing packages are downloaded on the host
//! (`dnf download` / `yumdownloader` / `apt-get download`) into the cache (or
//! `~/.cache/guestkit/packages`) before staging.

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

use crate::cli::plan::package_fetch::{
    detect_package_kind, ensure_fetch_cache_dirs, fetch_enabled, fetch_packages,
};
use crate::cli::plan::types::PackageInstall;

const PENDING_DIR: &str = "/var/cache/guestkit/pending";
const INSTALL_SCRIPT: &str = "/usr/lib/guestkit/firstboot-packages.sh";
const UNIT_PATH: &str = "/etc/systemd/system/guestkit-firstboot-packages.service";
const WANTS_LINK: &str =
    "/etc/systemd/system/multi-user.target.wants/guestkit-firstboot-packages.service";

/// Host directories to search for package files.
pub fn package_cache_dirs(pi: &PackageInstall) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(h) = &pi.host_cache {
        let p = PathBuf::from(h);
        if p.is_dir() {
            dirs.push(p);
        }
    }
    if let Ok(env) = std::env::var("GUESTKIT_PACKAGE_CACHE") {
        for part in env.split(':') {
            let p = PathBuf::from(part.trim());
            if !p.as_os_str().is_empty() && p.is_dir() {
                dirs.push(p);
            }
        }
    }
    dirs
}

/// Locate a host package file for `name` (e.g. `fail2ban` → `fail2ban-1.0.rpm`).
pub fn find_package_file(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let name_l = name.to_ascii_lowercase();
    let mut candidates = Vec::new();
    for dir in dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let fname = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let ok_ext = fname.ends_with(".rpm")
                || fname.ends_with(".deb")
                || fname.ends_with(".pkg.tar.zst")
                || fname.ends_with(".pkg.tar.xz");
            if !ok_ext {
                continue;
            }
            // Exact prefix match: fail2ban.rpm, fail2ban_1.deb, fail2ban-1.2.rpm
            let stem_ok = fname == format!("{name_l}.rpm")
                || fname == format!("{name_l}.deb")
                || fname.starts_with(&format!("{name_l}-"))
                || fname.starts_with(&format!("{name_l}_"));
            if stem_ok {
                candidates.push(path);
            }
        }
    }
    // Prefer newest mtime when multiple versions exist.
    candidates.sort_by_key(|p| {
        std::cmp::Reverse(
            fs::metadata(p)
                .and_then(|m| m.modified())
                .ok()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
        )
    });
    candidates.into_iter().next()
}

/// True when packages can be staged offline (cache hit, or host fetch enabled).
pub fn can_stage_offline(pi: &PackageInstall) -> bool {
    let dirs = package_cache_dirs(pi);
    if dirs.is_empty() {
        return fetch_enabled();
    }
    if pi
        .packages
        .iter()
        .all(|pkg| find_package_file(pkg, &dirs).is_some())
    {
        return true;
    }
    fetch_enabled()
}

/// Stage packages + first-boot installer into the offline guest.
pub fn stage_packages_offline(
    g: &mut crate::guestfs::Guestfs,
    pi: &PackageInstall,
) -> Result<bool> {
    let mut dirs = ensure_fetch_cache_dirs(package_cache_dirs(pi));
    if dirs.is_empty() {
        eprintln!(
            "Warning: PackageInstall ({}) skipped offline — set GUESTKIT_PACKAGE_CACHE \
             or host_cache to a directory of .rpm/.deb files, or GUESTKIT_PACKAGE_FETCH=1 \
             to download on the host",
            pi.packages.join(", ")
        );
        return Ok(false);
    }

    if fetch_enabled() {
        let missing: Vec<String> = pi
            .packages
            .iter()
            .filter(|pkg| find_package_file(pkg, &dirs).is_none())
            .cloned()
            .collect();
        if !missing.is_empty() {
            let dest = dirs[0].clone();
            let kind = detect_package_kind(g);
            eprintln!(
                "GUESTKIT_PACKAGE_FETCH: downloading {} package(s) ({kind:?}) into {}",
                missing.len(),
                dest.display()
            );
            let _ = fetch_packages(&missing, &dest, kind)?;
            dirs = ensure_fetch_cache_dirs(package_cache_dirs(pi));
            if dirs.is_empty() {
                dirs.push(dest);
            }
        }
    }

    let mut staged: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    g.mkdir_p(PENDING_DIR)
        .map_err(|e| anyhow::anyhow!("mkdir {PENDING_DIR}: {e}"))?;

    for pkg in &pi.packages {
        match find_package_file(pkg, &dirs) {
            Some(host_path) => {
                let fname = host_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .ok_or_else(|| anyhow::anyhow!("bad package filename {}", host_path.display()))?
                    .to_string();
                let remote = format!("{PENDING_DIR}/{fname}");
                g.upload(host_path.to_str().unwrap(), &remote)
                    .with_context(|| format!("upload {} → {}", host_path.display(), remote))?;
                staged.push(fname);
            }
            None => missing.push(pkg.clone()),
        }
    }

    if staged.is_empty() {
        eprintln!(
            "Warning: PackageInstall — no matching files in cache for [{}]",
            pi.packages.join(", ")
        );
        return Ok(false);
    }
    if !missing.is_empty() {
        eprintln!(
            "Warning: PackageInstall — missing from cache (not staged): {}",
            missing.join(", ")
        );
    }

    g.mkdir_p("/usr/lib/guestkit")
        .map_err(|e| anyhow::anyhow!("mkdir /usr/lib/guestkit: {e}"))?;
    g.mkdir_p("/etc/systemd/system")
        .map_err(|e| anyhow::anyhow!("mkdir systemd: {e}"))?;
    g.mkdir_p("/etc/systemd/system/multi-user.target.wants")
        .map_err(|e| anyhow::anyhow!("mkdir wants: {e}"))?;

    let script = firstboot_script();
    g.write(INSTALL_SCRIPT, script.as_bytes())
        .map_err(|e| anyhow::anyhow!("write {INSTALL_SCRIPT}: {e}"))?;
    // Best-effort executable bit via chmod if available
    let _ = g.chmod(0o755, INSTALL_SCRIPT);

    let unit = firstboot_unit();
    g.write(UNIT_PATH, unit.as_bytes())
        .map_err(|e| anyhow::anyhow!("write {UNIT_PATH}: {e}"))?;

    g.ln_sf("../guestkit-firstboot-packages.service", WANTS_LINK)
        .or_else(|_| {
            g.ln_sf(
                "/etc/systemd/system/guestkit-firstboot-packages.service",
                WANTS_LINK,
            )
        })
        .map_err(|e| anyhow::anyhow!("enable firstboot unit: {e}"))?;

    eprintln!(
        "Staged {} package(s) for first-boot install: {}",
        staged.len(),
        staged.join(", ")
    );
    Ok(true)
}

fn firstboot_script() -> String {
    r#"#!/bin/bash
set -euo pipefail
PENDING=/var/cache/guestkit/pending
shopt -s nullglob
pkgs=("$PENDING"/*)
if [ ${#pkgs[@]} -eq 0 ]; then
  exit 0
fi
for f in "${pkgs[@]}"; do
  case "$f" in
    *.rpm)
      if command -v dnf >/dev/null 2>&1; then dnf -y install "$f"
      elif command -v yum >/dev/null 2>&1; then yum -y localinstall "$f"
      else rpm -Uvh "$f"
      fi
      ;;
    *.deb)
      if command -v apt-get >/dev/null 2>&1; then
        DEBIAN_FRONTEND=noninteractive apt-get install -y "$f" || dpkg -i "$f" || apt-get install -yf
      else
        dpkg -i "$f"
      fi
      ;;
    *.pkg.tar.zst|*.pkg.tar.xz)
      if command -v pacman >/dev/null 2>&1; then pacman -U --noconfirm "$f"; fi
      ;;
  esac
done
rm -f "$PENDING"/*
systemctl disable guestkit-firstboot-packages.service >/dev/null 2>&1 || true
rm -f /etc/systemd/system/multi-user.target.wants/guestkit-firstboot-packages.service
"#
    .to_string()
}

fn firstboot_unit() -> String {
    format!(
        r#"[Unit]
Description=GuestKit first-boot package install from offline stage
After=network-online.target
Wants=network-online.target
ConditionPathExists={PENDING_DIR}

[Service]
Type=oneshot
ExecStart={INSTALL_SCRIPT}
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
"#
    )
}

/// Summarize staging availability for preview/CLI.
pub fn stage_preview_note(pi: &PackageInstall) -> String {
    let dirs = package_cache_dirs(pi);
    if dirs.is_empty() {
        if fetch_enabled() {
            return "offline: will host-fetch (GUESTKIT_PACKAGE_FETCH) then stage for first-boot"
                .into();
        }
        return "offline: live-only (set GUESTKIT_PACKAGE_CACHE or GUESTKIT_PACKAGE_FETCH=1)"
            .into();
    }
    let mut ok = 0usize;
    let mut miss = Vec::new();
    for pkg in &pi.packages {
        if find_package_file(pkg, &dirs).is_some() {
            ok += 1;
        } else {
            miss.push(pkg.as_str());
        }
    }
    if miss.is_empty() && ok > 0 {
        format!("offline: will stage {ok} package(s) for first-boot install")
    } else if ok > 0 {
        if fetch_enabled() {
            format!(
                "offline: stage {ok}; host-fetch missing: {}",
                miss.join(", ")
            )
        } else {
            format!(
                "offline: stage {ok} package(s); missing: {}",
                miss.join(", ")
            )
        }
    } else if fetch_enabled() {
        format!(
            "offline: will host-fetch then stage ({})",
            pi.packages.join(", ")
        )
    } else {
        format!(
            "offline: live-only (no cache match for {})",
            pi.packages.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn find_package_matches_rpm_and_deb() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("fail2ban-1.0.2-1.el9.noarch.rpm"), b"rpm").unwrap();
        fs::write(dir.path().join("aide_0.17.deb"), b"deb").unwrap();
        let dirs = vec![dir.path().to_path_buf()];
        let fail = find_package_file("fail2ban", &dirs).unwrap();
        assert!(fail
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with(".rpm"));
        let aide = find_package_file("aide", &dirs).unwrap();
        assert!(aide
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with(".deb"));
        assert!(find_package_file("nosuch", &dirs).is_none());
    }

    #[test]
    fn can_stage_requires_all_packages() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("fail2ban-1.rpm"), b"x").unwrap();
        std::env::set_var("GUESTKIT_PACKAGE_CACHE", dir.path());
        std::env::remove_var("GUESTKIT_PACKAGE_FETCH");
        let pi = PackageInstall {
            packages: vec!["fail2ban".into()],
            estimated_size: None,
            host_cache: None,
        };
        assert!(can_stage_offline(&pi));
        let pi2 = PackageInstall {
            packages: vec!["fail2ban".into(), "aide".into()],
            estimated_size: None,
            host_cache: None,
        };
        assert!(!can_stage_offline(&pi2));
        std::env::set_var("GUESTKIT_PACKAGE_FETCH", "1");
        assert!(can_stage_offline(&pi2));
        std::env::remove_var("GUESTKIT_PACKAGE_CACHE");
        std::env::remove_var("GUESTKIT_PACKAGE_FETCH");
    }
}
