// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Windows VSS requestor (diskshadow-based).
//!
//! Creating a shadow copy through diskshadow drives the full VSS writer
//! protocol (freeze → shadow → thaw), which is the application-consistent
//! moment: SQL Server, Exchange, AD and every other registered writer
//! flushes and quiesces for the shadow creation. The host storage snapshot
//! taken while our marker shadow exists is therefore app-consistent for
//! VSS-aware applications. The marker shadow is deleted on completion.
//!
//! This intentionally avoids an in-process COM IVssBackupComponents
//! implementation; diskshadow ships on all supported client and server
//! SKUs and exercises the same writer path.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VssSnapshotResult {
    pub created: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow_id: Option<String>,
    pub writers_total: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writers_failed: Vec<String>,
    /// True when every writer reported stable after shadow creation.
    pub app_consistent: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[cfg(target_os = "windows")]
pub fn create_marker_shadow(volume: &str) -> anyhow::Result<VssSnapshotResult> {
    use std::io::Write;
    use std::process::Command;

    let script_path = std::env::temp_dir().join("guestkit-vss.dsh");
    let mut script = std::fs::File::create(&script_path)?;
    // Volatile context: the shadow disappears when deleted/on reboot; we
    // only need the writer-quiesced moment plus a short validity window.
    writeln!(script, "SET CONTEXT VOLATILE")?;
    writeln!(script, "SET VERBOSE ON")?;
    writeln!(script, "BEGIN BACKUP")?;
    writeln!(script, "ADD VOLUME {volume} ALIAS guestkit_marker")?;
    writeln!(script, "CREATE")?;
    writeln!(script, "END BACKUP")?;
    drop(script);

    let output = Command::new("diskshadow")
        .args(["/s", &script_path.display().to_string()])
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let _ = std::fs::remove_file(&script_path);
    if !output.status.success() {
        anyhow::bail!(
            "diskshadow failed: {}",
            stdout.lines().rev().take(5).collect::<Vec<_>>().join(" | ")
        );
    }

    let shadow_id = stdout
        .lines()
        .find(|l| l.contains("Shadow copy ID") || l.contains("shadow copy ID"))
        .and_then(|l| l.split('{').nth(1))
        .and_then(|s| s.split('}').next())
        .map(|s| format!("{{{s}}}"));

    // Writer state after creation is the consistency verdict.
    let vss = crate::collectors::windows_live::collect_vss_health().unwrap_or_default();
    Ok(VssSnapshotResult {
        created: true,
        shadow_id,
        writers_total: vss.writers_total,
        app_consistent: vss.healthy,
        writers_failed: vss.writers_failed,
        detail: None,
    })
}

#[cfg(target_os = "windows")]
pub fn delete_marker_shadow(shadow_id: &str) -> anyhow::Result<String> {
    use std::process::Command;
    let output = Command::new("vssadmin")
        .args([
            "delete",
            "shadows",
            &format!("/Shadow={shadow_id}"),
            "/Quiet",
        ])
        .output()?;
    if output.status.success() {
        Ok(format!("shadow {shadow_id} deleted"))
    } else {
        // Volatile shadows self-clean; report but don't fail the flow.
        Ok(format!(
            "shadow {shadow_id} delete returned {} (volatile shadows self-clean)",
            output.status
        ))
    }
}

#[cfg(not(target_os = "windows"))]
pub fn create_marker_shadow(_volume: &str) -> anyhow::Result<VssSnapshotResult> {
    anyhow::bail!("VSS is a Windows facility")
}

#[cfg(not(target_os = "windows"))]
pub fn delete_marker_shadow(_shadow_id: &str) -> anyhow::Result<String> {
    anyhow::bail!("VSS is a Windows facility")
}
