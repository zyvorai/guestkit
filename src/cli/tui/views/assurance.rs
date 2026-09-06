// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Assurance view — boot gate and migration scoring (doctor / migrate-plan parity).

use crate::cli::tui::app::App;
use crate::cli::tui::theme::{
    self, content_block, label_style, ACCENT, ERROR, INFO, SUCCESS, TEXT, WARNING,
};
use crate::cli::tui::widgets;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),
            Constraint::Min(6),
            Constraint::Length(2),
        ])
        .split(area);

    draw_boot_gate(f, chunks[0], app);
    draw_migration(f, chunks[1], app);
    draw_footer(f, chunks[2], app);
}

fn draw_boot_gate(f: &mut Frame, area: Rect, app: &App) {
    let lines = if let Some(ref boot) = app.boot_report {
        let score = boot.score.round() as u8;
        let color = widgets::health_score_color(score);
        let mut lines = vec![Line::from(vec![
            Span::styled("Score ", label_style()),
            Span::styled(
                format!("{score}%"),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  target ", label_style()),
            Span::styled(&app.assurance_target, Style::default().fg(ACCENT)),
        ])];
        if app.agent_live {
            lines[0].spans.push(Span::styled(
                "  LIVE",
                Style::default().fg(SUCCESS).add_modifier(Modifier::BOLD),
            ));
        }
        lines.push(Line::from(Span::styled(
            boot.boot_probability_message(),
            Style::default().fg(SUCCESS),
        )));
        if !boot.blockers.is_empty() {
            lines.push(Line::from(Span::styled(
                "Blockers:",
                Style::default().fg(ERROR).add_modifier(Modifier::BOLD),
            )));
            for b in boot.blockers.iter().take(3) {
                lines.push(Line::from(vec![
                    Span::styled("  ✗ ", Style::default().fg(ERROR)),
                    Span::styled(
                        &b.title,
                        Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" — "),
                    Span::styled(&b.message, theme::label_style()),
                ]));
            }
        } else if !boot.warnings.is_empty() {
            lines.push(Line::from(Span::styled(
                "Warnings:",
                Style::default().fg(WARNING).add_modifier(Modifier::BOLD),
            )));
            for w in boot.warnings.iter().take(3) {
                lines.push(Line::from(vec![
                    Span::styled("  ⚠ ", Style::default().fg(WARNING)),
                    Span::styled(&w.title, theme::value_style()),
                    Span::raw(" — "),
                    Span::styled(&w.message, theme::label_style()),
                ]));
            }
        }
        lines
    } else if app.refreshing {
        vec![Line::from(Span::styled(
            "Running doctor…",
            Style::default().fg(INFO),
        ))]
    } else {
        vec![
            Line::from(Span::styled("No boot report yet.", theme::label_style())),
            Line::from(Span::styled(
                "Press d to run doctor",
                Style::default().fg(ACCENT),
            )),
        ]
    };

    f.render_widget(
        Paragraph::new(lines).block(content_block("Boot gate", app.theme())),
        area,
    );
}

fn draw_migration(f: &mut Frame, area: Rect, app: &App) {
    let inner_h = area.height.saturating_sub(2) as usize;
    let mut lines = Vec::new();

    if let Some(ref mig) = app.migration_report {
        let score = mig.score.round() as u8;
        let color = widgets::health_score_color(score);
        lines.push(Line::from(vec![
            Span::styled("Migration ", label_style()),
            Span::styled(
                format!("{score}%"),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  → ", label_style()),
            Span::styled(&app.assurance_target, Style::default().fg(ACCENT)),
            Span::styled(
                format!("  (~{} min downtime)", mig.estimated_downtime_minutes),
                theme::label_style(),
            ),
        ]));

        if !mig.driver_injections.is_empty() {
            lines.push(Line::from(Span::styled(
                "Driver injections:",
                Style::default().fg(WARNING).add_modifier(Modifier::BOLD),
            )));
            for d in &mig.driver_injections {
                lines.push(Line::from(vec![
                    Span::raw("  • "),
                    Span::styled(d, theme::value_style()),
                ]));
            }
        }

        if !mig.required_changes.is_empty() {
            lines.push(Line::from(Span::styled(
                "Required changes:",
                Style::default().fg(INFO).add_modifier(Modifier::BOLD),
            )));
            let scroll = app.scroll_offset;
            for c in mig
                .required_changes
                .iter()
                .skip(scroll)
                .take(inner_h.saturating_sub(lines.len()))
            {
                lines.push(Line::from(vec![
                    Span::raw("  • "),
                    Span::styled(c, theme::value_style()),
                ]));
            }
        }

        if !mig.licensing_warnings.is_empty() {
            lines.push(Line::from(Span::styled(
                "Licensing:",
                Style::default().fg(ERROR).add_modifier(Modifier::BOLD),
            )));
            for w in &mig.licensing_warnings {
                lines.push(Line::from(vec![
                    Span::raw("  • "),
                    Span::styled(w, Style::default().fg(WARNING)),
                ]));
            }
        }
    } else {
        lines.push(Line::from(Span::styled(
            "Run doctor (d) to compute migration score.",
            theme::label_style(),
        )));
    }

    f.render_widget(
        Paragraph::new(lines).block(content_block("Migration", app.theme())),
        area,
    );
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let hint = if app.migration_report.is_some() {
        "d doctor  t target  p plan preview  e export  ↑↓ scroll changes"
    } else {
        "d doctor  t target  p preview / e export (after load)"
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(hint, theme::label_style()))),
        area,
    );
}
