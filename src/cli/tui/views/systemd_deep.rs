// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Systemd Deep Dive — Phase 0/1 TUI (units, timers, sockets, problems).

use crate::cli::tui::app::App;
use crate::cli::tui::ui::{BORDER_COLOR, ORANGE, TEXT_COLOR, WARNING_COLOR};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_COLOR))
        .title(" 🧩 Systemd Deep Dive ")
        .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD));

    let Some(intel) = app.intelligence.as_ref() else {
        let empty = Paragraph::new("Load Assurance / doctor evidence first (Ctrl+P → Assurance).")
            .block(block)
            .style(Style::default().fg(TEXT_COLOR));
        f.render_widget(empty, area);
        return;
    };

    let sem = &intel.semantic;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Percentage(35),
            Constraint::Percentage(35),
            Constraint::Min(6),
        ])
        .margin(1)
        .split(area);

    let summary = Paragraph::new(format!(
        "Graph: {} nodes / {} edges · {} timers · {} sockets · {} problems · {} sandbox reviews",
        sem.dependency_graph.nodes.len(),
        sem.dependency_graph.edges.len(),
        sem.timer_units.len(),
        sem.socket_units.len(),
        sem.problem_units.len(),
        sem.sandbox_scores.len(),
    ))
    .style(Style::default().fg(TEXT_COLOR));
    f.render_widget(summary, chunks[0]);

    let problems: Vec<ListItem> = sem
        .problem_units
        .iter()
        .take(12)
        .map(|u| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{} ", u.name),
                    Style::default()
                        .fg(WARNING_COLOR)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(u.description.clone().unwrap_or_default()),
            ]))
        })
        .collect();
    f.render_widget(
        List::new(problems).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(" Problem units "),
        ),
        chunks[1],
    );

    let timers: Vec<ListItem> = sem
        .timer_units
        .iter()
        .chain(sem.socket_units.iter())
        .take(14)
        .map(|u| ListItem::new(format!("{} [{}] {}", u.name, u.unit_type, u.state)))
        .collect();
    f.render_widget(
        List::new(timers).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(" Timers & sockets "),
        ),
        chunks[2],
    );

    let sandbox: Vec<ListItem> = sem
        .sandbox_scores
        .iter()
        .take(8)
        .map(|s| {
            ListItem::new(format!(
                "{} score {} (root={})",
                s.unit, s.score, s.runs_as_root
            ))
        })
        .collect();
    f.render_widget(
        List::new(sandbox).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(" Sandboxing scores "),
        ),
        chunks[3],
    );
}
