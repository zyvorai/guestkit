// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Kernel view - Kernel modules and parameters

use crate::cli::tui::app::App;
use crate::cli::tui::ui::{
    BORDER_COLOR, LIGHT_ORANGE, ORANGE, SUCCESS_COLOR, TEXT_COLOR, WARNING_COLOR,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60), // Kernel modules
            Constraint::Percentage(40), // Kernel parameters
        ])
        .split(area);

    draw_modules(f, chunks[0], app);
    draw_parameters(f, chunks[1], app);
}

fn draw_modules(f: &mut Frame, area: Rect, app: &App) {
    if app.kernel_modules.is_empty() {
        let empty = Paragraph::new("⚠️  No kernel modules configured to load at boot")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(BORDER_COLOR))
                    .title(" 🧩 Kernel Modules ")
                    .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().fg(TEXT_COLOR));
        f.render_widget(empty, area);
        return;
    }

    let filtered_modules: Vec<_> = if app.is_searching() && !app.search_query.is_empty() {
        app.kernel_modules
            .iter()
            .filter(|module| {
                module
                    .to_lowercase()
                    .contains(&app.search_query.to_lowercase())
            })
            .collect()
    } else {
        app.kernel_modules.iter().collect()
    };

    let items: Vec<ListItem> = filtered_modules
        .iter()
        .skip(app.scroll_offset)
        .take(area.height.saturating_sub(2) as usize)
        .map(|module| {
            // Color code based on common module categories
            let color =
                if module.contains("net") || module.contains("ethernet") || module.contains("wifi")
                {
                    SUCCESS_COLOR // Network modules in green
                } else if module.contains("fs")
                    || module.contains("ext")
                    || module.contains("xfs")
                    || module.contains("btrfs")
                {
                    WARNING_COLOR // Filesystem modules in yellow
                } else if module.contains("usb") || module.contains("hid") {
                    LIGHT_ORANGE // USB/HID modules in light orange
                } else {
                    TEXT_COLOR // Other modules in default color
                };

            ListItem::new(Line::from(vec![
                Span::styled("● ", Style::default().fg(ORANGE)),
                Span::styled(module.as_str(), Style::default().fg(color)),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR))
            .title(format!(
                " 🧩 Kernel Modules • {} showing of {} total ",
                filtered_modules.len(),
                app.kernel_modules.len()
            ))
            .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
    );

    f.render_widget(list, area);
}

fn draw_parameters(f: &mut Frame, area: Rect, app: &App) {
    if app.kernel_params.is_empty() {
        let empty = Paragraph::new("⚠️  No kernel parameters configured")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(BORDER_COLOR))
                    .title(" ⚙️  Kernel Parameters ")
                    .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().fg(TEXT_COLOR));
        f.render_widget(empty, area);
        return;
    }

    // Convert HashMap to sorted Vec for display
    let mut params: Vec<_> = app.kernel_params.iter().collect();
    params.sort_by_key(|(k, _)| *k);

    let filtered_params: Vec<_> = if app.is_searching() && !app.search_query.is_empty() {
        params
            .iter()
            .filter(|(key, value)| {
                key.to_lowercase()
                    .contains(&app.search_query.to_lowercase())
                    || value
                        .to_lowercase()
                        .contains(&app.search_query.to_lowercase())
            })
            .collect()
    } else {
        params.iter().collect()
    };

    let items: Vec<ListItem> = filtered_params
        .iter()
        .skip(app.scroll_offset)
        .take(area.height.saturating_sub(2) as usize)
        .map(|(key, value)| {
            // Color code security-relevant parameters
            let value_color = if key.contains("kernel.") || key.contains("security.") {
                WARNING_COLOR // Security params in warning color
            } else if key.contains("net.") {
                SUCCESS_COLOR // Network params in success color
            } else if key.contains("fs.") {
                LIGHT_ORANGE // Filesystem params in light orange
            } else {
                TEXT_COLOR
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!("{:40} ", key), Style::default().fg(LIGHT_ORANGE)),
                Span::styled("= ", Style::default().fg(TEXT_COLOR)),
                Span::styled(value.as_str(), Style::default().fg(value_color)),
            ]))
        })
        .collect();

    // Count different parameter categories
    let kernel_count = app
        .kernel_params
        .keys()
        .filter(|k| k.contains("kernel."))
        .count();
    let net_count = app
        .kernel_params
        .keys()
        .filter(|k| k.contains("net."))
        .count();
    let fs_count = app
        .kernel_params
        .keys()
        .filter(|k| k.contains("fs."))
        .count();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR))
            .title(format!(
                " ⚙️  Kernel Parameters (sysctl) • {} showing • {} kernel • {} net • {} fs ",
                filtered_params.len(),
                kernel_count,
                net_count,
                fs_count
            ))
            .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
    );

    f.render_widget(list, area);
}
