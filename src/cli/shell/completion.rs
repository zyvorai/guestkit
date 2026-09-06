// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Tab completion for interactive shell

use rustyline::completion::{Completer, Pair};
use rustyline::Context;
use rustyline::Result;

#[allow(dead_code)]
pub struct ShellCompleter {
    commands: Vec<String>,
}

#[allow(dead_code)]
impl ShellCompleter {
    pub fn new() -> Self {
        Self {
            commands: vec![
                "ls".to_string(),
                "cat".to_string(),
                "cd".to_string(),
                "pwd".to_string(),
                "find".to_string(),
                "grep".to_string(),
                "info".to_string(),
                "mounts".to_string(),
                "packages".to_string(),
                "services".to_string(),
                "users".to_string(),
                "network".to_string(),
                "security".to_string(),
                "health".to_string(),
                "risks".to_string(),
                "help".to_string(),
                "clear".to_string(),
                "history".to_string(),
                "exit".to_string(),
                "quit".to_string(),
            ],
        }
    }
}

impl Completer for ShellCompleter {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Result<(usize, Vec<Pair>)> {
        let mut candidates = Vec::new();

        // Get the word being completed
        let start = line[..pos].rfind(' ').map(|i| i + 1).unwrap_or(0);
        let word = &line[start..pos];

        // Complete commands
        for cmd in &self.commands {
            if cmd.starts_with(word) {
                candidates.push(Pair {
                    display: cmd.clone(),
                    replacement: cmd.clone(),
                });
            }
        }

        Ok((start, candidates))
    }
}

impl Default for ShellCompleter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustyline::completion::Completer;

    #[test]
    fn test_completer_creation() {
        let completer = ShellCompleter::new();
        assert!(!completer.commands.is_empty());
    }

    #[test]
    fn test_completer_default() {
        let completer = ShellCompleter::default();
        assert!(!completer.commands.is_empty());
    }

    #[test]
    fn test_completer_has_common_commands() {
        let completer = ShellCompleter::new();
        assert!(completer.commands.contains(&"ls".to_string()));
        assert!(completer.commands.contains(&"cat".to_string()));
        assert!(completer.commands.contains(&"cd".to_string()));
        assert!(completer.commands.contains(&"pwd".to_string()));
    }

    #[test]
    fn test_completer_has_guestkit_commands() {
        let completer = ShellCompleter::new();
        assert!(completer.commands.contains(&"packages".to_string()));
        assert!(completer.commands.contains(&"services".to_string()));
        assert!(completer.commands.contains(&"users".to_string()));
        assert!(completer.commands.contains(&"network".to_string()));
        assert!(completer.commands.contains(&"security".to_string()));
    }

    #[test]
    fn test_completer_has_control_commands() {
        let completer = ShellCompleter::new();
        assert!(completer.commands.contains(&"help".to_string()));
        assert!(completer.commands.contains(&"clear".to_string()));
        assert!(completer.commands.contains(&"history".to_string()));
        assert!(completer.commands.contains(&"exit".to_string()));
        assert!(completer.commands.contains(&"quit".to_string()));
    }

    #[test]
    fn test_completion_exact_match() {
        let completer = ShellCompleter::new();
        let history = rustyline::history::DefaultHistory::new();
        let ctx = Context::new(&history);
        let (start, candidates) = completer.complete("ls", 2, &ctx).unwrap();

        assert_eq!(start, 0);
        assert!(candidates.iter().any(|c| c.replacement == "ls"));
    }

    #[test]
    fn test_completion_prefix_match() {
        let completer = ShellCompleter::new();
        let history = rustyline::history::DefaultHistory::new();
        let ctx = Context::new(&history);
        let (start, candidates) = completer.complete("pa", 2, &ctx).unwrap();

        assert_eq!(start, 0);
        assert!(candidates.iter().any(|c| c.replacement == "packages"));
    }

    #[test]
    fn test_completion_multiple_matches() {
        let completer = ShellCompleter::new();
        let history = rustyline::history::DefaultHistory::new();
        let ctx = Context::new(&history);
        let (start, candidates) = completer.complete("c", 1, &ctx).unwrap();

        assert_eq!(start, 0);
        // Should match: cat, cd, clear
        assert!(candidates.len() >= 3);
        assert!(candidates.iter().any(|c| c.replacement == "cat"));
        assert!(candidates.iter().any(|c| c.replacement == "cd"));
        assert!(candidates.iter().any(|c| c.replacement == "clear"));
    }

    #[test]
    fn test_completion_no_match() {
        let completer = ShellCompleter::new();
        let history = rustyline::history::DefaultHistory::new();
        let ctx = Context::new(&history);
        let (start, candidates) = completer.complete("xyz", 3, &ctx).unwrap();

        assert_eq!(start, 0);
        assert_eq!(candidates.len(), 0);
    }

    #[test]
    fn test_completion_with_spaces() {
        let completer = ShellCompleter::new();
        let history = rustyline::history::DefaultHistory::new();
        let ctx = Context::new(&history);
        let (start, candidates) = completer.complete("ls -l pa", 8, &ctx).unwrap();

        // Should start from position after last space
        assert_eq!(start, 6);
        assert!(candidates.iter().any(|c| c.replacement == "packages"));
    }

    #[test]
    fn test_completion_empty_input() {
        let completer = ShellCompleter::new();
        let history = rustyline::history::DefaultHistory::new();
        let ctx = Context::new(&history);
        let (start, candidates) = completer.complete("", 0, &ctx).unwrap();

        assert_eq!(start, 0);
        // Should return all commands when no prefix
        assert_eq!(candidates.len(), completer.commands.len());
    }

    #[test]
    fn test_completion_case_sensitive() {
        let completer = ShellCompleter::new();
        let history = rustyline::history::DefaultHistory::new();
        let ctx = Context::new(&history);
        let (_, candidates) = completer.complete("LS", 2, &ctx).unwrap();

        // Should not match because case doesn't match
        assert_eq!(candidates.len(), 0);
    }

    #[test]
    fn test_completion_partial_word() {
        let completer = ShellCompleter::new();
        let history = rustyline::history::DefaultHistory::new();
        let ctx = Context::new(&history);
        let (start, candidates) = completer.complete("se", 2, &ctx).unwrap();

        assert_eq!(start, 0);
        assert!(candidates.iter().any(|c| c.replacement == "services"));
        assert!(candidates.iter().any(|c| c.replacement == "security"));
    }

    #[test]
    fn test_completion_display_matches_replacement() {
        let completer = ShellCompleter::new();
        let history = rustyline::history::DefaultHistory::new();
        let ctx = Context::new(&history);
        let (_, candidates) = completer.complete("net", 3, &ctx).unwrap();

        for candidate in &candidates {
            assert_eq!(candidate.display, candidate.replacement);
        }
    }

    #[test]
    fn test_completion_midword() {
        let completer = ShellCompleter::new();
        let history = rustyline::history::DefaultHistory::new();
        let ctx = Context::new(&history);

        // Cursor in the middle of a word
        let (start, candidates) = completer.complete("pack", 2, &ctx).unwrap();

        // Should start from beginning since no space before cursor
        assert_eq!(start, 0);
        assert!(candidates.iter().any(|c| c.replacement == "packages"));
    }

    #[test]
    fn test_completion_after_command() {
        let completer = ShellCompleter::new();
        let history = rustyline::history::DefaultHistory::new();
        let ctx = Context::new(&history);
        let (start, candidates) = completer.complete("ls ", 3, &ctx).unwrap();

        // After a complete command with space, should complete from that position
        assert_eq!(start, 3);
        // With empty prefix, should return all commands
        assert_eq!(candidates.len(), completer.commands.len());
    }
}
