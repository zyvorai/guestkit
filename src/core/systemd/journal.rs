// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Systemd journal reading and analysis

use super::{JournalEntry, SystemdAnalyzer};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Journal filter options
#[derive(Debug, Clone, Default)]
pub struct JournalFilter {
    /// Filter by priority (0-7)
    pub priority: Option<u8>,
    /// Filter by unit name
    pub unit: Option<String>,
    /// Minimum timestamp (microseconds since epoch)
    pub since: Option<u64>,
    /// Maximum timestamp (microseconds since epoch)
    pub until: Option<u64>,
    /// Maximum number of entries to return
    pub limit: Option<usize>,
}

/// Journal statistics
#[derive(Debug, Clone)]
pub struct JournalStats {
    /// Total number of entries
    pub total_entries: usize,
    /// Entries by priority
    pub by_priority: HashMap<u8, usize>,
    /// Entries by unit
    pub by_unit: HashMap<String, usize>,
    /// Error count (priority 0-3)
    pub error_count: usize,
    /// Warning count (priority 4)
    pub warning_count: usize,
}

impl JournalStats {
    /// Create new journal statistics
    pub fn new() -> Self {
        Self {
            total_entries: 0,
            by_priority: HashMap::new(),
            by_unit: HashMap::new(),
            error_count: 0,
            warning_count: 0,
        }
    }

    /// Update statistics with an entry
    pub fn add_entry(&mut self, entry: &JournalEntry) {
        self.total_entries += 1;

        *self.by_priority.entry(entry.priority).or_insert(0) += 1;

        if let Some(ref unit) = entry.unit {
            *self.by_unit.entry(unit.clone()).or_insert(0) += 1;
        }

        if entry.priority <= 3 {
            self.error_count += 1;
        } else if entry.priority == 4 {
            self.warning_count += 1;
        }
    }
}

impl Default for JournalStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Journal reader
#[derive(Debug)]
pub struct JournalReader {
    analyzer: SystemdAnalyzer,
}

impl JournalReader {
    /// Create a new journal reader
    pub fn new(analyzer: SystemdAnalyzer) -> Self {
        Self { analyzer }
    }

    /// Read journal entries with filter
    ///
    /// Note: This is a simplified implementation that reads text-based journal files.
    /// For binary journal files (.journal), full implementation would require
    /// libsystemd bindings or custom binary parser.
    pub fn read_entries(&self, filter: &JournalFilter) -> Result<Vec<JournalEntry>> {
        let journal_dir = self.analyzer.journal_dir();

        if !journal_dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = Vec::new();

        // Read journal files (simplified - reads exported text journals)
        // In production, this would parse binary .journal files
        for entry in fs::read_dir(&journal_dir).with_context(|| {
            format!(
                "Failed to read journal directory: {}",
                journal_dir.display()
            )
        })? {
            let entry = entry?;
            let path = entry.path();

            // Skip if not a readable file
            if !path.is_file() {
                continue;
            }

            // For now, look for exported journal files (.txt or .export)
            if let Some(ext) = path.extension() {
                if ext == "txt" || ext == "export" {
                    if let Ok(file_entries) = self.parse_exported_journal(&path, filter) {
                        entries.extend(file_entries);
                    }
                }
            }
        }

        // Apply limit if specified
        if let Some(limit) = filter.limit {
            entries.truncate(limit);
        }

        Ok(entries)
    }

    /// Parse exported journal file (text format)
    fn parse_exported_journal(
        &self,
        path: &Path,
        filter: &JournalFilter,
    ) -> Result<Vec<JournalEntry>> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read journal file: {}", path.display()))?;

        let mut entries = Vec::new();
        let mut current_entry: Option<JournalEntry> = None;
        let mut current_fields = HashMap::new();

        for line in content.lines() {
            if line.is_empty() {
                // Entry separator
                if let Some(entry) = current_entry.take() {
                    if self.matches_filter(&entry, filter) {
                        entries.push(entry);
                    }
                }
                current_fields.clear();
                continue;
            }

            // Parse key=value pairs
            if let Some((key, value)) = line.split_once('=') {
                current_fields.insert(key.to_string(), value.to_string());

                // Build entry when we have required fields
                if key == "MESSAGE" {
                    let entry = self.build_entry_from_fields(&current_fields);
                    current_entry = Some(entry);
                }
            }
        }

        // Don't forget the last entry
        if let Some(entry) = current_entry {
            if self.matches_filter(&entry, filter) {
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    /// Build journal entry from parsed fields
    fn build_entry_from_fields(&self, fields: &HashMap<String, String>) -> JournalEntry {
        let timestamp = fields
            .get("__REALTIME_TIMESTAMP")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        let priority = fields
            .get("PRIORITY")
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(6); // Default to INFO

        let unit = fields
            .get("_SYSTEMD_UNIT")
            .cloned()
            .or_else(|| fields.get("UNIT").cloned());

        let message = fields
            .get("MESSAGE")
            .cloned()
            .unwrap_or_else(|| "(no message)".to_string());

        let pid = fields.get("_PID").and_then(|s| s.parse::<u32>().ok());

        JournalEntry {
            timestamp,
            priority,
            unit,
            message,
            pid,
            fields: fields.clone(),
        }
    }

    /// Check if entry matches filter
    fn matches_filter(&self, entry: &JournalEntry, filter: &JournalFilter) -> bool {
        if let Some(priority) = filter.priority {
            if entry.priority > priority {
                return false;
            }
        }

        if let Some(ref unit_filter) = filter.unit {
            if let Some(ref unit) = entry.unit {
                if !unit.contains(unit_filter) {
                    return false;
                }
            } else {
                return false;
            }
        }

        if let Some(since) = filter.since {
            if entry.timestamp < since {
                return false;
            }
        }

        if let Some(until) = filter.until {
            if entry.timestamp > until {
                return false;
            }
        }

        true
    }

    /// Get journal statistics
    pub fn get_statistics(&self, filter: &JournalFilter) -> Result<JournalStats> {
        let entries = self.read_entries(filter)?;
        let mut stats = JournalStats::new();

        for entry in &entries {
            stats.add_entry(entry);
        }

        Ok(stats)
    }

    /// Get entries with errors (priority 0-3)
    pub fn get_errors(&self) -> Result<Vec<JournalEntry>> {
        self.read_entries(&JournalFilter {
            priority: Some(3), // ERR and above
            ..Default::default()
        })
    }

    /// Get entries with warnings (priority 4)
    pub fn get_warnings(&self) -> Result<Vec<JournalEntry>> {
        self.read_entries(&JournalFilter {
            priority: Some(4), // WARNING
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_journal_filter_default() {
        let filter = JournalFilter::default();
        assert!(filter.priority.is_none());
        assert!(filter.unit.is_none());
    }

    #[test]
    fn test_journal_stats_new() {
        let stats = JournalStats::new();
        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.error_count, 0);
        assert_eq!(stats.warning_count, 0);
    }

    #[test]
    fn test_journal_stats_add_entry() {
        let mut stats = JournalStats::new();

        let entry1 = JournalEntry {
            timestamp: 0,
            priority: 3, // ERR
            unit: Some("test.service".to_string()),
            message: "Error".to_string(),
            pid: None,
            fields: HashMap::new(),
        };

        let entry2 = JournalEntry {
            timestamp: 0,
            priority: 4, // WARNING
            unit: Some("test.service".to_string()),
            message: "Warning".to_string(),
            pid: None,
            fields: HashMap::new(),
        };

        stats.add_entry(&entry1);
        stats.add_entry(&entry2);

        assert_eq!(stats.total_entries, 2);
        assert_eq!(stats.error_count, 1);
        assert_eq!(stats.warning_count, 1);
        assert_eq!(stats.by_unit.get("test.service"), Some(&2));
    }

    #[test]
    fn test_journal_reader_creation() {
        let analyzer = SystemdAnalyzer::new("/tmp");
        let _reader = JournalReader::new(analyzer);
    }
}
