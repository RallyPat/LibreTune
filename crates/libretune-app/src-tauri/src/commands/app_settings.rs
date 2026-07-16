//! App-level Settings struct, defaults, and load/save helpers.

use crate::paths::get_settings_path;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Default)]
pub(crate) struct Settings {
    pub(crate) last_ini_path: Option<String>,
    pub(crate) units_system: String,     // "metric" or "imperial"
    pub(crate) auto_burn_on_close: bool, // Auto-burn toggle
    pub(crate) gauge_snap_to_grid: bool, // Dashboard gauge snap to grid
    pub(crate) gauge_free_move: bool,    // Dashboard gauge free move
    pub(crate) gauge_lock: bool,         // Dashboard gauge lock in place
    #[serde(default = "default_true")]
    pub(crate) auto_sync_gauge_ranges: bool, // Auto-sync gauge ranges from INI
    pub(crate) indicator_column_count: String, // "auto" or number like "12"
    pub(crate) indicator_fill_empty: bool, // Fill empty cells in last row
    pub(crate) indicator_text_fit: String, // "scale" or "wrap"

    // Status bar channel configuration
    #[serde(default)]
    pub(crate) status_bar_channels: Vec<String>, // User-selected channels for status bar (max 8)

    // Help icon visibility setting
    #[serde(default = "default_true")]
    pub(crate) show_all_help_icons: bool, // Show help icons on all fields (true) or only fields with descriptions (false)

    // Session persistence
    #[serde(default)]
    pub(crate) last_project_path: Option<String>,
    #[serde(default)]
    pub(crate) last_active_tab: Option<String>,

    /// Render table Y axis with the origin at the bottom-left (lowest load
    /// row at the bottom) instead of the top-left.
    #[serde(default)]
    pub(crate) table_y_axis_bottom: bool,

    // Heatmap color scheme settings
    #[serde(default = "default_heatmap_scheme")]
    pub(crate) heatmap_value_scheme: String, // Scheme for VE/timing tables
    #[serde(default = "default_heatmap_scheme")]
    pub(crate) heatmap_change_scheme: String, // Scheme for AFR correction magnitude
    #[serde(default = "default_heatmap_scheme")]
    pub(crate) heatmap_coverage_scheme: String, // Scheme for hit weighting visualization
    #[serde(default)]
    pub(crate) heatmap_value_custom: Vec<String>, // Custom color stops for value context
    #[serde(default)]
    pub(crate) heatmap_change_custom: Vec<String>, // Custom color stops for change context
    #[serde(default)]
    pub(crate) heatmap_coverage_custom: Vec<String>, // Custom color stops for coverage context

    // Git version control settings
    #[serde(default = "default_auto_commit")]
    pub(crate) auto_commit_on_save: String, // "always", "never", "ask"
    #[serde(default = "default_commit_message_format")]
    pub(crate) commit_message_format: String, // Format string with {date}, {time} placeholders

    /// Global override for runtime packet mode (Auto|ForceBurst|ForceOCH|Disabled)
    #[serde(default = "default_runtime_packet_mode")]
    pub(crate) runtime_packet_mode: String,

    /// Last serial port that successfully connected (app-wide, survives project switches).
    #[serde(default)]
    pub(crate) last_serial_port: Option<String>,

    /// Auto-sync and reconnect after controller commands that reboot the ECU.
    #[serde(default = "default_true")]
    pub(crate) auto_reconnect_after_controller_command: bool,

    /// Automatically reconnect after firmware updates when the ECU reboots.
    #[serde(default = "default_true")]
    pub(crate) auto_reconnect_after_firmware: bool,

    /// FOME-specific fast comms mode for console commands
    /// When enabled for FOME ECUs, attempts a faster protocol path; falls back on error
    #[serde(default = "default_true")]
    pub(crate) fome_fast_comms_enabled: bool,

    // Auto-record settings
    #[serde(default = "default_false")]
    pub(crate) auto_record_enabled: bool, // Enable auto-start/stop recording on key-on/off
    #[serde(default = "default_key_on_rpm")]
    pub(crate) key_on_threshold_rpm: f64, // RPM threshold to detect key-on (default 100)
    #[serde(default = "default_key_off_timeout")]
    pub(crate) key_off_timeout_sec: u32, // Seconds of zero RPM to detect key-off (default 2)

    // Alert rules settings
    #[serde(default = "default_true")]
    pub(crate) alert_large_change_enabled: bool, // Warn when a cell change exceeds thresholds
    #[serde(default = "default_alert_large_change_abs")]
    pub(crate) alert_large_change_abs: f64, // Absolute change threshold
    #[serde(default = "default_alert_large_change_percent")]
    pub(crate) alert_large_change_percent: f64, // Percent change threshold

    // Keyboard shortcut customization (mapping from action to key binding)
    #[serde(default)]
    pub(crate) hotkey_bindings: HashMap<String, String>, // e.g., {"table.setEqual": "=", "table.smooth": "s"}

    // Onboarding state
    #[serde(default = "default_false")]
    pub(crate) onboarding_completed: bool, // Track if user has completed onboarding

    // UI language preference (BCP-47 code such as "en" or "pt-BR").
    // None = let the frontend's language detector decide (querystring/localStorage/navigator).
    #[serde(default)]
    pub(crate) language: Option<String>,
}

pub(crate) fn default_runtime_packet_mode() -> String {
    "Auto".to_string()
}

fn default_heatmap_scheme() -> String {
    "tunerstudio".to_string()
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_key_on_rpm() -> f64 {
    100.0
}

fn default_key_off_timeout() -> u32 {
    2
}

fn default_alert_large_change_abs() -> f64 {
    5.0
}

fn default_alert_large_change_percent() -> f64 {
    10.0
}

fn default_auto_commit() -> String {
    "ask".to_string()
}

fn default_commit_message_format() -> String {
    "Tune saved on {date} at {time}".to_string()
}

pub(crate) fn save_settings(app: &tauri::AppHandle, settings: &Settings) {
    let settings_path = get_settings_path(app);
    // Ensure parent directory exists
    if let Some(parent) = settings_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(&settings_path, json);
    }
}

/// Helper exposed to extracted command modules: returns the commit message format string.
pub(crate) fn get_commit_message_format(app: &tauri::AppHandle) -> String {
    load_settings(app).commit_message_format
}

pub(crate) fn load_settings(app: &tauri::AppHandle) -> Settings {
    let settings_path = get_settings_path(app);
    if let Ok(content) = std::fs::read_to_string(&settings_path) {
        if let Ok(mut settings) = serde_json::from_str::<Settings>(&content) {
            if settings.runtime_packet_mode.trim().is_empty() {
                settings.runtime_packet_mode = default_runtime_packet_mode();
            }
            return settings;
        }
    }
    // Ensure default runtime mode is set when no file exists
    let mut s = Settings::default();
    if s.runtime_packet_mode.trim().is_empty() {
        s.runtime_packet_mode = default_runtime_packet_mode();
    }
    s
}
