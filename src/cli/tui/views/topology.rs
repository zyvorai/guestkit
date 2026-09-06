// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! System topology and architecture visualization

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

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Left: System architecture
            Constraint::Percentage(50), // Right: Network topology
        ])
        .split(area);

    draw_system_architecture(f, chunks[0], app);
    draw_network_topology(f, chunks[1], app);
}

fn draw_system_architecture(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50), // Top: System stack
            Constraint::Percentage(50), // Bottom: Service dependencies
        ])
        .split(area);

    draw_system_stack(f, chunks[0], app);
    draw_service_dependencies(f, chunks[1], app);
}

fn draw_system_stack(f: &mut Frame, area: Rect, app: &App) {
    let mut lines = vec![
        Line::from(vec![Span::styled(
            "┌────────────────────────────────────────┐",
            Style::default().fg(LIGHT_ORANGE),
        )]),
        Line::from(vec![
            Span::styled("│ ", Style::default().fg(LIGHT_ORANGE)),
            Span::styled(
                "  👥 Applications & Services",
                Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD),
            ),
            Span::styled("           │", Style::default().fg(LIGHT_ORANGE)),
        ]),
    ];

    // Applications layer
    if !app.web_servers.is_empty() || !app.databases.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(LIGHT_ORANGE)),
            Span::raw("    "),
        ]));

        if !app.web_servers.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("│ ", Style::default().fg(LIGHT_ORANGE)),
                Span::styled("    🌐 Web Servers: ", Style::default().fg(SUCCESS_COLOR)),
                Span::styled(
                    format!("{}", app.web_servers.len()),
                    Style::default().fg(TEXT_COLOR),
                ),
                Span::styled("                  │", Style::default().fg(LIGHT_ORANGE)),
            ]));
        }

        if !app.databases.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("│ ", Style::default().fg(LIGHT_ORANGE)),
                Span::styled("    🗄️  Databases: ", Style::default().fg(SUCCESS_COLOR)),
                Span::styled(
                    format!("{}", app.databases.len()),
                    Style::default().fg(TEXT_COLOR),
                ),
                Span::styled("                     │", Style::default().fg(LIGHT_ORANGE)),
            ]));
        }
    }

    lines.push(Line::from(vec![Span::styled(
        "├────────────────────────────────────────┤",
        Style::default().fg(LIGHT_ORANGE),
    )]));
    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(LIGHT_ORANGE)),
        Span::styled(
            "  ⚙️  System Services",
            Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD),
        ),
        Span::styled("                    │", Style::default().fg(LIGHT_ORANGE)),
    ]));

    // Services layer
    let enabled_services = app.services.iter().filter(|s| s.enabled).count();
    let running_services = app.services.iter().filter(|s| s.state == "running").count();

    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(LIGHT_ORANGE)),
        Span::styled("    Total: ", Style::default().fg(INFO_COLOR)),
        Span::styled(
            format!("{}", app.services.len()),
            Style::default().fg(TEXT_COLOR),
        ),
        Span::raw("  Enabled: "),
        Span::styled(
            format!("{}", enabled_services),
            Style::default().fg(SUCCESS_COLOR),
        ),
        Span::raw("  Running: "),
        Span::styled(
            format!("{}", running_services),
            Style::default().fg(SUCCESS_COLOR),
        ),
        Span::styled("  │", Style::default().fg(LIGHT_ORANGE)),
    ]));

    lines.push(Line::from(vec![Span::styled(
        "├────────────────────────────────────────┤",
        Style::default().fg(LIGHT_ORANGE),
    )]));
    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(LIGHT_ORANGE)),
        Span::styled(
            "  🐧 Operating System",
            Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD),
        ),
        Span::styled("                    │", Style::default().fg(LIGHT_ORANGE)),
    ]));

    // OS layer
    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(LIGHT_ORANGE)),
        Span::styled("    OS: ", Style::default().fg(INFO_COLOR)),
        Span::styled(&app.os_name, Style::default().fg(TEXT_COLOR)),
        Span::styled(
            "                         │",
            Style::default().fg(LIGHT_ORANGE),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(LIGHT_ORANGE)),
        Span::styled("    Init: ", Style::default().fg(INFO_COLOR)),
        Span::styled(&app.init_system, Style::default().fg(TEXT_COLOR)),
        Span::styled("                      │", Style::default().fg(LIGHT_ORANGE)),
    ]));

    lines.push(Line::from(vec![Span::styled(
        "├────────────────────────────────────────┤",
        Style::default().fg(LIGHT_ORANGE),
    )]));
    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(LIGHT_ORANGE)),
        Span::styled(
            "  🧩 Kernel",
            Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "                              │",
            Style::default().fg(LIGHT_ORANGE),
        ),
    ]));

    // Kernel layer
    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(LIGHT_ORANGE)),
        Span::styled("    Version: ", Style::default().fg(INFO_COLOR)),
        Span::styled(&app.kernel_version, Style::default().fg(TEXT_COLOR)),
        Span::styled("                 │", Style::default().fg(LIGHT_ORANGE)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(LIGHT_ORANGE)),
        Span::styled("    Modules: ", Style::default().fg(INFO_COLOR)),
        Span::styled(
            format!("{}", app.kernel_modules.len()),
            Style::default().fg(TEXT_COLOR),
        ),
        Span::styled(
            "                       │",
            Style::default().fg(LIGHT_ORANGE),
        ),
    ]));

    lines.push(Line::from(vec![Span::styled(
        "├────────────────────────────────────────┤",
        Style::default().fg(LIGHT_ORANGE),
    )]));
    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(LIGHT_ORANGE)),
        Span::styled(
            "  💻 Hardware",
            Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "                            │",
            Style::default().fg(LIGHT_ORANGE),
        ),
    ]));

    // Hardware layer
    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(LIGHT_ORANGE)),
        Span::styled("    Arch: ", Style::default().fg(INFO_COLOR)),
        Span::styled(&app.architecture, Style::default().fg(TEXT_COLOR)),
        Span::styled(
            "                       │",
            Style::default().fg(LIGHT_ORANGE),
        ),
    ]));

    lines.push(Line::from(vec![Span::styled(
        "└────────────────────────────────────────┘",
        Style::default().fg(LIGHT_ORANGE),
    )]));

    let stack = Paragraph::new(lines).block(
        Block::default()
            .title(Span::styled(
                "🏗️  System Architecture Stack",
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR)),
    );

    f.render_widget(stack, area);
}

fn draw_service_dependencies(f: &mut Frame, area: Rect, app: &App) {
    let mut items = vec![];

    // Critical services
    let critical_services = vec!["sshd", "systemd", "network", "firewalld", "iptables"];

    items.push(ListItem::new(vec![Line::from(vec![Span::styled(
        "Critical Services:",
        Style::default()
            .fg(LIGHT_ORANGE)
            .add_modifier(Modifier::BOLD),
    )])]));

    for critical in &critical_services {
        let found = app.services.iter().any(|s| s.name.contains(critical));
        let status_icon = if found { "✓" } else { "✗" };
        let status_color = if found { SUCCESS_COLOR } else { ERROR_COLOR };

        items.push(ListItem::new(vec![Line::from(vec![
            Span::raw("  "),
            Span::styled(status_icon, Style::default().fg(status_color)),
            Span::raw(" "),
            Span::styled(*critical, Style::default().fg(TEXT_COLOR)),
        ])]));
    }

    items.push(ListItem::new(vec![Line::from("")]));
    items.push(ListItem::new(vec![Line::from(vec![Span::styled(
        "Service Categories:",
        Style::default()
            .fg(LIGHT_ORANGE)
            .add_modifier(Modifier::BOLD),
    )])]));

    // Count service types
    let system_svcs = app
        .services
        .iter()
        .filter(|s| s.name.starts_with("system"))
        .count();
    let network_svcs = app
        .services
        .iter()
        .filter(|s| s.name.contains("network") || s.name.contains("net"))
        .count();
    let security_svcs = app
        .services
        .iter()
        .filter(|s| {
            s.name.contains("firewall") || s.name.contains("security") || s.name.contains("audit")
        })
        .count();

    items.push(ListItem::new(vec![Line::from(vec![
        Span::raw("  ⚙️  System: "),
        Span::styled(format!("{}", system_svcs), Style::default().fg(INFO_COLOR)),
    ])]));
    items.push(ListItem::new(vec![Line::from(vec![
        Span::raw("  🌐 Network: "),
        Span::styled(format!("{}", network_svcs), Style::default().fg(INFO_COLOR)),
    ])]));
    items.push(ListItem::new(vec![Line::from(vec![
        Span::raw("  🔒 Security: "),
        Span::styled(
            format!("{}", security_svcs),
            Style::default().fg(INFO_COLOR),
        ),
    ])]));

    let list = List::new(items).block(
        Block::default()
            .title(Span::styled(
                "🔗 Service Dependencies",
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR)),
    );

    f.render_widget(list, area);
}

fn draw_network_topology(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60), // Network diagram
            Constraint::Percentage(40), // Connection details
        ])
        .split(area);

    draw_network_diagram(f, chunks[0], app);
    draw_connection_details(f, chunks[1], app);
}

fn draw_network_diagram(f: &mut Frame, area: Rect, app: &App) {
    let mut lines = vec![
        Line::from(vec![Span::styled(
            "                 🌍 Internet",
            Style::default().fg(INFO_COLOR).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            "                      │",
            Style::default().fg(TEXT_COLOR),
        )]),
        Line::from(vec![Span::styled(
            "                      ▼",
            Style::default().fg(TEXT_COLOR),
        )]),
    ];

    // Firewall layer
    if app.firewall.enabled {
        lines.push(Line::from(vec![Span::styled(
            "            ┌─────────────────┐",
            Style::default().fg(SUCCESS_COLOR),
        )]));
        lines.push(Line::from(vec![
            Span::styled(
                "            │  🔥 Firewall",
                Style::default().fg(SUCCESS_COLOR),
            ),
            Span::raw("    │"),
        ]));
        lines.push(Line::from(vec![
            Span::styled("            │  ", Style::default().fg(SUCCESS_COLOR)),
            Span::styled(&app.firewall.firewall_type, Style::default().fg(TEXT_COLOR)),
            Span::styled("      │", Style::default().fg(SUCCESS_COLOR)),
        ]));
        lines.push(Line::from(vec![Span::styled(
            "            └─────────────────┘",
            Style::default().fg(SUCCESS_COLOR),
        )]));
    } else {
        lines.push(Line::from(vec![Span::styled(
            "            ┌─────────────────┐",
            Style::default().fg(ERROR_COLOR),
        )]));
        lines.push(Line::from(vec![Span::styled(
            "            │ ⚠️  No Firewall  │",
            Style::default()
                .fg(ERROR_COLOR)
                .add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(vec![Span::styled(
            "            └─────────────────┘",
            Style::default().fg(ERROR_COLOR),
        )]));
    }

    lines.push(Line::from(vec![Span::styled(
        "                      │",
        Style::default().fg(TEXT_COLOR),
    )]));
    lines.push(Line::from(vec![Span::styled(
        "                      ▼",
        Style::default().fg(TEXT_COLOR),
    )]));

    // Network interfaces
    lines.push(Line::from(vec![Span::styled(
        "         Network Interfaces",
        Style::default()
            .fg(LIGHT_ORANGE)
            .add_modifier(Modifier::BOLD),
    )]));

    for (idx, iface) in app.network_interfaces.iter().enumerate().take(3) {
        let has_ip = !iface.ip_address.is_empty();
        let status_color = if has_ip { SUCCESS_COLOR } else { WARNING_COLOR };
        let status_icon = if has_ip { "✓" } else { "○" };

        lines.push(Line::from(vec![Span::styled(
            "         ┌──────────────────┐",
            Style::default().fg(status_color),
        )]));
        lines.push(Line::from(vec![
            Span::styled("         │ ", Style::default().fg(status_color)),
            Span::styled(status_icon, Style::default().fg(status_color)),
            Span::raw(" "),
            Span::styled(&iface.name, Style::default().fg(TEXT_COLOR)),
            Span::styled("           │", Style::default().fg(status_color)),
        ]));
        lines.push(Line::from(vec![Span::styled(
            "         └──────────────────┘",
            Style::default().fg(status_color),
        )]));

        if idx < app.network_interfaces.len() - 1 && idx < 2 {
            lines.push(Line::from(vec![Span::styled(
                "                │",
                Style::default().fg(TEXT_COLOR),
            )]));
        }
    }

    if app.network_interfaces.len() > 3 {
        lines.push(Line::from(vec![Span::styled(
            "              ...",
            Style::default().fg(TEXT_COLOR),
        )]));
        lines.push(Line::from(vec![Span::styled(
            format!("         ({} more)", app.network_interfaces.len() - 3),
            Style::default().fg(TEXT_COLOR),
        )]));
    }

    let diagram = Paragraph::new(lines).block(
        Block::default()
            .title(Span::styled(
                "🌐 Network Topology",
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR)),
    );

    f.render_widget(diagram, area);
}

fn draw_connection_details(f: &mut Frame, area: Rect, app: &App) {
    let mut items = vec![];

    items.push(ListItem::new(vec![Line::from(vec![Span::styled(
        "Network Configuration:",
        Style::default()
            .fg(LIGHT_ORANGE)
            .add_modifier(Modifier::BOLD),
    )])]));

    items.push(ListItem::new(vec![Line::from(vec![
        Span::raw("  Interfaces: "),
        Span::styled(
            format!("{}", app.network_interfaces.len()),
            Style::default().fg(SUCCESS_COLOR),
        ),
    ])]));

    let configured = app
        .network_interfaces
        .iter()
        .filter(|i| !i.ip_address.is_empty())
        .count();
    items.push(ListItem::new(vec![Line::from(vec![
        Span::raw("  Configured: "),
        Span::styled(
            format!("{}", configured),
            Style::default().fg(SUCCESS_COLOR),
        ),
    ])]));

    items.push(ListItem::new(vec![Line::from(vec![
        Span::raw("  DNS Servers: "),
        Span::styled(
            format!("{}", app.dns_servers.len()),
            Style::default().fg(INFO_COLOR),
        ),
    ])]));

    items.push(ListItem::new(vec![Line::from("")]));
    items.push(ListItem::new(vec![Line::from(vec![Span::styled(
        "Security:",
        Style::default()
            .fg(LIGHT_ORANGE)
            .add_modifier(Modifier::BOLD),
    )])]));

    let firewall_status = if app.firewall.enabled {
        ("Enabled", SUCCESS_COLOR)
    } else {
        ("Disabled", ERROR_COLOR)
    };

    items.push(ListItem::new(vec![Line::from(vec![
        Span::raw("  Firewall: "),
        Span::styled(firewall_status.0, Style::default().fg(firewall_status.1)),
    ])]));

    if app.firewall.enabled {
        items.push(ListItem::new(vec![Line::from(vec![
            Span::raw("  Type: "),
            Span::styled(&app.firewall.firewall_type, Style::default().fg(TEXT_COLOR)),
        ])]));
    }

    let list = List::new(items).block(
        Block::default()
            .title(Span::styled(
                "📊 Connection Details",
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR)),
    );

    f.render_widget(list, area);
}
