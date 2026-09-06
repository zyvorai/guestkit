// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Native sd-journal reader via libsystemd (Linux only).

use crate::journal::analyze::analyze_journal;
use crate::journal::sd_journal::BootSelector;
use guestkit_agent_protocol::{JournalEntrySummary, JournalSlice};
use libsystemd_sys::journal::{
    sd_journal, sd_journal_add_match, sd_journal_close, sd_journal_get_cursor, sd_journal_get_data,
    sd_journal_get_realtime_usec, sd_journal_next, sd_journal_open, sd_journal_previous,
    sd_journal_seek_cursor, sd_journal_seek_tail, SD_JOURNAL_LOCAL_ONLY,
};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::c_char;
use std::path::Path;

const CURSOR_PATH: &str = "/var/lib/zyvor/journal-cursors.json";

pub fn try_collect_journal_slice(
    unit: &str,
    limit: usize,
    boot: BootSelector,
) -> Option<JournalSlice> {
    if std::env::var("ZYVOR_JOURNAL_JOURNALCTL")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return None;
    }

    let mut journal: *mut sd_journal = std::ptr::null_mut();
    unsafe {
        if sd_journal_open(&mut journal, SD_JOURNAL_LOCAL_ONLY) < 0 {
            return None;
        }

        let boot_id = boot_id_for(boot);
        let cursor_key = format!("{}:{}", unit, boot.as_str());
        let mut store = load_cursors();
        let after_cursor = store.cursors.get(&cursor_key).cloned();

        if !boot_id.is_empty() {
            if let Ok(match_str) = CString::new(format!("_BOOT_ID={boot_id}")) {
                sd_journal_add_match(
                    journal,
                    match_str.as_ptr() as *const std::ffi::c_void,
                    match_str.as_bytes().len(),
                );
            }
        }
        if !unit.is_empty() {
            if let Ok(match_str) = CString::new(format!("_SYSTEMD_UNIT={unit}")) {
                sd_journal_add_match(
                    journal,
                    match_str.as_ptr() as *const std::ffi::c_void,
                    match_str.as_bytes().len(),
                );
            }
        }

        let mut entries = Vec::new();
        let mut last_cursor: Option<String> = None;

        if let Some(cursor) = after_cursor.as_ref() {
            if let Ok(c) = CString::new(cursor.as_str()) {
                if sd_journal_seek_cursor(journal, c.as_ptr()) >= 0 {
                    sd_journal_next(journal);
                }
            }
            while entries.len() < limit && sd_journal_next(journal) > 0 {
                if let Some(entry) = read_entry(journal, unit) {
                    last_cursor = entry_cursor(journal).or(last_cursor);
                    entries.push(entry);
                }
            }
        } else {
            sd_journal_seek_tail(journal);
            sd_journal_previous(journal);
            while entries.len() < limit {
                if let Some(entry) = read_entry(journal, unit) {
                    last_cursor = entry_cursor(journal).or(last_cursor);
                    entries.push(entry);
                }
                if sd_journal_previous(journal) <= 0 {
                    break;
                }
            }
            entries.reverse();
        }

        sd_journal_close(journal);

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
        Some(JournalSlice {
            summary: if analyzed.top_patterns.is_empty() {
                format!(
                    "{} journal entries collected (sd-journal)",
                    analyzed.entries.len()
                )
            } else {
                analyzed.top_patterns.join("; ")
            },
            ..analyzed
        })
    }
}

fn read_entry(journal: *mut sd_journal, default_unit: &str) -> Option<JournalEntrySummary> {
    unsafe {
        let message = journal_field(journal, "MESSAGE").unwrap_or_default();
        let priority = journal_field(journal, "PRIORITY")
            .and_then(|s| s.parse().ok())
            .unwrap_or(6);
        let unit_name =
            journal_field(journal, "_SYSTEMD_UNIT").unwrap_or_else(|| default_unit.to_string());
        let timestamp = {
            let mut usec: u64 = 0;
            if sd_journal_get_realtime_usec(journal, &mut usec) >= 0 {
                usec.to_string()
            } else {
                String::new()
            }
        };
        Some(JournalEntrySummary {
            timestamp,
            priority,
            unit: unit_name,
            message,
        })
    }
}

fn entry_cursor(journal: *mut sd_journal) -> Option<String> {
    unsafe {
        let mut cursor_ptr: *const c_char = std::ptr::null();
        if sd_journal_get_cursor(journal, &mut cursor_ptr) < 0 {
            return None;
        }
        let cursor = CStr::from_ptr(cursor_ptr).to_string_lossy().into_owned();
        Some(cursor)
    }
}

fn journal_field(journal: *mut sd_journal, field: &str) -> Option<String> {
    let field = CString::new(field).ok()?;
    unsafe {
        let mut data: *mut u8 = std::ptr::null_mut();
        let mut length: usize = 0;
        if sd_journal_get_data(journal, field.as_ptr(), &mut data, &mut length) < 0 {
            return None;
        }
        let bytes = std::slice::from_raw_parts(data, length);
        let text = String::from_utf8_lossy(bytes);
        text.split_once('=')
            .map(|(_, v)| v.to_string())
            .or(Some(text.into_owned()))
    }
}

fn boot_id_for(boot: BootSelector) -> String {
    match boot {
        BootSelector::Current => fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .map(|s| s.trim().to_string())
            .unwrap_or_default(),
        BootSelector::Previous => previous_boot_id().unwrap_or_default(),
        BootSelector::All => String::new(),
    }
}

fn previous_boot_id() -> Option<String> {
    let current = fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .ok()
        .map(|s| s.trim().to_string())?;
    let out = std::process::Command::new("journalctl")
        .args(["--list-boots", "--no-pager"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 && parts[2] != current {
            return Some(parts[2].to_string());
        }
    }
    None
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct CursorStore {
    cursors: HashMap<String, String>,
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

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn native_journal_smoke_when_systemd_available() {
        if !Path::new("/run/systemd/journal").exists() {
            return;
        }
        let slice = try_collect_journal_slice("", 1, BootSelector::Current);
        assert!(slice.is_some());
        let slice = slice.unwrap();
        assert!(slice.entries.len() <= 1);
    }
}
