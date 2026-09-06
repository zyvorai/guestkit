// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Interactive logs viewer with filtering

use crate::cli::tui::app::App;
use crate::cli::tui::ui::{
    BORDER_COLOR, ERROR_COLOR, INFO_COLOR, LIGHT_ORANGE, ORANGE, SUCCESS_COLOR, TEXT_COLOR,
    WARNING_COLOR,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
    Frame,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogCategory {
    Auth,
    System,
    Kernel,
    Security,
    Application,
}

impl LogCategory {
    fn all() -> Vec<LogCategory> {
        vec![
            LogCategory::Auth,
            LogCategory::System,
            LogCategory::Kernel,
            LogCategory::Security,
            LogCategory::Application,
        ]
    }

    fn title(&self) -> &'static str {
        match self {
            LogCategory::Auth => "🔐 Auth",
            LogCategory::System => "🖥️  System",
            LogCategory::Kernel => "🧩 Kernel",
            LogCategory::Security => "🔒 Security",
            LogCategory::Application => "📦 Apps",
        }
    }

    fn color(&self) -> ratatui::style::Color {
        match self {
            LogCategory::Auth => INFO_COLOR,
            LogCategory::System => TEXT_COLOR,
            LogCategory::Kernel => LIGHT_ORANGE,
            LogCategory::Security => WARNING_COLOR,
            LogCategory::Application => SUCCESS_COLOR,
        }
    }
}

#[derive(Debug, Clone)]
struct LogEntry {
    timestamp: String,
    level: LogLevel,
    category: LogCategory,
    message: String,
    source: String,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum LogLevel {
    Error,
    Warning,
    Info,
    Debug,
}

impl LogLevel {
    fn color(&self) -> ratatui::style::Color {
        match self {
            LogLevel::Error => ERROR_COLOR,
            LogLevel::Warning => WARNING_COLOR,
            LogLevel::Info => INFO_COLOR,
            LogLevel::Debug => TEXT_COLOR,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            LogLevel::Error => "ERR",
            LogLevel::Warning => "WRN",
            LogLevel::Info => "INF",
            LogLevel::Debug => "DBG",
        }
    }
}

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Category tabs
            Constraint::Length(3), // Filter bar
            Constraint::Min(0),    // Log entries
            Constraint::Length(3), // Summary footer
        ])
        .split(area);

    draw_category_tabs(f, chunks[0], app);
    draw_filter_bar(f, chunks[1], app);
    draw_log_entries(f, chunks[2], app);
    draw_log_summary(f, chunks[3], app);
}

fn draw_category_tabs(f: &mut Frame, area: Rect, app: &App) {
    let categories = LogCategory::all();
    let titles: Vec<_> = categories.iter().map(|c| c.title()).collect();

    let selected_tab = app.selected_index % categories.len();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR)),
        )
        .select(selected_tab)
        .style(Style::default().fg(TEXT_COLOR))
        .highlight_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD));

    f.render_widget(tabs, area);
}

fn draw_filter_bar(f: &mut Frame, area: Rect, app: &App) {
    let filter_text = if app.searching {
        vec![Line::from(vec![
            Span::styled(
                "🔍 Filter: ",
                Style::default()
                    .fg(LIGHT_ORANGE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(&app.search_query, Style::default().fg(TEXT_COLOR)),
            Span::styled(
                "_",
                Style::default()
                    .fg(ORANGE)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
        ])]
    } else {
        vec![Line::from(vec![
            Span::styled("Press ", Style::default().fg(TEXT_COLOR)),
            Span::styled(
                "/",
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to filter logs │ ", Style::default().fg(TEXT_COLOR)),
            Span::styled(
                "↑↓",
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" scroll │ ", Style::default().fg(TEXT_COLOR)),
            Span::styled(
                "Tab",
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" switch category", Style::default().fg(TEXT_COLOR)),
        ])]
    };

    let filter = Paragraph::new(filter_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR)),
    );

    f.render_widget(filter, area);
}

fn draw_log_entries(f: &mut Frame, area: Rect, app: &App) {
    let logs = generate_log_entries(app);

    // Filter logs based on search query
    let filtered_logs: Vec<_> = if app.searching && !app.search_query.is_empty() {
        logs.iter()
            .filter(|log| {
                if app.search_case_sensitive {
                    log.message.contains(&app.search_query)
                        || log.source.contains(&app.search_query)
                } else {
                    log.message
                        .to_lowercase()
                        .contains(&app.search_query.to_lowercase())
                        || log
                            .source
                            .to_lowercase()
                            .contains(&app.search_query.to_lowercase())
                }
            })
            .collect()
    } else {
        logs.iter().collect()
    };

    // Apply scrolling
    let visible_logs: Vec<_> = filtered_logs
        .iter()
        .skip(app.scroll_offset)
        .take((area.height as usize).saturating_sub(2))
        .collect();

    let items: Vec<ListItem> = visible_logs
        .iter()
        .enumerate()
        .map(|(i, log)| {
            let is_selected = i + app.scroll_offset == app.selected_index;

            let level_badge = Span::styled(
                format!("[{}]", log.level.label()),
                Style::default()
                    .fg(log.level.color())
                    .add_modifier(Modifier::BOLD),
            );

            let mut content = vec![
                Span::styled(
                    format!("{} ", log.timestamp),
                    Style::default().fg(TEXT_COLOR),
                ),
                level_badge,
                Span::raw(" "),
                Span::styled(&log.source, Style::default().fg(log.category.color())),
                Span::raw(": "),
                Span::styled(&log.message, Style::default().fg(TEXT_COLOR)),
            ];

            if is_selected {
                content.insert(0, Span::styled("▶ ", Style::default().fg(ORANGE)));
            } else {
                content.insert(0, Span::raw("  "));
            }

            ListItem::new(Line::from(content))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(Span::styled(
                format!(
                    "📋 Log Entries ({} / {} total)",
                    filtered_logs.len(),
                    logs.len()
                ),
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR)),
    );

    f.render_widget(list, area);
}

fn draw_log_summary(f: &mut Frame, area: Rect, app: &App) {
    let logs = generate_log_entries(app);

    let error_count = logs
        .iter()
        .filter(|l| matches!(l.level, LogLevel::Error))
        .count();
    let warning_count = logs
        .iter()
        .filter(|l| matches!(l.level, LogLevel::Warning))
        .count();
    let info_count = logs
        .iter()
        .filter(|l| matches!(l.level, LogLevel::Info))
        .count();

    let summary_text = vec![Line::from(vec![
        Span::styled("Summary: ", Style::default().fg(TEXT_COLOR)),
        Span::styled(
            format!("🔴 {} errors", error_count),
            Style::default()
                .fg(ERROR_COLOR)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" │ "),
        Span::styled(
            format!("🟠 {} warnings", warning_count),
            Style::default()
                .fg(WARNING_COLOR)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" │ "),
        Span::styled(
            format!("🔵 {} info", info_count),
            Style::default().fg(INFO_COLOR).add_modifier(Modifier::BOLD),
        ),
    ])];

    let summary = Paragraph::new(summary_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR)),
    );

    f.render_widget(summary, area);
}

fn generate_log_entries(app: &App) -> Vec<LogEntry> {
    let mut logs = Vec::new();

    // System logs
    logs.push(LogEntry {
        timestamp: "2024-01-27 10:00:00".to_string(),
        level: LogLevel::Info,
        category: LogCategory::System,
        message: format!("System boot completed - {} {}", app.os_name, app.os_version),
        source: "systemd".to_string(),
    });

    // Authentication logs
    for user in &app.users {
        logs.push(LogEntry {
            timestamp: "2024-01-27 10:01:00".to_string(),
            level: LogLevel::Info,
            category: LogCategory::Auth,
            message: format!(
                "User account created: {} (UID: {})",
                user.username, user.uid
            ),
            source: "useradd".to_string(),
        });

        if user.uid == "0" && user.username != "root" {
            logs.push(LogEntry {
                timestamp: "2024-01-27 10:01:05".to_string(),
                level: LogLevel::Error,
                category: LogCategory::Security,
                message: format!("CRITICAL: Non-root user {} has UID 0", user.username),
                source: "security-audit".to_string(),
            });
        }
    }

    // Service logs
    for service in &app.services {
        if service.enabled {
            logs.push(LogEntry {
                timestamp: "2024-01-27 10:02:00".to_string(),
                level: LogLevel::Info,
                category: LogCategory::System,
                message: format!(
                    "Service {} enabled (state: {})",
                    service.name, service.state
                ),
                source: "systemd".to_string(),
            });
        }
    }

    // Security logs
    if !app.firewall.enabled {
        logs.push(LogEntry {
            timestamp: "2024-01-27 10:03:00".to_string(),
            level: LogLevel::Warning,
            category: LogCategory::Security,
            message: "Firewall is disabled - system may be exposed".to_string(),
            source: "security-check".to_string(),
        });
    }

    if app.security.selinux == "disabled" && !app.security.apparmor {
        logs.push(LogEntry {
            timestamp: "2024-01-27 10:03:05".to_string(),
            level: LogLevel::Warning,
            category: LogCategory::Security,
            message: "No MAC system (SELinux/AppArmor) enabled".to_string(),
            source: "security-check".to_string(),
        });
    }

    // Package logs
    if app.packages.package_count > 2000 {
        logs.push(LogEntry {
            timestamp: "2024-01-27 10:04:00".to_string(),
            level: LogLevel::Warning,
            category: LogCategory::Application,
            message: format!(
                "High package count: {} packages installed",
                app.packages.package_count
            ),
            source: "package-manager".to_string(),
        });
    }

    // Kernel logs
    logs.push(LogEntry {
        timestamp: "2024-01-27 10:05:00".to_string(),
        level: LogLevel::Info,
        category: LogCategory::Kernel,
        message: format!(
            "Kernel {} loaded with {} modules",
            app.kernel_version,
            app.kernel_modules.len()
        ),
        source: "kernel".to_string(),
    });

    // Database logs
    for db in &app.databases {
        logs.push(LogEntry {
            timestamp: "2024-01-27 10:06:00".to_string(),
            level: LogLevel::Info,
            category: LogCategory::Application,
            message: format!("Database detected: {}", db.name),
            source: "db-scanner".to_string(),
        });
    }

    // Web server logs
    for ws in &app.web_servers {
        logs.push(LogEntry {
            timestamp: "2024-01-27 10:07:00".to_string(),
            level: LogLevel::Info,
            category: LogCategory::Application,
            message: format!("Web server detected: {} {}", ws.name, ws.version),
            source: "web-scanner".to_string(),
        });
    }

    // Sort logs by timestamp (newest first)
    logs.reverse();

    logs
}
