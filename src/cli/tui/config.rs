// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! TUI Configuration system
//!
//! Loads and saves user preferences from ~/.config/guestkit/tui.toml

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// TUI Configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TuiConfig {
    /// UI settings
    pub ui: UiConfig,

    /// Behavior settings
    pub behavior: BehaviorConfig,

    /// Keybindings (future: allow custom bindings)
    #[serde(default)]
    pub keybindings: KeybindingsConfig,

    /// View navigation (pinned tabs, icons)
    #[serde(default)]
    pub views: ViewsConfig,
}

/// UI appearance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// Show splash screen on startup
    pub show_splash: bool,

    /// Splash duration in milliseconds
    pub splash_duration_ms: u64,

    /// Show stats bar at startup
    pub show_stats_bar: bool,

    /// Color theme (`carbon` = dark graphite + orange accent)
    pub theme: String,

    /// Enable mouse (tab click, list selection)
    #[serde(default = "default_true")]
    pub mouse_enabled: bool,

    /// Tab icons: `emoji` or `ascii`
    #[serde(default = "default_icon_mode")]
    pub icon_mode: String,

    /// Show emoji in labels (false = ASCII-only chrome)
    #[serde(default = "default_true")]
    pub show_emoji: bool,

    /// Row density: `comfortable` or `compact`
    #[serde(default = "default_density")]
    pub density: String,

    /// Terminal width below which tab labels switch to compact (icon-only) mode
    #[serde(default = "default_auto_compact_width")]
    pub auto_compact_width: u16,

    /// Let the terminal background show through (enable transparency in your terminal)
    #[serde(default = "default_true")]
    pub transparent: bool,

    /// Surface opacity 40–100 when `transparent` is on (Zellij-style glass; default 82)
    #[serde(default = "default_glass_opacity")]
    pub glass_opacity: u8,
}

fn default_glass_opacity() -> u8 {
    82
}

fn default_auto_compact_width() -> u16 {
    100
}

fn default_density() -> String {
    "comfortable".to_string()
}

fn default_true() -> bool {
    true
}

fn default_icon_mode() -> String {
    "emoji".to_string()
}

/// View navigation preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewsConfig {
    /// Pinned tab names shown first (e.g. `["dashboard", "issues", "files"]`)
    pub pinned: Vec<String>,

    /// Default layout mode: `list`, `split`, `detail`
    pub default_layout: String,
}

impl Default for ViewsConfig {
    fn default() -> Self {
        Self {
            pinned: vec![
                "dashboard".to_string(),
                "issues".to_string(),
                "files".to_string(),
            ],
            default_layout: "split".to_string(),
        }
    }
}

/// Behavior configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BehaviorConfig {
    /// Default view on startup
    pub default_view: String,

    /// Auto-refresh interval in seconds (0 = disabled)
    pub auto_refresh_seconds: u64,

    /// Search case-sensitive by default
    pub search_case_sensitive: bool,

    /// Search regex mode by default
    pub search_regex_mode: bool,

    /// Maximum bookmarks
    pub max_bookmarks: usize,

    /// Scroll amount for page up/down
    pub page_scroll_lines: usize,

    /// Default migration target for assurance (`kvm`, `proxmox`, `aws`, …)
    pub default_migration_target: String,

    /// Run doctor/migrate scoring when the TUI finishes loading
    pub assurance_on_startup: bool,

    /// Show one-time onboarding toast after splash
    pub show_assurance_hint: bool,
}

/// Keybindings configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeybindingsConfig {
    /// Enable vim-style keybindings
    pub vim_mode: bool,

    /// Enable Ctrl+P quick jump menu
    pub quick_jump_enabled: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_splash: true,
            splash_duration_ms: 800,
            show_stats_bar: true,
            theme: "carbon".to_string(),
            mouse_enabled: true,
            icon_mode: "emoji".to_string(),
            show_emoji: true,
            density: "comfortable".to_string(),
            auto_compact_width: default_auto_compact_width(),
            transparent: true,
            glass_opacity: default_glass_opacity(),
        }
    }
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            default_view: "dashboard".to_string(),
            auto_refresh_seconds: 0,
            search_case_sensitive: false,
            search_regex_mode: false,
            max_bookmarks: 20,
            page_scroll_lines: 10,
            default_migration_target: "kvm".to_string(),
            assurance_on_startup: false,
            show_assurance_hint: true,
        }
    }
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            vim_mode: true,
            quick_jump_enabled: true,
        }
    }
}

impl TuiConfig {
    /// Get the default config file path
    pub fn default_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().context("Could not determine config directory")?;

        Ok(config_dir.join("guestkit").join("tui.toml"))
    }

    /// Load configuration from default path, or return default config
    pub fn load() -> Self {
        Self::load_from_file().unwrap_or_default()
    }

    /// Load configuration from file
    fn load_from_file() -> Result<Self> {
        let path = Self::default_path()?;

        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(e) => {
                return Err(anyhow::Error::new(e).context("Failed to read config file"));
            }
        };

        let config: TuiConfig = toml::from_str(&contents).context("Failed to parse config file")?;

        Ok(config)
    }

    /// Save configuration to default path
    #[allow(dead_code)]
    pub fn save(&self) -> Result<()> {
        let path = Self::default_path()?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("Failed to create config directory")?;
        }

        let contents = toml::to_string_pretty(self).context("Failed to serialize config")?;

        fs::write(&path, contents).context("Failed to write config file")?;

        Ok(())
    }

    /// Create a default config file if it doesn't exist
    #[allow(dead_code)]
    pub fn init() -> Result<PathBuf> {
        let path = Self::default_path()?;

        if path.exists() {
            return Ok(path);
        }

        let config = Self::default();
        config.save()?;

        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TuiConfig::default();
        assert!(config.ui.show_splash);
        assert_eq!(config.ui.splash_duration_ms, 800);
        assert_eq!(config.behavior.max_bookmarks, 20);
        assert!(config.keybindings.vim_mode);
    }

    #[test]
    fn test_serialize_deserialize() {
        let config = TuiConfig::default();
        let toml_str = toml::to_string(&config).unwrap();
        let deserialized: TuiConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(config.ui.show_splash, deserialized.ui.show_splash);
        assert_eq!(
            config.behavior.default_view,
            deserialized.behavior.default_view
        );
    }

    #[test]
    fn test_ui_config_defaults() {
        let ui = UiConfig::default();
        assert!(ui.show_splash);
        assert_eq!(ui.splash_duration_ms, 800);
        assert!(ui.show_stats_bar);
        assert_eq!(ui.theme, "carbon");
    }

    #[test]
    fn test_behavior_config_defaults() {
        let behavior = BehaviorConfig::default();
        assert_eq!(behavior.default_view, "dashboard");
        assert_eq!(behavior.auto_refresh_seconds, 0);
        assert!(!behavior.search_case_sensitive);
        assert!(!behavior.search_regex_mode);
        assert_eq!(behavior.max_bookmarks, 20);
        assert_eq!(behavior.page_scroll_lines, 10);
    }

    #[test]
    fn test_keybindings_config_defaults() {
        let kb = KeybindingsConfig::default();
        assert!(kb.vim_mode);
        assert!(kb.quick_jump_enabled);
    }

    #[test]
    fn test_config_clone() {
        let config = TuiConfig::default();
        let cloned = config.clone();
        assert_eq!(config.ui.show_splash, cloned.ui.show_splash);
        assert_eq!(config.behavior.max_bookmarks, cloned.behavior.max_bookmarks);
        assert_eq!(config.keybindings.vim_mode, cloned.keybindings.vim_mode);
    }

    #[test]
    fn test_ui_config_custom() {
        let ui = UiConfig {
            show_splash: false,
            splash_duration_ms: 1000,
            theme: "dark".to_string(),
            ..Default::default()
        };

        assert!(!ui.show_splash);
        assert_eq!(ui.splash_duration_ms, 1000);
        assert_eq!(ui.theme, "dark");
    }

    #[test]
    fn test_behavior_config_custom() {
        let behavior = BehaviorConfig {
            default_view: "analytics".to_string(),
            auto_refresh_seconds: 30,
            search_case_sensitive: true,
            max_bookmarks: 50,
            ..Default::default()
        };

        assert_eq!(behavior.default_view, "analytics");
        assert_eq!(behavior.auto_refresh_seconds, 30);
        assert!(behavior.search_case_sensitive);
        assert_eq!(behavior.max_bookmarks, 50);
    }

    #[test]
    fn test_keybindings_config_custom() {
        let kb = KeybindingsConfig {
            vim_mode: false,
            quick_jump_enabled: false,
        };

        assert!(!kb.vim_mode);
        assert!(!kb.quick_jump_enabled);
    }

    #[test]
    fn test_config_serialize_custom() {
        let mut config = TuiConfig::default();
        config.ui.theme = "custom".to_string();
        config.behavior.max_bookmarks = 100;

        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("custom"));
        assert!(toml_str.contains("100"));
    }

    #[test]
    fn test_config_deserialize_partial() {
        // Test that partial config with defaults works
        let toml_str = r#"
        [ui]
        show_splash = false

        [behavior]
        max_bookmarks = 50
        "#;

        let config: TuiConfig = toml::from_str(toml_str).unwrap();
        assert!(!config.ui.show_splash);
        assert_eq!(config.ui.splash_duration_ms, 800); // default
        assert_eq!(config.behavior.max_bookmarks, 50);
        assert_eq!(config.behavior.default_view, "dashboard"); // default
    }
}
