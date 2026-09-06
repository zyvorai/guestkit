// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Security view - Security tools and configuration

use crate::cli::tui::app::App;
use crate::cli::tui::ui::{
    BORDER_COLOR, ERROR_COLOR, LIGHT_ORANGE, ORANGE, SUCCESS_COLOR, TEXT_COLOR,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Security tools
            Constraint::Min(0),     // Firewall + SSH keys
        ])
        .split(area);

    draw_security_tools(f, chunks[0], app);
    draw_firewall_ssh(f, chunks[1], app);
}

fn draw_security_tools(f: &mut Frame, area: Rect, app: &App) {
    let items = vec![
        create_status_item(
            "SELinux",
            &app.security.selinux,
            &app.security.selinux != "disabled",
        ),
        create_status_item(
            "AppArmor",
            if app.security.apparmor {
                "enabled"
            } else {
                "disabled"
            },
            app.security.apparmor,
        ),
        create_status_item(
            "fail2ban",
            if app.security.fail2ban {
                "installed"
            } else {
                "not found"
            },
            app.security.fail2ban,
        ),
        create_status_item(
            "AIDE",
            if app.security.aide {
                "installed"
            } else {
                "not found"
            },
            app.security.aide,
        ),
        create_status_item(
            "auditd",
            if app.security.auditd {
                "enabled"
            } else {
                "disabled"
            },
            app.security.auditd,
        ),
    ];

    let enabled_count = [
        app.security.selinux != "disabled",
        app.security.apparmor,
        app.security.fail2ban,
        app.security.aide,
        app.security.auditd,
    ]
    .iter()
    .filter(|&&x| x)
    .count();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR))
            .title(format!(
                " 🔒 Security Tools • {} of 5 enabled ",
                enabled_count
            ))
            .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
    );

    f.render_widget(list, area);
}

fn draw_firewall_ssh(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Firewall
    let fw_items = vec![
        ListItem::new(ratatui::text::Line::from(vec![
            ratatui::text::Span::styled("Type:    ", Style::default().fg(LIGHT_ORANGE)),
            ratatui::text::Span::styled(
                &app.firewall.firewall_type,
                Style::default().fg(TEXT_COLOR),
            ),
        ])),
        ListItem::new(ratatui::text::Line::from(vec![
            ratatui::text::Span::styled("Enabled: ", Style::default().fg(LIGHT_ORANGE)),
            ratatui::text::Span::styled(
                if app.firewall.enabled { "Yes" } else { "No" },
                Style::default().fg(if app.firewall.enabled {
                    SUCCESS_COLOR
                } else {
                    ERROR_COLOR
                }),
            ),
        ])),
        ListItem::new(ratatui::text::Line::from(vec![
            ratatui::text::Span::styled("Rules:   ", Style::default().fg(LIGHT_ORANGE)),
            ratatui::text::Span::styled(
                format!("{}", app.firewall.rules_count),
                Style::default().fg(TEXT_COLOR),
            ),
        ])),
    ];

    let fw_status = if app.firewall.enabled {
        "🟢 Active"
    } else {
        "🔴 Inactive"
    };

    let fw_list = List::new(fw_items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR))
            .title(format!(" 🛡️  Firewall • {} ", fw_status))
            .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
    );

    f.render_widget(fw_list, chunks[0]);

    // SSH Keys
    let ssh_items: Vec<ListItem> = if app.security.ssh_keys.is_empty() {
        vec![ListItem::new("No SSH keys found")]
    } else {
        let filtered_keys: Vec<_> = if app.is_searching() && !app.search_query.is_empty() {
            app.security
                .ssh_keys
                .iter()
                .filter(|(user, _)| {
                    user.to_lowercase()
                        .contains(&app.search_query.to_lowercase())
                })
                .collect()
        } else {
            app.security.ssh_keys.iter().collect()
        };

        filtered_keys
            .iter()
            .map(|(user, count)| {
                ListItem::new(ratatui::text::Line::from(vec![
                    ratatui::text::Span::styled(user, Style::default().fg(LIGHT_ORANGE)),
                    ratatui::text::Span::raw(": "),
                    ratatui::text::Span::styled(
                        format!("{} key(s)", count),
                        Style::default().fg(TEXT_COLOR),
                    ),
                ]))
            })
            .collect()
    };

    let total_keys: usize = app.security.ssh_keys.iter().map(|(_, count)| count).sum();

    let ssh_list = List::new(ssh_items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR))
            .title(format!(
                " 🔑 SSH Keys • {} keys for {} users ",
                total_keys,
                app.security.ssh_keys.len()
            ))
            .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
    );

    f.render_widget(ssh_list, chunks[1]);
}

fn create_status_item(name: &str, status: &str, enabled: bool) -> ListItem<'static> {
    let (symbol, color) = if enabled {
        ("✓", SUCCESS_COLOR)
    } else {
        ("✗", ERROR_COLOR)
    };

    ListItem::new(ratatui::text::Line::from(vec![
        ratatui::text::Span::styled(
            symbol.to_string(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        ratatui::text::Span::raw(" "),
        ratatui::text::Span::styled(format!("{:12} ", name), Style::default().fg(LIGHT_ORANGE)),
        ratatui::text::Span::styled(status.to_string(), Style::default().fg(TEXT_COLOR)),
    ]))
}
