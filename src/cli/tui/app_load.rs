// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Staged inspection loading, lazy views, and refresh for the TUI.

use super::app::{App, CompareSummary, IssueRiskFilter, LayoutMode, View};
use super::cache::{self, InspectCacheSnapshot, CACHE_VERSION};
use super::icons;
use super::loading::{LoadingStage, LoadingState};
use super::palette::PaletteAction;
use crate::cli::profiles::{
    ComplianceProfile, HardeningProfile, InspectionProfile, MigrationProfile, PerformanceProfile,
    SecurityProfile,
};
use crate::guestfs::inspect_enhanced::{FirewallInfo, PackageInfo, SecurityInfo};
use crate::Guestfs;
use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

impl View {
    pub fn name(self) -> &'static str {
        match self {
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
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "dashboard" => Some(View::Dashboard),
            "analytics" => Some(View::Analytics),
            "timeline" => Some(View::Timeline),
            "recommendations" => Some(View::Recommendations),
            "topology" => Some(View::Topology),
            "network" => Some(View::Network),
            "packages" => Some(View::Packages),
            "services" => Some(View::Services),
            "databases" => Some(View::Databases),
            "webservers" | "web_servers" => Some(View::WebServers),
            "security" => Some(View::Security),
            "issues" => Some(View::Issues),
            "storage" => Some(View::Storage),
            "users" => Some(View::Users),
            "kernel" => Some(View::Kernel),
            "logs" => Some(View::Logs),
            "profiles" => Some(View::Profiles),
            "assurance" | "doctor" => Some(View::Assurance),
            "systemd-deep" | "systemd_deep" | "systemd" => Some(View::SystemdDeep),
            "ai-insights" | "ai_insights" | "ai" => Some(View::AiInsights),
            "files" => Some(View::Files),
            _ => None,
        }
    }

    pub fn group(self) -> &'static str {
        match self {
            View::Dashboard
            | View::Analytics
            | View::Timeline
            | View::Recommendations
            | View::Topology => "Overview",
            View::Network
            | View::Packages
            | View::Services
            | View::Databases
            | View::WebServers
            | View::Users
            | View::Kernel
            | View::Logs
            | View::Storage
            | View::Files => "System",
            View::Security
            | View::Issues
            | View::Profiles
            | View::Assurance
            | View::SystemdDeep
            | View::AiInsights => "Security",
        }
    }

    pub fn all_groups() -> &'static [&'static str] {
        &["Overview", "System", "Security"]
    }

    pub fn views_in_group(group: &str) -> &'static [View] {
        match group {
            "Overview" => &[
                View::Dashboard,
                View::Analytics,
                View::Timeline,
                View::Recommendations,
                View::Topology,
            ],
            "System" => &[
                View::Network,
                View::Packages,
                View::Services,
                View::Databases,
                View::WebServers,
                View::Users,
                View::Kernel,
                View::Logs,
                View::Storage,
                View::Files,
            ],
            "Security" => &[
                View::Security,
                View::Issues,
                View::Profiles,
                View::Assurance,
                View::SystemdDeep,
                View::AiInsights,
            ],
            _ => &[],
        }
    }

    /// Heavy views loaded on first visit when lazy loading is active.
    pub fn is_lazy(self) -> bool {
        matches!(
            self,
            View::Packages
                | View::Kernel
                | View::Databases
                | View::WebServers
                | View::Logs
                | View::Analytics
                | View::Timeline
                | View::Recommendations
                | View::Topology
        )
    }
}

impl App {
    /// Fast path: mount image and read OS metadata only.
    pub fn bootstrap(
        image_path: &Path,
        compare_path: Option<&Path>,
        fleet_paths: Vec<PathBuf>,
    ) -> Result<Self> {
        let config = super::config::TuiConfig::load();
        let mut guestfs = Guestfs::new()?;
        guestfs.add_drive_ro(image_path)?;
        guestfs.launch()?;

        let roots = guestfs.inspect_os()?;
        let root = roots
            .first()
            .ok_or_else(|| anyhow::anyhow!("No operating systems found in image"))?
            .clone();

        guestfs.mount_ro(&root, "/")?;

        let os_name = guestfs
            .inspect_get_product_name(&root)
            .unwrap_or_else(|_| "Unknown".to_string());
        let os_version = guestfs
            .inspect_get_product_variant(&root)
            .unwrap_or_else(|_| "Unknown".to_string());
        let hostname = guestfs
            .inspect_get_hostname(&root)
            .unwrap_or_else(|_| "Unknown".to_string());
        let kernel_version = if let (Ok(major), Ok(minor)) = (
            guestfs.inspect_get_major_version(&root),
            guestfs.inspect_get_minor_version(&root),
        ) {
            format!("{}.{}", major, minor)
        } else {
            "Unknown".to_string()
        };
        let architecture = guestfs
            .inspect_get_arch(&root)
            .unwrap_or_else(|_| "Unknown".to_string());
        let init_system = guestfs
            .inspect_init_system(&root)
            .unwrap_or_else(|_| "unknown".to_string());
        let timezone = guestfs
            .inspect_timezone(&root)
            .unwrap_or_else(|_| "unknown".to_string());
        let locale = guestfs
            .inspect_locale(&root)
            .unwrap_or_else(|_| "unknown".to_string());

        let layout_mode = match config.views.default_layout.as_str() {
            "list" => LayoutMode::ListOnly,
            "detail" => LayoutMode::DetailFull,
            _ => LayoutMode::SplitDetail,
        };

        let current_view =
            View::from_name(&config.behavior.default_view).unwrap_or(View::Dashboard);

        let mut loaded_views = HashSet::new();
        loaded_views.insert(View::Dashboard);

        let mut app = Self::empty_shell(
            image_path,
            compare_path,
            config,
            guestfs,
            root,
            os_name,
            os_version,
            hostname,
            kernel_version,
            architecture,
            init_system,
            timezone,
            locale,
            current_view,
            layout_mode,
            loaded_views,
        );

        app.fleet_images = fleet_paths
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        app.fleet_index = fleet_paths
            .iter()
            .position(|p| p.as_path() == image_path)
            .unwrap_or(0);

        if let Ok(Some(snap)) = cache::load_snapshot(image_path) {
            app.apply_cache_snapshot(snap);
            app.loading = None;
            app.last_updated = chrono::Local::now();
            app.show_notification("Loaded inspect data from cache".to_string());
        } else {
            app.loading = Some(LoadingState::new());
        }
        Ok(app)
    }

    fn apply_cache_snapshot(&mut self, snap: InspectCacheSnapshot) {
        self.os_name = snap.os_name;
        self.os_version = snap.os_version;
        self.hostname = snap.hostname;
        self.kernel_version = snap.kernel_version;
        self.architecture = snap.architecture;
        self.init_system = snap.init_system;
        self.timezone = snap.timezone;
        self.locale = snap.locale;
        self.network_interfaces = snap.network_interfaces;
        self.dns_servers = snap.dns_servers;
        self.packages = snap.packages;
        self.services = snap.services;
        self.firewall = snap.firewall;
        self.security = snap.security;
        self.users = snap.users;
        self.security_profile = snap.security_profile;
        self.compliance_profile = snap.compliance_profile;
        self.hardening_profile = snap.hardening_profile;
        for v in View::all() {
            self.loaded_views.insert(*v);
        }
    }

    fn build_cache_snapshot(&self, image_path: &Path) -> Result<InspectCacheSnapshot> {
        let mtime = std::fs::metadata(image_path)
            .ok()
            .and_then(|m| {
                m.modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
            })
            .unwrap_or(0);
        Ok(InspectCacheSnapshot {
            version: CACHE_VERSION,
            image_mtime: mtime,
            os_name: self.os_name.clone(),
            os_version: self.os_version.clone(),
            hostname: self.hostname.clone(),
            kernel_version: self.kernel_version.clone(),
            architecture: self.architecture.clone(),
            init_system: self.init_system.clone(),
            timezone: self.timezone.clone(),
            locale: self.locale.clone(),
            network_interfaces: self.network_interfaces.clone(),
            dns_servers: self.dns_servers.clone(),
            packages: self.packages.clone(),
            services: self.services.clone(),
            firewall: self.firewall.clone(),
            security: self.security.clone(),
            users: self.users.clone(),
            security_profile: self.security_profile.clone(),
            compliance_profile: self.compliance_profile.clone(),
            hardening_profile: self.hardening_profile.clone(),
        })
    }

    pub fn fleet_next(&mut self) -> Result<()> {
        if self.fleet_images.len() <= 1 {
            return Ok(());
        }
        let next = (self.fleet_index + 1) % self.fleet_images.len();
        self.switch_fleet_index(next)
    }

    pub fn fleet_previous(&mut self) -> Result<()> {
        if self.fleet_images.len() <= 1 {
            return Ok(());
        }
        let prev = if self.fleet_index == 0 {
            self.fleet_images.len() - 1
        } else {
            self.fleet_index - 1
        };
        self.switch_fleet_index(prev)
    }

    pub fn switch_fleet_index(&mut self, index: usize) -> Result<()> {
        if index >= self.fleet_images.len() {
            return Ok(());
        }
        let fleet_paths: Vec<PathBuf> = self.fleet_images.iter().map(PathBuf::from).collect();
        let compare = self.compare_image_path.clone();
        let path = fleet_paths[index].clone();
        let label = super::fleet::fleet_label(&path);

        self.cleanup()?;
        *self = Self::bootstrap(&path, compare.as_deref(), fleet_paths)?;
        while self.loading.is_some() {
            self.advance_loading()?;
        }
        self.show_notification(format!("Fleet → {}", label));
        Ok(())
    }

    /// Advance one loading stage; returns true when fully loaded.
    pub fn advance_loading(&mut self) -> Result<bool> {
        if self.loading.is_none() {
            return Ok(true);
        }

        let stage = self
            .loading
            .as_ref()
            .map(|l| l.stage)
            .unwrap_or(LoadingStage::Done);
        if stage == LoadingStage::Done {
            self.loading = None;
            return Ok(true);
        }

        let root = self.inspect_root.clone();

        match stage {
            LoadingStage::Bootstrap => {
                self.set_loading_stage(LoadingStage::NetworkSecurity);
            }
            LoadingStage::NetworkSecurity => {
                if let Some(ref mut gfs) = self.guestfs {
                    self.network_interfaces = gfs.inspect_network(&root).unwrap_or_default();
                    self.dns_servers = gfs.inspect_dns(&root).unwrap_or_default();
                    self.firewall = gfs
                        .inspect_firewall(&root)
                        .unwrap_or_else(|_| FirewallInfo {
                            firewall_type: "none".to_string(),
                            enabled: false,
                            rules_count: 0,
                            zones: Vec::new(),
                        });
                    self.security = gfs
                        .inspect_security(&root)
                        .unwrap_or_else(|_| SecurityInfo {
                            selinux: "unknown".to_string(),
                            apparmor: false,
                            fail2ban: false,
                            aide: false,
                            auditd: false,
                            ssh_keys: Vec::new(),
                        });
                    self.users = gfs.inspect_users(&root).unwrap_or_default();
                }
                self.loaded_views.insert(View::Network);
                self.loaded_views.insert(View::Users);
                self.loaded_views.insert(View::Security);
                self.set_loading_stage(LoadingStage::PackagesServices);
            }
            LoadingStage::PackagesServices => {
                if let Some(ref mut gfs) = self.guestfs {
                    self.packages = gfs.inspect_packages(&root).unwrap_or_else(|_| PackageInfo {
                        manager: "unknown".to_string(),
                        package_count: 0,
                        packages: Vec::new(),
                    });
                    self.services = gfs.inspect_systemd_services(&root).unwrap_or_default();
                    self.databases = gfs.inspect_databases(&root).unwrap_or_default();
                    self.web_servers = gfs.inspect_web_servers(&root).unwrap_or_default();
                }
                self.loaded_views.insert(View::Packages);
                self.loaded_views.insert(View::Services);
                self.loaded_views.insert(View::Databases);
                self.loaded_views.insert(View::WebServers);
                self.set_loading_stage(LoadingStage::Profiles);
            }
            LoadingStage::Profiles => {
                if let Some(ref mut gfs) = self.guestfs {
                    self.security_profile = SecurityProfile.inspect(gfs, &root).ok();
                    self.migration_profile = MigrationProfile.inspect(gfs, &root).ok();
                    self.performance_profile = PerformanceProfile.inspect(gfs, &root).ok();
                    self.compliance_profile = ComplianceProfile.inspect(gfs, &root).ok();
                    self.hardening_profile = HardeningProfile.inspect(gfs, &root).ok();
                }
                self.loaded_views.insert(View::Profiles);
                self.loaded_views.insert(View::Issues);
                self.set_loading_stage(LoadingStage::StorageKernel);
            }
            LoadingStage::StorageKernel => {
                if let Some(ref mut gfs) = self.guestfs {
                    self._hosts = gfs.inspect_hosts(&root).unwrap_or_default();
                    self.fstab = gfs.inspect_fstab(&root).unwrap_or_default();
                    self.lvm_info = gfs.inspect_lvm(&root).ok();
                    self.raid_arrays = gfs.inspect_raid(&root).unwrap_or_default();
                    self.kernel_modules = gfs.inspect_kernel_modules(&root).unwrap_or_default();
                    self.kernel_params = gfs.inspect_kernel_params(&root).unwrap_or_default();
                }
                self.loaded_views.insert(View::Storage);
                self.loaded_views.insert(View::Kernel);
                self.set_loading_stage(LoadingStage::CompareImage);
            }
            LoadingStage::CompareImage => {
                if let Some(compare) = self.compare_image_path.clone() {
                    self.load_compare_image(&compare)?;
                }
                self.loading = None;
                self.last_updated = chrono::Local::now();
                if let Ok(snap) = self.build_cache_snapshot(&self._image_path_buf) {
                    let _ = cache::save_snapshot(&self._image_path_buf, &snap);
                }
                if self.config.behavior.assurance_on_startup {
                    let _ = self.load_assurance();
                }
                if self.config.behavior.show_assurance_hint {
                    self.show_notification(
                        "Press : for palette · Ctrl+P jump · d in Assurance for doctor".to_string(),
                    );
                    self.config.behavior.show_assurance_hint = false;
                    let _ = self.config.save();
                }
            }
            LoadingStage::Done => {}
        }

        Ok(self.loading.is_none())
    }

    fn set_loading_stage(&mut self, stage: LoadingStage) {
        if let Some(loading) = &mut self.loading {
            loading.stage = stage;
            loading.message = stage.label().to_string();
        }
    }

    fn load_compare_image(&mut self, path: &Path) -> Result<()> {
        let mut gfs = Guestfs::new()?;
        gfs.add_drive_ro(path)?;
        gfs.launch()?;
        let roots = gfs.inspect_os()?;
        let root = roots
            .first()
            .ok_or_else(|| anyhow::anyhow!("No OS in compare image"))?;
        gfs.mount_ro(root, "/")?;

        let os_name = gfs
            .inspect_get_product_name(root)
            .unwrap_or_else(|_| "Unknown".to_string());
        let hostname = gfs
            .inspect_get_hostname(root)
            .unwrap_or_else(|_| "Unknown".to_string());
        let packages = gfs.inspect_packages(root).ok();
        let pkg_count = packages.as_ref().map(|p| p.package_count).unwrap_or(0);
        let (critical, high, medium) = {
            let sec = SecurityProfile.inspect(&mut gfs, root).ok();
            let mut c = 0usize;
            let mut h = 0usize;
            let mut m = 0usize;
            if let Some(report) = sec {
                for section in &report.sections {
                    for f in &section.findings {
                        match f.risk_level {
                            Some(crate::cli::profiles::RiskLevel::Critical) => c += 1,
                            Some(crate::cli::profiles::RiskLevel::High) => h += 1,
                            Some(crate::cli::profiles::RiskLevel::Medium) => m += 1,
                            _ => {}
                        }
                    }
                }
            }
            (c, h, m)
        };
        let _ = gfs.shutdown();

        self.compare_summary = Some(CompareSummary {
            path: path.display().to_string(),
            os_name,
            hostname,
            package_count: pkg_count,
            critical,
            high,
            medium,
        });
        Ok(())
    }

    pub fn ensure_view_loaded(&mut self, view: View) -> Result<()> {
        if !view.is_lazy() || self.loaded_views.contains(&view) {
            return Ok(());
        }
        let root = self.inspect_root.clone();
        if let Some(ref mut gfs) = self.guestfs {
            match view {
                View::Packages => {
                    self.packages = gfs.inspect_packages(&root).unwrap_or_else(|_| PackageInfo {
                        manager: "unknown".to_string(),
                        package_count: 0,
                        packages: Vec::new(),
                    });
                }
                View::Kernel => {
                    self.kernel_modules = gfs.inspect_kernel_modules(&root).unwrap_or_default();
                    self.kernel_params = gfs.inspect_kernel_params(&root).unwrap_or_default();
                }
                View::Databases => {
                    self.databases = gfs.inspect_databases(&root).unwrap_or_default();
                }
                View::WebServers => {
                    self.web_servers = gfs.inspect_web_servers(&root).unwrap_or_default();
                }
                View::Analytics
                | View::Timeline
                | View::Recommendations
                | View::Topology
                | View::Logs => {}
                _ => {}
            }
        }
        self.loaded_views.insert(view);
        Ok(())
    }

    pub fn reload_current_view(&mut self, full: bool) -> Result<()> {
        self.refreshing = true;
        let root = self.inspect_root.clone();
        if full {
            self.loaded_views.clear();
            self.loaded_views.insert(View::Dashboard);
            if let Some(ref mut gfs) = self.guestfs {
                self.packages = gfs.inspect_packages(&root).unwrap_or_else(|_| PackageInfo {
                    manager: "unknown".to_string(),
                    package_count: 0,
                    packages: Vec::new(),
                });
                self.services = gfs.inspect_systemd_services(&root).unwrap_or_default();
                self.security_profile = SecurityProfile.inspect(gfs, &root).ok();
                self.loaded_views.insert(View::Packages);
                self.loaded_views.insert(View::Services);
                self.loaded_views.insert(View::Profiles);
                self.loaded_views.insert(View::Issues);
            }
        } else {
            let view = self.current_view;
            self.loaded_views.remove(&view);
            self.ensure_view_loaded(view)?;
            if let Some(ref mut gfs) = self.guestfs {
                match view {
                    View::Packages => {
                        self.packages =
                            gfs.inspect_packages(&root).unwrap_or_else(|_| PackageInfo {
                                manager: "unknown".to_string(),
                                package_count: 0,
                                packages: Vec::new(),
                            });
                    }
                    View::Services => {
                        self.services = gfs.inspect_systemd_services(&root).unwrap_or_default();
                    }
                    View::Network => {
                        self.network_interfaces = gfs.inspect_network(&root).unwrap_or_default();
                    }
                    View::Issues | View::Profiles => {
                        self.security_profile = SecurityProfile.inspect(gfs, &root).ok();
                        self.compliance_profile = ComplianceProfile.inspect(gfs, &root).ok();
                        self.hardening_profile = HardeningProfile.inspect(gfs, &root).ok();
                    }
                    View::Files => {
                        self.init_file_browser();
                    }
                    _ => {}
                }
            }
            self.loaded_views.insert(view);
        }
        self.refreshing = false;
        self.last_updated = chrono::Local::now();
        Ok(())
    }

    pub fn run_palette_action(&mut self, action: PaletteAction) {
        match action {
            PaletteAction::Goto(v) => {
                self.set_view(v);
            }
            PaletteAction::AssuranceRun => {
                self.set_view(View::Assurance);
                let _ = self.load_assurance();
            }
            PaletteAction::MigratePlan => {
                let _ = self.load_assurance();
                self.show_notification(format!(
                    "Migration score: {:.0}% → {}",
                    self.migration_report
                        .as_ref()
                        .map(|m| m.score)
                        .unwrap_or(0.0),
                    self.assurance_target
                ));
            }
            PaletteAction::ExportFixPlan => match self.export_assurance_plan() {
                Ok(path) => self.show_notification(format!("Exported {}", path)),
                Err(e) => self.show_notification(format!("Export failed: {e}")),
            },
            PaletteAction::ApplyFixPlan => match self.apply_assurance_plan(false) {
                Ok(msg) => self.show_notification(msg),
                Err(e) => self.show_notification(format!("Apply failed: {e:#}")),
            },
            PaletteAction::ApplyFixPlanDryRun => match self.apply_assurance_plan(true) {
                Ok(msg) => self.show_notification(msg),
                Err(e) => self.show_notification(format!("Dry-run failed: {e:#}")),
            },
            PaletteAction::PlanPreview => {
                self.toggle_plan_preview();
            }
            PaletteAction::ExportJson => {
                self.current_view = View::Dashboard;
                self.toggle_export_menu();
                self.select_export_format(super::app::ExportFormat::Json);
            }
            PaletteAction::ExportHtml => {
                self.current_view = View::Issues;
                self.toggle_export_menu();
                self.select_export_format(super::app::ExportFormat::Html);
            }
            PaletteAction::Refresh => {
                let _ = self.reload_current_view(false);
                self.show_notification("View refreshed".to_string());
            }
            PaletteAction::RefreshFull => {
                let _ = self.reload_current_view(true);
                self.show_notification("Full re-inspect complete".to_string());
            }
            PaletteAction::CompareToggle => self.toggle_comparison_mode(),
            PaletteAction::PinView => self.pin_current_view(),
            PaletteAction::Help => self.toggle_help(),
            PaletteAction::Unknown => {
                self.show_notification("Unknown command — try 'goto issues'".to_string());
            }
        }
    }

    pub fn set_view(&mut self, view: View) {
        let _ = self.ensure_view_loaded(view);
        self.current_view = view;
        self.scroll_offset = 0;
        self.selected_index = 0;
        if view == View::Files {
            self.init_file_browser();
        }
        if view == View::Assurance && self.boot_report.is_none() {
            let _ = self.load_assurance();
        }
        self.ensure_view_tab_scroll_visible();
    }

    /// Run doctor + migration scoring using the mounted guest (or CLI fallback).
    pub fn load_assurance(&mut self) -> Result<()> {
        use crate::boot::{analyze_bootability, BootTarget};
        use crate::cli::commands::assurance::collect_assurance_data;
        use crate::cli::migrate::plan::compute_migration_score;
        use crate::evidence::build_evidence;

        self.refreshing = true;
        let target_str = self.assurance_target.clone();
        let boot_target = BootTarget::parse(&target_str);
        let image_path = &self._image_path_buf;

        let result = if let Some(ref mut gfs) = self.guestfs {
            let root = self.inspect_root.clone();
            build_evidence(gfs, &root, image_path).map(|evidence| {
                let boot_report = analyze_bootability(&evidence, boot_target);
                let migration_report =
                    compute_migration_score(&evidence, &boot_report, &target_str);
                (evidence, boot_report, migration_report)
            })
        } else {
            collect_assurance_data(image_path, boot_target, false).map(|(evidence, boot_report)| {
                let migration_report =
                    compute_migration_score(&evidence, &boot_report, &target_str);
                (evidence, boot_report, migration_report)
            })
        };

        self.refreshing = false;
        match result {
            Ok((evidence, boot_report, migration_report)) => {
                self.intelligence = Some(crate::ai::build_intelligence(
                    &evidence,
                    Some(&boot_report),
                    None,
                ));
                self.assurance_evidence = Some(evidence);
                self.boot_report = Some(boot_report);
                self.migration_report = Some(migration_report);
                self.plan_preview = None;
                self.show_plan_preview = false;
                #[cfg(feature = "agent")]
                {
                    self.agent_live = std::env::var("GUESTKIT_AGENT_SOCKET")
                        .map(|s| crate::agent::ping_agent_socket(&s))
                        .unwrap_or(false);
                }
                self.show_notification(format!("Assurance updated (target: {target_str})"));
                Ok(())
            }
            Err(e) => {
                self.show_notification(format!("Assurance failed: {e:#}"));
                Err(e)
            }
        }
    }

    pub fn cycle_assurance_target(&mut self) {
        const TARGETS: &[&str] = &["kvm", "proxmox", "aws"];
        let idx = TARGETS
            .iter()
            .position(|t| *t == self.assurance_target)
            .unwrap_or(0);
        self.assurance_target = TARGETS[(idx + 1) % TARGETS.len()].to_string();
        self.boot_report = None;
        self.migration_report = None;
        self.assurance_evidence = None;
        self.intelligence = None;
        self.plan_preview = None;
        self.show_plan_preview = false;
        self.show_notification(format!("Target → {}", self.assurance_target));
        let _ = self.load_assurance();
    }

    pub fn build_assurance_fix_plan(&mut self) -> Result<crate::cli::plan::types::FixPlan> {
        use crate::cli::plan::PlanGenerator;

        let migration = self
            .migration_report
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Run assurance first (d)"))?;
        let boot = self
            .boot_report
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Run assurance first (d)"))?;
        let generator = PlanGenerator::new(self.image_path.clone());
        let plan = generator.from_migration_report(
            migration,
            boot,
            &self.assurance_target,
            &self._image_path_buf,
        )?;
        self.plan_preview = Some(plan.clone());
        Ok(plan)
    }

    pub fn export_assurance_plan(&mut self) -> Result<String> {
        use crate::cli::plan::PlanExporter;

        let plan = self.build_assurance_fix_plan()?;
        let path = format!(
            "guestkit-fix-plan-{}-{}.yaml",
            self.hostname.replace('.', "-"),
            self.assurance_target
        );
        let content = PlanExporter::to_yaml(&plan)?;
        std::fs::write(&path, content)?;
        Ok(format!("{path} ({} operations)", plan.operations.len()))
    }

    pub fn apply_assurance_plan(&mut self, dry_run: bool) -> Result<String> {
        use crate::cli::plan::PlanApplicator;

        let plan = self.build_assurance_fix_plan()?;
        let applicator = PlanApplicator::new(self.image_path.clone(), dry_run);
        let result = applicator.apply(&plan)?;
        Ok(format!(
            "{} — applied {} · failed {} · skipped {} ({})",
            if result.success { "OK" } else { "FAILED" },
            result.operations_applied,
            result.operations_failed,
            result.operations_skipped,
            result.message
        ))
    }

    pub fn go_to_assurance(&mut self) {
        self.set_view(View::Assurance);
        if self.boot_report.is_none() {
            let _ = self.load_assurance();
        }
    }

    pub fn toggle_plan_preview(&mut self) {
        if self.show_plan_preview {
            self.close_plan_preview();
            return;
        }
        match self.build_assurance_fix_plan() {
            Ok(plan) => {
                let n = plan.operations.len();
                self.plan_preview_scroll = 0;
                self.show_plan_preview = true;
                self.show_notification(format!("Fix plan preview ({n} operations, read-only)"));
            }
            Err(e) => self.show_notification(format!("Plan preview: {e:#}")),
        }
    }

    pub fn close_plan_preview(&mut self) {
        self.show_plan_preview = false;
    }

    pub fn plan_preview_scroll_up(&mut self) {
        self.plan_preview_scroll = self.plan_preview_scroll.saturating_sub(1);
    }

    pub fn plan_preview_scroll_down(&mut self) {
        if let Some(ref plan) = self.plan_preview {
            let max = plan.operations.len().saturating_sub(1);
            if self.plan_preview_scroll < max {
                self.plan_preview_scroll += 1;
            }
        }
    }

    pub fn view_tab_scroll_left(&mut self) {
        self.view_tab_scroll = self.view_tab_scroll.saturating_sub(1);
    }

    pub fn view_tab_scroll_right(&mut self) {
        let max = self.view_tab_entries().len().saturating_sub(1);
        if self.view_tab_scroll < max {
            self.view_tab_scroll += 1;
        }
    }

    /// Keep the active view tab visible in the tab row.
    pub fn ensure_view_tab_scroll_visible(&mut self) {
        let entries = self.view_tab_entries();
        if entries.is_empty() {
            self.view_tab_scroll = 0;
            return;
        }
        let active_idx = entries
            .iter()
            .position(|(v, _)| *v == self.current_view)
            .unwrap_or(0);
        let visible = self.visible_view_tab_count();
        if active_idx < self.view_tab_scroll {
            self.view_tab_scroll = active_idx;
        } else if visible > 0 && active_idx >= self.view_tab_scroll + visible {
            self.view_tab_scroll = active_idx + 1 - visible;
        }
    }

    fn visible_view_tab_count(&self) -> usize {
        let entries = self.view_tab_entries();
        if entries.is_empty() {
            return 0;
        }
        let width = self.terminal_width.saturating_sub(4) as usize;
        let mut used = 0usize;
        let mut count = 0usize;
        for (_, title) in entries.iter().skip(self.view_tab_scroll) {
            let w = title.chars().count() + 3;
            if used + w > width && count > 0 {
                break;
            }
            used += w;
            count += 1;
        }
        count.max(1)
    }

    pub fn pin_current_view(&mut self) {
        let name = self.current_view.name().to_string();
        if !self.pinned_views.contains(&name) {
            self.pinned_views.push(name.clone());
            if !self.config.views.pinned.contains(&name) {
                self.config.views.pinned.push(name);
                if let Err(e) = self.config.save() {
                    self.show_notification(format!(
                        "Pin saved locally; config write failed: {}",
                        e
                    ));
                } else {
                    self.show_notification(format!(
                        "Pinned {} (saved to tui.toml)",
                        self.current_view.title()
                    ));
                }
            } else {
                self.show_notification(format!("Pinned {}", self.current_view.title()));
            }
        }
    }

    pub fn cycle_layout_mode(&mut self) {
        self.layout_mode = match self.layout_mode {
            LayoutMode::ListOnly => LayoutMode::SplitDetail,
            LayoutMode::SplitDetail => LayoutMode::DetailFull,
            LayoutMode::DetailFull => LayoutMode::ListOnly,
        };
        let label = match self.layout_mode {
            LayoutMode::ListOnly => "List",
            LayoutMode::SplitDetail => "Split",
            LayoutMode::DetailFull => "Detail",
        };
        self.show_notification(format!("Layout: {}", label));
    }

    pub fn cycle_issue_filter(&mut self) {
        self.issue_filter = match self.issue_filter {
            IssueRiskFilter::All => IssueRiskFilter::Critical,
            IssueRiskFilter::Critical => IssueRiskFilter::High,
            IssueRiskFilter::High => IssueRiskFilter::Medium,
            IssueRiskFilter::Medium => IssueRiskFilter::All,
        };
        self.scroll_offset = 0;
        let label = match self.issue_filter {
            IssueRiskFilter::All => "All",
            IssueRiskFilter::Critical => "Critical",
            IssueRiskFilter::High => "High",
            IssueRiskFilter::Medium => "Medium",
        };
        self.show_notification(format!("Filter: {}", label));
    }

    pub fn current_group(&self) -> &'static str {
        self.current_view.group()
    }

    pub fn views_in_current_group(&self) -> Vec<View> {
        View::views_in_group(self.current_group()).to_vec()
    }

    pub fn pinned_view_list(&self) -> Vec<View> {
        self.pinned_views
            .iter()
            .filter_map(|name| View::from_name(name))
            .collect()
    }

    pub fn is_compact_tabs(&self) -> bool {
        self.config.ui.density == "compact"
            || self.terminal_width < self.config.ui.auto_compact_width
    }

    pub fn set_group(&mut self, group: &str) {
        let views = View::views_in_group(group);
        if views.is_empty() {
            return;
        }
        if self.current_view.group() != group {
            self.set_view(views[0]);
            self.show_notification(format!("→ {} group", group));
        }
    }

    pub fn next_group(&mut self) {
        let groups = View::all_groups();
        let idx = groups
            .iter()
            .position(|g| *g == self.current_group())
            .unwrap_or(0);
        let next = groups[(idx + 1) % groups.len()];
        if let Some(&view) = View::views_in_group(next).first() {
            self.set_view(view);
            self.show_notification(format!("→ {} group", next));
        }
    }

    pub fn previous_group(&mut self) {
        let groups = View::all_groups();
        let idx = groups
            .iter()
            .position(|g| *g == self.current_group())
            .unwrap_or(0);
        let prev = groups[(idx + groups.len() - 1) % groups.len()];
        if let Some(&view) = View::views_in_group(prev).first() {
            self.set_view(view);
            self.show_notification(format!("← {} group", prev));
        }
    }

    /// Views shown in the view tab row: pinned quick-access, then current group (deduped).
    pub fn view_tab_entries(&self) -> Vec<(View, String)> {
        let compact = self.is_compact_tabs();
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for v in self.pinned_view_list() {
            if seen.insert(v) {
                out.push((v, self.tab_title_for(v, compact, true)));
            }
        }
        for &v in View::views_in_group(self.current_group()) {
            if seen.insert(v) {
                out.push((v, self.tab_title_for(v, compact, false)));
            }
        }
        out
    }

    /// Y positions of group/view tab rows (content area, 0-based).
    pub fn tab_row_y(&self) -> (u16, u16) {
        let group_y = if self.show_stats_bar { 5 } else { 3 };
        (group_y, group_y + 2)
    }

    pub fn tab_titles_ordered(&self) -> Vec<(View, String)> {
        let all = View::all();
        let mut pinned: Vec<View> = Vec::new();
        let mut rest: Vec<View> = Vec::new();
        for v in all {
            if self.pinned_views.iter().any(|p| p == v.name()) {
                pinned.push(*v);
            } else {
                rest.push(*v);
            }
        }
        let ordered: Vec<View> = pinned.into_iter().chain(rest).collect();
        ordered
            .into_iter()
            .map(|v| {
                let title = self.tab_title_for(v, self.is_compact_tabs(), false);
                (v, title)
            })
            .collect()
    }

    pub fn tab_title_for(&self, v: View, compact: bool, pinned: bool) -> String {
        let ascii = self.config.ui.icon_mode == "ascii";
        let icon = icons::view_icon(v, ascii);
        if compact {
            let prefix = if pinned { "★" } else { "" };
            return format!("{}{}", prefix, icon);
        }
        let count = match v {
            View::Network => Some(self.network_interfaces.len()),
            View::Packages => Some(self.packages.package_count),
            View::Services => Some(self.services.len()),
            View::Issues => {
                let (c, h, m) = self.get_risk_summary();
                let t = c + h + m;
                if t > 0 {
                    Some(t)
                } else {
                    None
                }
            }
            View::Assurance => self.boot_report.as_ref().map(|b| b.score.round() as usize),
            _ => None,
        };
        let prefix = if pinned { "★" } else { "" };
        if let Some(n) = count {
            format!("{}{}{} ({})", prefix, icon, v.title(), n)
        } else {
            format!("{}{}{}", prefix, icon, v.title())
        }
    }

    pub fn export_migration_bundle(&mut self) -> Result<String> {
        let out = format!(
            r#"{{
  "artifact_version": "1",
  "source": "guestkit-tui",
  "image": {:?},
  "os_hint": {:?},
  "hostname": {:?},
  "architecture": {:?},
  "pipeline": {{
    "enable_pipeline": true,
    "enable_rdp": null
  }}
}}"#,
            self.image_path, self.os_name, self.hostname, self.architecture
        );
        let path = format!(
            "guestkit-migration-{}.json",
            self.hostname.replace('.', "-")
        );
        std::fs::write(&path, &out)?;
        self.show_notification(format!("Wrote {}", path));
        Ok(path)
    }
}
