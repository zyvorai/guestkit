// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! TUI (Terminal User Interface) module for interactive VM inspection

pub mod app;
pub mod app_load;
pub mod cache;
pub mod config;
pub mod events;
pub mod fleet;
pub mod icons;
pub mod loading;
pub mod palette;
pub mod plan_preview;
pub mod splash;
pub mod theme;
pub mod ui;
pub mod views;
pub mod widgets;

use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use indicatif::{ProgressBar, ProgressStyle};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

pub use app::App;

/// RAII guard to restore terminal state on drop (including panics)
struct TerminalGuard;

impl TerminalGuard {
    fn new() -> Result<Self> {
        enable_raw_mode().context("Failed to enable raw mode")?;
        execute!(io::stdout(), EnterAlternateScreen).context("Failed to enter alternate screen")?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

/// Run the TUI application
pub fn run_tui<P: AsRef<Path>>(
    image_path: P,
    compare_image: Option<&Path>,
    fleet_dir: Option<&Path>,
) -> Result<()> {
    let config = config::TuiConfig::load();

    // Setup terminal with RAII guard (restores on drop, including panics)
    let _guard = TerminalGuard::new()?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

    // Show splash screen if enabled
    if config.ui.show_splash {
        terminal.draw(|f| splash::draw_splash(f, &config.ui))?;
        std::thread::sleep(Duration::from_millis(config.ui.splash_duration_ms));
    }

    // Loading spinner — carbon accent (#FF7A00), ~120ms tick
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.rgb(255,122,0)} {msg:.rgb(125,133,144)}")
            .expect("valid spinner template")
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    spinner.set_message("Inspecting disk image…");
    spinner.enable_steady_tick(Duration::from_millis(120));

    let fleet_list = fleet::build_fleet_list(image_path.as_ref(), fleet_dir)?;
    let primary = fleet_list
        .first()
        .ok_or_else(|| anyhow::anyhow!("no disk image"))?
        .clone();
    let mut app = App::bootstrap(&primary, compare_image, fleet_list)?;
    if app.fleet_active() {
        app.show_notification(format!(
            "Fleet mode: {} images (N next, P previous)",
            app.fleet_images.len()
        ));
    }

    spinner.finish_and_clear();

    // Run the event loop
    let result = run_app(&mut terminal, &mut app);

    // Cleanup guestfs handle
    let _ = app.cleanup();

    // Show cursor before guard drops and restores terminal
    terminal.show_cursor().context("Failed to show cursor")?;

    result
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            match event::read()? {
                Event::Mouse(mouse) if app.config.ui.mouse_enabled => {
                    if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                        app.handle_mouse_click(mouse.column, mouse.row);
                    }
                }
                Event::Mouse(_) => {}
                Event::Key(key) => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        // Close file preview/info overlays first
                        if app.show_file_preview {
                            app.close_file_preview();
                        } else if app.show_file_info {
                            app.close_file_info();
                        } else if app.file_filtering {
                            // Cancel file filter and clear it
                            app.cancel_file_filter();
                        } else if app.show_plan_preview {
                            app.close_plan_preview();
                        } else if app.show_palette {
                            app.toggle_palette();
                        } else if app.show_jump_menu {
                            app.toggle_jump_menu();
                        } else if app.is_searching() {
                            app.cancel_search();
                        } else if app.is_exporting() {
                            app.cancel_export();
                        } else if app.show_export_menu {
                            app.toggle_export_menu();
                        } else if app.show_help {
                            app.toggle_help();
                        } else {
                            return Ok(());
                        }
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(());
                    }
                    KeyCode::Char('N') if app.fleet_active() && !app.is_searching() => {
                        let _ = app.fleet_next();
                    }
                    KeyCode::Char('P') if app.fleet_active() && !app.is_searching() => {
                        let _ = app.fleet_previous();
                    }
                    KeyCode::Char('p')
                        if key.modifiers.contains(KeyModifiers::CONTROL)
                            && app.config.keybindings.quick_jump_enabled =>
                    {
                        app.toggle_jump_menu();
                    }
                    KeyCode::Char('p')
                        if key.modifiers.contains(KeyModifiers::CONTROL)
                            && key.modifiers.contains(KeyModifiers::SHIFT) =>
                    {
                        app.start_global_search();
                    }
                    KeyCode::Char(':') if !app.is_searching() => {
                        app.toggle_palette();
                    }
                    KeyCode::Char('?') => app.toggle_context_help(),
                    KeyCode::Char('[') if !app.is_searching() => app.cycle_layout_mode_backward(),
                    KeyCode::Char(']') if !app.is_searching() => app.cycle_layout_mode(),
                    KeyCode::Char('{')
                        if !app.is_searching()
                            && !app.show_help
                            && !app.show_jump_menu
                            && !app.show_palette =>
                    {
                        app.previous_group();
                    }
                    KeyCode::Char('}')
                        if !app.is_searching()
                            && !app.show_help
                            && !app.show_jump_menu
                            && !app.show_palette =>
                    {
                        app.next_group();
                    }
                    KeyCode::Char('i')
                        if key.modifiers.contains(KeyModifiers::CONTROL) && app.is_searching() =>
                    {
                        app.toggle_case_sensitive();
                    }
                    KeyCode::Char('r')
                        if key.modifiers.contains(KeyModifiers::CONTROL) && app.is_searching() =>
                    {
                        app.toggle_regex_mode();
                    }
                    KeyCode::Tab => app.next_view(),
                    KeyCode::BackTab => app.previous_view(),
                    KeyCode::Char('h') | KeyCode::F(1) => app.toggle_help(),
                    KeyCode::Char('a')
                        if app.current_view == app::View::Dashboard && !app.is_searching() =>
                    {
                        app.go_to_assurance();
                    }
                    KeyCode::Char('p')
                        if app.current_view == app::View::Assurance
                            && app.migration_report.is_some()
                            && !app.is_searching() =>
                    {
                        app.toggle_plan_preview();
                    }
                    KeyCode::Char('p') if !app.show_plan_preview => {
                        app.current_view = app::View::Profiles;
                        app.scroll_offset = 0;
                    }
                    KeyCode::Char('d')
                        if app.current_view == app::View::Assurance
                            && !app.is_searching()
                            && !key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        let _ = app.load_assurance();
                    }
                    KeyCode::Char('t')
                        if app.current_view == app::View::Assurance && !app.is_searching() =>
                    {
                        app.cycle_assurance_target();
                    }
                    KeyCode::Char('e')
                        if app.current_view == app::View::Assurance
                            && app.migration_report.is_some()
                            && !app.is_searching() =>
                    {
                        match app.export_assurance_plan() {
                            Ok(msg) => app.show_notification(msg),
                            Err(e) => app.show_notification(format!("Export failed: {e:#}")),
                        }
                    }
                    KeyCode::Char('A')
                        if key.modifiers.contains(KeyModifiers::SHIFT)
                            && app.current_view == app::View::Assurance
                            && app.migration_report.is_some()
                            && !app.is_searching() =>
                    {
                        match app.apply_assurance_plan(false) {
                            Ok(msg) => app.show_notification(msg),
                            Err(e) => app.show_notification(format!("Apply failed: {e:#}")),
                        }
                    }
                    KeyCode::Char('e') => app.toggle_export_menu(),
                    KeyCode::Char(',')
                        if !app.is_searching()
                            && !app.show_help
                            && !app.show_jump_menu
                            && !app.show_palette =>
                    {
                        app.view_tab_scroll_left();
                    }
                    KeyCode::Char('.')
                        if !app.is_searching()
                            && !app.show_help
                            && !app.show_jump_menu
                            && !app.show_palette =>
                    {
                        app.view_tab_scroll_right();
                    }
                    KeyCode::Char('s') => app.cycle_sort_mode(),
                    KeyCode::Char('x')
                        if app.current_view == app::View::Files && !app.is_searching() =>
                    {
                        app.file_browser_extract();
                    }
                    KeyCode::Char('v')
                        if app.current_view == app::View::Files && !app.is_searching() =>
                    {
                        // View file preview in Files view
                        app.show_file_preview();
                    }
                    KeyCode::Char('i')
                        if app.current_view == app::View::Files && !app.is_searching() =>
                    {
                        // Show file info in Files view
                        app.show_file_information();
                    }
                    KeyCode::Char('i') => app.toggle_stats_bar(),
                    KeyCode::Char('t')
                        if !app.is_searching()
                            && !matches!(
                                app.export_mode,
                                Some(app::ExportMode::EnteringFilename)
                            ) =>
                    {
                        app.toggle_table_mode();
                    }
                    KeyCode::Char('c')
                        if !app.is_searching()
                            && !matches!(
                                app.export_mode,
                                Some(app::ExportMode::EnteringFilename)
                            ) =>
                    {
                        app.toggle_comparison_mode();
                    }
                    KeyCode::Char('m')
                        if !app.is_searching()
                            && !matches!(
                                app.export_mode,
                                Some(app::ExportMode::EnteringFilename)
                            ) =>
                    {
                        app.toggle_multi_select();
                    }
                    KeyCode::Char('f')
                        if app.current_view == app::View::Issues && !app.is_searching() =>
                    {
                        app.cycle_issue_filter();
                    }
                    KeyCode::Char('f')
                        if !app.is_searching()
                            && !matches!(
                                app.export_mode,
                                Some(app::ExportMode::EnteringFilename)
                            ) =>
                    {
                        app.cycle_filter();
                    }
                    KeyCode::Char('l')
                        if !app.is_searching()
                            && !matches!(
                                app.export_mode,
                                Some(app::ExportMode::EnteringFilename)
                            ) =>
                    {
                        app.toggle_live_filter();
                    }
                    KeyCode::Char('a')
                        if key.modifiers.contains(KeyModifiers::CONTROL) && !app.is_searching() =>
                    {
                        app.select_all_items();
                    }
                    KeyCode::Char(' ') if app.multi_select_mode && !app.is_searching() => {
                        app.toggle_item_selection();
                    }
                    KeyCode::Char('r')
                        if key.modifiers.contains(KeyModifiers::SHIFT) && !app.is_searching() =>
                    {
                        app.start_refresh(true);
                    }
                    KeyCode::Char('r')
                        if !app.is_searching()
                            && !matches!(
                                app.export_mode,
                                Some(app::ExportMode::EnteringFilename)
                            ) =>
                    {
                        app.start_refresh(false);
                    }
                    KeyCode::Char('m')
                        if key.modifiers.contains(KeyModifiers::CONTROL) && !app.is_searching() =>
                    {
                        let _ = app.export_migration_bundle();
                    }
                    KeyCode::Char('b') => {
                        // Bookmark current view
                        let bookmark = format!("{} view", app.current_view.title());
                        app.add_bookmark(bookmark);
                    }
                    KeyCode::Char('/') => {
                        if app.current_view == app::View::Files && !app.is_searching() {
                            // Start file filter in Files view
                            app.start_file_filter();
                        } else {
                            // Start search in other views
                            app.start_search();
                        }
                    }
                    KeyCode::Left if app.current_view == app::View::Profiles => {
                        app.previous_profile_tab();
                    }
                    KeyCode::Right if app.current_view == app::View::Profiles => {
                        app.next_profile_tab();
                    }
                    KeyCode::Up => {
                        if app.show_plan_preview {
                            app.plan_preview_scroll_up();
                        } else if app.show_palette {
                            if app.palette_selected > 0 {
                                app.palette_selected -= 1;
                            }
                        } else if app.global_search && app.is_searching() {
                            app.global_search_prev();
                        } else if app.show_jump_menu {
                            app.jump_menu_previous();
                        } else if app.show_help && !app.help_context {
                            app.help_scroll_up();
                        } else {
                            app.scroll_up();
                        }
                    }
                    KeyCode::Down => {
                        if app.show_plan_preview {
                            app.plan_preview_scroll_down();
                        } else if app.show_palette {
                            let n = crate::cli::tui::palette::filtered_commands(&app.palette_query)
                                .len();
                            if n > 0 {
                                app.palette_selected = (app.palette_selected + 1) % n;
                            }
                        } else if app.global_search && app.is_searching() {
                            app.global_search_next();
                        } else if app.show_jump_menu {
                            app.jump_menu_next();
                        } else if app.show_help && !app.help_context {
                            let visible = app.help_visible_lines();
                            app.help_scroll_down(app::App::FULL_HELP_LINES, visible);
                        } else {
                            app.scroll_down();
                        }
                    }
                    KeyCode::PageUp => {
                        if app.show_help && !app.help_context {
                            app.help_page_up(app.config.behavior.page_scroll_lines);
                        } else {
                            app.page_up();
                        }
                    }
                    KeyCode::PageDown => {
                        if app.show_help && !app.help_context {
                            let visible = app.help_visible_lines();
                            app.help_page_down(
                                app::App::FULL_HELP_LINES,
                                visible,
                                app.config.behavior.page_scroll_lines,
                            );
                        } else {
                            app.page_down();
                        }
                    }
                    KeyCode::Home => app.scroll_top(),
                    KeyCode::End => app.scroll_bottom(),
                    // Vim-style navigation
                    KeyCode::Char('k')
                        if app.config.keybindings.vim_mode
                            && !app.is_searching()
                            && !matches!(
                                app.export_mode,
                                Some(app::ExportMode::EnteringFilename)
                            ) =>
                    {
                        if app.show_plan_preview {
                            app.plan_preview_scroll_up();
                        } else if app.show_help && !app.help_context {
                            app.help_scroll_up();
                        } else {
                            app.scroll_up();
                        }
                    }
                    KeyCode::Char('j')
                        if app.config.keybindings.vim_mode
                            && !app.is_searching()
                            && !matches!(
                                app.export_mode,
                                Some(app::ExportMode::EnteringFilename)
                            ) =>
                    {
                        if app.show_plan_preview {
                            app.plan_preview_scroll_down();
                        } else if app.show_help && !app.help_context {
                            let visible = app.help_visible_lines();
                            app.help_scroll_down(app::App::FULL_HELP_LINES, visible);
                        } else {
                            app.scroll_down();
                        }
                    }
                    KeyCode::Char('g')
                        if !app.is_searching()
                            && !matches!(
                                app.export_mode,
                                Some(app::ExportMode::EnteringFilename)
                            ) =>
                    {
                        app.scroll_top()
                    }
                    KeyCode::Char('G')
                        if !app.is_searching()
                            && !matches!(
                                app.export_mode,
                                Some(app::ExportMode::EnteringFilename)
                            ) =>
                    {
                        app.scroll_bottom()
                    }
                    KeyCode::Char('u')
                        if key.modifiers.contains(KeyModifiers::CONTROL) && !app.is_searching() =>
                    {
                        app.page_up()
                    }
                    KeyCode::Char('d')
                        if key.modifiers.contains(KeyModifiers::CONTROL) && !app.is_searching() =>
                    {
                        app.page_down()
                    }
                    KeyCode::Enter => {
                        use app::ExportMode;

                        if app.global_search && app.is_searching() {
                            app.global_search_activate();
                        } else if app.show_palette {
                            app.palette_execute();
                        } else if app.show_jump_menu {
                            app.jump_menu_select();
                        } else if matches!(app.export_mode, Some(ExportMode::EnteringFilename)) {
                            let _ = app.execute_export();
                        } else if app.file_filtering {
                            // Finish file filter
                            app.finish_file_filter();
                        } else if app.current_view == app::View::Files && !app.is_searching() {
                            // Enter directory in Files view
                            app.file_browser_enter();
                        } else if !app.is_searching() && !app.show_export_menu {
                            app.toggle_detail();
                        } else {
                            app.select_item();
                        }
                    }
                    KeyCode::Char(c) => {
                        use app::{ExportFormat, ExportMode};

                        if app.show_palette {
                            app.palette_input(c);
                        } else if app.show_jump_menu {
                            app.jump_menu_input(c);
                        } else if matches!(app.export_mode, Some(ExportMode::Selecting)) {
                            // Handle format selection
                            match c {
                                '1' => app.select_export_format(ExportFormat::Json),
                                '2' => app.select_export_format(ExportFormat::Yaml),
                                '3' => app.select_export_format(ExportFormat::Html),
                                '4' => app.select_export_format(ExportFormat::Pdf),
                                _ => {}
                            }
                        } else if matches!(app.export_mode, Some(ExportMode::EnteringFilename)) {
                            app.export_input(c);
                        } else if app.file_filtering {
                            // Add character to file filter
                            app.file_filter_input_char(c);
                        } else if app.is_searching() {
                            app.search_input(c);
                        } else if app.current_view == app::View::Files && c == '.' {
                            // Toggle hidden files in Files view
                            app.file_browser_toggle_hidden();
                        } else if c.is_ascii_digit() {
                            // Quick jump to views with number keys 1-9
                            if let Some(digit) = c.to_digit(10) {
                                if digit > 0 {
                                    app.jump_to_view((digit - 1) as usize);
                                }
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        use app::ExportMode;

                        if app.show_palette {
                            app.palette_backspace();
                        } else if app.show_jump_menu {
                            app.jump_menu_backspace();
                        } else if matches!(app.export_mode, Some(ExportMode::EnteringFilename)) {
                            app.export_backspace();
                        } else if app.file_filtering {
                            // Remove character from file filter
                            app.file_filter_backspace();
                        } else if app.is_searching() {
                            app.search_backspace();
                        } else if app.current_view == app::View::Files {
                            // Go to parent directory in Files view
                            app.file_browser_go_up();
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }
    }
}
