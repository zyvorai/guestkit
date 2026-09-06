// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Journal collection with cursor tracking (journalctl with sd-journal semantics).

use crate::journal::analyze::analyze_journal;
use guestkit_agent_protocol::{JournalEntrySummary, JournalSlice};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

const CURSOR_PATH: &str = "/var/lib/zyvor/journal-cursors.json";

#[derive(Debug, Default, Serialize, Deserialize)]
struct CursorStore {
    cursors: HashMap<String, String>,
}

pub fn collect_journal_slice(unit: &str, limit: usize, boot: BootSelector) -> JournalSlice {
    #[cfg(all(target_os = "linux", feature = "journal-native"))]
    if let Some(slice) =
        crate::journal::sd_journal_native::try_collect_journal_slice(unit, limit, boot)
    {
        return slice;
    }
    collect_journal_slice_journalctl(unit, limit, boot)
}

fn collect_journal_slice_journalctl(unit: &str, limit: usize, boot: BootSelector) -> JournalSlice {
    let boot_id = read_boot_id();
    let cursor_key = format!("{}:{}", unit, boot.as_str());
    let mut store = load_cursors();
    let after_cursor = store.cursors.get(&cursor_key).cloned();

    let mut cmd = Command::new("journalctl");
    cmd.args([
        "--no-pager",
        "-o",
        "json",
        "-n",
        &limit.to_string(),
        "--show-cursor",
    ]);
    match boot {
        BootSelector::Current => {
            cmd.arg("-b");
        }
        BootSelector::Previous => {
            cmd.args(["-b", "-1"]);
        }
        BootSelector::All => {}
    }
    if !unit.is_empty() {
        cmd.args(["-u", unit]);
    }
    if let Some(cursor) = after_cursor {
        cmd.args(["--after-cursor", &cursor]);
    }

    let output = cmd.output().ok();
    let mut entries = Vec::new();
    let mut last_cursor: Option<String> = None;

    if let Some(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if line.starts_with("-- cursor:") {
                last_cursor = Some(line.trim_start_matches("-- cursor:").trim().to_string());
                continue;
            }
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                entries.push(parse_journal_json(&json, unit));
            }
        }
    }

    if let Some(cursor) = last_cursor {
        store.cursors.insert(cursor_key.clone(), cursor);
        save_cursors(&store);
    }

    let slice = JournalSlice {
        unit: unit.to_string(),
        boot_id,
        entries,
        summary: String::new(),
        error_count: 0,
        top_patterns: Vec::new(),
        last_error: None,
        cursor: store.cursors.get(&cursor_key).cloned(),
    };

    let analyzed = analyze_journal(&slice, 5);
    JournalSlice {
        summary: if analyzed.top_patterns.is_empty() {
            format!("{} journal entries collected", analyzed.entries.len())
        } else {
            analyzed.top_patterns.join("; ")
        },
        ..analyzed
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub enum BootSelector {
    Current,
    Previous,
    #[default]
    All,
}

impl BootSelector {
    pub fn parse(s: &str) -> Self {
        match s {
            "current" | "0" => BootSelector::Current,
            "previous" | "-1" => BootSelector::Previous,
            _ => BootSelector::All,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            BootSelector::Current => "current",
            BootSelector::Previous => "previous",
            BootSelector::All => "all",
        }
    }
}

fn parse_journal_json(json: &serde_json::Value, default_unit: &str) -> JournalEntrySummary {
    let message = json
        .get("MESSAGE")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let priority = json.get("PRIORITY").and_then(|v| v.as_u64()).unwrap_or(6) as u8;
    let unit_name = json
        .get("_SYSTEMD_UNIT")
        .and_then(|v| v.as_str())
        .unwrap_or(default_unit)
        .to_string();
    let timestamp = json
        .get("__REALTIME_TIMESTAMP")
        .and_then(|v| v.as_u64())
        .map(|t| t.to_string())
        .unwrap_or_default();

    JournalEntrySummary {
        timestamp,
        priority,
        unit: unit_name,
        message,
    }
}

fn read_boot_id() -> String {
    fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn load_cursors() -> CursorStore {
    if Path::new(CURSOR_PATH).exists() {
        fs::read_to_string(CURSOR_PATH)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        CursorStore::default()
    }
}

fn save_cursors(store: &CursorStore) {
    if let Ok(json) = serde_json::to_string(store) {
        let parent = Path::new(CURSOR_PATH)
            .parent()
            .unwrap_or(Path::new("/var/lib/zyvor"));
        fs::create_dir_all(parent).ok();
        fs::write(CURSOR_PATH, json).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::BootSelector;

    #[test]
    fn boot_selector_from_str() {
        assert_eq!(BootSelector::parse("current").as_str(), "current");
        assert_eq!(BootSelector::parse("0").as_str(), "current");
        assert_eq!(BootSelector::parse("previous").as_str(), "previous");
        assert_eq!(BootSelector::parse("-1").as_str(), "previous");
        assert_eq!(BootSelector::parse("all").as_str(), "all");
        assert_eq!(BootSelector::parse("bogus").as_str(), "all");
    }
}
