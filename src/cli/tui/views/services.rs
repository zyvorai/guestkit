// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Services view - Systemd services viewer

use crate::cli::tui::app::{App, LayoutMode};
use crate::cli::tui::ui::{
    content_block, label_style, BORDER_COLOR, ERROR_COLOR, INFO_COLOR, LIGHT_ORANGE, ORANGE,
    SUCCESS_COLOR, TEXT_COLOR, WARNING_COLOR,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Gauge, List, ListItem, Paragraph, Row, Table},
    Frame,
};

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    if app.services.is_empty() {
        let empty = Paragraph::new("⚠️  No systemd services found")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(BORDER_COLOR))
                    .title(" ⚙️  Systemd Services ")
                    .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().fg(TEXT_COLOR));
        f.render_widget(empty, area);
        return;
    }

    // Split area into summary and list sections
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // Summary gauges
            Constraint::Min(0),    // Service list
        ])
        .split(area);

    draw_service_summary(f, chunks[0], app);
    match app.layout_mode {
        LayoutMode::ListOnly => draw_service_list(f, chunks[1], app),
        LayoutMode::DetailFull => draw_service_detail(f, chunks[1], app),
        LayoutMode::SplitDetail => {
            let split = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
                .split(chunks[1]);
            draw_service_list(f, split[0], app);
            draw_service_detail(f, split[1], app);
        }
    }
}

fn filtered_service_indices(app: &App) -> Vec<usize> {
    if app.is_searching() && !app.search_query.is_empty() {
        let q = app.search_query.to_lowercase();
        app.services
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                s.name.to_lowercase().contains(&q) || s.state.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect()
    } else {
        (0..app.services.len()).collect()
    }
}

fn draw_service_detail(f: &mut Frame, area: Rect, app: &App) {
    let indices = filtered_service_indices(app);
    let lines = if let Some(&idx) = indices.get(app.selected_index) {
        let svc = &app.services[idx];
        vec![
            Line::from(vec![
                Span::styled("Unit: ", label_style()),
                Span::styled(
                    &svc.name,
                    Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("State: ", label_style()),
                Span::styled(
                    &svc.state,
                    Style::default().fg(if svc.state == "running" {
                        SUCCESS_COLOR
                    } else {
                        WARNING_COLOR
                    }),
                ),
            ]),
            Line::from(vec![
                Span::styled("Enabled: ", label_style()),
                Span::raw(if svc.enabled { "yes" } else { "no" }),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Check: ", label_style()),
                Span::raw(format!("systemctl status {}", svc.name)),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled("Select a service", label_style()))]
    };
    f.render_widget(
        Paragraph::new(lines)
            .block(content_block("Service detail", app.theme()))
            .wrap(ratatui::widgets::Wrap { trim: true }),
        area,
    );
}

fn draw_service_summary(f: &mut Frame, area: Rect, app: &App) {
    let enabled_count = app.services.iter().filter(|s| s.enabled).count();
    let disabled_count = app.services.len() - enabled_count;
    let running_count = app
        .services
        .iter()
        .filter(|s| s.state == "running" || s.state == "active")
        .count();
    let stopped_count = app.services.len() - running_count;

    let enabled_pct = if !app.services.is_empty() {
        (enabled_count as f64 / app.services.len() as f64 * 100.0) as u16
    } else {
        0
    };

    let running_pct = if !app.services.is_empty() {
        (running_count as f64 / app.services.len() as f64 * 100.0) as u16
    } else {
        0
    };

    // Split into two gauge sections
    let gauge_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Length(3), // Enabled gauge
            Constraint::Length(3), // Running gauge
            Constraint::Length(1), // Padding
        ])
        .split(area);

    // Header
    let header = Paragraph::new(Line::from(vec![Span::styled(
        " 📊 Service Status Overview",
        Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
    )]));
    f.render_widget(header, gauge_chunks[0]);

    // Enabled/Disabled gauge
    let enabled_gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(" Enabled Services "),
        )
        .gauge_style(Style::default().fg(SUCCESS_COLOR))
        .percent(enabled_pct)
        .label(format!(
            "{} enabled • {} disabled ({}%)",
            enabled_count, disabled_count, enabled_pct
        ));

    f.render_widget(enabled_gauge, gauge_chunks[1]);

    // Running/Stopped gauge
    let running_gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(" Running Services "),
        )
        .gauge_style(Style::default().fg(INFO_COLOR))
        .percent(running_pct)
        .label(format!(
            "{} running • {} stopped ({}%)",
            running_count, stopped_count, running_pct
        ));

    f.render_widget(running_gauge, gauge_chunks[2]);
}

fn draw_service_list(f: &mut Frame, area: Rect, app: &App) {
    if app.table_mode {
        draw_service_table_view(f, area, app);
    } else {
        draw_service_list_view(f, area, app);
    }
}

fn draw_service_list_view(f: &mut Frame, area: Rect, app: &App) {
    // Get sorted indices
    let sorted_indices = app.get_sorted_service_indices();

    // Apply filtering if searching
    let filtered_indices: Vec<usize> = if app.is_searching() && !app.search_query.is_empty() {
        sorted_indices
            .into_iter()
            .filter(|&idx| {
                let svc = &app.services[idx];
                svc.name
                    .to_lowercase()
                    .contains(&app.search_query.to_lowercase())
                    || svc
                        .state
                        .to_lowercase()
                        .contains(&app.search_query.to_lowercase())
            })
            .collect()
    } else {
        sorted_indices
    };

    let items: Vec<ListItem> = filtered_indices
        .iter()
        .skip(app.scroll_offset)
        .take(area.height.saturating_sub(2) as usize)
        .enumerate()
        .map(|(display_idx, &svc_idx)| {
            let svc = &app.services[svc_idx];
            let actual_idx = app.scroll_offset + display_idx;

            // Determine status symbol and color based on state and enabled status
            let (status_symbol, _status_color) = match (svc.enabled, svc.state.as_str()) {
                (true, "running") => ("🟢", SUCCESS_COLOR),
                (true, "active") => ("🟢", SUCCESS_COLOR),
                (true, _) => ("🟡", WARNING_COLOR),
                (false, "running") => ("🟠", WARNING_COLOR),
                (false, _) => ("⚫", TEXT_COLOR),
            };

            // Color the state based on its value
            let state_color = match svc.state.as_str() {
                "running" | "active" => SUCCESS_COLOR,
                "stopped" | "inactive" | "failed" => ERROR_COLOR,
                _ => WARNING_COLOR,
            };

            // Multi-select checkbox or comparison indicator
            let (prefix, prefix_color) = if app.comparison_mode {
                if let Some(ref snapshot) = app.snapshot_services {
                    if let Some(old_svc) = snapshot.iter().find(|s| s.name == svc.name) {
                        if old_svc.state != svc.state {
                            if svc.state == "running" && old_svc.state != "running" {
                                ("↑ ", SUCCESS_COLOR) // Started
                            } else if svc.state != "running" && old_svc.state == "running" {
                                ("↓ ", ERROR_COLOR) // Stopped
                            } else {
                                ("⟳ ", WARNING_COLOR) // Changed
                            }
                        } else {
                            ("  ", TEXT_COLOR) // Unchanged
                        }
                    } else {
                        ("+ ", SUCCESS_COLOR) // New service
                    }
                } else {
                    ("  ", TEXT_COLOR)
                }
            } else if app.multi_select_mode {
                if app.is_item_selected(actual_idx) {
                    ("☑ ", TEXT_COLOR)
                } else {
                    ("☐ ", TEXT_COLOR)
                }
            } else {
                ("", TEXT_COLOR)
            };

            ListItem::new(ratatui::text::Line::from(vec![
                ratatui::text::Span::styled(prefix, Style::default().fg(prefix_color)),
                ratatui::text::Span::raw(format!("{} ", status_symbol)),
                ratatui::text::Span::styled(
                    &svc.name,
                    Style::default()
                        .fg(LIGHT_ORANGE)
                        .add_modifier(Modifier::BOLD),
                ),
                ratatui::text::Span::raw("  "),
                ratatui::text::Span::styled(&svc.state, Style::default().fg(state_color)),
            ]))
        })
        .collect();

    let enabled_count = app.services.iter().filter(|s| s.enabled).count();
    let running_count = app
        .services
        .iter()
        .filter(|s| s.state == "running" || s.state == "active")
        .count();

    // Calculate scroll position
    let visible_items = area.height.saturating_sub(2) as usize;
    let total_items = filtered_indices.len();
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

    // Multi-select indicator
    let multiselect_indicator = if app.multi_select_mode {
        format!(" [{}  selected] ", app.get_selected_count())
    } else {
        String::new()
    };

    // Filter indicator
    let filter_indicator = if let Some(label) = app.get_filter_label() {
        format!(" [{}] ", label)
    } else {
        String::new()
    };

    // Sort indicator
    let sort_indicator = if !matches!(app.sort_mode, crate::cli::tui::app::SortMode::Default) {
        format!(" [Sort: {}] ", app.sort_mode.label())
    } else {
        String::new()
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR))
            .title(format!(
                " ⚙️  Systemd Services • {} showing • {} enabled • {} running{}{}{}{} ",
                filtered_indices.len(),
                enabled_count,
                running_count,
                scroll_indicator,
                multiselect_indicator,
                filter_indicator,
                sort_indicator
            ))
            .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
    );

    f.render_widget(list, area);
}

fn draw_service_table_view(f: &mut Frame, area: Rect, app: &App) {
    // Get sorted indices
    let sorted_indices = app.get_sorted_service_indices();

    // Apply filtering if searching
    let filtered_indices: Vec<usize> = if app.is_searching() && !app.search_query.is_empty() {
        sorted_indices
            .into_iter()
            .filter(|&idx| {
                let svc = &app.services[idx];
                svc.name
                    .to_lowercase()
                    .contains(&app.search_query.to_lowercase())
                    || svc
                        .state
                        .to_lowercase()
                        .contains(&app.search_query.to_lowercase())
            })
            .collect()
    } else {
        sorted_indices
    };

    // Create table header
    let header = Row::new(vec![
        Cell::from(Span::styled(
            "#",
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Status",
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Service Name",
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "State",
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Enabled",
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        )),
    ])
    .height(1)
    .bottom_margin(1);

    // Create table rows
    let rows: Vec<Row> = filtered_indices
        .iter()
        .skip(app.scroll_offset)
        .take(area.height.saturating_sub(4) as usize)
        .enumerate()
        .map(|(display_idx, &svc_idx)| {
            let svc = &app.services[svc_idx];
            let actual_idx = app.scroll_offset + display_idx;

            // Determine status symbol and color
            let (status_symbol, status_color) = match (svc.enabled, svc.state.as_str()) {
                (true, "running") | (true, "active") => ("🟢", SUCCESS_COLOR),
                (true, _) => ("🟡", WARNING_COLOR),
                (false, "running") => ("🟠", WARNING_COLOR),
                (false, _) => ("⚫", TEXT_COLOR),
            };

            // State color
            let state_color = match svc.state.as_str() {
                "running" | "active" => SUCCESS_COLOR,
                "stopped" | "inactive" | "failed" => ERROR_COLOR,
                _ => WARNING_COLOR,
            };

            // Multi-select checkbox
            let checkbox = if app.multi_select_mode {
                if app.is_item_selected(actual_idx) {
                    "☑"
                } else {
                    "☐"
                }
            } else {
                ""
            };

            Row::new(vec![
                Cell::from(format!(
                    "{}{}",
                    checkbox,
                    if checkbox.is_empty() { "" } else { " " }
                )),
                Cell::from(Span::styled(
                    status_symbol,
                    Style::default().fg(status_color),
                )),
                Cell::from(Span::styled(
                    &svc.name,
                    Style::default()
                        .fg(LIGHT_ORANGE)
                        .add_modifier(Modifier::BOLD),
                )),
                Cell::from(Span::styled(&svc.state, Style::default().fg(state_color))),
                Cell::from(Span::styled(
                    if svc.enabled { "Yes" } else { "No" },
                    Style::default().fg(if svc.enabled {
                        SUCCESS_COLOR
                    } else {
                        TEXT_COLOR
                    }),
                )),
            ])
        })
        .collect();

    let enabled_count = app.services.iter().filter(|s| s.enabled).count();
    let running_count = app
        .services
        .iter()
        .filter(|s| s.state == "running" || s.state == "active")
        .count();

    // Calculate indicators
    let visible_items = area.height.saturating_sub(4) as usize;
    let total_items = filtered_indices.len();
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

    let multiselect_indicator = if app.multi_select_mode {
        format!(" [{}  selected] ", app.get_selected_count())
    } else {
        String::new()
    };

    let filter_indicator = if let Some(label) = app.get_filter_label() {
        format!(" [{}] ", label)
    } else {
        String::new()
    };

    let sort_indicator = if !matches!(app.sort_mode, crate::cli::tui::app::SortMode::Default) {
        format!(" [Sort: {}] ", app.sort_mode.label())
    } else {
        String::new()
    };

    // Create table widget
    let widths = [
        Constraint::Length(3),      // Checkbox
        Constraint::Length(6),      // Status
        Constraint::Percentage(60), // Service name
        Constraint::Percentage(20), // State
        Constraint::Percentage(20), // Enabled
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(format!(
                    " ⚙️  Services (Table) • {} showing • {} enabled • {} running{}{}{}{} ",
                    filtered_indices.len(),
                    enabled_count,
                    running_count,
                    scroll_indicator,
                    multiselect_indicator,
                    filter_indicator,
                    sort_indicator
                ))
                .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
        )
        .column_spacing(2);

    f.render_widget(table, area);
}
