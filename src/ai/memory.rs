// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Cross-run memory for the AI copilot — lets repeated `doctor --ai` /
//! `migrate-plan --ai` runs against the *same* VM see a summary of what
//! was found last time, instead of every invocation starting from zero.
//!
//! One JSON file per VM (keyed by canonicalized image path), capped to the
//! most recent [`MAX_ENTRIES`] runs, under `dirs::cache_dir()/guestkit/ai-memory`
//! (override with `GUESTKIT_AI_MEMORY_DIR`; disable entirely with
//! `GUESTKIT_AI_MEMORY=0`). Follows the same versioned-JSON-file-per-key
//! convention as `cli/tui/cache.rs` and `core/binary_cache.rs`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: u32 = 1;
const MAX_ENTRIES: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub timestamp: String,
    pub query: String,
    pub answer: String,
    pub boot_score: Option<f64>,
    pub tool_call_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMemory {
    schema_version: u32,
    pub image_key: String,
    pub first_seen: String,
    pub last_seen: String,
    pub entries: Vec<MemoryEntry>,
}

/// Whether cross-run memory is enabled. On by default.
pub fn memory_enabled() -> bool {
    match std::env::var("GUESTKIT_AI_MEMORY") {
        Ok(v) => !(v == "0" || v.eq_ignore_ascii_case("false") || v.eq_ignore_ascii_case("off")),
        Err(_) => true,
    }
}

fn memory_root() -> Result<PathBuf> {
    let root = if let Ok(dir) = std::env::var("GUESTKIT_AI_MEMORY_DIR") {
        PathBuf::from(dir)
    } else {
        dirs::cache_dir()
            .context("could not determine cache directory")?
            .join("guestkit")
            .join("ai-memory")
    };
    fs::create_dir_all(&root)?;
    Ok(root)
}

/// Stable key for a VM's memory file — canonicalized image path, hashed.
/// Deliberately does *not* include mtime/size (unlike `cli/cache.rs`'s
/// evidence cache key): the point of memory is continuity across the VM
/// changing between runs (e.g. re-running doctor after applying a fix),
/// not invalidation on change.
fn image_key(image_path: &Path) -> String {
    let canonical = image_path
        .canonicalize()
        .unwrap_or_else(|_| image_path.to_path_buf());
    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string_lossy().as_bytes());
    format!("{:x}", hasher.finalize())
}

fn memory_path(image_path: &Path) -> Result<PathBuf> {
    Ok(memory_root()?.join(format!("{}-v{SCHEMA_VERSION}.json", image_key(image_path))))
}

/// Load prior memory for a VM, if any exists and memory isn't disabled.
pub fn load(image_path: &Path) -> Option<AgentMemory> {
    if !memory_enabled() {
        return None;
    }
    let path = memory_path(image_path).ok()?;
    let data = fs::read_to_string(&path).ok()?;
    match serde_json::from_str::<AgentMemory>(&data) {
        Ok(mem) if mem.schema_version == SCHEMA_VERSION => Some(mem),
        _ => None, // stale schema or corrupt file — treat as no memory, don't error the run
    }
}

/// Append a run's result to this VM's memory, capped to the most recent
/// `MAX_ENTRIES`, and save atomically (write to `.partial`, then rename).
pub fn record(image_path: &Path, entry: MemoryEntry) -> Result<()> {
    if !memory_enabled() {
        return Ok(());
    }
    let path = memory_path(image_path)?;
    let key = image_key(image_path);
    let mut mem = load(image_path).unwrap_or_else(|| AgentMemory {
        schema_version: SCHEMA_VERSION,
        image_key: key,
        first_seen: entry.timestamp.clone(),
        last_seen: entry.timestamp.clone(),
        entries: Vec::new(),
    });
    mem.last_seen = entry.timestamp.clone();
    mem.entries.push(entry);
    if mem.entries.len() > MAX_ENTRIES {
        let drop = mem.entries.len() - MAX_ENTRIES;
        mem.entries.drain(0..drop);
    }

    let json = serde_json::to_string_pretty(&mem)?;
    let partial = path.with_extension("json.partial");
    fs::write(&partial, json)
        .with_context(|| format!("write AI memory to {}", partial.display()))?;
    fs::rename(&partial, &path)
        .with_context(|| format!("finalize AI memory at {}", path.display()))?;
    Ok(())
}

/// Render prior findings as a short block to inject into the agent's
/// context, most-recent-first. Empty string if there's no prior memory.
pub fn context_summary(mem: &AgentMemory, max_entries: usize) -> String {
    if mem.entries.is_empty() {
        return String::new();
    }
    let mut out = format!(
        "Prior findings on this VM ({} previous run(s), first seen {}):\n",
        mem.entries.len(),
        mem.first_seen
    );
    for entry in mem.entries.iter().rev().take(max_entries) {
        let score = entry
            .boot_score
            .map(|s| format!("{s:.0}"))
            .unwrap_or_else(|| "n/a".to_string());
        out.push_str(&format!(
            "- [{}] score={score} query=\"{}\" → {}\n",
            entry.timestamp,
            truncate(&entry.query, 80),
            truncate(&entry.answer, 200)
        ));
    }
    out
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars).collect();
    format!("{truncated}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // GUESTKIT_AI_MEMORY_DIR / GUESTKIT_AI_MEMORY are process-global env
    // vars — serialize tests that touch them so they don't race.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn records_and_loads_across_calls() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("GUESTKIT_AI_MEMORY_DIR", dir.path());
        std::env::remove_var("GUESTKIT_AI_MEMORY");

        let image = dir.path().join("disk.qcow2");
        std::fs::write(&image, b"fake").unwrap();

        assert!(load(&image).is_none());

        record(
            &image,
            MemoryEntry {
                timestamp: "2026-01-01T00:00:00Z".into(),
                query: "why won't it boot".into(),
                answer: "missing GRUB config".into(),
                boot_score: Some(42.0),
                tool_call_count: 2,
            },
        )
        .unwrap();

        let mem = load(&image).expect("memory should persist");
        assert_eq!(mem.entries.len(), 1);
        assert_eq!(mem.entries[0].boot_score, Some(42.0));

        let summary = context_summary(&mem, 5);
        assert!(summary.contains("missing GRUB config"));

        std::env::remove_var("GUESTKIT_AI_MEMORY_DIR");
    }

    #[test]
    fn caps_at_max_entries() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("GUESTKIT_AI_MEMORY_DIR", dir.path());
        std::env::remove_var("GUESTKIT_AI_MEMORY");

        let image = dir.path().join("disk2.qcow2");
        std::fs::write(&image, b"fake").unwrap();

        for i in 0..(MAX_ENTRIES + 5) {
            record(
                &image,
                MemoryEntry {
                    timestamp: format!("2026-01-01T00:00:{i:02}Z"),
                    query: format!("query {i}"),
                    answer: format!("answer {i}"),
                    boot_score: None,
                    tool_call_count: 0,
                },
            )
            .unwrap();
        }

        let mem = load(&image).unwrap();
        assert_eq!(mem.entries.len(), MAX_ENTRIES);
        // oldest entries were dropped, newest kept
        assert_eq!(mem.entries.last().unwrap().query, "query 24");

        std::env::remove_var("GUESTKIT_AI_MEMORY_DIR");
    }

    #[test]
    fn disabled_via_env_is_a_noop() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("GUESTKIT_AI_MEMORY_DIR", dir.path());
        std::env::set_var("GUESTKIT_AI_MEMORY", "0");

        let image = dir.path().join("disk3.qcow2");
        std::fs::write(&image, b"fake").unwrap();

        record(
            &image,
            MemoryEntry {
                timestamp: "2026-01-01T00:00:00Z".into(),
                query: "q".into(),
                answer: "a".into(),
                boot_score: None,
                tool_call_count: 0,
            },
        )
        .unwrap();

        assert!(load(&image).is_none());

        std::env::remove_var("GUESTKIT_AI_MEMORY_DIR");
        std::env::remove_var("GUESTKIT_AI_MEMORY");
    }
}
