// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Journal log pattern analysis.

use guestkit_agent_protocol::{JournalEntrySummary, JournalSlice};
use regex::Regex;
use std::collections::HashMap;

static DIGIT_RE: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| {
    Regex::new(r"\b\d+\b").unwrap_or_else(|_| Regex::new("").unwrap())
});

/// Bucket journal messages into normalized patterns and count top-N.
pub fn analyze_journal(slice: &JournalSlice, top_n: usize) -> JournalSlice {
    let mut out = slice.clone();
    let error_entries: Vec<&JournalEntrySummary> =
        slice.entries.iter().filter(|e| e.priority <= 3).collect();

    out.error_count = error_entries.len();

    let mut pattern_counts: HashMap<String, usize> = HashMap::new();
    for entry in &error_entries {
        let pattern = normalize_pattern(&entry.message);
        if !pattern.is_empty() {
            *pattern_counts.entry(pattern).or_insert(0) += 1;
        }
    }

    out.top_patterns = {
        let mut pairs: Vec<(String, usize)> = pattern_counts.into_iter().collect();
        pairs.sort_by_key(|p| std::cmp::Reverse(p.1));
        pairs.into_iter().take(top_n).map(|(p, _)| p).collect()
    };

    out.last_error = error_entries.last().map(|e| (*e).clone());
    out
}

pub fn normalize_pattern(message: &str) -> String {
    let lowered = message.to_lowercase();
    let normalized = DIGIT_RE.replace_all(&lowered, "N");
    normalized.trim().chars().take(120).collect()
}

pub fn suggest_action_from_patterns(patterns: &[String]) -> Option<String> {
    for p in patterns {
        let lower = p.to_lowercase();
        if lower.contains("address already in use") || lower.contains("bind()") {
            return Some("Check which process is already using the target port".into());
        }
        if lower.contains("permission denied") {
            return Some("Check file/directory permissions on the service data path".into());
        }
        if lower.contains("no such file") {
            return Some("Verify config paths and missing files referenced by the service".into());
        }
        if lower.contains("certificate") && lower.contains("expired") {
            return Some("Renew TLS certificate and reload the service".into());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_digits() {
        let p = normalize_pattern("bind() to 0.0.0.0:443 failed");
        assert!(p.contains("N"));
        assert!(!p.contains("443"));
    }

    #[test]
    fn suggest_port_conflict() {
        let action = suggest_action_from_patterns(&["bind() address already in use".into()]);
        assert!(action.unwrap().contains("port"));
    }
}
