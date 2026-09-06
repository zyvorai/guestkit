// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! CLI for selinux-relabel, sysprep, bitlocker escrow.

use crate::cli::plan::cutover_prep::{
    bitlocker_escrow, default_escrow_path, selinux_relabel_plan, windows_sysprep_plan,
};
use crate::cli::plan::types::FixPlan;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub fn write_plan(plan: &FixPlan, output: &Path) -> Result<()> {
    let ext = output
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("yaml");
    let body = if ext.eq_ignore_ascii_case("json") {
        serde_json::to_string_pretty(plan)?
    } else {
        serde_yaml::to_string(plan)?
    };
    if let Some(dir) = output.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }
    std::fs::write(output, body)?;
    println!("wrote plan {}", output.display());
    Ok(())
}

pub fn selinux_relabel(image: &Path, export: Option<&Path>) -> Result<()> {
    let plan = selinux_relabel_plan(&image.display().to_string());
    let dest = export
        .map(Path::to_path_buf)
        .unwrap_or_else(|| image.with_extension("selinux-relabel.yaml"));
    write_plan(&plan, &dest)
}

pub fn sysprep(
    image: &Path,
    hostname: Option<&str>,
    firstboot: bool,
    export: Option<&Path>,
) -> Result<()> {
    let plan = windows_sysprep_plan(&image.display().to_string(), hostname, firstboot);
    let dest = export
        .map(Path::to_path_buf)
        .unwrap_or_else(|| image.with_extension("sysprep.yaml"));
    write_plan(&plan, &dest)
}

pub fn bitlocker_status(image: &Path, verbose: bool) -> Result<()> {
    #[cfg(not(target_os = "windows"))]
    {
        use crate::assurance::{boot_target_from_str, collect_assurance_data};
        match collect_assurance_data(image, boot_target_from_str("kvm"), verbose) {
            Ok((ev, _)) => {
                if let Some(win) = ev.windows {
                    println!("bitlocker_detected: {}", win.bitlocker_detected);
                    if let Some(st) = win.bitlocker {
                        println!("any_protected: {}", st.any_protected);
                        println!("offline_uncertain: {}", st.offline_uncertain);
                        for v in st.volumes {
                            println!("  volume {} protection={}", v.mount_point, v.protection);
                        }
                    } else {
                        println!("no BitLockerState on evidence");
                    }
                } else {
                    println!("not a Windows image (or Windows evidence missing)");
                }
            }
            Err(e) => {
                anyhow::bail!("could not inspect {}: {e}", image.display());
            }
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        let _ = (image, verbose);
        anyhow::bail!("bitlocker status is a host-side offline command");
    }
}

pub fn bitlocker_escrow_cmd(
    image: &Path,
    key_file: &Path,
    output: Option<&Path>,
    include_secret: bool,
    export_plan: Option<&Path>,
) -> Result<()> {
    let dest = output
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_escrow_path(image));
    let (record, plan) = bitlocker_escrow(image, key_file, include_secret, &dest)?;
    println!(
        "escrow {} (sha256={} bytes={})",
        dest.display(),
        record.key_sha256,
        record.key_bytes
    );
    if let Some(p) = export_plan {
        write_plan(&plan, p)?;
    } else {
        write_plan(&plan, &image.with_extension("bitlocker-plan.yaml"))?;
    }
    Ok(())
}

pub fn default_plan_path(image: &Path, suffix: &str) -> PathBuf {
    image.with_extension(suffix)
}
