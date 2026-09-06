// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Sed-like file editing operations for disk image manipulation
//!
//! This implementation provides in-place file editing functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use regex::Regex;

impl Guestfs {
    /// Edit file using sed-like expressions
    ///
    pub fn sed(&mut self, expression: &str, path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: sed {} {}", expression, path);
        }

        let host_path = self.resolve_guest_path(path)?;
        let content = std::fs::read_to_string(&host_path).map_err(Error::Io)?;

        // Parse sed expression (simple s/pattern/replacement/flags)
        if let Some(expr_body) = expression.strip_prefix("s/") {
            let parts: Vec<&str> = expr_body.splitn(3, '/').collect();
            if parts.len() >= 2 {
                let pattern = parts[0];
                let replacement = parts[1];
                let flags = if parts.len() == 3 { parts[2] } else { "" };

                let re = Regex::new(pattern)
                    .map_err(|e| Error::InvalidFormat(format!("Invalid regex: {}", e)))?;

                let new_content = if flags.contains('g') {
                    re.replace_all(&content, replacement).to_string()
                } else {
                    re.replace(&content, replacement).to_string()
                };

                std::fs::write(&host_path, new_content).map_err(Error::Io)?;
            } else {
                return Err(Error::InvalidFormat("Invalid sed expression".to_string()));
            }
        } else {
            return Err(Error::InvalidFormat(
                "Only s/// expressions are supported".to_string(),
            ));
        }

        Ok(())
    }

    /// Edit file using sed expressions (from file)
    ///
    pub fn sed_file(&mut self, sedfile: &str, path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: sed_file {} {}", sedfile, path);
        }

        let expressions = std::fs::read_to_string(sedfile).map_err(Error::Io)?;

        for expr in expressions.lines() {
            if !expr.is_empty() && !expr.starts_with('#') {
                self.sed(expr, path)?;
            }
        }

        Ok(())
    }

    /// Replace all occurrences in file
    ///
    /// Additional functionality for simple replacements
    pub fn replace_all(&mut self, path: &str, pattern: &str, replacement: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: replace_all {} {} {}", path, pattern, replacement);
        }

        let host_path = self.resolve_guest_path(path)?;
        let content = std::fs::read_to_string(&host_path).map_err(Error::Io)?;

        let new_content = content.replace(pattern, replacement);

        std::fs::write(&host_path, new_content).map_err(Error::Io)?;

        Ok(())
    }

    /// Replace first occurrence in file
    ///
    /// Additional functionality for simple replacements
    pub fn replace_first(&mut self, path: &str, pattern: &str, replacement: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!(
                "guestfs: replace_first {} {} {}",
                path, pattern, replacement
            );
        }

        let host_path = self.resolve_guest_path(path)?;
        let content = std::fs::read_to_string(&host_path).map_err(Error::Io)?;

        let new_content = content.replacen(pattern, replacement, 1);

        std::fs::write(&host_path, new_content).map_err(Error::Io)?;

        Ok(())
    }

    /// Delete lines matching pattern
    ///
    /// Additional functionality for line deletion
    pub fn delete_lines(&mut self, path: &str, pattern: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: delete_lines {} {}", path, pattern);
        }

        let host_path = self.resolve_guest_path(path)?;
        let content = std::fs::read_to_string(&host_path).map_err(Error::Io)?;

        let re = Regex::new(pattern)
            .map_err(|e| Error::InvalidFormat(format!("Invalid regex: {}", e)))?;

        let new_content: String = content
            .lines()
            .filter(|line| !re.is_match(line))
            .collect::<Vec<_>>()
            .join("\n");

        std::fs::write(&host_path, new_content + "\n").map_err(Error::Io)?;

        Ok(())
    }

    /// Insert line before pattern
    ///
    /// Additional functionality for line insertion
    pub fn insert_before(&mut self, path: &str, pattern: &str, line: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: insert_before {} {} {}", path, pattern, line);
        }

        let host_path = self.resolve_guest_path(path)?;
        let content = std::fs::read_to_string(&host_path).map_err(Error::Io)?;

        let re = Regex::new(pattern)
            .map_err(|e| Error::InvalidFormat(format!("Invalid regex: {}", e)))?;

        let mut new_lines = Vec::new();
        for existing_line in content.lines() {
            if re.is_match(existing_line) {
                new_lines.push(line.to_string());
            }
            new_lines.push(existing_line.to_string());
        }

        std::fs::write(&host_path, new_lines.join("\n") + "\n").map_err(Error::Io)?;

        Ok(())
    }

    /// Append line after pattern
    ///
    /// Additional functionality for line insertion
    pub fn append_after(&mut self, path: &str, pattern: &str, line: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: append_after {} {} {}", path, pattern, line);
        }

        let host_path = self.resolve_guest_path(path)?;
        let content = std::fs::read_to_string(&host_path).map_err(Error::Io)?;

        let re = Regex::new(pattern)
            .map_err(|e| Error::InvalidFormat(format!("Invalid regex: {}", e)))?;

        let mut new_lines = Vec::new();
        for existing_line in content.lines() {
            new_lines.push(existing_line.to_string());
            if re.is_match(existing_line) {
                new_lines.push(line.to_string());
            }
        }

        std::fs::write(&host_path, new_lines.join("\n") + "\n").map_err(Error::Io)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sed_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
