// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Carbon control-plane palette with restrained orange accents (Zellij / k9s inspired).

use super::app::App;
use super::config::UiConfig;
use ratatui::{
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Gauge, Sparkline},
};

// ── Carbon (default) ───────────────────────────────────────────────────────────
pub const BG: Color = Color::Rgb(11, 14, 18);
pub const SURFACE: Color = Color::Rgb(17, 21, 27);
pub const SURFACE_RAISED: Color = Color::Rgb(22, 27, 34);
pub const ACCENT: Color = Color::Rgb(255, 122, 0);
pub const ACCENT_SOFT: Color = Color::Rgb(255, 158, 64);
pub const BORDER_MUTED: Color = Color::Rgb(42, 47, 56);
pub const TEXT: Color = Color::Rgb(220, 227, 234);
pub const TEXT_MUTED: Color = Color::Rgb(125, 133, 144);
pub const SUCCESS: Color = Color::Rgb(63, 185, 80);
pub const WARNING: Color = Color::Rgb(210, 153, 34);
pub const ERROR: Color = Color::Rgb(248, 81, 73);
pub const INFO: Color = TEXT_MUTED;
pub const SPARKLINE_MUTED: Color = Color::Rgb(60, 68, 78);
pub const TRACK: Color = BORDER_MUTED;
/// Warm tint for active group/view chips
pub const CHIP_ACTIVE: Color = Color::Rgb(42, 32, 22);
pub const CHIP_BG: Color = Color::Rgb(26, 31, 40);
pub const SELECTION: Color = Color::Rgb(36, 42, 54);
pub const DIM_OVERLAY: Color = Color::Rgb(5, 7, 10);
pub const LINK: Color = ACCENT_SOFT;

// Legacy aliases
pub const BG_COLOR: Color = BG;
pub const ORANGE: Color = ACCENT;
pub const LIGHT_ORANGE: Color = ACCENT_SOFT;
pub const DARK_ORANGE: Color = BORDER_MUTED;
pub const BORDER_COLOR: Color = BORDER_MUTED;
pub const TEXT_COLOR: Color = TEXT;
pub const SUCCESS_COLOR: Color = SUCCESS;
pub const WARNING_COLOR: Color = WARNING;
pub const ERROR_COLOR: Color = ERROR;
pub const INFO_COLOR: Color = INFO;

/// Resolved palette for active theme.
#[derive(Clone, Copy)]
pub struct UiPalette {
    pub bg: Color,
    pub surface: Color,
    pub surface_raised: Color,
    pub accent: Color,
    pub accent_soft: Color,
    pub border: Color,
    pub text: Color,
    pub text_muted: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
}

impl UiPalette {
    pub const CARBON: Self = Self {
        bg: BG,
        surface: SURFACE,
        surface_raised: SURFACE_RAISED,
        accent: ACCENT,
        accent_soft: ACCENT_SOFT,
        border: BORDER_MUTED,
        text: TEXT,
        text_muted: TEXT_MUTED,
        success: SUCCESS,
        warning: WARNING,
        error: ERROR,
    };

    pub const HIGH_CONTRAST: Self = Self {
        bg: Color::Rgb(0, 0, 0),
        surface: Color::Rgb(18, 18, 18),
        surface_raised: Color::Rgb(32, 32, 32),
        accent: Color::Rgb(255, 140, 0),
        accent_soft: Color::Rgb(255, 180, 80),
        border: Color::Rgb(100, 100, 100),
        text: Color::Rgb(240, 240, 240),
        text_muted: Color::Rgb(180, 180, 180),
        success: Color::Rgb(80, 220, 100),
        warning: Color::Rgb(255, 200, 50),
        error: Color::Rgb(255, 90, 90),
    };

    pub const MINIMAL: Self = Self {
        bg: Color::Rgb(8, 10, 12),
        surface: BG,
        surface_raised: SURFACE,
        accent: ACCENT,
        accent_soft: ACCENT_SOFT,
        border: Color::Rgb(35, 40, 48),
        text: TEXT,
        text_muted: TEXT_MUTED,
        success: SUCCESS,
        warning: WARNING,
        error: ERROR,
    };
}

pub fn for_config(cfg: &UiConfig) -> UiPalette {
    match cfg.theme.as_str() {
        "high-contrast" | "high_contrast" => UiPalette::HIGH_CONTRAST,
        "minimal" => UiPalette::MINIMAL,
        _ => UiPalette::CARBON,
    }
}

pub fn for_app(app: &App) -> UiPalette {
    for_config(&app.config.ui)
}

/// Runtime colors (glass / transparency applied from config).
#[derive(Debug, Clone, Copy)]
pub struct ResolvedTheme {
    pub transparent: bool,
    pub bg: Color,
    pub surface: Color,
    pub surface_raised: Color,
    pub chip_bg: Color,
    pub chip_active: Color,
    pub selection: Color,
    pub dim_overlay: Color,
    pub tab_strip_bg: Color,
}

impl ResolvedTheme {
    pub fn opaque() -> Self {
        Self {
            transparent: false,
            bg: BG,
            surface: SURFACE,
            surface_raised: SURFACE_RAISED,
            chip_bg: CHIP_BG,
            chip_active: CHIP_ACTIVE,
            selection: SELECTION,
            dim_overlay: DIM_OVERLAY,
            tab_strip_bg: BG,
        }
    }
}

/// Blend `color` toward the terminal backdrop (assumed dark) for a glass pane look.
fn glass_color(color: Color, opacity_pct: u8) -> Color {
    let Color::Rgb(r, g, b) = color else {
        return color;
    };
    let a = (opacity_pct as f32 / 100.0).clamp(0.35, 1.0);
    Color::Rgb(
        (r as f32 * a) as u8,
        (g as f32 * a) as u8,
        (b as f32 * a) as u8,
    )
}

pub fn resolve(cfg: &UiConfig) -> ResolvedTheme {
    let use_glass =
        cfg.transparent && !matches!(cfg.theme.as_str(), "high-contrast" | "high_contrast");
    if !use_glass {
        return ResolvedTheme::opaque();
    }
    let op = cfg.glass_opacity.clamp(40, 100);
    let dim_op = (op as u16 * 55 / 100).max(35) as u8;
    ResolvedTheme {
        transparent: true,
        bg: Color::Reset,
        surface: glass_color(SURFACE, op),
        surface_raised: glass_color(SURFACE_RAISED, op.saturating_add(4).min(100)),
        chip_bg: glass_color(CHIP_BG, op.saturating_sub(6)),
        chip_active: glass_color(CHIP_ACTIVE, op.saturating_sub(10).max(50)),
        selection: glass_color(SELECTION, op),
        dim_overlay: glass_color(DIM_OVERLAY, dim_op),
        tab_strip_bg: Color::Reset,
    }
}

pub fn use_emoji(cfg: &UiConfig) -> bool {
    cfg.show_emoji && cfg.icon_mode != "ascii"
}

pub fn list_row_height(cfg: &UiConfig) -> u16 {
    if cfg.density == "compact" {
        1
    } else {
        2
    }
}

pub fn fill_background(t: &ResolvedTheme) -> Block<'static> {
    if t.transparent {
        Block::default()
    } else {
        Block::default().style(Style::default().bg(t.bg))
    }
}

pub fn pane_block<'a>(title: &'a str, focused: bool, t: &ResolvedTheme) -> Block<'a> {
    let border_color = if focused { ACCENT } else { BORDER_MUTED };
    let title_color = if focused { ACCENT } else { TEXT_MUTED };
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(format!(" {title} "))
        .title_style(Style::default().fg(title_color))
        .style(Style::default().bg(t.surface))
}

pub fn content_block<'a>(title: &'a str, t: &ResolvedTheme) -> Block<'a> {
    pane_block(title, false, t)
}

pub fn risk_border_color(critical: usize, high: usize, medium: usize) -> Color {
    if critical > 0 {
        ERROR
    } else if high > 0 {
        ACCENT
    } else if medium > 0 {
        WARNING
    } else {
        BORDER_MUTED
    }
}

pub fn key_primary() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn key_muted() -> Style {
    Style::default().fg(TEXT_MUTED)
}

pub fn pane_block_with_border(title: &str, border: Color) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border))
        .title(format!(" {title} "))
        .title_style(Style::default().fg(TEXT_MUTED))
        .style(Style::default().bg(SURFACE))
}

pub fn label_style() -> Style {
    Style::default().fg(TEXT_MUTED)
}

pub fn value_style() -> Style {
    Style::default().fg(TEXT).add_modifier(Modifier::BOLD)
}

pub fn focus_style(t: &ResolvedTheme) -> Style {
    Style::default()
        .fg(ACCENT)
        .bg(t.selection)
        .add_modifier(Modifier::BOLD)
}

/// Active group tab chip (Overview · System · Security).
pub fn group_tab_span(name: &str, active: bool, t: &ResolvedTheme) -> ratatui::text::Span<'static> {
    use ratatui::text::Span;
    if active {
        Span::styled(
            format!(" ┃ {name} ┃ "),
            Style::default()
                .fg(ACCENT)
                .bg(t.chip_active)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            format!("  {name}  "),
            Style::default().fg(TEXT_MUTED).bg(t.chip_bg),
        )
    }
}

/// View tab label; pinned tabs use a soft accent marker.
pub fn view_tab_span(
    label: &str,
    active: bool,
    pinned: bool,
    t: &ResolvedTheme,
) -> ratatui::text::Span<'static> {
    use ratatui::text::Span;
    if active {
        Span::styled(
            format!("▸ {label} "),
            focus_style(t).add_modifier(Modifier::UNDERLINED),
        )
    } else if pinned {
        Span::styled(
            format!(" {label} "),
            Style::default().fg(LINK).bg(t.chip_bg),
        )
    } else {
        Span::styled(format!(" {label} "), Style::default().fg(TEXT_MUTED))
    }
}

pub fn chrome_header_block<'a>(title: &'a str, border: Color, t: &ResolvedTheme) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border))
        .title(format!(" {title} "))
        .title_style(Style::default().fg(LINK).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(t.surface_raised))
}

pub fn chrome_footer_style(t: &ResolvedTheme) -> Style {
    if t.transparent {
        Style::default().fg(TEXT_MUTED)
    } else {
        Style::default().bg(t.surface).fg(TEXT_MUTED)
    }
}

pub fn modal_block<'a>(title: &'a str, t: &ResolvedTheme) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT))
        .title(format!(" {title} "))
        .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(t.surface_raised))
}

pub fn toast_block(t: &ResolvedTheme) -> Block<'static> {
    Block::default()
        .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
        .border_style(Style::default().fg(ACCENT))
        .title(" ◆ ")
        .title_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(t.surface_raised))
}

pub fn tab_strip_block<'a>(title: &'a str, t: &ResolvedTheme) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_MUTED))
        .title(format!(" {title} "))
        .title_style(Style::default().fg(TEXT_MUTED))
        .style(Style::default().bg(t.tab_strip_bg))
}

pub fn gauge_block<'a>(title: &'a str, t: &ResolvedTheme) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_MUTED))
        .title(format!(" {title} "))
        .title_style(Style::default().fg(TEXT_MUTED))
        .style(Style::default().bg(t.surface))
}

pub fn gauge_widget<'a>(
    title: &'a str,
    percent: u16,
    label: &str,
    fill: Color,
    t: &ResolvedTheme,
) -> Gauge<'a> {
    Gauge::default()
        .block(gauge_block(title, t))
        .gauge_style(Style::default().fg(fill))
        .percent(percent.min(100))
        .label(label.to_string())
}

pub fn sparkline_block<'a>(title: &'a str, t: &ResolvedTheme) -> Block<'a> {
    gauge_block(title, t)
}

pub fn sparkline_widget<'a>(
    title: &'a str,
    data: &'a [u64],
    line_color: Color,
    t: &ResolvedTheme,
) -> Sparkline<'a> {
    Sparkline::default()
        .block(sparkline_block(title, t))
        .data(data)
        .style(Style::default().fg(line_color))
}

pub fn glow_error() -> Style {
    Style::default().fg(Color::Rgb(180, 60, 55))
}

pub fn glow_warning() -> Style {
    Style::default().fg(Color::Rgb(160, 110, 40))
}
