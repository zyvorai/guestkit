// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Profiles view - Security, Migration, Performance, Compliance, and Hardening profile results

use crate::cli::profiles::{FindingStatus, ProfileReport, RiskLevel};
use crate::cli::tui::app::App;
use crate::cli::tui::ui::{
    BORDER_COLOR, ERROR_COLOR, LIGHT_ORANGE, ORANGE, SUCCESS_COLOR, TEXT_COLOR, WARNING_COLOR,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
    Frame,
};

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Tab bar
            Constraint::Min(0),    // Content area
        ])
        .split(area);

    draw_tabs(f, chunks[0], app);
    draw_profile_content(f, chunks[1], app);
}

fn draw_tabs(f: &mut Frame, area: Rect, app: &App) {
    let tab_titles = vec![
        "Security",
        "Migration",
        "Performance",
        "Compliance",
        "Hardening",
    ];
    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(" Profile Reports ")
                .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
        )
        .select(app.selected_profile_tab)
        .style(Style::default().fg(TEXT_COLOR))
        .highlight_style(
            Style::default()
                .fg(ORANGE)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        );

    f.render_widget(tabs, area);
}

fn draw_profile_content(f: &mut Frame, area: Rect, app: &App) {
    if let Some(report) = app.get_current_profile_report() {
        draw_profile_report(f, area, report, app);
    } else {
        let empty = Paragraph::new("Profile data not available")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(BORDER_COLOR)),
            )
            .style(Style::default().fg(TEXT_COLOR));
        f.render_widget(empty, area);
    }
}

fn draw_profile_report(f: &mut Frame, area: Rect, report: &ProfileReport, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Summary/Overall Risk
            Constraint::Min(0),    // Findings list
        ])
        .split(area);

    // Draw summary section
    draw_summary(f, chunks[0], report);

    // Draw findings
    draw_findings(f, chunks[1], report, app);
}

fn draw_summary(f: &mut Frame, area: Rect, report: &ProfileReport) {
    let summary_text = if let Some(ref summary) = report.summary {
        summary.clone()
    } else {
        format!("{} Profile Report", report.profile_name)
    };

    let risk_info = if let Some(risk) = report.overall_risk {
        let (risk_text, risk_color) = match risk {
            RiskLevel::Critical => ("CRITICAL", ERROR_COLOR),
            RiskLevel::High => ("HIGH", ERROR_COLOR),
            RiskLevel::Medium => ("MEDIUM", WARNING_COLOR),
            RiskLevel::Low => ("LOW", SUCCESS_COLOR),
            RiskLevel::Info => ("INFO", TEXT_COLOR),
        };
        vec![
            Span::raw("  Overall Risk: "),
            Span::styled(
                risk_text,
                Style::default().fg(risk_color).add_modifier(Modifier::BOLD),
            ),
        ]
    } else {
        vec![Span::raw("  No risk assessment available")]
    };

    let summary_paragraph = Paragraph::new(vec![Line::from(summary_text), Line::from(risk_info)])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(" Summary ")
                .title_style(Style::default().fg(ORANGE)),
        )
        .style(Style::default().fg(TEXT_COLOR));

    f.render_widget(summary_paragraph, area);
}

fn draw_findings(f: &mut Frame, area: Rect, report: &ProfileReport, app: &App) {
    let mut items: Vec<ListItem> = Vec::new();

    // Collect all findings from all sections
    for section in &report.sections {
        // Add section header
        items.push(ListItem::new(Line::from(vec![Span::styled(
            format!("▶ {}", section.title),
            Style::default()
                .fg(LIGHT_ORANGE)
                .add_modifier(Modifier::BOLD),
        )])));

        // Add findings
        for finding in &section.findings {
            let status_symbol = format!("{}", finding.status);
            let status_color = match finding.status {
                FindingStatus::Pass => SUCCESS_COLOR,
                FindingStatus::Warning => WARNING_COLOR,
                FindingStatus::Fail => ERROR_COLOR,
                FindingStatus::Info => TEXT_COLOR,
            };

            let mut spans = vec![
                Span::raw("  "),
                Span::styled(status_symbol, Style::default().fg(status_color)),
                Span::raw(" "),
                Span::styled(&finding.item, Style::default().fg(LIGHT_ORANGE)),
                Span::raw(": "),
                Span::styled(&finding.message, Style::default().fg(TEXT_COLOR)),
            ];

            // Add risk level if present
            if let Some(risk) = finding.risk_level {
                let (risk_text, risk_color) = match risk {
                    RiskLevel::Critical => (" [CRITICAL]", ERROR_COLOR),
                    RiskLevel::High => (" [HIGH]", ERROR_COLOR),
                    RiskLevel::Medium => (" [MEDIUM]", WARNING_COLOR),
                    RiskLevel::Low => (" [LOW]", SUCCESS_COLOR),
                    RiskLevel::Info => (" [INFO]", TEXT_COLOR),
                };
                spans.push(Span::styled(
                    risk_text,
                    Style::default().fg(risk_color).add_modifier(Modifier::BOLD),
                ));
            }

            items.push(ListItem::new(Line::from(spans)));
        }

        // Add spacing between sections
        items.push(ListItem::new(Line::from("")));
    }

    // Apply scrolling
    let visible_items: Vec<ListItem> = items
        .into_iter()
        .skip(app.scroll_offset)
        .take(area.height.saturating_sub(2) as usize)
        .collect();

    let list = List::new(visible_items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR))
            .title(format!(" {} Findings ", report.profile_name))
            .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
    );

    f.render_widget(list, area);
}
