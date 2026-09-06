// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! AI-like intelligent recommendations view

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
struct Recommendation {
    icon: &'static str,
    category: &'static str,
    priority: Priority,
    title: String,
    description: String,
    impact: Impact,
    effort: Effort,
    steps: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
enum Priority {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl Priority {
    fn color(&self) -> ratatui::style::Color {
        match self {
            Priority::Critical => ERROR_COLOR,
            Priority::High => WARNING_COLOR,
            Priority::Medium => INFO_COLOR,
            Priority::Low => SUCCESS_COLOR,
            Priority::Info => TEXT_COLOR,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Priority::Critical => "CRITICAL",
            Priority::High => "HIGH",
            Priority::Medium => "MEDIUM",
            Priority::Low => "LOW",
            Priority::Info => "INFO",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Impact {
    High,
    Medium,
    Low,
}

impl Impact {
    #[allow(dead_code)]
    fn label(&self) -> &'static str {
        match self {
            Impact::High => "High Impact",
            Impact::Medium => "Medium Impact",
            Impact::Low => "Low Impact",
        }
    }

    fn color(&self) -> ratatui::style::Color {
        match self {
            Impact::High => SUCCESS_COLOR,
            Impact::Medium => INFO_COLOR,
            Impact::Low => TEXT_COLOR,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum Effort {
    High,
    Medium,
    Low,
}

impl Effort {
    #[allow(dead_code)]
    fn label(&self) -> &'static str {
        match self {
            Effort::High => "High Effort",
            Effort::Medium => "Medium Effort",
            Effort::Low => "Low Effort",
        }
    }

    fn color(&self) -> ratatui::style::Color {
        match self {
            Effort::High => ERROR_COLOR,
            Effort::Medium => WARNING_COLOR,
            Effort::Low => SUCCESS_COLOR,
        }
    }
}

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(5), // Quick stats
            Constraint::Min(0),    // Recommendations list
            Constraint::Length(3), // Footer
        ])
        .split(area);

    draw_recommendations_header(f, chunks[0], app);
    draw_quick_stats(f, chunks[1], app);
    draw_recommendations_list(f, chunks[2], app);
    draw_recommendations_footer(f, chunks[3]);
}

fn draw_recommendations_header(f: &mut Frame, area: Rect, app: &App) {
    let recommendations = generate_recommendations(app);
    let critical_count = recommendations
        .iter()
        .filter(|r| matches!(r.priority, Priority::Critical))
        .count();
    let high_count = recommendations
        .iter()
        .filter(|r| matches!(r.priority, Priority::High))
        .count();

    let header_text = vec![Line::from(vec![
        Span::styled("🤖 ", Style::default().fg(ORANGE)),
        Span::styled(
            "Intelligent Recommendations",
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" │ "),
        Span::styled(
            format!("{} total", recommendations.len()),
            Style::default().fg(TEXT_COLOR),
        ),
        Span::raw(" │ "),
        Span::styled(
            format!("🔴 {}", critical_count),
            Style::default()
                .fg(ERROR_COLOR)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("🟠 {}", high_count),
            Style::default()
                .fg(WARNING_COLOR)
                .add_modifier(Modifier::BOLD),
        ),
    ])];

    let header = Paragraph::new(header_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR)),
    );

    f.render_widget(header, area);
}

fn draw_quick_stats(f: &mut Frame, area: Rect, app: &App) {
    let recommendations = generate_recommendations(app);

    let quick_wins = recommendations
        .iter()
        .filter(|r| {
            matches!(r.effort, Effort::Low) && matches!(r.impact, Impact::High | Impact::Medium)
        })
        .count();

    let security_recs = recommendations
        .iter()
        .filter(|r| r.category == "Security")
        .count();

    let performance_recs = recommendations
        .iter()
        .filter(|r| r.category == "Performance")
        .count();

    let stats_text = vec![
        Line::from(vec![Span::styled(
            "📊 Quick Stats:",
            Style::default()
                .fg(LIGHT_ORANGE)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  Quick Wins: ", Style::default().fg(TEXT_COLOR)),
            Span::styled(
                format!("{}", quick_wins),
                Style::default()
                    .fg(SUCCESS_COLOR)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " (Low effort, high impact)",
                Style::default().fg(TEXT_COLOR),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Security: ", Style::default().fg(TEXT_COLOR)),
            Span::styled(
                format!("{}", security_recs),
                Style::default().fg(WARNING_COLOR),
            ),
            Span::raw("  │  "),
            Span::styled("Performance: ", Style::default().fg(TEXT_COLOR)),
            Span::styled(
                format!("{}", performance_recs),
                Style::default().fg(INFO_COLOR),
            ),
        ]),
    ];

    let stats = Paragraph::new(stats_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR)),
    );

    f.render_widget(stats, area);
}

fn draw_recommendations_list(f: &mut Frame, area: Rect, app: &App) {
    let recommendations = generate_recommendations(app);

    // Apply scrolling
    let visible_recs: Vec<_> = recommendations
        .iter()
        .skip(app.scroll_offset)
        .take((area.height as usize).saturating_sub(2))
        .collect();

    let items: Vec<ListItem> = visible_recs
        .iter()
        .enumerate()
        .map(|(i, rec)| {
            let is_selected = i + app.scroll_offset == app.selected_index;

            let priority_badge = Span::styled(
                format!("[{}]", rec.priority.label()),
                Style::default()
                    .fg(rec.priority.color())
                    .add_modifier(Modifier::BOLD),
            );

            let impact_badge = Span::styled(
                format!(
                    "Impact:{}",
                    match rec.impact {
                        Impact::High => "⬆️",
                        Impact::Medium => "➡️",
                        Impact::Low => "⬇️",
                    }
                ),
                Style::default().fg(rec.impact.color()),
            );

            let effort_badge = Span::styled(
                format!(
                    "Effort:{}",
                    match rec.effort {
                        Effort::High => "🔴",
                        Effort::Medium => "🟡",
                        Effort::Low => "🟢",
                    }
                ),
                Style::default().fg(rec.effort.color()),
            );

            let mut lines = vec![
                Line::from(vec![
                    if is_selected {
                        Span::styled("▶ ", Style::default().fg(ORANGE))
                    } else {
                        Span::raw("  ")
                    },
                    Span::raw(format!("{} ", rec.icon)),
                    priority_badge,
                    Span::raw(" "),
                    Span::styled(rec.category, Style::default().fg(LIGHT_ORANGE)),
                    Span::raw(": "),
                    Span::styled(
                        &rec.title,
                        Style::default().fg(TEXT_COLOR).add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("    "),
                    Span::styled(&rec.description, Style::default().fg(TEXT_COLOR)),
                ]),
                Line::from(vec![
                    Span::raw("    "),
                    impact_badge,
                    Span::raw("  "),
                    effort_badge,
                ]),
            ];

            if is_selected && !rec.steps.is_empty() {
                lines.push(Line::from(vec![Span::styled(
                    "    Steps:",
                    Style::default()
                        .fg(LIGHT_ORANGE)
                        .add_modifier(Modifier::BOLD),
                )]));
                for (idx, step) in rec.steps.iter().enumerate() {
                    lines.push(Line::from(vec![
                        Span::raw(format!("      {}. ", idx + 1)),
                        Span::styled(step, Style::default().fg(TEXT_COLOR)),
                    ]));
                }
            }

            lines.push(Line::from(""));

            ListItem::new(lines)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(Span::styled(
                format!(
                    "💡 Recommendations ({}/{})",
                    recommendations
                        .len()
                        .min(app.scroll_offset + (area.height as usize).saturating_sub(2)),
                    recommendations.len()
                ),
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR)),
    );

    f.render_widget(list, area);
}

fn draw_recommendations_footer(f: &mut Frame, area: Rect) {
    let footer_text = vec![Line::from(vec![
        Span::styled(
            "↑↓",
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        ),
        Span::raw(": Scroll │ "),
        Span::styled(
            "Enter",
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        ),
        Span::raw(": Expand │ "),
        Span::styled(
            "q",
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        ),
        Span::raw(": Quick Wins Filter │ "),
        Span::styled(
            "s",
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
        ),
        Span::raw(": Security Only"),
    ])];

    let footer = Paragraph::new(footer_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR)),
    );

    f.render_widget(footer, area);
}

fn generate_recommendations(app: &App) -> Vec<Recommendation> {
    let mut recommendations = Vec::new();

    // Security recommendations
    if !app.firewall.enabled {
        recommendations.push(Recommendation {
            icon: "🔥",
            category: "Security",
            priority: Priority::Critical,
            title: "Enable Firewall Protection".to_string(),
            description:
                "Firewall is currently disabled, leaving system exposed to network threats"
                    .to_string(),
            impact: Impact::High,
            effort: Effort::Low,
            steps: vec![
                "Install firewall package (ufw, firewalld, or iptables)".to_string(),
                "Configure default deny policy for incoming traffic".to_string(),
                "Allow only necessary ports (SSH, HTTP, HTTPS)".to_string(),
                "Enable firewall service on boot".to_string(),
                "Test firewall rules thoroughly".to_string(),
            ],
        });
    }

    if app.security.selinux == "disabled" && !app.security.apparmor {
        recommendations.push(Recommendation {
            icon: "🛡️",
            category: "Security",
            priority: Priority::High,
            title: "Enable Mandatory Access Control (MAC)".to_string(),
            description: "No MAC system (SELinux/AppArmor) is active, reducing security isolation"
                .to_string(),
            impact: Impact::High,
            effort: Effort::Medium,
            steps: vec![
                "Choose SELinux or AppArmor based on distribution".to_string(),
                "Install required packages".to_string(),
                "Configure in permissive mode first".to_string(),
                "Monitor audit logs for policy violations".to_string(),
                "Switch to enforcing mode after validation".to_string(),
            ],
        });
    }

    let privileged_users = app.users.iter().filter(|u| u.uid == "0").count();
    if privileged_users > 1 {
        recommendations.push(Recommendation {
            icon: "👥",
            category: "Security",
            priority: Priority::Critical,
            title: "Remove Non-Root UID 0 Accounts".to_string(),
            description: format!(
                "Found {} accounts with UID 0 - only root should have superuser privileges",
                privileged_users
            ),
            impact: Impact::High,
            effort: Effort::Low,
            steps: vec![
                "Identify all UID 0 accounts: getent passwd 0".to_string(),
                "Verify if additional accounts are legitimate".to_string(),
                "Remove or reassign UID for non-root accounts".to_string(),
                "Audit sudo access for affected users".to_string(),
                "Document changes for compliance".to_string(),
            ],
        });
    }

    // Performance recommendations
    if app.packages.package_count > 2000 {
        recommendations.push(Recommendation {
            icon: "📦",
            category: "Performance",
            priority: Priority::Medium,
            title: "Reduce Package Bloat".to_string(),
            description: format!(
                "{} packages installed - consider cleanup to improve performance",
                app.packages.package_count
            ),
            impact: Impact::Medium,
            effort: Effort::Medium,
            steps: vec![
                "List installed packages by size".to_string(),
                "Remove unused development packages".to_string(),
                "Clean up old kernels (keep latest 2-3)".to_string(),
                "Remove orphaned dependencies".to_string(),
                "Document essential packages for future reference".to_string(),
            ],
        });
    }

    let enabled_services = app.services.iter().filter(|s| s.enabled).count();
    if enabled_services > 50 {
        recommendations.push(Recommendation {
            icon: "⚙️",
            category: "Performance",
            priority: Priority::Medium,
            title: "Optimize Service Load".to_string(),
            description: format!(
                "{} services enabled - reducing unnecessary services improves boot time",
                enabled_services
            ),
            impact: Impact::Medium,
            effort: Effort::Low,
            steps: vec![
                "List all enabled services and their purposes".to_string(),
                "Identify services not critical for operation".to_string(),
                "Disable unused services: systemctl disable <service>".to_string(),
                "Test system functionality after changes".to_string(),
                "Monitor resource usage improvements".to_string(),
            ],
        });
    }

    // Maintenance recommendations
    if !app.databases.is_empty() {
        recommendations.push(Recommendation {
            icon: "🗄️",
            category: "Maintenance",
            priority: Priority::Low,
            title: "Regular Database Maintenance".to_string(),
            description: format!(
                "{} database(s) detected - schedule regular maintenance tasks",
                app.databases.len()
            ),
            impact: Impact::Medium,
            effort: Effort::Low,
            steps: vec![
                "Schedule automated backups".to_string(),
                "Set up log rotation for database logs".to_string(),
                "Configure VACUUM/OPTIMIZE schedules".to_string(),
                "Monitor database size and performance".to_string(),
                "Plan for capacity scaling".to_string(),
            ],
        });
    }

    if !app.web_servers.is_empty() {
        recommendations.push(Recommendation {
            icon: "🌐",
            category: "Security",
            priority: Priority::High,
            title: "Harden Web Server Configuration".to_string(),
            description: format!(
                "{} web server(s) detected - ensure security best practices",
                app.web_servers.len()
            ),
            impact: Impact::High,
            effort: Effort::Medium,
            steps: vec![
                "Disable unnecessary HTTP methods".to_string(),
                "Configure HTTPS with strong TLS versions".to_string(),
                "Set security headers (CSP, HSTS, X-Frame-Options)".to_string(),
                "Implement rate limiting".to_string(),
                "Regular security updates and patching".to_string(),
            ],
        });
    }

    // Compliance recommendations
    if app.packages.package_count > 0 {
        recommendations.push(Recommendation {
            icon: "📋",
            category: "Compliance",
            priority: Priority::Info,
            title: "Implement Change Management".to_string(),
            description: "Track system changes for compliance and audit purposes".to_string(),
            impact: Impact::Low,
            effort: Effort::Low,
            steps: vec![
                "Enable system auditing (auditd)".to_string(),
                "Configure change tracking for critical files".to_string(),
                "Set up centralized logging".to_string(),
                "Document standard operating procedures".to_string(),
                "Regular compliance reviews".to_string(),
            ],
        });
    }

    // Backup recommendations
    recommendations.push(Recommendation {
        icon: "💾",
        category: "Backup",
        priority: Priority::High,
        title: "Implement Backup Strategy".to_string(),
        description: "Ensure business continuity with proper backup procedures".to_string(),
        impact: Impact::High,
        effort: Effort::Medium,
        steps: vec![
            "Identify critical data and configurations".to_string(),
            "Choose backup solution (rsync, Bacula, Duplicity)".to_string(),
            "Implement 3-2-1 backup rule".to_string(),
            "Test restore procedures regularly".to_string(),
            "Document backup and restore processes".to_string(),
        ],
    });

    // Monitoring recommendations
    if enabled_services > 20 {
        recommendations.push(Recommendation {
            icon: "📊",
            category: "Monitoring",
            priority: Priority::Medium,
            title: "Set Up System Monitoring".to_string(),
            description: "Proactive monitoring prevents issues before they impact operations"
                .to_string(),
            impact: Impact::High,
            effort: Effort::Medium,
            steps: vec![
                "Install monitoring solution (Prometheus, Nagios, Zabbix)".to_string(),
                "Configure metrics collection for key services".to_string(),
                "Set up alerting thresholds".to_string(),
                "Create monitoring dashboards".to_string(),
                "Establish on-call procedures".to_string(),
            ],
        });
    }

    // Update management
    recommendations.push(Recommendation {
        icon: "🔄",
        category: "Maintenance",
        priority: Priority::Medium,
        title: "Establish Update Policy".to_string(),
        description: "Regular updates are critical for security and stability".to_string(),
        impact: Impact::High,
        effort: Effort::Low,
        steps: vec![
            "Enable automatic security updates".to_string(),
            "Schedule regular maintenance windows".to_string(),
            "Test updates in staging environment first".to_string(),
            "Keep rollback plan ready".to_string(),
            "Monitor security advisories".to_string(),
        ],
    });

    // Sort by priority
    recommendations.sort_by(|a, b| {
        let priority_order = |p: &Priority| match p {
            Priority::Critical => 0,
            Priority::High => 1,
            Priority::Medium => 2,
            Priority::Low => 3,
            Priority::Info => 4,
        };
        priority_order(&a.priority).cmp(&priority_order(&b.priority))
    });

    recommendations
}
