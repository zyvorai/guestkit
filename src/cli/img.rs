// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! `guestkit img` — qemu-img owned by GuestKit.

use crate::qemu::QemuImg;
use anyhow::{Context, Result};
use std::path::Path;

pub fn info(image: &Path) -> Result<()> {
    let v = QemuImg::new()
        .info_json(image)
        .with_context(|| format!("qemu-img info {}", image.display()))?;
    println!("{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}

pub fn check(image: &Path, repair: bool) -> Result<()> {
    let report = QemuImg::new()
        .check(image, repair)
        .with_context(|| format!("qemu-img check {}", image.display()))?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if !report.ok {
        anyhow::bail!("image check failed for {}", image.display());
    }
    Ok(())
}

pub fn snapshots(image: &Path) -> Result<()> {
    let v = QemuImg::new().snapshot_list(image)?;
    println!("{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}

pub fn snapshot_create(image: &Path, name: &str) -> Result<()> {
    QemuImg::new().snapshot_create(image, name)?;
    println!("snapshot {name} created on {}", image.display());
    Ok(())
}

pub fn snapshot_delete(image: &Path, name: &str) -> Result<()> {
    QemuImg::new().snapshot_delete(image, name)?;
    println!("snapshot {name} deleted on {}", image.display());
    Ok(())
}

pub fn snapshot_apply(image: &Path, name: &str) -> Result<()> {
    QemuImg::new().snapshot_apply(image, name)?;
    println!("snapshot {name} applied on {}", image.display());
    Ok(())
}

pub fn resize(image: &Path, size: &str) -> Result<()> {
    QemuImg::new().resize(image, size)?;
    println!("resized {} to {size}", image.display());
    Ok(())
}

pub fn rebase(image: &Path, backing: &Path, unsafe_mode: bool) -> Result<()> {
    QemuImg::new().rebase(image, backing, unsafe_mode)?;
    println!("rebased {} onto {}", image.display(), backing.display());
    Ok(())
}

pub fn commit(image: &Path) -> Result<()> {
    QemuImg::new().commit(image)?;
    println!("committed {}", image.display());
    Ok(())
}
