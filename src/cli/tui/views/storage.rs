// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Storage view - LVM, RAID, and mount points

use crate::cli::tui::app::App;
use crate::cli::tui::ui::{
    BORDER_COLOR, ERROR_COLOR, LIGHT_ORANGE, ORANGE, SUCCESS_COLOR, TEXT_COLOR, WARNING_COLOR,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame,
};

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // LVM summary
            Constraint::Length(6), // RAID arrays
            Constraint::Min(0),    // Fstab entries
        ])
        .split(area);

    draw_lvm_summary(f, chunks[0], app);
    draw_raid_summary(f, chunks[1], app);
    draw_fstab(f, chunks[2], app);
}

fn draw_lvm_summary(f: &mut Frame, area: Rect, app: &App) {
    let mut items = Vec::new();

    if let Some(ref lvm) = app.lvm_info {
        // Physical volumes
        items.push(ListItem::new(Line::from(vec![
            Span::styled("Physical Volumes: ", Style::default().fg(LIGHT_ORANGE)),
            Span::styled(
                format!("{}", lvm.physical_volumes.len()),
                Style::default().fg(TEXT_COLOR),
            ),
        ])));

        // Volume groups
        items.push(ListItem::new(Line::from(vec![
            Span::styled("Volume Groups:    ", Style::default().fg(LIGHT_ORANGE)),
            Span::styled(
                format!("{}", lvm.volume_groups.len()),
                Style::default().fg(TEXT_COLOR),
            ),
        ])));

        if !lvm.volume_groups.is_empty() {
            for vg in &lvm.volume_groups {
                items.push(ListItem::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(&vg.name, Style::default().fg(ORANGE)),
                    Span::raw(": "),
                    Span::styled(format!("{} ", vg.size), Style::default().fg(TEXT_COLOR)),
                    Span::styled(
                        format!("({} PV, {} LV)", vg.pv_count, vg.lv_count),
                        Style::default().fg(LIGHT_ORANGE),
                    ),
                ])));
            }
        }

        // Logical volumes
        items.push(ListItem::new(Line::from(vec![
            Span::styled("Logical Volumes:  ", Style::default().fg(LIGHT_ORANGE)),
            Span::styled(
                format!("{}", lvm.logical_volumes.len()),
                Style::default().fg(TEXT_COLOR),
            ),
        ])));
    } else {
        items.push(ListItem::new(Line::from(vec![Span::styled(
            "No LVM detected",
            Style::default().fg(TEXT_COLOR),
        )])));
    }

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR))
            .title(" 💾 LVM Configuration ")
            .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
    );

    f.render_widget(list, area);
}

fn draw_raid_summary(f: &mut Frame, area: Rect, app: &App) {
    if app.raid_arrays.is_empty() {
        let empty = Paragraph::new("No RAID arrays detected")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(BORDER_COLOR))
                    .title(" 🔧 RAID Arrays ")
                    .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().fg(TEXT_COLOR));
        f.render_widget(empty, area);
        return;
    }

    // Calculate RAID health metrics
    let total_arrays = app.raid_arrays.len();
    let healthy_arrays = app
        .raid_arrays
        .iter()
        .filter(|r| r.status == "active" && r.active_devices == r.total_devices)
        .count();
    let degraded_arrays = total_arrays - healthy_arrays;

    let health_pct = if total_arrays > 0 {
        (healthy_arrays as f64 / total_arrays as f64 * 100.0) as u16
    } else {
        100
    };

    // Split into gauge and list
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Health gauge
            Constraint::Min(0),    // RAID list
        ])
        .split(area);

    // RAID health gauge
    let gauge_color = if health_pct == 100 {
        SUCCESS_COLOR
    } else if health_pct >= 50 {
        WARNING_COLOR
    } else {
        ERROR_COLOR
    };

    let health_gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BORDER_COLOR))
                .title(" 🔧 RAID Array Health "),
        )
        .gauge_style(Style::default().fg(gauge_color))
        .percent(health_pct)
        .label(format!(
            "{}/{} healthy • {} degraded ({}%)",
            healthy_arrays, total_arrays, degraded_arrays, health_pct
        ));

    f.render_widget(health_gauge, chunks[0]);

    // RAID array list
    let items: Vec<ListItem> = app
        .raid_arrays
        .iter()
        .map(|raid| {
            let (status_icon, status_color) =
                if raid.status == "active" && raid.active_devices == raid.total_devices {
                    ("✓", SUCCESS_COLOR)
                } else if raid.active_devices < raid.total_devices {
                    ("⚠", ERROR_COLOR)
                } else {
                    ("●", WARNING_COLOR)
                };

            ListItem::new(Line::from(vec![
                Span::raw(format!("{} ", status_icon)),
                Span::styled(
                    &raid.device,
                    Style::default().fg(ORANGE).add_modifier(Modifier::BOLD),
                ),
                Span::raw(": "),
                Span::styled(&raid.level, Style::default().fg(LIGHT_ORANGE)),
                Span::raw(" - "),
                Span::styled(&raid.status, Style::default().fg(status_color)),
                Span::raw(" "),
                Span::styled(
                    format!("[{}/{}]", raid.active_devices, raid.total_devices),
                    Style::default().fg(status_color),
                ),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_COLOR))
            .title(format!(
                " 📋 RAID Arrays • {} configured ",
                app.raid_arrays.len()
            ))
            .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
    );

    f.render_widget(list, chunks[1]);
}

fn draw_fstab(f: &mut Frame, area: Rect, app: &App) {
    if app.fstab.is_empty() {
        let empty = Paragraph::new("⚠️  No fstab entries found")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(BORDER_COLOR))
                    .title(" 📁 Mount Points ")
                    .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
            )
            .style(Style::default().fg(TEXT_COLOR));
        f.render_widget(empty, area);
        return;
    }

    // Get sorted indices
    let sorted_indices = app.get_sorted_storage_indices();

    // Apply filtering if searching
    let filtered_indices: Vec<usize> = if app.is_searching() && !app.search_query.is_empty() {
        sorted_indices
            .into_iter()
            .filter(|&idx| {
                let (device, mountpoint, fstype) = &app.fstab[idx];
                device
                    .to_lowercase()
                    .contains(&app.search_query.to_lowercase())
                    || mountpoint
                        .to_lowercase()
                        .contains(&app.search_query.to_lowercase())
                    || fstype
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
        .map(|&idx| {
            let (device, mountpoint, fstype) = &app.fstab[idx];
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:25} ", device), Style::default().fg(LIGHT_ORANGE)),
                Span::raw("→ "),
                Span::styled(
                    format!("{:20} ", mountpoint),
                    Style::default().fg(TEXT_COLOR),
                ),
                Span::styled(format!("({})", fstype), Style::default().fg(ORANGE)),
            ]))
        })
        .collect();

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
                " 📁 Mount Points / fstab • {} showing of {} total{} ",
                filtered_indices.len(),
                app.fstab.len(),
                sort_indicator
            ))
            .title_style(Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)),
    );

    f.render_widget(list, area);
}
