// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Packages view - Installed packages browser

use crate::cli::tui::app::{App, LayoutMode};
use crate::cli::tui::ui::{
    content_block, label_style, BORDER_COLOR, INFO_COLOR, LIGHT_ORANGE, ORANGE, SUCCESS_COLOR,
    TEXT_COLOR, WARNING_COLOR,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Gauge, List, ListItem, Paragraph, Row, Table},
    Frame,
};

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    // Determine package manager icon
    let manager_icon = match app.packages.manager.to_lowercase().as_str() {
        "rpm" | "dnf" | "yum" => "📦",
        "deb" | "apt" | "dpkg" => "📦",
        "pacman" => "📦",
        "apk" => "📦",
        "zypper" => "📦",
        _ => "📦",
    };

    if app.packages.packages.is_empty() {
        let empty = Paragraph::new(format!(
            "⚠️  No packages found (manager: {})",
            app.packages.manager
        ))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(format!(" {} Installed Packages ", manager_icon))
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
            Constraint::Length(9), // Summary with statistics
            Constraint::Min(0),    // Package list
        ])
        .split(area);

    draw_package_summary(f, chunks[0], app);
    match app.layout_mode {
        LayoutMode::ListOnly => draw_package_list(f, chunks[1], app),
        LayoutMode::DetailFull => draw_package_detail(f, chunks[1], app),
        LayoutMode::SplitDetail => {
            let split = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
                .split(chunks[1]);
            draw_package_list(f, split[0], app);
            draw_package_detail(f, split[1], app);
        }
    }
}

fn filtered_package_indices(app: &App) -> Vec<usize> {
    let sorted = app.get_sorted_package_indices();
    if app.is_searching() && !app.search_query.is_empty() {
        let q = app.search_query.to_lowercase();
        sorted
            .into_iter()
            .filter(|&idx| {
                let pkg = &app.packages.packages[idx];
                pkg.name.to_lowercase().contains(&q) || pkg.version.contains(&app.search_query)
            })
            .collect()
    } else {
        sorted
    }
}

fn draw_package_detail(f: &mut Frame, area: Rect, app: &App) {
    let indices = filtered_package_indices(app);
    let lines = if let Some(&idx) = indices.get(app.selected_index) {
        let pkg = &app.packages.packages[idx];
        vec![
            Line::from(vec![
                Span::styled("Name: ", label_style()),
                Span::styled(
                    &pkg.name,
                    Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Version: ", label_style()),
                Span::styled(&pkg.version, Style::default().fg(SUCCESS_COLOR)),
            ]),
            Line::from(vec![
                Span::styled("Manager: ", label_style()),
                Span::styled(&pkg.manager, Style::default().fg(INFO_COLOR)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Query: ", label_style()),
                Span::raw(format!(
                    "rpm -q {} 2>/dev/null || dpkg -l {}",
                    pkg.name, pkg.name
                )),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled("Select a package", label_style()))]
    };
    f.render_widget(
        Paragraph::new(lines)
            .block(content_block("Package detail", app.theme()))
            .wrap(ratatui::widgets::Wrap { trim: true }),
        area,
    );
}

fn draw_package_summary(f: &mut Frame, area: Rect, app: &App) {
    // Count packages by prefix
    let lib_count = app
        .packages
        .packages
        .iter()
        .filter(|p| p.name.starts_with("lib"))
        .count();
    let python_count = app
        .packages
        .packages
        .iter()
        .filter(|p| p.name.starts_with("python"))
        .count();
    let total_count = app.packages.package_count;

    let lib_pct = if total_count > 0 {
        (lib_count as f64 / total_count as f64 * 100.0) as u16
    } else {
        0
    };

    let python_pct = if total_count > 0 {
        (python_count as f64 / total_count as f64 * 100.0) as u16
    } else {
        0
    };

    // Split into header and gauges
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(3), // Lib packages gauge
            Constraint::Length(3), // Python packages gauge
        ])
        .split(area);

    // Header with package info
    let header = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            " 📊 Package Statistics",
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("Total Packages: ", Style::default().fg(LIGHT_ORANGE)),
            Span::styled(
                format!("{}", total_count),
                Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  │  "),
            Span::styled("Manager: ", Style::default().fg(LIGHT_ORANGE)),
            Span::styled(
                &app.packages.manager,
                Style::default().fg(INFO_COLOR).add_modifier(Modifier::BOLD),
            ),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR)),
    );

    f.render_widget(header, chunks[0]);

    // Library packages gauge
    let lib_gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(" 📚 Library Packages (lib*) "),
        )
        .gauge_style(Style::default().fg(INFO_COLOR))
        .percent(lib_pct)
        .label(format!("{} libraries ({}% of total)", lib_count, lib_pct));

    f.render_widget(lib_gauge, chunks[1]);

    // Python packages gauge
    let python_gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(" 🐍 Python Packages "),
        )
        .gauge_style(Style::default().fg(WARNING_COLOR))
        .percent(python_pct)
        .label(format!(
            "{} python packages ({}% of total)",
            python_count, python_pct
        ));

    f.render_widget(python_gauge, chunks[2]);
}

fn draw_package_list(f: &mut Frame, area: Rect, app: &App) {
    if app.table_mode {
        draw_package_table_view(f, area, app);
    } else {
        draw_package_list_view(f, area, app);
    }
}

fn draw_package_list_view(f: &mut Frame, area: Rect, app: &App) {
    let manager_icon = match app.packages.manager.to_lowercase().as_str() {
        "rpm" | "dnf" | "yum" => "📦",
        "deb" | "apt" | "dpkg" => "📦",
        "pacman" => "📦",
        "apk" => "📦",
        "zypper" => "📦",
        _ => "📦",
    };

    // Get sorted indices
    let sorted_indices = app.get_sorted_package_indices();

    // Apply filtering if searching
    let filtered_indices: Vec<usize> = if app.is_searching() && !app.search_query.is_empty() {
        sorted_indices
            .into_iter()
            .filter(|&idx| {
                let pkg = &app.packages.packages[idx];
                pkg.name
                    .to_lowercase()
                    .contains(&app.search_query.to_lowercase())
                    || pkg.version.contains(&app.search_query)
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
        .map(|(display_idx, &pkg_idx)| {
            let pkg = &app.packages.packages[pkg_idx];
            // Alternate colors for better readability
            let name_color = if display_idx % 2 == 0 {
                LIGHT_ORANGE
            } else {
                ORANGE
            };
            let actual_idx = app.scroll_offset + display_idx;

            // Multi-select checkbox or comparison indicator
            let (prefix, prefix_color) = if app.comparison_mode {
                if let Some(ref snapshot) = app.snapshot_packages {
                    if let Some(old_pkg) = snapshot.iter().find(|p| p.name == pkg.name) {
                        if old_pkg.version != pkg.version {
                            ("⟳ ", WARNING_COLOR) // Modified
                        } else {
                            ("  ", TEXT_COLOR) // Unchanged
                        }
                    } else {
                        ("+ ", SUCCESS_COLOR) // Added
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
                ("• ", TEXT_COLOR)
            };

            ListItem::new(Line::from(vec![
                ratatui::text::Span::styled(prefix, Style::default().fg(prefix_color)),
                ratatui::text::Span::styled(
                    &pkg.name,
                    Style::default().fg(name_color).add_modifier(Modifier::BOLD),
                ),
                ratatui::text::Span::raw("  "),
                ratatui::text::Span::styled("v", Style::default().fg(INFO_COLOR)),
                ratatui::text::Span::styled(&pkg.version, Style::default().fg(SUCCESS_COLOR)),
            ]))
        })
        .collect();

    // Calculate scroll position percentage
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
                " {} Installed Packages • {} showing of {} total • Manager: {}{}{}{}{} ",
                manager_icon,
                filtered_indices.len(),
                app.packages.package_count,
                app.packages.manager,
                scroll_indicator,
                multiselect_indicator,
                filter_indicator,
                sort_indicator
            ))
            .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
    );

    f.render_widget(list, area);
}

fn draw_package_table_view(f: &mut Frame, area: Rect, app: &App) {
    let manager_icon = match app.packages.manager.to_lowercase().as_str() {
        "rpm" | "dnf" | "yum" => "📦",
        "deb" | "apt" | "dpkg" => "📦",
        "pacman" => "📦",
        "apk" => "📦",
        "zypper" => "📦",
        _ => "📦",
    };

    // Get sorted indices
    let sorted_indices = app.get_sorted_package_indices();

    // Apply filtering if searching
    let filtered_indices: Vec<usize> = if app.is_searching() && !app.search_query.is_empty() {
        sorted_indices
            .into_iter()
            .filter(|&idx| {
                let pkg = &app.packages.packages[idx];
                pkg.name
                    .to_lowercase()
                    .contains(&app.search_query.to_lowercase())
                    || pkg.version.contains(&app.search_query)
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
            "Package Name",
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Version",
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Manager",
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        )),
    ])
    .height(1)
    .bottom_margin(1);

    // Create table rows
    let rows: Vec<Row> = filtered_indices
        .iter()
        .skip(app.scroll_offset)
        .take(area.height.saturating_sub(4) as usize) // Account for header and borders
        .enumerate()
        .map(|(display_idx, &pkg_idx)| {
            let pkg = &app.packages.packages[pkg_idx];
            let actual_idx = app.scroll_offset + display_idx;

            // Alternate row colors
            let row_style = if display_idx % 2 == 0 {
                Style::default()
            } else {
                Style::default().fg(LIGHT_ORANGE)
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
                    &pkg.name,
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Cell::from(Span::styled(
                    &pkg.version,
                    Style::default().fg(SUCCESS_COLOR),
                )),
                Cell::from(Span::styled(&pkg.manager, Style::default().fg(INFO_COLOR))),
            ])
            .style(row_style)
        })
        .collect();

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
        Constraint::Percentage(50), // Package name
        Constraint::Percentage(30), // Version
        Constraint::Percentage(20), // Manager
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(format!(
                    " {} Packages (Table) • {} showing of {} total • Manager: {}{}{}{}{} ",
                    manager_icon,
                    filtered_indices.len(),
                    app.packages.package_count,
                    app.packages.manager,
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
