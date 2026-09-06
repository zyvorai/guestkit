// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Users view - System user accounts browser

use crate::cli::tui::app::App;
use crate::cli::tui::ui::{
    BORDER_COLOR, ERROR_COLOR, INFO_COLOR, LIGHT_ORANGE, ORANGE, SUCCESS_COLOR, TEXT_COLOR,
    WARNING_COLOR,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Gauge, List, ListItem, Paragraph, Row, Table},
    Frame,
};

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    if app.users.is_empty() {
        let empty = Paragraph::new("⚠️  No user accounts found")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(BORDER_COLOR))
                    .title(" 👥 User Accounts ")
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
            Constraint::Length(12), // User type summary with gauges
            Constraint::Min(0),     // User list
        ])
        .split(area);

    draw_user_summary(f, chunks[0], app);
    draw_user_list(f, chunks[1], app);
}

fn draw_user_summary(f: &mut Frame, area: Rect, app: &App) {
    // Count different user types
    let root_count = app.users.iter().filter(|u| u.uid == "0").count();
    let system_count = app
        .users
        .iter()
        .filter(|u| {
            let uid: i32 = u.uid.parse().unwrap_or(99999);
            uid > 0 && uid < 1000
        })
        .count();
    let normal_count = app
        .users
        .iter()
        .filter(|u| {
            let uid: i32 = u.uid.parse().unwrap_or(99999);
            (1000..65534).contains(&uid)
        })
        .count();
    let total_count = app.users.len();

    // Interactive shells count
    let interactive_shells = app
        .users
        .iter()
        .filter(|u| u.shell.contains("bash") || u.shell.contains("zsh") || u.shell.contains("sh"))
        .count();

    let normal_pct = if total_count > 0 {
        (normal_count as f64 / total_count as f64 * 100.0) as u16
    } else {
        0
    };

    let system_pct = if total_count > 0 {
        (system_count as f64 / total_count as f64 * 100.0) as u16
    } else {
        0
    };

    let interactive_pct = if total_count > 0 {
        (interactive_shells as f64 / total_count as f64 * 100.0) as u16
    } else {
        0
    };

    // Split into header and gauges
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(3), // Normal users gauge
            Constraint::Length(3), // System users gauge
            Constraint::Length(3), // Interactive shells gauge
        ])
        .split(area);

    // Header
    let header = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            " 📊 User Account Statistics",
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("Total Users: ", Style::default().fg(LIGHT_ORANGE)),
            Span::styled(
                format!("{}", total_count),
                Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  │  "),
            Span::styled("👑 Root: ", Style::default().fg(ERROR_COLOR)),
            Span::styled(
                format!("{}", root_count),
                Style::default()
                    .fg(ERROR_COLOR)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("⚙️  System: ", Style::default().fg(WARNING_COLOR)),
            Span::styled(
                format!("{}", system_count),
                Style::default().fg(WARNING_COLOR),
            ),
            Span::raw("  "),
            Span::styled("👤 Normal: ", Style::default().fg(SUCCESS_COLOR)),
            Span::styled(
                format!("{}", normal_count),
                Style::default().fg(SUCCESS_COLOR),
            ),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR)),
    );

    f.render_widget(header, chunks[0]);

    // Normal users gauge
    let normal_gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(" 👤 Normal Users (UID ≥ 1000) "),
        )
        .gauge_style(Style::default().fg(SUCCESS_COLOR))
        .percent(normal_pct)
        .label(format!(
            "{} normal users ({}% of total)",
            normal_count, normal_pct
        ));

    f.render_widget(normal_gauge, chunks[1]);

    // System users gauge
    let system_gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(" ⚙️  System Users (1 ≤ UID < 1000) "),
        )
        .gauge_style(Style::default().fg(WARNING_COLOR))
        .percent(system_pct)
        .label(format!(
            "{} system users ({}% of total)",
            system_count, system_pct
        ));

    f.render_widget(system_gauge, chunks[2]);

    // Interactive shells gauge
    let shell_gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(" 💻 Interactive Shells "),
        )
        .gauge_style(Style::default().fg(INFO_COLOR))
        .percent(interactive_pct)
        .label(format!(
            "{} with interactive shells ({}% of total)",
            interactive_shells, interactive_pct
        ));

    f.render_widget(shell_gauge, chunks[3]);
}

fn draw_user_list(f: &mut Frame, area: Rect, app: &App) {
    if app.table_mode {
        draw_user_table_view(f, area, app);
    } else {
        draw_user_list_view(f, area, app);
    }
}

fn draw_user_list_view(f: &mut Frame, area: Rect, app: &App) {
    // Get sorted indices
    let sorted_indices = app.get_sorted_user_indices();

    // Apply filtering if searching
    let filtered_indices: Vec<usize> = if app.is_searching() && !app.search_query.is_empty() {
        sorted_indices
            .into_iter()
            .filter(|&idx| {
                let user = &app.users[idx];
                user.username
                    .to_lowercase()
                    .contains(&app.search_query.to_lowercase())
                    || user.uid.contains(&app.search_query)
                    || user
                        .shell
                        .to_lowercase()
                        .contains(&app.search_query.to_lowercase())
                    || user
                        .home
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
        .map(|&user_idx| {
            let user = &app.users[user_idx];
            // Parse UID to determine user type
            let uid: i32 = user.uid.parse().unwrap_or(99999);

            // Color coding and icon:
            // - Root (UID 0): Red with crown icon
            // - System users (UID < 1000): Yellow with gear icon
            // - Normal users (UID >= 1000): Green with person icon
            let (icon, username_color, uid_color) = if uid == 0 {
                ("👑", ERROR_COLOR, ERROR_COLOR) // Root in red with crown
            } else if uid < 1000 {
                ("⚙️ ", WARNING_COLOR, WARNING_COLOR) // System users in warning color
            } else {
                ("👤", SUCCESS_COLOR, LIGHT_ORANGE) // Normal users in success color
            };

            // Detect potentially problematic shells
            let shell_color = if user.shell.contains("nologin") || user.shell.contains("false") {
                TEXT_COLOR // Disabled shells
            } else if user.shell.contains("bash")
                || user.shell.contains("zsh")
                || user.shell.contains("sh")
            {
                SUCCESS_COLOR // Interactive shells
            } else {
                WARNING_COLOR // Other shells
            };

            ListItem::new(Line::from(vec![
                ratatui::text::Span::raw(format!("{} ", icon)),
                ratatui::text::Span::styled(
                    format!("{:16}", user.username),
                    Style::default()
                        .fg(username_color)
                        .add_modifier(Modifier::BOLD),
                ),
                ratatui::text::Span::styled(
                    format!("UID:{:5} ", user.uid),
                    Style::default().fg(uid_color),
                ),
                ratatui::text::Span::styled(
                    format!("GID:{:5} ", user.gid),
                    Style::default().fg(TEXT_COLOR),
                ),
                ratatui::text::Span::styled(
                    format!("{:30} ", user.home),
                    Style::default().fg(LIGHT_ORANGE),
                ),
                ratatui::text::Span::styled(&user.shell, Style::default().fg(shell_color)),
            ]))
        })
        .collect();

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
                " 👥 User Account List • {} showing{}{} ",
                filtered_indices.len(),
                scroll_indicator,
                sort_indicator
            ))
            .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
    );

    f.render_widget(list, area);
}

fn draw_user_table_view(f: &mut Frame, area: Rect, app: &App) {
    // Get sorted indices
    let sorted_indices = app.get_sorted_user_indices();

    // Apply filtering if searching
    let filtered_indices: Vec<usize> = if app.is_searching() && !app.search_query.is_empty() {
        sorted_indices
            .into_iter()
            .filter(|&idx| {
                let user = &app.users[idx];
                user.username
                    .to_lowercase()
                    .contains(&app.search_query.to_lowercase())
                    || user.uid.contains(&app.search_query)
                    || user
                        .shell
                        .to_lowercase()
                        .contains(&app.search_query.to_lowercase())
                    || user
                        .home
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
            "Type",
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Username",
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "UID",
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "GID",
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Home",
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Shell",
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
        .map(|&user_idx| {
            let user = &app.users[user_idx];

            // Parse UID to determine user type
            let uid: i32 = user.uid.parse().unwrap_or(99999);

            // Type icon and colors
            let (icon, username_color, uid_color) = if uid == 0 {
                ("👑", ERROR_COLOR, ERROR_COLOR)
            } else if uid < 1000 {
                ("⚙️", WARNING_COLOR, WARNING_COLOR)
            } else {
                ("👤", SUCCESS_COLOR, LIGHT_ORANGE)
            };

            // Shell color
            let shell_color = if user.shell.contains("nologin") || user.shell.contains("false") {
                TEXT_COLOR
            } else if user.shell.contains("bash")
                || user.shell.contains("zsh")
                || user.shell.contains("sh")
            {
                SUCCESS_COLOR
            } else {
                WARNING_COLOR
            };

            Row::new(vec![
                Cell::from(Span::styled(icon, Style::default().fg(username_color))),
                Cell::from(Span::styled(
                    &user.username,
                    Style::default()
                        .fg(username_color)
                        .add_modifier(Modifier::BOLD),
                )),
                Cell::from(Span::styled(&user.uid, Style::default().fg(uid_color))),
                Cell::from(Span::styled(&user.gid, Style::default().fg(INFO_COLOR))),
                Cell::from(Span::styled(&user.home, Style::default().fg(TEXT_COLOR))),
                Cell::from(Span::styled(&user.shell, Style::default().fg(shell_color))),
            ])
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

    let sort_indicator = if !matches!(app.sort_mode, crate::cli::tui::app::SortMode::Default) {
        format!(" [Sort: {}] ", app.sort_mode.label())
    } else {
        String::new()
    };

    // Create table widget
    let widths = [
        Constraint::Length(4),      // Type icon
        Constraint::Percentage(20), // Username
        Constraint::Length(8),      // UID
        Constraint::Length(8),      // GID
        Constraint::Percentage(30), // Home
        Constraint::Percentage(50), // Shell
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(format!(
                    " 👥 User Accounts (Table) • {} showing{}{} ",
                    filtered_indices.len(),
                    scroll_indicator,
                    sort_indicator
                ))
                .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
        )
        .column_spacing(2);

    f.render_widget(table, area);
}
