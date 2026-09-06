// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Shared TUI widgets — chips, list rows, empty states, progress, severity.

use super::app::App;
use super::theme::{self, ACCENT, BORDER_MUTED, ERROR, SUCCESS, TEXT_MUTED, WARNING};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Muted stat chip: `│ Pkgs 1247 │`
pub fn stat_chip<'a>(
    label: &'a str,
    value: &str,
    value_color: Color,
    t: &theme::ResolvedTheme,
) -> Span<'a> {
    Span::styled(
        format!("│ {label} {value} │"),
        Style::default()
            .fg(value_color)
            .bg(t.chip_bg)
            .add_modifier(Modifier::BOLD),
    )
}

pub fn risk_dots(critical: usize, high: usize, medium: usize, emoji: bool) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    if critical > 0 {
        let s = if emoji { " C " } else { "!!" };
        spans.push(Span::styled(s.to_string(), Style::default().fg(ERROR)));
    }
    if high > 0 {
        let s = if emoji { " H " } else { " !" };
        spans.push(Span::styled(s.to_string(), Style::default().fg(ACCENT)));
    }
    if medium > 0 {
        let s = if emoji { " M " } else { " ~" };
        spans.push(Span::styled(s.to_string(), Style::default().fg(WARNING)));
    }
    if spans.is_empty() {
        spans.push(Span::styled(
            if emoji { " ok " } else { " OK" },
            Style::default().fg(SUCCESS),
        ));
    }
    spans
}

/// Left severity bar prefix for list rows.
pub fn severity_prefix(level: Option<char>, selected: bool) -> (String, Color) {
    match level {
        Some('C') | Some('c') => ("▌".to_string(), ERROR),
        Some('H') | Some('h') => ("▌".to_string(), ACCENT),
        Some('M') | Some('m') => ("▌".to_string(), WARNING),
        Some('L') | Some('l') => ("▌".to_string(), TEXT_MUTED),
        _ if selected => ("▌".to_string(), ACCENT),
        _ => (" ".to_string(), TEXT_MUTED),
    }
}

pub fn list_line_spans(
    selected: bool,
    severity: Option<char>,
    parts: Vec<Span<'_>>,
    t: &theme::ResolvedTheme,
) -> Line<'static> {
    let (bar, bar_color) = severity_prefix(severity, selected);
    let mut spans = vec![Span::styled(bar, Style::default().fg(bar_color))];
    let row_style = if selected {
        Style::default()
            .bg(t.selection)
            .fg(ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    for part in parts {
        spans.push(Span::styled(
            part.content.to_string(),
            part.style.patch(row_style),
        ));
    }
    Line::from(spans)
}

pub fn empty_state<'a>(title: &'a str, hint: &'a str, t: &theme::ResolvedTheme) -> Paragraph<'a> {
    let art = [
        "    ┌──────────────┐",
        "    │   no data    │",
        "    └──────────────┘",
    ];
    let mut lines: Vec<Line> = art
        .iter()
        .map(|l| Line::from(Span::styled(*l, theme::label_style())))
        .collect();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(title, theme::value_style())));
    lines.push(Line::from(Span::styled(hint, theme::label_style())));
    Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_MUTED))
            .style(Style::default().bg(t.surface)),
    )
}

/// ASCII progress bar `████░░░░ 4/7 label`
pub fn progress_bar(current: u8, total: u8, label: &str, width: u16) -> String {
    let total = total.max(1);
    let w = width.max(8) as usize;
    let filled = (current as usize * w) / total as usize;
    let bar: String = (0..w).map(|i| if i < filled { '█' } else { '░' }).collect();
    format!("{} {}/{} {}", bar, current, total, label)
}

/// Horizontal donut-style risk meter `[####··] 62% healthy`
pub fn risk_donut_ascii(
    critical: usize,
    high: usize,
    medium: usize,
    width: usize,
) -> (String, Color) {
    let total = critical + high + medium;
    if total == 0 {
        return (format!("[{:width$}]", "ok", width = width), SUCCESS);
    }
    let bad = critical + high;
    let pct_bad = (bad * 100) / total.max(1);
    let filled = (pct_bad * width) / 100;
    let mut s = String::from("[");
    for i in 0..width {
        s.push(if i < filled { '#' } else { '·' });
    }
    s.push(']');
    let color = if critical > 0 {
        ERROR
    } else if high > 0 {
        ACCENT
    } else {
        WARNING
    };
    (s, color)
}

pub fn truncate_path(path: &str, max: usize) -> String {
    if path.len() <= max {
        path.to_string()
    } else {
        format!("…{}", &path[path.len().saturating_sub(max - 1)..])
    }
}

pub fn render_dim_layer(f: &mut Frame, area: Rect, t: &theme::ResolvedTheme) {
    let block = Block::default().style(Style::default().bg(t.dim_overlay));
    f.render_widget(block, area);
}

pub fn health_score_color(score: u8) -> Color {
    match score {
        90..=100 => SUCCESS,
        75..=89 => WARNING,
        60..=74 => ACCENT,
        _ => ERROR,
    }
}

pub fn breadcrumb_line(app: &App) -> Option<String> {
    use super::app::{IssueRiskFilter, View};
    let mut parts = vec![app.current_view.title().to_string()];
    if app.current_view == View::Issues {
        let f = match app.issue_filter {
            IssueRiskFilter::All => "all",
            IssueRiskFilter::Critical => "critical",
            IssueRiskFilter::High => "high",
            IssueRiskFilter::Medium => "medium",
        };
        parts.push(f.to_string());
    }
    if app.is_searching() && !app.search_query.is_empty() {
        parts.push(format!("search:{}", app.search_query));
    }
    if app.comparison_mode {
        parts.push("compare".to_string());
    }
    if parts.len() > 1 {
        Some(parts.join(" › "))
    } else {
        None
    }
}
