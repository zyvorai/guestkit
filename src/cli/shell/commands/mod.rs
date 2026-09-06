// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Command implementations for interactive shell

mod ai;
mod analysis;
mod core;
mod navigation;
mod workflow;

pub use self::core::*;
pub use ai::*;
pub use analysis::*;
pub use navigation::*;
pub use workflow::*;

use crate::Guestfs;
use std::collections::HashMap;
use std::time::Instant;

pub struct ShellContext {
    pub guestfs: Guestfs,
    pub root: String,
    pub current_path: String,
    pub aliases: HashMap<String, String>,
    pub bookmarks: HashMap<String, String>,
    pub last_command_time: Option<std::time::Duration>,
    pub command_count: usize,
    pub os_info: String,
}

impl ShellContext {
    pub fn new(guestfs: Guestfs, root: String) -> Self {
        let mut aliases = HashMap::new();

        // Add default aliases
        aliases.insert("ll".to_string(), "ls -l".to_string());
        aliases.insert("la".to_string(), "ls -a".to_string());
        aliases.insert("..".to_string(), "cd ..".to_string());
        aliases.insert("~".to_string(), "cd /".to_string());
        aliases.insert("q".to_string(), "quit".to_string());

        Self {
            guestfs,
            root,
            current_path: "/".to_string(),
            aliases,
            bookmarks: HashMap::new(),
            last_command_time: None,
            command_count: 0,
            os_info: String::new(),
        }
    }

    /// Get OS information for display
    pub fn get_os_info(&self) -> &str {
        if self.os_info.is_empty() {
            "Unknown OS"
        } else {
            &self.os_info
        }
    }

    /// Set OS information
    pub fn set_os_info(&mut self, info: String) {
        self.os_info = info;
    }

    /// Add an alias
    pub fn add_alias(&mut self, name: String, command: String) {
        self.aliases.insert(name, command);
    }

    /// Get alias expansion
    pub fn get_alias(&self, name: &str) -> Option<&String> {
        self.aliases.get(name)
    }

    /// Add a bookmark
    pub fn add_bookmark(&mut self, name: String, path: String) {
        self.bookmarks.insert(name, path);
    }

    /// Get bookmark path
    pub fn get_bookmark(&self, name: &str) -> Option<&String> {
        self.bookmarks.get(name)
    }

    /// Start timing a command
    #[allow(dead_code)]
    pub fn start_timing(&mut self) -> Instant {
        Instant::now()
    }

    /// End timing and store duration
    pub fn end_timing(&mut self, start: Instant) {
        self.last_command_time = Some(start.elapsed());
        self.command_count += 1;
    }
}

/// Helper: Resolve relative path
pub fn resolve_path(current: &str, path: &str) -> String {
    let full_path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("{}/{}", current.trim_end_matches('/'), path)
    };

    // Normalize the path: resolve `.`, `..`, and collapse multiple slashes
    let mut components = Vec::new();
    for part in full_path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                components.pop();
            }
            other => components.push(other),
        }
    }

    let normalized = format!("/{}", components.join("/"));
    if normalized.is_empty() {
        "/".to_string()
    } else {
        normalized
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_idx])
}
