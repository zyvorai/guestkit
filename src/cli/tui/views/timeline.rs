// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Timeline view showing system events and changes

use crate::cli::tui::app::App;
use crate::cli::tui::ui::{
    BORDER_COLOR, ERROR_COLOR, INFO_COLOR, LIGHT_ORANGE, ORANGE, SUCCESS_COLOR, TEXT_COLOR,
    WARNING_COLOR,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

#[derive(Debug, Clone)]
struct TimelineEvent {
    icon: &'static str,
    category: &'static str,
    description: String,
    severity: Severity,
    timestamp: String,
}

#[derive(Debug, Clone, Copy)]
enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl Severity {
    fn color(&self) -> ratatui::style::Color {
        match self {
            Severity::Critical => ERROR_COLOR,
            Severity::High => WARNING_COLOR,
            Severity::Medium => INFO_COLOR,
            Severity::Low => SUCCESS_COLOR,
            Severity::Info => TEXT_COLOR,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Severity::Critical => "CRIT",
            Severity::High => "HIGH",
            Severity::Medium => "MED",
            Severity::Low => "LOW",
            Severity::Info => "INFO",
        }
    }
}

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Summary header
            Constraint::Min(0),    // Timeline events
        ])
        .split(area);

    draw_timeline_summary(f, chunks[0], app);
    draw_timeline_events(f, chunks[1], app);
}

fn draw_timeline_summary(f: &mut Frame, area: Rect, app: &App) {
    let summary_text = vec![Line::from(vec![
        Span::styled(
            "📅 System Timeline",
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" │ "),
        Span::styled("OS: ", Style::default().fg(TEXT_COLOR)),
        Span::styled(&app.os_name, Style::default().fg(LIGHT_ORANGE)),
        Span::raw(" │ "),
        Span::styled("Kernel: ", Style::default().fg(TEXT_COLOR)),
        Span::styled(&app.kernel_version, Style::default().fg(LIGHT_ORANGE)),
    ])];

    let summary = Paragraph::new(summary_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR)),
    );

    f.render_widget(summary, area);
}

fn draw_timeline_events(f: &mut Frame, area: Rect, app: &App) {
    let events = generate_timeline_events(app);

    // Apply scrolling
    let visible_events: Vec<_> = events
        .iter()
        .skip(app.scroll_offset)
        .take((area.height as usize).saturating_sub(2))
        .collect();

    let items: Vec<ListItem> = visible_events
        .iter()
        .map(|event| {
            let severity_badge = Span::styled(
                format!("[{}]", event.severity.label()),
                Style::default()
                    .fg(event.severity.color())
                    .add_modifier(Modifier::BOLD),
            );

            let content = Line::from(vec![
                Span::raw(format!("{} ", event.icon)),
                severity_badge,
                Span::raw(" "),
                Span::styled(&event.timestamp, Style::default().fg(TEXT_COLOR)),
                Span::raw(" │ "),
                Span::styled(event.category, Style::default().fg(LIGHT_ORANGE)),
                Span::raw(": "),
                Span::styled(&event.description, Style::default().fg(TEXT_COLOR)),
            ]);

            ListItem::new(content)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(Span::styled(
                format!("⏰ Timeline Events ({} total)", events.len()),
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR)),
    );

    f.render_widget(list, area);
}

fn generate_timeline_events(app: &App) -> Vec<TimelineEvent> {
    let mut events = Vec::new();

    // System boot event
    events.push(TimelineEvent {
        icon: "🚀",
        category: "System",
        description: format!("System initialized with {} kernel", app.kernel_version),
        severity: Severity::Info,
        timestamp: "T-0d".to_string(),
    });

    // Package installations
    if app.packages.package_count > 0 {
        events.push(TimelineEvent {
            icon: "📦",
            category: "Packages",
            description: format!("{} packages installed", app.packages.package_count),
            severity: if app.packages.package_count > 2000 {
                Severity::High
            } else {
                Severity::Info
            },
            timestamp: "T-0d".to_string(),
        });
    }

    // Service configurations
    let enabled_services = app.services.iter().filter(|s| s.enabled).count();
    if enabled_services > 0 {
        events.push(TimelineEvent {
            icon: "⚙️",
            category: "Services",
            description: format!("{} services enabled", enabled_services),
            severity: if enabled_services > 50 {
                Severity::Medium
            } else {
                Severity::Low
            },
            timestamp: "T-0d".to_string(),
        });
    }

    // Security configurations
    if app.security.selinux != "disabled" {
        events.push(TimelineEvent {
            icon: "🔒",
            category: "Security",
            description: format!("SELinux enabled: {}", app.security.selinux),
            severity: Severity::Info,
            timestamp: "T-0d".to_string(),
        });
    }

    if app.security.apparmor {
        events.push(TimelineEvent {
            icon: "🔒",
            category: "Security",
            description: "AppArmor enabled".to_string(),
            severity: Severity::Info,
            timestamp: "T-0d".to_string(),
        });
    }

    // Firewall configuration
    if app.firewall.enabled {
        events.push(TimelineEvent {
            icon: "🛡️",
            category: "Security",
            description: format!("Firewall enabled: {}", app.firewall.firewall_type),
            severity: Severity::Low,
            timestamp: "T-0d".to_string(),
        });
    } else {
        events.push(TimelineEvent {
            icon: "⚠️",
            category: "Security",
            description: "Firewall disabled - security risk".to_string(),
            severity: Severity::High,
            timestamp: "T-0d".to_string(),
        });
    }

    // User account events
    let user_count = app.users.len();
    if user_count > 0 {
        let privileged_users = app.users.iter().filter(|u| u.uid == "0").count();
        events.push(TimelineEvent {
            icon: "👥",
            category: "Users",
            description: format!(
                "{} user accounts created ({} privileged)",
                user_count, privileged_users
            ),
            severity: if privileged_users > 1 {
                Severity::Critical
            } else {
                Severity::Info
            },
            timestamp: "T-0d".to_string(),
        });
    }

    // Database installations
    for db in &app.databases {
        events.push(TimelineEvent {
            icon: "🗄️",
            category: "Database",
            description: format!("{} installed at {}", db.name, db.data_dir),
            severity: Severity::Info,
            timestamp: "T-0d".to_string(),
        });
    }

    // Web server installations
    for ws in &app.web_servers {
        events.push(TimelineEvent {
            icon: "🌐",
            category: "WebServer",
            description: format!("{} {} configured", ws.name, ws.version),
            severity: Severity::Info,
            timestamp: "T-0d".to_string(),
        });
    }

    // Network interface configuration
    for iface in &app.network_interfaces {
        if !iface.ip_address.is_empty() {
            events.push(TimelineEvent {
                icon: "🌐",
                category: "Network",
                description: format!("Interface {} configured", iface.name),
                severity: Severity::Low,
                timestamp: "T-0d".to_string(),
            });
        }
    }

    // Storage configuration
    if let Some(ref lvm) = app.lvm_info {
        events.push(TimelineEvent {
            icon: "💾",
            category: "Storage",
            description: format!(
                "LVM configured: {} VG(s), {} LV(s)",
                lvm.volume_groups.len(),
                lvm.logical_volumes.len()
            ),
            severity: Severity::Info,
            timestamp: "T-0d".to_string(),
        });
    }

    if !app.raid_arrays.is_empty() {
        events.push(TimelineEvent {
            icon: "💾",
            category: "Storage",
            description: format!("{} RAID array(s) configured", app.raid_arrays.len()),
            severity: Severity::Info,
            timestamp: "T-0d".to_string(),
        });
    }

    // Kernel modules
    if !app.kernel_modules.is_empty() {
        events.push(TimelineEvent {
            icon: "🧩",
            category: "Kernel",
            description: format!("{} kernel modules loaded", app.kernel_modules.len()),
            severity: Severity::Low,
            timestamp: "T-0d".to_string(),
        });
    }

    // System locale and timezone
    if !app.timezone.is_empty() {
        events.push(TimelineEvent {
            icon: "🌍",
            category: "System",
            description: format!("Timezone set to: {}", app.timezone),
            severity: Severity::Info,
            timestamp: "T-0d".to_string(),
        });
    }

    // Sort events by severity (critical first)
    events.sort_by(|a, b| {
        let severity_order = |s: &Severity| match s {
            Severity::Critical => 0,
            Severity::High => 1,
            Severity::Medium => 2,
            Severity::Low => 3,
            Severity::Info => 4,
        };
        severity_order(&a.severity).cmp(&severity_order(&b.severity))
    });

    events
}
