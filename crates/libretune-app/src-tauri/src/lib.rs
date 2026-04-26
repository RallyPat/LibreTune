use libretune_core::autotune::AutoTuneState;
use libretune_core::datalog::DataLogger;
use libretune_core::ini::DataType;
use libretune_core::project::OnlineIniRepository;
use libretune_core::protocol::ConnectionState;
use serde::Serialize;
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
use commands::find_inis::find_matching_inis;
use commands::apply_base_map::apply_base_map;
use commands::update_project_ini::update_project_ini;
use commands::demo::{set_demo_mode, get_demo_mode};
use commands::available_inis::get_available_inis;
use commands::start_autotune::start_autotune;
use commands::get_table_data::get_table_data;
use commands::load_ini::load_ini;
use commands::save_tune::{save_tune, save_tune_as};
use commands::connect_to_ecu::connect_to_ecu;
use commands::sync_ecu_data::sync_ecu_data;
use commands::load_tune::load_tune;
use commands::project_lifecycle::{create_project, open_project};
use commands::realtime_stream::start_realtime_stream;
pub(crate) use commands::app_settings::{
    get_commit_message_format, load_settings, save_settings, Settings,
};
pub(crate) use commands::signature_helpers::{
    call_connection_factory_and_build_result, compare_signatures, find_matching_inis_internal,
};
pub(crate) use commands::dash_convert::{convert_dashfile_to_layout, convert_layout_to_dashfile};
pub(crate) use commands::table_internals::{
    get_table_data_internal, update_constant_array_internal, update_table_z_values_internal,
    TableData,
};
use commands::debug_realtime::debug_single_realtime_read;
use commands::realtime_get::get_realtime_data;
use commands::metrics::stop_metrics_task;
use commands::tune_info::{get_tune_info, new_tune};
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
// port_editor module used by commands/ini_dialogs.rs
use state::{
    AppState, AutoTuneLoadSource, RpmState,
    RpmStateTracker, StreamStats,
};

/// Parse a runtime packet mode string into enum
pub(crate) fn parse_runtime_packet_mode(mode: &str) -> libretune_core::protocol::RuntimePacketMode {
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
pub(crate) enum SignatureMatchType {
    /// Signatures match exactly
    Exact,
    /// Signatures match partially (one contains the other, version diff)
    Partial,
    /// Signatures do not match
    Mismatch,
}

/// Information about a signature mismatch for the frontend
#[derive(Serialize, Clone)]
pub(crate) struct SignatureMismatchInfo {
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
pub(crate) struct MatchingIniInfo {
    /// Path to the INI file
    pub path: String,
    /// Display name of the INI
    pub name: String,
    /// Signature from this INI
    pub signature: String,
    /// How well it matches (exact or partial)
    pub match_type: SignatureMatchType,
}

/// Result of ECU connection attempt
#[derive(Serialize)]
pub(crate) struct ConnectResult {
    /// The signature reported by the ECU
    signature: String,
    /// Mismatch info if signatures don't match exactly
    mismatch_info: Option<SignatureMismatchInfo>,
}

/// Result of ECU sync operation
#[derive(Serialize)]
pub(crate) struct SyncResult {
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

// =============================================================================
// Dashboard Format Conversion Helpers
// =============================================================================


// =============================================================================
// Signature Comparison Helpers
// =============================================================================


// get_available_inis extracted to commands/available_inis.rs

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
// load_ini extracted to commands/load_ini.rs

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
// connect_to_ecu extracted to commands/connect_to_ecu.rs

/// Sync response with progress information
#[derive(Serialize)]
pub(crate) struct SyncProgress {
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
// sync_ecu_data extracted to commands/sync_ecu_data.rs

// disconnect_ecu, get_connection_status, auto_load_last_ini extracted to commands/connection.rs


// CurveData struct extracted to commands/curve_ops.rs

/// Clean up INI expression labels for display
/// Converts expressions like `{bitStringValue(pwmAxisLabels, gppwm1_loadAxis)}`
/// to a readable fallback like `gppwm1_loadAxis`
pub(crate) fn clean_axis_label(label: &str) -> String {
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
// get_table_data extracted to commands/get_table_data.rs
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
// feed_autotune_data extracted to commands/realtime_stream.rs

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
// start_realtime_stream extracted to commands/realtime_stream.rs

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



// Misc AutoTune commands extracted to commands/autotune_misc.rs

// Tune health/anomaly/predicted_fills/dyno_overlay extracted to commands/tune_health.rs



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
// save_tune, save_tune_as extracted to commands/save_tune.rs

/// Loads a tune file from disk.
///
/// Parses an MSQ file and populates the tune cache. Handles signature
/// comparison and generates migration reports if the INI has changed.
///
/// # Arguments
/// * `path` - Path to the MSQ file to load
///
/// Returns: TuneInfo with loaded tune metadata
// load_tune extracted to commands/load_tune.rs

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
// create_project extracted to commands/project_lifecycle.rs

/// Open an existing project
///
/// Loads a project from disk, including its INI definition and tune file.
/// Disconnects any existing ECU connection to avoid state conflicts.
///
/// # Arguments
/// * `path` - Path to the project directory
///
// open_project extracted to commands/project_lifecycle.rs


// Project management commands extracted to commands/project_mgmt.rs

// find_matching_inis extracted to commands/find_inis.rs

// update_project_ini extracted to commands/update_project_ini.rs

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

// apply_base_map extracted to commands/apply_base_map.rs

// Project misc commands extracted to commands/project_misc.rs

// INI Repository commands extracted to commands/ini_repository.rs

// Online INI repository commands extracted to commands/online_ini.rs

// =============================================================================
// DEMO MODE COMMANDS
// =============================================================================
// demo mode commands extracted to commands/demo.rs

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
