// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! AI Insights panel — recommendations + narrative (Phase 2/3 TUI).

use crate::cli::tui::app::App;
use crate::cli::tui::ui::{BORDER_COLOR, ORANGE, SUCCESS_COLOR, TEXT_COLOR, WARNING_COLOR};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_COLOR))
        .title(" 🤖 AI Insights ")
        .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD));

    let Some(intel) = app.intelligence.as_ref() else {
        let empty = Paragraph::new(
            "Run doctor --explain or migrate-plan --explain to populate Guest Intelligence.",
        )
        .block(block)
        .style(Style::default().fg(TEXT_COLOR));
        f.render_widget(empty, area);
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(0),
            Constraint::Length(6),
        ])
        .margin(1)
        .split(area);

    let summary = Paragraph::new(intel.narrative.executive_summary.as_str())
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(TEXT_COLOR));
    f.render_widget(summary, chunks[0]);

    let recs: Vec<ListItem> = intel
        .recommendations
        .iter()
        .take(16)
        .map(|r| {
            let color = match format!("{:?}", r.category).as_str() {
                "Critical" => WARNING_COLOR,
                "Security" => ORANGE,
                _ => SUCCESS_COLOR,
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("[{:?}] ", r.category),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    r.title.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" — {}", r.detail)),
            ]))
        })
        .collect();
    f.render_widget(
        List::new(recs).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(" Recommendations "),
        ),
        chunks[1],
    );

    let cis = format!(
        "CIS-lite profile {} — score {}/100 ({} pass, {} fail)",
        intel.security_profile.profile,
        intel.security_profile.score,
        intel.security_profile.passed.len(),
        intel.security_profile.failed.len()
    );
    f.render_widget(
        Paragraph::new(cis).style(Style::default().fg(TEXT_COLOR)),
        chunks[2],
    );

    let _ = block;
}
