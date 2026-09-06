// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Windows registry and filesystem evidence enrichment.

use crate::evidence::snapshot::{
    WindowsAppEntry, WindowsEventLogSummary, WindowsForensicProfile, WindowsPersistenceEvidence,
    WindowsServiceEntry, WindowsServiceType, WindowsStartType,
};
use crate::Guestfs;

/// Enrich Windows service/app/persistence details using guestfs-mounted hives.
pub fn collect_windows_details(g: &mut Guestfs, root: &str) -> WindowsDetails {
    let mut details = WindowsDetails::default();

    if let Ok(services) = g.inspect_windows_services(root) {
        details.services = services
            .into_iter()
            .map(|svc| {
                let auto_start = svc.start_type == "Automatic" || svc.start_type == "Boot";
                WindowsServiceEntry {
                    name: svc.name,
                    display_name: Some(svc.display_name),
                    image_path: None,
                    start_type: parse_start_type(&svc.start_type),
                    service_type: WindowsServiceType::Win32,
                    object_name: None,
                    description: None,
                    kernel_driver: false,
                    auto_start,
                }
            })
            .collect();
    }

    if let Ok(apps) = g.inspect_windows_software(root) {
        details.installed_apps = apps
            .into_iter()
            .take(50)
            .map(|app| WindowsAppEntry {
                name: app.name,
                version: app.version,
                publisher: app.publisher,
                install_location: None,
            })
            .collect();
    }

    let systemroot = g
        .inspect_get_windows_systemroot(root)
        .unwrap_or_else(|_| "/Windows".to_string());
    details.event_logs = summarize_event_logs(g, &systemroot);
    details.persistence = collect_windows_persistence(g, root, &systemroot);

    details
}

fn collect_windows_persistence(
    g: &mut Guestfs,
    root: &str,
    systemroot: &str,
) -> crate::evidence::snapshot::WindowsPersistenceEvidence {
    use crate::evidence::snapshot::{WindowsPersistenceEntry, WindowsPersistenceEvidence};
    use std::path::PathBuf;

    let mut persistence = WindowsPersistenceEvidence::default();

    let software_hive = format!("{root}/Windows/System32/config/SOFTWARE");
    if g.exists(&software_hive).unwrap_or(false) {
        if let Ok(keys) = crate::guestfs::windows_registry::parse_run_keys(
            PathBuf::from(&software_hive).as_path(),
        ) {
            persistence.run_keys = keys
                .into_iter()
                .map(|k| WindowsPersistenceEntry {
                    location: k.location,
                    name: k.name,
                    command: k.command,
                })
                .collect();
        }
    }

    let tasks_dir = format!("{systemroot}/System32/Tasks");
    if g.exists(&tasks_dir).unwrap_or(false) {
        if let Ok(entries) = g.ls(&tasks_dir) {
            persistence.scheduled_tasks = entries.into_iter().take(100).collect();
        }
    }

    persistence
}

#[derive(Debug, Default)]
pub struct WindowsDetails {
    pub services: Vec<WindowsServiceEntry>,
    pub installed_apps: Vec<WindowsAppEntry>,
    pub persistence: WindowsPersistenceEvidence,
    pub event_logs: WindowsEventLogSummary,
}

fn parse_start_type(raw: &str) -> WindowsStartType {
    match raw {
        "Boot" => WindowsStartType::Boot,
        "System" => WindowsStartType::System,
        "Automatic" => WindowsStartType::Automatic,
        "Manual" => WindowsStartType::Manual,
        "Disabled" => WindowsStartType::Disabled,
        _ => WindowsStartType::Unknown,
    }
}

fn summarize_event_logs(g: &mut Guestfs, systemroot: &str) -> WindowsEventLogSummary {
    use crate::guestfs::forensic::{build_forensic_profile, parse_evtx_forensic};
    use std::io::Write;

    let log_dir = format!("{systemroot}/System32/winevt/Logs");
    let mut summary = WindowsEventLogSummary::default();
    let mut forensic_entries = Vec::new();

    if !g.exists(&log_dir).unwrap_or(false) {
        return summary;
    }

    if let Ok(entries) = g.ls(&log_dir) {
        for entry in entries {
            if !entry.ends_with(".evtx") {
                continue;
            }
            summary.log_count += 1;
            let path = format!("{log_dir}/{entry}");
            if let Ok(stat) = g.stat(&path) {
                summary.total_bytes = summary.total_bytes.saturating_add(stat.size.max(0) as u64);
            }
            if entry == "Security.evtx" || entry == "System.evtx" || entry == "Application.evtx" {
                if let Ok(bytes) = g.read_file(&path) {
                    if let Ok(mut tmp) = tempfile::NamedTempFile::new() {
                        if tmp.write_all(&bytes).is_ok() {
                            if let Some(profile) = parse_evtx_forensic(tmp.path(), 500) {
                                merge_forensic(&mut summary, profile);
                            }
                            if let Ok(parsed) =
                                crate::guestfs::windows_registry::parse_evtx_file(tmp.path(), 200)
                            {
                                forensic_entries.extend(parsed);
                            }
                        }
                    }
                }
            }
        }
    }

    if summary.forensic.is_none() && !forensic_entries.is_empty() {
        summary.forensic = Some(build_forensic_profile(&forensic_entries));
    }
    summary
}

fn merge_forensic(summary: &mut WindowsEventLogSummary, add: WindowsForensicProfile) {
    let profile = summary
        .forensic
        .get_or_insert_with(WindowsForensicProfile::default);
    profile.failed_logons += add.failed_logons;
    profile.successful_logons += add.successful_logons;
    profile.service_failures += add.service_failures;
    profile.unexpected_shutdowns += add.unexpected_shutdowns;
    profile.privilege_escalations += add.privilege_escalations;
    for id in add.suspicious_event_ids {
        if !profile.suspicious_event_ids.contains(&id) {
            profile.suspicious_event_ids.push(id);
        }
    }
    for evt in add.recent_critical {
        if profile.recent_critical.len() < 25 {
            profile.recent_critical.push(evt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_start_types() {
        assert!(matches!(
            parse_start_type("Automatic"),
            WindowsStartType::Automatic
        ));
        assert!(matches!(
            parse_start_type("Disabled"),
            WindowsStartType::Disabled
        ));
    }
}
