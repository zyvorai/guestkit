// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Files view - Interactive file browser

use crate::cli::tui::app::App;
use crate::cli::tui::ui::{BORDER_COLOR, LIGHT_ORANGE, ORANGE, TEXT_COLOR, WARNING_COLOR};
use anyhow::Result;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

/// File entry for display
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: i64,
}

/// File browser state for TUI
pub struct FileBrowserState {
    pub current_path: String,
    pub entries: Vec<FileEntry>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub show_hidden: bool,
    pub filter: String,
    pub all_entries: Vec<FileEntry>, // Unfiltered entries cache
}

impl Default for FileBrowserState {
    fn default() -> Self {
        Self {
            current_path: "/".to_string(),
            entries: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            show_hidden: false,
            filter: String::new(),
            all_entries: Vec::new(),
        }
    }
}

impl FileBrowserState {
    /// Load directory entries from guestfs
    pub fn load_directory(&mut self, guestfs: &mut crate::Guestfs) -> Result<()> {
        let mut entries = Vec::new();

        // Add parent directory entry if not at root
        if self.current_path != "/" {
            entries.push(FileEntry {
                name: "..".to_string(),
                is_dir: true,
                size: 0,
            });
        }

        // List directory contents
        let files = match guestfs.ls(&self.current_path) {
            Ok(files) => files,
            Err(e) => {
                log::warn!("Failed to list {}: {}", self.current_path, e);
                return Ok(());
            }
        };

        for file in files {
            // Filter hidden files if needed
            if !self.show_hidden && file.starts_with('.') {
                continue;
            }

            let full_path = if self.current_path == "/" {
                format!("/{}", file)
            } else {
                format!("{}/{}", self.current_path, file)
            };

            let is_dir = guestfs.is_dir(&full_path).unwrap_or(false);
            let size = if !is_dir {
                guestfs.filesize(&full_path).unwrap_or(0)
            } else {
                0
            };

            entries.push(FileEntry {
                name: file,
                is_dir,
                size,
            });
        }

        // Sort: directories first, then by name
        entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        });

        // Store all entries
        self.all_entries = entries;

        // Apply filter if active
        self.apply_filter();

        Ok(())
    }

    /// Apply current filter to entries
    pub fn apply_filter(&mut self) {
        if self.filter.is_empty() {
            // No filter - show all entries
            self.entries = self.all_entries.clone();
        } else {
            // Apply filter - case insensitive substring match
            let filter_lower = self.filter.to_lowercase();
            self.entries = self
                .all_entries
                .iter()
                .filter(|entry| {
                    // Always show ".." entry
                    if entry.name == ".." {
                        return true;
                    }
                    entry.name.to_lowercase().contains(&filter_lower)
                })
                .cloned()
                .collect();
        }

        // Reset selection if out of bounds
        if self.selected >= self.entries.len() && !self.entries.is_empty() {
            self.selected = self.entries.len() - 1;
        }
        if self.selected > 0 && self.entries.is_empty() {
            self.selected = 0;
        }
        self.scroll_offset = 0;
    }

    /// Set filter and apply it
    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
        self.apply_filter();
    }

    /// Clear filter
    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.apply_filter();
    }

    /// Navigate up one directory
    pub fn go_up(&mut self) {
        if self.current_path != "/" {
            let parent = std::path::Path::new(&self.current_path)
                .parent()
                .unwrap_or(std::path::Path::new("/"))
                .to_string_lossy()
                .to_string();
            self.current_path = if parent.is_empty() {
                "/".to_string()
            } else {
                parent
            };
        }
    }

    /// Navigate into selected directory
    pub fn enter_directory(&mut self) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }

        let entry = &self.entries[self.selected];
        if !entry.is_dir {
            return None;
        }

        if entry.name == ".." {
            self.go_up();
        } else {
            let new_path = if self.current_path == "/" {
                format!("/{}", entry.name)
            } else {
                format!("{}/{}", self.current_path, entry.name)
            };
            self.current_path = new_path.clone();
            return Some(new_path);
        }

        Some(self.current_path.clone())
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            if self.selected < self.scroll_offset {
                self.scroll_offset = self.selected;
            }
        }
    }

    /// Move selection down
    pub fn move_down(&mut self, visible_items: usize) {
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
            if self.selected >= self.scroll_offset + visible_items {
                self.scroll_offset = self.selected - visible_items + 1;
            }
        }
    }

    /// Toggle hidden files visibility
    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
    }
}

/// Draw the files browser view
pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header with path
            Constraint::Min(0),    // File list
            Constraint::Length(3), // Footer with help
        ])
        .split(area);

    draw_header(f, chunks[0], app);
    draw_file_list(f, chunks[1], app);
    draw_footer(f, chunks[2], app);
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let (current_path, item_count, filter) = if let Some(ref browser) = app.file_browser {
        (
            browser.current_path.clone(),
            browser.entries.len(),
            browser.filter.clone(),
        )
    } else {
        ("/".to_string(), 0, String::new())
    };

    let mut spans = vec![
        Span::styled("📍 ", Style::default().fg(ORANGE)),
        Span::styled(
            "Path: ",
            Style::default()
                .fg(LIGHT_ORANGE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(&current_path, Style::default().fg(TEXT_COLOR)),
        Span::raw("  "),
        Span::styled("📊 ", Style::default().fg(ORANGE)),
        Span::styled(
            format!("Items: {}", item_count),
            Style::default().fg(TEXT_COLOR),
        ),
    ];

    // Show filter if active
    if !filter.is_empty() {
        spans.push(Span::raw("  "));
        spans.push(Span::styled("🔍 ", Style::default().fg(ORANGE)));
        spans.push(Span::styled(
            "Filter: ",
            Style::default()
                .fg(LIGHT_ORANGE)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            &filter,
            Style::default()
                .fg(WARNING_COLOR)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let header = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR))
            .title(" 📂 File Browser ")
            .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
    );

    f.render_widget(header, area);
}

fn draw_file_list(f: &mut Frame, area: Rect, app: &App) {
    let browser = match &app.file_browser {
        Some(b) => b,
        None => {
            let empty = Paragraph::new("File browser not initialized")
                .style(Style::default().fg(TEXT_COLOR));
            f.render_widget(empty, area);
            return;
        }
    };

    if browser.entries.is_empty() {
        let empty = Paragraph::new("Empty directory").style(Style::default().fg(TEXT_COLOR));
        f.render_widget(empty, area);
        return;
    }

    let visible_height = area.height.saturating_sub(2) as usize; // Account for borders
    let scroll_offset = browser.scroll_offset;
    let visible_entries = browser
        .entries
        .iter()
        .skip(scroll_offset)
        .take(visible_height);

    let items: Vec<ListItem> = visible_entries
        .enumerate()
        .map(|(idx, entry)| {
            let absolute_idx = scroll_offset + idx;
            let is_selected = absolute_idx == browser.selected;

            let (icon, color) = get_file_icon_and_color(entry);
            let size_str = if entry.is_dir {
                "<DIR>".to_string()
            } else {
                format_size(entry.size)
            };

            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(ORANGE)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(color)
            };

            let line = Line::from(vec![
                Span::raw(if is_selected { "▸ " } else { "  " }),
                Span::styled(icon, style),
                Span::raw(" "),
                Span::styled(format!("{:<40}", entry.name), style),
                Span::styled(
                    format!("{:>12}", size_str),
                    if is_selected {
                        Style::default().fg(Color::Black).bg(ORANGE)
                    } else {
                        Style::default().fg(LIGHT_ORANGE)
                    },
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR)),
    );

    f.render_widget(list, area);
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let is_filtering = app
        .file_browser
        .as_ref()
        .map(|b| !b.filter.is_empty())
        .unwrap_or(false);

    let help = if app.file_filtering {
        // Show filter input mode
        Paragraph::new(Line::from(vec![
            Span::styled(
                "🔍 Filter: ",
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &app.file_filter_input,
                Style::default()
                    .fg(TEXT_COLOR)
                    .add_modifier(Modifier::UNDERLINED),
            ),
            Span::styled("_", Style::default().fg(ORANGE)),
            Span::raw("  "),
            Span::styled(
                "ESC",
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Cancel  "),
            Span::styled(
                "Enter",
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Apply"),
        ]))
    } else if is_filtering {
        Paragraph::new(Line::from(vec![
            Span::styled("🔍 ", Style::default().fg(ORANGE)),
            Span::styled(
                "Filter active",
                Style::default()
                    .fg(WARNING_COLOR)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                "ESC",
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Clear filter"),
        ]))
    } else {
        Paragraph::new(Line::from(vec![
            Span::styled(
                "↑↓",
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Navigate  "),
            Span::styled(
                "Enter",
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Open  "),
            Span::styled(
                "v",
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" View  "),
            Span::styled(
                "i",
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Info  "),
            Span::styled(
                "/",
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Filter  "),
            Span::styled(
                ".",
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Hidden"),
        ]))
    };

    let widget = help.block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR)),
    );

    f.render_widget(widget, area);
}

/// Get icon and color for file type
fn get_file_icon_and_color(entry: &FileEntry) -> (&'static str, Color) {
    if entry.is_dir {
        return ("📁", Color::Blue);
    }

    let name_lower = entry.name.to_lowercase();

    // Check by extension
    if let Some(ext) = name_lower.rsplit('.').next() {
        match ext {
            // Source code
            "rs" | "py" | "js" | "ts" | "java" | "c" | "cpp" | "go" | "rb" | "php" => {
                ("💻", Color::Yellow)
            }
            // Config files
            "json" | "yaml" | "yml" | "toml" | "xml" | "conf" | "cfg" | "ini" => {
                ("⚙️", Color::Cyan)
            }
            // Scripts
            "sh" | "bash" | "zsh" | "fish" => ("🔧", Color::Green),
            // Archives
            "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" => ("📦", Color::Red),
            // Images
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "svg" | "ico" => ("🖼️", Color::Magenta),
            // Documents
            "pdf" => ("📕", Color::Red),
            "txt" | "md" | "log" => ("📄", Color::White),
            // Default
            _ => ("📝", Color::White),
        }
    } else {
        ("📝", Color::White)
    }
}

/// Format file size to human-readable format
fn format_size(size: i64) -> String {
    crate::cli::output::format_size(size as u64)
}

/// Get the full path of the currently selected file
pub fn get_selected_file_path(browser: &FileBrowserState) -> Option<String> {
    if browser.entries.is_empty() {
        return None;
    }

    let entry = &browser.entries[browser.selected];

    // Skip ".." entry
    if entry.name == ".." {
        return None;
    }

    let path = if browser.current_path == "/" {
        format!("/{}", entry.name)
    } else {
        format!("{}/{}", browser.current_path, entry.name)
    };

    Some(path)
}
