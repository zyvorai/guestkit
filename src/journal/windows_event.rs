// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Windows Event Log journal slice (parity with Linux journal RPC).

use crate::journal::analyze::analyze_journal;
use guestkit_agent_protocol::{JournalEntrySummary, JournalSlice};
use std::process::Command;

pub fn collect_event_log_slice(source: &str, limit: usize) -> JournalSlice {
    let log_name = if source.is_empty() { "System" } else { source };
    let script = format!(
        "Get-WinEvent -LogName '{log}' -MaxEvents {limit} -ErrorAction SilentlyContinue | \
         Select-Object TimeCreated, Id, LevelDisplayName, ProviderName, Message | \
         ConvertTo-Json -Compress",
        log = log_name.replace('\'', "''"),
        limit = limit.max(1),
    );
    let mut entries = Vec::new();
    if let Ok(out) = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .output()
    {
        if out.status.success() {
            let text_cow = String::from_utf8_lossy(&out.stdout);
            let text = text_cow.trim();
            if text.starts_with('[') {
                if let Ok(json) = serde_json::from_str::<Vec<serde_json::Value>>(text) {
                    for row in json {
                        entries.push(parse_event_row(&row, log_name));
                    }
                }
            } else if let Ok(row) = serde_json::from_str::<serde_json::Value>(text) {
                entries.push(parse_event_row(&row, log_name));
            }
        }
    }

    let slice = JournalSlice {
        unit: log_name.to_string(),
        boot_id: String::new(),
        entries,
        summary: String::new(),
        error_count: 0,
        top_patterns: Vec::new(),
        last_error: None,
        cursor: None,
    };
    let analyzed = analyze_journal(&slice, 5);
    JournalSlice {
        summary: if analyzed.top_patterns.is_empty() {
            format!("{} Windows event log entries", analyzed.entries.len())
        } else {
            analyzed.top_patterns.join("; ")
        },
        ..analyzed
    }
}

fn parse_event_row(row: &serde_json::Value, log_name: &str) -> JournalEntrySummary {
    let message = row
        .get("Message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .chars()
        .take(500)
        .collect();
    let level = row
        .get("LevelDisplayName")
        .and_then(|v| v.as_str())
        .unwrap_or("Information");
    let priority = match level {
        "Critical" => 2,
        "Error" => 3,
        "Warning" => 4,
        _ => 6,
    };
    let unit = row
        .get("ProviderName")
        .and_then(|v| v.as_str())
        .unwrap_or(log_name)
        .to_string();
    let timestamp = row
        .get("TimeCreated")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    JournalEntrySummary {
        timestamp,
        priority,
        unit,
        message,
    }
}
