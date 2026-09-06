// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Interactive explore command for visual filesystem navigation

use anyhow::{Context, Result};
use colored::Colorize;
use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use std::io::{stdout, Write};

use super::commands::ShellContext;

/// File entry in the explorer
#[derive(Clone)]
struct FileEntry {
    name: String,
    is_dir: bool,
    size: i64,
}

/// Explorer state
struct ExplorerState {
    current_path: String,
    entries: Vec<FileEntry>,
    selected: usize,
    scroll_offset: usize,
    filter: String,
    show_hidden: bool,
    sort_by: SortMode,
    panel_height: u16,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum SortMode {
    Name,
    Size,
    Type,
}

impl ExplorerState {
    fn new(path: String) -> Self {
        Self {
            current_path: path,
            entries: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            filter: String::new(),
            show_hidden: false,
            sort_by: SortMode::Name,
            panel_height: 20,
        }
    }

    fn navigate_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            if self.selected < self.scroll_offset {
                self.scroll_offset = self.selected;
            }
        }
    }

    fn navigate_down(&mut self) {
        if self.selected < self.entries.len().saturating_sub(1) {
            self.selected += 1;
            let visible_lines = self.panel_height as usize;
            if self.selected >= self.scroll_offset + visible_lines {
                self.scroll_offset = self.selected - visible_lines + 1;
            }
        }
    }

    fn page_up(&mut self) {
        let page_size = (self.panel_height as usize).saturating_sub(2);
        self.selected = self.selected.saturating_sub(page_size);
        self.scroll_offset = self.scroll_offset.saturating_sub(page_size);
    }

    fn page_down(&mut self) {
        let page_size = (self.panel_height as usize).saturating_sub(2);
        let max_idx = self.entries.len().saturating_sub(1);
        self.selected = (self.selected + page_size).min(max_idx);

        let visible_lines = self.panel_height as usize;
        if self.selected >= self.scroll_offset + visible_lines {
            self.scroll_offset = self.selected.saturating_sub(visible_lines - 1);
        }
    }

    fn get_selected_entry(&self) -> Option<&FileEntry> {
        self.entries.get(self.selected)
    }

    /// Get the parent directory path
    fn parent_path(&self) -> String {
        let parts: Vec<&str> = self.current_path.rsplitn(2, '/').collect();
        if parts.len() > 1 && !parts[1].is_empty() {
            format!("/{}", parts[1])
        } else {
            "/".to_string()
        }
    }

    /// Reset navigation state after changing directory
    fn reset_navigation(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;
        self.filter.clear();
    }

    fn apply_filter(&mut self) {
        if !self.filter.is_empty() {
            self.entries
                .retain(|e| e.name.to_lowercase().contains(&self.filter.to_lowercase()));
        }

        if self.selected >= self.entries.len() {
            self.selected = self.entries.len().saturating_sub(1);
            self.scroll_offset = 0;
        }
    }

    fn sort_entries(&mut self) {
        match self.sort_by {
            SortMode::Name => {
                self.entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                });
            }
            SortMode::Size => {
                self.entries.sort_by_key(|b| std::cmp::Reverse(b.size));
            }
            SortMode::Type => {
                self.entries.sort_by(|a, b| {
                    let ext_a = get_extension(&a.name);
                    let ext_b = get_extension(&b.name);
                    ext_a.cmp(ext_b).then(a.name.cmp(&b.name))
                });
            }
        }
    }
}

fn get_extension(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or("")
}

fn join_path(base: &str, name: &str) -> String {
    if base == "/" {
        format!("/{}", name)
    } else {
        format!("{}/{}", base, name)
    }
}

fn format_size(size: i64) -> String {
    crate::cli::output::format_size(size as u64)
}

fn get_file_icon(entry: &FileEntry) -> &'static str {
    if entry.is_dir {
        "📁"
    } else {
        let ext = get_extension(&entry.name).to_lowercase();
        match ext.as_str() {
            "txt" | "md" | "log" => "📄",
            "rs" | "py" | "js" | "java" | "c" | "cpp" | "go" => "💻",
            "json" | "yaml" | "yml" | "toml" | "xml" => "⚙️ ",
            "jpg" | "png" | "gif" | "bmp" | "svg" => "🖼️ ",
            "pdf" => "📕",
            "zip" | "tar" | "gz" | "bz2" | "xz" => "📦",
            "sh" | "bash" => "🔧",
            "conf" | "config" | "cfg" => "🔐",
            _ => "📝",
        }
    }
}

fn get_file_color(entry: &FileEntry) -> colored::Color {
    if entry.is_dir {
        colored::Color::Blue
    } else if entry.name.starts_with('.') {
        colored::Color::BrightBlack
    } else {
        let ext = get_extension(&entry.name).to_lowercase();
        match ext.as_str() {
            "sh" | "bash" | "py" | "rb" => colored::Color::Green,
            "rs" | "c" | "cpp" | "java" | "go" => colored::Color::Yellow,
            "txt" | "md" | "log" => colored::Color::White,
            "conf" | "config" | "cfg" | "yaml" | "yml" | "json" | "toml" => colored::Color::Cyan,
            "tar" | "gz" | "zip" | "bz2" => colored::Color::Red,
            _ => colored::Color::White,
        }
    }
}

/// Load directory entries
fn load_entries(ctx: &mut ShellContext, path: &str, show_hidden: bool) -> Result<Vec<FileEntry>> {
    let mut entries = Vec::new();

    // Add parent directory entry if not at root
    if path != "/" {
        entries.push(FileEntry {
            name: "..".to_string(),
            is_dir: true,
            size: 0,
        });
    }

    // List directory contents
    let files = ctx.guestfs.ls(path).context("Failed to list directory")?;

    for file in files {
        // Skip hidden files if not showing them
        if !show_hidden && file.starts_with('.') {
            continue;
        }

        let full_path = join_path(path, &file);

        // Get file stats
        let is_dir = ctx.guestfs.is_dir(&full_path).unwrap_or(false);
        let size = if !is_dir {
            ctx.guestfs.filesize(&full_path).unwrap_or(0)
        } else {
            0
        };

        entries.push(FileEntry {
            name: file,
            is_dir,
            size,
        });
    }

    Ok(entries)
}

/// Draw the explorer UI
fn draw_explorer(state: &ExplorerState, ctx: &ShellContext) -> Result<()> {
    let mut stdout = stdout();

    execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;

    // Header
    println!(
        "{}",
        "╔═══════════════════════════════════════════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        format!("║ {} GuestKit File Explorer - {}", "📂", ctx.get_os_info()).cyan()
    );
    println!(
        "{}",
        "╠═══════════════════════════════════════════════════════════════════════════════╣".cyan()
    );

    // Current path with breadcrumbs
    let breadcrumb = if state.current_path.len() > 70 {
        format!(
            "...{}",
            &state.current_path[state.current_path.len() - 67..]
        )
    } else {
        state.current_path.clone()
    };
    println!(
        "{} {}",
        "📍 Path:".yellow().bold(),
        breadcrumb.bright_white()
    );

    // File count and filter
    let visible_count = state.entries.len();
    let filter_text = if !state.filter.is_empty() {
        format!(" (filter: '{}')", state.filter)
    } else {
        String::new()
    };

    println!(
        "{} {}{}",
        "📊 Items:".yellow().bold(),
        visible_count.to_string().bright_white(),
        filter_text.bright_black()
    );

    println!(
        "{}",
        "├───────────────────────────────────────────────────────────────────────────────┤".cyan()
    );

    // File list
    let visible_lines = state.panel_height as usize;
    let end_idx = (state.scroll_offset + visible_lines).min(state.entries.len());

    for (idx, entry) in state.entries[state.scroll_offset..end_idx]
        .iter()
        .enumerate()
    {
        let global_idx = state.scroll_offset + idx;
        let is_selected = global_idx == state.selected;

        let icon = get_file_icon(entry);
        let color = get_file_color(entry);

        let size_str = if entry.is_dir && entry.name != ".." {
            "<DIR>".to_string()
        } else if entry.name == ".." {
            "".to_string()
        } else {
            format_size(entry.size)
        };

        let name_display = if entry.name.len() > 50 {
            format!("{}...", &entry.name[..47])
        } else {
            entry.name.clone()
        };

        if is_selected {
            println!(
                "{} {} {:<50} {:>12}",
                "▸".bright_yellow().bold(),
                icon,
                name_display.color(color).bold(),
                size_str.bright_black()
            );
        } else {
            println!(
                "  {} {:<50} {:>12}",
                icon,
                name_display.color(color),
                size_str.bright_black()
            );
        }
    }

    // Fill remaining lines
    for _ in (end_idx - state.scroll_offset)..visible_lines {
        println!();
    }

    println!(
        "{}",
        "├───────────────────────────────────────────────────────────────────────────────┤".cyan()
    );

    // Selected file info
    if let Some(entry) = state.get_selected_entry() {
        if entry.name != ".." {
            let file_type = if entry.is_dir { "Directory" } else { "File" };
            println!(
                "{} {} | {} {}",
                "ℹ️  Info:".yellow().bold(),
                file_type.bright_white(),
                "Size:".yellow(),
                format_size(entry.size).bright_white()
            );
        } else {
            println!("{} Parent directory", "ℹ️  Info:".yellow().bold());
        }
    }

    println!(
        "{}",
        "╠═══════════════════════════════════════════════════════════════════════════════╣".cyan()
    );

    // Help bar
    println!(
        "{}",
        format!(
            "║ {} {} {} {} {} {} {} {}",
            "↑↓".green().bold(),
            "Navigate".bright_black(),
            "Enter".green().bold(),
            "Open".bright_black(),
            "h".green().bold(),
            "Help".bright_black(),
            "q".green().bold(),
            "Quit".bright_black(),
        )
        .cyan()
    );

    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════════════════════════╝".cyan()
    );

    stdout.flush()?;
    Ok(())
}

/// Show help overlay
fn show_help() -> Result<()> {
    println!(
        "\n{}",
        "╔════════════════════ Explorer Help ═══════════════════╗"
            .bright_cyan()
            .bold()
    );
    println!(
        "{}",
        "║                                                       ║".cyan()
    );
    println!(
        "{}",
        format!(
            "║ {} Navigation                                       ║",
            "📖"
        )
        .cyan()
    );
    println!(
        "{}",
        "║   ↑/↓ or k/j    - Move selection up/down              ║".cyan()
    );
    println!(
        "{}",
        "║   PgUp/PgDn     - Page up/down                        ║".cyan()
    );
    println!(
        "{}",
        "║   Enter         - Enter directory / view file        ║".cyan()
    );
    println!(
        "{}",
        "║   Backspace     - Go to parent directory             ║".cyan()
    );
    println!(
        "{}",
        "║                                                       ║".cyan()
    );
    println!(
        "{}",
        format!(
            "║ {} Actions                                         ║",
            "⚡"
        )
        .cyan()
    );
    println!(
        "{}",
        "║   v             - View file content                  ║".cyan()
    );
    println!(
        "{}",
        "║   i             - Show file info                     ║".cyan()
    );
    println!(
        "{}",
        "║   /             - Filter files                       ║".cyan()
    );
    println!(
        "{}",
        "║   .             - Toggle hidden files                ║".cyan()
    );
    println!(
        "{}",
        "║   s             - Cycle sort mode                    ║".cyan()
    );
    println!(
        "{}",
        "║                                                       ║".cyan()
    );
    println!(
        "{}",
        format!(
            "║ {} General                                         ║",
            "🔧"
        )
        .cyan()
    );
    println!(
        "{}",
        "║   h or ?        - Show this help                     ║".cyan()
    );
    println!(
        "{}",
        "║   q or Esc      - Exit explorer                      ║".cyan()
    );
    println!(
        "{}",
        "║   Ctrl+C        - Force exit                         ║".cyan()
    );
    println!(
        "{}",
        "║                                                       ║".cyan()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════╝"
            .bright_cyan()
            .bold()
    );
    println!("\n{}", "Press any key to continue...".yellow());

    // Wait for any key
    loop {
        if let Event::Key(_) = event::read()? {
            break;
        }
    }

    Ok(())
}

/// View file content
fn view_file(ctx: &mut ShellContext, path: &str) -> Result<()> {
    let content = ctx.guestfs.cat(path).context("Failed to read file")?;

    // Show in pager-like view
    println!("\n{}", format!("╔═ Viewing: {} ═╗", path).cyan().bold());

    let lines: Vec<&str> = content.lines().collect();
    let max_lines = 100; // Show first 100 lines

    for (i, line) in lines.iter().take(max_lines).enumerate() {
        println!("{:4} │ {}", (i + 1).to_string().bright_black(), line);
    }

    if lines.len() > max_lines {
        println!(
            "\n{}",
            format!("... ({} more lines)", lines.len() - max_lines).yellow()
        );
    }

    println!("\n{}", "Press any key to return...".yellow());

    // Wait for key
    loop {
        if let Event::Key(_) = event::read()? {
            break;
        }
    }

    Ok(())
}

/// Show file information
fn show_file_info(ctx: &mut ShellContext, path: &str, entry: &FileEntry) -> Result<()> {
    println!(
        "\n{}",
        format!("╔═ File Information: {} ═╗", entry.name)
            .cyan()
            .bold()
    );
    println!("{} {}", "Path:".yellow().bold(), path.bright_white());
    println!(
        "{} {}",
        "Type:".yellow().bold(),
        if entry.is_dir { "Directory" } else { "File" }.bright_white()
    );
    println!(
        "{} {}",
        "Size:".yellow().bold(),
        format_size(entry.size).bright_white()
    );

    // Try to get permissions
    if let Ok(stat) = ctx.guestfs.stat(path) {
        println!(
            "{} {}",
            "Mode:".yellow().bold(),
            format!("{:o}", stat.mode).bright_white()
        );
        println!(
            "{} {}",
            "UID:".yellow().bold(),
            stat.uid.to_string().bright_white()
        );
        println!(
            "{} {}",
            "GID:".yellow().bold(),
            stat.gid.to_string().bright_white()
        );
    }

    // For non-directories, show file type details
    if !entry.is_dir {
        if let Ok(file_type) = ctx.guestfs.file(path) {
            println!(
                "{} {}",
                "File Type:".yellow().bold(),
                file_type.bright_white()
            );
        }
    }

    println!("\n{}", "Press any key to return...".yellow());

    // Wait for key
    loop {
        if let Event::Key(_) = event::read()? {
            break;
        }
    }

    Ok(())
}

/// Get filter input from user
fn get_filter_input() -> Result<String> {
    println!("\n{}", "Enter filter (filename contains): ".yellow().bold());
    print!("> ");
    stdout().flush()?;

    let mut filter = String::new();
    std::io::stdin().read_line(&mut filter)?;

    Ok(filter.trim().to_string())
}

/// Run the interactive file explorer
pub fn run_explorer(ctx: &mut ShellContext, start_path: Option<&str>) -> Result<()> {
    let initial_path = start_path
        .map(|p| p.to_string())
        .unwrap_or_else(|| ctx.current_path.clone());

    let mut state = ExplorerState::new(initial_path.clone());

    // Load initial entries
    state.entries = load_entries(ctx, &state.current_path, state.show_hidden)?;
    state.sort_entries();

    // Enable raw mode for key capture
    enable_raw_mode()?;

    let result = explorer_loop(ctx, &mut state);

    // Disable raw mode before returning
    disable_raw_mode()?;

    // Clear screen
    execute!(stdout(), Clear(ClearType::All), MoveTo(0, 0))?;

    result
}

/// Main explorer event loop
fn explorer_loop(ctx: &mut ShellContext, state: &mut ExplorerState) -> Result<()> {
    loop {
        // Draw UI
        draw_explorer(state, ctx)?;

        // Handle input
        if let Event::Key(KeyEvent {
            code, modifiers, ..
        }) = event::read()?
        {
            match (code, modifiers) {
                // Navigation
                (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
                    state.navigate_up();
                }
                (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
                    state.navigate_down();
                }
                (KeyCode::PageUp, _) => {
                    state.page_up();
                }
                (KeyCode::PageDown, _) => {
                    state.page_down();
                }

                // Enter directory or view file
                (KeyCode::Enter, _) => {
                    if let Some(entry) = state.get_selected_entry().cloned() {
                        if entry.is_dir {
                            // Navigate into directory
                            let new_path = if entry.name == ".." {
                                // Go to parent
                                state.parent_path()
                            } else {
                                // Go into subdirectory
                                join_path(&state.current_path, &entry.name)
                            };

                            state.current_path = new_path;
                            state.entries =
                                load_entries(ctx, &state.current_path, state.show_hidden)?;
                            state.sort_entries();
                            state.reset_navigation();
                        } else {
                            // View file
                            disable_raw_mode()?;
                            let file_path = join_path(&state.current_path, &entry.name);
                            let _ = view_file(ctx, &file_path);
                            enable_raw_mode()?;
                        }
                    }
                }

                // Go to parent
                (KeyCode::Backspace, _) if state.current_path != "/" => {
                    state.current_path = state.parent_path();
                    state.entries = load_entries(ctx, &state.current_path, state.show_hidden)?;
                    state.sort_entries();
                    state.reset_navigation();
                }

                // View file content
                (KeyCode::Char('v'), KeyModifiers::NONE) => {
                    if let Some(entry) = state.get_selected_entry().cloned() {
                        if !entry.is_dir && entry.name != ".." {
                            disable_raw_mode()?;
                            let file_path = join_path(&state.current_path, &entry.name);
                            let _ = view_file(ctx, &file_path);
                            enable_raw_mode()?;
                        }
                    }
                }

                // Show file info
                (KeyCode::Char('i'), KeyModifiers::NONE) => {
                    if let Some(entry) = state.get_selected_entry().cloned() {
                        if entry.name != ".." {
                            disable_raw_mode()?;
                            let file_path = join_path(&state.current_path, &entry.name);
                            let _ = show_file_info(ctx, &file_path, &entry);
                            enable_raw_mode()?;
                        }
                    }
                }

                // Toggle hidden files
                (KeyCode::Char('.'), KeyModifiers::NONE) => {
                    state.show_hidden = !state.show_hidden;
                    state.entries = load_entries(ctx, &state.current_path, state.show_hidden)?;
                    state.sort_entries();
                    state.apply_filter();
                    state.reset_navigation();
                }

                // Cycle sort mode
                (KeyCode::Char('s'), KeyModifiers::NONE) => {
                    state.sort_by = match state.sort_by {
                        SortMode::Name => SortMode::Size,
                        SortMode::Size => SortMode::Type,
                        SortMode::Type => SortMode::Name,
                    };
                    state.sort_entries();
                }

                // Filter
                (KeyCode::Char('/'), KeyModifiers::NONE) => {
                    disable_raw_mode()?;
                    if let Ok(filter) = get_filter_input() {
                        state.filter = filter;
                        state.entries = load_entries(ctx, &state.current_path, state.show_hidden)?;
                        state.sort_entries();
                        state.apply_filter();
                        state.reset_navigation();
                    }
                    enable_raw_mode()?;
                }

                // Help
                (KeyCode::Char('h'), KeyModifiers::NONE)
                | (KeyCode::Char('?'), KeyModifiers::NONE) => {
                    disable_raw_mode()?;
                    let _ = show_help();
                    enable_raw_mode()?;
                }

                // Quit
                (KeyCode::Char('q'), KeyModifiers::NONE) | (KeyCode::Esc, _) => {
                    break;
                }

                // Force quit
                (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                    break;
                }

                _ => {}
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_entry(name: &str, is_dir: bool, size: i64) -> FileEntry {
        FileEntry {
            name: name.to_string(),
            is_dir,
            size,
        }
    }

    #[test]
    fn test_explorer_state_creation() {
        let state = ExplorerState::new("/home".to_string());
        assert_eq!(state.current_path, "/home");
        assert_eq!(state.selected, 0);
        assert_eq!(state.scroll_offset, 0);
        assert_eq!(state.filter, "");
        assert!(!state.show_hidden);
        assert!(matches!(state.sort_by, SortMode::Name));
    }

    #[test]
    fn test_navigate_up() {
        let mut state = ExplorerState::new("/".to_string());
        state.entries = vec![
            create_test_entry("file1.txt", false, 100),
            create_test_entry("file2.txt", false, 200),
            create_test_entry("file3.txt", false, 300),
        ];
        state.selected = 2;

        state.navigate_up();
        assert_eq!(state.selected, 1);

        state.navigate_up();
        assert_eq!(state.selected, 0);

        // Should not go below 0
        state.navigate_up();
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn test_navigate_down() {
        let mut state = ExplorerState::new("/".to_string());
        state.entries = vec![
            create_test_entry("file1.txt", false, 100),
            create_test_entry("file2.txt", false, 200),
            create_test_entry("file3.txt", false, 300),
        ];
        state.selected = 0;

        state.navigate_down();
        assert_eq!(state.selected, 1);

        state.navigate_down();
        assert_eq!(state.selected, 2);

        // Should not go beyond last entry
        state.navigate_down();
        assert_eq!(state.selected, 2);
    }

    #[test]
    fn test_navigate_down_empty_list() {
        let mut state = ExplorerState::new("/".to_string());
        state.entries = vec![];
        state.selected = 0;

        state.navigate_down();
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn test_page_down() {
        let mut state = ExplorerState::new("/".to_string());
        state.panel_height = 10;
        state.entries = (0..50)
            .map(|i| create_test_entry(&format!("file{}.txt", i), false, i * 100))
            .collect();
        state.selected = 0;

        state.page_down();
        // Should move down by panel_height - 2 = 8
        assert_eq!(state.selected, 8);

        state.page_down();
        assert_eq!(state.selected, 16);
    }

    #[test]
    fn test_page_up() {
        let mut state = ExplorerState::new("/".to_string());
        state.panel_height = 10;
        state.entries = (0..50)
            .map(|i| create_test_entry(&format!("file{}.txt", i), false, i * 100))
            .collect();
        state.selected = 20;
        state.scroll_offset = 15;

        state.page_up();
        // Should move up by panel_height - 2 = 8
        assert_eq!(state.selected, 12);
        assert_eq!(state.scroll_offset, 7);
    }

    #[test]
    fn test_page_up_at_start() {
        let mut state = ExplorerState::new("/".to_string());
        state.panel_height = 10;
        state.entries = (0..50)
            .map(|i| create_test_entry(&format!("file{}.txt", i), false, i * 100))
            .collect();
        state.selected = 5;
        state.scroll_offset = 0;

        state.page_up();
        assert_eq!(state.selected, 0);
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn test_get_selected_entry() {
        let mut state = ExplorerState::new("/".to_string());
        state.entries = vec![
            create_test_entry("file1.txt", false, 100),
            create_test_entry("file2.txt", false, 200),
        ];
        state.selected = 1;

        let entry = state.get_selected_entry().unwrap();
        assert_eq!(entry.name, "file2.txt");
    }

    #[test]
    fn test_get_selected_entry_out_of_bounds() {
        let mut state = ExplorerState::new("/".to_string());
        state.entries = vec![create_test_entry("file1.txt", false, 100)];
        state.selected = 5;

        let entry = state.get_selected_entry();
        assert!(entry.is_none());
    }

    #[test]
    fn test_apply_filter() {
        let mut state = ExplorerState::new("/".to_string());
        state.entries = vec![
            create_test_entry("readme.txt", false, 100),
            create_test_entry("config.yaml", false, 200),
            create_test_entry("data.txt", false, 300),
        ];
        state.filter = "txt".to_string();

        state.apply_filter();

        assert_eq!(state.entries.len(), 2);
        assert!(state.entries.iter().any(|e| e.name == "readme.txt"));
        assert!(state.entries.iter().any(|e| e.name == "data.txt"));
        assert!(!state.entries.iter().any(|e| e.name == "config.yaml"));
    }

    #[test]
    fn test_apply_filter_case_insensitive() {
        let mut state = ExplorerState::new("/".to_string());
        state.entries = vec![
            create_test_entry("README.TXT", false, 100),
            create_test_entry("Config.YAML", false, 200),
        ];
        state.filter = "readme".to_string();

        state.apply_filter();

        assert_eq!(state.entries.len(), 1);
        assert_eq!(state.entries[0].name, "README.TXT");
    }

    #[test]
    fn test_apply_filter_adjusts_selection() {
        let mut state = ExplorerState::new("/".to_string());
        state.entries = vec![
            create_test_entry("file1.txt", false, 100),
            create_test_entry("file2.txt", false, 200),
            create_test_entry("image.png", false, 300),
        ];
        state.selected = 2; // Pointing to image.png
        state.filter = "txt".to_string();

        state.apply_filter();

        // Should adjust selected to last valid index (1)
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn test_apply_filter_empty_filter() {
        let mut state = ExplorerState::new("/".to_string());
        let original_entries = vec![
            create_test_entry("file1.txt", false, 100),
            create_test_entry("file2.txt", false, 200),
        ];
        state.entries = original_entries.clone();
        state.filter = "".to_string();

        state.apply_filter();

        // Empty filter should not filter anything
        assert_eq!(state.entries.len(), 2);
    }

    #[test]
    fn test_sort_by_name() {
        let mut state = ExplorerState::new("/".to_string());
        state.entries = vec![
            create_test_entry("zebra.txt", false, 100),
            create_test_entry("apple.txt", false, 200),
            create_test_entry("banana", true, 0),
        ];
        state.sort_by = SortMode::Name;

        state.sort_entries();

        // Directories should come first, then alphabetical
        assert_eq!(state.entries[0].name, "banana");
        assert!(state.entries[0].is_dir);
        assert_eq!(state.entries[1].name, "apple.txt");
        assert_eq!(state.entries[2].name, "zebra.txt");
    }

    #[test]
    fn test_sort_by_size() {
        let mut state = ExplorerState::new("/".to_string());
        state.entries = vec![
            create_test_entry("small.txt", false, 100),
            create_test_entry("large.txt", false, 1000),
            create_test_entry("medium.txt", false, 500),
        ];
        state.sort_by = SortMode::Size;

        state.sort_entries();

        // Should be sorted by size descending
        assert_eq!(state.entries[0].name, "large.txt");
        assert_eq!(state.entries[1].name, "medium.txt");
        assert_eq!(state.entries[2].name, "small.txt");
    }

    #[test]
    fn test_sort_by_type() {
        let mut state = ExplorerState::new("/".to_string());
        state.entries = vec![
            create_test_entry("file.txt", false, 100),
            create_test_entry("image.png", false, 200),
            create_test_entry("data.txt", false, 300),
            create_test_entry("config.yaml", false, 400),
        ];
        state.sort_by = SortMode::Type;

        state.sort_entries();

        // Should be sorted by extension, then name
        // png, txt (2 files), yaml
        let extensions: Vec<&str> = state
            .entries
            .iter()
            .map(|e| get_extension(&e.name))
            .collect();

        // txt files should be together
        assert!(extensions
            .windows(2)
            .any(|w| w[0] == "txt" && w[1] == "txt"));
    }

    #[test]
    fn test_get_extension() {
        assert_eq!(get_extension("file.txt"), "txt");
        assert_eq!(get_extension("archive.tar.gz"), "gz");
        assert_eq!(get_extension("no_extension"), "no_extension");
        assert_eq!(get_extension(".hidden"), "hidden");
        assert_eq!(get_extension("file."), "");
    }

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1023), "1023 B");
    }

    #[test]
    fn test_format_size_kilobytes() {
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1536), "1.50 KB");
        assert_eq!(format_size(2048), "2.00 KB");
    }

    #[test]
    fn test_format_size_megabytes() {
        assert_eq!(format_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_size(1024 * 1024 + 512 * 1024), "1.50 MB");
    }

    #[test]
    fn test_format_size_gigabytes() {
        assert_eq!(format_size(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(format_size(2 * 1024 * 1024 * 1024), "2.00 GB");
    }

    #[test]
    fn test_format_size_terabytes() {
        let tb = 1024i64 * 1024 * 1024 * 1024;
        assert_eq!(format_size(tb), "1.00 TB");
        assert_eq!(format_size(2 * tb), "2.00 TB");
    }

    #[test]
    fn test_get_file_icon_directory() {
        let entry = create_test_entry("folder", true, 0);
        assert_eq!(get_file_icon(&entry), "📁");
    }

    #[test]
    fn test_get_file_icon_text_files() {
        assert_eq!(
            get_file_icon(&create_test_entry("readme.txt", false, 100)),
            "📄"
        );
        assert_eq!(
            get_file_icon(&create_test_entry("notes.md", false, 100)),
            "📄"
        );
        assert_eq!(
            get_file_icon(&create_test_entry("debug.log", false, 100)),
            "📄"
        );
    }

    #[test]
    fn test_get_file_icon_code_files() {
        assert_eq!(
            get_file_icon(&create_test_entry("main.rs", false, 100)),
            "💻"
        );
        assert_eq!(
            get_file_icon(&create_test_entry("script.py", false, 100)),
            "💻"
        );
        assert_eq!(
            get_file_icon(&create_test_entry("app.js", false, 100)),
            "💻"
        );
    }

    #[test]
    fn test_get_file_icon_config_files() {
        assert_eq!(
            get_file_icon(&create_test_entry("config.json", false, 100)),
            "⚙️ "
        );
        assert_eq!(
            get_file_icon(&create_test_entry("settings.yaml", false, 100)),
            "⚙️ "
        );
        assert_eq!(
            get_file_icon(&create_test_entry("data.xml", false, 100)),
            "⚙️ "
        );
    }

    #[test]
    fn test_get_file_icon_images() {
        assert_eq!(
            get_file_icon(&create_test_entry("photo.jpg", false, 100)),
            "🖼️ "
        );
        assert_eq!(
            get_file_icon(&create_test_entry("icon.png", false, 100)),
            "🖼️ "
        );
        assert_eq!(
            get_file_icon(&create_test_entry("logo.svg", false, 100)),
            "🖼️ "
        );
    }

    #[test]
    fn test_get_file_icon_pdf() {
        assert_eq!(
            get_file_icon(&create_test_entry("document.pdf", false, 100)),
            "📕"
        );
    }

    #[test]
    fn test_get_file_icon_archives() {
        assert_eq!(
            get_file_icon(&create_test_entry("archive.zip", false, 100)),
            "📦"
        );
        assert_eq!(
            get_file_icon(&create_test_entry("backup.tar", false, 100)),
            "📦"
        );
        assert_eq!(
            get_file_icon(&create_test_entry("data.gz", false, 100)),
            "📦"
        );
    }

    #[test]
    fn test_get_file_icon_scripts() {
        assert_eq!(
            get_file_icon(&create_test_entry("build.sh", false, 100)),
            "🔧"
        );
        assert_eq!(
            get_file_icon(&create_test_entry("deploy.bash", false, 100)),
            "🔧"
        );
    }

    #[test]
    fn test_get_file_icon_config() {
        assert_eq!(
            get_file_icon(&create_test_entry("app.conf", false, 100)),
            "🔐"
        );
        assert_eq!(
            get_file_icon(&create_test_entry("server.config", false, 100)),
            "🔐"
        );
    }

    #[test]
    fn test_get_file_icon_default() {
        assert_eq!(
            get_file_icon(&create_test_entry("unknown.xyz", false, 100)),
            "📝"
        );
    }

    #[test]
    fn test_get_file_color_directory() {
        let entry = create_test_entry("folder", true, 0);
        assert_eq!(get_file_color(&entry), colored::Color::Blue);
    }

    #[test]
    fn test_get_file_color_hidden() {
        let entry = create_test_entry(".hidden", false, 100);
        assert_eq!(get_file_color(&entry), colored::Color::BrightBlack);
    }

    #[test]
    fn test_get_file_color_executables() {
        assert_eq!(
            get_file_color(&create_test_entry("script.sh", false, 100)),
            colored::Color::Green
        );
        assert_eq!(
            get_file_color(&create_test_entry("script.py", false, 100)),
            colored::Color::Green
        );
    }

    #[test]
    fn test_get_file_color_source_code() {
        assert_eq!(
            get_file_color(&create_test_entry("main.rs", false, 100)),
            colored::Color::Yellow
        );
        assert_eq!(
            get_file_color(&create_test_entry("app.c", false, 100)),
            colored::Color::Yellow
        );
    }

    #[test]
    fn test_get_file_color_text() {
        assert_eq!(
            get_file_color(&create_test_entry("readme.txt", false, 100)),
            colored::Color::White
        );
        assert_eq!(
            get_file_color(&create_test_entry("notes.md", false, 100)),
            colored::Color::White
        );
    }

    #[test]
    fn test_get_file_color_config() {
        assert_eq!(
            get_file_color(&create_test_entry("config.yaml", false, 100)),
            colored::Color::Cyan
        );
        assert_eq!(
            get_file_color(&create_test_entry("data.json", false, 100)),
            colored::Color::Cyan
        );
    }

    #[test]
    fn test_get_file_color_archives() {
        assert_eq!(
            get_file_color(&create_test_entry("archive.tar", false, 100)),
            colored::Color::Red
        );
        assert_eq!(
            get_file_color(&create_test_entry("data.zip", false, 100)),
            colored::Color::Red
        );
    }

    #[test]
    fn test_file_entry_clone() {
        let entry = create_test_entry("test.txt", false, 100);
        let cloned = entry.clone();

        assert_eq!(entry.name, cloned.name);
        assert_eq!(entry.is_dir, cloned.is_dir);
        assert_eq!(entry.size, cloned.size);
    }

    #[test]
    fn test_sort_mode_equality() {
        assert_eq!(SortMode::Name, SortMode::Name);
        assert_eq!(SortMode::Size, SortMode::Size);
        assert_eq!(SortMode::Type, SortMode::Type);
        assert_ne!(SortMode::Name, SortMode::Size);
    }
}
