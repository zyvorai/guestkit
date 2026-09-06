// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Offline and live systemd unit collection for evidence snapshots.

use crate::evidence::snapshot::{
    SystemdInfo, SystemdProblemHint, SystemdProblemSeverity, SystemdUnit, SystemdUnitSection,
    SystemdUnitState,
};
#[cfg(not(target_os = "windows"))]
use crate::Guestfs;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const UNIT_SUFFIXES: &[&str] = &[
    ".service", ".timer", ".socket", ".mount", ".path", ".target", ".device", ".slice", ".scope",
];

const UNIT_DIRS: &[&str] = &[
    "/etc/systemd/system",
    "/usr/lib/systemd/system",
    "/lib/systemd/system",
    "/run/systemd/system",
];

/// Collect systemd evidence from a mounted guest image via Guestfs.
#[cfg(not(target_os = "windows"))]
pub fn collect_systemd_guest(g: &mut Guestfs, init_system: &str) -> Option<SystemdInfo> {
    if !init_system.eq_ignore_ascii_case("systemd")
        && !g.exists("/usr/lib/systemd/system").unwrap_or(false)
        && !g.exists("/lib/systemd/system").unwrap_or(false)
    {
        return None;
    }

    let mut info = SystemdInfo {
        version: read_guest_file(g, "/etc/systemd/system.conf")
            .or_else(|| read_guest_file(g, "/usr/lib/systemd/systemd"))
            .and_then(|_| read_systemd_version_guest(g)),
        ..Default::default()
    };

    let enabled = discover_enabled_units_guest(g);
    let masked = discover_masked_units_guest(g);

    for dir in UNIT_DIRS {
        if !g.exists(dir).unwrap_or(false) {
            continue;
        }
        if let Ok(entries) = g.ls(dir) {
            for entry in entries {
                if !is_unit_file(&entry) {
                    continue;
                }
                let path = format!("{dir}/{entry}");
                if let Ok(content) = g.read_file(&path) {
                    let text = String::from_utf8_lossy(&content);
                    let unit = parse_unit_file(&path, &text, &enabled, &masked);
                    info.units.push(unit);
                }
            }
        }
    }

    info.units.sort_by(|a, b| a.name.cmp(&b.name));
    info.problem_hints = analyze_systemd_problems(&info.units);
    info.unit_count = info.units.len();
    info.timer_count = info.units.iter().filter(|u| u.unit_type == "timer").count();
    info.service_count = info
        .units
        .iter()
        .filter(|u| u.unit_type == "service")
        .count();

    Some(info)
}

/// Collect systemd evidence from the live running host.
pub fn collect_systemd_live() -> Option<SystemdInfo> {
    if !Path::new("/usr/lib/systemd/system").exists() && !Path::new("/lib/systemd/system").exists()
    {
        return None;
    }

    let mut info = SystemdInfo {
        version: read_systemd_version_live(),
        ..Default::default()
    };
    let enabled = discover_enabled_units_live();
    let masked = discover_masked_units_live();

    for dir in UNIT_DIRS {
        let path = Path::new(dir.strip_prefix('/').unwrap_or(dir));
        if !path.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if !is_unit_file(&file_name) {
                    continue;
                }
                let guest_path = format!("{dir}/{file_name}");
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    let unit = parse_unit_file(&guest_path, &content, &enabled, &masked);
                    info.units.push(unit);
                }
            }
        }
    }

    info.units.sort_by(|a, b| a.name.cmp(&b.name));
    info.problem_hints = analyze_systemd_problems(&info.units);
    info.unit_count = info.units.len();
    info.timer_count = info.units.iter().filter(|u| u.unit_type == "timer").count();
    info.service_count = info
        .units
        .iter()
        .filter(|u| u.unit_type == "service")
        .count();

    Some(info)
}

#[cfg(not(target_os = "windows"))]
fn read_guest_file(g: &mut Guestfs, path: &str) -> Option<String> {
    g.read_file(path)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
}

#[cfg(not(target_os = "windows"))]
fn read_systemd_version_guest(g: &mut Guestfs) -> Option<String> {
    for path in ["/usr/lib/systemd/systemd", "/lib/systemd/systemd"] {
        if g.exists(path).unwrap_or(false) {
            return Some("present".to_string());
        }
    }
    None
}

fn read_systemd_version_live() -> Option<String> {
    std::process::Command::new("systemd")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .map(|line| line.to_string())
        })
}

fn is_unit_file(name: &str) -> bool {
    UNIT_SUFFIXES.iter().any(|s| name.ends_with(s))
}

fn unit_type_from_name(name: &str) -> String {
    UNIT_SUFFIXES
        .iter()
        .find(|s| name.ends_with(*s))
        .map(|s| s.trim_start_matches('.'))
        .unwrap_or("unknown")
        .to_string()
}

fn parse_unit_file(
    path: &str,
    content: &str,
    enabled: &HashSet<String>,
    masked: &HashSet<String>,
) -> SystemdUnit {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    let name = file_name.to_string();
    let unit_type = unit_type_from_name(&name);
    let sections = parse_ini_sections(content);

    let unit_section = sections.get("Unit").cloned().unwrap_or_default();
    let service_section = sections.get("Service").cloned().unwrap_or_default();
    let install_section = sections.get("Install").cloned().unwrap_or_default();
    let timer_section = sections.get("Timer").cloned().unwrap_or_default();

    let state = if masked.contains(&name) {
        SystemdUnitState::Masked
    } else if enabled.contains(&name) {
        SystemdUnitState::Enabled
    } else {
        SystemdUnitState::Disabled
    };

    SystemdUnit {
        name,
        unit_type,
        path: path.to_string(),
        state,
        description: unit_section.get("Description").cloned(),
        exec_start: service_section.get("ExecStart").cloned(),
        service_type: service_section.get("Type").cloned(),
        restart: service_section.get("Restart").cloned(),
        remain_after_exit: service_section
            .get("RemainAfterExit")
            .map(|v| v.eq_ignore_ascii_case("yes") || v == "true"),
        user: service_section.get("User").cloned(),
        group: service_section.get("Group").cloned(),
        after: split_deps(unit_section.get("After")),
        before: split_deps(unit_section.get("Before")),
        requires: split_deps(unit_section.get("Requires")),
        wants: split_deps(unit_section.get("Wants")),
        wanted_by: split_deps(install_section.get("WantedBy")),
        on_calendar: timer_section.get("OnCalendar").cloned(),
        sections: sections
            .into_iter()
            .map(|(k, v)| SystemdUnitSection { name: k, keys: v })
            .collect(),
    }
}

fn parse_ini_sections(content: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut sections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut current = String::from("Unit");

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current = line[1..line.len() - 1].to_string();
            sections.entry(current.clone()).or_default();
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            sections
                .entry(current.clone())
                .or_default()
                .insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    sections
}

fn split_deps(value: Option<&String>) -> Vec<String> {
    value
        .map(|v| {
            v.split_whitespace()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

#[cfg(not(target_os = "windows"))]
fn discover_enabled_units_guest(g: &mut Guestfs) -> HashSet<String> {
    let mut enabled = HashSet::new();
    for base in UNIT_DIRS {
        if !g.exists(base).unwrap_or(false) {
            continue;
        }
        if let Ok(entries) = g.ls(base) {
            for entry in entries {
                if entry.ends_with(".wants") || entry.ends_with(".requires") {
                    let wants_dir = format!("{base}/{entry}");
                    if let Ok(units) = g.ls(&wants_dir) {
                        for unit in units {
                            enabled.insert(unit);
                        }
                    }
                }
            }
        }
    }
    enabled
}

#[cfg(not(target_os = "windows"))]
fn discover_masked_units_guest(g: &mut Guestfs) -> HashSet<String> {
    let mut masked = HashSet::new();
    for base in ["/etc/systemd/system", "/run/systemd/system"] {
        if !g.exists(base).unwrap_or(false) {
            continue;
        }
        if let Ok(entries) = g.ls(base) {
            for entry in entries {
                if is_unit_file(&entry) {
                    let path = format!("{base}/{entry}");
                    if g.is_symlink(&path).unwrap_or(false) {
                        if let Ok(target) = g.readlink(&path) {
                            if target.contains("/dev/null") {
                                masked.insert(entry);
                            }
                        }
                    }
                }
            }
        }
    }
    masked
}

fn discover_enabled_units_live() -> HashSet<String> {
    let mut enabled = HashSet::new();
    for base in UNIT_DIRS {
        let path = PathBuf::from(base.strip_prefix('/').unwrap_or(base));
        if !path.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".wants") || name.ends_with(".requires") {
                    if let Ok(units) = fs::read_dir(entry.path()) {
                        for unit in units.flatten() {
                            enabled.insert(unit.file_name().to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
    }
    enabled
}

fn discover_masked_units_live() -> HashSet<String> {
    let mut masked = HashSet::new();
    for base in ["/etc/systemd/system", "/run/systemd/system"] {
        let path = PathBuf::from(base.strip_prefix('/').unwrap_or(base));
        if !path.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if is_unit_file(&name) && entry.path().is_symlink() {
                    if let Ok(target) = fs::read_link(entry.path()) {
                        if target.to_string_lossy().contains("/dev/null") {
                            masked.insert(name);
                        }
                    }
                }
            }
        }
    }
    masked
}

fn analyze_systemd_problems(units: &[SystemdUnit]) -> Vec<SystemdProblemHint> {
    let mut hints = Vec::new();

    for unit in units {
        if unit.unit_type != "service" {
            continue;
        }

        if unit.service_type.as_deref() == Some("oneshot") && unit.remain_after_exit != Some(true) {
            hints.push(SystemdProblemHint {
                unit: unit.name.clone(),
                code: "oneshot-no-remain".to_string(),
                severity: SystemdProblemSeverity::Warning,
                message:
                    "Type=oneshot without RemainAfterExit=yes may cause ordering issues at boot"
                        .to_string(),
                path: unit.path.clone(),
            });
        }

        let runs_as_root = unit
            .user
            .as_deref()
            .is_none_or(|u| u == "root" || u.is_empty());
        let has_sandbox = unit.sections.iter().any(|section| {
            section.name == "Service"
                && section.keys.keys().any(|k| {
                    k.starts_with("Protect") || k.starts_with("Private") || k == "NoNewPrivileges"
                })
        });
        if runs_as_root && !has_sandbox && unit.state == SystemdUnitState::Enabled {
            hints.push(SystemdProblemHint {
                unit: unit.name.clone(),
                code: "root-no-sandbox".to_string(),
                severity: SystemdProblemSeverity::Info,
                message: "Enabled service runs as root without Protect*/Private* sandbox flags"
                    .to_string(),
                path: unit.path.clone(),
            });
        }

        if unit.state == SystemdUnitState::Enabled {
            let exec = unit.exec_start.as_deref().unwrap_or("");
            let needs_network = exec.contains("curl")
                || exec.contains("wget")
                || unit.description.as_deref().is_some_and(|d| {
                    d.to_lowercase().contains("network") || d.to_lowercase().contains("sync")
                });
            let has_network_dep = unit.after.iter().any(|d| d.contains("network"));
            if needs_network && !has_network_dep {
                hints.push(SystemdProblemHint {
                    unit: unit.name.clone(),
                    code: "missing-network-after".to_string(),
                    severity: SystemdProblemSeverity::Warning,
                    message:
                        "Network-related service lacks After=network-online.target (or similar)"
                            .to_string(),
                    path: unit.path.clone(),
                });
            }
        }
    }

    hints
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_service_unit_sections() {
        let content = r#"
[Unit]
Description=Example
After=network.target

[Service]
Type=oneshot
ExecStart=/usr/bin/true
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
"#;
        let unit = parse_unit_file(
            "/etc/systemd/system/example.service",
            content,
            &HashSet::new(),
            &HashSet::new(),
        );
        assert_eq!(unit.name, "example.service");
        assert_eq!(unit.unit_type, "service");
        assert_eq!(unit.description.as_deref(), Some("Example"));
        assert_eq!(unit.remain_after_exit, Some(true));
        assert_eq!(unit.wanted_by, vec!["multi-user.target"]);
    }

    #[test]
    fn detects_oneshot_problem() {
        let unit = SystemdUnit {
            name: "bad.service".to_string(),
            unit_type: "service".to_string(),
            path: "/etc/systemd/system/bad.service".to_string(),
            state: SystemdUnitState::Enabled,
            service_type: Some("oneshot".to_string()),
            remain_after_exit: Some(false),
            ..Default::default()
        };
        let hints = analyze_systemd_problems(&[unit]);
        assert!(hints.iter().any(|h| h.code == "oneshot-no-remain"));
    }
}
