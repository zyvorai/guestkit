// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Web servers view - Web server installations and configurations

use crate::cli::tui::app::App;
use crate::cli::tui::ui::{
    BORDER_COLOR, LIGHT_ORANGE, ORANGE, SUCCESS_COLOR, TEXT_COLOR, WARNING_COLOR,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{BarChart, Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame,
};

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    if app.web_servers.is_empty() {
        let empty = Paragraph::new("⚠️  No web server installations found")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(BORDER_COLOR))
                    .title(" 🌐 Web Servers ")
                    .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().fg(TEXT_COLOR));
        f.render_widget(empty, area);
        return;
    }

    // Split area into summary and list
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(13), // Summary with chart and gauge
            Constraint::Min(0),     // Server list
        ])
        .split(area);

    draw_server_summary(f, chunks[0], app);
    draw_server_list(f, chunks[1], app);
}

fn draw_server_summary(f: &mut Frame, area: Rect, app: &App) {
    // Count server types
    let nginx_count = app
        .web_servers
        .iter()
        .filter(|ws| ws.name.to_lowercase().contains("nginx"))
        .count();
    let apache_count = app
        .web_servers
        .iter()
        .filter(|ws| {
            ws.name.to_lowercase().contains("apache") || ws.name.to_lowercase().contains("httpd")
        })
        .count();
    let caddy_count = app
        .web_servers
        .iter()
        .filter(|ws| ws.name.to_lowercase().contains("caddy"))
        .count();
    let lighttpd_count = app
        .web_servers
        .iter()
        .filter(|ws| ws.name.to_lowercase().contains("lighttpd"))
        .count();
    let other_count =
        app.web_servers.len() - nginx_count - apache_count - caddy_count - lighttpd_count;

    let enabled_count = app.web_servers.iter().filter(|ws| ws.enabled).count();
    let total_count = app.web_servers.len();

    let enabled_pct = if total_count > 0 {
        (enabled_count as f64 / total_count as f64 * 100.0) as u16
    } else {
        0
    };

    // Split into chart and gauge
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Type distribution chart
            Constraint::Length(3),  // Enabled gauge
        ])
        .split(area);

    // Type distribution chart
    let mut data = Vec::new();
    if nginx_count > 0 {
        data.push(("Nginx", nginx_count as u64));
    }
    if apache_count > 0 {
        data.push(("Apache", apache_count as u64));
    }
    if caddy_count > 0 {
        data.push(("Caddy", caddy_count as u64));
    }
    if lighttpd_count > 0 {
        data.push(("Lighttpd", lighttpd_count as u64));
    }
    if other_count > 0 {
        data.push(("Other", other_count as u64));
    }

    let barchart = BarChart::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(format!(
                    " 📊 Web Server Type Distribution • {} total ",
                    total_count
                ))
                .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
        )
        .data(&data)
        .bar_width(10)
        .bar_gap(2)
        .bar_style(Style::default().fg(SUCCESS_COLOR))
        .value_style(Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD));

    f.render_widget(barchart, chunks[0]);

    // Enabled status gauge
    let enabled_gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(" ⚡ Enabled Web Servers "),
        )
        .gauge_style(Style::default().fg(SUCCESS_COLOR))
        .percent(enabled_pct)
        .label(format!(
            "{}/{} enabled ({}%)",
            enabled_count, total_count, enabled_pct
        ));

    f.render_widget(enabled_gauge, chunks[1]);
}

fn draw_server_list(f: &mut Frame, area: Rect, app: &App) {
    let filtered_webservers: Vec<_> = if app.is_searching() && !app.search_query.is_empty() {
        app.web_servers
            .iter()
            .filter(|ws| {
                ws.name
                    .to_lowercase()
                    .contains(&app.search_query.to_lowercase())
                    || ws
                        .version
                        .to_lowercase()
                        .contains(&app.search_query.to_lowercase())
                    || ws
                        .config_path
                        .to_lowercase()
                        .contains(&app.search_query.to_lowercase())
            })
            .collect()
    } else {
        app.web_servers.iter().collect()
    };

    let items: Vec<ListItem> = filtered_webservers
        .iter()
        .skip(app.scroll_offset)
        .take(area.height.saturating_sub(2) as usize)
        .map(|ws| {
            // Determine web server icon and color
            let (icon, server_color) = match ws.name.to_lowercase().as_str() {
                s if s.contains("nginx") => ("⚡", SUCCESS_COLOR),
                s if s.contains("apache") || s.contains("httpd") => ("🪶", WARNING_COLOR),
                s if s.contains("caddy") => ("📦", LIGHT_ORANGE),
                s if s.contains("lighttpd") => ("💡", TEXT_COLOR),
                _ => ("🌐", TEXT_COLOR),
            };

            // Determine enabled status
            let status = if ws.enabled {
                ("✓", SUCCESS_COLOR)
            } else {
                ("✗", WARNING_COLOR)
            };

            ListItem::new(Line::from(vec![
                ratatui::text::Span::raw(format!("{} ", icon)),
                ratatui::text::Span::styled(
                    format!("{:15} ", ws.name),
                    Style::default()
                        .fg(server_color)
                        .add_modifier(Modifier::BOLD),
                ),
                ratatui::text::Span::styled(
                    format!("{} ", status.0),
                    Style::default().fg(status.1).add_modifier(Modifier::BOLD),
                ),
                ratatui::text::Span::styled(
                    format!("v{:10} ", ws.version),
                    Style::default().fg(LIGHT_ORANGE),
                ),
                ratatui::text::Span::styled(
                    format!("config: {}", ws.config_path),
                    Style::default().fg(TEXT_COLOR),
                ),
            ]))
        })
        .collect();

    // Calculate scroll position
    let visible_items = area.height.saturating_sub(2) as usize;
    let total_items = filtered_webservers.len();
    let scroll_pct = if total_items > 0 {
        ((app.scroll_offset as f32 / total_items.max(1) as f32) * 100.0) as u16
    } else {
        0
    };

    let scroll_indicator = if total_items > visible_items {
        format!(" 📜 {}% ", scroll_pct)
    } else {
        String::new()
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR))
            .title(format!(
                " 🌐 Web Server List • {} showing{} ",
                filtered_webservers.len(),
                scroll_indicator
            ))
            .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
    );

    f.render_widget(list, area);
}
