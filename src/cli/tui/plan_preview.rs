// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Read-only fix plan preview overlay (migration assurance).

use crate::cli::plan::types::{Operation, Priority};
use crate::cli::tui::app::App;
use crate::cli::tui::theme::{
    self, focus_style, modal_block, ResolvedTheme, ACCENT, ERROR, INFO, SUCCESS, TEXT, WARNING,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    let Some(ref plan) = app.plan_preview else {
        return;
    };

    let area = centered_rect(75, 80, f.area());
    f.render_widget(ratatui::widgets::Clear, area);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(4), Constraint::Length(2)])
        .margin(1)
        .split(area);

    let th = app.theme();
    let visible = inner[0].height.saturating_sub(2) as usize;
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Profile ", theme::label_style()),
            Span::styled(&plan.profile, Style::default().fg(ACCENT)),
            Span::styled("  risk ", theme::label_style()),
            Span::styled(&plan.overall_risk, priority_style_str(&plan.overall_risk)),
            Span::styled(
                format!(
                    "  · {} ops (preview only — not applied)",
                    plan.operations.len()
                ),
                theme::label_style(),
            ),
        ]),
        Line::from(""),
    ];

    for (idx, op) in plan
        .operations
        .iter()
        .enumerate()
        .skip(app.plan_preview_scroll)
    {
        if lines.len() >= visible + 2 {
            break;
        }
        let selected = idx == app.plan_preview_scroll;
        lines.push(operation_line(op, selected, th));
    }

    if plan.operations.is_empty() {
        lines.push(Line::from(Span::styled(
            "No operations in this plan.",
            theme::label_style(),
        )));
    }

    let title = format!(
        " Fix plan preview ({}/{}) ",
        app.plan_preview_scroll + 1,
        plan.operations.len().max(1)
    );
    f.render_widget(
        Paragraph::new(lines)
            .block(modal_block(&title, th))
            .wrap(ratatui::widgets::Wrap { trim: true }),
        inner[0],
    );

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Esc close · j/k scroll · e export YAML (Assurance view)",
            theme::label_style(),
        ))),
        inner[1],
    );
}

fn operation_line(op: &Operation, selected: bool, th: &ResolvedTheme) -> Line<'static> {
    let desc_style = if selected {
        focus_style(th)
    } else {
        Style::default().fg(TEXT)
    };
    Line::from(vec![
        Span::styled("[ ] ", Style::default().fg(INFO)),
        Span::styled(
            format!("{:<9} ", op.priority.as_str()),
            priority_style_priority(&op.priority),
        ),
        Span::styled(
            format!("{}  ", op.id),
            Style::default().fg(TEXT).add_modifier(Modifier::DIM),
        ),
        Span::styled(op.description.clone(), desc_style),
    ])
}

fn priority_style_priority(p: &Priority) -> Style {
    let color = match p {
        Priority::Critical => ERROR,
        Priority::High => ACCENT,
        Priority::Medium => WARNING,
        Priority::Low => SUCCESS,
        Priority::Info => INFO,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn priority_style_str(risk: &str) -> Style {
    match risk.to_lowercase().as_str() {
        "critical" => Style::default().fg(ERROR).add_modifier(Modifier::BOLD),
        "high" => Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        "medium" => Style::default().fg(WARNING),
        "low" => Style::default().fg(SUCCESS),
        _ => Style::default().fg(INFO),
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
