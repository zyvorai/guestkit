// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Dashboard view - System overview

use crate::cli::profiles::RiskLevel;
use crate::cli::tui::app::App;
use crate::cli::tui::theme::value_style;
use crate::cli::tui::theme::{
    self, content_block, label_style, ACCENT, ERROR, INFO, SUCCESS, TEXT, WARNING,
};
use crate::cli::tui::widgets;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{BarChart, Gauge, List, ListItem, Paragraph},
    Frame,
};
use std::cmp;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
        .split(area);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(9),
            Constraint::Min(0),
        ])
        .split(main_chunks[0]);

    draw_system_info(f, left_chunks[0], app);
    draw_risk_chart(f, left_chunks[1], app);
    draw_stats(f, left_chunks[2], app);
    draw_quick_info(f, left_chunks[3], app);
    draw_health_meter(f, main_chunks[1], app);
}

fn draw_system_info(f: &mut Frame, area: Rect, app: &App) {
    let emoji = theme::use_emoji(&app.config.ui);
    let os_icon = if emoji {
        if app.os_name.to_lowercase().contains("windows") {
            "🪟"
        } else if app.os_name.to_lowercase().contains("linux")
            || app.os_name.to_lowercase().contains("ubuntu")
            || app.os_name.to_lowercase().contains("debian")
        {
            "🐧"
        } else if app.os_name.to_lowercase().contains("macos") {
            "🍎"
        } else {
            "💻"
        }
    } else {
        "[os]"
    };

    let mut info_lines = vec![
        Line::from(vec![
            Span::raw(format!("{os_icon} ")),
            Span::styled("OS:         ", label_style()),
            Span::styled(
                &app.os_name,
                Style::default().fg(SUCCESS).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Version:    ", label_style()),
            Span::styled(&app.os_version, value_style()),
        ]),
        Line::from(vec![
            Span::styled("Kernel:     ", label_style()),
            Span::styled(&app.kernel_version, Style::default().fg(INFO)),
        ]),
        Line::from(vec![
            Span::styled("Arch:       ", label_style()),
            Span::styled(
                &app.architecture,
                Style::default().fg(WARNING).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Hostname:   ", label_style()),
            Span::styled(
                &app.hostname,
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    if let Some(ref cmp) = app.compare_summary {
        info_lines.push(Line::from(vec![
            Span::styled("Compare:    ", label_style()),
            Span::styled(&cmp.hostname, Style::default().fg(ACCENT)),
            Span::raw(format!("  {} pkgs", cmp.package_count)),
        ]));
    }

    if let Some(ref boot) = app.boot_report {
        let score = boot.score.round() as u8;
        let color = widgets::health_score_color(score);
        info_lines.push(Line::from(vec![
            Span::styled("Boot:       ", label_style()),
            Span::styled(
                format!("{score}%"),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" ({})", app.assurance_target), theme::label_style()),
        ]));
    } else {
        info_lines.push(Line::from(vec![
            Span::styled("Boot:       ", label_style()),
            Span::styled("—  press ", theme::label_style()),
            Span::styled(
                "a",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" for Assurance", theme::label_style()),
        ]));
    }

    f.render_widget(
        Paragraph::new(info_lines).block(content_block("System", app.theme())),
        area,
    );
}

fn draw_risk_chart(f: &mut Frame, area: Rect, app: &App) {
    fn risk_to_value(risk: Option<RiskLevel>) -> u64 {
        match risk {
            Some(RiskLevel::Critical) => 5,
            Some(RiskLevel::High) => 4,
            Some(RiskLevel::Medium) => 3,
            Some(RiskLevel::Low) => 2,
            Some(RiskLevel::Info) => 1,
            None => 1,
        }
    }

    let data = vec![
        (
            "Sec",
            risk_to_value(app.security_profile.as_ref().and_then(|p| p.overall_risk)),
        ),
        (
            "Mig",
            risk_to_value(app.migration_profile.as_ref().and_then(|p| p.overall_risk)),
        ),
        (
            "Perf",
            risk_to_value(
                app.performance_profile
                    .as_ref()
                    .and_then(|p| p.overall_risk),
            ),
        ),
        (
            "Comp",
            risk_to_value(app.compliance_profile.as_ref().and_then(|p| p.overall_risk)),
        ),
        (
            "Hard",
            risk_to_value(app.hardening_profile.as_ref().and_then(|p| p.overall_risk)),
        ),
    ];

    let barchart = BarChart::default()
        .block(content_block("Profile risk", app.theme()))
        .data(&data)
        .bar_width(6)
        .bar_gap(2)
        .bar_style(label_style())
        .value_style(value_style())
        .label_style(label_style())
        .bar_set(symbols::bar::NINE_LEVELS);

    f.render_widget(barchart, area);
}

fn draw_stats(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(area);

    let spark = |count: usize, seed: usize, base: usize, cap: usize| -> Vec<u64> {
        (0..12)
            .map(|i| {
                let b = cmp::max(base, count.saturating_sub(cap));
                (b + (i * seed + count) % 30) as u64
            })
            .collect()
    };

    let pkg_count = app.packages.package_count;
    let svc_count = app.services.len();
    let net_count = app.network_interfaces.len();

    for (i, (title, count, max, color)) in [
        ("Packages", pkg_count, 1000, ACCENT),
        ("Services", svc_count, 100, SUCCESS),
        ("Network", net_count, 10, WARNING),
    ]
    .iter()
    .enumerate()
    {
        let col = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(4), Constraint::Length(4)])
            .split(chunks[i]);
        let data = spark(*count, 7 + i, 5, 50);
        f.render_widget(
            theme::sparkline_widget(
                &format!("{title} trend"),
                &data,
                theme::SPARKLINE_MUTED,
                app.theme(),
            ),
            col[0],
        );
        let pct = ((*count).min(*max) as f64 / *max as f64 * 100.0) as u16;
        f.render_widget(
            theme::gauge_widget(title, pct, &format!("{count}"), *color, app.theme()),
            col[1],
        );
    }
}

fn draw_quick_info(f: &mut Frame, area: Rect, app: &App) {
    if app.comparison_mode {
        draw_comparison_stats(f, area, app);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let apparmor = if app.security.apparmor {
        "enabled".to_string()
    } else {
        "disabled".to_string()
    };
    let security_items: Vec<ListItem> = [
        (
            "SELinux",
            app.security.selinux.as_str(),
            app.security.selinux != "disabled",
        ),
        ("AppArmor", apparmor.as_str(), app.security.apparmor),
        (
            "Firewall",
            app.firewall.firewall_type.as_str(),
            app.firewall.enabled,
        ),
    ]
    .iter()
    .map(|(n, s, ok)| status_item(n, s, *ok))
    .collect();

    f.render_widget(
        List::new(security_items).block(content_block("Security", app.theme())),
        chunks[0],
    );

    let mut app_items = Vec::new();
    if !app.databases.is_empty() {
        let names: Vec<&str> = app.databases.iter().map(|d| d.name.as_str()).collect();
        app_items.push(ListItem::new(Line::from(vec![
            Span::styled("DB: ", label_style()),
            Span::styled(names.join(", "), Style::default().fg(SUCCESS)),
        ])));
    }
    app_items.push(ListItem::new(Line::from(vec![
        Span::styled("TZ: ", label_style()),
        Span::styled(&app.timezone, Style::default().fg(TEXT)),
    ])));

    f.render_widget(
        List::new(app_items).block(content_block("Details", app.theme())),
        chunks[1],
    );
}

fn status_item(name: &str, status: &str, enabled: bool) -> ListItem<'static> {
    let (sym, color) = if enabled {
        ("+", SUCCESS)
    } else {
        ("-", ERROR)
    };
    ListItem::new(Line::from(vec![
        Span::styled(sym, Style::default().fg(color).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {name:10} "), label_style()),
        Span::styled(status.to_string(), Style::default().fg(TEXT)),
    ]))
}

fn draw_health_meter(f: &mut Frame, area: Rect, app: &App) {
    let health_score = app.calculate_health_score();
    let (status_text, _) = app.get_health_status();
    let health_color = widgets::health_score_color(health_score);
    let (critical, high, medium) = app.get_risk_summary();
    let (donut, donut_color) = widgets::risk_donut_ascii(critical, high, medium, 14);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(4),
            Constraint::Min(0),
        ])
        .split(area);

    let gauge = Gauge::default()
        .block(content_block("Health", app.theme()))
        .gauge_style(Style::default().fg(health_color))
        .percent(u16::from(health_score.min(100)))
        .label(format!("{health_score}% {status_text}"));

    f.render_widget(gauge, chunks[0]);

    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(donut, Style::default().fg(donut_color))),
            Line::from(vec![
                Span::styled(format!("C:{critical} "), Style::default().fg(ERROR)),
                Span::styled(format!("H:{high} "), Style::default().fg(ACCENT)),
                Span::styled(format!("M:{medium}"), Style::default().fg(WARNING)),
            ]),
        ])
        .alignment(Alignment::Center),
        chunks[1],
    );

    let details = vec![ListItem::new(Line::from(vec![
        Span::styled("Issues view ", label_style()),
        Span::styled("8", Style::default().fg(ACCENT)),
    ]))];
    f.render_widget(
        List::new(details).block(content_block("Risk", app.theme())),
        chunks[2],
    );
}

fn draw_comparison_stats(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let (pkg_added, pkg_removed, pkg_modified) = app.get_package_diff_stats();
    let package_items = vec![
        ListItem::new(Line::from(vec![
            Span::styled("Added ", label_style()),
            Span::styled(format!("{pkg_added}"), Style::default().fg(SUCCESS)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("Removed ", label_style()),
            Span::styled(format!("{pkg_removed}"), Style::default().fg(ERROR)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("Modified ", label_style()),
            Span::styled(format!("{pkg_modified}"), Style::default().fg(WARNING)),
        ])),
    ];
    f.render_widget(
        List::new(package_items).block(content_block("Package diff", app.theme())),
        chunks[0],
    );

    let (svc_started, svc_stopped, svc_changed) = app.get_service_diff_stats();
    let service_items = vec![
        ListItem::new(Line::from(vec![
            Span::styled("Started ", label_style()),
            Span::styled(format!("{svc_started}"), Style::default().fg(SUCCESS)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("Stopped ", label_style()),
            Span::styled(format!("{svc_stopped}"), Style::default().fg(ERROR)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("Changed ", label_style()),
            Span::styled(format!("{svc_changed}"), Style::default().fg(WARNING)),
        ])),
    ];
    f.render_widget(
        List::new(service_items).block(content_block("Service diff", app.theme())),
        chunks[1],
    );
}
