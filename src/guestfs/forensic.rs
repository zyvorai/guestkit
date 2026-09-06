// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Deep Windows EVTX forensic profiles from offline disk evidence.

use crate::evidence::snapshot::{WindowsForensicEvent, WindowsForensicProfile};
use crate::guestfs::windows_registry::{parse_evtx_file, WindowsEventEntry};
use std::path::Path;

const SECURITY_FAILED_LOGON: u32 = 4625;
const SECURITY_SUCCESS_LOGON: u32 = 4624;
const SECURITY_PRIV_USE: u32 = 4672;
const SYSTEM_SERVICE_FAIL: u32 = 7034;
const SYSTEM_UNEXPECTED_SHUTDOWN: u32 = 6008;

/// Build a forensic profile from parsed EVTX entries (Security + System channels).
pub fn build_forensic_profile(entries: &[WindowsEventEntry]) -> WindowsForensicProfile {
    let mut profile = WindowsForensicProfile::default();
    for entry in entries {
        match entry.event_id {
            SECURITY_FAILED_LOGON => profile.failed_logons += 1,
            SECURITY_SUCCESS_LOGON => profile.successful_logons += 1,
            SECURITY_PRIV_USE => profile.privilege_escalations += 1,
            SYSTEM_SERVICE_FAIL => profile.service_failures += 1,
            SYSTEM_UNEXPECTED_SHUTDOWN => profile.unexpected_shutdowns += 1,
            id if is_suspicious_event(id) && !profile.suspicious_event_ids.contains(&id) => {
                profile.suspicious_event_ids.push(id);
            }
            _ => {}
        }
        if (is_critical_level(&entry.level) || is_suspicious_event(entry.event_id))
            && profile.recent_critical.len() < 25
        {
            profile.recent_critical.push(to_forensic_event(entry));
        }
    }
    profile
}

pub fn parse_evtx_forensic(path: &Path, limit: usize) -> Option<WindowsForensicProfile> {
    parse_evtx_file(path, limit)
        .ok()
        .filter(|e| !e.is_empty())
        .map(|entries| build_forensic_profile(&entries))
}

fn is_suspicious_event(id: u32) -> bool {
    matches!(
        id,
        1102 | 4698 | 4699 | 4702 | 7045 | 4720 | 4722 | 4732 | 4738 | 4776 | 5140 | 5145
    )
}

fn is_critical_level(level: &str) -> bool {
    matches!(level, "1" | "2" | "Critical" | "Error")
}

fn to_forensic_event(entry: &WindowsEventEntry) -> WindowsForensicEvent {
    WindowsForensicEvent {
        event_id: entry.event_id,
        channel: entry.channel.clone(),
        source: entry.source.clone(),
        level: entry.level.clone(),
        time_created: entry.time_created.clone(),
        summary: truncate(&entry.message, 240),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_failed_logons() {
        let entries = vec![WindowsEventEntry {
            event_id: 4625,
            level: "2".into(),
            source: "Microsoft-Windows-Security-Auditing".into(),
            message: "failed".into(),
            time_created: "2026-01-01T00:00:00Z".into(),
            computer: "WIN".into(),
            channel: "Security".into(),
        }];
        let p = build_forensic_profile(&entries);
        assert_eq!(p.failed_logons, 1);
    }
}
