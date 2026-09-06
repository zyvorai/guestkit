// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Build compressed support bundles (evidence + health + journal slices).

use anyhow::{Context, Result};
use serde_json::json;
use std::io::Cursor;

/// Collect a tar.zst bundle with evidence, health, semantic analysis, and journal excerpts.
pub fn build_support_bundle_bytes() -> Result<Vec<u8>> {
    let evidence = crate::evidence::build_evidence_live()?;
    let health = crate::health::build_guest_health(&evidence);
    let semantic = crate::ai::semantic::analyze_semantic(&evidence);
    let guest_info = crate::health::build_guest_info(&evidence);

    let manifest = json!({
        "bundle_version": 1,
        "format": "tar.zst",
        "agent_version": crate::VERSION,
        "collected_at": chrono::Utc::now().to_rfc3339(),
        "hostname": evidence.os.hostname,
    });

    let mut entries: Vec<(String, Vec<u8>)> = vec![
        (
            "manifest.json".into(),
            serde_json::to_vec_pretty(&manifest).context("manifest json")?,
        ),
        (
            "evidence.json".into(),
            serde_json::to_vec_pretty(&evidence).context("evidence json")?,
        ),
        (
            "guest_health.json".into(),
            serde_json::to_vec_pretty(&health).context("guest_health json")?,
        ),
        (
            "semantic.json".into(),
            serde_json::to_vec_pretty(&semantic).context("semantic json")?,
        ),
        (
            "guest_info.json".into(),
            serde_json::to_vec_pretty(&guest_info).context("guest_info json")?,
        ),
    ];

    if let Some(process) = &evidence.process {
        entries.push((
            "process.json".into(),
            serde_json::to_vec_pretty(process).context("process json")?,
        ));
    }

    #[cfg(target_os = "linux")]
    {
        let events = crate::collectors::dbus::systemd_events::recent_events(100);
        entries.push((
            "systemd_events.json".into(),
            serde_json::to_vec_pretty(&events).context("systemd_events json")?,
        ));

        let general = crate::journal::live::collect_journal_slice("", 100);
        entries.push((
            "journal/recent.json".into(),
            serde_json::to_vec_pretty(&general).context("journal recent json")?,
        ));

        for svc in health.critical_services.iter().take(8) {
            let slice = crate::journal::live::collect_journal_slice(&svc.name, 50);
            let safe_name = svc.name.replace('/', "_");
            entries.push((
                format!("journal/{safe_name}.json"),
                serde_json::to_vec_pretty(&slice).context("unit journal json")?,
            ));
        }
    }

    let mut tar_buf = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_buf);
        for (path, data) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, path, &mut Cursor::new(data))
                .context("append tar entry")?;
        }
        builder.finish().context("finish tar")?;
    }

    zstd::encode_all(&tar_buf[..], 3).context("zstd compress bundle")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_is_zstd_tar() {
        let bytes = build_support_bundle_bytes().expect("bundle");
        assert!(!bytes.is_empty());
        let tar_bytes = zstd::decode_all(&bytes[..]).expect("zstd decode");
        let mut archive = tar::Archive::new(Cursor::new(tar_bytes));
        let names: Vec<String> = archive
            .entries()
            .expect("entries")
            .filter_map(|e| e.ok())
            .filter_map(|e| e.path().ok().map(|p| p.to_string_lossy().into_owned()))
            .collect();
        assert!(names.contains(&"manifest.json".to_string()));
        assert!(names.contains(&"guest_health.json".to_string()));
    }
}
