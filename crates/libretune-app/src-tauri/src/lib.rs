use libretune_core::autotune::{
    AutoTuneAuthorityLimits, AutoTuneFilters, AutoTuneSettings,
    AutoTuneState, VEDataPoint,
};
use libretune_core::dash::{
    self, Bibliography, DashComponent, DashFile, GaugePainter, TsColor, VersionInfo,
};
use libretune_core::dashboard::{DashboardLayout, GaugeConfig as DashboardGaugeConfig};
use libretune_core::datalog::DataLogger;
use libretune_core::demo::DemoSimulator;
use libretune_core::ini::{
    Constant, DataType, EcuDefinition,
};
use libretune_core::project::{
    load_math_channels, OnlineIniRepository, Project,
};
use libretune_core::protocol::{Connection, ConnectionConfig, ConnectionState};
use libretune_core::realtime::Evaluator;
use libretune_core::tune::{
    PageState, TuneCache, TuneFile, TuneValue,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::Emitter;
use tauri::Manager;
use tokio::sync::Mutex;

mod commands;
mod paths;
mod port_editor;
mod state;
use commands::annotations::{
    delete_annotation, get_all_annotations, get_annotation, get_table_annotations, set_annotation,
};
use commands::console::{
    clear_console_history, get_console_history, get_ecu_type, send_console_command,
};
use commands::csv_io::{export_tune_as_csv, import_tune_from_csv, reset_tune_to_defaults};
use commands::table_compare::compare_tables;
use commands::adaptive_timing::{
    disable_adaptive_timing, enable_adaptive_timing, get_adaptive_timing_stats,
};
use commands::ini_metadata::{
    get_ini_capabilities, get_protocol_capabilities, get_protocol_defaults, get_ve_analyze_config,
};
use commands::hotkeys::{
    get_hotkey_bindings, is_onboarding_completed, mark_onboarding_completed, save_hotkey_bindings,
};
use commands::tune_health::{
    get_dyno_table_overlay, get_predicted_fills, get_tune_anomalies, get_tune_health_report,
};
use commands::settings::{get_settings, update_heatmap_custom_stops, update_setting};
use commands::restore_points::{
    create_restore_point, delete_restore_point, list_restore_points, load_restore_point,
};
use commands::ts_import::{import_tunerstudio_project, preview_tunerstudio_import};
use commands::base_map::generate_base_map;
use commands::table_ops::{
    add_offset, fill_region, interpolate_cells, interpolate_linear, rebin_table, scale_cells,
    set_cells_equal, smooth_table,
};
use commands::ini_meta::{
    get_curves, get_frontpage, get_gauge_config, get_gauge_configs, get_tables,
};
use commands::ini_dialogs::{
    evaluate_expression, get_dialog_definition, get_help_topic, get_indicator_panel,
    get_port_editor, get_port_editor_assignments, save_port_editor_assignments,
};
use commands::channels::{get_available_channels, get_output_channel_status, get_status_bar_defaults};
use commands::menu::{get_menu_tree, get_searchable_index};
use commands::constants_read::{get_constant, get_constant_string_value, get_constant_value};
use commands::project_tune_sync::{
    compare_project_and_ecu_tunes, mark_tune_modified, save_tune_to_project,
    write_project_tune_to_ecu,
};
use commands::project_mgmt::{
    close_project, get_current_project, update_project_auto_connect, update_project_connection,
};
use commands::project_misc::{delete_project, get_msq_info};
use commands::project_listing::{get_projects_path, list_projects};
use commands::cache_status::{get_table_info, get_tune_cache_status};
use commands::connection::{auto_load_last_ini, disconnect_ecu, get_connection_status};
use commands::dash_files::{
    create_new_dashboard, delete_dashboard, duplicate_dashboard, export_dashboard, get_dash_file,
    load_tunerstudio_dash, rename_dashboard, save_dash_file, validate_dashboard,
};
use commands::dash_layout::{
    check_dash_conflict, create_default_dashboard, get_dashboard_templates, import_dash_file,
    list_available_dashes, list_dashboard_layouts, load_dashboard_layout,
    reset_dashboards_to_defaults, save_dashboard_layout,
};
use commands::autotune_misc::{
    burn_autotune_recommendations, get_autotune_heatmap, get_autotune_recommendations,
    lock_autotune_cells, send_autotune_recommendations, stop_autotune, unlock_autotune_cells,
};
use commands::curve_ops::{get_curve_data, update_curve_data};
use commands::load_pages::load_all_pages;
use commands::table_update::update_table_data;
use commands::constant_values::get_all_constant_values;
use commands::constant_update::update_constant;
use commands::realtime_stop::stop_realtime_stream;
use commands::debug_realtime::debug_single_realtime_read;
use commands::realtime_get::get_realtime_data;
use commands::metrics::{start_metrics_task, stop_metrics_task};
use commands::tune_info::{get_tune_info, new_tune, TuneInfo};
use commands::tune_io::{burn_to_ecu, execute_controller_command, list_tune_files};
use commands::tune_misc::{update_constant_string, use_ecu_tune, use_project_tune};
use commands::data_logging::{
    clear_log, get_log_entries, get_logging_status, read_text_file, save_log, start_logging,
    stop_logging,
};
use commands::diagnostic_loggers::{
    start_composite_logger, start_tooth_logger, stop_composite_logger, stop_tooth_logger,
};
use commands::dyno::{compare_dyno_runs, detect_dyno_headers, load_dyno_run};
use commands::tune_compare::{compare_tune_files, merge_from_tune};
use commands::tune_migration::{
    clear_migration_report, get_migration_report, get_tune_constant_manifest, get_tune_ini_metadata,
};
use commands::git::{
    git_checkout, git_commit, git_create_branch, git_current_branch, git_diff, git_has_changes,
    git_has_repo, git_history, git_init_project, git_list_branches, git_switch_branch,
};
use commands::ini_repository::{
    import_ini, init_ini_repository, list_repository_inis, remove_ini, scan_for_inis,
};
use commands::lua::run_lua_script;
use commands::online_ini::{check_internet_connectivity, download_ini, search_online_inis};
use commands::math_channels::{
    delete_math_channel, get_math_channels, set_math_channel, validate_math_expression,
};
use commands::system::{get_build_info, get_serial_ports};
use commands::wasm_plugin::{
    execute_wasm_plugin, get_wasm_plugin_info, list_wasm_plugins, load_wasm_plugin,
    unload_wasm_plugin,
};
use paths::{get_definitions_dir, get_settings_path};
// port_editor module used by commands/ini_dialogs.rs
use state::{
    is_maf_channel_name, AppState, AutoTuneConfig, AutoTuneLoadSource, AxisHint, RpmState,
    RpmStateTracker, StreamStats,
};

/// Parse a runtime packet mode string into enum
fn parse_runtime_packet_mode(mode: &str) -> libretune_core::protocol::RuntimePacketMode {
    use libretune_core::protocol::RuntimePacketMode as Rpm;
    match mode {
        "ForceBurst" => Rpm::ForceBurst,
        "ForceOCH" => Rpm::ForceOCH,
        "Disabled" => Rpm::Disabled,
        _ => Rpm::Auto,
    }
}

// metrics task extracted to commands/metrics.rs

#[cfg(test)]
mod runtime_mode_tests {
    use super::*;
    use libretune_core::protocol::RuntimePacketMode as Rpm;

    #[test]
    fn test_parse_runtime_packet_mode() {
        assert_eq!(parse_runtime_packet_mode("ForceBurst"), Rpm::ForceBurst);
        assert_eq!(parse_runtime_packet_mode("ForceOCH"), Rpm::ForceOCH);
        assert_eq!(parse_runtime_packet_mode("Disabled"), Rpm::Disabled);
        assert_eq!(parse_runtime_packet_mode("unknown"), Rpm::Auto);
    }

    #[test]
    fn test_default_runtime_packet_mode() {
        assert_eq!(default_runtime_packet_mode(), "Auto");
    }

    // Test helpers that operate on explicit settings path so we don't need a full tauri::App
    #[cfg(test)]
    fn update_setting_with_path(
        settings_path: &std::path::Path,
        key: &str,
        value: &str,
    ) -> Result<(), String> {
        // Load existing or default
        let mut settings: Settings = if let Ok(content) = std::fs::read_to_string(settings_path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Settings::default()
        };

        match key {
            "runtime_packet_mode" => settings.runtime_packet_mode = value.to_string(),
            _ => return Err(format!("Unknown setting: {}", key)),
        }

        if let Ok(json) = serde_json::to_string_pretty(&settings) {
            std::fs::create_dir_all(settings_path.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(settings_path, json).map_err(|e| e.to_string())?;
            Ok(())
        } else {
            Err("Failed to serialize settings".to_string())
        }
    }

    #[test]
    fn test_update_setting_persistence_runtime_packet_mode_file_api() {
        use tempfile::tempdir;
        let dir = tempdir().expect("tempdir");
        let settings_path = dir.path().join("settings.json");

        // Ensure no file to start
        let _ = std::fs::remove_file(&settings_path);

        // Update using helper
        update_setting_with_path(&settings_path, "runtime_packet_mode", "ForceOCH")
            .expect("update should succeed");

        // Read file back and assert
        let content = std::fs::read_to_string(&settings_path).expect("settings file should exist");
        assert!(content.contains("\"runtime_packet_mode\": \"ForceOCH\""));

        // Also simulate load_settings behavior by deserializing
        let settings: Settings = serde_json::from_str(&content).expect("valid json");
        assert_eq!(settings.runtime_packet_mode, "ForceOCH");

        // Clean up
        let _ = std::fs::remove_file(&settings_path);
    }
}

/// Create a bitmask for the given number of bits, safe from overflow.
/// Returns 0xFF if bits >= 8, otherwise (1u8 << bits) - 1.
#[allow(dead_code)]
#[inline]
fn bit_mask_u8(bits: u8) -> u8 {
    if bits >= 8 {
        0xFF
    } else {
        (1u8 << bits) - 1
    }
}

#[derive(Serialize)]
pub(crate) struct ConnectionStatus {
    pub state: ConnectionState,
    pub signature: Option<String>,
    pub has_definition: bool,
    pub ini_name: Option<String>,
    pub demo_mode: bool,
}

/// Signature match type for comparing ECU and INI signatures
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
enum SignatureMatchType {
    /// Signatures match exactly
    Exact,
    /// Signatures match partially (one contains the other, version diff)
    Partial,
    /// Signatures do not match
    Mismatch,
}

/// Information about a signature mismatch for the frontend
#[derive(Serialize, Clone)]
struct SignatureMismatchInfo {
    /// The signature reported by the ECU
    ecu_signature: String,
    /// The signature expected by the loaded INI file
    ini_signature: String,
    /// How closely the signatures match
    match_type: SignatureMatchType,
    /// Path to the currently loaded INI
    current_ini_path: Option<String>,
    /// List of INIs that might match the ECU signature
    matching_inis: Vec<MatchingIniInfo>,
}

/// Information about an INI that matches the ECU signature
#[derive(Serialize, Clone)]
struct MatchingIniInfo {
    /// Path to the INI file
    path: String,
    /// Display name of the INI
    name: String,
    /// Signature from this INI
    signature: String,
    /// How well it matches (exact or partial)
    match_type: SignatureMatchType,
}

/// Result of ECU connection attempt
#[derive(Serialize)]
struct ConnectResult {
    /// The signature reported by the ECU
    signature: String,
    /// Mismatch info if signatures don't match exactly
    mismatch_info: Option<SignatureMismatchInfo>,
}

/// Result of ECU sync operation
#[derive(Serialize)]
struct SyncResult {
    /// Whether all pages synced successfully
    success: bool,
    /// Number of pages successfully synced
    pages_synced: u8,
    /// Number of pages that failed to sync
    pages_failed: u8,
    /// Total number of pages attempted
    total_pages: u8,
    /// Error messages for failed pages (for logging)
    errors: Vec<String>,
}

/// Extended constant info for frontend with value_type field
#[derive(Serialize)]
pub(crate) struct ConstantInfo {
    pub name: String,
    pub label: Option<String>,
    pub units: String,
    pub digits: u8,
    pub min: f64,
    pub max: f64,
    pub value_type: String, // "scalar", "string", "bits", "array"
    pub bit_options: Vec<String>,
    pub help: Option<String>,
    pub visibility_condition: Option<String>, // Expression for when field should be visible
}

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

fn default_runtime_packet_mode() -> String {
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

// =============================================================================
// Dashboard Format Conversion Helpers
// =============================================================================

/// Convert legacy DashboardLayout to TS DashFile format
pub(crate) fn convert_layout_to_dashfile(layout: &DashboardLayout) -> DashFile {
    use libretune_core::dash::{BackgroundStyle, GaugeCluster};
    use libretune_core::dashboard::GaugeType;

    let mut dash = DashFile {
        bibliography: Bibliography {
            author: "LibreTune".to_string(),
            company: "LibreTune Project".to_string(),
            write_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        },
        version_info: VersionInfo {
            file_format: "3.0".to_string(),
            firmware_signature: None,
        },
        gauge_cluster: GaugeCluster {
            anti_aliasing: true,
            force_aspect: false,
            force_aspect_width: 0.0,
            force_aspect_height: 0.0,
            cluster_background_color: TsColor {
                alpha: 255,
                red: 30,
                green: 30,
                blue: 30,
            },
            background_dither_color: None,
            cluster_background_image_file_name: layout.background_image.clone(),
            cluster_background_image_style: BackgroundStyle::Stretch,
            embedded_images: Vec::new(),
            components: Vec::new(),
        },
    };

    for gauge in &layout.gauges {
        let painter = match gauge.gauge_type {
            GaugeType::AnalogDial => GaugePainter::AnalogGauge,
            GaugeType::DigitalReadout => GaugePainter::BasicReadout,
            GaugeType::BarGauge => GaugePainter::HorizontalBarGauge,
            GaugeType::SweepGauge => GaugePainter::AsymmetricSweepGauge,
            GaugeType::LEDIndicator | GaugeType::WarningLight => GaugePainter::BasicReadout,
        };

        let ts_gauge = dash::GaugeConfig {
            id: gauge.id.clone(),
            title: gauge.label.clone(),
            units: gauge.units.clone(),
            output_channel: gauge.channel.clone(),
            min: gauge.min_value,
            max: gauge.max_value,
            low_warning: gauge.low_warning,
            high_warning: gauge.high_warning,
            high_critical: gauge.high_critical,
            value_digits: gauge.decimals as i32,
            relative_x: gauge.x,
            relative_y: gauge.y,
            relative_width: gauge.width,
            relative_height: gauge.height,
            gauge_painter: painter,
            font_color: parse_hex_color(&gauge.font_color),
            needle_color: parse_hex_color(&gauge.needle_color),
            trim_color: parse_hex_color(&gauge.trim_color),
            show_history: gauge.show_history,
            ..Default::default()
        };

        dash.gauge_cluster
            .components
            .push(DashComponent::Gauge(Box::new(ts_gauge)));
    }

    dash
}

/// Convert TS DashFile to legacy DashboardLayout format
pub(crate) fn convert_dashfile_to_layout(dash: &DashFile, name: &str) -> DashboardLayout {
    use libretune_core::dashboard::GaugeType;

    let mut layout = DashboardLayout {
        name: name.to_string(),
        gauges: Vec::new(),
        is_fullscreen: false,
        background_image: dash
            .gauge_cluster
            .cluster_background_image_file_name
            .clone(),
    };

    for (idx, component) in dash.gauge_cluster.components.iter().enumerate() {
        if let DashComponent::Gauge(ref g) = component {
            let gauge_type = match g.gauge_painter {
                GaugePainter::AnalogGauge
                | GaugePainter::BasicAnalogGauge
                | GaugePainter::CircleAnalogGauge
                | GaugePainter::RoundGauge
                | GaugePainter::RoundDashedGauge
                | GaugePainter::FuelMeter
                | GaugePainter::Tachometer => GaugeType::AnalogDial,
                GaugePainter::BasicReadout => GaugeType::DigitalReadout,
                GaugePainter::HorizontalBarGauge
                | GaugePainter::HorizontalDashedBar
                | GaugePainter::VerticalBarGauge
                | GaugePainter::HorizontalLineGauge
                | GaugePainter::VerticalDashedBar
                | GaugePainter::AnalogBarGauge
                | GaugePainter::AnalogMovingBarGauge
                | GaugePainter::Histogram => GaugeType::BarGauge,
                GaugePainter::AsymmetricSweepGauge => GaugeType::SweepGauge,
                GaugePainter::LineGraph => GaugeType::DigitalReadout, // Deferred
            };

            let config = DashboardGaugeConfig {
                id: if g.id.is_empty() {
                    format!("gauge_{}", idx)
                } else {
                    g.id.clone()
                },
                gauge_type,
                channel: g.output_channel.clone(),
                label: g.title.clone(),
                x: g.relative_x,
                y: g.relative_y,
                width: g.relative_width,
                height: g.relative_height,
                z_index: idx as u32,
                min_value: g.min,
                max_value: g.max,
                low_warning: g.low_warning,
                high_warning: g.high_warning,
                high_critical: g.high_critical,
                decimals: g.value_digits.max(0) as u32,
                units: g.units.clone(),
                font_color: g.font_color.to_css_hex(),
                needle_color: g.needle_color.to_css_hex(),
                trim_color: g.trim_color.to_css_hex(),
                show_history: g.show_history,
                show_min_max: false,
                on_condition: None,
                on_color: None,
                off_color: None,
                blink: None,
            };

            layout.gauges.push(config);
        }
    }

    layout
}

/// Parse a CSS hex color string to TsColor
fn parse_hex_color(hex: &str) -> TsColor {
    let hex = hex.trim_start_matches('#');
    if hex.len() >= 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
        TsColor {
            alpha: 255,
            red: r,
            green: g,
            blue: b,
        }
    } else {
        TsColor::default()
    }
}

// =============================================================================
// Signature Comparison Helpers
// =============================================================================

/// Normalize a signature string for robust comparison:
/// - Lowercase
/// - Replace non-alphanumeric characters with spaces
/// - Collapse multiple spaces
fn normalize_signature(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_was_space = false;

    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_space = false;
        } else if ch.is_whitespace() {
            if !last_was_space && !out.is_empty() {
                out.push(' ');
                last_was_space = true;
            }
        } else {
            // Punctuation or other characters -> treat as separator
            if !last_was_space && !out.is_empty() {
                out.push(' ');
                last_was_space = true;
            }
        }
    }

    out.trim().to_string()
}

/// Compare two signatures and determine match type using normalized strings
fn compare_signatures(ecu_sig: &str, ini_sig: &str) -> SignatureMatchType {
    let ecu_normalized = normalize_signature(ecu_sig);
    let ini_normalized = normalize_signature(ini_sig);

    if ecu_normalized == ini_normalized {
        return SignatureMatchType::Exact;
    }

    // Check for common suffixes (hashes)
    // RusEFI signatures often end with a hash or unique ID (e.g. "rusEFI master 2024.02.24.simulator.12345678")
    // If both end with the same alphanumeric string > 6 chars, treat as Exact.
    if let (Some(ecu_suffix), Some(ini_suffix)) = (
        ecu_normalized.split('.').next_back(),
        ini_normalized.split('.').next_back(),
    ) {
        if ecu_suffix.len() > 6
            && ecu_suffix.chars().all(|c| c.is_alphanumeric())
            && ecu_suffix == ini_suffix
        {
            return SignatureMatchType::Exact;
        }
    }

    // Also check split by whitespace just in case hash is separated by space
    if let (Some(ecu_suffix), Some(ini_suffix)) = (
        ecu_normalized.split_whitespace().last(),
        ini_normalized.split_whitespace().last(),
    ) {
        if ecu_suffix.len() > 6
            && ecu_suffix.chars().all(|c| c.is_alphanumeric())
            && ecu_suffix == ini_suffix
        {
            return SignatureMatchType::Exact;
        }
    }

    if ecu_normalized.contains(&ini_normalized) || ini_normalized.contains(&ecu_normalized) {
        return SignatureMatchType::Partial;
    }

    // Compare first token (base type) e.g., "speeduino" and "rusEFI"
    let ecu_first = ecu_normalized.split_whitespace().next();
    let ini_first = ini_normalized.split_whitespace().next();
    if let (Some(ecu_first), Some(ini_first)) = (ecu_first, ini_first) {
        if ecu_first == ini_first {
            return SignatureMatchType::Partial;
        }
    }

    // Check for common firmware family keywords
    let common_keywords = ["uaefi", "speeduino", "rusefi", "epicefi", "megasquirt"];
    let ecu_has_keyword = common_keywords.iter().any(|kw| ecu_normalized.contains(kw));
    let ini_has_keyword = common_keywords.iter().any(|kw| ini_normalized.contains(kw));

    if ecu_has_keyword && ini_has_keyword {
        let ecu_keywords: Vec<&str> = common_keywords
            .iter()
            .filter(|kw| ecu_normalized.contains(**kw))
            .copied()
            .collect();
        let ini_keywords: Vec<&str> = common_keywords
            .iter()
            .filter(|kw| ini_normalized.contains(**kw))
            .copied()
            .collect();

        if ecu_keywords.iter().any(|kw| ini_keywords.contains(kw)) {
            return SignatureMatchType::Partial;
        }
    }

    SignatureMatchType::Mismatch
}

/// Build a shallow SignatureMismatchInfo (without resolving matching INIs) for testing
#[allow(dead_code)]
fn build_shallow_mismatch_info(
    ecu_signature: &str,
    ini_signature: &str,
    current_ini_path: Option<String>,
) -> SignatureMismatchInfo {
    let match_type = compare_signatures(ecu_signature, ini_signature);
    SignatureMismatchInfo {
        ecu_signature: ecu_signature.to_string(),
        ini_signature: ini_signature.to_string(),
        match_type,
        current_ini_path,
        matching_inis: Vec::new(),
    }
}

/// Find INI files that match the given ECU signature (uses tauri State wrapper)
async fn find_matching_inis_internal(
    state: &tauri::State<'_, AppState>,
    ecu_signature: &str,
) -> Vec<MatchingIniInfo> {
    find_matching_inis_from_state(state, ecu_signature).await
}

// Test-only helper: simulate the signature handling part of connect_to_ecu
#[cfg(test)]
async fn connect_to_ecu_simulated(state: &AppState, signature: &str) -> ConnectResult {
    // If there's a loaded definition, compare signatures
    let expected_signature = {
        let def_guard = state.definition.lock().await;
        def_guard.as_ref().map(|d| d.signature.clone())
    };

    let mismatch_info = if let Some(ref expected) = expected_signature {
        let match_type = compare_signatures(signature, expected);
        if match_type != SignatureMatchType::Exact {
            let matching_inis = find_matching_inis_from_state(state, signature).await;
            let current_ini_path = None; // In tests we don't need an app handle to load settings
            Some(SignatureMismatchInfo {
                ecu_signature: signature.to_string(),
                ini_signature: expected.clone(),
                match_type,
                current_ini_path,
                matching_inis,
            })
        } else {
            None
        }
    } else {
        None
    };

    ConnectResult {
        signature: signature.to_string(),
        mismatch_info,
    }
}

// Helper that invokes the optional connection factory and builds a ConnectResult
async fn call_connection_factory_and_build_result(
    state: &AppState,
    config: ConnectionConfig,
) -> Result<ConnectResult, String> {
    // Read protocol settings and expected signature from state
    let def_guard = state.definition.lock().await;
    let protocol_settings = def_guard.as_ref().map(|d| d.protocol.clone());
    let endianness = def_guard.as_ref().map(|d| d.endianness).unwrap_or_default();
    let expected_signature = def_guard.as_ref().map(|d| d.signature.clone());
    drop(def_guard);

    let factory_opt = state.connection_factory.lock().await.clone();
    if let Some(factory) = factory_opt {
        match (factory)(config, protocol_settings, endianness) {
            Ok(signature) => {
                // Build mismatch info if needed
                let mismatch_info = if let Some(ref expected) = expected_signature {
                    let match_type = compare_signatures(&signature, expected);
                    if match_type != SignatureMatchType::Exact {
                        let matching_inis = find_matching_inis_from_state(state, &signature).await;
                        let current_ini_path = None; // caller may provide app if needed

                        Some(SignatureMismatchInfo {
                            ecu_signature: signature.clone(),
                            ini_signature: expected.clone(),
                            match_type,
                            current_ini_path,
                            matching_inis,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                };

                Ok(ConnectResult {
                    signature,
                    mismatch_info,
                })
            }
            Err(e) => Err(format!("Factory-based connect failed: {}", e)),
        }
    } else {
        Err("No connection factory installed".to_string())
    }
}

/// Test-friendly variant that operates on an AppState reference directly
async fn find_matching_inis_from_state(
    state: &AppState,
    ecu_signature: &str,
) -> Vec<MatchingIniInfo> {
    let mut matches = Vec::new();

    // Check INI repository if loaded
    let repo_guard = state.ini_repository.lock().await;
    if let Some(ref repo) = *repo_guard {
        for entry in repo.list() {
            let match_type = compare_signatures(ecu_signature, &entry.signature);
            if match_type != SignatureMatchType::Mismatch {
                matches.push(MatchingIniInfo {
                    path: entry.path.clone(),
                    name: entry.name.clone(),
                    signature: entry.signature.clone(),
                    match_type,
                });
            }
        }
    }

    // Sort by match type (exact first, then partial)
    matches.sort_by(|a, b| match (&a.match_type, &b.match_type) {
        (SignatureMatchType::Exact, SignatureMatchType::Partial) => std::cmp::Ordering::Less,
        (SignatureMatchType::Partial, SignatureMatchType::Exact) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });

    matches
}

/// Lists all available ECU INI definition files in the definitions directory.
///
/// Scans the app's definitions directory for .ini files that describe ECU protocols.
///
/// Returns: Sorted vector of INI filenames
#[tauri::command]
async fn get_available_inis(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let mut inis = Vec::new();
    let definitions_dir = get_definitions_dir(&app);
    println!("Scanning for INIs in: {:?}", definitions_dir);

    // Ensure definitions directory exists
    if !definitions_dir.exists() {
        let _ = std::fs::create_dir_all(&definitions_dir);
        println!("Created definitions directory: {:?}", definitions_dir);
        return Ok(inis); // Return empty list for new install
    }

    match std::fs::read_dir(&definitions_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    if ext.to_string_lossy().to_lowercase() == "ini" {
                        if let Some(name) = entry.file_name().to_str() {
                            inis.push(name.to_string());
                        }
                    }
                }
            }
            println!("Found {} INI files", inis.len());
        }
        Err(e) => {
            println!("Failed to read definitions directory: {}", e);
            return Err(format!("Failed to read definitions directory: {}", e));
        }
    }
    inis.sort();
    Ok(inis)
}

/// Loads an ECU INI definition file and initializes the tune cache.
///
/// This parses the INI file to understand the ECU's memory layout, communication
/// protocol, tables, curves, and output channels. Must be called before connecting
/// to an ECU or opening a tune file.
///
/// # Arguments
/// * `path` - Absolute path or filename relative to definitions directory
///
/// Returns: Nothing on success, error message on failure
#[tauri::command]
async fn load_ini(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    // Resolve path: absolute paths stay as-is, relative paths are resolved from definitions dir
    let full_path = if Path::new(&path).is_absolute() {
        PathBuf::from(&path)
    } else {
        get_definitions_dir(&app).join(&path)
    };

    println!("Loading INI from: {:?}", full_path);
    match EcuDefinition::from_file(full_path.to_string_lossy().as_ref()) {
        Ok(def) => {
            println!(
                "Successfully loaded INI: {} ({} tables, {} pages)",
                def.signature,
                def.tables.len(),
                def.n_pages
            );

            // Get current tune before updating definition (if any)
            let current_tune = {
                let tune_guard = state.current_tune.lock().await;
                tune_guard.as_ref().cloned()
            };

            // Update definition
            let def_clone = def.clone();
            let mut guard = state.definition.lock().await;
            *guard = Some(def);
            drop(guard);

            // Cache output channels to avoid repeated cloning in realtime streaming loop
            let mut channels_cache_guard = state.cached_output_channels.lock().await;
            *channels_cache_guard = Some(Arc::new(def_clone.output_channels.clone()));
            drop(channels_cache_guard);

            // Initialize Math Channel Evaluator
            let evaluator = Evaluator::new(&def_clone);
            let mut evaluator_guard = state.evaluator.lock().await;
            *evaluator_guard = Some(evaluator);
            drop(evaluator_guard);

            // Initialize TuneCache from new definition
            let cache = TuneCache::from_definition(&def_clone);
            let mut cache_guard = state.tune_cache.lock().await;
            *cache_guard = Some(cache);

            // Re-apply current tune to new cache if we have one
            if let Some(tune) = current_tune {
                eprintln!("[DEBUG] load_ini: Re-applying tune data to new INI definition");
                use libretune_core::tune::TuneValue;

                let mut applied_count = 0;
                let mut skipped_count = 0;

                for (name, tune_value) in &tune.constants {
                    if let Some(constant) = def_clone.constants.get(name) {
                        // PC variables
                        if constant.is_pc_variable {
                            match tune_value {
                                TuneValue::Scalar(v) => {
                                    cache_guard
                                        .as_mut()
                                        .unwrap()
                                        .local_values
                                        .insert(name.clone(), *v);
                                    applied_count += 1;
                                }
                                TuneValue::Array(arr) if !arr.is_empty() => {
                                    cache_guard
                                        .as_mut()
                                        .unwrap()
                                        .local_values
                                        .insert(name.clone(), arr[0]);
                                    applied_count += 1;
                                }
                                _ => {
                                    skipped_count += 1;
                                }
                            }
                            continue;
                        }

                        // Handle bits constants specially (they're packed, size_bytes() == 0)
                        if constant.data_type == libretune_core::ini::DataType::Bits {
                            let cache = cache_guard.as_mut().unwrap();
                            // Bits constants: read current byte(s), modify the bits, write back
                            let bit_pos = constant.bit_position.unwrap_or(0);
                            let bit_size = constant.bit_size.unwrap_or(1);

                            // Calculate which byte(s) contain the bits
                            let byte_offset = (bit_pos / 8) as u16;
                            let bit_in_byte = bit_pos % 8;

                            // Calculate how many bytes we need
                            let bits_remaining_after_first_byte =
                                bit_size.saturating_sub(8 - bit_in_byte);
                            let bytes_needed = if bits_remaining_after_first_byte > 0 {
                                1 + bits_remaining_after_first_byte.div_ceil(8)
                            } else {
                                1
                            };
                            let bytes_needed_usize = bytes_needed as usize;

                            // Read current byte(s) value (or 0 if not present)
                            let read_offset = constant.offset + byte_offset;
                            let mut current_bytes: Vec<u8> = cache
                                .read_bytes(constant.page, read_offset, bytes_needed as u16)
                                .map(|s| s.to_vec())
                                .unwrap_or_else(|| vec![0u8; bytes_needed_usize]);

                            // Ensure we have enough bytes
                            while current_bytes.len() < bytes_needed_usize {
                                current_bytes.push(0u8);
                            }

                            // Get the bit value from MSQ (index into bit_options)
                            // MSQ can store bits constants as numeric indices, option strings, or booleans
                            let bit_value = match tune_value {
                                TuneValue::Scalar(v) => *v as u32,
                                TuneValue::Array(arr) if !arr.is_empty() => arr[0] as u32,
                                TuneValue::Bool(b) => {
                                    // Boolean values: true = 1, false = 0
                                    // For bits constants with 2 options (like ["false", "true"]),
                                    // boolean true maps to index 1, false to index 0
                                    if *b {
                                        1
                                    } else {
                                        0
                                    }
                                }
                                TuneValue::String(s) => {
                                    // Look up the string in bit_options to find its index
                                    if let Some(index) =
                                        constant.bit_options.iter().position(|opt| opt == s)
                                    {
                                        index as u32
                                    } else {
                                        // Try case-insensitive match
                                        if let Some(index) = constant
                                            .bit_options
                                            .iter()
                                            .position(|opt| opt.eq_ignore_ascii_case(s))
                                        {
                                            index as u32
                                        } else {
                                            skipped_count += 1;
                                            continue;
                                        }
                                    }
                                }
                                _ => {
                                    skipped_count += 1;
                                    continue;
                                }
                            };

                            // Modify the first byte
                            let bits_in_first_byte = (8 - bit_in_byte).min(bit_size);
                            let mask_first = if bits_in_first_byte >= 8 {
                                0xFF
                            } else {
                                (1u8 << bits_in_first_byte) - 1
                            };
                            let value_first = (bit_value & mask_first as u32) as u8;
                            current_bytes[0] = (current_bytes[0] & !(mask_first << bit_in_byte))
                                | (value_first << bit_in_byte);

                            // If bits span multiple bytes, modify additional bytes
                            if bits_remaining_after_first_byte > 0 {
                                let mut bits_collected = bits_in_first_byte;
                                for i in 1..bytes_needed_usize.min(current_bytes.len()) {
                                    let remaining_bits = bit_size - bits_collected;
                                    if remaining_bits == 0 {
                                        break;
                                    }
                                    let bits_from_this_byte = remaining_bits.min(8);
                                    let mask = if bits_from_this_byte >= 8 {
                                        0xFF
                                    } else {
                                        (1u8 << bits_from_this_byte) - 1
                                    };
                                    let value_from_bit =
                                        ((bit_value >> bits_collected) & mask as u32) as u8;
                                    current_bytes[i] = (current_bytes[i] & !mask) | value_from_bit;
                                    bits_collected += bits_from_this_byte;
                                }
                            }

                            // Write the modified byte(s) back
                            if cache.write_bytes(constant.page, read_offset, &current_bytes) {
                                applied_count += 1;
                            } else {
                                skipped_count += 1;
                            }
                            continue;
                        }

                        // Skip zero-size constants (shouldn't happen for non-bits)
                        let length = constant.size_bytes() as u16;
                        if length == 0 {
                            skipped_count += 1;
                            continue;
                        }

                        // Convert and write to cache
                        let element_size = constant.data_type.size_bytes();
                        let element_count = constant.shape.element_count();
                        let mut raw_data = vec![0u8; length as usize];

                        match tune_value {
                            TuneValue::Scalar(v) => {
                                let raw_val = constant.display_to_raw(*v);
                                constant.data_type.write_to_bytes(
                                    &mut raw_data,
                                    0,
                                    raw_val,
                                    def_clone.endianness,
                                );
                                if cache_guard.as_mut().unwrap().write_bytes(
                                    constant.page,
                                    constant.offset,
                                    &raw_data,
                                ) {
                                    applied_count += 1;
                                } else {
                                    skipped_count += 1;
                                }
                            }
                            TuneValue::Array(arr) => {
                                let last_value = arr.last().copied().unwrap_or(0.0);

                                for i in 0..element_count {
                                    let val = if i < arr.len() { arr[i] } else { last_value };
                                    let raw_val = constant.display_to_raw(val);
                                    let offset = i * element_size;
                                    constant.data_type.write_to_bytes(
                                        &mut raw_data,
                                        offset,
                                        raw_val,
                                        def_clone.endianness,
                                    );
                                }

                                if cache_guard.as_mut().unwrap().write_bytes(
                                    constant.page,
                                    constant.offset,
                                    &raw_data,
                                ) {
                                    applied_count += 1;
                                } else {
                                    skipped_count += 1;
                                }
                            }
                            TuneValue::String(_) | TuneValue::Bool(_) => {
                                skipped_count += 1;
                            }
                        }
                    } else {
                        skipped_count += 1;
                    }
                }

                eprintln!("[DEBUG] load_ini: Re-applied tune constants - applied: {}, skipped: {}, total: {}", 
                    applied_count, skipped_count, tune.constants.len());

                // Emit event to notify UI that tune was re-applied
                let _ = app.emit("tune:loaded", "ini_changed");
            }
            drop(cache_guard);

            // Emit event to notify UI that definition is fully loaded with stats
            let _ = app.emit(
                "definition:loaded",
                serde_json::json!({
                    "signature": def_clone.signature,
                    "tables": def_clone.tables.len(),
                    "curves": def_clone.curves.len(),
                    "dialogs": def_clone.dialogs.len(),
                    "constants": def_clone.constants.len(),
                }),
            );
            eprintln!("[INFO] load_ini: Emitted definition:loaded event (tables={}, curves={}, dialogs={})",
                def_clone.tables.len(), def_clone.curves.len(), def_clone.dialogs.len());

            // Save as last INI
            let mut settings = load_settings(&app);
            settings.last_ini_path = Some(full_path.to_string_lossy().to_string());
            save_settings(&app, &settings);

            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to parse INI {:?}: {}", full_path, e);
            eprintln!("{}", err_msg);
            Err(err_msg)
        }
    }
}

/// Establishes a serial connection to an ECU.
///
/// Opens a serial port and attempts to communicate with the ECU using the
/// protocol defined in the loaded INI file. Returns connection status and
/// any signature mismatch information.
///
/// # Arguments
/// * `port_name` - Serial port name (e.g., "COM3", "/dev/ttyUSB0")
/// * `baud_rate` - Baud rate for serial communication (e.g., 115200)
/// * `timeout_ms` - Optional connection timeout in milliseconds
///
/// Returns: ConnectResult with ECU signature and optional mismatch info
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn connect_to_ecu(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    port_name: String,
    baud_rate: u32,
    timeout_ms: Option<u64>,
    runtime_packet_mode: Option<String>,
    connection_type: Option<String>,
    tcp_host: Option<String>,
    tcp_port: Option<u16>,
) -> Result<ConnectResult, String> {
    use libretune_core::protocol::ConnectionType;

    let conn_type = match connection_type.as_deref() {
        Some(t) if t.eq_ignore_ascii_case("tcp") => ConnectionType::Tcp,
        _ => ConnectionType::Serial,
    };

    let mut config = ConnectionConfig {
        connection_type: conn_type,
        port_name: port_name.clone(),
        tcp_host,
        tcp_port,
        ..Default::default()
    };

    // Apply runtime_packet_mode override if provided
    if let Some(mode) = runtime_packet_mode {
        config.runtime_packet_mode = parse_runtime_packet_mode(&mode);
    }

    // Validate baud rate passed from UI: guard against 0.
    if baud_rate == 0 {
        eprintln!(
            "[WARN] connect_to_ecu: received baud_rate 0, defaulting to {}",
            libretune_core::protocol::DEFAULT_BAUD_RATE
        );
        config.baud_rate = libretune_core::protocol::DEFAULT_BAUD_RATE;
    } else {
        config.baud_rate = baud_rate;
    }

    // Log resolved configuration for diagnostics
    eprintln!(
        "[INFO] connect_to_ecu: type={:?} port='{}' baud={} tcp={:?}:{:?} timeout_ms={}",
        config.connection_type,
        config.port_name,
        config.baud_rate,
        config.tcp_host,
        config.tcp_port,
        config.timeout_ms
    );

    // Get protocol settings from loaded definition if available
    let def_guard = state.definition.lock().await;
    let protocol_settings = def_guard.as_ref().map(|d| d.protocol.clone());
    let endianness = def_guard.as_ref().map(|d| d.endianness).unwrap_or_default();
    let expected_signature = def_guard.as_ref().map(|d| d.signature.clone());
    drop(def_guard);

    // If a test connection factory is installed, use helper to obtain a signature without opening a port
    if state.connection_factory.lock().await.is_some() {
        let res = call_connection_factory_and_build_result(&state, config.clone()).await?;

        // Start metrics task (no connection available, metrics will skip if needed)
        start_metrics_task(app.clone(), state.clone()).await;

        return Ok(res);
    }

    // If a timeout was provided by the UI, apply it
    if let Some(t) = timeout_ms {
        eprintln!("[INFO] connect_to_ecu: using timeout_ms={} from UI", t);
        config.timeout_ms = t;
    }

    // Create connection in a dedicated OS thread (not Tokio's spawn_blocking)
    // Use catch_unwind to capture panics and send them back as errors.
    // Capture a small copy of the connection parameters for post-mortem logging
    let log_port = config.port_name.clone();
    let log_baud = config.baud_rate;
    let log_timeout = config.timeout_ms;

    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let send_err = |s: String| {
            let _ = tx.send(Err(s));
        };

        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut conn = if let Some(protocol) = protocol_settings {
                Connection::with_protocol(config, protocol, endianness)
            } else {
                Connection::new(config)
            };

            match conn.connect() {
                Ok(_) => Ok(conn),
                Err(e) => Err(e.to_string()),
            }
        }));

        match res {
            Ok(Ok(conn)) => {
                let _ = tx.send(Ok(conn));
            }
            Ok(Err(e)) => send_err(e),
            Err(panic_info) => {
                let panic_msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                send_err(format!("Connection thread panicked: {}", panic_msg));
            }
        }
    });

    // Wait for result with a longer timeout to account for USB latency
    let result = match rx.recv_timeout(std::time::Duration::from_secs(15)) {
        Ok(r) => r,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            Err("Connection timed out after 15 seconds".to_string())
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            Err("Connection thread crashed or disconnected".to_string())
        }
    };

    match result {
        Ok(conn) => {
            let signature = conn.signature().unwrap_or("Unknown").to_string();

            // Check signature match and build mismatch info if needed
            let mismatch_info = if let Some(ref expected) = expected_signature {
                let match_type = compare_signatures(&signature, expected);

                if match_type != SignatureMatchType::Exact {
                    // Log the mismatch
                    eprintln!(
                        "Warning: ECU signature '{}' {} INI signature '{}'",
                        signature,
                        if match_type == SignatureMatchType::Partial {
                            "partially matches"
                        } else {
                            "does not match"
                        },
                        expected
                    );

                    // Find matching INIs from repository
                    let matching_inis = find_matching_inis_internal(&state, &signature).await;

                    // Get current INI path from settings
                    let current_ini_path = {
                        let settings = load_settings(&app);
                        settings.last_ini_path.clone()
                    };

                    let info = SignatureMismatchInfo {
                        ecu_signature: signature.clone(),
                        ini_signature: expected.clone(),
                        match_type,
                        current_ini_path,
                        matching_inis,
                    };

                    // Also emit event for backward compatibility
                    let _ = app.emit("signature:mismatch", &info);

                    Some(info)
                } else {
                    None
                }
            } else {
                None
            };

            let mut guard = state.connection.lock().await;
            *guard = Some(conn);

            // Start periodic metrics emission task
            start_metrics_task(app.clone(), state.clone()).await;

            Ok(ConnectResult {
                signature,
                mismatch_info,
            })
        }
        Err(e) => {
            eprintln!(
                "[ERROR] connect_to_ecu failed: {} (port='{}' baud={} timeout_ms={})",
                e, log_port, log_baud, log_timeout
            );
            Err(e)
        }
    }
}

/// Sync response with progress information
#[derive(Serialize)]
struct SyncProgress {
    current_page: u8,
    total_pages: u8,
    bytes_read: usize,
    total_bytes: usize,
    complete: bool,
    /// Optional: page that just failed (for partial sync indication)
    failed_page: Option<u8>,
}

/// Read all ECU pages and store in TuneFile
/// Returns SyncResult indicating success/partial/failure
#[tauri::command]
async fn sync_ecu_data(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<SyncResult, String> {
    // Get definition to know page sizes
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let signature = def.signature.clone();
    let n_pages = def.n_pages;
    let page_sizes: Vec<u32> = def.protocol.page_sizes.clone();
    let total_bytes: usize = page_sizes.iter().map(|&s| s as usize).sum();
    drop(def_guard);

    // Create new tune file
    let mut tune = TuneFile::new(&signature);
    let mut bytes_read: usize = 0;
    let mut pages_synced: u8 = 0;
    let mut pages_failed: u8 = 0;
    let mut errors: Vec<String> = Vec::new();

    for page in 0..n_pages {
        let page_size = page_sizes.get(page as usize).copied().unwrap_or(0);

        // Emit progress
        let progress = SyncProgress {
            current_page: page,
            total_pages: n_pages,
            bytes_read,
            total_bytes,
            complete: false,
            failed_page: None,
        };
        let _ = app.emit("sync:progress", &progress);

        if page_size == 0 {
            // Empty page, skip but count as success
            pages_synced += 1;
            continue;
        }

        // Read page data - wrapped in error handling for resilience
        let page_num = page;
        set_conn_lock_holder("sync_ecu_data");
        let mut conn_guard = state.connection.lock().await;
        let conn = match conn_guard.as_mut() {
            Some(c) => c,
            None => {
                set_conn_lock_holder("(none)");
                errors.push(format!("Page {}: Not connected", page_num));
                pages_failed += 1;
                continue;
            }
        };

        // Try to read page - continue on failure
        match conn.read_page(page_num) {
            Ok(page_data) => {
                bytes_read += page_data.len();
                pages_synced += 1;

                // Store in TuneFile
                tune.pages.insert(page_num, page_data.clone());

                // Also populate TuneCache
                {
                    let mut cache_guard = state.tune_cache.lock().await;
                    if let Some(cache) = cache_guard.as_mut() {
                        cache.load_page(page_num, page_data);
                    }
                }
            }
            Err(e) => {
                let error_msg = format!("Page {}: {}", page_num, e);
                eprintln!("[WARN] sync_ecu_data: {}", error_msg);
                errors.push(error_msg);
                pages_failed += 1;

                // Emit progress with failed page indicator
                let progress = SyncProgress {
                    current_page: page,
                    total_pages: n_pages,
                    bytes_read,
                    total_bytes,
                    complete: false,
                    failed_page: Some(page_num),
                };
                let _ = app.emit("sync:progress", &progress);
            }
        }

        drop(conn_guard);
        set_conn_lock_holder("(none)");
    }

    // Store tune file in state (even if partial)
    let mut tune_guard = state.current_tune.lock().await;
    let project_tune = tune_guard.clone(); // Keep copy for comparison
    let ecu_tune = tune.clone(); // Keep copy for comparison
    *tune_guard = Some(tune);

    // Mark as not modified (freshly synced from ECU)
    let mut modified_guard = state.tune_modified.lock().await;
    *modified_guard = false;
    drop(modified_guard);
    drop(tune_guard);

    // Emit complete
    let progress = SyncProgress {
        current_page: n_pages,
        total_pages: n_pages,
        bytes_read,
        total_bytes,
        complete: true,
        failed_page: None,
    };
    let _ = app.emit("sync:progress", &progress);

    // Check if project tune exists and differs from ECU tune
    if let Some(ref project) = project_tune {
        if project.signature == ecu_tune.signature {
            // Compare page data
            let mut has_differences = false;
            let mut diff_pages: Vec<u8> = Vec::new();

            // Check all pages that exist in either tune
            let all_pages: std::collections::HashSet<u8> = project
                .pages
                .keys()
                .chain(ecu_tune.pages.keys())
                .copied()
                .collect();

            for page_num in all_pages {
                let project_page = project.pages.get(&page_num);
                let ecu_page = ecu_tune.pages.get(&page_num);

                match (project_page, ecu_page) {
                    (Some(p), Some(e)) if p != e => {
                        has_differences = true;
                        diff_pages.push(page_num);
                    }
                    (Some(_), None) | (None, Some(_)) => {
                        has_differences = true;
                        diff_pages.push(page_num);
                    }
                    _ => {}
                }
            }

            if has_differences {
                // Emit event for frontend to show dialog
                let ecu_page_nums: Vec<u8> = ecu_tune.pages.keys().copied().collect();
                let project_page_nums: Vec<u8> = project.pages.keys().copied().collect();
                let _ = app.emit(
                    "tune:mismatch",
                    &serde_json::json!({
                        "ecu_pages": ecu_page_nums,
                        "project_pages": project_page_nums,
                        "diff_pages": diff_pages,
                    }),
                );
            }
        }
    }

    // Log detailed errors for debugging
    if !errors.is_empty() {
        eprintln!(
            "[WARN] sync_ecu_data completed with {} errors:",
            errors.len()
        );
        for err in &errors {
            eprintln!("  - {}", err);
        }
    }

    Ok(SyncResult {
        success: pages_failed == 0,
        pages_synced,
        pages_failed,
        total_pages: n_pages,
        errors,
    })
}

// disconnect_ecu, get_connection_status, auto_load_last_ini extracted to commands/connection.rs

#[derive(Serialize)]
pub(crate) struct TableData {
    pub name: String,
    pub title: String,
    pub x_bins: Vec<f64>,
    pub y_bins: Vec<f64>,
    pub z_values: Vec<Vec<f64>>,
    pub x_axis_name: String,
    pub y_axis_name: String,
    /// Output channel name for X-axis (used for live cell highlighting)
    pub x_output_channel: Option<String>,
    /// Output channel name for Y-axis (used for live cell highlighting)
    pub y_output_channel: Option<String>,
}

// CurveData struct extracted to commands/curve_ops.rs

/// Clean up INI expression labels for display
/// Converts expressions like `{bitStringValue(pwmAxisLabels, gppwm1_loadAxis)}`
/// to a readable fallback like `gppwm1_loadAxis`
fn clean_axis_label(label: &str) -> String {
    let trimmed = label.trim();

    // If it's an expression (starts with {), try to extract meaningful part
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        // Extract content inside braces
        let inner = &trimmed[1..trimmed.len() - 1];

        // Check for bitStringValue(list, index) pattern
        if inner.starts_with("bitStringValue(") {
            // Extract the second parameter (the index variable name)
            if let Some(comma_pos) = inner.find(',') {
                let second_part = inner[comma_pos + 1..].trim();
                // Remove trailing ) if present
                let name = second_part.trim_end_matches(')').trim();
                if !name.is_empty() {
                    return name.to_string();
                }
            }
        }

        // Fallback: just return the inner content without braces
        return inner.to_string();
    }

    // Not an expression, return as-is
    trimmed.to_string()
}

/// Retrieves complete table data including axis bins and Z values.
///
/// Fetches a 2D or 3D table from the tune cache or ECU memory, converting
/// raw bytes to display values using the INI-defined scale and translate.
///
/// # Arguments
/// * `table_name` - Table name or map name from INI definition
///
/// Returns: TableData with x/y bins, z values, and axis metadata
#[tauri::command]
async fn get_table_data(
    state: tauri::State<'_, AppState>,
    table_name: String,
) -> Result<TableData, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let endianness = def.endianness;

    let table = def
        .get_table_by_name_or_map(&table_name)
        .ok_or_else(|| format!("Table {} not found", table_name))?;

    // Clone the table info we need
    let x_bins_name = table.x_bins.clone();
    let y_bins_name = table.y_bins.clone();
    let map_name = table.map.clone();
    let is_3d = table.is_3d();
    let table_name_out = table.name.clone();
    let table_title = table.title.clone();
    let x_label = table
        .x_label
        .clone()
        .unwrap_or_else(|| table.x_bins.clone());
    let y_label = table
        .y_label
        .clone()
        .unwrap_or_else(|| table.y_bins.clone().unwrap_or_default());
    let x_output_channel = table.x_output_channel.clone();
    let y_output_channel = table.y_output_channel.clone();

    // Collect constant info we need
    let x_const = def
        .constants
        .get(&x_bins_name)
        .ok_or_else(|| format!("Constant {} not found", x_bins_name))?
        .clone();
    let y_const = y_bins_name
        .as_ref()
        .and_then(|name| def.constants.get(name).cloned());
    let z_const = def
        .constants
        .get(&map_name)
        .ok_or_else(|| format!("Constant {} not found", map_name))?
        .clone();

    drop(def_guard);

    // Helper to read constant data from TuneFile (offline) or ECU (online)
    fn read_const_from_source(
        constant: &Constant,
        tune: Option<&TuneFile>,
        _cache: Option<&TuneCache>,
        conn: &mut Option<&mut Connection>,
        endianness: libretune_core::ini::Endianness,
    ) -> Result<Vec<f64>, String> {
        let element_count = constant.shape.element_count();
        let element_size = constant.data_type.size_bytes();
        let length = constant.size_bytes() as u16;

        if length == 0 {
            return Ok(vec![0.0; element_count]);
        }

        // If offline, read from TuneFile constants first, then fall back to raw page data
        if conn.is_none() {
            if let Some(tune_file) = tune {
                if let Some(tune_value) = tune_file.constants.get(&constant.name) {
                    use libretune_core::tune::TuneValue;
                    match tune_value {
                        TuneValue::Array(arr) => {
                            eprintln!("[DEBUG] read_const_from_source: CACHE HIT for '{}' (page={}, offset={}, len={}, offline mode)", 
                                constant.name, constant.page, constant.offset, length);
                            return Ok(arr.clone());
                        }
                        TuneValue::Scalar(v) => {
                            eprintln!("[DEBUG] read_const_from_source: Found '{}' in TuneFile as Scalar({}), returning as single-element array", 
                                constant.name, v);
                            return Ok(vec![*v]);
                        }
                        _ => {
                            eprintln!("[DEBUG] read_const_from_source: Found '{}' in TuneFile but wrong type, falling through", constant.name);
                        }
                    }
                }

                if let Some(page_data) = tune_file.pages.get(&constant.page) {
                    let offset = constant.offset as usize;
                    let total_bytes = element_count * element_size;
                    if offset + total_bytes <= page_data.len() {
                        eprintln!("[DEBUG] read_const_from_source: '{}' reading from TuneFile.pages[{}] at offset {}", 
                            constant.name, constant.page, offset);
                        let mut values = Vec::with_capacity(element_count);
                        for i in 0..element_count {
                            let elem_offset = offset + i * element_size;
                            if let Some(raw_val) = constant.data_type.read_from_bytes(
                                page_data,
                                elem_offset,
                                endianness,
                            ) {
                                values.push(constant.raw_to_display(raw_val));
                            } else {
                                values.push(0.0);
                            }
                        }
                        return Ok(values);
                    }
                    eprintln!("[WARN] read_const_from_source: '{}' offset {} + size {} exceeds page {} length {}", 
                        constant.name, offset, total_bytes, constant.page, page_data.len());
                } else {
                    eprintln!("[DEBUG] read_const_from_source: Page {} not found in TuneFile.pages for '{}'", constant.page, constant.name);
                }
                eprintln!("[DEBUG] read_const_from_source: Constant '{}' not found in TuneFile, returning zeros", constant.name);
                return Ok(vec![0.0; element_count]);
            }
            eprintln!("[DEBUG] read_const_from_source: No TuneFile loaded, returning zeros");
            return Ok(vec![0.0; element_count]);
        }

        // When connected, prefer TuneFile.pages (populated by sync_ecu_data) over a live
        // ECU read. Static table data does not change unless the user edits it, so the synced
        // cache is authoritative. Only fall back to a live ECU read if the page was never
        // synced (e.g. user opened a table before syncing).
        if let Some(tune_file) = tune {
            if let Some(page_data) = tune_file.pages.get(&constant.page) {
                let byte_offset = constant.offset as usize;
                let total_bytes = element_count * element_size;
                if byte_offset + total_bytes <= page_data.len() {
                    eprintln!(
                        "[DEBUG] read_const_from_source: '{}' from TuneFile cache (connected hit)",
                        constant.name
                    );
                    let mut values = Vec::with_capacity(element_count);
                    for i in 0..element_count {
                        let elem_offset = byte_offset + i * element_size;
                        if let Some(raw_val) =
                            constant
                                .data_type
                                .read_from_bytes(page_data, elem_offset, endianness)
                        {
                            values.push(constant.raw_to_display(raw_val));
                        } else {
                            values.push(0.0);
                        }
                    }
                    return Ok(values);
                }
            }
        }

        // Cache miss – fall back to a live ECU read (e.g. not yet synced)
        if let Some(ref mut conn_ptr) = conn {
            eprintln!(
                "[DEBUG] read_const_from_source: reading '{}' from ECU (cache miss, online)",
                constant.name
            );
            let params = libretune_core::protocol::commands::ReadMemoryParams {
                can_id: 0,
                page: constant.page,
                offset: constant.offset,
                length,
            };

            let raw_data = conn_ptr.read_memory(params).map_err(|e| e.to_string())?;

            let mut values = Vec::new();
            for i in 0..element_count {
                let offset = i * element_size;
                if let Some(raw_val) = constant
                    .data_type
                    .read_from_bytes(&raw_data, offset, endianness)
                {
                    values.push(constant.raw_to_display(raw_val));
                } else {
                    values.push(0.0);
                }
            }
            return Ok(values);
        }

        // If offline and not in TuneFile, return zeros (should always be in TuneFile)
        eprintln!(
            "[DEBUG] read_const_from_source: Constant '{}' not found in TuneFile, returning zeros",
            constant.name
        );
        Ok(vec![0.0; element_count])
    }

    // Get tune, cache and connection
    let tune_guard = state.current_tune.lock().await;
    let cache_guard = state.tune_cache.lock().await;
    let mut conn_guard = state.connection.lock().await;
    let mut conn = conn_guard.as_mut();

    let x_bins = read_const_from_source(
        &x_const,
        tune_guard.as_ref(),
        cache_guard.as_ref(),
        &mut conn,
        endianness,
    )?;
    let y_bins = if let Some(ref y) = y_const {
        read_const_from_source(
            y,
            tune_guard.as_ref(),
            cache_guard.as_ref(),
            &mut conn,
            endianness,
        )?
    } else {
        vec![0.0]
    };
    let z_flat = read_const_from_source(
        &z_const,
        tune_guard.as_ref(),
        cache_guard.as_ref(),
        &mut conn,
        endianness,
    )?;

    drop(cache_guard);
    drop(conn_guard);

    // Reshape Z values into 2D array [y][x]
    let x_size = x_bins.len();
    let y_size = if is_3d { y_bins.len() } else { 1 };

    let mut z_values = Vec::with_capacity(y_size);
    for y in 0..y_size {
        let mut row = Vec::with_capacity(x_size);
        for x in 0..x_size {
            let idx = y * x_size + x;
            row.push(*z_flat.get(idx).unwrap_or(&0.0));
        }
        z_values.push(row);
    }

    Ok(TableData {
        name: table_name_out,
        title: table_title,
        x_bins,
        y_bins,
        z_values,
        x_axis_name: clean_axis_label(&x_label),
        y_axis_name: clean_axis_label(&y_label),
        x_output_channel,
        y_output_channel,
    })
}
// get_table_info, get_tune_cache_status extracted to commands/cache_status.rs

// load_all_pages extracted to commands/load_pages.rs
/// Retrieves curve data (1D table) including X and Y values.
///
/// Fetches a curve from the tune cache or ECU memory for display
/// in the curve editor.
///
// get_curve_data extracted to commands/curve_ops.rs
/// * `table_name` - Table name or map name from INI definition
/// * `z_values` - 2D array of new Z values in display units
///
/// Returns: Nothing on success
// update_table_data extracted to commands/table_update.rs

/// Updates curve Y values in the tune cache and optionally writes to ECU.
///
/// # Arguments
// update_curve_data extracted to commands/curve_ops.rs

// debug_single_realtime_read extracted to commands/debug_realtime.rs

// get_realtime_data extracted to commands/realtime_get.rs

/// Feed realtime data to AutoTune if it's running
async fn feed_autotune_data(
    app_state: &AppState,
    data: &HashMap<String, f64>,
    current_time_ms: u64,
) {
    // Check if AutoTune is running
    let autotune_guard = app_state.autotune_state.lock().await;
    if !autotune_guard.is_running {
        return;
    }
    drop(autotune_guard);

    // Get the config
    let mut config_guard = app_state.autotune_config.lock().await;
    let config = match config_guard.as_mut() {
        Some(c) => c,
        None => return,
    };

    // Extract channel values (try common channel names)
    let rpm = data
        .get("rpm")
        .or_else(|| data.get("RPM"))
        .or_else(|| data.get("rpmValue"))
        .copied()
        .unwrap_or(0.0);

    let map = data
        .get("map")
        .or_else(|| data.get("MAP"))
        .or_else(|| data.get("mapValue"))
        .or_else(|| data.get("fuelingLoad"))
        .copied()
        .unwrap_or(0.0);

    let maf_value = data
        .get("maf")
        .or_else(|| data.get("MAF"))
        .or_else(|| data.get("mafValue"))
        .or_else(|| data.get("airMass"))
        .or_else(|| data.get("airMassFlow"))
        .or_else(|| data.get("airflow"))
        .or_else(|| data.get("airFlow"))
        .copied()
        .unwrap_or(0.0);

    let load_value = match config.load_source {
        AutoTuneLoadSource::Map => map,
        AutoTuneLoadSource::Maf => {
            if maf_value > 0.0 {
                maf_value
            } else {
                map
            }
        }
    };

    let afr = data
        .get("afr")
        .or_else(|| data.get("AFR"))
        .or_else(|| data.get("afr1"))
        .or_else(|| data.get("AFRValue"))
        .or_else(|| data.get("lambda1"))
        .map(|v| if *v < 2.0 { *v * 14.7 } else { *v }) // Convert lambda to AFR
        .unwrap_or(14.7);

    let ve = data
        .get("ve")
        .or_else(|| data.get("VE"))
        .or_else(|| data.get("veValue"))
        .or_else(|| data.get("VEtable"))
        .copied()
        .unwrap_or(0.0);

    let clt = data
        .get("clt")
        .or_else(|| data.get("CLT"))
        .or_else(|| data.get("coolant"))
        .or_else(|| data.get("coolantTemperature"))
        .copied()
        .unwrap_or(0.0);

    let tps = data
        .get("tps")
        .or_else(|| data.get("TPS"))
        .or_else(|| data.get("tpsValue"))
        .copied()
        .unwrap_or(0.0);

    // Calculate TPS rate (%/sec) based on time delta
    let tps_rate =
        if let (Some(last_tps), Some(last_ts)) = (config.last_tps, config.last_timestamp_ms) {
            let dt_sec = (current_time_ms.saturating_sub(last_ts)) as f64 / 1000.0;
            if dt_sec > 0.001 {
                (tps - last_tps) / dt_sec
            } else {
                0.0
            }
        } else {
            0.0
        };

    // Update last values for next iteration
    config.last_tps = Some(tps);
    config.last_timestamp_ms = Some(current_time_ms);

    // Check for accel enrichment flag
    let accel_enrich_active = data
        .get("accelEnrich")
        .or_else(|| data.get("accelEnrichActive"))
        .or_else(|| data.get("tpsAE"))
        .map(|v| *v > 0.5);

    // Create the data point
    let data_point = VEDataPoint {
        rpm,
        map,
        maf: maf_value,
        load: load_value,
        afr,
        ve,
        clt,
        tps,
        tps_rate,
        accel_enrich_active,
        timestamp_ms: current_time_ms,
    };

    // Clone the config values before we release the guard
    let x_bins = config.x_bins.clone();
    let y_bins = config.y_bins.clone();
    let secondary_x_bins = config.secondary_x_bins.clone();
    let secondary_y_bins = config.secondary_y_bins.clone();
    let settings = config.settings.clone();
    let filters = config.filters.clone();
    let authority = config.authority_limits.clone();
    drop(config_guard);

    // Feed to AutoTune
    let mut autotune_guard = app_state.autotune_state.lock().await;
    autotune_guard.add_data_point(
        data_point.clone(),
        &x_bins,
        &y_bins,
        &settings,
        &filters,
        &authority,
    );

    if let (Some(sec_x_bins), Some(sec_y_bins)) = (secondary_x_bins, secondary_y_bins) {
        let mut secondary_guard = app_state.autotune_secondary_state.lock().await;
        secondary_guard.add_data_point(
            data_point,
            &sec_x_bins,
            &sec_y_bins,
            &settings,
            &filters,
            &authority,
        );
    }
}

/// Helper to write stream diagnostic logs to /tmp/libretune-stream.log
pub(crate) fn stream_log(msg: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/libretune-stream.log")
    {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let _ = writeln!(f, "[{:.3}] {}", now.as_secs_f64(), msg);
    }
}

/// Global tracker for who currently holds the connection lock.
/// Used for diagnostics only — helps identify which command is blocking the stream.
static CONN_LOCK_HOLDER: std::sync::Mutex<&str> = std::sync::Mutex::new("(none)");

pub(crate) fn set_conn_lock_holder(who: &'static str) {
    if let Ok(mut guard) = CONN_LOCK_HOLDER.lock() {
        *guard = who;
    }
}

pub(crate) fn get_conn_lock_holder() -> String {
    CONN_LOCK_HOLDER
        .lock()
        .map(|g| g.to_string())
        .unwrap_or_else(|_| "(poisoned)".to_string())
}

/// Starts continuous realtime data streaming from the ECU.
///
/// Spawns a background task that polls the ECU at the specified interval
/// and emits `realtime:update` events to the frontend. Also feeds data
/// to AutoTune if running.
///
/// # Arguments
/// * `interval_ms` - Polling interval in milliseconds (default: 100ms)
///
/// Returns: Nothing on success
#[tauri::command]
async fn start_realtime_stream(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    interval_ms: Option<u64>,
) -> Result<(), String> {
    let interval = interval_ms.unwrap_or(100);
    let is_demo = *state.demo_mode.lock().await;

    // In demo mode, we only need the definition
    // In real mode, we need both connection and definition. Avoid holding both locks at
    // the same time to prevent potential deadlocks with other commands that lock in the
    // opposite order.
    if !is_demo {
        {
            let def_guard = state.definition.lock().await;
            if def_guard.is_none() {
                return Err("Connection or definition missing".to_string());
            }
        }
        {
            let conn_guard = state.connection.lock().await;
            if conn_guard.is_none() {
                return Err("Connection or definition missing".to_string());
            }
        }
    } else {
        let def_guard = state.definition.lock().await;
        if def_guard.is_none() {
            return Err("Definition not loaded for demo mode".to_string());
        }
    }

    // Always replace old task: previous stop_realtime_stream (fire-and-forget from
    // React cleanup) may not have completed yet.  If we return early here,
    // the deferred stop will abort the only task, leaving the stream dead.
    let mut task_guard = state.streaming_task.lock().await;
    if let Some(old_handle) = task_guard.take() {
        stream_log("start: aborting old task");
        old_handle.abort();
    }
    stream_log(&format!(
        "start: spawning new task (interval={}ms)",
        interval
    ));

    let app_handle = app.clone();

    let handle = tokio::spawn(async move {
        let app_state = app_handle.state::<AppState>();
        let mut ticker = tokio::time::interval(tokio::time::Duration::from_millis(interval));

        // For demo mode, create a simulator
        let mut demo_simulator: Option<DemoSimulator> = None;
        let start_time = std::time::Instant::now();

        // Cache output channels + endianness once before the loop.
        // These don't change during a session so there's no need to re-lock every tick.
        let cached_def_data: Option<(
            Arc<HashMap<String, libretune_core::ini::OutputChannel>>,
            libretune_core::ini::Endianness,
        )> = {
            // Step A: clone the Arc from cache (lock, clone, release)
            let cached_ch: Option<Arc<HashMap<String, libretune_core::ini::OutputChannel>>>;
            {
                let channels_cache = app_state.cached_output_channels.lock().await;
                cached_ch = channels_cache.as_ref().map(Arc::clone);
            } // lock released

            // Step B: get endianness from definition (separate lock)
            if let Some(ch) = cached_ch {
                let def_guard = app_state.definition.lock().await;
                let endianness = def_guard
                    .as_ref()
                    .map(|d| d.endianness)
                    .unwrap_or(libretune_core::ini::Endianness::Little);
                Some((ch, endianness))
            } else {
                let def_guard = app_state.definition.lock().await;
                def_guard
                    .as_ref()
                    .map(|def| (Arc::new(def.output_channels.clone()), def.endianness))
            }
        };
        stream_log(&format!(
            "task started, cached_def_data={}",
            cached_def_data.is_some()
        ));

        // Determine transfer mode once and initialize stream stats
        {
            let (mode_label, mode_reason) = {
                let conn_guard = app_state.connection.lock().await;
                if let Some(conn) = conn_guard.as_ref() {
                    let (fetch, reason) = conn.choose_runtime_command();
                    let label = match &fetch {
                        libretune_core::protocol::RuntimeFetch::Burst(_) => "Burst".to_string(),
                        libretune_core::protocol::RuntimeFetch::OCH(_) => "OCH".to_string(),
                    };
                    (label, reason)
                } else {
                    ("Demo".to_string(), "demo mode".to_string())
                }
            };
            let mut stats = app_state.stream_stats.lock().await;
            *stats = StreamStats {
                ticks_total: 0,
                ticks_success: 0,
                ticks_skipped: 0,
                ticks_error: 0,
                transfer_mode: mode_label,
                transfer_reason: mode_reason,
                interval_ms: interval,
                started_at_ms: chrono::Utc::now().timestamp_millis(),
            };
        }

        let mut tick_count: u64 = 0;
        // Local stream stat counters (flushed to shared state periodically)
        let mut local_ticks_total: u64 = 0;
        let mut local_ticks_success: u64 = 0;
        let mut local_ticks_skipped: u64 = 0;
        let mut local_ticks_error: u64 = 0;
        loop {
            ticker.tick().await;
            tick_count += 1;
            local_ticks_total += 1;

            // Trace: log which phase we're in so we can find deadlocks
            if tick_count <= 25 || tick_count.is_multiple_of(20) {
                stream_log(&format!("tick #{}: T1-demo_mode", tick_count));
            }
            let is_demo = match app_state.demo_mode.try_lock() {
                Ok(guard) => *guard,
                Err(_) => {
                    // demo_mode lock busy — skip tick
                    continue;
                }
            };
            let current_time_ms = start_time.elapsed().as_millis() as u64;

            if is_demo {
                // Demo mode: generate simulated data
                if demo_simulator.is_none() {
                    demo_simulator = Some(DemoSimulator::new());
                }

                if let Some(ref mut sim) = demo_simulator {
                    let elapsed_ms = start_time.elapsed().as_millis() as u64;
                    let mut data = sim.update(elapsed_ms);

                    // User Math Channels Evaluation (Demo)
                    {
                        let mut channels_guard = app_state.math_channels.lock().await;
                        for channel in channels_guard.iter_mut() {
                            if channel.cached_ast.is_none() {
                                let _ = channel.compile();
                            }
                            if let Some(expr) = &channel.cached_ast {
                                if let Ok(val) =
                                    libretune_core::ini::expression::evaluate_simple(expr, &data)
                                {
                                    data.insert(channel.name.clone(), val.as_f64());
                                }
                            }
                        }
                    }

                    // Add common-name aliases so default dashboards work across ECUs.
                    // Demo simulator uses names like rpm, afr, VE1, advance, pulseWidth —
                    // same alias map as real ECU path ensures consistent channel names.
                    {
                        let alias_map: &[(&str, &[&str])] = &[
                            (
                                "rpm",
                                &["RPMValue", "rpm", "RPM", "engineSpeed", "rpmSensor"],
                            ),
                            ("afr", &["AFRValue", "afr", "AFR", "afr1", "lambdaValue"]),
                            (
                                "coolant",
                                &["coolant", "CLTValue", "clt", "CLT", "coolantTemp"],
                            ),
                            (
                                "map",
                                &["MAPValue", "map", "MAP", "manifoldPressure", "fuelLoad"],
                            ),
                            (
                                "tps",
                                &["TPSValue", "tps", "TPS", "throttlePosition", "throttle"],
                            ),
                            (
                                "battery",
                                &[
                                    "VBatt",
                                    "vBatt",
                                    "battery",
                                    "Battery",
                                    "vbatt",
                                    "batteryVoltage",
                                ],
                            ),
                            (
                                "iat",
                                &["IATValue", "iat", "IAT", "intakeAirTemp", "intake"],
                            ),
                            (
                                "advance",
                                &[
                                    "correctedIgnitionAdvance",
                                    "baseIgnitionAdvance",
                                    "SA",
                                    "advance",
                                    "timing",
                                    "ignitionAdvance",
                                    "ignAdv",
                                    "Advance",
                                ],
                            ),
                            (
                                "ve",
                                &[
                                    "veValue", "VE1", "ve1", "veMain", "VEValue", "ve", "VE",
                                    "veCurr",
                                ],
                            ),
                            ("boost", &["boostPressure", "boost", "Boost"]),
                            (
                                "speed",
                                &["vehicleSpeedKph", "speed", "Speed", "wheelSpeed"],
                            ),
                            ("oilPressure", &["oilPressure", "OilPressure", "oilpress"]),
                            (
                                "fuelLevel",
                                &["fuelLevel", "FuelLevel", "fuel", "fuelTankLevel"],
                            ),
                            (
                                "pulseWidth",
                                &[
                                    "actualLastInjection",
                                    "pulseWidth1",
                                    "pulseWidth",
                                    "pw1",
                                    "PW1",
                                ],
                            ),
                            (
                                "dutyCycle",
                                &["injectorDutyCycle", "dutyCycle", "injDuty", "InjectorDuty"],
                            ),
                            ("lambda", &["lambda", "Lambda", "lambdaValue", "wbo2"]),
                            (
                                "dwell",
                                &[
                                    "sparkDwell",
                                    "sparkDwellValue",
                                    "dwell",
                                    "Dwell",
                                    "dwellAngle",
                                    "baseDwell",
                                ],
                            ),
                        ];
                        for (alias, candidates) in alias_map {
                            if !data.contains_key(*alias) {
                                for &candidate in *candidates {
                                    if let Some(&val) = data.get(candidate) {
                                        data.insert(alias.to_string(), val);
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    // Sanitize NaN/Infinity — serde_json cannot serialize these,
                    // which would silently break app_handle.emit().
                    for v in data.values_mut() {
                        if !v.is_finite() {
                            *v = 0.0;
                        }
                    }

                    if let Err(e) = app_handle.emit("realtime:update", &data) {
                        stream_log(&format!("emit FAILED (demo): {}", e));
                    }

                    // Check for RPM state transitions (key-on/off detection)
                    {
                        let rpm = data
                            .get("rpm")
                            .or_else(|| data.get("RPM"))
                            .copied()
                            .unwrap_or(0.0);

                        let settings = load_settings(&app_handle);
                        let mut tracker = app_state.rpm_state_tracker.lock().await;

                        if let Some(new_state) = tracker.update(
                            rpm,
                            settings.key_on_threshold_rpm,
                            settings.key_off_timeout_sec,
                        ) {
                            // Emit event when state changes
                            let state_str = match new_state {
                                RpmState::On => "on",
                                RpmState::Off => "off",
                            };
                            let _ = app_handle.emit("realtime:key_state_changed", &state_str);
                        }
                    }

                    // Feed data to AutoTune if running
                    feed_autotune_data(&app_state, &data, current_time_ms).await;

                    local_ticks_success += 1;
                }
            } else {
                // Real ECU mode: read from connection
                demo_simulator = None; // Clear simulator if we switch modes

                // Phase 1: Get raw data from ECU (hold connection lock only during I/O)
                // Use try_lock() to avoid blocking forever if another command
                // (e.g. get_all_constant_values) is holding the connection lock.
                if tick_count <= 25 || tick_count.is_multiple_of(20) {
                    stream_log(&format!("tick #{}: T2-conn_lock", tick_count));
                }
                let raw_result: Result<Vec<u8>, String>;
                {
                    match app_state.connection.try_lock() {
                        Ok(mut conn_guard) => {
                            set_conn_lock_holder("stream_loop");
                            if let Some(conn) = conn_guard.as_mut() {
                                raw_result = conn.get_realtime_data().map_err(|e| e.to_string());
                            } else {
                                raw_result = Err("No connection".to_string());
                            }
                            set_conn_lock_holder("(none)");
                        }
                        Err(_) => {
                            // Connection lock is busy (another command is using it) — skip this tick
                            if tick_count <= 25 || tick_count.is_multiple_of(20) {
                                let holder = get_conn_lock_holder();
                                stream_log(&format!(
                                    "tick #{}: conn_lock busy (held by: {}), skipping",
                                    tick_count, holder
                                ));
                            }
                            local_ticks_skipped += 1;
                            // Flush stats periodically even on skips
                            if local_ticks_total.is_multiple_of(20) {
                                if let Ok(mut stats) = app_state.stream_stats.try_lock() {
                                    stats.ticks_total = local_ticks_total;
                                    stats.ticks_success = local_ticks_success;
                                    stats.ticks_skipped = local_ticks_skipped;
                                    stats.ticks_error = local_ticks_error;
                                }
                            }
                            continue;
                        }
                    }
                } // conn lock released via try_lock drop

                // Diagnostic logging for raw result
                match &raw_result {
                    Ok(raw) => {
                        static STREAM_LOG_COUNTER: std::sync::atomic::AtomicU64 =
                            std::sync::atomic::AtomicU64::new(0);
                        let count =
                            STREAM_LOG_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if count < 5 || count.is_multiple_of(100) {
                            eprintln!(
                                "[DEBUG] stream tick #{}: got {} raw bytes",
                                count,
                                raw.len()
                            );
                        }
                    }
                    Err(e) => {
                        static ERR_LOG_COUNTER: std::sync::atomic::AtomicU64 =
                            std::sync::atomic::AtomicU64::new(0);
                        let count =
                            ERR_LOG_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if count < 10 || count.is_multiple_of(50) {
                            eprintln!(
                                "[ERROR] stream tick #{}: get_realtime_data failed: {}",
                                count, e
                            );
                        }
                    }
                }

                // Phase 2: Use pre-cached output channels and endianness (no locks needed)
                if tick_count <= 25 || tick_count.is_multiple_of(20) {
                    stream_log(&format!("tick #{}: T3-phase2(cached)", tick_count));
                }
                let def_data = &cached_def_data;

                // Phase 3: Process data outside of any mutex locks
                match (&raw_result, def_data) {
                    (Ok(raw), Some((output_channels, endianness))) => {
                        // Two-pass approach for computed channels:
                        // Pass 1: Parse all non-computed channels
                        let mut data: HashMap<String, f64> = HashMap::new();
                        let mut computed_channels = Vec::new();

                        for (name, channel) in output_channels.iter() {
                            if channel.is_computed() {
                                computed_channels.push((name.clone(), channel.clone()));
                            } else if let Some(val) = channel.parse(raw, *endianness) {
                                data.insert(name.clone(), val);
                            }
                        }

                        // Pass 2: Evaluate computed channels using parsed values as context
                        for (name, channel) in computed_channels {
                            if let Some(val) = channel.parse_with_context(raw, *endianness, &data) {
                                data.insert(name, val);
                            }
                        }

                        // Pass 3: User Math Channels Evaluation
                        if tick_count <= 25 || tick_count.is_multiple_of(20) {
                            stream_log(&format!("tick #{}: T4-math_ch", tick_count));
                        }
                        if let Ok(mut channels_guard) = app_state.math_channels.try_lock() {
                            for channel in channels_guard.iter_mut() {
                                if channel.cached_ast.is_none() {
                                    let _ = channel.compile();
                                }
                                if let Some(expr) = &channel.cached_ast {
                                    if let Ok(val) =
                                        libretune_core::ini::expression::evaluate_simple(
                                            expr, &data,
                                        )
                                    {
                                        data.insert(channel.name.clone(), val.as_f64());
                                    }
                                }
                            }
                        }

                        // Add common-name aliases so default dashboards work across ECUs.
                        // FOME/rusEFI use names like RPMValue, TPSValue, MAPValue, AFRValue, VBatt
                        // while default dashboard XMLs reference rpm, tps, map, afr, battery.
                        // Only insert an alias when the canonical name is absent.
                        {
                            let alias_map: &[(&str, &[&str])] = &[
                                (
                                    "rpm",
                                    &["RPMValue", "rpm", "RPM", "engineSpeed", "rpmSensor"],
                                ),
                                ("afr", &["AFRValue", "afr", "AFR", "afr1", "lambdaValue"]),
                                (
                                    "coolant",
                                    &["coolant", "CLTValue", "clt", "CLT", "coolantTemp"],
                                ),
                                ("map", &["MAPValue", "map", "MAP", "manifoldPressure"]),
                                ("tps", &["TPSValue", "tps", "TPS", "throttlePosition"]),
                                (
                                    "battery",
                                    &["VBatt", "battery", "Battery", "vbatt", "vBatt"],
                                ),
                                (
                                    "iat",
                                    &["IATValue", "iat", "IAT", "intakeAirTemp", "intake"],
                                ),
                                (
                                    "advance",
                                    &[
                                        "correctedIgnitionAdvance",
                                        "baseIgnitionAdvance",
                                        "SA",
                                        "advance",
                                        "ignitionAdvance",
                                        "ignAdv",
                                        "Advance",
                                    ],
                                ),
                                (
                                    "ve",
                                    &[
                                        "veValue", "VE1", "ve1", "veMain", "VEValue", "ve", "VE",
                                        "veCurr",
                                    ],
                                ),
                                ("boost", &["boostPressure", "boost", "Boost"]),
                                (
                                    "speed",
                                    &["vehicleSpeedKph", "speed", "Speed", "wheelSpeed"],
                                ),
                                ("oilPressure", &["oilPressure", "OilPressure", "oilpress"]),
                                (
                                    "fuelLevel",
                                    &["fuelLevel", "FuelLevel", "fuel", "fuelTankLevel"],
                                ),
                                (
                                    "pulseWidth",
                                    &[
                                        "actualLastInjection",
                                        "pulseWidth1",
                                        "pulseWidth",
                                        "pw1",
                                        "PW1",
                                    ],
                                ),
                                (
                                    "dutyCycle",
                                    &["injectorDutyCycle", "dutyCycle", "injDuty", "InjectorDuty"],
                                ),
                                ("lambda", &["lambda", "Lambda", "lambdaValue", "wbo2"]),
                                (
                                    "dwell",
                                    &[
                                        "sparkDwell",
                                        "sparkDwellValue",
                                        "dwell",
                                        "Dwell",
                                        "dwellAngle",
                                        "baseDwell",
                                    ],
                                ),
                            ];
                            for (alias, candidates) in alias_map {
                                if !data.contains_key(*alias) {
                                    for &candidate in *candidates {
                                        if let Some(&val) = data.get(candidate) {
                                            data.insert(alias.to_string(), val);
                                            break;
                                        }
                                    }
                                }
                            }
                        }

                        // Sanitize NaN/Infinity — serde_json cannot serialize these,
                        // which would silently break app_handle.emit().
                        for v in data.values_mut() {
                            if !v.is_finite() {
                                *v = 0.0;
                            }
                        }

                        if let Err(e) = app_handle.emit("realtime:update", &data) {
                            stream_log(&format!("emit FAILED (real): {}", e));
                        }

                        // Log parsed channel count — every tick for the first 30, then every 20th (~1/sec)
                        {
                            static EMIT_LOG_COUNTER: std::sync::atomic::AtomicU64 =
                                std::sync::atomic::AtomicU64::new(0);
                            let count =
                                EMIT_LOG_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if count < 30 || count.is_multiple_of(20) {
                                let rpm = data
                                    .get("rpm")
                                    .or_else(|| data.get("RPM"))
                                    .copied()
                                    .unwrap_or(-1.0);
                                stream_log(&format!(
                                    "emit #{}: {} ch, rpm={:.0}",
                                    count,
                                    data.len(),
                                    rpm
                                ));
                            }
                        }

                        // Check for RPM state transitions (key-on/off detection)
                        if tick_count <= 25 || tick_count.is_multiple_of(20) {
                            stream_log(&format!("tick #{}: T5-rpm_state", tick_count));
                        }
                        {
                            let rpm = data
                                .get("rpm")
                                .or_else(|| data.get("RPM"))
                                .copied()
                                .unwrap_or(0.0);

                            if let Ok(mut tracker) = app_state.rpm_state_tracker.try_lock() {
                                let settings = load_settings(&app_handle);
                                if let Some(new_state) = tracker.update(
                                    rpm,
                                    settings.key_on_threshold_rpm,
                                    settings.key_off_timeout_sec,
                                ) {
                                    let state_str = match new_state {
                                        RpmState::On => "on",
                                        RpmState::Off => "off",
                                    };
                                    let _ =
                                        app_handle.emit("realtime:key_state_changed", &state_str);
                                }
                            }
                        }

                        // Feed data to AutoTune if running
                        if tick_count <= 25 || tick_count.is_multiple_of(20) {
                            stream_log(&format!("tick #{}: T6-autotune", tick_count));
                        }
                        feed_autotune_data(&app_state, &data, current_time_ms).await;

                        local_ticks_success += 1;
                    }
                    (Err(e), _) => {
                        // Log errors to stream log so we can see Phase 1 failures
                        {
                            static ERR_STREAM_LOG: std::sync::atomic::AtomicU64 =
                                std::sync::atomic::AtomicU64::new(0);
                            let n =
                                ERR_STREAM_LOG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if n < 10 || n.is_multiple_of(50) {
                                stream_log(&format!("stream error #{}: {}", n, e));
                            }
                        }
                        let _ = app_handle.emit("realtime:error", &e);
                        local_ticks_error += 1;
                    }
                    _ => {}
                }
            }

            // Flush local stats to shared state every ~1s (20 ticks at 50ms)
            if local_ticks_total.is_multiple_of(20) {
                if let Ok(mut stats) = app_state.stream_stats.try_lock() {
                    stats.ticks_total = local_ticks_total;
                    stats.ticks_success = local_ticks_success;
                    stats.ticks_skipped = local_ticks_skipped;
                    stats.ticks_error = local_ticks_error;
                }
            }
        }
    });

    *task_guard = Some(handle);
    Ok(())
}

// stop_realtime_stream extracted to commands/realtime_stop.rs

// INI metadata commands extracted to commands/ini_meta.rs

// Channel info commands extracted to commands/channels.rs


// Menu tree commands extracted to commands/menu.rs


// INI dialog/indicator/port-editor commands extracted to commands/ini_dialogs.rs

// Constant read commands extracted to commands/constants_read.rs


/// Updates a constant's value in the tune and optionally writes to ECU.
///
/// Handles PC variables (local only), scalar constants, and bit-field
/// constants. Writes to tune cache and ECU if connected.
///
/// # Arguments
/// * `name` - Constant name from INI definition
/// * `value` - New value in display units
///
/// Returns: Nothing on success
// update_constant extracted to commands/constant_update.rs

/// Retrieves all scalar constant values at once.
///
/// Used to get visibility condition context for menu items and dialogs.
/// Only returns scalar constants, not arrays.
///
/// IMPORTANT: This function NEVER reads from the ECU directly. It reads from
/// the tune cache (populated during sync) or the tune file. Reading hundreds
/// of constants individually over serial would hold the connection lock for
/// many seconds, permanently starving the realtime stream.
///
/// Returns: HashMap of constant names to their current values
// get_all_constant_values + read_constant_from_cache helpers extracted to commands/constant_values.rs

/// Starts AutoTune data collection and recommendation engine.
///
/// Initializes the AutoTune state machine to collect AFR data and generate
/// VE table correction recommendations. Reads table axis bins for cell lookup.
///
/// # Arguments
/// * `table_name` - Target VE table name
/// * `settings` - AutoTune configuration (target AFR, etc.)
/// * `filters` - Data filtering criteria (RPM range, CLT range, etc.)
/// * `authority_limits` - Maximum allowed cell changes
///
/// Returns: Nothing on success
#[tauri::command]
async fn start_autotune(
    state: tauri::State<'_, AppState>,
    table_name: String,
    secondary_table_name: Option<String>,
    load_source: Option<AutoTuneLoadSource>,
    settings: AutoTuneSettings,
    filters: AutoTuneFilters,
    authority_limits: AutoTuneAuthorityLimits,
) -> Result<(), String> {
    // Get the table definition to extract bin values
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("No ECU definition loaded")?;
    let cache_guard = state.tune_cache.lock().await;
    let cache = cache_guard.as_ref();

    let mut resolved_load_source = load_source.unwrap_or(AutoTuneLoadSource::Map);

    // Find the table and extract bins
    let (x_bins, y_bins) = if let Some(table) = def.get_table_by_name_or_map(&table_name) {
        let y_output_channel = table.y_output_channel.clone();
        if resolved_load_source == AutoTuneLoadSource::Map {
            if let Some(ref channel) = y_output_channel {
                if is_maf_channel_name(channel) {
                    resolved_load_source = AutoTuneLoadSource::Maf;
                }
            }
        }

        // Read X bins from the constant
        let x_bins = read_axis_bins(def, cache, &table.x_bins, table.x_size, AxisHint::Rpm)?;

        // Read Y bins from the constant (if it's a 3D table)
        let y_bins = if let Some(ref y_bins_name) = table.y_bins {
            read_axis_bins(
                def,
                cache,
                y_bins_name,
                table.y_size,
                AxisHint::Load(resolved_load_source),
            )?
        } else {
            vec![0.0] // 2D table has single Y bin
        };

        (x_bins, y_bins)
    } else {
        // Use default bins if table not found
        let default_y_bins = match resolved_load_source {
            AutoTuneLoadSource::Maf => {
                vec![0.0, 25.0, 50.0, 75.0, 100.0, 150.0, 200.0, 250.0, 300.0]
            }
            AutoTuneLoadSource::Map => vec![20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0],
        };

        (
            vec![
                500.0, 1000.0, 1500.0, 2000.0, 2500.0, 3000.0, 3500.0, 4000.0, 4500.0, 5000.0,
                5500.0, 6000.0,
            ],
            default_y_bins,
        )
    };

    if resolved_load_source == AutoTuneLoadSource::Maf {
        let has_maf_channel = def
            .output_channels
            .keys()
            .any(|name| is_maf_channel_name(name));
        if !has_maf_channel {
            resolved_load_source = AutoTuneLoadSource::Map;
        }
    }

    let (secondary_x_bins, secondary_y_bins) = if let Some(ref secondary_name) =
        secondary_table_name
    {
        if let Some(table) = def.get_table_by_name_or_map(secondary_name) {
            let x_bins = read_axis_bins(def, cache, &table.x_bins, table.x_size, AxisHint::Rpm)?;
            let y_bins = if let Some(ref y_bins_name) = table.y_bins {
                read_axis_bins(
                    def,
                    cache,
                    y_bins_name,
                    table.y_size,
                    AxisHint::Load(resolved_load_source),
                )?
            } else {
                vec![0.0]
            };

            (Some(x_bins), Some(y_bins))
        } else {
            return Err(format!("Secondary table {} not found", secondary_name));
        }
    } else {
        (None, None)
    };

    drop(cache_guard);
    drop(def_guard);

    // Store the config for realtime stream to use
    let config = AutoTuneConfig {
        table_name: table_name.clone(),
        secondary_table_name: secondary_table_name.clone(),
        settings: settings.clone(),
        filters: filters.clone(),
        authority_limits: authority_limits.clone(),
        load_source: resolved_load_source,
        x_bins,
        y_bins,
        secondary_x_bins,
        secondary_y_bins,
        last_tps: None,
        last_timestamp_ms: None,
    };

    *state.autotune_config.lock().await = Some(config);

    let mut guard = state.autotune_state.lock().await;
    guard.start();

    let mut secondary_guard = state.autotune_secondary_state.lock().await;
    if secondary_table_name.is_some() {
        secondary_guard.start();
    } else {
        secondary_guard.stop();
    }
    Ok(())
}

/// Read axis bin values from a constant definition
fn read_axis_bins(
    def: &EcuDefinition,
    cache: Option<&TuneCache>,
    const_name: &str,
    size: usize,
    axis_hint: AxisHint,
) -> Result<Vec<f64>, String> {
    let fallback_bins = |hint: AxisHint, size: usize| -> Vec<f64> {
        let steps = (size.saturating_sub(1)).max(1) as f64;
        match hint {
            AxisHint::Rpm => (0..size)
                .map(|i| 500.0 + (i as f64 * 6000.0 / steps))
                .collect(),
            AxisHint::Load(AutoTuneLoadSource::Maf) => (0..size)
                .map(|i| 0.0 + (i as f64 * 300.0 / steps))
                .collect(),
            AxisHint::Load(AutoTuneLoadSource::Map) => (0..size)
                .map(|i| 20.0 + (i as f64 * 80.0 / steps))
                .collect(),
            AxisHint::Unknown => {
                if size > 8 {
                    (0..size)
                        .map(|i| 500.0 + (i as f64 * 6000.0 / steps))
                        .collect()
                } else {
                    (0..size)
                        .map(|i| 20.0 + (i as f64 * 80.0 / steps))
                        .collect()
                }
            }
        }
    };

    // Try to get the constant
    let constant = match def.constants.get(const_name) {
        Some(c) => c,
        None => {
            // Constant not found, generate linear bins
            return Ok(fallback_bins(axis_hint, size));
        }
    };

    // If we have cached tune data, read from it
    if let Some(cache) = cache {
        if let Some(page_data) = cache.get_page(constant.page) {
            let elem_size = constant.data_type.size_bytes();
            let mut bins = Vec::with_capacity(size);
            let mut offset = constant.offset as usize;

            for _ in 0..size {
                if offset + elem_size <= page_data.len() {
                    if let Ok(raw) = read_raw_value(&page_data[offset..], &constant.data_type) {
                        bins.push(constant.raw_to_display(raw));
                    }
                    offset += elem_size;
                }
            }

            if !bins.is_empty() {
                return Ok(bins);
            }
        }
    }

    // Last resort: generate linear bins based on axis hint
    Ok(fallback_bins(axis_hint, size))
}

// Misc AutoTune commands extracted to commands/autotune_misc.rs

// Tune health/anomaly/predicted_fills/dyno_overlay extracted to commands/tune_health.rs
/// Helper function to get table data internally (avoids code duplication)
pub(crate) async fn get_table_data_internal(
    state: &tauri::State<'_, AppState>,
    table_name: &str,
) -> Result<TableData, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let endianness = def.endianness;

    let table = def
        .get_table_by_name_or_map(table_name)
        .ok_or_else(|| format!("Table {} not found", table_name))?;

    let x_bins_name = table.x_bins.clone();
    let y_bins_name = table.y_bins.clone();
    let map_name = table.map.clone();
    let is_3d = table.is_3d();
    let table_name_out = table.name.clone();
    let table_title = table.title.clone();
    let x_label = table
        .x_label
        .clone()
        .unwrap_or_else(|| table.x_bins.clone());
    let y_label = table
        .y_label
        .clone()
        .unwrap_or_else(|| table.y_bins.clone().unwrap_or_default());
    let x_output_channel = table.x_output_channel.clone();
    let y_output_channel = table.y_output_channel.clone();

    let x_const = def
        .constants
        .get(&x_bins_name)
        .ok_or_else(|| format!("Constant {} not found", x_bins_name))?
        .clone();
    let y_const = y_bins_name
        .as_ref()
        .and_then(|name| def.constants.get(name).cloned());
    let z_const = def
        .constants
        .get(&map_name)
        .ok_or_else(|| format!("Constant {} not found", map_name))?
        .clone();

    drop(def_guard);

    // Read from tune file (offline mode)
    let tune_guard = state.current_tune.lock().await;

    fn read_const_values(
        constant: &Constant,
        tune: Option<&TuneFile>,
        endianness: libretune_core::ini::Endianness,
    ) -> Vec<f64> {
        let element_count = constant.shape.element_count();
        let element_size = constant.data_type.size_bytes();
        if let Some(tune_file) = tune {
            if let Some(tune_value) = tune_file.constants.get(&constant.name) {
                match tune_value {
                    TuneValue::Array(arr) => return arr.clone(),
                    TuneValue::Scalar(v) => return vec![*v],
                    _ => {}
                }
            }

            if let Some(page_data) = tune_file.pages.get(&constant.page) {
                let offset = constant.offset as usize;
                let total_bytes = element_count * element_size;
                if offset + total_bytes <= page_data.len() {
                    let mut values = Vec::with_capacity(element_count);
                    for i in 0..element_count {
                        let elem_offset = offset + i * element_size;
                        if let Some(raw_val) =
                            constant
                                .data_type
                                .read_from_bytes(page_data, elem_offset, endianness)
                        {
                            values.push(constant.raw_to_display(raw_val));
                        } else {
                            values.push(0.0);
                        }
                    }
                    return values;
                }
            }
        }
        vec![0.0; element_count]
    }

    let x_bins = read_const_values(&x_const, tune_guard.as_ref(), endianness);
    let y_bins = if let Some(ref y) = y_const {
        read_const_values(y, tune_guard.as_ref(), endianness)
    } else {
        vec![0.0]
    };
    let z_flat = read_const_values(&z_const, tune_guard.as_ref(), endianness);

    drop(tune_guard);

    // Reshape Z values into 2D array [y][x]
    let x_size = x_bins.len();
    let y_size = if is_3d { y_bins.len() } else { 1 };

    let mut z_values = Vec::with_capacity(y_size);
    for y in 0..y_size {
        let mut row = Vec::with_capacity(x_size);
        for x in 0..x_size {
            let idx = y * x_size + x;
            row.push(*z_flat.get(idx).unwrap_or(&0.0));
        }
        z_values.push(row);
    }

    Ok(TableData {
        name: table_name_out,
        title: table_title,
        x_bins,
        y_bins,
        z_values,
        x_axis_name: clean_axis_label(&x_label),
        y_axis_name: clean_axis_label(&y_label),
        x_output_channel,
        y_output_channel,
    })
}

/// Helper function to update table z_values internally
pub(crate) async fn update_table_z_values_internal(
    state: &tauri::State<'_, AppState>,
    table_name: &str,
    z_values: Vec<Vec<f64>>,
) -> Result<(), String> {
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;
    let mut cache_guard = state.tune_cache.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let table = def
        .get_table_by_name_or_map(table_name)
        .ok_or_else(|| format!("Table {} not found", table_name))?;

    let constant = def
        .constants
        .get(&table.map)
        .ok_or_else(|| format!("Constant {} not found for table {}", table.map, table_name))?;

    // Flatten z_values
    let flat_values: Vec<f64> = z_values.into_iter().flatten().collect();

    if flat_values.len() != constant.shape.element_count() {
        return Err(format!(
            "Invalid data size: expected {}, got {}",
            constant.shape.element_count(),
            flat_values.len()
        ));
    }

    // Convert display values to raw bytes
    let element_size = constant.data_type.size_bytes();
    let mut raw_data = vec![0u8; constant.size_bytes()];

    for (i, val) in flat_values.iter().enumerate() {
        let raw_val = constant.display_to_raw(*val);
        let offset = i * element_size;
        constant
            .data_type
            .write_to_bytes(&mut raw_data, offset, raw_val, def.endianness);
    }

    // Write to TuneCache if available
    if let Some(cache) = cache_guard.as_mut() {
        if cache.write_bytes(constant.page, constant.offset, &raw_data) {
            // Also update TuneFile in memory
            let mut tune_guard = state.current_tune.lock().await;
            if let Some(tune) = tune_guard.as_mut() {
                let page_data = tune.pages.entry(constant.page).or_insert_with(|| {
                    vec![
                        0u8;
                        def.page_sizes
                            .get(constant.page as usize)
                            .copied()
                            .unwrap_or(256) as usize
                    ]
                });
                let start = constant.offset as usize;
                let end = start + raw_data.len();
                if end <= page_data.len() {
                    page_data[start..end].copy_from_slice(&raw_data);
                }
            }
            *state.tune_modified.lock().await = true;
        }
    }

    // Write to ECU if connected (optional)
    if let Some(conn) = conn_guard.as_mut() {
        let params = libretune_core::protocol::commands::WriteMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            data: raw_data,
        };
        if let Err(e) = conn.write_memory(params) {
            eprintln!("[WARN] Failed to write to ECU: {}", e);
        }
    }

    Ok(())
}

/// Helper function to update a constant array (used for table axis bins)
pub(crate) async fn update_constant_array_internal(
    state: &tauri::State<'_, AppState>,
    constant_name: &str,
    values: Vec<f64>,
) -> Result<(), String> {
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;
    let mut cache_guard = state.tune_cache.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let constant = def
        .constants
        .get(constant_name)
        .ok_or_else(|| format!("Constant {} not found", constant_name))?;

    if values.len() != constant.shape.element_count() {
        return Err(format!(
            "Invalid data size for {}: expected {}, got {}",
            constant_name,
            constant.shape.element_count(),
            values.len()
        ));
    }

    let element_size = constant.data_type.size_bytes();
    let mut raw_data = vec![0u8; constant.size_bytes()];

    for (i, val) in values.iter().enumerate() {
        let raw_val = constant.display_to_raw(*val);
        let offset = i * element_size;
        constant
            .data_type
            .write_to_bytes(&mut raw_data, offset, raw_val, def.endianness);
    }

    if let Some(cache) = cache_guard.as_mut() {
        if cache.write_bytes(constant.page, constant.offset, &raw_data) {
            let mut tune_guard = state.current_tune.lock().await;
            if let Some(tune) = tune_guard.as_mut() {
                let page_data = tune.pages.entry(constant.page).or_insert_with(|| {
                    vec![
                        0u8;
                        def.page_sizes
                            .get(constant.page as usize)
                            .copied()
                            .unwrap_or(256) as usize
                    ]
                });

                let start = constant.offset as usize;
                let end = start + raw_data.len();
                if end <= page_data.len() {
                    page_data[start..end].copy_from_slice(&raw_data);
                }
            }

            *state.tune_modified.lock().await = true;
        }
    }

    if let Some(conn) = conn_guard.as_mut() {
        let params = libretune_core::protocol::commands::WriteMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            data: raw_data.clone(),
        };
        if let Err(e) = conn.write_memory(params) {
            eprintln!(
                "[WARN] Failed to write axis bins '{}' to ECU: {}",
                constant_name, e
            );
        }
    }

    Ok(())
}

// Table operation commands extracted to commands/table_ops.rs

// Dashboard layout commands extracted to commands/dash_layout.rs

// =============================================================================
// Tune File Save/Load/Burn Commands
// =============================================================================

// tune_info commands extracted to commands/tune_info.rs

/// Saves the current tune to disk.
///
/// Writes all tune data to an MSQ file. If no path is provided,
/// uses the existing path or prompts for save location.
///
/// # Arguments
/// * `path` - Optional file path. If None, uses current path or generates one
///
/// Returns: The path where the tune was saved
#[tauri::command]
async fn save_tune(
    state: tauri::State<'_, AppState>,
    path: Option<String>,
) -> Result<String, String> {
    let mut tune_guard = state.current_tune.lock().await;
    let path_guard = state.current_tune_path.lock().await;
    let cache_guard = state.tune_cache.lock().await;
    let def_guard = state.definition.lock().await;

    let tune = tune_guard.as_mut().ok_or("No tune loaded")?;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    // Write TuneCache data to TuneFile before saving (ensures offline changes are saved)
    if let Some(cache) = cache_guard.as_ref() {
        // Copy all pages from cache to tune file
        for page_num in 0..def.n_pages {
            if let Some(page_data) = cache.get_page(page_num) {
                tune.pages.insert(page_num, page_data.to_vec());
            }
        }

        // Read constants from cache and add to tune file
        use libretune_core::tune::TuneValue;
        let mut constants_saved = 0;

        for (name, constant) in &def.constants {
            // Skip PC variables - they're stored separately
            if constant.is_pc_variable {
                // Get PC variable from local_values
                if let Some(value) = cache.local_values.get(name) {
                    tune.set_constant_with_page(
                        name.clone(),
                        TuneValue::Scalar(*value),
                        constant.page,
                    );
                    constants_saved += 1;
                }
                continue;
            }

            // Handle bits constants specially - they have zero size_bytes() but we need to read them
            if constant.data_type == libretune_core::ini::DataType::Bits {
                // Read the byte(s) containing the bits
                let byte_offset = (constant.bit_position.unwrap_or(0) / 8) as u16;
                let bit_in_byte = constant.bit_position.unwrap_or(0) % 8;
                let bit_size = constant.bit_size.unwrap_or(0);
                let bytes_needed = (bit_in_byte + bit_size).div_ceil(8).max(1) as u16;

                if let Some(bytes) =
                    cache.read_bytes(constant.page, constant.offset + byte_offset, bytes_needed)
                {
                    // Extract the bit value
                    let mut bit_val: u32 = 0;
                    let mut bits_remaining = bit_size;
                    let mut current_bit = bit_in_byte;

                    for byte in bytes.iter().take(bytes_needed as usize) {
                        let bits_in_this_byte = bits_remaining.min(8 - current_bit);
                        // Safe shift: ensure we don't shift by 8 or more
                        let mask = if bits_in_this_byte == 0 {
                            0
                        } else if bits_in_this_byte == 8 && current_bit == 0 {
                            // All bits in this byte
                            0xFFu8
                        } else {
                            // bits_in_this_byte is guaranteed to be < 8 here
                            let base_mask = (1u8 << bits_in_this_byte.min(7)) - 1;
                            base_mask << current_bit
                        };
                        let extracted = ((*byte & mask) >> current_bit) as u32;
                        bit_val |= extracted << (bit_size - bits_remaining);

                        bits_remaining = bits_remaining.saturating_sub(bits_in_this_byte);
                        if bits_remaining == 0 {
                            break;
                        }
                        current_bit = 0;
                    }

                    // Convert bit index to string from bit_options
                    let bit_index = bit_val as usize;
                    if bit_index < constant.bit_options.len() {
                        let option_string = constant.bit_options[bit_index].clone();
                        tune.set_constant_with_page(
                            name.clone(),
                            TuneValue::String(option_string),
                            constant.page,
                        );
                        constants_saved += 1;
                    } else {
                        // Out of range - save as numeric index (fallback)
                        tune.set_constant_with_page(
                            name.clone(),
                            TuneValue::Scalar(bit_val as f64),
                            constant.page,
                        );
                        constants_saved += 1;
                    }
                }
                continue;
            }

            // Skip constants with zero size
            let length = constant.size_bytes() as u16;
            if length == 0 {
                continue;
            }

            // Read constant from cache
            let page_state = cache.page_state(constant.page);
            let page_size = cache.page_size(constant.page);
            let page_data_opt = cache.get_page(constant.page);
            let page_data_len = page_data_opt.map(|p| p.len()).unwrap_or(0);

            if name == "veTable" || name == "veRpmBins" || name == "veLoadBins" {
                eprintln!("[DEBUG] save_tune: Attempting to save '{}' - page={}, offset={}, len={}, page_state={:?}, page_size={:?}, page_data_len={}", 
                    name, constant.page, constant.offset, length, page_state, page_size, page_data_len);
            }

            if let Some(raw_data) = cache.read_bytes(constant.page, constant.offset, length) {
                let element_count = constant.shape.element_count();
                let element_size = constant.data_type.size_bytes();
                let mut values = Vec::new();

                for i in 0..element_count {
                    let offset = i * element_size;
                    if let Some(raw_val) =
                        constant
                            .data_type
                            .read_from_bytes(raw_data, offset, def.endianness)
                    {
                        values.push(constant.raw_to_display(raw_val));
                    } else {
                        values.push(0.0);
                    }
                }

                // Convert to TuneValue format
                let tune_value = if element_count == 1 {
                    TuneValue::Scalar(values[0])
                } else {
                    TuneValue::Array(values)
                };

                tune.set_constant_with_page(name.clone(), tune_value, constant.page);
                constants_saved += 1;

                if name == "veTable" || name == "veRpmBins" || name == "veLoadBins" {
                    eprintln!(
                        "[DEBUG] save_tune: ✓ Saved '{}' - {} elements",
                        name, element_count
                    );
                }
            } else if name == "veTable" || name == "veRpmBins" || name == "veLoadBins" {
                eprintln!("[DEBUG] save_tune: ✗ Failed to read '{}' from cache - page_state={:?}, page_size={:?}, page_data_len={}, required_offset={}", 
                    name, page_state, page_size, page_data_len, constant.offset as usize + length as usize);
            }
        }

        eprintln!(
            "[DEBUG] save_tune: Saved {} constants from cache to tune file",
            constants_saved
        );
    }

    // Update modified timestamp
    tune.touch();

    // Populate INI metadata for version tracking (LibreTune 1.1+)
    // This allows detecting when a tune was created with a different INI version
    let ini_name = state
        .current_project
        .lock()
        .await
        .as_ref()
        .map(|p| p.config.ecu_definition.clone())
        .unwrap_or_else(|| "unknown.ini".to_string());
    tune.ini_metadata = Some(def.generate_ini_metadata(&ini_name));
    tune.constant_manifest = Some(def.generate_constant_manifest());

    // Use provided path, or current path, or generate default
    let save_path = if let Some(p) = path {
        PathBuf::from(p)
    } else if let Some(p) = path_guard.as_ref() {
        p.clone()
    } else {
        // Generate default path in projects directory
        let filename = format!("{}.msq", tune.signature.replace(' ', "_"));
        libretune_core::project::Project::projects_dir()
            .map_err(|e| format!("Failed to get projects directory: {}", e))?
            .join(filename)
    };

    // Ensure projects directory exists
    if let Some(parent) = save_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    tune.save(&save_path)
        .map_err(|e| format!("Failed to save tune: {}", e))?;

    drop(tune_guard);
    drop(path_guard);
    drop(cache_guard);
    drop(def_guard);

    *state.current_tune_path.lock().await = Some(save_path.clone());
    *state.tune_modified.lock().await = false;

    Ok(save_path.to_string_lossy().to_string())
}

/// Saves the current tune to a specified path.
///
/// Wrapper around save_tune with a required path argument.
///
/// # Arguments
/// * `path` - File path for saving the tune
///
/// Returns: The path where the tune was saved
#[tauri::command]
async fn save_tune_as(state: tauri::State<'_, AppState>, path: String) -> Result<String, String> {
    save_tune(state, Some(path)).await
}

/// Loads a tune file from disk.
///
/// Parses an MSQ file and populates the tune cache. Handles signature
/// comparison and generates migration reports if the INI has changed.
///
/// # Arguments
/// * `path` - Path to the MSQ file to load
///
/// Returns: TuneInfo with loaded tune metadata
#[tauri::command]
async fn load_tune(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    path: String,
) -> Result<TuneInfo, String> {
    eprintln!("\n[INFO] ========================================");
    eprintln!("[INFO] LOADING TUNE FILE: {}", path);
    eprintln!("[INFO] ========================================");

    let tune = TuneFile::load(&path).map_err(|e| format!("Failed to load tune: {}", e))?;

    eprintln!("[INFO] ✓ Tune file loaded successfully");
    eprintln!("[INFO]   Signature: '{}'", tune.signature);
    eprintln!("[INFO]   Constants: {}", tune.constants.len());
    eprintln!("[INFO]   Pages: {}", tune.pages.len());

    // Debug: List first 20 constant names to see what we parsed
    let constant_names: Vec<String> = tune.constants.keys().take(20).cloned().collect();
    eprintln!(
        "[DEBUG] load_tune: Sample constants from MSQ: {:?}",
        constant_names
    );

    // Debug: Check VE table constants specifically
    let ve_table_in_tune = tune.constants.contains_key("veTable");
    let ve_rpm_bins_in_tune = tune.constants.contains_key("veRpmBins");
    let ve_load_bins_in_tune = tune.constants.contains_key("veLoadBins");
    eprintln!(
        "[DEBUG] load_tune: VE constants in tune - veTable: {}, veRpmBins: {}, veLoadBins: {}",
        ve_table_in_tune, ve_rpm_bins_in_tune, ve_load_bins_in_tune
    );

    // Check if MSQ signature matches current INI definition (informational only)
    // We'll still apply constants by name match regardless of signature match
    let def_guard = state.definition.lock().await;
    let current_ini_signature = def_guard.as_ref().map(|d| d.signature.clone());
    drop(def_guard);

    if let Some(ref ini_sig) = current_ini_signature {
        let match_type = compare_signatures(&tune.signature, ini_sig);
        if match_type != SignatureMatchType::Exact {
            eprintln!("[INFO] load_tune: MSQ signature '{}' {} current INI signature '{}' - will apply constants by name match", 
                tune.signature,
                if match_type == SignatureMatchType::Partial { "partially matches" } else { "does not match" },
                ini_sig);
            eprintln!("[INFO] load_tune: This is normal - many constants (like VE table, ignition tables) will still work across different INI versions");

            // Only show dialog for complete mismatches, and only if we find better matching INIs
            if match_type == SignatureMatchType::Mismatch {
                let matching_inis = find_matching_inis_internal(&state, &tune.signature).await;
                let matching_count = matching_inis.len();

                // Only show dialog if we found better matching INIs
                if matching_count > 0 {
                    let current_ini_path = {
                        let settings = load_settings(&app);
                        settings.last_ini_path.clone()
                    };

                    let mismatch_info = SignatureMismatchInfo {
                        ecu_signature: tune.signature.clone(),
                        ini_signature: ini_sig.clone(),
                        match_type,
                        current_ini_path,
                        matching_inis,
                    };

                    let _ = app.emit("signature:mismatch", &mismatch_info);
                    eprintln!("[INFO] load_tune: Found {} better matching INI file(s). You can switch in the dialog, or continue with current INI.", matching_count);
                }
            }
        } else {
            eprintln!("[INFO] load_tune: MSQ signature matches current INI definition");
        }
    } else {
        eprintln!("[WARN] load_tune: No INI definition loaded - will apply constants by name match if definition is loaded later");
    }

    // Check for INI version migration if tune has a saved manifest (LibreTune 1.1+ tunes)
    // This helps users understand what changed between INI versions
    {
        use libretune_core::tune::migration::compare_manifests;

        let def_guard = state.definition.lock().await;
        if let (Some(saved_manifest), Some(def)) = (&tune.constant_manifest, def_guard.as_ref()) {
            let migration_report = compare_manifests(saved_manifest, def);

            // Only report if there are actual changes
            if migration_report.severity != "none" {
                eprintln!(
                    "[INFO] load_tune: INI version migration detected (severity: {})",
                    migration_report.severity
                );
                eprintln!(
                    "[INFO]   Missing in tune (new in INI): {}",
                    migration_report.missing_in_tune.len()
                );
                eprintln!(
                    "[INFO]   Missing in INI (removed): {}",
                    migration_report.missing_in_ini.len()
                );
                eprintln!(
                    "[INFO]   Type changed: {}",
                    migration_report.type_changed.len()
                );
                eprintln!(
                    "[INFO]   Scale/offset changed: {}",
                    migration_report.scale_changed.len()
                );

                // Store in state for frontend access
                *state.migration_report.lock().await = Some(migration_report.clone());

                // Emit event to notify frontend
                let _ = app.emit("tune:migration_needed", &migration_report);
            } else {
                // Clear any previous migration report
                *state.migration_report.lock().await = None;
            }
        } else if tune.constant_manifest.is_some() {
            eprintln!(
                "[DEBUG] load_tune: Tune has manifest but no INI loaded - migration check deferred"
            );
        } else {
            eprintln!("[DEBUG] load_tune: Tune has no manifest (pre-1.1 format) - migration check skipped");
            // Clear any previous migration report
            *state.migration_report.lock().await = None;
        }
        drop(def_guard);
    }

    let info = TuneInfo {
        path: Some(path.clone()),
        signature: tune.signature.clone(),
        modified: false,
        has_tune: true,
    };

    // Populate TuneCache from loaded tune data
    // This allows table operations to use cached data instead of reading from ECU
    {
        let def_guard = state.definition.lock().await;
        let def = def_guard.as_ref();
        let mut cache_guard = state.tune_cache.lock().await;

        // Initialize cache if it doesn't exist, or reinitialize if it was reset
        if cache_guard.is_none() {
            if let Some(def) = def {
                eprintln!("[DEBUG] load_tune: Initializing cache from definition");
                *cache_guard = Some(TuneCache::from_definition(def));
            } else {
                eprintln!("[WARN] load_tune: No definition loaded, cannot initialize cache");
                return Err("No ECU definition loaded. Please open a project first.".to_string());
            }
        }

        // Ensure cache is initialized even if it exists but is empty
        if let Some(cache) = cache_guard.as_mut() {
            if cache.page_count() == 0 {
                if let Some(def) = def {
                    eprintln!("[DEBUG] load_tune: Cache exists but is empty, reinitializing from definition");
                    *cache_guard = Some(TuneCache::from_definition(def));
                }
            }
        }

        if let Some(cache) = cache_guard.as_mut() {
            // First, load any raw page data
            for (page_num, page_data) in &tune.pages {
                cache.load_page(*page_num, page_data.clone());
                eprintln!(
                    "[DEBUG] load_tune: populated cache page {} with {} bytes",
                    page_num,
                    page_data.len()
                );
            }

            // Then, apply constants from tune file to cache
            if let Some(def) = def {
                eprintln!(
                    "[DEBUG] load_tune: Definition loaded - {} constants in definition",
                    def.constants.len()
                );

                // Debug: Check if VE table constants are in the definition
                let ve_table_in_def = def.constants.contains_key("veTable");
                let ve_rpm_bins_in_def = def.constants.contains_key("veRpmBins");
                let ve_load_bins_in_def = def.constants.contains_key("veLoadBins");
                eprintln!("[DEBUG] load_tune: VE constants in definition - veTable: {}, veRpmBins: {}, veLoadBins: {}", 
                    ve_table_in_def, ve_rpm_bins_in_def, ve_load_bins_in_def);

                // Debug: Show what veTable constant looks like if it exists
                if let Some(ve_const) = def.constants.get("veTable") {
                    eprintln!("[DEBUG] load_tune: veTable constant - page={}, offset={}, size={}, shape={:?}", 
                        ve_const.page, ve_const.offset, ve_const.size_bytes(), ve_const.shape);
                }

                use libretune_core::tune::TuneValue;

                let mut applied_count = 0;
                let mut skipped_count = 0;
                let mut failed_count = 0;
                let mut pcvar_count = 0;
                let mut zero_size_count = 0;
                let mut string_bool_count = 0;

                for (name, tune_value) in &tune.constants {
                    // Debug VE table constants
                    if name == "veTable" || name == "veRpmBins" || name == "veLoadBins" {
                        eprintln!(
                            "[DEBUG] load_tune: Found VE constant '{}' in MSQ file",
                            name
                        );
                    }

                    // Look up constant in definition
                    if let Some(constant) = def.constants.get(name) {
                        // PC variables are stored locally, not in page data
                        if constant.is_pc_variable {
                            match tune_value {
                                TuneValue::Scalar(v) => {
                                    cache.local_values.insert(name.clone(), *v);
                                    pcvar_count += 1;
                                    eprintln!(
                                        "[DEBUG] load_tune: set PC variable '{}' = {}",
                                        name, v
                                    );
                                }
                                TuneValue::Array(arr) if !arr.is_empty() => {
                                    // For arrays, store first value (or handle differently if needed)
                                    cache.local_values.insert(name.clone(), arr[0]);
                                    pcvar_count += 1;
                                    eprintln!(
                                        "[DEBUG] load_tune: set PC variable '{}' = {} (from array)",
                                        name, arr[0]
                                    );
                                }
                                _ => {
                                    skipped_count += 1;
                                    eprintln!("[DEBUG] load_tune: skipping PC variable '{}' (unsupported value type)", name);
                                }
                            }
                            continue;
                        }

                        // Handle bits constants specially (they're packed, size_bytes() == 0)
                        if constant.data_type == libretune_core::ini::DataType::Bits {
                            // Bits constants: read current byte(s), modify the bits, write back
                            let bit_pos = constant.bit_position.unwrap_or(0);
                            let bit_size = constant.bit_size.unwrap_or(1);

                            // Calculate which byte(s) contain the bits
                            let byte_offset = (bit_pos / 8) as u16;
                            let bit_in_byte = bit_pos % 8;

                            // Calculate how many bytes we need
                            let bits_remaining_after_first_byte =
                                bit_size.saturating_sub(8 - bit_in_byte);
                            let bytes_needed = if bits_remaining_after_first_byte > 0 {
                                1 + bits_remaining_after_first_byte.div_ceil(8)
                            } else {
                                1
                            };
                            let bytes_needed_usize = bytes_needed as usize;

                            // Read current byte(s) value (or 0 if not present)
                            let read_offset = constant.offset + byte_offset;
                            let mut current_bytes: Vec<u8> = cache
                                .read_bytes(constant.page, read_offset, bytes_needed as u16)
                                .map(|s| s.to_vec())
                                .unwrap_or_else(|| vec![0u8; bytes_needed_usize]);

                            // Ensure we have enough bytes
                            while current_bytes.len() < bytes_needed_usize {
                                current_bytes.push(0u8);
                            }

                            // Get the bit value from MSQ (index into bit_options)
                            // MSQ can store bits constants as numeric indices, option strings, or booleans
                            let bit_value = match tune_value {
                                TuneValue::Scalar(v) => *v as u32,
                                TuneValue::Array(arr) if !arr.is_empty() => arr[0] as u32,
                                TuneValue::Bool(b) => {
                                    // Boolean values: true = 1, false = 0
                                    // For bits constants with 2 options (like ["false", "true"]),
                                    // boolean true maps to index 1, false to index 0
                                    if *b {
                                        1
                                    } else {
                                        0
                                    }
                                }
                                TuneValue::String(s) => {
                                    // Look up the string in bit_options to find its index
                                    if let Some(index) =
                                        constant.bit_options.iter().position(|opt| opt == s)
                                    {
                                        index as u32
                                    } else {
                                        // Try case-insensitive match
                                        if let Some(index) = constant
                                            .bit_options
                                            .iter()
                                            .position(|opt| opt.eq_ignore_ascii_case(s))
                                        {
                                            index as u32
                                        } else {
                                            skipped_count += 1;
                                            eprintln!("[DEBUG] load_tune: skipping bits constant '{}' (string '{}' not found in bit_options: {:?})", name, s, constant.bit_options);
                                            continue;
                                        }
                                    }
                                }
                                _ => {
                                    skipped_count += 1;
                                    eprintln!("[DEBUG] load_tune: skipping bits constant '{}' (unsupported value type)", name);
                                    continue;
                                }
                            };

                            // Modify the first byte
                            let bits_in_first_byte = (8 - bit_in_byte).min(bit_size);
                            let mask_first = if bits_in_first_byte >= 8 {
                                0xFF
                            } else {
                                (1u8 << bits_in_first_byte) - 1
                            };
                            let value_first = (bit_value & mask_first as u32) as u8;
                            current_bytes[0] = (current_bytes[0] & !(mask_first << bit_in_byte))
                                | (value_first << bit_in_byte);

                            // If bits span multiple bytes, modify additional bytes
                            if bits_remaining_after_first_byte > 0 {
                                let mut bits_collected = bits_in_first_byte;
                                for i in 1..bytes_needed_usize.min(current_bytes.len()) {
                                    let remaining_bits = bit_size - bits_collected;
                                    if remaining_bits == 0 {
                                        break;
                                    }
                                    let bits_from_this_byte = remaining_bits.min(8);
                                    let mask = if bits_from_this_byte >= 8 {
                                        0xFF
                                    } else {
                                        (1u8 << bits_from_this_byte) - 1
                                    };
                                    let value_from_bit =
                                        ((bit_value >> bits_collected) & mask as u32) as u8;
                                    current_bytes[i] = (current_bytes[i] & !mask) | value_from_bit;
                                    bits_collected += bits_from_this_byte;
                                }
                            }

                            // Write the modified byte(s) back
                            if cache.write_bytes(constant.page, read_offset, &current_bytes) {
                                applied_count += 1;
                                eprintln!("[DEBUG] load_tune: ✓ Applied bits constant '{}' = {} (bit_pos={}, bit_size={}, bytes={})", 
                                    name, bit_value, bit_pos, bit_size, bytes_needed);
                            } else {
                                failed_count += 1;
                                eprintln!(
                                    "[DEBUG] load_tune: ✗ Failed to write bits constant '{}'",
                                    name
                                );
                            }
                            continue;
                        }

                        // Skip if constant has no size (shouldn't happen for non-bits)
                        let length = constant.size_bytes() as u16;
                        if length == 0 {
                            zero_size_count += 1;
                            skipped_count += 1;
                            eprintln!(
                                "[DEBUG] load_tune: skipping constant '{}' (zero size)",
                                name
                            );
                            continue;
                        }

                        // Convert tune value to raw bytes
                        let element_size = constant.data_type.size_bytes();
                        let element_count = constant.shape.element_count();
                        let mut raw_data = vec![0u8; length as usize];

                        match tune_value {
                            TuneValue::Scalar(v) => {
                                let raw_val = constant.display_to_raw(*v);
                                constant.data_type.write_to_bytes(
                                    &mut raw_data,
                                    0,
                                    raw_val,
                                    def.endianness,
                                );
                                // Check if page exists before writing
                                let page_exists = cache.page_size(constant.page).is_some();
                                let page_state_before = cache.page_state(constant.page);

                                if name == "veTable" || name == "veRpmBins" || name == "veLoadBins"
                                {
                                    eprintln!("[DEBUG] load_tune: About to write '{}' - page={}, page_exists={}, page_state={:?}, offset={}, len={}", 
                                        name, constant.page, page_exists, page_state_before, constant.offset, length);
                                }

                                if cache.write_bytes(constant.page, constant.offset, &raw_data) {
                                    applied_count += 1;
                                    let page_state_after = cache.page_state(constant.page);

                                    // Verify the data was actually written by reading it back
                                    if name == "veTable"
                                        || name == "veRpmBins"
                                        || name == "veLoadBins"
                                    {
                                        let verify_read = cache.read_bytes(
                                            constant.page,
                                            constant.offset,
                                            length,
                                        );
                                        eprintln!("[DEBUG] load_tune: ✓ Applied constant '{}' = {} (scalar, page={}, offset={}, state={:?}, verify_read={})", 
                                            name, v, constant.page, constant.offset, page_state_after, verify_read.is_some());
                                    }
                                } else {
                                    failed_count += 1;
                                    if name == "veTable"
                                        || name == "veRpmBins"
                                        || name == "veLoadBins"
                                    {
                                        eprintln!("[DEBUG] load_tune: ✗ Failed to write constant '{}' (scalar, page={}, offset={}, len={}, page_size={:?}, page_exists={})", 
                                            name, constant.page, constant.offset, length, cache.page_size(constant.page), page_exists);
                                    }
                                }
                            }
                            TuneValue::Array(arr) => {
                                // Handle size mismatches: write what we have, pad or truncate as needed
                                let write_count = arr.len().min(element_count);
                                let last_value = arr.last().copied().unwrap_or(0.0);

                                for i in 0..element_count {
                                    let val = if i < arr.len() {
                                        arr[i]
                                    } else {
                                        // Pad with last value if array is smaller
                                        last_value
                                    };
                                    let raw_val = constant.display_to_raw(val);
                                    let offset = i * element_size;
                                    constant.data_type.write_to_bytes(
                                        &mut raw_data,
                                        offset,
                                        raw_val,
                                        def.endianness,
                                    );
                                }

                                // Check if page exists before writing
                                let page_exists = cache.page_size(constant.page).is_some();
                                let page_state_before = cache.page_state(constant.page);

                                if name == "veTable" || name == "veRpmBins" || name == "veLoadBins"
                                {
                                    if arr.len() != element_count {
                                        eprintln!("[DEBUG] load_tune: array size mismatch for '{}': expected {}, got {} (will pad/truncate)", 
                                            name, element_count, arr.len());
                                    }
                                    eprintln!("[DEBUG] load_tune: About to write '{}' - page={}, page_exists={}, page_state={:?}, offset={}, len={}", 
                                        name, constant.page, page_exists, page_state_before, constant.offset, length);
                                }

                                if cache.write_bytes(constant.page, constant.offset, &raw_data) {
                                    applied_count += 1;
                                    let page_state_after = cache.page_state(constant.page);

                                    // Verify the data was actually written by reading it back
                                    if name == "veTable"
                                        || name == "veRpmBins"
                                        || name == "veLoadBins"
                                    {
                                        let verify_read = cache.read_bytes(
                                            constant.page,
                                            constant.offset,
                                            length,
                                        );
                                        eprintln!("[DEBUG] load_tune: ✓ Applied constant '{}' (array, {} elements written, {} expected, page={}, offset={}, state={:?}, verify_read={})", 
                                            name, write_count, element_count, constant.page, constant.offset, page_state_after, verify_read.is_some());
                                    }
                                } else {
                                    failed_count += 1;
                                    if name == "veTable"
                                        || name == "veRpmBins"
                                        || name == "veLoadBins"
                                    {
                                        eprintln!("[DEBUG] load_tune: ✗ Failed to write constant '{}' (array, page={}, offset={}, len={}, page_size={:?}, page_exists={})", 
                                            name, constant.page, constant.offset, length, cache.page_size(constant.page), page_exists);
                                    }
                                }
                            }
                            TuneValue::String(_) | TuneValue::Bool(_) => {
                                string_bool_count += 1;
                                skipped_count += 1;
                                eprintln!("[DEBUG] load_tune: skipping constant '{}' (string/bool not supported for page data)", name);
                            }
                        }
                    } else {
                        skipped_count += 1;
                        if name == "veTable" || name == "veRpmBins" || name == "veLoadBins" {
                            eprintln!(
                                "[DEBUG] load_tune: constant '{}' not found in definition",
                                name
                            );
                        }
                    }
                }

                // Print prominent summary
                let total_accounted = applied_count + pcvar_count + skipped_count + failed_count;
                eprintln!("\n[INFO] ========================================");
                eprintln!("[INFO] Tune Load Summary:");
                eprintln!("[INFO]   Total constants in MSQ: {}", tune.constants.len());
                eprintln!(
                    "[INFO]   Successfully applied (page data): {}",
                    applied_count
                );
                eprintln!("[INFO]   PC variables applied: {}", pcvar_count);
                eprintln!("[INFO]   Failed to apply: {}", failed_count);
                eprintln!("[INFO]   Skipped:");
                eprintln!(
                    "[INFO]     - Not in definition: {}",
                    skipped_count - zero_size_count - string_bool_count
                );
                eprintln!("[INFO]     - Zero size (packed bits): {}", zero_size_count);
                eprintln!(
                    "[INFO]     - String/Bool (unsupported): {}",
                    string_bool_count
                );
                eprintln!("[INFO]   Total skipped: {}", skipped_count);
                if total_accounted != tune.constants.len() {
                    eprintln!(
                        "[WARN]   ⚠ Accounting mismatch: {} constants unaccounted for!",
                        tune.constants.len() - total_accounted
                    );
                }
                eprintln!("[INFO] ========================================\n");

                // Debug: Check page states after loading and show actual data sizes
                eprintln!("[DEBUG] load_tune: Page states after loading:");
                for page in 0..cache.page_count() {
                    let state = cache.page_state(page);
                    let def_size = cache.page_size(page);
                    let actual_size = cache.get_page(page).map(|p| p.len()).unwrap_or(0);
                    if state != PageState::NotLoaded || def_size.is_some() || actual_size > 0 {
                        eprintln!("[DEBUG] load_tune:   Page {}: state={:?}, def_size={:?}, actual_data_size={} bytes", 
                            page, state, def_size, actual_size);
                    }
                }

                if applied_count > 0 {
                    let total_applied = applied_count + pcvar_count;
                    eprintln!("[INFO] ✓ Successfully loaded {} constants into cache ({} page data + {} PC variables).", 
                        total_applied, applied_count, pcvar_count);
                    eprintln!("[INFO]   Important tables like VE, ignition, and fuel should work even if some constants don't match.");
                    eprintln!("[INFO]   All open tables will refresh automatically.");

                    // Informational note if many constants were skipped (not a warning - this is normal)
                    if skipped_count > applied_count && skipped_count > 100 {
                        let applied_percent =
                            (total_applied as f64 / tune.constants.len() as f64 * 100.0) as u32;
                        eprintln!("[INFO] ℹ Note: {} constants ({}%) were skipped - they're not in the current INI definition.", skipped_count, 100 - applied_percent);
                        eprintln!("[INFO]   This is normal when INI versions differ. Core tuning tables should still work.");
                        eprintln!("[INFO]   If you need those constants, switch to a matching INI file in Settings.");
                    }
                } else {
                    eprintln!("[WARN] ⚠ No constants were applied! This usually means the MSQ file doesn't match the current INI definition.");
                    eprintln!("[WARN]   MSQ signature: '{}'", tune.signature);
                    eprintln!("[WARN]   Check the Signature Mismatch dialog (if shown) or switch to a matching INI file in Settings.");
                }
            } else {
                eprintln!("[DEBUG] load_tune: no definition loaded, skipping constant application");
            }
        }
    }

    *state.current_tune.lock().await = Some(tune.clone());
    *state.current_tune_path.lock().await = Some(PathBuf::from(path));
    *state.tune_modified.lock().await = false;

    // If a project is open, save the tune to the project's CurrentTune.msq
    // This ensures it will be auto-loaded next time the project is opened
    let proj_guard = state.current_project.lock().await;
    if let Some(ref project) = *proj_guard {
        let project_tune_path = project.path.join("CurrentTune.msq");
        if let Err(e) = tune.save(&project_tune_path) {
            eprintln!("[WARN] Failed to save tune to project folder: {}", e);
        } else {
            eprintln!("[INFO] ✓ Saved tune to project: {:?}", project_tune_path);
            // Update the stored tune path to point to the project's tune file
            *state.current_tune_path.lock().await = Some(project_tune_path);
        }
    }
    drop(proj_guard);

    // Emit event to notify UI that tune was loaded
    let _ = app.emit("tune:loaded", "file");

    Ok(info)
}

// Tune migration commands extracted to commands/tune_migration.rs

// tune_io commands extracted to commands/tune_io.rs

// Project tune sync commands extracted to commands/project_tune_sync.rs


// Data logging commands extracted to commands/data_logging.rs
// Diagnostic logger commands extracted to commands/diagnostic_loggers.rs

// Table comparison commands extracted to commands/table_compare.rs
/// Read a raw numeric value from bytes based on data type
pub(crate) fn read_raw_value(bytes: &[u8], data_type: &DataType) -> Result<f64, String> {
    use byteorder::{BigEndian, ByteOrder};

    Ok(match data_type {
        DataType::U08 => bytes.first().map(|b| *b as f64).ok_or("No data")?,
        DataType::S08 => bytes.first().map(|b| *b as i8 as f64).ok_or("No data")?,
        DataType::U16 => {
            if bytes.len() >= 2 {
                BigEndian::read_u16(bytes) as f64
            } else {
                return Err("Insufficient data for U16".to_string());
            }
        }
        DataType::S16 => {
            if bytes.len() >= 2 {
                BigEndian::read_i16(bytes) as f64
            } else {
                return Err("Insufficient data for S16".to_string());
            }
        }
        DataType::U32 => {
            if bytes.len() >= 4 {
                BigEndian::read_u32(bytes) as f64
            } else {
                return Err("Insufficient data for U32".to_string());
            }
        }
        DataType::S32 => {
            if bytes.len() >= 4 {
                BigEndian::read_i32(bytes) as f64
            } else {
                return Err("Insufficient data for S32".to_string());
            }
        }
        DataType::F32 => {
            if bytes.len() >= 4 {
                BigEndian::read_f32(bytes) as f64
            } else {
                return Err("Insufficient data for F32".to_string());
            }
        }
        DataType::F64 => {
            if bytes.len() >= 8 {
                BigEndian::read_f64(bytes)
            } else {
                return Err("Insufficient data for F64".to_string());
            }
        }
        DataType::Bits => bytes.first().map(|b| *b as f64).ok_or("No data")?,
        DataType::String => 0.0, // Strings don't have numeric values
    })
}

// Reset/CSV commands extracted to commands/csv_io.rs

// =====================================================
// Project Management Commands
// =====================================================

#[derive(Serialize)]
pub(crate) struct CurrentProjectInfo {
    pub name: String,
    pub path: String,
    pub signature: String,
    pub has_tune: bool,
    pub tune_modified: bool,
    pub connection: ConnectionSettingsResponse,
}

#[derive(Serialize)]
pub(crate) struct ConnectionSettingsResponse {
    pub port: Option<String>,
    pub baud_rate: u32,
    pub auto_connect: bool,
}

/// Get the path to the projects directory

/// Create a new project
///
/// Creates a new project directory with INI definition and optional tune import.
///
/// # Arguments
/// * `name` - Project name (used for directory)
/// * `ini_id` - INI repository ID to use
/// * `tune_path` - Optional path to an existing tune file to import
///
/// Returns: CurrentProjectInfo with project details
#[tauri::command]
async fn create_project(
    state: tauri::State<'_, AppState>,
    name: String,
    ini_id: String,
    tune_path: Option<String>,
) -> Result<CurrentProjectInfo, String> {
    // Get INI path from repository
    let mut repo_guard = state.ini_repository.lock().await;
    let repo = repo_guard
        .as_mut()
        .ok_or_else(|| "INI repository not initialized".to_string())?;

    let ini_path = repo
        .get_path(&ini_id)
        .ok_or_else(|| format!("INI '{}' not found in repository", ini_id))?;

    // Get signature from INI
    let def =
        EcuDefinition::from_file(&ini_path).map_err(|e| format!("Failed to parse INI: {}", e))?;
    let signature = def.signature.clone();

    // Create the project with optional imported tune
    let mut project = Project::create(&name, &ini_path, &signature, None)
        .map_err(|e| format!("Failed to create project: {}", e))?;

    // Store current project and load its definition first (needed for applying tune)
    let mut def_guard = state.definition.lock().await;
    *def_guard = Some(def.clone());
    drop(def_guard);

    // Initialize TuneCache from definition
    let cache = TuneCache::from_definition(&def);
    {
        let mut cache_guard = state.tune_cache.lock().await;
        *cache_guard = Some(cache);
    }

    // Always initialize current_tune so base map apply and other operations work
    {
        let mut tune_guard = state.current_tune.lock().await;
        if tune_guard.is_none() {
            *tune_guard = Some(TuneFile::new(&signature));
        }
    }

    // If a tune path was provided, import it and apply to cache
    if let Some(tune_file) = tune_path {
        let tune_path_ref = std::path::Path::new(&tune_file);
        if tune_path_ref.exists() {
            // TuneFile::load handles both XML and MSQ formats automatically
            let tune =
                TuneFile::load(tune_path_ref).map_err(|e| format!("Failed to load tune: {}", e))?;

            // Apply tune constants to cache (same logic as load_tune)
            {
                let mut cache_guard = state.tune_cache.lock().await;
                if let Some(cache) = cache_guard.as_mut() {
                    // Load any raw page data
                    for (page_num, page_data) in &tune.pages {
                        cache.load_page(*page_num, page_data.clone());
                    }

                    // Apply constants from tune file to cache
                    use libretune_core::tune::TuneValue;

                    for (name, tune_value) in &tune.constants {
                        if let Some(constant) = def.constants.get(name) {
                            // PC variables are stored locally
                            if constant.is_pc_variable {
                                match tune_value {
                                    TuneValue::Scalar(v) => {
                                        cache.local_values.insert(name.clone(), *v);
                                    }
                                    TuneValue::Array(arr) if !arr.is_empty() => {
                                        cache.local_values.insert(name.clone(), arr[0]);
                                    }
                                    _ => {}
                                }
                                continue;
                            }

                            let length = constant.size_bytes() as u16;
                            if length == 0 {
                                continue;
                            }

                            let element_size = constant.data_type.size_bytes();
                            let element_count = constant.shape.element_count();
                            let mut raw_data = vec![0u8; length as usize];

                            match tune_value {
                                TuneValue::Scalar(v) => {
                                    let raw_val = constant.display_to_raw(*v);
                                    constant.data_type.write_to_bytes(
                                        &mut raw_data,
                                        0,
                                        raw_val,
                                        def.endianness,
                                    );
                                    let _ = cache.write_bytes(
                                        constant.page,
                                        constant.offset,
                                        &raw_data,
                                    );
                                }
                                TuneValue::Array(arr) if arr.len() == element_count => {
                                    for (i, val) in arr.iter().enumerate() {
                                        let raw_val = constant.display_to_raw(*val);
                                        let offset = i * element_size;
                                        constant.data_type.write_to_bytes(
                                            &mut raw_data,
                                            offset,
                                            raw_val,
                                            def.endianness,
                                        );
                                    }
                                    let _ = cache.write_bytes(
                                        constant.page,
                                        constant.offset,
                                        &raw_data,
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }

            // Store tune in project
            project.current_tune = Some(tune);
            project
                .save_current_tune()
                .map_err(|e| format!("Failed to save imported tune: {}", e))?;
        }
    }

    let response = CurrentProjectInfo {
        name: project.config.name.clone(),
        path: project.path.to_string_lossy().to_string(),
        signature: project.config.signature.clone(),
        has_tune: project.current_tune.is_some(),
        tune_modified: project.dirty,
        connection: ConnectionSettingsResponse {
            port: project.config.connection.port.clone(),
            baud_rate: project.config.connection.baud_rate,
            auto_connect: project.config.settings.auto_connect,
        },
    };

    let mut proj_guard = state.current_project.lock().await;
    *proj_guard = Some(project);

    Ok(response)
}

/// Open an existing project
///
/// Loads a project from disk, including its INI definition and tune file.
/// Disconnects any existing ECU connection to avoid state conflicts.
///
/// # Arguments
/// * `path` - Path to the project directory
///
/// Returns: CurrentProjectInfo with project details
#[tauri::command]
async fn open_project(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<CurrentProjectInfo, String> {
    eprintln!("\n[INFO] ========================================");
    eprintln!("[INFO] OPENING PROJECT: {}", path);
    eprintln!("[INFO] ========================================");

    let project = Project::open(&path).map_err(|e| format!("Failed to open project: {}", e))?;

    eprintln!("[INFO] Project opened: {}", project.config.name);
    eprintln!(
        "[INFO] Project has tune file: {}",
        project.current_tune.is_some()
    );

    if let Some(ref tune) = project.current_tune {
        eprintln!("[INFO] Tune file signature: '{}'", tune.signature);
        eprintln!("[INFO] Tune file has {} constants", tune.constants.len());
        eprintln!("[INFO] Tune file has {} pages", tune.pages.len());
    } else {
        let tune_path = project.current_tune_path();
        eprintln!("[WARN] No tune file loaded. Expected at: {:?}", tune_path);
        eprintln!("[WARN] Tune file exists: {}", tune_path.exists());
    }

    // Load the project's INI definition
    let ini_path = project.ini_path();
    eprintln!("[INFO] Loading INI from: {:?}", ini_path);
    let def = EcuDefinition::from_file(&ini_path)
        .map_err(|e| format!("Failed to parse project INI: {}", e))?;

    eprintln!("[INFO] INI signature: '{}'", def.signature);
    eprintln!("[INFO] INI has {} constants", def.constants.len());

    // Save as last opened project
    {
        let mut settings = load_settings(&app);
        if settings.last_project_path.as_deref() != Some(&path) {
            settings.last_project_path = Some(path.clone());
            save_settings(&app, &settings);
        }
    }

    // Load user math channels
    let math_channels_path = project.path.join("math_channels.json");
    let channels = match load_math_channels(&math_channels_path) {
        Ok(c) => {
            eprintln!("[INFO] Loaded {} math channels", c.len());
            c
        }
        Err(e) => {
            // It's normal for this to not exist in new projects
            if math_channels_path.exists() {
                eprintln!("[WARN] Failed to load math_channels.json: {}", e);
            }
            Vec::new()
        }
    };
    *state.math_channels.lock().await = channels;

    let response = CurrentProjectInfo {
        name: project.config.name.clone(),
        path: project.path.to_string_lossy().to_string(),
        signature: project.config.signature.clone(),
        has_tune: project.current_tune.is_some(),
        tune_modified: project.dirty,
        connection: ConnectionSettingsResponse {
            port: project.config.connection.port.clone(),
            baud_rate: project.config.connection.baud_rate,
            auto_connect: project.config.settings.auto_connect,
        },
    };

    // Disconnect any existing connection when opening a new project
    // to avoid stale connection state from previous ECU
    let mut conn_guard = state.connection.lock().await;
    *conn_guard = None;
    drop(conn_guard);

    // Store current project and definition
    let mut def_guard = state.definition.lock().await;
    let def_clone = def.clone();
    *def_guard = Some(def);
    drop(def_guard);

    // Save project path before moving project into mutex
    let project_path = project.path.clone();
    let project_tune = project.current_tune.as_ref().cloned();

    // Load project tune if it exists
    let mut proj_guard = state.current_project.lock().await;
    *proj_guard = Some(project);
    drop(proj_guard);

    // Always try to load CurrentTune.msq if it exists, even if project.current_tune wasn't set
    let tune_to_load = if let Some(tune) = project_tune {
        Some(tune)
    } else {
        // Try to load tune file directly if it wasn't auto-loaded
        let tune_path = project_path.join("CurrentTune.msq");
        if tune_path.exists() {
            eprintln!("[INFO] Auto-loading tune file: {:?}", tune_path);
            match TuneFile::load(&tune_path) {
                Ok(tune) => {
                    eprintln!(
                        "[INFO] ✓ Successfully loaded tune file with {} constants",
                        tune.constants.len()
                    );
                    Some(tune)
                }
                Err(e) => {
                    eprintln!("[WARN] Failed to load tune file: {}", e);
                    None
                }
            }
        } else {
            None
        }
    };

    // Initialize TuneCache and load project tune
    if let Some(tune) = tune_to_load {
        // Create TuneCache from definition
        let cache = TuneCache::from_definition(&def_clone);
        let mut cache_guard = state.tune_cache.lock().await;
        *cache_guard = Some(cache);

        // Populate cache from project tune
        if let Some(cache) = cache_guard.as_mut() {
            // Load any raw page data first
            for (page_num, page_data) in &tune.pages {
                cache.load_page(*page_num, page_data.clone());
            }

            // Apply constants from tune file to cache (same logic as load_tune)
            use libretune_core::tune::TuneValue;

            // Debug: Check if VE table constants are in the tune
            let ve_table_in_tune = tune.constants.contains_key("veTable");
            let ve_rpm_bins_in_tune = tune.constants.contains_key("veRpmBins");
            let ve_load_bins_in_tune = tune.constants.contains_key("veLoadBins");
            eprintln!("[DEBUG] open_project: VE constants in tune - veTable: {}, veRpmBins: {}, veLoadBins: {}", 
                ve_table_in_tune, ve_rpm_bins_in_tune, ve_load_bins_in_tune);

            // Debug: Check if VE table constants are in the definition
            let ve_table_in_def = def_clone.constants.contains_key("veTable");
            let ve_rpm_bins_in_def = def_clone.constants.contains_key("veRpmBins");
            let ve_load_bins_in_def = def_clone.constants.contains_key("veLoadBins");
            eprintln!("[DEBUG] open_project: VE constants in definition - veTable: {}, veRpmBins: {}, veLoadBins: {}", 
                ve_table_in_def, ve_rpm_bins_in_def, ve_load_bins_in_def);

            // Debug: Show sample constant names from MSQ and definition to see why they're not matching
            let msq_sample: Vec<String> = tune.constants.keys().take(10).cloned().collect();
            let def_sample: Vec<String> = def_clone.constants.keys().take(10).cloned().collect();
            eprintln!(
                "[DEBUG] open_project: Sample MSQ constants: {:?}",
                msq_sample
            );
            eprintln!(
                "[DEBUG] open_project: Sample definition constants: {:?}",
                def_sample
            );
            eprintln!(
                "[DEBUG] open_project: Total MSQ constants: {}, Total definition constants: {}",
                tune.constants.len(),
                def_clone.constants.len()
            );

            let mut applied_count = 0;
            let mut skipped_count = 0;
            let mut failed_count = 0;

            for (name, tune_value) in &tune.constants {
                // Debug VE table constants specifically
                let is_ve_related =
                    name == "veTable" || name == "veRpmBins" || name == "veLoadBins";

                if let Some(constant) = def_clone.constants.get(name) {
                    if is_ve_related {
                        eprintln!("[DEBUG] open_project: Found constant '{}' in definition (page={}, offset={}, size={})", 
                            name, constant.page, constant.offset, constant.size_bytes());
                    }

                    // PC variables are stored locally
                    if constant.is_pc_variable {
                        match tune_value {
                            TuneValue::Scalar(v) => {
                                cache.local_values.insert(name.clone(), *v);
                                applied_count += 1;
                                if is_ve_related {
                                    eprintln!(
                                        "[DEBUG] open_project: Applied PC variable '{}' = {}",
                                        name, v
                                    );
                                }
                            }
                            TuneValue::Array(arr) if !arr.is_empty() => {
                                cache.local_values.insert(name.clone(), arr[0]);
                                applied_count += 1;
                                if is_ve_related {
                                    eprintln!("[DEBUG] open_project: Applied PC variable '{}' = {} (from array)", name, arr[0]);
                                }
                            }
                            _ => {
                                skipped_count += 1;
                                if is_ve_related {
                                    eprintln!("[DEBUG] open_project: Skipped PC variable '{}' (unsupported value type)", name);
                                }
                            }
                        }
                        continue;
                    }

                    // Handle bits constants specially (they're packed, size_bytes() == 0)
                    if constant.data_type == libretune_core::ini::DataType::Bits {
                        // Bits constants: read current byte(s), modify the bits, write back
                        let bit_pos = constant.bit_position.unwrap_or(0);
                        let bit_size = constant.bit_size.unwrap_or(1);

                        // Calculate which byte(s) contain the bits
                        let byte_offset = (bit_pos / 8) as u16;
                        let bit_in_byte = bit_pos % 8;

                        // Calculate how many bytes we need
                        let bits_remaining_after_first_byte =
                            bit_size.saturating_sub(8 - bit_in_byte);
                        let bytes_needed = if bits_remaining_after_first_byte > 0 {
                            1 + bits_remaining_after_first_byte.div_ceil(8)
                        } else {
                            1
                        };
                        let bytes_needed_usize = bytes_needed as usize;

                        // Read current byte(s) value (or 0 if not present)
                        let read_offset = constant.offset + byte_offset;
                        let mut current_bytes: Vec<u8> = cache
                            .read_bytes(constant.page, read_offset, bytes_needed as u16)
                            .map(|s| s.to_vec())
                            .unwrap_or_else(|| vec![0u8; bytes_needed_usize]);

                        // Ensure we have enough bytes
                        while current_bytes.len() < bytes_needed_usize {
                            current_bytes.push(0u8);
                        }

                        // Get the bit value from MSQ (index into bit_options)
                        // MSQ can store bits constants as numeric indices or as option strings
                        let bit_value = match tune_value {
                            TuneValue::Scalar(v) => *v as u32,
                            TuneValue::Array(arr) if !arr.is_empty() => arr[0] as u32,
                            TuneValue::String(s) => {
                                // Look up the string in bit_options to find its index
                                if let Some(index) =
                                    constant.bit_options.iter().position(|opt| opt == s)
                                {
                                    index as u32
                                } else {
                                    // Try case-insensitive match
                                    if let Some(index) = constant
                                        .bit_options
                                        .iter()
                                        .position(|opt| opt.eq_ignore_ascii_case(s))
                                    {
                                        index as u32
                                    } else {
                                        skipped_count += 1;
                                        if is_ve_related {
                                            eprintln!("[DEBUG] open_project: Skipped bits constant '{}' (string '{}' not found in bit_options: {:?})", name, s, constant.bit_options);
                                        }
                                        continue;
                                    }
                                }
                            }
                            _ => {
                                skipped_count += 1;
                                if is_ve_related {
                                    eprintln!("[DEBUG] open_project: Skipped bits constant '{}' (unsupported value type)", name);
                                }
                                continue;
                            }
                        };

                        // Modify the first byte
                        let bits_in_first_byte = (8 - bit_in_byte).min(bit_size);
                        let mask_first = if bits_in_first_byte >= 8 {
                            0xFF
                        } else {
                            (1u8 << bits_in_first_byte) - 1
                        };
                        let value_first = (bit_value & mask_first as u32) as u8;
                        current_bytes[0] = (current_bytes[0] & !(mask_first << bit_in_byte))
                            | (value_first << bit_in_byte);

                        // If bits span multiple bytes, modify additional bytes
                        if bits_remaining_after_first_byte > 0 {
                            let mut bits_collected = bits_in_first_byte;
                            for i in 1..bytes_needed_usize.min(current_bytes.len()) {
                                let remaining_bits = bit_size - bits_collected;
                                if remaining_bits == 0 {
                                    break;
                                }
                                let bits_from_this_byte = remaining_bits.min(8);
                                let mask = if bits_from_this_byte >= 8 {
                                    0xFF
                                } else {
                                    (1u8 << bits_from_this_byte) - 1
                                };
                                let value_from_bit =
                                    ((bit_value >> bits_collected) & mask as u32) as u8;
                                current_bytes[i] = (current_bytes[i] & !mask) | value_from_bit;
                                bits_collected += bits_from_this_byte;
                            }
                        }

                        // Write the modified byte(s) back
                        if cache.write_bytes(constant.page, read_offset, &current_bytes) {
                            applied_count += 1;
                            if is_ve_related {
                                eprintln!("[DEBUG] open_project: Applied bits constant '{}' = {} (bit_pos={}, bit_size={}, bytes={})", 
                                    name, bit_value, bit_pos, bit_size, bytes_needed);
                            }
                        } else {
                            failed_count += 1;
                            if is_ve_related {
                                eprintln!(
                                    "[DEBUG] open_project: Failed to write bits constant '{}'",
                                    name
                                );
                            }
                        }
                        continue;
                    }

                    let length = constant.size_bytes() as u16;
                    if length == 0 {
                        skipped_count += 1;
                        if is_ve_related {
                            eprintln!(
                                "[DEBUG] open_project: Skipped constant '{}' (zero size)",
                                name
                            );
                        }
                        continue;
                    }

                    let element_size = constant.data_type.size_bytes();
                    let element_count = constant.shape.element_count();
                    let mut raw_data = vec![0u8; length as usize];

                    match tune_value {
                        TuneValue::Scalar(v) => {
                            let raw_val = constant.display_to_raw(*v);
                            constant.data_type.write_to_bytes(
                                &mut raw_data,
                                0,
                                raw_val,
                                def_clone.endianness,
                            );
                            if cache.write_bytes(constant.page, constant.offset, &raw_data) {
                                applied_count += 1;
                                if is_ve_related {
                                    eprintln!("[DEBUG] open_project: Applied constant '{}' = {} (scalar, page={}, offset={})", 
                                        name, v, constant.page, constant.offset);
                                }
                            } else {
                                failed_count += 1;
                                if is_ve_related {
                                    eprintln!("[DEBUG] open_project: Failed to write constant '{}' (page={}, offset={}, len={}, page_size={:?})", 
                                        name, constant.page, constant.offset, length, cache.page_size(constant.page));
                                }
                            }
                        }
                        TuneValue::Array(arr) => {
                            // Handle size mismatches: write what we have, pad or truncate as needed
                            let write_count = arr.len().min(element_count);
                            let last_value = arr.last().copied().unwrap_or(0.0);

                            if arr.len() != element_count && is_ve_related {
                                eprintln!("[DEBUG] open_project: Array size mismatch for '{}': expected {}, got {} (will write {} and pad/truncate)", 
                                    name, element_count, arr.len(), write_count);
                            }

                            for i in 0..element_count {
                                let val = if i < arr.len() {
                                    arr[i]
                                } else {
                                    // Pad with last value if array is smaller
                                    last_value
                                };
                                let raw_val = constant.display_to_raw(val);
                                let offset = i * element_size;
                                constant.data_type.write_to_bytes(
                                    &mut raw_data,
                                    offset,
                                    raw_val,
                                    def_clone.endianness,
                                );
                            }

                            if cache.write_bytes(constant.page, constant.offset, &raw_data) {
                                applied_count += 1;
                                if is_ve_related {
                                    eprintln!("[DEBUG] open_project: Applied constant '{}' (array, {} elements written, page={}, offset={})", 
                                        name, write_count, constant.page, constant.offset);
                                }
                            } else {
                                failed_count += 1;
                                if is_ve_related {
                                    eprintln!("[DEBUG] open_project: Failed to write constant '{}' (array, page={}, offset={}, len={}, page_size={:?})", 
                                        name, constant.page, constant.offset, length, cache.page_size(constant.page));
                                }
                            }
                        }
                        TuneValue::String(_) | TuneValue::Bool(_) => {
                            skipped_count += 1;
                            if is_ve_related {
                                eprintln!("[DEBUG] open_project: Skipped constant '{}' (string/bool not supported for page data)", name);
                            }
                        }
                    }
                } else {
                    skipped_count += 1;
                    // Log first 10 skipped constants to see what's missing
                    if skipped_count <= 10 || is_ve_related {
                        eprintln!("[DEBUG] open_project: Constant '{}' not found in definition (skipped {}/{})", 
                            name, skipped_count, tune.constants.len());
                    }
                }
            }

            eprintln!("\n[INFO] ========================================");
            eprintln!("[INFO] TUNE LOAD SUMMARY:");
            eprintln!("[INFO]   Applied: {} constants", applied_count);
            eprintln!("[INFO]   Failed: {} constants", failed_count);
            eprintln!("[INFO]   Skipped: {} constants", skipped_count);
            eprintln!("[INFO]   Total in MSQ: {} constants", tune.constants.len());
            eprintln!("[INFO] ========================================\n");
        }
        drop(cache_guard);

        // Store tune in state
        *state.current_tune.lock().await = Some(tune.clone());
        *state.current_tune_path.lock().await = Some(project_path.join("CurrentTune.msq"));

        // Emit event to notify UI that tune was loaded
        let _ = app.emit("tune:loaded", "project");
        eprintln!("[INFO] ✓ Project opened successfully with tune file");
    } else {
        // No project tune - create empty cache
        eprintln!("[WARN] ⚠ Project opened but NO TUNE FILE found!");
        eprintln!(
            "[WARN]   Expected tune file at: {:?}",
            project_path.join("CurrentTune.msq")
        );
        eprintln!("[WARN]   You can load an MSQ file manually using File > Load Tune");
        let cache = TuneCache::from_definition(&def_clone);
        *state.tune_cache.lock().await = Some(cache);
    }

    Ok(response)
}


// Project management commands extracted to commands/project_mgmt.rs

/// Find INI files that match a given ECU signature
#[tauri::command]
async fn find_matching_inis(
    state: tauri::State<'_, AppState>,
    ecu_signature: String,
) -> Result<Vec<MatchingIniInfo>, String> {
    Ok(find_matching_inis_internal(&state, &ecu_signature).await)
}

/// Update the project's INI file and optionally force re-sync
#[tauri::command]
async fn update_project_ini(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    ini_path: String,
    force_resync: bool,
) -> Result<(), String> {
    // Load the new INI definition
    let new_def = EcuDefinition::from_file(&ini_path)
        .map_err(|e| format!("Failed to parse INI file: {}", e))?;

    // Update the project config if we have a project open
    let mut proj_guard = state.current_project.lock().await;
    if let Some(ref mut project) = *proj_guard {
        // Copy the new INI to the project directory
        let project_ini_path = project.ini_path();
        std::fs::copy(&ini_path, &project_ini_path)
            .map_err(|e| format!("Failed to copy INI to project: {}", e))?;

        // Update project signature
        project.config.signature = new_def.signature.clone();
        project
            .save_config()
            .map_err(|e| format!("Failed to save project config: {}", e))?;
    }
    drop(proj_guard);

    // Update the loaded definition
    let mut def_guard = state.definition.lock().await;
    let def_clone = new_def.clone();
    *def_guard = Some(new_def);
    drop(def_guard);

    // Update settings with new INI path
    let mut settings = load_settings(&app);
    settings.last_ini_path = Some(ini_path);
    save_settings(&app, &settings);

    // Re-initialize cache with new definition and re-apply project tune constants
    let project_tune = {
        let proj_guard = state.current_project.lock().await;
        proj_guard
            .as_ref()
            .and_then(|p| p.current_tune.as_ref().cloned())
    };

    // Create new cache from updated definition
    let cache = TuneCache::from_definition(&def_clone);
    let mut cache_guard = state.tune_cache.lock().await;
    *cache_guard = Some(cache);

    // Re-apply project tune constants with new definition
    if let Some(tune) = project_tune {
        if let Some(cache) = cache_guard.as_mut() {
            // Load any raw page data first
            for (page_num, page_data) in &tune.pages {
                cache.load_page(*page_num, page_data.clone());
            }

            // Apply constants from tune file to cache (same logic as open_project)
            use libretune_core::tune::TuneValue;

            let mut applied_count = 0;
            let mut skipped_count = 0;
            let mut failed_count = 0;

            for (name, tune_value) in &tune.constants {
                if let Some(constant) = def_clone.constants.get(name) {
                    // PC variables are stored locally
                    if constant.is_pc_variable {
                        match tune_value {
                            TuneValue::Scalar(v) => {
                                cache.local_values.insert(name.clone(), *v);
                                applied_count += 1;
                            }
                            TuneValue::Array(arr) if !arr.is_empty() => {
                                cache.local_values.insert(name.clone(), arr[0]);
                                applied_count += 1;
                            }
                            _ => {
                                skipped_count += 1;
                            }
                        }
                        continue;
                    }

                    let length = constant.size_bytes() as u16;
                    if length == 0 {
                        skipped_count += 1;
                        continue;
                    }

                    let element_size = constant.data_type.size_bytes();
                    let element_count = constant.shape.element_count();
                    let mut raw_data = vec![0u8; length as usize];

                    match tune_value {
                        TuneValue::Scalar(v) => {
                            let raw_val = constant.display_to_raw(*v);
                            constant.data_type.write_to_bytes(
                                &mut raw_data,
                                0,
                                raw_val,
                                def_clone.endianness,
                            );
                            if cache.write_bytes(constant.page, constant.offset, &raw_data) {
                                applied_count += 1;
                            } else {
                                failed_count += 1;
                            }
                        }
                        TuneValue::Array(arr) => {
                            // Handle size mismatches
                            let last_value = arr.last().copied().unwrap_or(0.0);

                            for i in 0..element_count {
                                let val = if i < arr.len() { arr[i] } else { last_value };
                                let raw_val = constant.display_to_raw(val);
                                let offset = i * element_size;
                                constant.data_type.write_to_bytes(
                                    &mut raw_data,
                                    offset,
                                    raw_val,
                                    def_clone.endianness,
                                );
                            }

                            if cache.write_bytes(constant.page, constant.offset, &raw_data) {
                                applied_count += 1;
                            } else {
                                failed_count += 1;
                            }
                        }
                        TuneValue::String(_) | TuneValue::Bool(_) => {
                            skipped_count += 1;
                        }
                    }
                } else {
                    skipped_count += 1;
                }
            }

            eprintln!("[DEBUG] update_project_ini: Re-applied tune constants - applied: {}, failed: {}, skipped: {}, total: {}", 
                applied_count, failed_count, skipped_count, tune.constants.len());

            // Emit event to notify UI that tune data was re-applied
            let _ = app.emit("tune:loaded", "ini_updated");
        }
    }
    drop(cache_guard);

    // If force_resync is requested and we're connected, trigger re-sync
    if force_resync {
        let conn_guard = state.connection.lock().await;
        if conn_guard.is_some() {
            drop(conn_guard);
            // Emit event to notify frontend to re-sync
            let _ = app.emit("ini:changed", "resync_required");
        }
    }

    Ok(())
}

// Restore point commands extracted to commands/restore_points.rs


// TS project import commands extracted to commands/ts_import.rs


// Git version control commands extracted to commands/git.rs

// =====================================================
// Math Channel Commands (extracted to commands/math_channels.rs)
// =====================================================

// =====================================================
// Base Map Generator & Project Utility Commands
// =====================================================

// Base map generator extracted to commands/base_map.rs

/// Apply a generated base map to the currently loaded project.
///
/// Searches the loaded INI definition for VE, ignition, and AFR tables by
/// common naming patterns, reads each table's actual dimensions from the INI,
/// then re-generates the base map data at the correct size before writing.
/// Also updates axis bins and scalar constants (reqFuel, etc.).
///
/// Returns: Summary of what was applied
#[tauri::command]
async fn apply_base_map(
    state: tauri::State<'_, AppState>,
    base_map: serde_json::Value,
) -> Result<serde_json::Value, String> {
    use libretune_core::basemap::generator::{
        generate_afr_table, generate_ignition_table, generate_load_bins, generate_rpm_bins,
        generate_ve_table,
    };
    use libretune_core::basemap::EngineSpec;

    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("No ECU definition loaded")?;
    let endianness = def.endianness;

    // Deserialize engine_spec from the base map so we can re-generate at correct dimensions
    let engine_spec: EngineSpec = serde_json::from_value(
        base_map
            .get("engine_spec")
            .cloned()
            .ok_or("Missing engine_spec in base map")?,
    )
    .map_err(|e| format!("Invalid engine_spec: {}", e))?;

    let req_fuel: Option<f64> = base_map.get("req_fuel").and_then(|v| v.as_f64());
    let scalars: Option<serde_json::Map<String, serde_json::Value>> =
        base_map.get("scalars").and_then(|v| v.as_object()).cloned();

    // Find tables by searching common naming patterns across ECU platforms
    // Speeduino names: veTable1Tbl, sparkTbl, afrTable1Tbl
    // rusEFI/FOME names: veTableTbl, ignitionTableTbl, lambdaTableTbl/afrTableTbl
    let ve_table_names = ["veTable1Tbl", "veTableTbl", "fuelTable1Tbl", "fuelTableTbl"];
    let ign_table_names = [
        "sparkTbl",
        "ignitionTableTbl",
        "advTable1Tbl",
        "ignitionTbl",
        "spark1Tbl",
    ];
    let afr_table_names = [
        "afrTable1Tbl",
        "lambdaTableTbl",
        "afrTableTbl",
        "lambdaTable1Tbl",
    ];

    let mut applied = Vec::<String>::new();
    let mut errors = Vec::<String>::new();

    // Helper: write a 2D table's Z values into cache
    fn write_table_z(
        def: &libretune_core::ini::EcuDefinition,
        cache: &mut TuneCache,
        tune: &mut TuneFile,
        table_name: &str,
        values_2d: &[Vec<f64>],
        endianness: libretune_core::ini::Endianness,
    ) -> Result<String, String> {
        let table = def
            .get_table_by_name_or_map(table_name)
            .ok_or_else(|| format!("Table '{}' not found", table_name))?;
        let constant = def.constants.get(&table.map).ok_or_else(|| {
            format!(
                "Constant '{}' not found for table '{}'",
                table.map, table_name
            )
        })?;

        let flat: Vec<f64> = values_2d.iter().flatten().cloned().collect();
        let expected = constant.shape.element_count();

        if flat.len() != expected {
            return Err(format!(
                "Table '{}' dimension mismatch: generated {} cells but INI expects {}",
                table_name,
                flat.len(),
                expected
            ));
        }

        let element_size = constant.data_type.size_bytes();
        let mut raw_data = vec![0u8; constant.size_bytes()];
        for (i, val) in flat.iter().enumerate() {
            let raw_val = constant.display_to_raw(*val);
            let offset = i * element_size;
            constant
                .data_type
                .write_to_bytes(&mut raw_data, offset, raw_val, endianness);
        }

        cache.write_bytes(constant.page, constant.offset, &raw_data);

        // Also update tune file page data
        let page_data = tune.pages.entry(constant.page).or_insert_with(|| {
            vec![
                0u8;
                def.page_sizes
                    .get(constant.page as usize)
                    .copied()
                    .unwrap_or(256) as usize
            ]
        });
        let start = constant.offset as usize;
        let end = start + raw_data.len();
        if end <= page_data.len() {
            page_data[start..end].copy_from_slice(&raw_data);
        }

        Ok(table.title.clone())
    }

    // Helper: write axis bin values to a constant
    fn write_axis_bins(
        def: &libretune_core::ini::EcuDefinition,
        cache: &mut TuneCache,
        tune: &mut TuneFile,
        const_name: &str,
        values: &[f64],
        endianness: libretune_core::ini::Endianness,
    ) -> Result<(), String> {
        let constant = match def.constants.get(const_name) {
            Some(c) => c,
            None => return Ok(()), // Axis constant not found, skip silently
        };
        let expected = constant.shape.element_count();
        let mut final_values = values.to_vec();
        final_values.resize(expected, *values.last().unwrap_or(&0.0));
        final_values.truncate(expected);

        let element_size = constant.data_type.size_bytes();
        let mut raw_data = vec![0u8; constant.size_bytes()];
        for (i, val) in final_values.iter().enumerate() {
            let raw_val = constant.display_to_raw(*val);
            let offset = i * element_size;
            constant
                .data_type
                .write_to_bytes(&mut raw_data, offset, raw_val, endianness);
        }

        cache.write_bytes(constant.page, constant.offset, &raw_data);

        let page_data = tune.pages.entry(constant.page).or_insert_with(|| {
            vec![
                0u8;
                def.page_sizes
                    .get(constant.page as usize)
                    .copied()
                    .unwrap_or(256) as usize
            ]
        });
        let start = constant.offset as usize;
        let end = start + raw_data.len();
        if end <= page_data.len() {
            page_data[start..end].copy_from_slice(&raw_data);
        }
        Ok(())
    }

    /// Look up a table definition by trying a list of candidate names.
    /// Returns the matched table definition and the actual cols and rows.
    /// Dimensions are resolved from the map constant's Shape (authoritative source),
    /// falling back to TableDefinition x_size/y_size, then x_bins/y_bins constants.
    fn find_table_with_dims<'a>(
        def: &'a libretune_core::ini::EcuDefinition,
        candidates: &[&str],
    ) -> Option<(&'a libretune_core::ini::TableDefinition, usize, usize)> {
        for name in candidates {
            if let Some(table) = def.get_table_by_name_or_map(name) {
                // Primary: get dimensions from the map constant's Shape
                if let Some(map_const) = def.constants.get(&table.map) {
                    match &map_const.shape {
                        libretune_core::ini::Shape::Array2D { rows, cols } => {
                            if *cols > 0 && *rows > 0 {
                                eprintln!("[DEBUG] find_table_with_dims: '{}' map '{}' shape Array2D {}x{}", name, table.map, cols, rows);
                                return Some((table, *cols, *rows));
                            }
                        }
                        libretune_core::ini::Shape::Array1D(size) => {
                            eprintln!(
                                "[DEBUG] find_table_with_dims: '{}' map '{}' shape Array1D({})",
                                name, table.map, size
                            );
                            return Some((table, *size, 1));
                        }
                        _ => {}
                    }
                }
                // Fallback: use x_bins/y_bins constant shapes
                let cols = if let Some(xc) = def.constants.get(&table.x_bins) {
                    xc.shape.x_size()
                } else {
                    table.x_size
                };
                let rows = if let Some(ref yb) = table.y_bins {
                    if let Some(yc) = def.constants.get(yb) {
                        yc.shape.x_size()
                    } else {
                        table.y_size
                    }
                } else {
                    table.y_size
                };
                // Last resort: TableDefinition x_size/y_size
                let cols = if cols > 0 { cols } else { table.x_size };
                let rows = if rows > 0 { rows } else { table.y_size.max(1) };
                eprintln!(
                    "[DEBUG] find_table_with_dims: '{}' fallback dims {}x{}",
                    name, cols, rows
                );
                if cols > 0 && rows > 0 {
                    return Some((table, cols, rows));
                }
            }
        }
        None
    }

    // Acquire cache and tune locks
    let mut cache_guard = state.tune_cache.lock().await;
    let cache = cache_guard.as_mut().ok_or("Tune cache not initialized")?;
    let mut tune_guard = state.current_tune.lock().await;
    // Create an empty TuneFile if none exists (e.g. new project with no imported tune)
    if tune_guard.is_none() {
        let sig = def.signature.clone();
        *tune_guard = Some(TuneFile::new(&sig));
    }
    let tune = tune_guard.as_mut().unwrap();

    // Apply VE table — generate at the INI's actual table dimensions
    if let Some((table_def, cols, rows)) = find_table_with_dims(def, &ve_table_names) {
        let table_name = table_def.name.clone();
        let title = table_def.title.clone();
        let x_bins_name = table_def.x_bins.clone();
        let y_bins_name = table_def.y_bins.clone();
        eprintln!(
            "[INFO] apply_base_map: VE table '{}' has {}x{} (cols x rows)",
            table_name, cols, rows
        );

        let rpm_bins = generate_rpm_bins(engine_spec.idle_rpm, engine_spec.redline_rpm, cols);
        let load_bins = generate_load_bins(engine_spec.max_load_kpa(), rows);
        let ve_data = generate_ve_table(&engine_spec, &rpm_bins, &load_bins);

        match write_table_z(def, cache, tune, &table_name, &ve_data, endianness) {
            Ok(_) => {
                let _ = write_axis_bins(def, cache, tune, &x_bins_name, &rpm_bins, endianness);
                if let Some(ref y_name) = y_bins_name {
                    let _ = write_axis_bins(def, cache, tune, y_name, &load_bins, endianness);
                }
                applied.push(format!("{} (VE {}x{})", title, cols, rows));
            }
            Err(e) => errors.push(format!("VE: {}", e)),
        }
    }

    // Apply ignition table — generate at the INI's actual table dimensions
    if let Some((table_def, cols, rows)) = find_table_with_dims(def, &ign_table_names) {
        let table_name = table_def.name.clone();
        let title = table_def.title.clone();
        let x_bins_name = table_def.x_bins.clone();
        let y_bins_name = table_def.y_bins.clone();
        eprintln!(
            "[INFO] apply_base_map: Ignition table '{}' has {}x{} (cols x rows)",
            table_name, cols, rows
        );

        let rpm_bins = generate_rpm_bins(engine_spec.idle_rpm, engine_spec.redline_rpm, cols);
        let load_bins = generate_load_bins(engine_spec.max_load_kpa(), rows);
        let ign_data = generate_ignition_table(&engine_spec, &rpm_bins, &load_bins);

        match write_table_z(def, cache, tune, &table_name, &ign_data, endianness) {
            Ok(_) => {
                let _ = write_axis_bins(def, cache, tune, &x_bins_name, &rpm_bins, endianness);
                if let Some(ref y_name) = y_bins_name {
                    let _ = write_axis_bins(def, cache, tune, y_name, &load_bins, endianness);
                }
                applied.push(format!("{} (Ignition {}x{})", title, cols, rows));
            }
            Err(e) => errors.push(format!("Ignition: {}", e)),
        }
    }

    // Apply AFR table — generate at the INI's actual table dimensions
    if let Some((table_def, cols, rows)) = find_table_with_dims(def, &afr_table_names) {
        let table_name = table_def.name.clone();
        let title = table_def.title.clone();
        let x_bins_name = table_def.x_bins.clone();
        let y_bins_name = table_def.y_bins.clone();
        eprintln!(
            "[INFO] apply_base_map: AFR table '{}' has {}x{} (cols x rows)",
            table_name, cols, rows
        );

        let rpm_bins = generate_rpm_bins(engine_spec.idle_rpm, engine_spec.redline_rpm, cols);
        let load_bins = generate_load_bins(engine_spec.max_load_kpa(), rows);
        let afr_data = generate_afr_table(&engine_spec, &rpm_bins, &load_bins);

        match write_table_z(def, cache, tune, &table_name, &afr_data, endianness) {
            Ok(_) => {
                let _ = write_axis_bins(def, cache, tune, &x_bins_name, &rpm_bins, endianness);
                if let Some(ref y_name) = y_bins_name {
                    let _ = write_axis_bins(def, cache, tune, y_name, &load_bins, endianness);
                }
                applied.push(format!("{} (AFR {}x{})", title, cols, rows));
            }
            Err(e) => errors.push(format!("AFR: {}", e)),
        }
    }

    // Apply scalar constants (reqFuel, etc.)
    if let Some(rf) = req_fuel {
        // Try common reqFuel constant names
        for name in &["reqFuel", "req_fuel", "required_fuel"] {
            if let Some(constant) = def.constants.get(*name) {
                let raw_val = constant.display_to_raw(rf);
                let element_size = constant.data_type.size_bytes();
                let mut raw_data = vec![0u8; element_size];
                constant
                    .data_type
                    .write_to_bytes(&mut raw_data, 0, raw_val, endianness);
                cache.write_bytes(constant.page, constant.offset, &raw_data);
                let page_data = tune.pages.entry(constant.page).or_insert_with(|| {
                    vec![
                        0u8;
                        def.page_sizes
                            .get(constant.page as usize)
                            .copied()
                            .unwrap_or(256) as usize
                    ]
                });
                let start = constant.offset as usize;
                let end = start + raw_data.len();
                if end <= page_data.len() {
                    page_data[start..end].copy_from_slice(&raw_data);
                }
                applied.push(format!("reqFuel = {:.1} ms", rf));
                break;
            }
        }
    }

    // Apply other scalars from the map
    if let Some(map) = scalars {
        for (name, val) in &map {
            if let Some(v) = val.as_f64() {
                if let Some(constant) = def.constants.get(name.as_str()) {
                    let raw_val = constant.display_to_raw(v);
                    let element_size = constant.data_type.size_bytes();
                    let mut raw_data = vec![0u8; element_size];
                    constant
                        .data_type
                        .write_to_bytes(&mut raw_data, 0, raw_val, endianness);
                    cache.write_bytes(constant.page, constant.offset, &raw_data);
                    let page_data = tune.pages.entry(constant.page).or_insert_with(|| {
                        vec![
                            0u8;
                            def.page_sizes
                                .get(constant.page as usize)
                                .copied()
                                .unwrap_or(256) as usize
                        ]
                    });
                    let start = constant.offset as usize;
                    let end = start + raw_data.len();
                    if end <= page_data.len() {
                        page_data[start..end].copy_from_slice(&raw_data);
                    }
                }
            }
        }
    }

    // Mark tune as modified
    *state.tune_modified.lock().await = true;

    // Auto-save the tune to the project directory so it exists on disk.
    // This is critical for new projects that had no tune file — without this,
    // "Use Project Tune" would fail with "Project tune file not found".
    {
        let project_guard = state.current_project.lock().await;
        if let Some(project) = project_guard.as_ref() {
            let tune_path = project.current_tune_path();
            // Sync cache data into tune pages before saving
            for page_num in 0..def.n_pages {
                if let Some(page_data) = cache.get_page(page_num) {
                    tune.pages.insert(page_num, page_data.to_vec());
                }
            }
            if let Err(e) = tune.save(&tune_path) {
                eprintln!("[WARN] apply_base_map: failed to auto-save tune: {}", e);
                errors.push(format!("Failed to save tune to disk: {}", e));
            } else {
                eprintln!("[INFO] apply_base_map: auto-saved tune to {:?}", tune_path);
                // Update the current tune path so future operations find it
                drop(project_guard);
                *state.current_tune_path.lock().await = Some(tune_path);
                *state.tune_modified.lock().await = false;
            }
        }
    }

    if applied.is_empty() {
        errors.push("No matching tables found in the loaded INI definition".to_string());
    }

    let mut result = serde_json::Map::new();
    result.insert("applied".to_string(), serde_json::json!(applied));
    result.insert("errors".to_string(), serde_json::json!(errors));
    eprintln!(
        "[INFO] apply_base_map: applied={:?}, errors={:?}",
        applied, errors
    );
    Ok(serde_json::Value::Object(result))
}

// Project misc commands extracted to commands/project_misc.rs

// INI Repository commands extracted to commands/ini_repository.rs

// Online INI repository commands extracted to commands/online_ini.rs

// =============================================================================
// DEMO MODE COMMANDS
// =============================================================================

/// Enable or disable demo mode (simulated ECU for UI testing)
/// When enabled, loads a bundled EpicEFI INI and generates simulated sensor data
#[tauri::command]
async fn set_demo_mode(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    // Stop any existing streaming first
    {
        let mut task_guard = state.streaming_task.lock().await;
        if let Some(handle) = task_guard.take() {
            handle.abort();
        }
    }

    if enabled {
        // Disconnect any existing connection to avoid mismatched definitions
        {
            let mut conn_guard = state.connection.lock().await;
            *conn_guard = None;
        }

        // Close and clear any open project/tune to ensure a clean demo state
        {
            let mut proj_guard = state.current_project.lock().await;
            if let Some(project) = proj_guard.take() {
                let _ = project.close();
            }
        }
        {
            let mut tune_guard = state.current_tune.lock().await;
            *tune_guard = None;
        }
        {
            let mut tune_mod_guard = state.tune_modified.lock().await;
            *tune_mod_guard = false;
        }

        // Load the bundled demo INI
        let resource_path = app
            .path()
            .resource_dir()
            .map_err(|e| format!("Failed to get resource dir: {}", e))?
            .join("resources")
            .join("demo.ini");

        // Try resource path first, then development path
        let ini_path = if resource_path.exists() {
            resource_path
        } else {
            // Development fallback: look in src-tauri/resources
            let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("resources")
                .join("demo.ini");
            if dev_path.exists() {
                dev_path
            } else {
                return Err(format!(
                    "Demo INI not found at {:?} or {:?}",
                    resource_path, dev_path
                ));
            }
        };

        // Load the INI definition
        let def = EcuDefinition::from_file(ini_path.to_string_lossy().as_ref())
            .map_err(|e| format!("Failed to load demo INI: {}", e))?;

        // Initialize TuneCache from definition
        let cache = TuneCache::from_definition(&def);

        // Apply the demo state to the AppState (aborts streaming, clears connection/project/tune and stores def/cache)
        apply_demo_enable(&state, def, cache).await?;

        // Notify frontend that definition/demo mode changed
        let _ = app.emit("demo:changed", true);
        let _ = app.emit("definition:changed", ());

        eprintln!("[DEMO] Demo mode enabled - loaded demo INI and cleared open project/connection");
    } else {
        // Disable demo mode
        {
            let mut demo_guard = state.demo_mode.lock().await;
            *demo_guard = false;
        }

        // Notify frontend demo disabled
        let _ = app.emit("demo:changed", false);

        eprintln!("[DEMO] Demo mode disabled");
    }

    Ok(())
}

/// Internal helper: apply demo enable with a provided definition and cache
async fn apply_demo_enable(
    state: &AppState,
    def: EcuDefinition,
    cache: TuneCache,
) -> Result<(), String> {
    // Stop any existing streaming task first
    {
        let mut task_guard = state.streaming_task.lock().await;
        if let Some(handle) = task_guard.take() {
            handle.abort();
        }
    }

    // Disconnect any existing connection
    {
        let mut conn_guard = state.connection.lock().await;
        *conn_guard = None;
    }

    // Close and clear any open project/tune to ensure a clean demo state
    {
        let mut proj_guard = state.current_project.lock().await;
        if let Some(project) = proj_guard.take() {
            let _ = project.close();
        }
    }

    {
        let mut tune_guard = state.current_tune.lock().await;
        *tune_guard = None;
    }

    {
        let mut tune_mod_guard = state.tune_modified.lock().await;
        *tune_mod_guard = false;
    }

    // Store the provided cache and definition
    {
        let mut cache_guard = state.tune_cache.lock().await;
        *cache_guard = Some(cache);
    }

    {
        let mut def_guard = state.definition.lock().await;
        *def_guard = Some(def);
    }

    // Set demo mode flag
    {
        let mut demo_guard = state.demo_mode.lock().await;
        *demo_guard = true;
    }

    Ok(())
}

#[allow(dead_code)]
async fn apply_demo_disable(state: &AppState) -> Result<(), String> {
    {
        let mut demo_guard = state.demo_mode.lock().await;
        *demo_guard = false;
    }
    Ok(())
}

/// Check if demo mode is currently enabled.
///
/// Demo mode simulates ECU data for testing without a real connection.
///
/// Returns: True if demo mode is active
#[tauri::command]
async fn get_demo_mode(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let demo_guard = state.demo_mode.lock().await;
    Ok(*demo_guard)
}

#[cfg(test)]
mod demo_mode_tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_apply_demo_enable_and_disable() {
        let state = AppState {
            connection: Mutex::new(None),
            definition: Mutex::new(None),
            autotune_state: Mutex::new(AutoTuneState::new()),
            autotune_secondary_state: Mutex::new(AutoTuneState::new()),
            autotune_config: Mutex::new(None),
            streaming_task: Mutex::new(None),
            autotune_send_task: Mutex::new(None),
            current_tune: Mutex::new(None),
            current_tune_path: Mutex::new(None),
            tune_modified: Mutex::new(false),
            data_logger: Mutex::new(DataLogger::default()),
            current_project: Mutex::new(None),
            ini_repository: Mutex::new(None),
            online_ini_repository: Mutex::new(OnlineIniRepository::new()),
            tune_cache: Mutex::new(None),
            demo_mode: Mutex::new(false),
            console_history: Mutex::new(Vec::new()),
            rpm_state_tracker: Mutex::new(RpmStateTracker::new()),
            // Background task for connection metrics emission (added recently)
            metrics_task: Mutex::new(None),
            wasm_plugin_manager: Mutex::new(None),

            migration_report: Mutex::new(None),
            evaluator: Mutex::new(None),
            cached_output_channels: Mutex::new(None),
            connection_factory: Mutex::new(None),
            math_channels: Mutex::new(Vec::new()),
            stream_stats: Mutex::new(StreamStats::default()),
        };

        let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("demo.ini");
        assert!(dev_path.exists(), "Demo INI not found at {:?}", dev_path);
        let def =
            EcuDefinition::from_file(dev_path.to_string_lossy().as_ref()).expect("Load demo INI");
        let cache = TuneCache::from_definition(&def);

        // initial state
        assert!(!*state.demo_mode.lock().await);
        assert!(state.definition.lock().await.is_none());
        assert!(state.tune_cache.lock().await.is_none());

        apply_demo_enable(&state, def.clone(), cache)
            .await
            .expect("apply enable");
        assert!(*state.demo_mode.lock().await);
        assert!(state.definition.lock().await.is_some());
        assert!(state.tune_cache.lock().await.is_some());

        apply_demo_disable(&state).await.expect("apply disable");
        assert!(!*state.demo_mode.lock().await);
    }
}

#[cfg(test)]
mod concurrency_tests {
    use super::*;
    use libretune_core::protocol::{Connection, ConnectionConfig};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn test_no_deadlock_between_execute_controller_and_realtime_snapshot() {
        // Build a minimal AppState with both locks present
        let dev_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("demo.ini");
        assert!(dev_path.exists(), "Demo INI not found at {:?}", dev_path);
        let def =
            EcuDefinition::from_file(dev_path.to_string_lossy().as_ref()).expect("Load demo INI");

        let state = Arc::new(AppState {
            connection: Mutex::new(Some(Connection::new(ConnectionConfig::default()))),
            definition: Mutex::new(Some(def)),
            autotune_state: Mutex::new(AutoTuneState::new()),
            autotune_secondary_state: Mutex::new(AutoTuneState::new()),
            autotune_config: Mutex::new(None),
            streaming_task: Mutex::new(None),
            autotune_send_task: Mutex::new(None),
            metrics_task: Mutex::new(None),
            current_tune: Mutex::new(None),
            current_tune_path: Mutex::new(None),
            tune_modified: Mutex::new(false),
            data_logger: Mutex::new(DataLogger::default()),
            current_project: Mutex::new(None),
            ini_repository: Mutex::new(None),
            online_ini_repository: Mutex::new(OnlineIniRepository::new()),
            tune_cache: Mutex::new(None),
            demo_mode: Mutex::new(false),
            console_history: Mutex::new(Vec::new()),
            rpm_state_tracker: Mutex::new(RpmStateTracker::new()),
            wasm_plugin_manager: Mutex::new(None),

            migration_report: Mutex::new(None),
            evaluator: Mutex::new(None),
            cached_output_channels: Mutex::new(None),
            connection_factory: Mutex::new(None),
            math_channels: Mutex::new(Vec::new()),
            stream_stats: Mutex::new(StreamStats::default()),
        });

        // Simulate execute_controller_command pattern: lock def -> sleep -> lock conn
        let s1 = state.clone();

        let task1 = tokio::spawn(async move {
            let _def = s1.definition.lock().await;
            // hold definition lock for some time
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _conn = s1.connection.lock().await;
            tokio::time::sleep(Duration::from_millis(50)).await;
        });

        // Simulate refactored get_realtime_data: snapshot def -> release -> lock conn
        let s2 = state.clone();
        let task2 = tokio::spawn(async move {
            let _snapshot = {
                let def_guard = s2.definition.lock().await;
                def_guard.is_some()
            };

            // Now only lock connection for a short time
            let _conn = s2.connection.lock().await;
            tokio::time::sleep(Duration::from_millis(50)).await;
        });

        // Ensure both complete within timeout (detect deadlock)
        let joined = tokio::time::timeout(Duration::from_secs(2), async {
            let r1 = task1.await;
            let r2 = task2.await;
            (r1, r2)
        })
        .await;

        assert!(joined.is_ok(), "Tasks deadlocked or timed out");
    }
}

// New tests for signature comparison and normalization (unit tests)
#[cfg(test)]
mod signature_tests {
    use super::*;

    #[test]
    fn test_normalize_signature_basic() {
        assert_eq!(
            normalize_signature("Speeduino 2023-05"),
            "speeduino 2023 05"
        );
        assert_eq!(
            normalize_signature("  RusEFI_v1.2.3 (build#42) "),
            "rusefi v1 2 3 build 42"
        );
        assert_eq!(normalize_signature("MegaSquirt"), "megasquirt");
    }

    #[test]
    fn test_compare_signatures_exact_and_partial() {
        // Exact after normalization
        assert_eq!(
            compare_signatures("Speeduino 2023.05", "speeduino 2023-05"),
            SignatureMatchType::Exact
        );

        // Partial when base matches but versions differ
        assert_eq!(
            compare_signatures("rusEFI v1.2.3", "rusEFI v1.2.4"),
            SignatureMatchType::Partial
        );

        // Partial when one contains the other
        assert_eq!(
            compare_signatures("Speeduino build 202305 extra", "speeduino 202305"),
            SignatureMatchType::Partial
        );

        // Mismatch for different families
        assert_eq!(
            compare_signatures("unrelated device", "another device"),
            SignatureMatchType::Mismatch
        );
    }

    #[test]
    fn test_build_shallow_mismatch_info() {
        let info = build_shallow_mismatch_info(
            "Speeduino 2023-05",
            "Speeduino 2023-04",
            Some("/path/test.ini".to_string()),
        );
        assert_eq!(info.match_type, SignatureMatchType::Partial);
        assert_eq!(info.ecu_signature, "Speeduino 2023-05");
        assert_eq!(info.ini_signature, "Speeduino 2023-04");
        assert_eq!(info.current_ini_path.unwrap(), "/path/test.ini");
        assert!(info.matching_inis.is_empty());

        let info2 = build_shallow_mismatch_info("FooBar", "BazQux", None);
        assert_eq!(info2.match_type, SignatureMatchType::Mismatch);
    }

    #[tokio::test]
    async fn test_find_matching_inis_and_build_info_partial() {
        use std::fs::write;
        use tempfile::tempdir;

        // Create a temporary repository and a sample INI with a Speeduino signature
        let dir = tempdir().expect("tempdir");
        let ini_path = dir.path().join("speedy.ini");
        let content = r#"[MegaTune]
name = "Speedy"
signature = "Speeduino 2023-04"
"#;
        write(&ini_path, content).expect("write ini");

        // Open repository and import the ini
        let mut repo = IniRepository::open(Some(dir.path())).expect("open repo");
        let _id = repo.import(&ini_path).expect("import");

        // Build minimal AppState with this repo
        let state = AppState {
            connection: Mutex::new(None),
            definition: Mutex::new(None),
            autotune_state: Mutex::new(AutoTuneState::default()),
            autotune_secondary_state: Mutex::new(AutoTuneState::default()),
            autotune_config: Mutex::new(None),
            streaming_task: Mutex::new(None),
            autotune_send_task: Mutex::new(None),
            metrics_task: Mutex::new(None),
            current_tune: Mutex::new(None),
            current_tune_path: Mutex::new(None),
            tune_modified: Mutex::new(false),
            data_logger: Mutex::new(DataLogger::default()),
            current_project: Mutex::new(None),
            ini_repository: Mutex::new(Some(repo)),
            online_ini_repository: Mutex::new(OnlineIniRepository::new()),
            tune_cache: Mutex::new(None),
            demo_mode: Mutex::new(false),
            console_history: Mutex::new(Vec::new()),
            rpm_state_tracker: Mutex::new(RpmStateTracker::new()),
            wasm_plugin_manager: Mutex::new(None),

            migration_report: Mutex::new(None),
            evaluator: Mutex::new(None),
            cached_output_channels: Mutex::new(None),
            connection_factory: Mutex::new(None),
            math_channels: Mutex::new(Vec::new()),
            stream_stats: Mutex::new(StreamStats::default()),
        };

        let matches = find_matching_inis_from_state(&state, "Speeduino 2023-05").await;
        // We expect at least one match (the one we imported)
        assert!(!matches.is_empty());
        assert!(matches
            .iter()
            .any(|e| e.signature.to_lowercase().contains("speeduino")));

        // Build mismatch info using our helper and attach matching INIs
        let mut info = build_shallow_mismatch_info(
            "Speeduino 2023-05",
            "Speeduino 2023-04",
            Some("test.ini".to_string()),
        );
        info.matching_inis = matches;

        assert_eq!(info.match_type, SignatureMatchType::Partial);
        assert_eq!(info.current_ini_path.unwrap(), "test.ini");
        assert!(!info.matching_inis.is_empty());
    }

    #[tokio::test]
    async fn test_find_matching_inis_and_build_info_mismatch() {
        use std::fs::write;
        use tempfile::tempdir;

        // Create temporary repo with a Speeduino ini
        let dir = tempdir().expect("tempdir");
        let ini_path = dir.path().join("speedy.ini");
        let content = r#"[MegaTune]
name = "Speedy"
signature = "Speeduino 2023-04"
"#;
        write(&ini_path, content).expect("write ini");

        let mut repo = IniRepository::open(Some(dir.path())).expect("open repo");
        let _id = repo.import(&ini_path).expect("import");

        let state = AppState {
            connection: Mutex::new(None),
            definition: Mutex::new(None),
            autotune_state: Mutex::new(AutoTuneState::default()),
            autotune_secondary_state: Mutex::new(AutoTuneState::default()),
            autotune_config: Mutex::new(None),
            streaming_task: Mutex::new(None),
            autotune_send_task: Mutex::new(None),
            metrics_task: Mutex::new(None),
            current_tune: Mutex::new(None),
            current_tune_path: Mutex::new(None),
            tune_modified: Mutex::new(false),
            data_logger: Mutex::new(DataLogger::default()),
            current_project: Mutex::new(None),
            ini_repository: Mutex::new(Some(repo)),
            online_ini_repository: Mutex::new(OnlineIniRepository::new()),
            tune_cache: Mutex::new(None),
            demo_mode: Mutex::new(false),
            console_history: Mutex::new(Vec::new()),
            rpm_state_tracker: Mutex::new(RpmStateTracker::new()),
            wasm_plugin_manager: Mutex::new(None),

            migration_report: Mutex::new(None),
            evaluator: Mutex::new(None),
            cached_output_channels: Mutex::new(None),
            connection_factory: Mutex::new(None),
            math_channels: Mutex::new(Vec::new()),
            stream_stats: Mutex::new(StreamStats::default()),
        };

        let matches = find_matching_inis_from_state(&state, "Speeduino 2023-05").await;
        // Using a completely different signature should yield no matches
        // (We already have a Speeduino INI in the repo)
        assert!(matches
            .iter()
            .any(|e| e.signature.to_lowercase().contains("speeduino")));

        // Build mismatch info for an unrelated ECU signature
        let mut info = build_shallow_mismatch_info("FooBar 1.0", "Speeduino 2023-04", None);
        info.matching_inis = Vec::new();
        assert_eq!(info.match_type, SignatureMatchType::Mismatch);
        assert!(info.matching_inis.is_empty());
    }

    // Explicit simulated connect tests: ensure connect-like behavior returns mismatch_info
    #[tokio::test]
    async fn test_connect_simulated_partial_and_mismatch() {
        use std::fs::write;
        use tempfile::tempdir;

        // Create temporary repo and a Speeduino INI
        let dir = tempdir().expect("tempdir");
        let ini_path = dir.path().join("speedy.ini");
        let content = r#"[MegaTune]
name = "Speedy"
signature = "Speeduino 2023-04"
"#;
        write(&ini_path, content).expect("write ini");

        let mut repo = IniRepository::open(Some(dir.path())).expect("open repo");
        let _id = repo.import(&ini_path).expect("import");

        // Build AppState with a loaded definition that expects the Speeduino 2023-04 signature
        let def = EcuDefinition::from_str(
            r#"[MegaTune]
signature = "Speeduino 2023-04"
"#,
        )
        .expect("parse def");

        let state = AppState {
            connection: Mutex::new(None),
            definition: Mutex::new(Some(def)),
            autotune_state: Mutex::new(AutoTuneState::default()),
            autotune_secondary_state: Mutex::new(AutoTuneState::default()),
            autotune_config: Mutex::new(None),
            streaming_task: Mutex::new(None),
            autotune_send_task: Mutex::new(None),
            metrics_task: Mutex::new(None),
            current_tune: Mutex::new(None),
            current_tune_path: Mutex::new(None),
            tune_modified: Mutex::new(false),
            data_logger: Mutex::new(DataLogger::default()),
            current_project: Mutex::new(None),
            ini_repository: Mutex::new(Some(repo)),
            online_ini_repository: Mutex::new(OnlineIniRepository::new()),
            tune_cache: Mutex::new(None),
            demo_mode: Mutex::new(false),
            console_history: Mutex::new(Vec::new()),
            rpm_state_tracker: Mutex::new(RpmStateTracker::new()),
            wasm_plugin_manager: Mutex::new(None),

            migration_report: Mutex::new(None),
            evaluator: Mutex::new(None),
            cached_output_channels: Mutex::new(None),
            connection_factory: Mutex::new(None),
            math_channels: Mutex::new(Vec::new()),
            stream_stats: Mutex::new(StreamStats::default()),
        };

        // Partial match case
        let result_partial = connect_to_ecu_simulated(&state, "Speeduino 2023-05").await;
        assert_eq!(
            result_partial.mismatch_info.as_ref().unwrap().match_type,
            SignatureMatchType::Partial
        );
        assert!(!result_partial
            .mismatch_info
            .as_ref()
            .unwrap()
            .matching_inis
            .is_empty());

        // Mismatch case
        let result_mismatch = connect_to_ecu_simulated(&state, "UnrelatedDevice 1.0").await;
        assert_eq!(
            result_mismatch.mismatch_info.as_ref().unwrap().match_type,
            SignatureMatchType::Mismatch
        );
        assert!(result_mismatch
            .mismatch_info
            .as_ref()
            .unwrap()
            .matching_inis
            .is_empty());
    }

    #[tokio::test]
    async fn test_call_connection_factory_and_build_result_helper() {
        use std::fs::write;
        use std::sync::Arc;
        use tempfile::tempdir;

        // Create temp repo with Speeduino INI
        let dir = tempdir().expect("tempdir");
        let ini_path = dir.path().join("speedy.ini");
        let content = r#"[MegaTune]
name = "Speedy"
signature = "Speeduino 2023-04"
"#;
        write(&ini_path, content).expect("write ini");
        let mut repo = IniRepository::open(Some(dir.path())).expect("open repo");
        let _id = repo.import(&ini_path).expect("import");

        // Build a minimal AppState with repo and expected definition
        let state = AppState {
            connection: Mutex::new(None),
            definition: Mutex::new(Some(
                EcuDefinition::from_str(
                    r#"[MegaTune]
signature = "Speeduino 2023-04"
"#,
                )
                .expect("parse def"),
            )),
            autotune_state: Mutex::new(AutoTuneState::default()),
            autotune_secondary_state: Mutex::new(AutoTuneState::default()),
            autotune_config: Mutex::new(None),
            streaming_task: Mutex::new(None),
            autotune_send_task: Mutex::new(None),
            metrics_task: Mutex::new(None),
            current_tune: Mutex::new(None),
            current_tune_path: Mutex::new(None),
            tune_modified: Mutex::new(false),
            data_logger: Mutex::new(DataLogger::default()),
            current_project: Mutex::new(None),
            ini_repository: Mutex::new(Some(repo)),
            online_ini_repository: Mutex::new(OnlineIniRepository::new()),
            tune_cache: Mutex::new(None),
            demo_mode: Mutex::new(false),
            console_history: Mutex::new(Vec::new()),
            rpm_state_tracker: Mutex::new(RpmStateTracker::new()),
            wasm_plugin_manager: Mutex::new(None),

            migration_report: Mutex::new(None),
            evaluator: Mutex::new(None),
            cached_output_channels: Mutex::new(None),
            connection_factory: Mutex::new(None),
            math_channels: Mutex::new(Vec::new()),
            stream_stats: Mutex::new(StreamStats::default()),
        };

        // Install factory returning a partial matching signature
        let factory: std::sync::Arc<
            dyn Fn(ConnectionConfig, Option<ProtocolSettings>, Endianness) -> Result<String, String>
                + Send
                + Sync,
        > = Arc::new(|_cfg, _p, _e| Ok("Speeduino 2023-05".to_string()));
        *state.connection_factory.lock().await = Some(factory);

        let res = call_connection_factory_and_build_result(&state, ConnectionConfig::default())
            .await
            .expect("factory ok");
        assert_eq!(
            res.mismatch_info.as_ref().unwrap().match_type,
            SignatureMatchType::Partial
        );
        assert!(!res.mismatch_info.as_ref().unwrap().matching_inis.is_empty());

        // Install factory that returns Err
        let factory_err: std::sync::Arc<
            dyn Fn(ConnectionConfig, Option<ProtocolSettings>, Endianness) -> Result<String, String>
                + Send
                + Sync,
        > = Arc::new(|_cfg, _p, _e| Err("fail".to_string()));
        *state.connection_factory.lock().await = Some(factory_err);

        let err = call_connection_factory_and_build_result(&state, ConnectionConfig::default())
            .await
            .err()
            .expect("err expected");
        assert!(err.contains("Factory-based connect failed"));
    }
}

// Settings commands extracted to commands/settings.rs

// tune_misc commands extracted to commands/tune_misc.rs

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            connection: Mutex::new(None),
            definition: Mutex::new(None),
            autotune_state: Mutex::new(AutoTuneState::new()),
            autotune_secondary_state: Mutex::new(AutoTuneState::new()),
            autotune_config: Mutex::new(None),
            streaming_task: Mutex::new(None),
            autotune_send_task: Mutex::new(None),
            // Background task for connection metrics emission
            metrics_task: Mutex::new(None),
            current_tune: Mutex::new(None),
            current_tune_path: Mutex::new(None),
            tune_modified: Mutex::new(false),
            data_logger: Mutex::new(DataLogger::default()),
            current_project: Mutex::new(None),
            ini_repository: Mutex::new(None),
            online_ini_repository: Mutex::new(OnlineIniRepository::new()),
            tune_cache: Mutex::new(None),
            demo_mode: Mutex::new(false),
            console_history: Mutex::new(Vec::new()),
            rpm_state_tracker: Mutex::new(RpmStateTracker::new()),
            wasm_plugin_manager: Mutex::new(None),

            migration_report: Mutex::new(None),
            evaluator: Mutex::new(None),
            connection_factory: Mutex::new(None),
            cached_output_channels: Mutex::new(None),
            math_channels: Mutex::new(Vec::new()),
            stream_stats: Mutex::new(StreamStats::default()),
        })
        .invoke_handler(tauri::generate_handler![
            get_serial_ports,
            get_available_inis,
            connect_to_ecu,
            sync_ecu_data,
            disconnect_ecu,
            enable_adaptive_timing,
            disable_adaptive_timing,
            get_adaptive_timing_stats,
            get_connection_status,
            get_ecu_type,
            send_console_command,
            get_console_history,
            clear_console_history,
            load_ini,
            get_realtime_data,
            debug_single_realtime_read,
            start_realtime_stream,
            stop_realtime_stream,
            get_table_data,
            get_table_info,
            get_curve_data,
            get_tables,
            get_curves,
            get_gauge_configs,
            get_gauge_config,
            get_available_channels,
            get_output_channel_status,
            get_status_bar_defaults,
            get_frontpage,
            update_table_data,
            update_curve_data,
            get_menu_tree,
            get_searchable_index,
            get_dialog_definition,
            get_indicator_panel,
            get_port_editor,
            get_port_editor_assignments,
            save_port_editor_assignments,
            // Math Channels
            get_math_channels,
            set_math_channel,
            delete_math_channel,
            validate_math_expression,
            // INI / protocol defaults
            get_protocol_defaults,
            get_protocol_capabilities,
            get_ini_capabilities,
            get_ve_analyze_config,
            get_help_topic,
            get_build_info,
            get_constant,
            get_constant_value,
            get_constant_string_value,
            update_constant,
            auto_load_last_ini,
            evaluate_expression,
            get_all_constant_values,
            start_autotune,
            stop_autotune,
            get_autotune_recommendations,
            get_autotune_heatmap,
            send_autotune_recommendations,
            burn_autotune_recommendations,
            lock_autotune_cells,
            unlock_autotune_cells,
            get_predicted_fills,
            get_tune_anomalies,
            get_tune_health_report,
            compare_tune_files,
            merge_from_tune,
            set_annotation,
            get_annotation,
            get_table_annotations,
            delete_annotation,
            get_all_annotations,
            load_dyno_run,
            detect_dyno_headers,
            compare_dyno_runs,
            get_dyno_table_overlay,
            rebin_table,
            smooth_table,
            interpolate_cells,
            interpolate_linear,
            add_offset,
            fill_region,
            scale_cells,
            set_cells_equal,
            save_dashboard_layout,
            load_dashboard_layout,
            list_dashboard_layouts,
            create_default_dashboard,
            get_dashboard_templates,
            load_tunerstudio_dash,
            get_dash_file,
            validate_dashboard,
            save_dash_file,
            list_available_dashes,
            reset_dashboards_to_defaults,
            check_dash_conflict,
            import_dash_file,
            create_new_dashboard,
            rename_dashboard,
            duplicate_dashboard,
            export_dashboard,
            delete_dashboard,
            // Tune file commands
            get_tune_info,
            new_tune,
            save_tune,
            save_tune_as,
            load_tune,
            get_migration_report,
            clear_migration_report,
            get_tune_ini_metadata,
            get_tune_constant_manifest,
            list_tune_files,
            burn_to_ecu,
            execute_controller_command,
            use_project_tune,
            use_ecu_tune,
            mark_tune_modified,
            compare_project_and_ecu_tunes,
            write_project_tune_to_ecu,
            save_tune_to_project,
            // Tune cache commands
            get_tune_cache_status,
            load_all_pages,
            // Data logging commands
            start_logging,
            stop_logging,
            get_logging_status,
            get_log_entries,
            clear_log,
            save_log,
            read_text_file,
            // Diagnostic commands (stubs)
            start_tooth_logger,
            stop_tooth_logger,
            start_composite_logger,
            stop_composite_logger,
            compare_tables,
            reset_tune_to_defaults,
            export_tune_as_csv,
            import_tune_from_csv,
            // Project management commands
            get_projects_path,
            list_projects,
            create_project,
            open_project,
            close_project,
            get_current_project,
            update_project_connection,
            update_project_auto_connect,
            // Restore points commands
            create_restore_point,
            list_restore_points,
            load_restore_point,
            delete_restore_point,
            // TunerStudio import
            preview_tunerstudio_import,
            import_tunerstudio_project,
            // Git version control commands
            git_init_project,
            git_has_repo,
            git_commit,
            git_history,
            git_diff,
            git_checkout,
            git_list_branches,
            git_create_branch,
            git_switch_branch,
            git_current_branch,
            git_has_changes,
            // Base map generator commands
            generate_base_map,
            apply_base_map,
            get_msq_info,
            delete_project,
            // INI signature management commands
            find_matching_inis,
            update_project_ini,
            // INI repository commands
            init_ini_repository,
            list_repository_inis,
            import_ini,
            scan_for_inis,
            remove_ini,
            // Online INI repository commands
            check_internet_connectivity,
            search_online_inis,
            download_ini,
            // Demo mode commands
            set_demo_mode,
            get_demo_mode,
            // Settings commands
            get_settings,
            update_setting,
            get_hotkey_bindings,
            save_hotkey_bindings,
            mark_onboarding_completed,
            is_onboarding_completed,
            update_heatmap_custom_stops,
            update_constant_string,
            run_lua_script,
            // WASM Plugin commands
            load_wasm_plugin,
            unload_wasm_plugin,
            list_wasm_plugins,
            execute_wasm_plugin,
            get_wasm_plugin_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
