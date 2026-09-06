// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Issues view - Aggregated security findings and recommendations

use crate::cli::profiles::RiskLevel;
use crate::cli::tui::app::{App, IssueRiskFilter, LayoutMode};
use crate::cli::tui::theme::{
    self, content_block, label_style, ACCENT, ERROR, SUCCESS, TEXT, WARNING,
};
use crate::cli::tui::widgets::{self, list_line_spans};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
    Frame,
};

struct IssueRow {
    risk: Option<RiskLevel>,
    section: String,
    item: String,
    message: String,
    remediation: String,
}

impl App {
    fn collect_issues(&self) -> Vec<IssueRow> {
        let mut issues = Vec::new();

        let mut push_profile = |tag: &str, profile: &crate::cli::profiles::ProfileReport| {
            for section in &profile.sections {
                for finding in &section.findings {
                    issues.push(IssueRow {
                        risk: finding.risk_level,
                        section: if tag.is_empty() {
                            section.title.clone()
                        } else {
                            format!("{} [{}]", section.title, tag)
                        },
                        item: finding.item.clone(),
                        message: finding.message.clone(),
                        remediation: format!("Review and remediate: {}", finding.item),
                    });
                }
            }
        };

        if let Some(ref p) = self.security_profile {
            push_profile("", p);
        }
        if let Some(ref p) = self.hardening_profile {
            push_profile("Hardening", p);
        }
        if let Some(ref p) = self.compliance_profile {
            push_profile("Compliance", p);
        }

        if &self.security.selinux == "disabled" {
            issues.push(IssueRow {
                risk: Some(RiskLevel::High),
                section: "Security".to_string(),
                item: "SELinux disabled".to_string(),
                message: "SELinux is not enforcing".to_string(),
                remediation: "Enable SELinux in /etc/selinux/config".to_string(),
            });
        }
        if !self.firewall.enabled {
            issues.push(IssueRow {
                risk: Some(RiskLevel::Critical),
                section: "Firewall".to_string(),
                item: "Firewall off".to_string(),
                message: "No host firewall detected".to_string(),
                remediation: "Enable firewalld/ufw and define default-deny rules".to_string(),
            });
        }
        if !self.security.auditd {
            issues.push(IssueRow {
                risk: Some(RiskLevel::Medium),
                section: "Auditing".to_string(),
                item: "auditd inactive".to_string(),
                message: "auditd not running".to_string(),
                remediation: "systemctl enable --now auditd".to_string(),
            });
        }

        issues
    }

    fn issue_matches_filter(&self, risk: Option<RiskLevel>) -> bool {
        match self.issue_filter {
            IssueRiskFilter::All => true,
            IssueRiskFilter::Critical => matches!(risk, Some(RiskLevel::Critical)),
            IssueRiskFilter::High => matches!(risk, Some(RiskLevel::High)),
            IssueRiskFilter::Medium => matches!(risk, Some(RiskLevel::Medium)),
        }
    }
}

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(14), Constraint::Min(0)])
        .split(area);

    draw_summary(f, chunks[0], app);

    match app.layout_mode {
        LayoutMode::ListOnly => draw_issues_list(f, chunks[1], app),
        LayoutMode::DetailFull => draw_issue_detail(f, chunks[1], app),
        LayoutMode::SplitDetail => {
            let split = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                .split(chunks[1]);
            draw_issues_list(f, split[0], app);
            draw_issue_detail(f, split[1], app);
        }
    }
}

fn draw_summary(f: &mut Frame, area: Rect, app: &App) {
    let (critical, high, medium) = app.get_risk_summary();
    let total_issues = critical + high + medium;

    let overall_status = if critical > 0 {
        ("CRITICAL", ERROR)
    } else if high > 0 {
        ("HIGH RISK", ACCENT)
    } else if medium > 0 {
        ("MEDIUM", WARNING)
    } else {
        ("HEALTHY", SUCCESS)
    };

    let filter_label = match app.issue_filter {
        IssueRiskFilter::All => "all",
        IssueRiskFilter::Critical => "critical",
        IssueRiskFilter::High => "high",
        IssueRiskFilter::Medium => "medium",
    };

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    let (donut, donut_color) = widgets::risk_donut_ascii(critical, high, medium, 18);
    let summary_lines = vec![
        Line::from(vec![
            Span::styled("Status ", label_style()),
            Span::styled(
                overall_status.0,
                Style::default()
                    .fg(overall_status.1)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  filter ", label_style()),
            Span::styled(filter_label, Style::default().fg(ACCENT)),
            Span::styled(" (f)", label_style()),
        ]),
        Line::from(vec![
            Span::styled(donut, Style::default().fg(donut_color)),
            Span::raw("  "),
            Span::styled(format!("{total_issues} findings"), theme::value_style()),
        ]),
        Line::from(vec![
            Span::styled(format!("C:{critical} "), Style::default().fg(ERROR)),
            Span::styled(format!("H:{high} "), Style::default().fg(ACCENT)),
            Span::styled(format!("M:{medium}"), Style::default().fg(WARNING)),
        ]),
    ];

    f.render_widget(
        Paragraph::new(summary_lines).block(content_block("Issues summary", app.theme())),
        chunks[0],
    );

    let row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(chunks[1]);

    let pct = |n: usize| -> u16 {
        if total_issues == 0 {
            0
        } else {
            (n as f64 / total_issues as f64 * 100.0) as u16
        }
    };

    for (i, (label, n, color)) in [
        ("Critical", critical, ERROR),
        ("High", high, ACCENT),
        ("Medium", medium, WARNING),
    ]
    .iter()
    .enumerate()
    {
        let g = theme::gauge_widget(label, pct(*n), &format!("{n}"), *color, app.theme());
        f.render_widget(g, row[i]);
    }
}

fn draw_issues_list(f: &mut Frame, area: Rect, app: &App) {
    let all = app.collect_issues();
    let filtered: Vec<&IssueRow> = all
        .iter()
        .filter(|row| app.issue_matches_filter(row.risk))
        .filter(|row| {
            if app.is_searching() && !app.search_query.is_empty() {
                let q = app.search_query.to_lowercase();
                row.item.to_lowercase().contains(&q)
                    || row.message.to_lowercase().contains(&q)
                    || row.section.to_lowercase().contains(&q)
            } else {
                true
            }
        })
        .collect();

    let row_h = theme::list_row_height(&app.config.ui) as usize;
    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(idx, row)| {
            let sev = match row.risk {
                Some(RiskLevel::Critical) => Some('C'),
                Some(RiskLevel::High) => Some('H'),
                Some(RiskLevel::Medium) => Some('M'),
                Some(RiskLevel::Low) => Some('L'),
                _ => None,
            };
            let selected = idx == app.selected_index;
            let mut lines = vec![list_line_spans(
                selected,
                sev,
                vec![
                    Span::styled(&row.section, Style::default().fg(TEXT)),
                    Span::raw(" · "),
                    Span::styled(&row.item, Style::default().fg(TEXT)),
                ],
                app.theme(),
            )];
            if row_h > 1 {
                lines.push(Line::from(vec![
                    Span::raw(" "),
                    Span::styled(&row.message, theme::label_style()),
                ]));
            }
            ListItem::new(lines)
        })
        .skip(app.scroll_offset)
        .take(area.height.saturating_sub(2) as usize)
        .collect();

    let list = List::new(items).block(content_block("Findings", app.theme()));
    f.render_widget(list, area);
}

fn draw_issue_detail(f: &mut Frame, area: Rect, app: &App) {
    let all = app.collect_issues();
    let filtered: Vec<&IssueRow> = all
        .iter()
        .filter(|row| app.issue_matches_filter(row.risk))
        .collect();

    let detail = if app.selected_index < filtered.len() {
        let row = filtered[app.selected_index];
        vec![
            Line::from(vec![
                Span::styled("Item: ", label_style()),
                Span::styled(
                    &row.item,
                    Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Finding: ", label_style()),
                Span::raw(&row.message),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Remediation: ", label_style()),
                Span::styled(&row.remediation, Style::default().fg(SUCCESS)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Copy fix: ", label_style()),
                Span::styled(
                    format!(
                        "# {} --profile security {}",
                        crate::cli::invocation::example("inspect"),
                        app.image_path
                    ),
                    Style::default().fg(theme::INFO),
                ),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            "Select an issue for remediation details",
            label_style(),
        ))]
    };

    f.render_widget(
        Paragraph::new(detail)
            .block(content_block("Detail", app.theme()))
            .wrap(ratatui::widgets::Wrap { trim: true }),
        area,
    );
}
