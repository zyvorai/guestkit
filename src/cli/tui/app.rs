// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! TUI application state management

use crate::guestfs::inspect_enhanced::{
    Database, FirewallInfo, HostEntry, LVMInfo, NetworkInterface, Package, PackageInfo, RAIDArray,
    SecurityInfo, SystemService, UserAccount, WebServer,
};
use crate::Guestfs;
use anyhow::Result;
use chrono::{DateTime, Local};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::loading::LoadingState;

use super::config::TuiConfig;
use crate::cli::profiles::ProfileReport;

/// Format file size to human-readable format
fn format_file_size(size: i64) -> String {
    crate::cli::output::format_size(if size < 0 { 0 } else { size as u64 })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Yaml,
    Html,
    Pdf,
}

impl ExportFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::Json => "json",
            ExportFormat::Yaml => "yaml",
            ExportFormat::Html => "html",
            ExportFormat::Pdf => "pdf",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            ExportFormat::Json => "JSON",
            ExportFormat::Yaml => "YAML",
            ExportFormat::Html => "HTML",
            ExportFormat::Pdf => "PDF",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportMode {
    Selecting,
    EnteringFilename,
    Exporting,
    Success(String), // filename
    Error(String),   // error message
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum View {
    Dashboard,
    Analytics,
    Timeline,
    Recommendations,
    Topology,
    Network,
    Packages,
    Services,
    Databases,
    WebServers,
    Security,
    Issues,
    Storage,
    Users,
    Kernel,
    Logs,
    Profiles,
    Assurance,
    SystemdDeep,
    AiInsights,
    Files,
}

/// Row in the grouped jump menu (Ctrl+P).
#[derive(Debug, Clone)]
pub enum JumpMenuRow {
    Header(String),
    Item {
        group: String,
        view: View,
        title: String,
    },
}

/// Layout preset per view (`[` / `]` to cycle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    ListOnly,
    SplitDetail,
    DetailFull,
}

/// Issues list risk filter (`f` in Issues view).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueRiskFilter {
    All,
    Critical,
    High,
    Medium,
}

/// Cross-view search hit (Ctrl+Shift+P).
#[derive(Debug, Clone)]
pub struct GlobalSearchHit {
    pub view: View,
    pub index: usize,
    pub label: String,
}

/// Summary of `--compare` second image.
#[derive(Debug, Clone)]
pub struct CompareSummary {
    pub path: String,
    pub os_name: String,
    pub hostname: String,
    pub package_count: usize,
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
}

impl View {
    pub fn title(&self) -> &str {
        match self {
            View::Dashboard => "Dashboard",
            View::Analytics => "Analytics",
            View::Timeline => "Timeline",
            View::Recommendations => "Recommendations",
            View::Topology => "Topology",
            View::Network => "Network",
            View::Packages => "Packages",
            View::Services => "Services",
            View::Databases => "Databases",
            View::WebServers => "WebServers",
            View::Security => "Security",
            View::Issues => "Issues",
            View::Storage => "Storage",
            View::Users => "Users",
            View::Kernel => "Kernel",
            View::Logs => "Logs",
            View::Profiles => "Profiles",
            View::Assurance => "Assurance",
            View::SystemdDeep => "SystemdDeep",
            View::AiInsights => "AiInsights",
            View::Files => "Files",
        }
    }

    pub fn all() -> &'static [View] {
        &[
            View::Dashboard,
            View::Analytics,
            View::Timeline,
            View::Recommendations,
            View::Topology,
            View::Network,
            View::Packages,
            View::Services,
            View::Databases,
            View::WebServers,
            View::Security,
            View::Issues,
            View::Storage,
            View::Users,
            View::Kernel,
            View::Logs,
            View::Profiles,
            View::Assurance,
            View::SystemdDeep,
            View::AiInsights,
            View::Files,
        ]
    }
}

/// Sort order for lists
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Default,
    NameAsc,
    NameDesc,
    VersionAsc, // For packages
    VersionDesc,
    SizeAsc, // For storage
    SizeDesc,
    StateAsc, // For services
    StateDesc,
    UidAsc, // For users
    UidDesc,
    EnabledFirst, // For services
}

impl SortMode {
    /// Get next sort mode for a specific view
    pub fn next(&self, view: &View) -> Self {
        match view {
            View::Packages => match self {
                SortMode::Default => SortMode::NameAsc,
                SortMode::NameAsc => SortMode::NameDesc,
                SortMode::NameDesc => SortMode::VersionAsc,
                SortMode::VersionAsc => SortMode::VersionDesc,
                SortMode::VersionDesc => SortMode::Default,
                _ => SortMode::Default,
            },
            View::Services => match self {
                SortMode::Default => SortMode::NameAsc,
                SortMode::NameAsc => SortMode::NameDesc,
                SortMode::NameDesc => SortMode::StateAsc,
                SortMode::StateAsc => SortMode::StateDesc,
                SortMode::StateDesc => SortMode::EnabledFirst,
                SortMode::EnabledFirst => SortMode::Default,
                _ => SortMode::Default,
            },
            View::Users => match self {
                SortMode::Default => SortMode::NameAsc,
                SortMode::NameAsc => SortMode::NameDesc,
                SortMode::NameDesc => SortMode::UidAsc,
                SortMode::UidAsc => SortMode::UidDesc,
                SortMode::UidDesc => SortMode::Default,
                _ => SortMode::Default,
            },
            View::Storage => match self {
                SortMode::Default => SortMode::NameAsc,
                SortMode::NameAsc => SortMode::NameDesc,
                SortMode::NameDesc => SortMode::SizeAsc,
                SortMode::SizeAsc => SortMode::SizeDesc,
                SortMode::SizeDesc => SortMode::Default,
                _ => SortMode::Default,
            },
            // Other views use simple name sorting
            _ => match self {
                SortMode::Default => SortMode::NameAsc,
                SortMode::NameAsc => SortMode::NameDesc,
                SortMode::NameDesc => SortMode::Default,
                _ => SortMode::Default,
            },
        }
    }

    pub fn label(&self) -> &str {
        match self {
            SortMode::Default => "Default",
            SortMode::NameAsc => "Name ↑",
            SortMode::NameDesc => "Name ↓",
            SortMode::VersionAsc => "Version ↑",
            SortMode::VersionDesc => "Version ↓",
            SortMode::SizeAsc => "Size ↑",
            SortMode::SizeDesc => "Size ↓",
            SortMode::StateAsc => "State ↑",
            SortMode::StateDesc => "State ↓",
            SortMode::UidAsc => "UID ↑",
            SortMode::UidDesc => "UID ↓",
            SortMode::EnabledFirst => "Enabled 1st",
        }
    }
}

pub struct App {
    pub current_view: View,
    pub show_help: bool,
    pub searching: bool,
    pub search_query: String,
    pub search_case_sensitive: bool,
    pub search_regex_mode: bool,
    pub search_results: Vec<usize>, // Filtered item indices
    pub live_filter_enabled: bool,

    // Multi-select state
    pub multi_select_mode: bool,
    pub selected_items: HashSet<usize>, // Set of selected indices
    pub select_all: bool,

    // File preview state
    pub show_file_preview: bool,
    pub file_preview_content: String,
    pub file_preview_path: String,
    pub show_file_info: bool,
    pub file_info_content: String,

    // File filter state
    pub file_filtering: bool,
    pub file_filter_input: String,

    // Quick filters
    pub active_filter: Option<String>,
    pub available_filters: Vec<String>,
    pub scroll_offset: usize,
    pub selected_index: usize,
    pub show_export_menu: bool,
    pub selected_profile_tab: usize,
    pub show_detail: bool,
    pub sort_mode: SortMode,
    pub show_stats_bar: bool,
    pub table_mode: bool,      // Toggle between list and table view
    pub comparison_mode: bool, // Toggle comparison/diff view
    pub snapshot_packages: Option<Vec<Package>>, // Snapshot for comparison
    pub snapshot_services: Option<Vec<SystemService>>,
    pub bookmarks: Vec<String>,
    pub search_history: Vec<String>,
    pub notification: Option<(String, u8)>, // (message, ticks_remaining)
    pub last_updated: DateTime<Local>,
    pub refreshing: bool,

    // Jump menu state
    pub show_jump_menu: bool,
    pub jump_query: String,
    pub jump_selected_index: usize,
    pub jump_scroll_offset: usize,

    /// Scroll offset for full help overlay (`h`)
    pub help_scroll: usize,

    /// Last known terminal size (updated each frame)
    pub terminal_width: u16,
    pub terminal_height: u16,

    // Export state
    pub export_mode: Option<ExportMode>,
    pub export_format: Option<ExportFormat>,
    pub export_filename: String,

    // Inspection data
    pub image_path: String,
    pub _image_path_buf: PathBuf,
    pub os_name: String,
    pub os_version: String,
    pub hostname: String,
    pub kernel_version: String,
    pub architecture: String,
    pub init_system: String,
    pub timezone: String,
    pub locale: String,

    pub network_interfaces: Vec<NetworkInterface>,
    pub dns_servers: Vec<String>,

    pub packages: PackageInfo,
    pub services: Vec<SystemService>,
    pub databases: Vec<Database>,
    pub web_servers: Vec<WebServer>,
    pub firewall: FirewallInfo,
    pub security: SecurityInfo,
    pub users: Vec<UserAccount>,

    pub _hosts: Vec<HostEntry>,
    pub fstab: Vec<(String, String, String)>,
    pub lvm_info: Option<LVMInfo>,
    pub raid_arrays: Vec<RAIDArray>,

    // Kernel configuration
    pub kernel_modules: Vec<String>,
    pub kernel_params: HashMap<String, String>,

    // Profile reports
    pub security_profile: Option<ProfileReport>,
    pub migration_profile: Option<ProfileReport>,
    pub performance_profile: Option<ProfileReport>,
    pub compliance_profile: Option<ProfileReport>,
    pub hardening_profile: Option<ProfileReport>,

    // Configuration
    #[allow(dead_code)]
    pub config: TuiConfig,

    // File browser state
    pub file_browser: Option<crate::cli::tui::views::files::FileBrowserState>,

    // Guestfs handle for file operations (kept alive for Files view)
    pub guestfs: Option<Guestfs>,

    // Progressive load & refresh
    pub inspect_root: String,
    pub loading: Option<LoadingState>,
    pub loaded_views: HashSet<View>,
    pub compare_image_path: Option<PathBuf>,
    pub compare_summary: Option<CompareSummary>,

    // UX enhancements
    pub layout_mode: LayoutMode,
    pub show_palette: bool,
    pub palette_query: String,
    pub palette_selected: usize,
    pub global_search: bool,
    pub global_search_hits: Vec<GlobalSearchHit>,
    pub global_search_selected: usize,
    pub issue_filter: IssueRiskFilter,
    pub issue_detail_text: Option<String>,
    pub pinned_views: Vec<String>,
    pub help_context: bool,
    pub last_auto_refresh: Instant,

    /// Fleet mode: multiple images (`N` / `P` to switch)
    pub fleet_images: Vec<String>,
    pub fleet_index: usize,

    /// Migration assurance (doctor / migrate-plan)
    pub boot_report: Option<crate::boot::BootabilityReport>,
    pub migration_report: Option<crate::cli::migrate::plan::MigrationScoreReport>,
    pub assurance_evidence: Option<crate::evidence::EvidenceSnapshot>,
    pub intelligence: Option<crate::ai::IntelligenceBundle>,
    pub assurance_target: String,
    /// True when live guest agent responds (GUESTKIT_AGENT_SOCKET)
    pub agent_live: bool,
    /// First visible tab index in the view tab row
    pub view_tab_scroll: usize,

    /// Read-only fix plan preview (from migration assurance)
    pub show_plan_preview: bool,
    pub plan_preview: Option<crate::cli::plan::types::FixPlan>,
    pub plan_preview_scroll: usize,

    /// Resolved colors (glass / transparency from `tui.toml`)
    pub resolved_theme: super::theme::ResolvedTheme,
}

impl App {
    /// Legacy entry — prefer `bootstrap` + staged loading.
    pub fn new(image_path: &Path) -> Result<Self> {
        Self::bootstrap(image_path, None, vec![image_path.to_path_buf()])
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn empty_shell(
        image_path: &Path,
        compare_path: Option<&Path>,
        config: TuiConfig,
        guestfs: Guestfs,
        inspect_root: String,
        os_name: String,
        os_version: String,
        hostname: String,
        kernel_version: String,
        architecture: String,
        init_system: String,
        timezone: String,
        locale: String,
        current_view: View,
        layout_mode: LayoutMode,
        loaded_views: HashSet<View>,
    ) -> Self {
        let pinned_views = config.views.pinned.clone();
        let resolved_theme = super::theme::resolve(&config.ui);
        let assurance_target = config.behavior.default_migration_target.clone();

        Self {
            current_view,
            show_help: false,
            searching: false,
            search_query: String::new(),
            search_case_sensitive: config.behavior.search_case_sensitive,
            search_regex_mode: config.behavior.search_regex_mode,
            search_results: Vec::new(),
            live_filter_enabled: true,

            multi_select_mode: false,
            selected_items: HashSet::new(),
            select_all: false,

            show_file_preview: false,
            file_preview_content: String::new(),
            file_preview_path: String::new(),
            show_file_info: false,
            file_info_content: String::new(),
            file_filtering: false,
            file_filter_input: String::new(),

            active_filter: None,
            available_filters: vec![
                "critical".to_string(),
                "enabled".to_string(),
                "running".to_string(),
                "failed".to_string(),
                "installed".to_string(),
                "dev".to_string(),
            ],
            scroll_offset: 0,
            selected_index: 0,
            show_export_menu: false,
            selected_profile_tab: 0,
            show_detail: false,
            sort_mode: SortMode::Default,
            show_stats_bar: config.ui.show_stats_bar,
            table_mode: false, // Start in list view by default
            comparison_mode: false,
            snapshot_packages: None,
            snapshot_services: None,
            bookmarks: Vec::new(),
            search_history: Vec::new(),
            notification: None,
            last_updated: Local::now(),
            refreshing: false,

            show_jump_menu: false,
            jump_query: String::new(),
            jump_selected_index: 0,
            jump_scroll_offset: 0,
            help_scroll: 0,
            terminal_width: 80,
            terminal_height: 24,

            export_mode: None,
            export_format: None,
            export_filename: String::new(),

            image_path: image_path.display().to_string(),
            _image_path_buf: image_path.to_path_buf(),
            os_name,
            os_version,
            hostname,
            kernel_version,
            architecture,
            init_system,
            timezone,
            locale,

            network_interfaces: Vec::new(),
            dns_servers: Vec::new(),
            packages: PackageInfo {
                manager: "loading…".to_string(),
                package_count: 0,
                packages: Vec::new(),
            },
            services: Vec::new(),
            databases: Vec::new(),
            web_servers: Vec::new(),
            firewall: FirewallInfo {
                firewall_type: "unknown".to_string(),
                enabled: false,
                rules_count: 0,
                zones: Vec::new(),
            },
            security: SecurityInfo {
                selinux: "unknown".to_string(),
                apparmor: false,
                fail2ban: false,
                aide: false,
                auditd: false,
                ssh_keys: Vec::new(),
            },
            users: Vec::new(),
            _hosts: Vec::new(),
            fstab: Vec::new(),
            lvm_info: None,
            raid_arrays: Vec::new(),
            kernel_modules: Vec::new(),
            kernel_params: HashMap::new(),
            security_profile: None,
            migration_profile: None,
            performance_profile: None,
            compliance_profile: None,
            hardening_profile: None,
            config,
            file_browser: None,
            guestfs: Some(guestfs),
            inspect_root,
            loading: None,
            loaded_views,
            compare_image_path: compare_path.map(Path::to_path_buf),
            compare_summary: None,
            layout_mode,
            show_palette: false,
            palette_query: String::new(),
            palette_selected: 0,
            global_search: false,
            global_search_hits: Vec::new(),
            global_search_selected: 0,
            issue_filter: IssueRiskFilter::All,
            issue_detail_text: None,
            pinned_views,
            help_context: false,
            last_auto_refresh: Instant::now(),
            fleet_images: Vec::new(),
            fleet_index: 0,
            boot_report: None,
            migration_report: None,
            assurance_evidence: None,
            intelligence: None,
            assurance_target,
            agent_live: false,
            view_tab_scroll: 0,
            show_plan_preview: false,
            plan_preview: None,
            plan_preview_scroll: 0,
            resolved_theme,
        }
    }

    pub fn theme(&self) -> &super::theme::ResolvedTheme {
        &self.resolved_theme
    }

    pub fn fleet_active(&self) -> bool {
        self.fleet_images.len() > 1
    }

    /// Cleanup guestfs handle on app exit
    pub fn cleanup(&mut self) -> Result<()> {
        if let Some(mut guestfs) = self.guestfs.take() {
            guestfs.shutdown()?;
        }
        Ok(())
    }

    /// Initialize file browser with root directory
    pub fn init_file_browser(&mut self) {
        if self.file_browser.is_none() {
            let mut browser = crate::cli::tui::views::files::FileBrowserState::default();
            // Load initial directory
            if let Some(ref mut guestfs) = self.guestfs {
                let _ = browser.load_directory(guestfs);
            }
            self.file_browser = Some(browser);
        }
    }

    /// Navigate into selected directory in file browser
    pub fn file_browser_enter(&mut self) {
        if let Some(ref mut browser) = self.file_browser {
            if let Some(_new_path) = browser.enter_directory() {
                // Reload directory after navigation
                if let Some(ref mut guestfs) = self.guestfs {
                    let _ = browser.load_directory(guestfs);
                }
            }
        }
    }

    /// Navigate to parent directory in file browser
    pub fn file_browser_go_up(&mut self) {
        if let Some(ref mut browser) = self.file_browser {
            browser.go_up();
            // Reload directory after navigation
            if let Some(ref mut guestfs) = self.guestfs {
                let _ = browser.load_directory(guestfs);
            }
        }
    }

    /// Toggle hidden files in file browser
    pub fn file_browser_toggle_hidden(&mut self) {
        if let Some(ref mut browser) = self.file_browser {
            browser.toggle_hidden();
            // Reload directory to apply filter
            if let Some(ref mut guestfs) = self.guestfs {
                let _ = browser.load_directory(guestfs);
            }
        }
    }

    /// Move selection up in file browser
    pub fn file_browser_up(&mut self) {
        if let Some(ref mut browser) = self.file_browser {
            browser.move_up();
        }
    }

    /// Move selection down in file browser
    pub fn file_browser_down(&mut self) {
        if let Some(ref mut browser) = self.file_browser {
            let visible_items = 20; // Approximate visible items
            browser.move_down(visible_items);
        }
    }

    /// Show preview of selected file
    pub fn show_file_preview(&mut self) {
        use crate::cli::tui::views::files;

        if let Some(ref browser) = self.file_browser {
            if let Some(path) = files::get_selected_file_path(browser) {
                if let Some(ref mut guestfs) = self.guestfs {
                    // Check if it's a file (not directory)
                    if let Ok(is_dir) = guestfs.is_dir(&path) {
                        if is_dir {
                            self.show_notification("Cannot preview directory".to_string());
                            return;
                        }
                    }

                    // Check file size - don't preview files > 1MB
                    match guestfs.filesize(&path) {
                        Ok(size) if size > 1024 * 1024 => {
                            self.show_notification(format!(
                                "File too large to preview ({} bytes)",
                                size
                            ));
                            return;
                        }
                        Err(_) => {
                            // Cannot determine size; proceed with caution
                        }
                        _ => {}
                    }

                    // Read file content
                    match guestfs.cat(&path) {
                        Ok(content) => {
                            self.file_preview_content = content;
                            self.file_preview_path = path;
                            self.show_file_preview = true;
                        }
                        Err(e) => {
                            self.show_notification(format!("Error reading file: {}", e));
                        }
                    }
                }
            }
        }
    }

    /// Show information about selected file
    pub fn show_file_information(&mut self) {
        use crate::cli::tui::views::files;

        if let Some(ref browser) = self.file_browser {
            if let Some(path) = files::get_selected_file_path(browser) {
                if let Some(ref mut guestfs) = self.guestfs {
                    let mut info = Vec::new();

                    info.push(format!("Path: {}", path));

                    // File type
                    if let Ok(is_dir) = guestfs.is_dir(&path) {
                        info.push(format!(
                            "Type: {}",
                            if is_dir { "Directory" } else { "File" }
                        ));
                    }

                    // File size
                    if let Ok(size) = guestfs.filesize(&path) {
                        let size_str = format_file_size(size);
                        info.push(format!("Size: {} ({} bytes)", size_str, size));
                    }

                    // Permissions
                    if let Ok(stat) = guestfs.stat(&path) {
                        info.push(format!("Mode: {:o}", stat.mode));
                        info.push(format!("UID: {}", stat.uid));
                        info.push(format!("GID: {}", stat.gid));
                        info.push(format!("Blocks: {}", stat.blocks));
                    }

                    // File type detection
                    if let Ok(file_type) = guestfs.file(&path) {
                        info.push(format!("File Type: {}", file_type));
                    }

                    self.file_info_content = info.join("\n");
                    self.show_file_info = true;
                }
            }
        }
    }

    /// Close file preview
    pub fn close_file_preview(&mut self) {
        self.show_file_preview = false;
        self.file_preview_content.clear();
        self.file_preview_path.clear();
    }

    /// Close file info
    pub fn close_file_info(&mut self) {
        self.show_file_info = false;
        self.file_info_content.clear();
    }

    /// Start file filtering mode
    pub fn start_file_filter(&mut self) {
        self.file_filtering = true;
        self.file_filter_input.clear();
    }

    /// Add character to file filter
    pub fn file_filter_input_char(&mut self, c: char) {
        if self.file_filtering {
            self.file_filter_input.push(c);
            // Apply filter in real-time
            if let Some(ref mut browser) = self.file_browser {
                browser.set_filter(self.file_filter_input.clone());
            }
        }
    }

    /// Remove last character from file filter
    pub fn file_filter_backspace(&mut self) {
        if self.file_filtering && !self.file_filter_input.is_empty() {
            self.file_filter_input.pop();
            // Apply filter in real-time
            if let Some(ref mut browser) = self.file_browser {
                browser.set_filter(self.file_filter_input.clone());
            }
        }
    }

    /// Finish file filtering
    pub fn finish_file_filter(&mut self) {
        self.file_filtering = false;
        // Filter is already applied, just exit filter mode
    }

    /// Cancel file filtering
    pub fn cancel_file_filter(&mut self) {
        self.file_filtering = false;
        self.file_filter_input.clear();
        // Clear filter from browser
        if let Some(ref mut browser) = self.file_browser {
            browser.clear_filter();
        }
    }

    pub fn next_view(&mut self) {
        let views: Vec<View> = self.views_in_current_group();
        if views.is_empty() {
            return;
        }
        let current_idx = views
            .iter()
            .position(|v| v == &self.current_view)
            .unwrap_or(0);
        let next = views[(current_idx + 1) % views.len()];
        self.set_view(next);
        self.show_notification(format!("→ {}", self.current_view.title()));
    }

    pub fn previous_view(&mut self) {
        let views: Vec<View> = self.views_in_current_group();
        if views.is_empty() {
            return;
        }
        let current_idx = views
            .iter()
            .position(|v| v == &self.current_view)
            .unwrap_or(0);
        let prev = views[(current_idx + views.len() - 1) % views.len()];
        self.set_view(prev);
        self.show_notification(format!("← {}", self.current_view.title()));
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
        if self.show_help {
            self.help_context = false;
            self.help_scroll = 0;
        }
    }

    pub fn start_search(&mut self) {
        self.searching = true;
        self.search_query.clear();
    }

    pub fn cancel_search(&mut self) {
        if !self.search_query.is_empty() {
            self.add_to_search_history(self.search_query.clone());
        }
        self.searching = false;
        self.global_search = false;
        self.global_search_hits.clear();
        self.global_search_selected = 0;
        self.search_query.clear();
        self.search_results.clear();
    }

    pub fn is_searching(&self) -> bool {
        self.searching
    }

    pub fn search_input(&mut self, c: char) {
        self.search_query.push(c);
        if self.global_search {
            self.update_global_search();
        } else if self.live_filter_enabled {
            self.update_search_results();
        }
    }

    pub fn search_backspace(&mut self) {
        self.search_query.pop();
        if self.global_search {
            self.update_global_search();
        } else if self.live_filter_enabled {
            self.update_search_results();
        }
    }

    /// Toggle live filtering
    pub fn toggle_live_filter(&mut self) {
        self.live_filter_enabled = !self.live_filter_enabled;
        let status = if self.live_filter_enabled {
            "enabled"
        } else {
            "disabled"
        };
        self.show_notification(format!("Live filter {}", status));
    }

    /// Search hostname, packages, services, users, and profile findings.
    pub fn update_global_search(&mut self) {
        self.global_search_hits.clear();
        self.global_search_selected = 0;
        if self.search_query.is_empty() {
            return;
        }
        let q = self.search_query.to_lowercase();

        if self.hostname.to_lowercase().contains(&q) {
            self.global_search_hits.push(GlobalSearchHit {
                view: View::Dashboard,
                index: 0,
                label: format!("hostname: {}", self.hostname),
            });
        }

        for (idx, pkg) in self.packages.packages.iter().enumerate().take(500) {
            if pkg.name.to_lowercase().contains(&q) {
                self.global_search_hits.push(GlobalSearchHit {
                    view: View::Packages,
                    index: idx,
                    label: format!("package: {} {}", pkg.name, pkg.version),
                });
            }
        }
        for (idx, svc) in self.services.iter().enumerate().take(200) {
            if svc.name.to_lowercase().contains(&q) {
                self.global_search_hits.push(GlobalSearchHit {
                    view: View::Services,
                    index: idx,
                    label: format!("service: {} ({})", svc.name, svc.state),
                });
            }
        }
        for (idx, user) in self.users.iter().enumerate() {
            if user.username.to_lowercase().contains(&q) {
                self.global_search_hits.push(GlobalSearchHit {
                    view: View::Users,
                    index: idx,
                    label: format!("user: {}", user.username),
                });
            }
        }
        if let Some(ref profile) = self.security_profile {
            for section in &profile.sections {
                for (fidx, finding) in section.findings.iter().enumerate() {
                    if finding.item.to_lowercase().contains(&q)
                        || finding.message.to_lowercase().contains(&q)
                    {
                        self.global_search_hits.push(GlobalSearchHit {
                            view: View::Issues,
                            index: fidx,
                            label: format!("issue: {} — {}", finding.item, finding.message),
                        });
                    }
                }
            }
        }

        if let Some(ref boot) = self.boot_report {
            for (idx, b) in boot.blockers.iter().enumerate() {
                if b.title.to_lowercase().contains(&q) || b.message.to_lowercase().contains(&q) {
                    self.global_search_hits.push(GlobalSearchHit {
                        view: View::Assurance,
                        index: idx,
                        label: format!("boot blocker: {} — {}", b.title, b.message),
                    });
                }
            }
            for (idx, w) in boot.warnings.iter().enumerate() {
                if w.title.to_lowercase().contains(&q) || w.message.to_lowercase().contains(&q) {
                    self.global_search_hits.push(GlobalSearchHit {
                        view: View::Assurance,
                        index: idx,
                        label: format!("boot warning: {} — {}", w.title, w.message),
                    });
                }
            }
        }
        if let Some(ref mig) = self.migration_report {
            for (idx, c) in mig.required_changes.iter().enumerate() {
                if c.to_lowercase().contains(&q) {
                    self.global_search_hits.push(GlobalSearchHit {
                        view: View::Assurance,
                        index: idx,
                        label: format!("migration: {c}"),
                    });
                }
            }
            for d in &mig.driver_injections {
                if d.to_lowercase().contains(&q) {
                    self.global_search_hits.push(GlobalSearchHit {
                        view: View::Assurance,
                        index: 0,
                        label: format!("driver: {d}"),
                    });
                }
            }
        }

        let n = self.global_search_hits.len();
        self.show_notification(format!("Global: {} hit(s)", n));
    }

    pub fn global_search_next(&mut self) {
        if !self.global_search_hits.is_empty() {
            self.global_search_selected =
                (self.global_search_selected + 1) % self.global_search_hits.len();
        }
    }

    pub fn global_search_prev(&mut self) {
        if !self.global_search_hits.is_empty() {
            self.global_search_selected = if self.global_search_selected == 0 {
                self.global_search_hits.len() - 1
            } else {
                self.global_search_selected - 1
            };
        }
    }

    pub fn global_search_activate(&mut self) {
        if let Some(hit) = self
            .global_search_hits
            .get(self.global_search_selected)
            .cloned()
        {
            self.set_view(hit.view);
            self.selected_index = hit.index;
            self.scroll_offset = hit.index.saturating_sub(5);
            self.global_search = false;
            self.searching = false;
            self.search_query.clear();
            self.global_search_hits.clear();
            self.show_notification(format!("→ {}", hit.label));
        }
    }

    /// Update search results based on current query
    pub fn update_search_results(&mut self) {
        if self.search_query.is_empty() {
            self.search_results.clear();
            return;
        }

        let query = if self.search_case_sensitive {
            self.search_query.clone()
        } else {
            self.search_query.to_lowercase()
        };

        self.search_results.clear();

        // Compile regex once if in regex mode, with length limit and error handling
        let re = if self.search_regex_mode {
            if query.len() > 1000 {
                self.show_notification("Regex pattern too long (max 1000 chars)".to_string());
                return;
            }
            match regex::Regex::new(&query) {
                Ok(r) => Some(r),
                Err(e) => {
                    self.show_notification(format!("Invalid regex: {}", e));
                    return;
                }
            }
        } else {
            None
        };

        // Filter based on current view
        match self.current_view {
            View::Packages => {
                for (idx, pkg) in self.packages.packages.iter().enumerate() {
                    let name = if self.search_case_sensitive {
                        pkg.name.clone()
                    } else {
                        pkg.name.to_lowercase()
                    };

                    if let Some(ref re) = re {
                        if re.is_match(&name) {
                            self.search_results.push(idx);
                        }
                    } else if name.contains(&query) {
                        self.search_results.push(idx);
                    }
                }
            }
            View::Services => {
                for (idx, service) in self.services.iter().enumerate() {
                    let name = if self.search_case_sensitive {
                        service.name.clone()
                    } else {
                        service.name.to_lowercase()
                    };

                    if let Some(ref re) = re {
                        if re.is_match(&name) {
                            self.search_results.push(idx);
                        }
                    } else if name.contains(&query) {
                        self.search_results.push(idx);
                    }
                }
            }
            View::Network => {
                for (idx, iface) in self.network_interfaces.iter().enumerate() {
                    let name = if self.search_case_sensitive {
                        iface.name.clone()
                    } else {
                        iface.name.to_lowercase()
                    };

                    if name.contains(&query) {
                        self.search_results.push(idx);
                    }
                }
            }
            View::Users => {
                for (idx, user) in self.users.iter().enumerate() {
                    let name = if self.search_case_sensitive {
                        user.username.clone()
                    } else {
                        user.username.to_lowercase()
                    };

                    if name.contains(&query) {
                        self.search_results.push(idx);
                    }
                }
            }
            View::Databases => {
                for (idx, db) in self.databases.iter().enumerate() {
                    let name = if self.search_case_sensitive {
                        db.name.clone()
                    } else {
                        db.name.to_lowercase()
                    };

                    if name.contains(&query) {
                        self.search_results.push(idx);
                    }
                }
            }
            View::WebServers => {
                for (idx, ws) in self.web_servers.iter().enumerate() {
                    let name = if self.search_case_sensitive {
                        ws.name.clone()
                    } else {
                        ws.name.to_lowercase()
                    };

                    if name.contains(&query) {
                        self.search_results.push(idx);
                    }
                }
            }
            View::Kernel => {
                for (idx, module) in self.kernel_modules.iter().enumerate() {
                    let name = if self.search_case_sensitive {
                        module.clone()
                    } else {
                        module.to_lowercase()
                    };

                    if name.contains(&query) {
                        self.search_results.push(idx);
                    }
                }
            }
            _ => {
                // Other views don't support filtering yet
            }
        }

        if !self.search_results.is_empty() {
            self.show_notification(format!("{} matches found", self.search_results.len()));
        } else {
            self.show_notification("No matches found".to_string());
        }
    }

    /// Get filtered items or all items if no filter active
    pub fn get_filtered_count(&self) -> usize {
        if self.searching && self.live_filter_enabled && !self.search_results.is_empty() {
            self.search_results.len()
        } else {
            match self.current_view {
                View::Packages => self.packages.packages.len(),
                View::Services => self.services.len(),
                View::Network => self.network_interfaces.len(),
                View::Users => self.users.len(),
                View::Databases => self.databases.len(),
                View::WebServers => self.web_servers.len(),
                View::Kernel => self.kernel_modules.len(),
                View::Storage => self.fstab.len(),
                View::Assurance => self
                    .migration_report
                    .as_ref()
                    .map(|m| m.required_changes.len())
                    .unwrap_or(0),
                _ => 0,
            }
        }
    }

    pub fn scroll_up(&mut self) {
        // Special handling for Files view
        if self.current_view == View::Files {
            self.file_browser_up();
        } else if self.selected_index > 0 {
            self.selected_index -= 1;
            self.scroll_offset = self.scroll_offset.min(self.selected_index);
        }
    }

    pub fn scroll_down(&mut self) {
        // Special handling for Files view
        if self.current_view == View::Files {
            self.file_browser_down();
        } else {
            let count = self.get_filtered_count();
            if count > 0 && self.selected_index < count - 1 {
                self.selected_index += 1;
                self.scroll_offset = self
                    .scroll_offset
                    .max(self.selected_index.saturating_sub(20));
            }
        }
    }

    pub fn page_up(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(10);
        self.scroll_offset = self.scroll_offset.saturating_sub(10);
    }

    pub fn page_down(&mut self) {
        let count = self.get_filtered_count();
        if count == 0 {
            return;
        }
        let max = count - 1;
        self.selected_index = (self.selected_index + 10).min(max);
        self.scroll_offset = (self.scroll_offset + 10).min(max);
    }

    pub fn scroll_top(&mut self) {
        self.scroll_offset = 0;
        self.selected_index = 0;
    }

    pub fn scroll_bottom(&mut self) {
        let count = self.get_filtered_count();
        if count == 0 {
            return;
        }
        let max = count - 1;
        self.scroll_offset = max;
        self.selected_index = max;
    }

    pub fn select_item(&mut self) {
        // Handle item selection based on current view
    }

    pub fn on_tick(&mut self) {
        if let Some((_, ref mut ticks)) = self.notification {
            if *ticks > 0 {
                *ticks -= 1;
            } else {
                self.notification = None;
            }
        }

        if self.loading.is_some() {
            let _ = self.advance_loading();
            return;
        }

        let interval = self.config.behavior.auto_refresh_seconds;
        if interval > 0
            && !self.refreshing
            && !self.is_searching()
            && !self.show_palette
            && self.last_auto_refresh.elapsed().as_secs() >= interval
        {
            let _ = self.reload_current_view(false);
            self.last_auto_refresh = Instant::now();
        }
    }

    pub fn show_notification(&mut self, message: String) {
        // Show notification for 8 ticks (2 seconds at 250ms tick rate)
        self.notification = Some((message, 8));
    }

    pub fn toggle_export_menu(&mut self) {
        self.show_export_menu = !self.show_export_menu;
        if self.show_export_menu {
            // Reset export state when opening menu
            self.export_mode = Some(ExportMode::Selecting);
            self.export_format = None;
            self.export_filename.clear();
        } else {
            // Clear export state when closing menu
            self.export_mode = None;
        }
    }

    pub fn select_export_format(&mut self, format: ExportFormat) {
        self.export_format = Some(format);
        self.export_mode = Some(ExportMode::EnteringFilename);
        // Generate default filename
        let view_name = match self.current_view {
            View::Dashboard => "dashboard",
            View::Analytics => "analytics",
            View::Timeline => "timeline",
            View::Recommendations => "recommendations",
            View::Topology => "topology",
            View::Network => "network",
            View::Packages => "packages",
            View::Services => "services",
            View::Databases => "databases",
            View::WebServers => "webservers",
            View::Security => "security",
            View::Issues => "issues",
            View::Storage => "storage",
            View::Users => "users",
            View::Kernel => "kernel",
            View::Logs => "logs",
            View::Profiles => "profiles",
            View::Assurance => "assurance",
            View::SystemdDeep => "systemd-deep",
            View::AiInsights => "ai-insights",
            View::Files => "files",
        };
        self.export_filename = format!("guestkit-{}.{}", view_name, format.extension());
    }

    pub fn export_input(&mut self, c: char) {
        if matches!(self.export_mode, Some(ExportMode::EnteringFilename)) {
            self.export_filename.push(c);
        }
    }

    pub fn export_backspace(&mut self) {
        if matches!(self.export_mode, Some(ExportMode::EnteringFilename)) {
            self.export_filename.pop();
        }
    }

    pub fn cancel_export(&mut self) {
        if matches!(self.export_mode, Some(ExportMode::EnteringFilename)) {
            // Go back to format selection
            self.export_mode = Some(ExportMode::Selecting);
            self.export_format = None;
            self.export_filename.clear();
        } else {
            // Close export menu
            self.toggle_export_menu();
        }
    }

    pub fn is_exporting(&self) -> bool {
        self.export_mode.is_some()
    }

    pub fn execute_export(&mut self) -> Result<()> {
        if let Some(format) = self.export_format {
            self.export_mode = Some(ExportMode::Exporting);

            // Perform the actual export
            match self.do_export(format, &self.export_filename.clone()) {
                Ok(()) => {
                    self.export_mode = Some(ExportMode::Success(self.export_filename.clone()));
                    self.show_notification(format!("✓ Exported to {}", self.export_filename));
                }
                Err(e) => {
                    self.export_mode = Some(ExportMode::Error(e.to_string()));
                    self.show_notification(format!("✗ Export failed: {}", e));
                }
            }
        }
        Ok(())
    }

    fn do_export(&self, format: ExportFormat, filename: &str) -> Result<()> {
        use std::fs;
        use std::path::PathBuf;

        // Validate filename to prevent path traversal
        if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
            return Err(anyhow::anyhow!(
                "Invalid filename: path traversal not allowed"
            ));
        }

        let output_path = PathBuf::from(filename);

        // Export based on format
        match format {
            ExportFormat::Json => {
                let data = self.collect_export_data();
                let json = serde_json::to_string_pretty(&data)?;
                fs::write(&output_path, json)?;
            }
            ExportFormat::Yaml => {
                let data = self.collect_export_data();
                let yaml = serde_yaml::to_string(&data)?;
                fs::write(&output_path, yaml)?;
            }
            ExportFormat::Html => {
                let data = self.collect_export_data();
                let json = serde_json::to_string_pretty(&data)?;
                let html = format!(
                    "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\">\
                    <title>GuestKit Export</title>\
                    <style>body{{font-family:monospace;margin:2em;background:#1e1e2e;color:#cdd6f4}}\
                    pre{{background:#313244;padding:1em;border-radius:8px;overflow:auto}}</style>\
                    </head><body><h1>GuestKit TUI Export</h1>\
                    <pre>{}</pre></body></html>",
                    json.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
                );
                fs::write(&output_path, html)?;
            }
            ExportFormat::Pdf => {
                return Err(anyhow::anyhow!(
                    "PDF export is not supported from TUI. Use CLI: {} --export pdf",
                    crate::cli::invocation::example("inspect <image>")
                ));
            }
        }

        Ok(())
    }

    fn collect_export_data(&self) -> serde_json::Value {
        use serde_json::json;

        match self.current_view {
            View::Dashboard => json!({
                "view": "dashboard",
                "system": {
                    "os_name": self.os_name,
                    "os_version": self.os_version,
                    "hostname": self.hostname,
                    "kernel_version": self.kernel_version,
                    "architecture": self.architecture,
                    "init_system": self.init_system,
                    "timezone": self.timezone,
                    "locale": self.locale,
                },
                "stats": {
                    "packages": self.packages.package_count,
                    "services": self.services.len(),
                    "network_interfaces": self.network_interfaces.len(),
                    "databases": self.databases.len(),
                    "web_servers": self.web_servers.len(),
                    "users": self.users.len(),
                },
                "profiles": {
                    "security": self.security_profile.as_ref().and_then(|p| p.overall_risk),
                    "migration": self.migration_profile.as_ref().and_then(|p| p.overall_risk),
                    "performance": self.performance_profile.as_ref().and_then(|p| p.overall_risk),
                    "compliance": self.compliance_profile.as_ref().and_then(|p| p.overall_risk),
                    "hardening": self.hardening_profile.as_ref().and_then(|p| p.overall_risk),
                }
            }),
            View::Network => json!({
                "view": "network",
                "interfaces": self.network_interfaces,
                "dns_servers": self.dns_servers,
            }),
            View::Packages => {
                // Get filtered/selected packages
                let packages_to_export = self.get_filtered_export_packages();
                json!({
                    "view": "packages",
                    "manager": self.packages.manager,
                    "count": packages_to_export.len(),
                    "total_count": self.packages.package_count,
                    "filtered": packages_to_export.len() != self.packages.packages.len(),
                    "packages": packages_to_export,
                })
            }
            View::Services => {
                let services_to_export = self.get_filtered_export_services();
                json!({
                    "view": "services",
                    "count": services_to_export.len(),
                    "total_count": self.services.len(),
                    "filtered": services_to_export.len() != self.services.len(),
                    "services": services_to_export,
                })
            }
            View::Databases => json!({
                "view": "databases",
                "count": self.databases.len(),
                "databases": self.databases,
            }),
            View::WebServers => json!({
                "view": "webservers",
                "count": self.web_servers.len(),
                "web_servers": self.web_servers,
            }),
            View::Security => json!({
                "view": "security",
                "selinux": self.security.selinux,
                "apparmor": self.security.apparmor,
                "fail2ban": self.security.fail2ban,
                "aide": self.security.aide,
                "auditd": self.security.auditd,
                "ssh_keys": self.security.ssh_keys,
                "firewall": self.firewall,
            }),
            View::Issues => {
                let (critical, high, medium) = self.get_risk_summary();
                let mut all_sections = Vec::new();

                if let Some(sec) = &self.security_profile {
                    all_sections.extend(sec.sections.clone());
                }
                if let Some(hard) = &self.hardening_profile {
                    all_sections.extend(hard.sections.clone());
                }
                if let Some(comp) = &self.compliance_profile {
                    all_sections.extend(comp.sections.clone());
                }

                json!({
                    "view": "issues",
                    "summary": {
                        "critical": critical,
                        "high": high,
                        "medium": medium,
                        "total": critical + high + medium,
                    },
                    "sections": all_sections,
                })
            }
            View::Storage => {
                let fstab_to_export = self.get_filtered_export_storage();
                json!({
                    "view": "storage",
                    "fstab": fstab_to_export,
                    "fstab_count": fstab_to_export.len(),
                    "total_fstab_count": self.fstab.len(),
                    "filtered": fstab_to_export.len() != self.fstab.len(),
                    "lvm": self.lvm_info,
                    "raid": self.raid_arrays,
                })
            }
            View::Users => {
                let users_to_export = self.get_filtered_export_users();
                json!({
                    "view": "users",
                    "count": users_to_export.len(),
                    "total_count": self.users.len(),
                    "filtered": users_to_export.len() != self.users.len(),
                    "users": users_to_export,
                })
            }
            View::Kernel => json!({
                "view": "kernel",
                "modules": {
                    "count": self.kernel_modules.len(),
                    "list": self.kernel_modules,
                },
                "parameters": self.kernel_params,
            }),
            View::Analytics => {
                let (critical, high, medium) = self.get_risk_summary();
                json!({
                    "view": "analytics",
                    "security_score": {
                        "critical": critical,
                        "high": high,
                        "medium": medium,
                    },
                    "package_stats": {
                        "total": self.packages.package_count,
                    },
                    "service_stats": {
                        "total": self.services.len(),
                        "enabled": self.services.iter().filter(|s| s.enabled).count(),
                    },
                })
            }
            View::Timeline => json!({
                "view": "timeline",
                "system": {
                    "os": self.os_name,
                    "kernel": self.kernel_version,
                },
            }),
            View::Recommendations => json!({
                "view": "recommendations",
                "summary": "System recommendations and optimization suggestions",
            }),
            View::Topology => json!({
                "view": "topology",
                "summary": "System architecture and network topology visualization",
            }),
            View::Logs => json!({
                "view": "logs",
                "summary": "System logs view",
            }),
            View::Profiles => {
                let current_profile = match self.selected_profile_tab {
                    0 => self.security_profile.as_ref().map(|p| ("security", p)),
                    1 => self.migration_profile.as_ref().map(|p| ("migration", p)),
                    2 => self
                        .performance_profile
                        .as_ref()
                        .map(|p| ("performance", p)),
                    3 => self.compliance_profile.as_ref().map(|p| ("compliance", p)),
                    4 => self.hardening_profile.as_ref().map(|p| ("hardening", p)),
                    _ => None,
                };

                if let Some((name, profile)) = current_profile {
                    json!({
                        "view": "profiles",
                        "profile": name,
                        "report": profile,
                    })
                } else {
                    json!({
                        "view": "profiles",
                        "error": "No profile data available"
                    })
                }
            }
            View::Files => {
                // Export file browser state
                let files = if let Some(ref browser) = self.file_browser {
                    browser
                        .entries
                        .iter()
                        .map(|entry| {
                            json!({
                                "name": entry.name,
                                "is_dir": entry.is_dir,
                                "size": entry.size,
                            })
                        })
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                };

                json!({
                    "view": "files",
                    "current_path": self.file_browser.as_ref().map(|b| &b.current_path).unwrap_or(&"/".to_string()),
                    "filter": self.file_browser.as_ref().map(|b| &b.filter).unwrap_or(&String::new()),
                    "show_hidden": self.file_browser.as_ref().map(|b| b.show_hidden).unwrap_or(false),
                    "file_count": files.len(),
                    "files": files,
                })
            }
            View::Assurance => json!({
                "view": "assurance",
                "target": self.assurance_target,
                "bootability": self.boot_report,
                "migration_score": self.migration_report,
            }),
            View::SystemdDeep => json!({
                "view": "systemd-deep",
                "intelligence": self.intelligence,
            }),
            View::AiInsights => json!({
                "view": "ai-insights",
                "intelligence": self.intelligence,
            }),
        }
    }

    pub fn next_profile_tab(&mut self) {
        self.selected_profile_tab = (self.selected_profile_tab + 1) % 5;
        let profile_names = [
            "Security",
            "Migration",
            "Performance",
            "Compliance",
            "Hardening",
        ];
        self.show_notification(format!(
            "→ {} Profile",
            profile_names[self.selected_profile_tab]
        ));
    }

    pub fn previous_profile_tab(&mut self) {
        self.selected_profile_tab = (self.selected_profile_tab + 4) % 5;
        let profile_names = [
            "Security",
            "Migration",
            "Performance",
            "Compliance",
            "Hardening",
        ];
        self.show_notification(format!(
            "← {} Profile",
            profile_names[self.selected_profile_tab]
        ));
    }

    pub fn get_current_profile_report(&self) -> Option<&ProfileReport> {
        match self.selected_profile_tab {
            0 => self.security_profile.as_ref(),
            1 => self.migration_profile.as_ref(),
            2 => self.performance_profile.as_ref(),
            3 => self.compliance_profile.as_ref(),
            4 => self.hardening_profile.as_ref(),
            _ => None,
        }
    }

    pub fn toggle_detail(&mut self) {
        self.show_detail = !self.show_detail;
    }

    pub fn cycle_sort_mode(&mut self) {
        self.sort_mode = self.sort_mode.next(&self.current_view);
        // Reset scroll when sorting changes
        self.scroll_offset = 0;
        self.selected_index = 0;
        self.show_notification(format!("Sort: {}", self.sort_mode.label()));
    }

    pub fn jump_to_view(&mut self, index: usize) {
        let ordered: Vec<View> = self
            .tab_titles_ordered()
            .into_iter()
            .map(|(v, _)| v)
            .collect();
        if index < ordered.len() {
            let v = ordered[index];
            self.set_view(v);
            self.show_notification(format!("⚡ {} ({})", self.current_view.title(), index + 1));
        }
    }

    /// Mouse click: tab rows change group or view.
    pub fn handle_mouse_click(&mut self, column: u16, row: u16) {
        let (group_y, view_y) = self.tab_row_y();
        if row == group_y || row == group_y + 1 {
            self.handle_group_tab_click(column);
        } else if row == view_y || row == view_y + 1 {
            self.handle_view_tab_click(column);
        }
    }

    fn handle_group_tab_click(&mut self, column: u16) {
        let mut x: u16 = 2;
        for group in View::all_groups() {
            let label = format!("  {}  ", group);
            let width = label.chars().count() as u16;
            if column >= x && column < x + width {
                self.set_group(group);
                return;
            }
            x += width + 1;
        }
    }

    fn handle_view_tab_click(&mut self, column: u16) {
        let mut x: u16 = 2;
        for (view, title) in self
            .view_tab_entries()
            .into_iter()
            .skip(self.view_tab_scroll)
        {
            let marker = if view == self.current_view {
                "▸ "
            } else {
                "  "
            };
            let label = format!("{}{} ", marker, title);
            let width = label.chars().count() as u16;
            if column >= x && column < x + width {
                self.set_view(view);
                return;
            }
            x += width;
        }
    }

    pub fn toggle_stats_bar(&mut self) {
        self.show_stats_bar = !self.show_stats_bar;
        let state = if self.show_stats_bar {
            "shown"
        } else {
            "hidden"
        };
        self.show_notification(format!("Stats bar {}", state));
    }

    pub fn toggle_table_mode(&mut self) {
        self.table_mode = !self.table_mode;
        let mode = if self.table_mode { "Table" } else { "List" };
        self.show_notification(format!("View mode: {}", mode));
    }

    pub fn toggle_comparison_mode(&mut self) {
        self.comparison_mode = !self.comparison_mode;
        if self.comparison_mode {
            // Take snapshot when entering comparison mode
            self.take_snapshot();
            self.show_notification("Comparison mode enabled - snapshot taken".to_string());
        } else {
            self.show_notification("Comparison mode disabled".to_string());
        }
    }

    pub fn take_snapshot(&mut self) {
        // Take snapshots of current state for comparison
        self.snapshot_packages = Some(self.packages.packages.clone());
        self.snapshot_services = Some(self.services.clone());
        self.show_notification("✓ Snapshot captured".to_string());
    }

    pub fn get_package_diff_stats(&self) -> (usize, usize, usize) {
        // Returns (added, removed, modified)
        if let Some(ref snapshot) = self.snapshot_packages {
            let current_names: std::collections::HashSet<&str> = self
                .packages
                .packages
                .iter()
                .map(|p| p.name.as_str())
                .collect();
            let snapshot_names: std::collections::HashSet<&str> =
                snapshot.iter().map(|p| p.name.as_str()).collect();

            let added = current_names.difference(&snapshot_names).count();
            let removed = snapshot_names.difference(&current_names).count();

            // Check for version changes (modified)
            let mut modified = 0;
            for pkg in &self.packages.packages {
                if let Some(old_pkg) = snapshot.iter().find(|p| p.name == pkg.name) {
                    if old_pkg.version != pkg.version {
                        modified += 1;
                    }
                }
            }

            (added, removed, modified)
        } else {
            (0, 0, 0)
        }
    }

    pub fn get_service_diff_stats(&self) -> (usize, usize, usize) {
        // Returns (started, stopped, changed)
        if let Some(ref snapshot) = self.snapshot_services {
            let mut started = 0;
            let mut stopped = 0;
            let mut changed = 0;

            for svc in &self.services {
                if let Some(old_svc) = snapshot.iter().find(|s| s.name == svc.name) {
                    if old_svc.state != svc.state {
                        if svc.state == "running" && old_svc.state != "running" {
                            started += 1;
                        } else if svc.state != "running" && old_svc.state == "running" {
                            stopped += 1;
                        } else {
                            changed += 1;
                        }
                    }
                } else {
                    // New service
                    if svc.state == "running" {
                        started += 1;
                    }
                }
            }

            // Check for removed services
            for old_svc in snapshot {
                if !self.services.iter().any(|s| s.name == old_svc.name)
                    && old_svc.state == "running"
                {
                    stopped += 1;
                }
            }

            (started, stopped, changed)
        } else {
            (0, 0, 0)
        }
    }

    pub fn add_bookmark(&mut self, item: String) {
        if !self.bookmarks.contains(&item) {
            self.bookmarks.push(item.clone());
            // Keep only last 20 bookmarks
            if self.bookmarks.len() > 20 {
                self.bookmarks.remove(0);
            }
            self.show_notification(format!("✓ Bookmarked: {}", item));
        } else {
            self.show_notification("⚠ Already bookmarked".to_string());
        }
    }

    pub fn add_to_search_history(&mut self, query: String) {
        if !query.is_empty() && !self.search_history.contains(&query) {
            self.search_history.push(query);
            // Keep only last 10 searches
            if self.search_history.len() > 10 {
                self.search_history.remove(0);
            }
        }
    }

    pub fn get_risk_summary(&self) -> (usize, usize, usize) {
        let mut critical = 0;
        let mut high = 0;
        let mut medium = 0;

        let profiles = vec![
            &self.security_profile,
            &self.migration_profile,
            &self.performance_profile,
            &self.compliance_profile,
            &self.hardening_profile,
        ];

        for p in profiles.into_iter().flatten() {
            if let Some(risk) = p.overall_risk {
                use crate::cli::profiles::RiskLevel;
                match risk {
                    RiskLevel::Critical => critical += 1,
                    RiskLevel::High => high += 1,
                    RiskLevel::Medium => medium += 1,
                    _ => {}
                }
            }
        }

        (critical, high, medium)
    }

    /// Calculate overall system health score (0-100)
    pub fn calculate_health_score(&self) -> u8 {
        let mut score: u8 = 100;

        // Deduct points for critical/high/medium risks
        let (critical, high, medium) = self.get_risk_summary();
        score = score.saturating_sub((critical * 20).min(255) as u8);
        score = score.saturating_sub((high * 10).min(255) as u8);
        score = score.saturating_sub((medium * 5).min(255) as u8);

        // Deduct points for missing security features
        if &self.security.selinux == "disabled" {
            score = score.saturating_sub(10);
        }
        if !self.firewall.enabled {
            score = score.saturating_sub(15);
        }
        if !self.security.auditd {
            score = score.saturating_sub(5);
        }
        if !self.security.fail2ban {
            score = score.saturating_sub(5);
        }
        if !self.security.aide {
            score = score.saturating_sub(5);
        }

        // Bonus points for good practices
        if self.security.apparmor || &self.security.selinux != "disabled" {
            score = (score + 5).min(100);
        }

        score
    }

    /// Get health status message and color based on score
    pub fn get_health_status(&self) -> (&str, &str) {
        let score = self.calculate_health_score();
        match score {
            90..=100 => ("Excellent", "green"),
            75..=89 => ("Good", "yellow"),
            60..=74 => ("Fair", "orange"),
            40..=59 => ("Poor", "red"),
            _ => ("Critical", "red"),
        }
    }

    /// Get time since last update in human-readable format
    pub fn get_time_since_update(&self) -> String {
        let duration = Local::now().signed_duration_since(self.last_updated);

        if duration.num_seconds() < 60 {
            format!("{}s ago", duration.num_seconds())
        } else if duration.num_minutes() < 60 {
            format!("{}m ago", duration.num_minutes())
        } else if duration.num_hours() < 24 {
            format!("{}h ago", duration.num_hours())
        } else {
            format!("{}d ago", duration.num_days())
        }
    }

    pub fn start_refresh(&mut self, full: bool) {
        self.show_notification(if full {
            "Full re-inspect…".to_string()
        } else {
            "Refreshing view…".to_string()
        });
        let _ = self.reload_current_view(full);
        self.show_notification("✓ Data refreshed".to_string());
    }

    pub fn complete_refresh(&mut self) {
        self.refreshing = false;
        self.last_updated = Local::now();
    }

    pub fn toggle_palette(&mut self) {
        self.show_palette = !self.show_palette;
        if self.show_palette {
            self.palette_query.clear();
            self.palette_selected = 0;
        }
    }

    pub fn palette_input(&mut self, c: char) {
        self.palette_query.push(c);
        self.palette_selected = 0;
    }

    pub fn palette_backspace(&mut self) {
        self.palette_query.pop();
        self.palette_selected = 0;
    }

    pub fn palette_execute(&mut self) {
        use super::palette::parse_command;
        let action = parse_command(&self.palette_query);
        self.show_palette = false;
        self.run_palette_action(action);
    }

    pub fn toggle_context_help(&mut self) {
        self.help_context = !self.help_context;
        self.show_help = self.help_context;
        if self.show_help {
            self.help_scroll = 0;
        }
    }

    pub fn start_global_search(&mut self) {
        self.global_search = true;
        self.searching = true;
        self.search_query.clear();
        self.global_search_hits.clear();
        self.show_notification("Global search — type query, Enter to jump".to_string());
    }

    /// Extract selected file from guest image to host cwd.
    pub fn file_browser_extract(&mut self) {
        use crate::cli::tui::views::files;
        let Some(ref browser) = self.file_browser else {
            return;
        };
        let Some(path) = files::get_selected_file_path(browser) else {
            return;
        };
        let Some(ref mut guestfs) = self.guestfs else {
            return;
        };
        if guestfs.is_dir(&path).unwrap_or(false) {
            self.show_notification("Cannot extract a directory (use cp in shell)".to_string());
            return;
        }
        let base = path.rsplit('/').next().unwrap_or("extracted");
        let dest = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(base);
        let dest_str = dest.display().to_string();
        match guestfs.download(&path, &dest_str) {
            Ok(()) => self.show_notification(format!("Extracted → {}", dest.display())),
            Err(e) => self.show_notification(format!("Extract failed: {}", e)),
        }
    }

    pub fn cycle_layout_mode_backward(&mut self) {
        self.layout_mode = match self.layout_mode {
            LayoutMode::ListOnly => LayoutMode::DetailFull,
            LayoutMode::SplitDetail => LayoutMode::ListOnly,
            LayoutMode::DetailFull => LayoutMode::SplitDetail,
        };
        let label = match self.layout_mode {
            LayoutMode::ListOnly => "List",
            LayoutMode::SplitDetail => "Split",
            LayoutMode::DetailFull => "Detail",
        };
        self.show_notification(format!("Layout: {}", label));
    }

    /// Toggle case-sensitive search
    pub fn toggle_case_sensitive(&mut self) {
        self.search_case_sensitive = !self.search_case_sensitive;
        let status = if self.search_case_sensitive {
            "ON"
        } else {
            "OFF"
        };
        self.show_notification(format!("Case-sensitive: {}", status));
    }

    /// Toggle regex search mode
    pub fn toggle_regex_mode(&mut self) {
        self.search_regex_mode = !self.search_regex_mode;
        let status = if self.search_regex_mode { "ON" } else { "OFF" };
        self.show_notification(format!("Regex mode: {}", status));
    }

    /// Get search mode indicator string
    pub fn get_search_mode_indicator(&self) -> String {
        let mut indicators = Vec::new();
        if self.search_case_sensitive {
            indicators.push("Aa");
        }
        if self.search_regex_mode {
            indicators.push(".*");
        }
        if indicators.is_empty() {
            String::new()
        } else {
            format!("[{}] ", indicators.join(" "))
        }
    }

    /// Toggle jump menu visibility
    pub fn toggle_jump_menu(&mut self) {
        self.show_jump_menu = !self.show_jump_menu;
        if self.show_jump_menu {
            self.jump_query.clear();
            self.jump_selected_index = 0;
            self.jump_scroll_offset = 0;
        }
    }

    /// Handle jump menu input
    pub fn jump_menu_input(&mut self, c: char) {
        self.jump_query.push(c);
        self.jump_selected_index = 0;
        self.jump_scroll_offset = 0;
    }

    /// Handle jump menu backspace
    pub fn jump_menu_backspace(&mut self) {
        self.jump_query.pop();
        self.jump_selected_index = 0;
        self.jump_scroll_offset = 0;
    }

    /// Grouped quick-jump entries: (group name, view, display title).
    pub fn get_grouped_jump_entries(&self) -> Vec<(String, View, String)> {
        let views = View::all();
        let query_lower = self.jump_query.to_lowercase();

        let mut entries: Vec<(String, View, String)> = views
            .iter()
            .filter(|v| {
                if query_lower.is_empty() {
                    return true;
                }
                let title = v.title().to_lowercase();
                let group = v.group().to_lowercase();
                title.contains(&query_lower) || group.contains(&query_lower)
            })
            .map(|v| (v.group().to_string(), *v, v.title().to_string()))
            .collect();

        entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.2.cmp(&b.2)));
        entries
    }

    /// Flat jump menu rows: group headers + selectable view items.
    pub fn build_jump_menu_rows(&self) -> Vec<JumpMenuRow> {
        let entries = self.get_grouped_jump_entries();
        let mut rows = Vec::new();
        let mut last_group = String::new();
        for (group, view, title) in entries {
            if group != last_group {
                rows.push(JumpMenuRow::Header(group.clone()));
                last_group = group.clone();
            }
            rows.push(JumpMenuRow::Item { group, view, title });
        }
        rows
    }

    fn jump_selected_display_index(&self) -> usize {
        let rows = self.build_jump_menu_rows();
        let mut item_idx = 0;
        for (i, row) in rows.iter().enumerate() {
            if let JumpMenuRow::Item { .. } = row {
                if item_idx == self.jump_selected_index {
                    return i;
                }
                item_idx += 1;
            }
        }
        0
    }

    pub fn ensure_jump_scroll_visible(&mut self, visible_height: usize) {
        if visible_height == 0 {
            return;
        }
        let sel = self.jump_selected_display_index();
        if sel < self.jump_scroll_offset {
            self.jump_scroll_offset = sel;
        } else if sel >= self.jump_scroll_offset + visible_height {
            self.jump_scroll_offset = sel + 1 - visible_height;
        }
    }

    /// Navigate jump menu selection
    pub fn jump_menu_next(&mut self) {
        let n = self.get_grouped_jump_entries().len();
        if n > 0 {
            self.jump_selected_index = (self.jump_selected_index + 1) % n;
            let visible = ((self.terminal_height as usize) * 60 / 100)
                .saturating_sub(6)
                .max(5);
            self.ensure_jump_scroll_visible(visible);
        }
    }

    pub fn jump_menu_previous(&mut self) {
        let n = self.get_grouped_jump_entries().len();
        if n > 0 {
            self.jump_selected_index = if self.jump_selected_index == 0 {
                n - 1
            } else {
                self.jump_selected_index - 1
            };
            let visible = ((self.terminal_height as usize) * 60 / 100)
                .saturating_sub(6)
                .max(5);
            self.ensure_jump_scroll_visible(visible);
        }
    }

    pub fn jump_menu_select(&mut self) {
        if let Some((_, view, _)) = self
            .get_grouped_jump_entries()
            .get(self.jump_selected_index)
        {
            let view = *view;
            self.show_jump_menu = false;
            self.set_view(view);
            self.show_notification(format!("→ {}", view.title()));
        }
    }

    pub fn help_scroll_up(&mut self) {
        self.help_scroll = self.help_scroll.saturating_sub(1);
    }

    pub fn help_scroll_down(&mut self, max_lines: usize, visible: usize) {
        let max_scroll = max_lines.saturating_sub(visible);
        self.help_scroll = (self.help_scroll + 1).min(max_scroll);
    }

    pub fn help_page_up(&mut self, page: usize) {
        self.help_scroll = self.help_scroll.saturating_sub(page);
    }

    pub fn help_page_down(&mut self, max_lines: usize, visible: usize, page: usize) {
        let max_scroll = max_lines.saturating_sub(visible);
        self.help_scroll = (self.help_scroll + page).min(max_scroll);
    }

    /// Approximate visible lines in the full help overlay.
    pub fn help_visible_lines(&self) -> usize {
        ((self.terminal_height as u32 * 85 / 100).saturating_sub(2)) as usize
    }

    /// Total lines in the full keyboard reference help overlay.
    pub const FULL_HELP_LINES: usize = 80;

    /// Toggle multi-select mode
    pub fn toggle_multi_select(&mut self) {
        self.multi_select_mode = !self.multi_select_mode;
        if !self.multi_select_mode {
            self.selected_items.clear();
            self.select_all = false;
        }
        let status = if self.multi_select_mode { "ON" } else { "OFF" };
        self.show_notification(format!("Multi-select: {}", status));
    }

    /// Toggle selection of current item
    pub fn toggle_item_selection(&mut self) {
        if !self.multi_select_mode {
            self.multi_select_mode = true;
            self.show_notification("Multi-select: ON".to_string());
        }

        let idx = self.selected_index;
        if self.selected_items.contains(&idx) {
            self.selected_items.remove(&idx);
        } else {
            self.selected_items.insert(idx);
        }

        self.show_notification(format!("{} items selected", self.selected_items.len()));
    }

    /// Select all items in current view
    pub fn select_all_items(&mut self) {
        if !self.multi_select_mode {
            self.multi_select_mode = true;
        }

        let max_items = self.get_filtered_count();

        if self.select_all {
            // Deselect all
            self.selected_items.clear();
            self.select_all = false;
            self.show_notification("Deselected all".to_string());
        } else {
            // Select all
            self.selected_items.clear();
            for i in 0..max_items {
                self.selected_items.insert(i);
            }
            self.select_all = true;
            self.show_notification(format!("Selected all {} items", max_items));
        }
    }

    /// Check if item is selected
    pub fn is_item_selected(&self, index: usize) -> bool {
        self.selected_items.contains(&index)
    }

    /// Get count of selected items
    pub fn get_selected_count(&self) -> usize {
        self.selected_items.len()
    }

    /// Apply quick filter
    pub fn apply_filter(&mut self, filter: &str) {
        if self.active_filter.as_deref() == Some(filter) {
            // Toggle off if same filter
            self.active_filter = None;
            self.show_notification(format!("Filter '{}' removed", filter));
        } else {
            self.active_filter = Some(filter.to_string());
            self.show_notification(format!("Filter: {}", filter));
            self.update_filtered_items();
        }
    }

    /// Update items based on active filter
    fn update_filtered_items(&mut self) {
        if self.active_filter.is_none() {
            return;
        }

        let Some(filter) = self.active_filter.as_ref() else {
            return;
        };

        match filter.as_str() {
            "critical" => {
                // Filter critical security issues
                self.current_view = View::Issues;
                self.scroll_offset = 0;
            }
            "enabled"
                // Filter enabled services
                if self.current_view == View::Services => {
                    // In real implementation, would filter the list
                    self.show_notification(format!("{} enabled services",
                        self.services.iter().filter(|s| s.enabled).count()));
                }
            "running"
                // Filter running services
                if self.current_view == View::Services => {
                    self.show_notification(format!("{} running services",
                        self.services.iter().filter(|s| s.state == "running").count()));
                }
            "failed"
                // Filter failed services
                if self.current_view == View::Services => {
                    self.show_notification(format!("{} failed services",
                        self.services.iter().filter(|s| s.state == "failed").count()));
                }
            "dev"
                // Filter development packages
                if self.current_view == View::Packages => {
                    let dev_count = self.packages.packages.iter()
                        .filter(|p| p.name.contains("devel") || p.name.contains("-dev"))
                        .count();
                    self.show_notification(format!("{} dev packages", dev_count));
                }
            _ => {}
        }
    }

    /// Cycle through available filters
    pub fn cycle_filter(&mut self) {
        if self.available_filters.is_empty() {
            return;
        }

        // Clone filter name to avoid borrow conflict
        let next_filter = if let Some(current) = &self.active_filter {
            if let Some(idx) = self.available_filters.iter().position(|f| f == current) {
                let next_idx = (idx + 1) % self.available_filters.len();
                self.available_filters[next_idx].clone()
            } else {
                self.available_filters[0].clone()
            }
        } else {
            self.available_filters[0].clone()
        };

        self.apply_filter(&next_filter);
    }

    /// Get active filter label for display
    pub fn get_filter_label(&self) -> Option<String> {
        self.active_filter.as_ref().map(|f| {
            match f.as_str() {
                "critical" => "🔴 Critical",
                "enabled" => "✅ Enabled",
                "running" => "▶️  Running",
                "failed" => "❌ Failed",
                "installed" => "📦 Installed",
                "dev" => "🔧 Dev Packages",
                _ => f.as_str(),
            }
            .to_string()
        })
    }

    /// Get sorted package indices based on current sort mode
    pub fn get_sorted_package_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..self.packages.packages.len()).collect();

        match self.sort_mode {
            SortMode::NameAsc => {
                indices.sort_by(|&a, &b| {
                    self.packages.packages[a]
                        .name
                        .to_lowercase()
                        .cmp(&self.packages.packages[b].name.to_lowercase())
                });
            }
            SortMode::NameDesc => {
                indices.sort_by(|&a, &b| {
                    self.packages.packages[b]
                        .name
                        .to_lowercase()
                        .cmp(&self.packages.packages[a].name.to_lowercase())
                });
            }
            SortMode::VersionAsc => {
                indices.sort_by(|&a, &b| {
                    self.packages.packages[a]
                        .version
                        .to_lowercase()
                        .cmp(&self.packages.packages[b].version.to_lowercase())
                });
            }
            SortMode::VersionDesc => {
                indices.sort_by(|&a, &b| {
                    self.packages.packages[b]
                        .version
                        .to_lowercase()
                        .cmp(&self.packages.packages[a].version.to_lowercase())
                });
            }
            _ => {} // Default order
        }

        indices
    }

    /// Get sorted service indices based on current sort mode
    pub fn get_sorted_service_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..self.services.len()).collect();

        match self.sort_mode {
            SortMode::NameAsc => {
                indices.sort_by(|&a, &b| {
                    self.services[a]
                        .name
                        .to_lowercase()
                        .cmp(&self.services[b].name.to_lowercase())
                });
            }
            SortMode::NameDesc => {
                indices.sort_by(|&a, &b| {
                    self.services[b]
                        .name
                        .to_lowercase()
                        .cmp(&self.services[a].name.to_lowercase())
                });
            }
            SortMode::StateAsc => {
                indices.sort_by(|&a, &b| self.services[a].state.cmp(&self.services[b].state));
            }
            SortMode::StateDesc => {
                indices.sort_by(|&a, &b| self.services[b].state.cmp(&self.services[a].state));
            }
            SortMode::EnabledFirst => {
                indices.sort_by(|&a, &b| {
                    // Enabled first (true > false in reverse)
                    self.services[b]
                        .enabled
                        .cmp(&self.services[a].enabled)
                        .then(
                            self.services[a]
                                .name
                                .to_lowercase()
                                .cmp(&self.services[b].name.to_lowercase()),
                        )
                });
            }
            _ => {} // Default order
        }

        indices
    }

    /// Get sorted user indices based on current sort mode
    pub fn get_sorted_user_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..self.users.len()).collect();

        match self.sort_mode {
            SortMode::NameAsc => {
                indices.sort_by(|&a, &b| {
                    self.users[a]
                        .username
                        .to_lowercase()
                        .cmp(&self.users[b].username.to_lowercase())
                });
            }
            SortMode::NameDesc => {
                indices.sort_by(|&a, &b| {
                    self.users[b]
                        .username
                        .to_lowercase()
                        .cmp(&self.users[a].username.to_lowercase())
                });
            }
            SortMode::UidAsc => {
                indices.sort_by(|&a, &b| self.users[a].uid.cmp(&self.users[b].uid));
            }
            SortMode::UidDesc => {
                indices.sort_by(|&a, &b| self.users[b].uid.cmp(&self.users[a].uid));
            }
            _ => {} // Default order
        }

        indices
    }

    /// Get sorted storage (fstab) indices based on current sort mode
    pub fn get_sorted_storage_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..self.fstab.len()).collect();

        match self.sort_mode {
            SortMode::NameAsc => {
                indices.sort_by(|&a, &b| {
                    self.fstab[a]
                        .0
                        .to_lowercase()
                        .cmp(&self.fstab[b].0.to_lowercase())
                });
            }
            SortMode::NameDesc => {
                indices.sort_by(|&a, &b| {
                    self.fstab[b]
                        .0
                        .to_lowercase()
                        .cmp(&self.fstab[a].0.to_lowercase())
                });
            }
            SortMode::SizeAsc => {
                // For fstab, sort by mountpoint instead of size
                indices.sort_by(|&a, &b| {
                    self.fstab[a]
                        .1
                        .to_lowercase()
                        .cmp(&self.fstab[b].1.to_lowercase())
                });
            }
            SortMode::SizeDesc => {
                // For fstab, sort by mountpoint reverse
                indices.sort_by(|&a, &b| {
                    self.fstab[b]
                        .1
                        .to_lowercase()
                        .cmp(&self.fstab[a].1.to_lowercase())
                });
            }
            _ => {} // Default order
        }

        indices
    }

    /// Get filtered/selected packages for export
    fn get_filtered_export_packages(&self) -> Vec<&Package> {
        // If multi-select mode with items selected, export only selected
        if self.multi_select_mode && !self.selected_items.is_empty() {
            return self
                .selected_items
                .iter()
                .filter_map(|&idx| self.packages.packages.get(idx))
                .collect();
        }

        // Get sorted and filtered indices (respects search and filters)
        let sorted_indices = self.get_sorted_package_indices();

        // Apply search filter if active
        let filtered_indices: Vec<usize> = if self.is_searching() && !self.search_query.is_empty() {
            sorted_indices
                .into_iter()
                .filter(|&idx| {
                    let pkg = &self.packages.packages[idx];
                    pkg.name
                        .to_lowercase()
                        .contains(&self.search_query.to_lowercase())
                        || pkg.version.contains(&self.search_query)
                })
                .collect()
        } else {
            sorted_indices
        };

        // Return packages in filtered order
        filtered_indices
            .iter()
            .filter_map(|&idx| self.packages.packages.get(idx))
            .collect()
    }

    /// Get filtered/selected services for export
    fn get_filtered_export_services(&self) -> Vec<&SystemService> {
        // If multi-select mode with items selected, export only selected
        if self.multi_select_mode && !self.selected_items.is_empty() {
            return self
                .selected_items
                .iter()
                .filter_map(|&idx| self.services.get(idx))
                .collect();
        }

        // Get sorted and filtered indices
        let sorted_indices = self.get_sorted_service_indices();

        // Apply search filter if active
        let filtered_indices: Vec<usize> = if self.is_searching() && !self.search_query.is_empty() {
            sorted_indices
                .into_iter()
                .filter(|&idx| {
                    let svc = &self.services[idx];
                    svc.name
                        .to_lowercase()
                        .contains(&self.search_query.to_lowercase())
                        || svc
                            .state
                            .to_lowercase()
                            .contains(&self.search_query.to_lowercase())
                })
                .collect()
        } else {
            sorted_indices
        };

        // Return services in filtered order
        filtered_indices
            .iter()
            .filter_map(|&idx| self.services.get(idx))
            .collect()
    }

    /// Get filtered/selected users for export
    fn get_filtered_export_users(&self) -> Vec<&UserAccount> {
        // If multi-select mode with items selected, export only selected
        if self.multi_select_mode && !self.selected_items.is_empty() {
            return self
                .selected_items
                .iter()
                .filter_map(|&idx| self.users.get(idx))
                .collect();
        }

        // Get sorted and filtered indices
        let sorted_indices = self.get_sorted_user_indices();

        // Apply search filter if active
        let filtered_indices: Vec<usize> = if self.is_searching() && !self.search_query.is_empty() {
            sorted_indices
                .into_iter()
                .filter(|&idx| {
                    let user = &self.users[idx];
                    user.username
                        .to_lowercase()
                        .contains(&self.search_query.to_lowercase())
                        || user.uid.contains(&self.search_query)
                        || user
                            .shell
                            .to_lowercase()
                            .contains(&self.search_query.to_lowercase())
                        || user
                            .home
                            .to_lowercase()
                            .contains(&self.search_query.to_lowercase())
                })
                .collect()
        } else {
            sorted_indices
        };

        // Return users in filtered order
        filtered_indices
            .iter()
            .filter_map(|&idx| self.users.get(idx))
            .collect()
    }

    /// Get filtered/selected storage entries for export
    fn get_filtered_export_storage(&self) -> Vec<&(String, String, String)> {
        // If multi-select mode with items selected, export only selected
        if self.multi_select_mode && !self.selected_items.is_empty() {
            return self
                .selected_items
                .iter()
                .filter_map(|&idx| self.fstab.get(idx))
                .collect();
        }

        // Get sorted and filtered indices
        let sorted_indices = self.get_sorted_storage_indices();

        // Apply search filter if active
        let filtered_indices: Vec<usize> = if self.is_searching() && !self.search_query.is_empty() {
            sorted_indices
                .into_iter()
                .filter(|&idx| {
                    let (device, mountpoint, fstype) = &self.fstab[idx];
                    device
                        .to_lowercase()
                        .contains(&self.search_query.to_lowercase())
                        || mountpoint
                            .to_lowercase()
                            .contains(&self.search_query.to_lowercase())
                        || fstype
                            .to_lowercase()
                            .contains(&self.search_query.to_lowercase())
                })
                .collect()
        } else {
            sorted_indices
        };

        // Return fstab entries in filtered order
        filtered_indices
            .iter()
            .filter_map(|&idx| self.fstab.get(idx))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_file_size_bytes() {
        assert_eq!(format_file_size(0), "0 B");
        assert_eq!(format_file_size(500), "500 B");
        assert_eq!(format_file_size(1023), "1023 B");
    }

    #[test]
    fn test_format_file_size_kilobytes() {
        assert_eq!(format_file_size(1024), "1.00 KB");
        assert_eq!(format_file_size(1536), "1.50 KB");
        assert_eq!(format_file_size(2048), "2.00 KB");
        assert_eq!(format_file_size(10240), "10.00 KB");
    }

    #[test]
    fn test_format_file_size_megabytes() {
        const MB: i64 = 1024 * 1024;
        assert_eq!(format_file_size(MB), "1.00 MB");
        assert_eq!(format_file_size(MB + 512 * 1024), "1.50 MB");
        assert_eq!(format_file_size(10 * MB), "10.00 MB");
    }

    #[test]
    fn test_format_file_size_gigabytes() {
        const GB: i64 = 1024 * 1024 * 1024;
        assert_eq!(format_file_size(GB), "1.00 GB");
        assert_eq!(format_file_size(2 * GB), "2.00 GB");
        assert_eq!(format_file_size(GB + 512 * 1024 * 1024), "1.50 GB");
    }

    #[test]
    fn test_export_format_extension() {
        assert_eq!(ExportFormat::Json.extension(), "json");
        assert_eq!(ExportFormat::Yaml.extension(), "yaml");
        assert_eq!(ExportFormat::Html.extension(), "html");
        assert_eq!(ExportFormat::Pdf.extension(), "pdf");
    }

    #[test]
    fn test_export_format_name() {
        assert_eq!(ExportFormat::Json.name(), "JSON");
        assert_eq!(ExportFormat::Yaml.name(), "YAML");
        assert_eq!(ExportFormat::Html.name(), "HTML");
        assert_eq!(ExportFormat::Pdf.name(), "PDF");
    }

    #[test]
    fn test_export_format_equality() {
        assert_eq!(ExportFormat::Json, ExportFormat::Json);
        assert_ne!(ExportFormat::Json, ExportFormat::Yaml);
    }

    #[test]
    fn test_export_format_clone() {
        let format = ExportFormat::Json;
        let cloned = format;
        assert_eq!(format, cloned);
    }

    #[test]
    fn test_export_mode_equality() {
        assert_eq!(ExportMode::Selecting, ExportMode::Selecting);
        assert_eq!(ExportMode::EnteringFilename, ExportMode::EnteringFilename);
        assert_eq!(ExportMode::Exporting, ExportMode::Exporting);
        assert_ne!(ExportMode::Selecting, ExportMode::Exporting);
    }

    #[test]
    fn test_export_mode_success() {
        let mode = ExportMode::Success("test.json".to_string());
        match mode {
            ExportMode::Success(filename) => assert_eq!(filename, "test.json"),
            _ => panic!("Expected Success variant"),
        }
    }

    #[test]
    fn test_export_mode_error() {
        let mode = ExportMode::Error("Failed to export".to_string());
        match mode {
            ExportMode::Error(msg) => assert_eq!(msg, "Failed to export"),
            _ => panic!("Expected Error variant"),
        }
    }

    #[test]
    fn test_export_mode_clone() {
        let mode = ExportMode::Success("file.json".to_string());
        let cloned = mode.clone();
        assert_eq!(mode, cloned);
    }

    #[test]
    fn test_view_title() {
        assert_eq!(View::Dashboard.title(), "Dashboard");
        assert_eq!(View::Analytics.title(), "Analytics");
        assert_eq!(View::Timeline.title(), "Timeline");
        assert_eq!(View::Recommendations.title(), "Recommendations");
        assert_eq!(View::Topology.title(), "Topology");
        assert_eq!(View::Network.title(), "Network");
        assert_eq!(View::Packages.title(), "Packages");
        assert_eq!(View::Services.title(), "Services");
        assert_eq!(View::Databases.title(), "Databases");
        assert_eq!(View::WebServers.title(), "WebServers");
        assert_eq!(View::Security.title(), "Security");
        assert_eq!(View::Issues.title(), "Issues");
        assert_eq!(View::Storage.title(), "Storage");
        assert_eq!(View::Users.title(), "Users");
        assert_eq!(View::Kernel.title(), "Kernel");
        assert_eq!(View::Logs.title(), "Logs");
        assert_eq!(View::Profiles.title(), "Profiles");
        assert_eq!(View::Files.title(), "Files");
    }

    #[test]
    fn test_view_all() {
        let all_views = View::all();
        assert_eq!(all_views.len(), 21);
        assert!(all_views.contains(&View::Dashboard));
        assert!(all_views.contains(&View::Analytics));
        assert!(all_views.contains(&View::Files));
    }

    #[test]
    fn test_view_all_completeness() {
        let all_views = View::all();
        // Verify all views are included
        assert!(all_views.contains(&View::Dashboard));
        assert!(all_views.contains(&View::Analytics));
        assert!(all_views.contains(&View::Timeline));
        assert!(all_views.contains(&View::Recommendations));
        assert!(all_views.contains(&View::Topology));
        assert!(all_views.contains(&View::Network));
        assert!(all_views.contains(&View::Packages));
        assert!(all_views.contains(&View::Services));
        assert!(all_views.contains(&View::Databases));
        assert!(all_views.contains(&View::WebServers));
        assert!(all_views.contains(&View::Security));
        assert!(all_views.contains(&View::Issues));
        assert!(all_views.contains(&View::Storage));
        assert!(all_views.contains(&View::Users));
        assert!(all_views.contains(&View::Kernel));
        assert!(all_views.contains(&View::Logs));
        assert!(all_views.contains(&View::Profiles));
        assert!(all_views.contains(&View::Assurance));
        assert!(all_views.contains(&View::SystemdDeep));
        assert!(all_views.contains(&View::AiInsights));
        assert!(all_views.contains(&View::Files));
    }

    #[test]
    fn test_view_equality() {
        assert_eq!(View::Dashboard, View::Dashboard);
        assert_ne!(View::Dashboard, View::Analytics);
    }

    #[test]
    fn test_view_clone() {
        let view = View::Dashboard;
        let cloned = view;
        assert_eq!(view, cloned);
    }

    #[test]
    fn test_sort_mode_next_packages() {
        assert_eq!(SortMode::Default.next(&View::Packages), SortMode::NameAsc);
        assert_eq!(SortMode::NameAsc.next(&View::Packages), SortMode::NameDesc);
        assert_eq!(
            SortMode::NameDesc.next(&View::Packages),
            SortMode::VersionAsc
        );
        assert_eq!(
            SortMode::VersionAsc.next(&View::Packages),
            SortMode::VersionDesc
        );
        assert_eq!(
            SortMode::VersionDesc.next(&View::Packages),
            SortMode::Default
        );
    }

    #[test]
    fn test_sort_mode_next_services() {
        assert_eq!(SortMode::Default.next(&View::Services), SortMode::NameAsc);
        assert_eq!(SortMode::NameAsc.next(&View::Services), SortMode::NameDesc);
        assert_eq!(SortMode::NameDesc.next(&View::Services), SortMode::StateAsc);
        assert_eq!(
            SortMode::StateAsc.next(&View::Services),
            SortMode::StateDesc
        );
        assert_eq!(
            SortMode::StateDesc.next(&View::Services),
            SortMode::EnabledFirst
        );
        assert_eq!(
            SortMode::EnabledFirst.next(&View::Services),
            SortMode::Default
        );
    }

    #[test]
    fn test_sort_mode_next_users() {
        assert_eq!(SortMode::Default.next(&View::Users), SortMode::NameAsc);
        assert_eq!(SortMode::NameAsc.next(&View::Users), SortMode::NameDesc);
        assert_eq!(SortMode::NameDesc.next(&View::Users), SortMode::UidAsc);
        assert_eq!(SortMode::UidAsc.next(&View::Users), SortMode::UidDesc);
        assert_eq!(SortMode::UidDesc.next(&View::Users), SortMode::Default);
    }

    #[test]
    fn test_sort_mode_next_storage() {
        assert_eq!(SortMode::Default.next(&View::Storage), SortMode::NameAsc);
        assert_eq!(SortMode::NameAsc.next(&View::Storage), SortMode::NameDesc);
        assert_eq!(SortMode::NameDesc.next(&View::Storage), SortMode::SizeAsc);
        assert_eq!(SortMode::SizeAsc.next(&View::Storage), SortMode::SizeDesc);
        assert_eq!(SortMode::SizeDesc.next(&View::Storage), SortMode::Default);
    }

    #[test]
    fn test_sort_mode_next_default_view() {
        // Dashboard should cycle through basic sort modes
        assert_eq!(SortMode::Default.next(&View::Dashboard), SortMode::NameAsc);
        assert_eq!(SortMode::NameAsc.next(&View::Dashboard), SortMode::NameDesc);
        assert_eq!(SortMode::NameDesc.next(&View::Dashboard), SortMode::Default);
    }

    #[test]
    fn test_sort_mode_next_invalid_returns_default() {
        // Testing invalid sort mode for a view should return Default
        assert_eq!(SortMode::SizeAsc.next(&View::Packages), SortMode::Default);
        assert_eq!(SortMode::UidAsc.next(&View::Services), SortMode::Default);
    }

    #[test]
    fn test_sort_mode_equality() {
        assert_eq!(SortMode::Default, SortMode::Default);
        assert_eq!(SortMode::NameAsc, SortMode::NameAsc);
        assert_ne!(SortMode::NameAsc, SortMode::NameDesc);
    }

    #[test]
    fn test_sort_mode_clone() {
        let mode = SortMode::NameAsc;
        let cloned = mode;
        assert_eq!(mode, cloned);
    }
}
