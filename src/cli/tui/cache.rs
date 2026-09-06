// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! On-disk inspect cache for faster repeat TUI opens.

use crate::cli::profiles::ProfileReport;
use crate::guestfs::inspect_enhanced::{
    FirewallInfo, NetworkInterface, PackageInfo, SecurityInfo, SystemService, UserAccount,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

pub const CACHE_VERSION: u32 = 1;

/// Cache directory: `~/.cache/guestkit/<hash>/`
pub fn cache_dir_for_image(image: &Path) -> Result<Option<PathBuf>> {
    let meta = match fs::metadata(image) {
        Ok(m) => m,
        Err(_) => return Ok(None),
    };
    let mtime = file_mtime_secs(&meta);

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    image.display().to_string().hash(&mut hasher);
    mtime.hash(&mut hasher);
    let hash = format!("{:016x}", hasher.finish());

    let base = dirs::cache_dir().context("cache dir")?;
    Ok(Some(base.join("guestkit").join(hash)))
}

fn file_mtime_secs(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn cache_path(image: &Path) -> Result<Option<PathBuf>> {
    Ok(cache_dir_for_image(image)?.map(|d| d.join("inspect.json")))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectCacheSnapshot {
    pub version: u32,
    pub image_mtime: u64,
    pub os_name: String,
    pub os_version: String,
    pub hostname: String,
    pub kernel_version: String,
    pub architecture: String,
    pub init_system: String,
    pub timezone: String,
    pub locale: String,
    pub network_interfaces: Vec<NetworkInterface>,
    pub dns_servers: Vec<String>,
    pub packages: PackageInfo,
    pub services: Vec<SystemService>,
    pub firewall: FirewallInfo,
    pub security: SecurityInfo,
    pub users: Vec<UserAccount>,
    pub security_profile: Option<ProfileReport>,
    pub compliance_profile: Option<ProfileReport>,
    pub hardening_profile: Option<ProfileReport>,
}

pub fn load_snapshot(image: &Path) -> Result<Option<InspectCacheSnapshot>> {
    let Some(path) = cache_path(image)? else {
        return Ok(None);
    };
    if !path.exists() {
        return Ok(None);
    }
    let meta = fs::metadata(image)?;
    let mtime = file_mtime_secs(&meta);
    let data = fs::read_to_string(&path)?;
    let snap: InspectCacheSnapshot = serde_json::from_str(&data)?;
    if snap.version != CACHE_VERSION || snap.image_mtime != mtime {
        return Ok(None);
    }
    Ok(Some(snap))
}

pub fn save_snapshot(image: &Path, snap: &InspectCacheSnapshot) -> Result<()> {
    let Some(dir) = cache_dir_for_image(image)? else {
        return Ok(());
    };
    fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(snap)?;
    fs::write(dir.join("inspect.json"), json)?;
    fs::write(dir.join("inspect.ok"), b"ok")?;
    Ok(())
}

pub fn read_cached_flag(image: &Path) -> bool {
    if let Ok(Some(dir)) = cache_dir_for_image(image) {
        dir.join("inspect.ok").exists()
    } else {
        false
    }
}

pub fn write_cached_flag(image: &Path) -> Result<()> {
    if let Some(dir) = cache_dir_for_image(image)? {
        fs::create_dir_all(&dir)?;
        fs::write(dir.join("inspect.ok"), b"ok")?;
    }
    Ok(())
}
