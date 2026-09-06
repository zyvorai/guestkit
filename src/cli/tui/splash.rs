// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Splash screen — carbon surfaces, orange accent, Zyvor branding.

use crate::cli::tui::config::UiConfig;
use crate::cli::tui::theme::{
    fill_background, resolve, ACCENT, ACCENT_SOFT, BORDER_MUTED, LINK, TEXT, TEXT_MUTED,
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Zyvor wordmark (terminal-safe ASCII). Logo reference: https://zyvor.dev
const ZYVOR_LOGO: &[&str] = &[
    "  ███████╗██╗   ██╗██╗   ██╗ ██████╗ ██████╗ ",
    "  ╚══███╔╝╚██╗ ██╔╝██║   ██║██╔═══██╗██╔══██╗",
    "    ██╔╝  ╚████╔╝ ██║   ██║██║   ██║██████╔╝",
    "    ██║    ╚██╔╝  ╚██╗ ██╔╝██║   ██║██╔══██╗",
    "    ███████╗██║    ╚████╔╝ ╚██████╔╝██║  ██║",
    "    ╚══════╝╚═╝     ╚═══╝   ╚═════╝ ╚═╝  ╚═╝",
];

pub fn draw_splash(f: &mut Frame, cfg: &UiConfig) {
    let th = resolve(cfg);
    f.render_widget(fill_background(&th), f.area());

    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(50),
            Constraint::Percentage(25),
        ])
        .split(area);

    let logo_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(15),
            Constraint::Percentage(70),
            Constraint::Percentage(15),
        ])
        .split(chunks[1]);

    let mut logo: Vec<Line> = Vec::new();
    logo.push(Line::from(""));
    for (i, row) in ZYVOR_LOGO.iter().enumerate() {
        let color = if i < 3 { ACCENT } else { ACCENT_SOFT };
        logo.push(Line::from(Span::styled(
            *row,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )));
    }
    logo.push(Line::from(""));
    logo.push(Line::from(vec![
        Span::styled("  HyperSDK Platform  ", Style::default().fg(TEXT_MUTED)),
        Span::styled("·", Style::default().fg(BORDER_MUTED)),
        Span::styled(
            "  zyvor.dev  ",
            Style::default().fg(LINK).add_modifier(Modifier::UNDERLINED),
        ),
    ]));
    logo.push(Line::from(""));
    logo.push(Line::from(vec![
        Span::styled(
            "  GuestKit  ",
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        ),
        Span::styled("VM inspection & analysis", Style::default().fg(TEXT_MUTED)),
    ]));
    logo.push(Line::from(""));
    logo.push(Line::from(Span::styled(
        "        Press any key to continue…",
        Style::default().fg(TEXT_MUTED),
    )));

    let splash = Paragraph::new(logo).alignment(Alignment::Center).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(ACCENT))
            .style(Style::default().bg(th.surface_raised)),
    );

    f.render_widget(splash, logo_chunks[1]);
}
