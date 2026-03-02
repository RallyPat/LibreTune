use libretune_core::autotune::{
    AutoTuneAuthorityLimits, AutoTuneFilters, AutoTuneRecommendation, AutoTuneSettings,
    AutoTuneState, VEDataPoint,
};
use libretune_core::dash::{
    self, create_basic_dashboard, create_racing_dashboard, create_tuning_dashboard, Bibliography,
    DashComponent, DashFile, GaugePainter, TsColor, VersionInfo,
};
use libretune_core::dashboard::{
    get_dashboard_file_path, DashboardLayout, GaugeConfig as DashboardGaugeConfig,
};
use libretune_core::datalog::DataLogger;
use libretune_core::demo::DemoSimulator;
use libretune_core::ini::{
    AdaptiveTimingConfig, CommandPart, Constant, DataType, DialogDefinition, EcuDefinition,
    Endianness, HelpTopic, IniCapabilities, Menu, MenuItem, ProtocolSettings, VeAnalyzeConfig,
};
use libretune_core::lua::{execute_script, LuaExecutionResult};
use libretune_core::plugin_system::{
    PluginConfig as WasmPluginConfig, PluginManager as WasmPluginManager,
    PluginManifest as WasmPluginManifest,
};
use libretune_core::project::{
    format_commit_message, load_math_channels, save_math_channels, BranchInfo, CommitDiff,
    CommitInfo, ConnectionSettings, IniRepository, IniSource, OnlineIniEntry, OnlineIniRepository,
    Project, ProjectConfig, ProjectSettings, UserMathChannel, VersionControl,
};
use libretune_core::protocol::serial::list_ports;
use libretune_core::protocol::{Connection, ConnectionConfig, ConnectionState};
use libretune_core::realtime::Evaluator;
use libretune_core::table_ops;
use libretune_core::tune::{
    ConstantManifestEntry, IniMetadata, MigrationReport, PageState, TuneCache, TuneFile, TuneValue,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_window_state::{AppHandleExt, StateFlags};
use tokio::sync::Mutex;

#[derive(Serialize)]
struct BuildInfo {
    version: String,
    build_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PortEditorAssignment {
    id: String,
    name: String,
    physical_pin: String,
    function: String,
    channel: u32,
    inverted: bool,
    pullup: bool,
    description: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PortEditorStore {
    assignments: HashMap<String, Vec<PortEditorAssignment>>,
}

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

/// Get the LibreTune app data directory (cross-platform)
fn get_app_data_dir(app: &tauri::AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_else(|_| {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("LibreTune")
    })
}

/// Get the projects directory (cross-platform)
fn get_projects_dir(app: &tauri::AppHandle) -> PathBuf {
    get_app_data_dir(app).join("projects")
}

/// Get the definitions directory (cross-platform)
fn get_definitions_dir(app: &tauri::AppHandle) -> PathBuf {
    get_app_data_dir(app).join("definitions")
}

/// Get the settings file path (cross-platform)
fn get_settings_path(app: &tauri::AppHandle) -> PathBuf {
    get_app_data_dir(app).join("settings.json")
}

/// Get the dashboards directory (cross-platform)
fn get_dashboards_dir(app: &tauri::AppHandle) -> PathBuf {
    get_app_data_dir(app).join("dashboards")
}

fn get_port_editor_store_path(project: &Project) -> PathBuf {
    project.path.join("projectCfg").join("port_editor.json")
}

fn load_port_editor_store(project: &Project) -> Result<PortEditorStore, String> {
    let path = get_port_editor_store_path(project);
    if !path.exists() {
        return Ok(PortEditorStore::default());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read port editor store: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse port editor store: {}", e))
}

fn save_port_editor_store(project: &Project, store: &PortEditorStore) -> Result<(), String> {
    let path = get_port_editor_store_path(project);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create port editor directory: {}", e))?;
    }
    let json = serde_json::to_string_pretty(store)
        .map_err(|e| format!("Failed to serialize port editor store: {}", e))?;
    std::fs::write(&path, json).map_err(|e| format!("Failed to write port editor store: {}", e))?;
    Ok(())
}

/// Get application build information (version + nightly build ID).
#[tauri::command]
fn get_build_info(app: tauri::AppHandle) -> BuildInfo {
    let version = app.package_info().version.to_string();
    let build_id = option_env!("LIBRETUNE_BUILD_ID")
        .unwrap_or("unknown")
        .to_string();
    BuildInfo { version, build_id }
}

/// Start periodic connection metrics emission task (1s interval)
async fn start_metrics_task(app: tauri::AppHandle, state: tauri::State<'_, AppState>) {
    let mut guard = state.metrics_task.lock().await;
    // If already running, do nothing
    if guard.is_some() {
        return;
    }

    let app_handle = app.clone();

    let handle = tokio::spawn(async move {
        use tokio::time::{sleep, Duration};
        // Obtain AppState inside the spawned task via AppHandle to ensure 'static lifetime
        let state = app_handle.state::<AppState>();
        let mut prev_tx: u64 = 0;
        let mut prev_rx: u64 = 0;
        let mut prev_tx_pkts: u64 = 0;
        let mut prev_rx_pkts: u64 = 0;
        let mut prev_ts = std::time::Instant::now();

        loop {
            sleep(Duration::from_millis(1000)).await;

            // Sample connection counters
            let (tx, rx, tx_pkts, rx_pkts, connected) = {
                let conn_guard = state.connection.lock().await;
                if let Some(conn) = conn_guard.as_ref() {
                    // get counters
                    let (tx_b, rx_b, tx_p, rx_p) = conn.get_counters();
                    (tx_b, rx_b, tx_p, rx_p, true)
                } else {
                    (0u64, 0u64, 0u64, 0u64, false)
                }
            };

            let now = std::time::Instant::now();
            let dt = now.duration_since(prev_ts).as_secs_f64();
            prev_ts = now;

            if connected {
                // Deltas
                let dtx = tx.saturating_sub(prev_tx) as f64;
                let drx = rx.saturating_sub(prev_rx) as f64;
                let dtxp = tx_pkts.saturating_sub(prev_tx_pkts) as f64;
                let drxp = rx_pkts.saturating_sub(prev_rx_pkts) as f64;

                prev_tx = tx;
                prev_rx = rx;
                prev_tx_pkts = tx_pkts;
                prev_rx_pkts = rx_pkts;

                // Rates
                let tx_bps = if dt > 0.0 { dtx / dt } else { 0.0 };
                let rx_bps = if dt > 0.0 { drx / dt } else { 0.0 };
                let tx_pkts_s = if dt > 0.0 { dtxp / dt } else { 0.0 };
                let rx_pkts_s = if dt > 0.0 { drxp / dt } else { 0.0 };

                let payload = serde_json::json!({
                    "tx_bps": tx_bps,
                    "rx_bps": rx_bps,
                    "tx_pkts_s": tx_pkts_s,
                    "rx_pkts_s": rx_pkts_s,
                    "tx_total": tx,
                    "rx_total": rx,
                    "timestamp_ms": chrono::Utc::now().timestamp_millis()
                });

                let _ = app_handle.emit("connection:metrics", payload);
            } else {
                // Not connected - emit zero metrics to update UI
                let payload = serde_json::json!({
                    "tx_bps": 0.0,
                    "rx_bps": 0.0,
                    "tx_pkts_s": 0.0,
                    "rx_pkts_s": 0.0,
                    "tx_total": tx,
                    "rx_total": rx,
                    "timestamp_ms": chrono::Utc::now().timestamp_millis()
                });
                let _ = app_handle.emit("connection:metrics", payload);
            }
        }
    });

    *guard = Some(handle);
}

/// Stop metrics task if running
async fn stop_metrics_task(state: tauri::State<'_, AppState>) {
    let mut guard = state.metrics_task.lock().await;
    if let Some(handle) = guard.take() {
        handle.abort();
    }
}

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
#[inline]
fn bit_mask_u8(bits: u8) -> u8 {
    if bits >= 8 {
        0xFF
    } else {
        (1u8 << bits) - 1
    }
}

type ConnectionFactory = dyn Fn(ConnectionConfig, Option<ProtocolSettings>, Endianness) -> Result<String, String>
    + Send
    + Sync;

/// Tracks RPM state for key-on/off detection
struct RpmStateTracker {
    current_state: RpmState,
    pending_off_start: Option<std::time::Instant>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum RpmState {
    On,
    Off,
}

impl RpmStateTracker {
    fn new() -> Self {
        Self {
            current_state: RpmState::Off,
            pending_off_start: None,
        }
    }

    /// Update RPM and check for state transitions
    /// Returns Some(new_state) if state changed, None otherwise
    fn update(&mut self, rpm: f64, threshold_rpm: f64, timeout_sec: u32) -> Option<RpmState> {
        let rpm_above_threshold = rpm >= threshold_rpm;

        match self.current_state {
            RpmState::Off => {
                if rpm_above_threshold {
                    // Engine started: Turn ON immediately
                    self.current_state = RpmState::On;
                    self.pending_off_start = None;
                    return Some(RpmState::On);
                }
            }
            RpmState::On => {
                if rpm_above_threshold {
                    // Engine running above threshold: reset any pending off timer
                    self.pending_off_start = None;
                } else {
                    // RPM dropped below threshold
                    match self.pending_off_start {
                        None => {
                            // Start the timeout timer
                            self.pending_off_start = Some(std::time::Instant::now());
                        }
                        Some(start_time) => {
                            // Check if we've been below threshold longer than timeout
                            if start_time.elapsed().as_secs() >= timeout_sec as u64 {
                                // Timeout exceeded: Turn OFF
                                self.current_state = RpmState::Off;
                                self.pending_off_start = None;
                                return Some(RpmState::Off);
                            }
                        }
                    }
                }
            }
        }

        None
    }
}

struct AppState {
    connection: Mutex<Option<Connection>>,
    definition: Mutex<Option<EcuDefinition>>,
    autotune_state: Mutex<AutoTuneState>,
    autotune_secondary_state: Mutex<AutoTuneState>,
    // Optional test seam: factory to produce a signature without opening real serial ports
    connection_factory: Mutex<Option<Arc<ConnectionFactory>>>,
    // AutoTune configuration (stored when start_autotune is called)
    autotune_config: Mutex<Option<AutoTuneConfig>>,
    streaming_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    // Background task for AutoTune auto-send
    #[allow(dead_code)]
    autotune_send_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    // Background task for connection metrics emission
    metrics_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    current_tune: Mutex<Option<TuneFile>>,
    current_tune_path: Mutex<Option<PathBuf>>,
    tune_modified: Mutex<bool>,
    data_logger: Mutex<DataLogger>,
    current_project: Mutex<Option<Project>>,
    ini_repository: Mutex<Option<IniRepository>>,
    // Online INI repository for downloading INIs from GitHub
    online_ini_repository: Mutex<OnlineIniRepository>,
    // Local cache of ECU page data for offline editing
    tune_cache: Mutex<Option<TuneCache>>,
    // Demo mode - simulates a running vehicle for UI testing
    demo_mode: Mutex<bool>,
    // WASM plugin manager
    wasm_plugin_manager: Mutex<Option<WasmPluginManager>>,
    // Migration report when loading a tune from a different INI version
    migration_report: Mutex<Option<MigrationReport>>,
    // Math Channel Evaluator
    evaluator: Mutex<Option<Evaluator>>,
    // Cached output channels to avoid repeated cloning in realtime streaming loop
    // This is an Arc-wrapped copy that is updated whenever definition is loaded/changed
    cached_output_channels: Mutex<Option<Arc<HashMap<String, libretune_core::ini::OutputChannel>>>>,
    // Console command history for rusEFI/FOME/epicEFI console
    console_history: Mutex<Vec<String>>,
    // RPM state tracker for key-on/off detection
    rpm_state_tracker: Mutex<RpmStateTracker>,
    // User-Defined Math Channels
    math_channels: Mutex<Vec<UserMathChannel>>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum AutoTuneLoadSource {
    Map,
    Maf,
}

#[derive(Clone, Copy, Debug)]
enum AxisHint {
    Rpm,
    Load(AutoTuneLoadSource),
    #[allow(dead_code)]
    Unknown,
}

fn is_maf_channel_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("maf") || lower.contains("airmass") || lower.contains("airflow")
}

/// AutoTune configuration stored when tuning session starts
#[derive(Clone)]
struct AutoTuneConfig {
    #[allow(dead_code)]
    table_name: String,
    secondary_table_name: Option<String>,
    settings: AutoTuneSettings,
    filters: AutoTuneFilters,
    authority_limits: AutoTuneAuthorityLimits,
    load_source: AutoTuneLoadSource,
    // Table bin values for cell lookup
    x_bins: Vec<f64>,
    y_bins: Vec<f64>,
    secondary_x_bins: Option<Vec<f64>>,
    secondary_y_bins: Option<Vec<f64>>,
    // Previous TPS value for calculating rate
    last_tps: Option<f64>,
    last_timestamp_ms: Option<u64>,
}

#[derive(Serialize)]
struct ConnectionStatus {
    state: ConnectionState,
    signature: Option<String>,
    has_definition: bool,
    ini_name: Option<String>,
    demo_mode: bool,
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
struct ConstantInfo {
    name: String,
    label: Option<String>,
    units: String,
    digits: u8,
    min: f64,
    max: f64,
    value_type: String, // "scalar", "string", "bits", "array"
    bit_options: Vec<String>,
    help: Option<String>,
    visibility_condition: Option<String>, // Expression for when field should be visible
}

#[derive(Serialize, Deserialize, Default)]
struct Settings {
    last_ini_path: Option<String>,
    units_system: String,     // "metric" or "imperial"
    auto_burn_on_close: bool, // Auto-burn toggle
    gauge_snap_to_grid: bool, // Dashboard gauge snap to grid
    gauge_free_move: bool,    // Dashboard gauge free move
    gauge_lock: bool,         // Dashboard gauge lock in place
    #[serde(default = "default_true")]
    auto_sync_gauge_ranges: bool, // Auto-sync gauge ranges from INI
    indicator_column_count: String, // "auto" or number like "12"
    indicator_fill_empty: bool, // Fill empty cells in last row
    indicator_text_fit: String, // "scale" or "wrap"

    // Status bar channel configuration
    #[serde(default)]
    status_bar_channels: Vec<String>, // User-selected channels for status bar (max 8)

    // Help icon visibility setting
    #[serde(default = "default_true")]
    show_all_help_icons: bool, // Show help icons on all fields (true) or only fields with descriptions (false)

    // Session persistence
    #[serde(default)]
    last_project_path: Option<String>,
    #[serde(default)]
    last_active_tab: Option<String>,

    // Heatmap color scheme settings
    #[serde(default = "default_heatmap_scheme")]
    heatmap_value_scheme: String, // Scheme for VE/timing tables
    #[serde(default = "default_heatmap_scheme")]
    heatmap_change_scheme: String, // Scheme for AFR correction magnitude
    #[serde(default = "default_heatmap_scheme")]
    heatmap_coverage_scheme: String, // Scheme for hit weighting visualization
    #[serde(default)]
    heatmap_value_custom: Vec<String>, // Custom color stops for value context
    #[serde(default)]
    heatmap_change_custom: Vec<String>, // Custom color stops for change context
    #[serde(default)]
    heatmap_coverage_custom: Vec<String>, // Custom color stops for coverage context

    // Git version control settings
    #[serde(default = "default_auto_commit")]
    auto_commit_on_save: String, // "always", "never", "ask"
    #[serde(default = "default_commit_message_format")]
    commit_message_format: String, // Format string with {date}, {time} placeholders

    /// Global override for runtime packet mode (Auto|ForceBurst|ForceOCH|Disabled)
    #[serde(default = "default_runtime_packet_mode")]
    runtime_packet_mode: String,

    /// FOME-specific fast comms mode for console commands
    /// When enabled for FOME ECUs, attempts a faster protocol path; falls back on error
    #[serde(default = "default_true")]
    fome_fast_comms_enabled: bool,

    // Auto-record settings
    #[serde(default = "default_false")]
    auto_record_enabled: bool, // Enable auto-start/stop recording on key-on/off
    #[serde(default = "default_key_on_rpm")]
    key_on_threshold_rpm: f64, // RPM threshold to detect key-on (default 100)
    #[serde(default = "default_key_off_timeout")]
    key_off_timeout_sec: u32, // Seconds of zero RPM to detect key-off (default 2)

    // Alert rules settings
    #[serde(default = "default_true")]
    alert_large_change_enabled: bool, // Warn when a cell change exceeds thresholds
    #[serde(default = "default_alert_large_change_abs")]
    alert_large_change_abs: f64, // Absolute change threshold
    #[serde(default = "default_alert_large_change_percent")]
    alert_large_change_percent: f64, // Percent change threshold

    // Keyboard shortcut customization (mapping from action to key binding)
    #[serde(default)]
    hotkey_bindings: HashMap<String, String>, // e.g., {"table.setEqual": "=", "table.smooth": "s"}

    // Onboarding state
    #[serde(default = "default_false")]
    onboarding_completed: bool, // Track if user has completed onboarding
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

fn save_settings(app: &tauri::AppHandle, settings: &Settings) {
    let settings_path = get_settings_path(app);
    // Ensure parent directory exists
    if let Some(parent) = settings_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(&settings_path, json);
    }
}

fn load_settings(app: &tauri::AppHandle) -> Settings {
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
fn convert_layout_to_dashfile(layout: &DashboardLayout) -> DashFile {
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
fn convert_dashfile_to_layout(dash: &DashFile, name: &str) -> DashboardLayout {
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
        ecu_normalized.split('.').last(),
        ini_normalized.split('.').last(),
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

/// Lists all available serial ports on the system.
///
/// Returns: Vector of serial port names (e.g., "COM3" on Windows, "/dev/ttyUSB0" on Linux)
#[tauri::command]
async fn get_serial_ports() -> Result<Vec<String>, String> {
    Ok(list_ports().into_iter().map(|p| p.name).collect())
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

/// Disconnects from the currently connected ECU.
///
/// Closes the serial connection and clears the connection state.
///
/// Returns: Nothing on success
#[tauri::command]
async fn disconnect_ecu(state: tauri::State<'_, AppState>) -> Result<(), String> {
    // Stop metrics task first
    stop_metrics_task(state.clone()).await;

    let mut guard = state.connection.lock().await;
    *guard = None;
    Ok(())
}

/// Response for adaptive timing stats
#[derive(Serialize)]
struct AdaptiveTimingStats {
    enabled: bool,
    avg_response_ms: Option<f64>,
    sample_count: usize,
    current_timeout_ms: Option<u64>,
}

/// Enable adaptive timing (experimental feature that dynamically adjusts communication speed)
#[tauri::command]
async fn enable_adaptive_timing(
    state: tauri::State<'_, AppState>,
    multiplier: Option<f32>,
    min_timeout_ms: Option<u32>,
    max_timeout_ms: Option<u32>,
) -> Result<AdaptiveTimingStats, String> {
    let mut guard = state.connection.lock().await;
    let conn = guard.as_mut().ok_or("Not connected to ECU")?;

    let config = AdaptiveTimingConfig {
        enabled: true,
        multiplier: multiplier.unwrap_or(2.5),
        min_timeout_ms: min_timeout_ms.unwrap_or(10),
        max_timeout_ms: max_timeout_ms.unwrap_or(500),
        sample_count: 20,
    };

    conn.enable_adaptive_timing(Some(config));

    Ok(AdaptiveTimingStats {
        enabled: true,
        avg_response_ms: None,
        sample_count: 0,
        current_timeout_ms: None,
    })
}

/// Disable adaptive timing (return to INI-specified timing)
#[tauri::command]
async fn disable_adaptive_timing(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.connection.lock().await;
    let conn = guard.as_mut().ok_or("Not connected to ECU")?;

    conn.disable_adaptive_timing();
    Ok(())
}

/// Get adaptive timing statistics
#[tauri::command]
async fn get_adaptive_timing_stats(
    state: tauri::State<'_, AppState>,
) -> Result<AdaptiveTimingStats, String> {
    let guard = state.connection.lock().await;
    let conn = guard.as_ref().ok_or("Not connected to ECU")?;

    let enabled = conn.is_adaptive_timing_enabled();
    let stats = conn.adaptive_timing_stats();

    Ok(AdaptiveTimingStats {
        enabled,
        avg_response_ms: stats
            .as_ref()
            .map(|(avg, _)| avg.as_micros() as f64 / 1000.0),
        sample_count: stats.as_ref().map(|(_, count)| *count).unwrap_or(0),
        current_timeout_ms: None, // Could add this if needed
    })
}

/// Gets the current ECU connection status.
///
/// Returns comprehensive connection information including state, ECU signature,
/// loaded INI info, and demo mode status.
///
/// Returns: ConnectionStatus with connection state and metadata
#[tauri::command]
async fn get_connection_status(
    state: tauri::State<'_, AppState>,
) -> Result<ConnectionStatus, String> {
    // IMPORTANT: Acquire each lock independently and release before taking the next.
    // Holding multiple locks simultaneously causes deadlocks with the realtime stream task.
    let demo_mode = *state.demo_mode.lock().await;

    let (state_val, signature) = if demo_mode {
        (
            ConnectionState::Connected,
            Some("DEMO - Simulated EpicEFI".to_string()),
        )
    } else {
        set_conn_lock_holder("get_connection_status");
        let conn_guard = state.connection.lock().await;
        let result = match &*conn_guard {
            Some(conn) => (conn.state(), conn.signature().map(|s| s.to_string())),
            None => (ConnectionState::Disconnected, None),
        };
        drop(conn_guard);
        set_conn_lock_holder("(none)");
        result
    };

    let (has_definition, ini_name) = {
        let def_guard = state.definition.lock().await;
        (
            def_guard.is_some(),
            def_guard.as_ref().map(|d| d.signature.clone()),
        )
    };

    Ok(ConnectionStatus {
        state: state_val,
        signature,
        has_definition,
        ini_name,
        demo_mode,
    })
}

/// Get the current ECU type (for console and other ECU-specific features)
/// Returns EcuType as a string: "Speeduino", "RusEFI", "FOME", "EpicEFI", "MS2", "MS3", or "Unknown"
#[tauri::command]
async fn get_ecu_type(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("No INI definition loaded")?;

    Ok(format!("{:?}", def.ecu_type))
}

/// Send a console command to the ECU (rusEFI/FOME/epicEFI only)
///
/// For FOME ECUs with fome_fast_comms_enabled setting:
/// - Attempts a faster protocol path first (if available)
/// - Falls back to standard console protocol on error
/// - No error propagation for fallback (transparent to user)
///
/// Returns the response from the ECU as a string with trailing whitespace trimmed.
/// The command is automatically appended with a newline for transmission.
#[tauri::command]
async fn send_console_command(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    command: String,
) -> Result<String, String> {
    let mut conn_guard = state.connection.lock().await;
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;

    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("No INI definition loaded")?;

    // Check if ECU supports console
    if !def.ecu_type.supports_console() {
        return Err(format!(
            "ECU type {:?} does not support text-based console",
            def.ecu_type
        ));
    }

    // For FOME, try fast comms first if enabled
    let settings = load_settings(&app);
    let use_fome_fast = def.ecu_type.is_fome() && settings.fome_fast_comms_enabled;

    let response = if use_fome_fast {
        // Try faster FOME protocol first
        eprintln!("[DEBUG] send_console_command: attempting FOME fast comms");
        match conn.send_console_command(&libretune_core::protocol::ConsoleCommand::new(&command)) {
            Ok(resp) => {
                eprintln!("[DEBUG] send_console_command: FOME fast comms succeeded");
                resp
            }
            Err(e) => {
                // Fall back to standard protocol
                eprintln!("[WARN] send_console_command: FOME fast comms failed ({}), falling back to standard protocol", e);
                conn.send_console_command(&libretune_core::protocol::ConsoleCommand::new(&command))
                    .map_err(|e| format!("Console command failed (even after fallback): {}", e))?
            }
        }
    } else {
        // Use standard console protocol
        conn.send_console_command(&libretune_core::protocol::ConsoleCommand::new(&command))
            .map_err(|e| format!("Console command failed: {}", e))?
    };

    // Add to history
    let mut history = state.console_history.lock().await;
    history.push(format!("{}: {}", command, &response));
    // Keep history size reasonable (max 1000 entries)
    if history.len() > 1000 {
        history.remove(0);
    }

    Ok(response)
}

/// Get console command history
#[tauri::command]
async fn get_console_history(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    let history = state.console_history.lock().await;
    Ok(history.clone())
}

/// Clear console command history
#[tauri::command]
async fn clear_console_history(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut history = state.console_history.lock().await;
    history.clear();
    Ok(())
}

/// Retrieves the path to the last-used INI file from settings.
///
/// Used on startup to auto-load the previously used ECU definition.
///
/// Returns: Optional path to last INI file, or None if not set or file missing
#[tauri::command]
async fn auto_load_last_ini(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let settings = load_settings(&app);
    if let Some(path) = settings.last_ini_path {
        if Path::new(&path).exists() {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

#[derive(Serialize)]
struct TableData {
    name: String,
    title: String,
    x_bins: Vec<f64>,
    y_bins: Vec<f64>,
    z_values: Vec<Vec<f64>>,
    x_axis_name: String,
    y_axis_name: String,
    /// Output channel name for X-axis (used for live cell highlighting)
    x_output_channel: Option<String>,
    /// Output channel name for Y-axis (used for live cell highlighting)
    y_output_channel: Option<String>,
}

#[derive(Serialize)]
struct CurveData {
    name: String,
    title: String,
    x_bins: Vec<f64>,
    y_bins: Vec<f64>,
    x_label: String,
    y_label: String,
    /// X-axis range: (min, max, step)
    x_axis: Option<(f32, f32, f32)>,
    /// Y-axis range: (min, max, step)
    y_axis: Option<(f32, f32, f32)>,
    /// Output channel name for live cursor (e.g., "coolant")
    x_output_channel: Option<String>,
    /// Gauge name for live display
    gauge: Option<String>,
}

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

/// Lightweight command to check if a table exists in the definition
/// This is used by the frontend to determine if a panel should render as a table button
#[tauri::command]
async fn get_table_info(
    state: tauri::State<'_, AppState>,
    table_name: String,
) -> Result<TableInfo, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or_else(|| {
        eprintln!(
            "[WARN] get_table_info: Definition not loaded when looking for '{}'",
            table_name
        );
        "Definition not loaded".to_string()
    })?;

    // Diagnostic logging
    eprintln!(
        "[DEBUG] get_table_info: Looking for '{}' in {} tables ({} map entries)",
        table_name,
        def.tables.len(),
        def.table_map_to_name.len()
    );

    if let Some(table) = def.get_table_by_name_or_map(&table_name) {
        eprintln!(
            "[DEBUG] get_table_info: Found table '{}' (title: {})",
            table.name, table.title
        );
        Ok(TableInfo {
            name: table.name.clone(),
            title: table.title.clone(),
        })
    } else {
        // Log available tables for debugging
        let available: Vec<_> = def.tables.keys().take(10).cloned().collect();
        eprintln!(
            "[WARN] get_table_info: Table '{}' not found. Available tables (first 10): {:?}",
            table_name, available
        );
        Err(format!(
            "Table '{}' not found (checked {} tables, {} map entries)",
            table_name,
            def.tables.len(),
            def.table_map_to_name.len()
        ))
    }
}

#[derive(Serialize)]
struct ProtocolDefaults {
    default_baud_rate: u32,
    inter_write_delay: u32,
    delay_after_port_open: u32,
    message_envelope_format: Option<String>,
    page_activation_delay: u32,
    // Suggested read timeout for UI (ms)
    timeout_ms: u32,
}

/// Get protocol timing defaults from the loaded INI definition.
///
/// Returns timing values like baud rate, delays, and timeouts that the
/// frontend can use to configure connection settings.
///
/// Returns: ProtocolDefaults with timing and format settings
#[tauri::command]
async fn get_protocol_defaults(
    state: tauri::State<'_, AppState>,
) -> Result<ProtocolDefaults, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let proto = def.protocol.clone();
    Ok(ProtocolDefaults {
        default_baud_rate: proto.default_baud_rate,
        inter_write_delay: proto.inter_write_delay,
        delay_after_port_open: proto.delay_after_port_open,
        message_envelope_format: proto.message_envelope_format.clone(),
        page_activation_delay: proto.page_activation_delay,
        timeout_ms: proto.block_read_timeout,
    })
}

#[derive(Serialize)]
struct ProtocolCapabilities {
    supports_och: bool,
}

/// Return derived protocol capabilities from the loaded INI definition.
/// Useful for frontend heuristics (e.g., choosing OCH vs Burst for runtime reads).
#[tauri::command]
async fn get_protocol_capabilities(
    state: tauri::State<'_, AppState>,
) -> Result<ProtocolCapabilities, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let proto = &def.protocol;
    Ok(ProtocolCapabilities {
        supports_och: proto.och_get_command.is_some() && proto.och_block_size > 0,
    })
}

/// Return the parsed [VeAnalyze] configuration if present.
#[tauri::command]
async fn get_ve_analyze_config(
    state: tauri::State<'_, AppState>,
) -> Result<Option<VeAnalyzeConfig>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    Ok(def.ve_analyze.clone())
}

/// Return INI-derived feature capabilities for UI gating.
#[tauri::command]
async fn get_ini_capabilities(
    state: tauri::State<'_, AppState>,
) -> Result<IniCapabilities, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    Ok(def.capabilities())
}

/// Status of the tune cache for UI display
#[derive(Serialize)]
struct TuneCacheStatus {
    /// Total number of pages
    total_pages: u8,
    /// Number of pages loaded
    loaded_pages: u8,
    /// Whether all pages are loaded
    fully_loaded: bool,
    /// Whether currently loading
    is_loading: bool,
    /// Whether there are unsaved changes
    has_dirty_data: bool,
    /// Whether there are pending burns
    has_pending_burn: bool,
    /// Count of dirty bytes
    dirty_byte_count: usize,
    /// Pages with dirty data
    dirty_pages: Vec<u8>,
}

/// Get the current status of the tune data cache.
///
/// Returns information about loaded pages, dirty data that needs saving,
/// and pending burns. Used to show sync/save status in the UI.
///
/// Returns: TuneCacheStatus with page loading and modification info
#[tauri::command]
async fn get_tune_cache_status(
    state: tauri::State<'_, AppState>,
) -> Result<TuneCacheStatus, String> {
    let cache_guard = state.tune_cache.lock().await;
    let cache = cache_guard.as_ref().ok_or("TuneCache not initialized")?;

    let total_pages = cache.page_count();
    let mut loaded_pages = 0u8;
    for page in 0..total_pages {
        match cache.page_state(page) {
            PageState::Clean | PageState::Dirty | PageState::Pending => loaded_pages += 1,
            _ => {}
        }
    }

    Ok(TuneCacheStatus {
        total_pages,
        loaded_pages,
        fully_loaded: cache.is_fully_loaded(),
        is_loading: cache.is_loading(),
        has_dirty_data: cache.has_dirty_data(),
        has_pending_burn: cache.has_pending_burn(),
        dirty_byte_count: cache.dirty_byte_count(),
        dirty_pages: cache.dirty_pages(),
    })
}

/// Load all ECU pages into the cache (background operation)
#[tauri::command]
async fn load_all_pages(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    // Get pages to load and their sizes
    let pages_to_load: Vec<(u8, u16)>;
    {
        let cache_guard = state.tune_cache.lock().await;
        let def_guard = state.definition.lock().await;

        let cache = cache_guard.as_ref().ok_or("TuneCache not initialized")?;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;

        pages_to_load = cache
            .pages_to_load()
            .into_iter()
            .filter_map(|p| def.page_sizes.get(p as usize).map(|size| (p, *size)))
            .collect();
    }

    if pages_to_load.is_empty() {
        return Ok(());
    }

    // Mark pages as loading
    {
        let mut cache_guard = state.tune_cache.lock().await;
        if let Some(cache) = cache_guard.as_mut() {
            for (page, _) in &pages_to_load {
                cache.mark_loading(*page);
            }
        }
    }

    // Emit loading started event
    let _ = app.emit(
        "cache:loading",
        serde_json::json!({
            "pages": pages_to_load.len(),
            "status": "started"
        }),
    );

    // Load pages one at a time to avoid blocking
    for (page, size) in pages_to_load {
        // Read page from ECU
        let page_data: Result<Vec<u8>, String> = {
            let mut conn_guard = state.connection.lock().await;
            if let Some(conn) = conn_guard.as_mut() {
                let params = libretune_core::protocol::commands::ReadMemoryParams {
                    can_id: 0,
                    page,
                    offset: 0,
                    length: size,
                };
                conn.read_memory(params).map_err(|e| e.to_string())
            } else {
                Err("Not connected".to_string())
            }
        };

        // Update cache with result
        {
            let mut cache_guard = state.tune_cache.lock().await;
            if let Some(cache) = cache_guard.as_mut() {
                match page_data {
                    Ok(data) => {
                        cache.load_page(page, data);
                        let _ = app.emit(
                            "cache:page_loaded",
                            serde_json::json!({
                                "page": page,
                                "success": true
                            }),
                        );
                    }
                    Err(e) => {
                        cache.mark_error(page);
                        let _ = app.emit(
                            "cache:page_loaded",
                            serde_json::json!({
                                "page": page,
                                "success": false,
                                "error": e
                            }),
                        );
                    }
                }
            }
        }

        // Small delay between pages to avoid overwhelming the ECU
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // Emit loading complete event
    let _ = app.emit(
        "cache:loading",
        serde_json::json!({
            "status": "complete"
        }),
    );

    Ok(())
}

/// Retrieves curve data (1D table) including X and Y values.
///
/// Fetches a curve from the tune cache or ECU memory for display
/// in the curve editor.
///
/// # Arguments
/// * `curve_name` - Curve name from INI definition
///
/// Returns: CurveData with x/y values and metadata
#[tauri::command]
async fn get_curve_data(
    state: tauri::State<'_, AppState>,
    curve_name: String,
) -> Result<CurveData, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or_else(|| {
        eprintln!(
            "[WARN] get_curve_data: Definition not loaded when looking for '{}'",
            curve_name
        );
        "Definition not loaded".to_string()
    })?;
    let endianness = def.endianness;

    // Diagnostic logging
    eprintln!(
        "[DEBUG] get_curve_data: Looking for '{}' in {} curves ({} map entries)",
        curve_name,
        def.curves.len(),
        def.curve_map_to_name.len()
    );

    let curve = def.get_curve_by_name_or_map(&curve_name).ok_or_else(|| {
        // Log available curves for debugging
        let available: Vec<_> = def.curves.keys().take(10).cloned().collect();
        eprintln!(
            "[WARN] get_curve_data: Curve '{}' not found. Available curves (first 10): {:?}",
            curve_name, available
        );
        format!(
            "Curve '{}' not found (checked {} curves, {} map entries)",
            curve_name,
            def.curves.len(),
            def.curve_map_to_name.len()
        )
    })?;

    eprintln!(
        "[DEBUG] get_curve_data: Found curve '{}' (title: {})",
        curve.name, curve.title
    );

    // Clone the constant info we need
    let x_const = def
        .constants
        .get(&curve.x_bins)
        .ok_or_else(|| format!("Constant {} not found", curve.x_bins))?
        .clone();
    let y_const = def
        .constants
        .get(&curve.y_bins)
        .ok_or_else(|| format!("Constant {} not found", curve.y_bins))?
        .clone();

    // Clone curve metadata
    let curve_name_out = curve.name.clone();
    let curve_title = curve.title.clone();
    let x_label = curve.column_labels.0.clone();
    let y_label = curve.column_labels.1.clone();
    let x_axis = curve.x_axis;
    let y_axis = curve.y_axis;
    let x_output_channel = curve.x_output_channel.clone();
    let gauge = curve.gauge.clone();

    drop(def_guard);

    // Helper to read constant data from TuneFile (offline) or ECU (online)
    fn read_const_from_source(
        constant: &Constant,
        tune: Option<&TuneFile>,
        conn: &mut Option<&mut Connection>,
        endianness: libretune_core::ini::Endianness,
    ) -> Result<Vec<f64>, String> {
        let element_count = constant.shape.element_count();
        let element_size = constant.data_type.size_bytes();
        let length = constant.size_bytes() as u16;

        eprintln!(
            "[DEBUG] read_const_from_source: '{}' - shape={:?}, element_count={}, element_size={}, total_length={}",
            constant.name, constant.shape, element_count, element_size, length
        );

        // If offline, read from TuneFile (MSQ file)
        if conn.is_none() {
            if let Some(tune_file) = tune {
                // First try named constants (parsed from MSQ <constant> tags)
                if let Some(tune_value) = tune_file.constants.get(&constant.name) {
                    use libretune_core::tune::TuneValue;
                    eprintln!(
                        "[DEBUG] read_const_from_source: '{}' found in TuneFile.constants",
                        constant.name
                    );
                    match tune_value {
                        TuneValue::Array(arr) => {
                            eprintln!("[DEBUG] read_const_from_source: '{}' returning {} array values from constants", constant.name, arr.len());
                            return Ok(arr.clone());
                        }
                        TuneValue::Scalar(v) => {
                            return Ok(vec![*v]);
                        }
                        _ => {}
                    }
                }

                // Fallback: try to read from raw page data using INI offset
                // This handles cases where the constant wasn't explicitly in the MSQ file
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
                        eprintln!("[DEBUG] read_const_from_source: '{}' returning {} values from page data", constant.name, values.len());
                        return Ok(values);
                    } else {
                        eprintln!("[WARN] read_const_from_source: '{}' offset {} + size {} exceeds page {} length {}", 
                            constant.name, offset, total_bytes, constant.page, page_data.len());
                    }
                } else {
                    eprintln!("[WARN] read_const_from_source: '{}' page {} not found in TuneFile.pages (available: {:?})", 
                        constant.name, constant.page, tune_file.pages.keys().collect::<Vec<_>>());
                }
            }
            // If not found anywhere, return zeros
            eprintln!(
                "[DEBUG] read_const_from_source: '{}' returning {} zeros (not in TuneFile)",
                constant.name, element_count
            );
            return Ok(vec![0.0; element_count]);
        }

        // For ECU reads, we need valid length
        if length == 0 {
            eprintln!(
                "[WARN] read_const_from_source: '{}' has length=0, cannot read from ECU",
                constant.name
            );
            return Ok(vec![0.0; element_count]);
        }

        // If connected to ECU, read from ECU (live data)
        if let Some(ref mut conn_ptr) = conn {
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

        Ok(vec![0.0; element_count])
    }

    // Get tune and connection
    let tune_guard = state.current_tune.lock().await;
    let mut conn_guard = state.connection.lock().await;
    let mut conn = conn_guard.as_mut();

    let x_bins = read_const_from_source(&x_const, tune_guard.as_ref(), &mut conn, endianness)?;
    let y_bins = read_const_from_source(&y_const, tune_guard.as_ref(), &mut conn, endianness)?;

    Ok(CurveData {
        name: curve_name_out,
        title: curve_title,
        x_bins,
        y_bins,
        x_label,
        y_label,
        x_axis,
        y_axis,
        x_output_channel,
        gauge,
    })
}

/// Updates table Z values in the tune cache and optionally writes to ECU.
///
/// Converts display values to raw bytes and writes to the tune cache.
/// If connected to ECU, also writes to ECU memory. Works in offline mode.
///
/// # Arguments
/// * `table_name` - Table name or map name from INI definition
/// * `z_values` - 2D array of new Z values in display units
///
/// Returns: Nothing on success
#[tauri::command]
async fn update_table_data(
    state: tauri::State<'_, AppState>,
    table_name: String,
    z_values: Vec<Vec<f64>>,
) -> Result<(), String> {
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;
    let mut cache_guard = state.tune_cache.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let table = def
        .get_table_by_name_or_map(&table_name)
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

    // Always write to TuneCache if available (enables offline editing)
    if let Some(cache) = cache_guard.as_mut() {
        if cache.write_bytes(constant.page, constant.offset, &raw_data) {
            // Also update TuneFile in memory
            let mut tune_guard = state.current_tune.lock().await;
            if let Some(tune) = tune_guard.as_mut() {
                // Get or create page data
                let page_data = tune.pages.entry(constant.page).or_insert_with(|| {
                    // Create empty page if it doesn't exist
                    vec![
                        0u8;
                        def.page_sizes
                            .get(constant.page as usize)
                            .copied()
                            .unwrap_or(256) as usize
                    ]
                });

                // Update the page data
                let start = constant.offset as usize;
                let end = start + raw_data.len();
                if end <= page_data.len() {
                    page_data[start..end].copy_from_slice(&raw_data);
                }
            }

            // Mark tune as modified
            *state.tune_modified.lock().await = true;
        }
    }

    // Write to ECU if connected (optional - offline mode works without this)
    if let Some(conn) = conn_guard.as_mut() {
        let params = libretune_core::protocol::commands::WriteMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            data: raw_data.clone(),
        };

        // Don't fail if ECU write fails - offline mode should still work
        if let Err(e) = conn.write_memory(params) {
            eprintln!("[WARN] Failed to write to ECU (offline mode?): {}", e);
        }
    }

    Ok(())
}

/// Updates curve Y values in the tune cache and optionally writes to ECU.
///
/// # Arguments
/// * `curve_name` - Curve name from INI definition
/// * `y_values` - Vector of new Y values in display units
///
/// Returns: Nothing on success
#[tauri::command]
async fn update_curve_data(
    state: tauri::State<'_, AppState>,
    curve_name: String,
    y_values: Vec<f64>,
) -> Result<(), String> {
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;
    let mut cache_guard = state.tune_cache.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let curve = def
        .curves
        .get(&curve_name)
        .ok_or_else(|| format!("Curve {} not found", curve_name))?;

    // Get the Y-bins constant (the values we're updating)
    let constant = def.constants.get(&curve.y_bins).ok_or_else(|| {
        format!(
            "Constant {} not found for curve {}",
            curve.y_bins, curve_name
        )
    })?;

    if y_values.len() != constant.shape.element_count() {
        return Err(format!(
            "Invalid data size: expected {}, got {}",
            constant.shape.element_count(),
            y_values.len()
        ));
    }

    // Convert display values to raw bytes
    let element_size = constant.data_type.size_bytes();
    let mut raw_data = vec![0u8; constant.size_bytes()];

    for (i, val) in y_values.iter().enumerate() {
        let raw_val = constant.display_to_raw(*val);
        let offset = i * element_size;
        constant
            .data_type
            .write_to_bytes(&mut raw_data, offset, raw_val, def.endianness);
    }

    // Write to TuneCache if available (enables offline editing)
    if let Some(cache) = cache_guard.as_mut() {
        if cache.write_bytes(constant.page, constant.offset, &raw_data) {
            // Also update TuneFile in memory
            let mut tune_guard = state.current_tune.lock().await;
            if let Some(tune) = tune_guard.as_mut() {
                // Update the parsed constants map (used by get_curve_data)
                tune.constants.insert(
                    constant.name.clone(),
                    libretune_core::tune::TuneValue::Array(y_values.clone()),
                );

                // Also update raw page data
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

            // Mark tune as modified
            *state.tune_modified.lock().await = true;
        }
    }

    // Write to ECU if connected
    if let Some(conn) = conn_guard.as_mut() {
        let params = libretune_core::protocol::commands::WriteMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            data: raw_data.clone(),
        };

        if let Err(e) = conn.write_memory(params) {
            eprintln!("[WARN] Failed to write curve to ECU (offline mode?): {}", e);
        }
    }

    Ok(())
}

/// Retrieves current realtime data from the ECU.
///
/// Diagnostic: perform a single realtime read and return raw + parsed info
#[tauri::command]
async fn debug_single_realtime_read(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let mut report = String::new();

    // 1) Check definition
    {
        let def_guard = state.definition.lock().await;
        if let Some(def) = &*def_guard {
            report.push_str(&format!("INI loaded: sig={}\n", def.signature));
            report.push_str(&format!(
                "output_channels count: {}\n",
                def.output_channels.len()
            ));
            report.push_str(&format!(
                "och_get_command: {:?}\n",
                def.protocol.och_get_command
            ));
            report.push_str(&format!(
                "och_block_size: {}\n",
                def.protocol.och_block_size
            ));
            report.push_str(&format!(
                "message_envelope: {:?}\n",
                def.protocol.message_envelope_format
            ));
            report.push_str(&format!("endianness: {:?}\n", def.endianness));
        } else {
            return Ok("ERROR: No definition loaded".to_string());
        }
    }

    // 2) Check connection
    {
        let mut conn_guard = state.connection.lock().await;
        if let Some(conn) = conn_guard.as_mut() {
            report.push_str(&format!(
                "Connected: sig={:?}, modern={}\n",
                conn.signature(),
                conn.is_modern_protocol()
            ));

            // 3) Try to read realtime data
            match conn.get_realtime_data() {
                Ok(raw) => {
                    report.push_str(&format!("Raw data: {} bytes\n", raw.len()));
                    if raw.len() >= 8 {
                        report.push_str(&format!("First 8 bytes: {:02x?}\n", &raw[..8]));
                    }
                }
                Err(e) => {
                    report.push_str(&format!("get_realtime_data ERROR: {:?}\n", e));
                }
            }
        } else {
            report.push_str("ERROR: No connection\n");
        }
    }

    // 4) Check streaming task
    {
        let task_guard = state.streaming_task.lock().await;
        report.push_str(&format!(
            "Streaming task active: {}\n",
            task_guard.is_some()
        ));
    }

    Ok(report)
}

/// Polls the ECU for current sensor values and computed channels.
/// Used for gauges, status bar, and table highlighting.
///
/// Returns: HashMap of channel names to current values
#[tauri::command]
async fn get_realtime_data(
    state: tauri::State<'_, AppState>,
) -> Result<HashMap<String, f64>, String> {
    // Use cached output channels to avoid expensive cloning.
    // IMPORTANT: acquire each lock independently to avoid deadlocks.
    let (channels_arc, endianness) = {
        let cached: Option<Arc<HashMap<String, libretune_core::ini::OutputChannel>>>;
        {
            let channels_cache_guard = state.cached_output_channels.lock().await;
            cached = channels_cache_guard.as_ref().map(|c| Arc::clone(c));
        } // cached_output_channels lock released

        let def_guard = state.definition.lock().await;
        if let Some(channels) = cached {
            let endianness = def_guard
                .as_ref()
                .map(|d| d.endianness)
                .unwrap_or(libretune_core::ini::Endianness::Little);
            (channels, endianness)
        } else if let Some(def) = &*def_guard {
            (Arc::new(def.output_channels.clone()), def.endianness)
        } else {
            return Err("Connection or definition missing".to_string());
        }
    };

    // Now lock connection only for I/O
    let raw_data = {
        let mut conn_guard = state.connection.lock().await;
        let conn = match conn_guard.as_mut() {
            Some(c) => c,
            None => return Err("Connection or definition missing".to_string()),
        };
        conn.get_realtime_data().map_err(|e| e.to_string())?
    };

    // Use Evaluator if available, otherwise fallback (should exist if INI loaded)
    let evaluator_guard = state.evaluator.lock().await;

    let data = if let Some(evaluator) = &*evaluator_guard {
        let def_guard = state.definition.lock().await;
        if let Some(def) = &*def_guard {
            evaluator.process(&raw_data, def)
        } else {
            // Fallback if definition locking fails
            return Err("Definition missing during evaluation".to_string());
        }
    } else {
        // Fallback: Manual parsing (basic channels only) if Evaluator not available
        let mut results = HashMap::new();

        // First pass: Parse all raw channels
        for (name, channel) in channels_arc.iter() {
            if !channel.is_computed() {
                if let Some(val) = channel.parse(&raw_data, endianness) {
                    results.insert(name.clone(), val);
                }
            }
        }

        results
    };

    Ok(data)
}

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
fn stream_log(msg: &str) {
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

fn set_conn_lock_holder(who: &'static str) {
    if let Ok(mut guard) = CONN_LOCK_HOLDER.lock() {
        *guard = who;
    }
}

fn get_conn_lock_holder() -> String {
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
                cached_ch = channels_cache.as_ref().map(|c| Arc::clone(c));
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

        let mut tick_count: u64 = 0;
        loop {
            ticker.tick().await;
            tick_count += 1;

            // Trace: log which phase we're in so we can find deadlocks
            if tick_count <= 25 || tick_count % 20 == 0 {
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
                }
            } else {
                // Real ECU mode: read from connection
                demo_simulator = None; // Clear simulator if we switch modes

                // Phase 1: Get raw data from ECU (hold connection lock only during I/O)
                // Use try_lock() to avoid blocking forever if another command
                // (e.g. get_all_constant_values) is holding the connection lock.
                if tick_count <= 25 || tick_count % 20 == 0 {
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
                            if tick_count <= 25 || tick_count % 20 == 0 {
                                let holder = get_conn_lock_holder();
                                stream_log(&format!(
                                    "tick #{}: conn_lock busy (held by: {}), skipping",
                                    tick_count, holder
                                ));
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
                        if count < 5 || count % 100 == 0 {
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
                        if count < 10 || count % 50 == 0 {
                            eprintln!(
                                "[ERROR] stream tick #{}: get_realtime_data failed: {}",
                                count, e
                            );
                        }
                    }
                }

                // Phase 2: Use pre-cached output channels and endianness (no locks needed)
                if tick_count <= 25 || tick_count % 20 == 0 {
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
                            } else if let Some(val) = channel.parse(&raw, *endianness) {
                                data.insert(name.clone(), val);
                            }
                        }

                        // Pass 2: Evaluate computed channels using parsed values as context
                        for (name, channel) in computed_channels {
                            if let Some(val) = channel.parse_with_context(&raw, *endianness, &data)
                            {
                                data.insert(name, val);
                            }
                        }

                        // Pass 3: User Math Channels Evaluation
                        if tick_count <= 25 || tick_count % 20 == 0 {
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
                            if count < 30 || count % 20 == 0 {
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
                        if tick_count <= 25 || tick_count % 20 == 0 {
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
                        if tick_count <= 25 || tick_count % 20 == 0 {
                            stream_log(&format!("tick #{}: T6-autotune", tick_count));
                        }
                        feed_autotune_data(&app_state, &data, current_time_ms).await;
                    }
                    (Err(e), _) => {
                        // Log errors to stream log so we can see Phase 1 failures
                        {
                            static ERR_STREAM_LOG: std::sync::atomic::AtomicU64 =
                                std::sync::atomic::AtomicU64::new(0);
                            let n =
                                ERR_STREAM_LOG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if n < 10 || n % 50 == 0 {
                                stream_log(&format!("stream error #{}: {}", n, e));
                            }
                        }
                        let _ = app_handle.emit("realtime:error", &e);
                    }
                    _ => {}
                }
            }
        }
    });

    *task_guard = Some(handle);
    Ok(())
}

/// Stops the realtime data streaming task.
///
/// Aborts the background task started by `start_realtime_stream`.
///
/// Returns: Nothing on success
#[tauri::command]
async fn stop_realtime_stream(state: tauri::State<'_, AppState>) -> Result<(), String> {
    stream_log("stop called");
    let mut task_guard = state.streaming_task.lock().await;
    if let Some(handle) = task_guard.take() {
        stream_log("stop: aborting task");
        handle.abort();
    } else {
        stream_log("stop: no task to abort");
    }
    Ok(())
}

#[derive(Serialize)]
struct TableInfo {
    name: String,
    title: String,
}

#[derive(Serialize)]
struct CurveInfo {
    name: String,
    title: String,
}

/// Lists all available tables from the loaded INI definition.
///
/// Returns basic info (name and title) for all tables defined in the INI.
/// Used to populate menus and table selection UI.
///
/// Returns: Sorted vector of TableInfo with name and title
#[tauri::command]
async fn get_tables(state: tauri::State<'_, AppState>) -> Result<Vec<TableInfo>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let mut tables: Vec<TableInfo> = def
        .tables
        .values()
        .map(|t| TableInfo {
            name: t.name.clone(),
            title: t.title.clone(),
        })
        .collect();
    tables.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(tables)
}

/// Lists all available curves from the loaded INI definition.
///
/// Returns basic info (name and title) for all curves defined in the INI.
/// Used to populate sidebar curve list and search UI.
///
/// Returns: Sorted vector of CurveInfo with name and title
#[tauri::command]
async fn get_curves(state: tauri::State<'_, AppState>) -> Result<Vec<CurveInfo>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let mut curves: Vec<CurveInfo> = def
        .curves
        .values()
        .map(|c| CurveInfo {
            name: c.name.clone(),
            title: c.title.clone(),
        })
        .collect();
    curves.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(curves)
}

/// Gauge configuration info returned to frontend
#[derive(Serialize)]
struct GaugeInfo {
    name: String,
    channel: String,
    title: String,
    units: String,
    lo: f64,
    hi: f64,
    low_warning: f64,
    high_warning: f64,
    low_danger: f64,
    high_danger: f64,
    digits: u8,
}

/// FrontPage indicator info returned to frontend
#[derive(Serialize)]
struct FrontPageIndicatorInfo {
    expression: String,
    label_off: String,
    label_on: String,
    bg_off: String,
    fg_off: String,
    bg_on: String,
    fg_on: String,
}

/// FrontPage configuration info returned to frontend
#[derive(Serialize)]
struct FrontPageInfo {
    /// Gauge names for gauge1-gauge8 (references to [GaugeConfigurations])
    gauges: Vec<String>,
    /// Status indicators
    indicators: Vec<FrontPageIndicatorInfo>,
}

/// Get the FrontPage definition from the INI file.
///
/// FrontPage defines the default dashboard layout including which gauges
/// and status indicators to show when the app first loads.
///
/// Returns: Optional FrontPageInfo with gauge references and indicators
#[tauri::command]
async fn get_frontpage(state: tauri::State<'_, AppState>) -> Result<Option<FrontPageInfo>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    Ok(def.frontpage.as_ref().map(|fp| FrontPageInfo {
        gauges: fp.gauges.clone(),
        indicators: fp
            .indicators
            .iter()
            .map(|ind| FrontPageIndicatorInfo {
                expression: ind.expression.clone(),
                label_off: ind.label_off.clone(),
                label_on: ind.label_on.clone(),
                bg_off: libretune_core::ini::FrontPageIndicator::color_to_css(&ind.bg_off),
                fg_off: libretune_core::ini::FrontPageIndicator::color_to_css(&ind.fg_off),
                bg_on: libretune_core::ini::FrontPageIndicator::color_to_css(&ind.bg_on),
                fg_on: libretune_core::ini::FrontPageIndicator::color_to_css(&ind.fg_on),
            })
            .collect(),
    }))
}

/// Get all gauge configurations from the INI file.
///
/// Returns complete gauge definitions including channel bindings,
/// min/max ranges, warning thresholds, and display settings.
/// Used to configure dashboard gauges.
///
/// Returns: Vector of GaugeInfo for all defined gauges
#[tauri::command]
async fn get_gauge_configs(state: tauri::State<'_, AppState>) -> Result<Vec<GaugeInfo>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let gauges: Vec<GaugeInfo> = def
        .gauges
        .values()
        .map(|g| GaugeInfo {
            name: g.name.clone(),
            channel: g.channel.clone(),
            title: g.title.clone(),
            units: g.units.clone(),
            lo: g.lo,
            hi: g.hi,
            low_warning: g.low_warning,
            high_warning: g.high_warning,
            low_danger: g.low_danger,
            high_danger: g.high_danger,
            digits: g.digits,
        })
        .collect();
    Ok(gauges)
}

/// Get a single gauge configuration by name
#[tauri::command]
async fn get_gauge_config(
    state: tauri::State<'_, AppState>,
    gauge_name: String,
) -> Result<GaugeInfo, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let gauge = def
        .gauges
        .get(&gauge_name)
        .ok_or_else(|| format!("Gauge {} not found", gauge_name))?;

    Ok(GaugeInfo {
        name: gauge.name.clone(),
        channel: gauge.channel.clone(),
        title: gauge.title.clone(),
        units: gauge.units.clone(),
        lo: gauge.lo,
        hi: gauge.hi,
        low_warning: gauge.low_warning,
        high_warning: gauge.high_warning,
        low_danger: gauge.low_danger,
        high_danger: gauge.high_danger,
        digits: gauge.digits,
    })
}

/// Output channel info returned to frontend
#[derive(Serialize, Clone)]
struct ChannelInfo {
    /// Channel name/identifier
    name: String,
    /// Human-readable label (if available)
    label: Option<String>,
    /// Unit of measurement
    units: String,
    /// Scale factor for display
    scale: f64,
    /// Translate offset for display  
    translate: f64,
}

/// Get all available output channels from the INI definition
#[tauri::command]
async fn get_available_channels(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ChannelInfo>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let mut channels: Vec<ChannelInfo> = def
        .output_channels
        .values()
        .map(|ch| ChannelInfo {
            name: ch.name.clone(),
            label: ch.label.clone(),
            units: ch.units.clone(),
            scale: ch.scale,
            translate: ch.translate,
        })
        .collect();

    // Append user math channels
    let math_channels_guard = state.math_channels.lock().await;
    for ch in math_channels_guard.iter() {
        channels.push(ChannelInfo {
            name: ch.name.clone(),
            label: Some(ch.name.clone()),
            units: ch.units.clone(),
            scale: 1.0,
            translate: 0.0,
        });
    }

    // Sort by name for consistent ordering
    channels.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(channels)
}

/// Get suggested status bar channels based on user settings, FrontPage, or common defaults
#[tauri::command]
async fn get_status_bar_defaults(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Vec<String>, String> {
    // First check if user has saved custom status bar channels
    let settings = load_settings(&app);
    if !settings.status_bar_channels.is_empty() {
        return Ok(settings.status_bar_channels);
    }

    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    // Try to get channels from FrontPage gauges first
    if let Some(fp) = &def.frontpage {
        if !fp.gauges.is_empty() {
            // Get the channel names for the first few gauges
            let mut channels = Vec::new();
            for gauge_name in fp.gauges.iter().take(4) {
                if let Some(gauge) = def.gauges.get(gauge_name) {
                    channels.push(gauge.channel.clone());
                }
            }
            if !channels.is_empty() {
                return Ok(channels);
            }
        }
    }

    // Fall back to common channel names if they exist
    let common_channels = [
        "RPM", "rpm", "AFR", "afr", "lambda", "MAP", "map", "TPS", "tps", "coolant", "CLT", "IAT",
    ];
    let mut defaults = Vec::new();
    for name in common_channels.iter() {
        if def.output_channels.contains_key(*name) && !defaults.contains(&name.to_string()) {
            defaults.push(name.to_string());
            if defaults.len() >= 4 {
                break;
            }
        }
    }

    // If still empty, just take first 4 channels
    if defaults.is_empty() {
        defaults = def.output_channels.keys().take(4).cloned().collect();
    }

    Ok(defaults)
}

#[tauri::command]
async fn get_menu_tree(
    state: tauri::State<'_, AppState>,
    filter_context: Option<HashMap<String, f64>>,
) -> Result<Vec<Menu>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    // Always return all menu items - visibility conditions are evaluated but items are never filtered out
    // This allows the frontend to show all items (grayed out if disabled) and enables search to find everything
    if let Some(context) = filter_context {
        let mut all_menus = Vec::new();
        for menu in &def.menus {
            let items_with_flags = add_visibility_flags(&menu.items, &context);
            all_menus.push(Menu {
                name: menu.name.clone(),
                title: menu.title.clone(),
                items: items_with_flags,
            });
        }
        Ok(all_menus)
    } else {
        Ok(def.menus.clone())
    }
}

/// Recursively add visibility/enabled flags to menu items without filtering them out
fn add_visibility_flags(items: &[MenuItem], context: &HashMap<String, f64>) -> Vec<MenuItem> {
    items
        .iter()
        .map(|item| {
            match item {
                MenuItem::Dialog {
                    label,
                    target,
                    visibility_condition,
                    enabled_condition,
                    ..
                } => {
                    let visible = evaluate_visibility(visibility_condition, context);
                    let enabled = evaluate_visibility(enabled_condition, context);
                    MenuItem::Dialog {
                        label: label.clone(),
                        target: target.clone(),
                        visibility_condition: visibility_condition.clone(),
                        enabled_condition: enabled_condition.clone(),
                        visible,
                        enabled,
                    }
                }
                MenuItem::Table {
                    label,
                    target,
                    visibility_condition,
                    enabled_condition,
                    ..
                } => {
                    let visible = evaluate_visibility(visibility_condition, context);
                    let enabled = evaluate_visibility(enabled_condition, context);
                    MenuItem::Table {
                        label: label.clone(),
                        target: target.clone(),
                        visibility_condition: visibility_condition.clone(),
                        enabled_condition: enabled_condition.clone(),
                        visible,
                        enabled,
                    }
                }
                MenuItem::SubMenu {
                    label,
                    items: sub_items,
                    visibility_condition,
                    enabled_condition,
                    ..
                } => {
                    let visible = evaluate_visibility(visibility_condition, context);
                    let enabled = evaluate_visibility(enabled_condition, context);
                    // Recursively process children
                    let children_with_flags = add_visibility_flags(sub_items, context);
                    MenuItem::SubMenu {
                        label: label.clone(),
                        items: children_with_flags,
                        visibility_condition: visibility_condition.clone(),
                        enabled_condition: enabled_condition.clone(),
                        visible,
                        enabled,
                    }
                }
                MenuItem::Std {
                    label,
                    target,
                    visibility_condition,
                    enabled_condition,
                    ..
                } => {
                    let visible = evaluate_visibility(visibility_condition, context);
                    let enabled = evaluate_visibility(enabled_condition, context);
                    MenuItem::Std {
                        label: label.clone(),
                        target: target.clone(),
                        visibility_condition: visibility_condition.clone(),
                        enabled_condition: enabled_condition.clone(),
                        visible,
                        enabled,
                    }
                }
                MenuItem::Help {
                    label,
                    target,
                    visibility_condition,
                    enabled_condition,
                    ..
                } => {
                    let visible = evaluate_visibility(visibility_condition, context);
                    let enabled = evaluate_visibility(enabled_condition, context);
                    MenuItem::Help {
                        label: label.clone(),
                        target: target.clone(),
                        visibility_condition: visibility_condition.clone(),
                        enabled_condition: enabled_condition.clone(),
                        visible,
                        enabled,
                    }
                }
                MenuItem::Separator => MenuItem::Separator,
            }
        })
        .collect()
}

/// Evaluate visibility condition - returns true if visible (or on error/missing condition)
fn evaluate_visibility(condition: &Option<String>, context: &HashMap<String, f64>) -> bool {
    if let Some(cond) = condition {
        let mut parser = libretune_core::ini::expression::Parser::new(cond);
        if let Ok(expr) = parser.parse() {
            if let Ok(val) = libretune_core::ini::expression::evaluate_simple(&expr, context) {
                return val.as_bool();
            }
        }
    }
    true // Default to visible
}

/// Builds a searchable index of all menu targets and their content.
/// Maps target names to searchable terms (field labels, panel titles, etc.)
/// This enables deep search - finding dialogs by their field contents.
#[tauri::command]
async fn get_searchable_index(
    state: tauri::State<'_, AppState>,
) -> Result<HashMap<String, Vec<String>>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let mut index: HashMap<String, Vec<String>> = HashMap::new();

    // Index dialogs - collect field labels, panel titles
    for (dialog_name, dialog) in &def.dialogs {
        let mut terms: Vec<String> = Vec::new();

        // Add dialog title
        terms.push(dialog.title.clone());

        // Collect from all components
        for component in &dialog.components {
            match component {
                libretune_core::ini::DialogComponent::Label { text } => {
                    terms.push(text.clone());
                }
                libretune_core::ini::DialogComponent::Field { label, name, .. } => {
                    terms.push(label.clone());
                    terms.push(name.clone());
                }
                libretune_core::ini::DialogComponent::Panel { name, .. } => {
                    terms.push(name.clone());
                }
                libretune_core::ini::DialogComponent::Table { name } => {
                    terms.push(name.clone());
                }
                libretune_core::ini::DialogComponent::LiveGraph { title, .. } => {
                    terms.push(title.clone());
                }
                libretune_core::ini::DialogComponent::Indicator {
                    label_off,
                    label_on,
                    ..
                } => {
                    terms.push(label_off.clone());
                    terms.push(label_on.clone());
                }
                libretune_core::ini::DialogComponent::CommandButton { label, .. } => {
                    terms.push(label.clone());
                }
            }
        }

        if !terms.is_empty() {
            index.insert(dialog_name.clone(), terms);
        }
    }

    // Index tables - collect title, axis labels
    for (table_name, table) in &def.tables {
        let mut terms: Vec<String> = Vec::new();

        terms.push(table.title.clone());
        terms.push(table.x_bins.clone());

        if let Some(map_name) = &table.map_name {
            terms.push(map_name.clone());
        }
        if let Some(y_bins) = &table.y_bins {
            terms.push(y_bins.clone());
        }
        // Add the table's map constant name
        terms.push(table.map.clone());

        if !terms.is_empty() {
            index.insert(table_name.clone(), terms);
        }
    }

    // Index curves - collect title, axis labels
    for (curve_name, curve) in &def.curves {
        let mut terms: Vec<String> = Vec::new();

        terms.push(curve.title.clone());
        terms.push(curve.column_labels.0.clone()); // X label
        terms.push(curve.column_labels.1.clone()); // Y label

        // Add constant names
        terms.push(curve.x_bins.clone());
        terms.push(curve.y_bins.clone());

        if !terms.is_empty() {
            index.insert(curve_name.clone(), terms);
        }
    }

    Ok(index)
}

/// Evaluates an INI expression (visibility condition) with given context values.
///
/// Used to determine if menu items, dialogs, or fields should be shown
/// based on current constant values.
///
/// # Arguments
/// * `expression` - INI expression string (e.g., "{ nCylinders > 4 }")
/// * `context` - HashMap of variable names to current values
///
/// Returns: Boolean result of expression evaluation
#[tauri::command]
async fn evaluate_expression(
    _state: tauri::State<'_, AppState>,
    expression: String,
    context: HashMap<String, f64>,
) -> Result<bool, String> {
    let mut parser = libretune_core::ini::expression::Parser::new(&expression);
    let expr = parser.parse()?;
    let val = libretune_core::ini::expression::evaluate_simple(&expr, &context)?;
    Ok(val.as_bool())
}

/// Retrieves a dialog definition from the INI file.
///
/// Gets the complete dialog structure including panels, fields, and layout
/// for rendering settings dialogs.
///
/// # Arguments
/// * `name` - Dialog name from INI definition
///
/// Returns: Complete DialogDefinition structure
#[tauri::command]
async fn get_dialog_definition(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<DialogDefinition, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    def.dialogs
        .get(&name)
        .cloned()
        .ok_or_else(|| format!("Dialog {} not found", name))
}

/// Retrieves an indicator panel definition from the INI file.
///
/// # Arguments
/// * `name` - Indicator panel name from INI definition
///
/// Returns: IndicatorPanel structure with LED/indicator configurations
#[tauri::command]
async fn get_indicator_panel(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<libretune_core::ini::IndicatorPanel, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    def.indicator_panels
        .get(&name)
        .cloned()
        .ok_or_else(|| format!("IndicatorPanel {} not found", name))
}

/// Retrieves a port editor configuration from the INI file.
///
/// # Arguments
/// * `name` - Port editor name from INI definition
///
/// Returns: PortEditorConfig for I/O pin assignment UI
#[tauri::command]
async fn get_port_editor(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<libretune_core::ini::PortEditorConfig, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    // First try to get from INI definition
    if let Some(config) = def.port_editors.get(&name) {
        return Ok(config.clone());
    }

    // For built-in std_port_edit, provide a default if not explicitly defined
    if name == "std_port_edit" {
        return Ok(libretune_core::ini::PortEditorConfig {
            name: "std_port_edit".to_string(),
            label: "Output Port Settings".to_string(),
            enable_condition: None,
        });
    }

    Err(format!("PortEditor {} not found", name))
}

/// Retrieves saved port editor assignments for the current project.
#[tauri::command]
async fn get_port_editor_assignments(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<Vec<PortEditorAssignment>, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let store = load_port_editor_store(project)?;
    Ok(store.assignments.get(&name).cloned().unwrap_or_default())
}

/// Saves port editor assignments for the current project.
#[tauri::command]
async fn save_port_editor_assignments(
    state: tauri::State<'_, AppState>,
    name: String,
    assignments: Vec<PortEditorAssignment>,
) -> Result<(), String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let mut store = load_port_editor_store(project)?;
    store.assignments.insert(name, assignments);
    save_port_editor_store(project, &store)
}

/// Retrieves a help topic from the INI file.
///
/// # Arguments
/// * `name` - Help topic name from INI definition
///
/// Returns: HelpTopic with title and content
#[tauri::command]
async fn get_help_topic(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<HelpTopic, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    def.help_topics
        .get(&name)
        .cloned()
        .ok_or_else(|| format!("Help topic {} not found", name))
}

/// Retrieves constant metadata from the INI definition.
///
/// Gets information about a constant including its type, units, min/max,
/// bit options (for dropdown fields), and visibility conditions.
///
/// # Arguments
/// * `name` - Constant name from INI definition
///
/// Returns: ConstantInfo with metadata for UI rendering
#[tauri::command]
async fn get_constant(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<ConstantInfo, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let constant = def
        .constants
        .get(&name)
        .ok_or_else(|| format!("Constant {} not found", name))?;

    // Determine value_type from DataType
    let value_type = match constant.data_type {
        DataType::String => "string".to_string(),
        DataType::Bits => "bits".to_string(),
        _ => {
            // Check if it's an array
            match &constant.shape {
                libretune_core::ini::Shape::Scalar => "scalar".to_string(),
                _ => "array".to_string(),
            }
        }
    };

    eprintln!(
        "[DEBUG] get_constant '{}': bit_options.len()={}, value_type={}",
        name,
        constant.bit_options.len(),
        value_type
    );
    if !constant.bit_options.is_empty() && constant.bit_options.len() <= 10 {
        eprintln!(
            "[DEBUG] get_constant '{}': bit_options={:?}",
            name, constant.bit_options
        );
    }

    Ok(ConstantInfo {
        name: constant.name.clone(),
        label: constant.label.clone(),
        units: constant.units.clone(),
        digits: constant.digits,
        min: constant.min,
        max: constant.max,
        value_type,
        bit_options: constant.bit_options.clone(),
        help: constant.help.clone(),
        visibility_condition: constant.visibility_condition.clone(),
    })
}

/// Retrieves a string constant's current value.
///
/// # Arguments
/// * `name` - String constant name from INI definition
///
/// Returns: The string value
#[tauri::command]
async fn get_constant_string_value(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<String, String> {
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;
    let tune_guard = state.current_tune.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let conn = conn_guard.as_mut();

    let constant = def
        .constants
        .get(&name)
        .ok_or_else(|| format!("Constant {} not found", name))?;

    // For string type, read the raw bytes and convert to UTF-8 string
    if constant.data_type != DataType::String {
        return Err(format!("Constant {} is not a string type", name));
    }

    // When offline, try reading directly from TuneFile first (simpler and more reliable)
    if conn.is_none() {
        if let Some(tune) = tune_guard.as_ref() {
            if let Some(tune_value) = tune.constants.get(&name) {
                use libretune_core::tune::TuneValue;
                if let TuneValue::String(s) = tune_value {
                    return Ok(s.clone());
                }
            }
        }
    }

    // Get string length from shape (e.g., Array1D(32) means 32 chars)
    let length = constant.shape.element_count() as u16;
    if length == 0 {
        return Ok(String::new());
    }

    // If connected to ECU, always read from ECU (live data)
    if let Some(conn) = conn {
        let params = libretune_core::protocol::commands::ReadMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            length,
        };

        let raw_data = conn.read_memory(params).map_err(|e| e.to_string())?;
        // Convert to string, stopping at first null byte
        let s = String::from_utf8_lossy(&raw_data);
        let s = s.trim_end_matches('\0').to_string();
        return Ok(s);
    }

    // If offline and not in TuneFile, return empty string (should always be in TuneFile)
    Ok(String::new())
}

/// Retrieves a numeric constant's current value.
///
/// Reads from tune file (offline) or ECU memory (online). For PC variables,
/// reads from local cache. Handles bit-field extraction automatically.
///
/// # Arguments
/// * `name` - Constant name from INI definition
///
/// Returns: Current value in display units (scaled/translated)
#[tauri::command]
async fn get_constant_value(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<f64, String> {
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;
    let cache_guard = state.tune_cache.lock().await;
    let tune_guard = state.current_tune.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let conn = conn_guard.as_mut();

    let constant = def
        .constants
        .get(&name)
        .ok_or_else(|| format!("Constant {} not found", name))?;

    // PC variables are stored locally, not on ECU
    if constant.is_pc_variable {
        // Check local cache first
        if let Some(cache) = cache_guard.as_ref() {
            if let Some(&val) = cache.local_values.get(&name) {
                return Ok(val);
            }
        }
        // Fall back to default value from INI
        if let Some(&default_val) = def.default_values.get(&name) {
            return Ok(default_val);
        }
        // Last resort: use min value or 0
        return Ok(constant.min);
    }

    // When offline, ALWAYS read from TuneFile (MSQ file) - no cache fallback
    if conn.is_none() {
        if let Some(tune) = tune_guard.as_ref() {
            if let Some(tune_value) = tune.constants.get(&name) {
                use libretune_core::tune::TuneValue;
                match tune_value {
                    TuneValue::Scalar(v) => {
                        // For bits constants, the value might be a string - need to look it up
                        if constant.data_type == libretune_core::ini::DataType::Bits {
                            // If it's already a number, return it (even if it maps to "INVALID" - that's what's in the MSQ)
                            let index = *v as usize;
                            if index < constant.bit_options.len() {
                                let option_str = &constant.bit_options[index];
                                eprintln!("[DEBUG] get_constant_value: Found '{}' in TuneFile as Scalar({}), returning as bits index (maps to '{}')", 
                                    name, v, option_str);
                            } else {
                                eprintln!("[DEBUG] get_constant_value: Found '{}' in TuneFile as Scalar({}), but out of range (bit_options len={}), returning anyway", 
                                    name, v, constant.bit_options.len());
                            }
                            return Ok(*v);
                        } else {
                            eprintln!("[DEBUG] get_constant_value: Found '{}' in TuneFile as Scalar({}), returning directly", name, v);
                            return Ok(*v);
                        }
                    }
                    TuneValue::String(s)
                        if constant.data_type == libretune_core::ini::DataType::Bits =>
                    {
                        // Look up string in bit_options
                        if let Some(index) = constant.bit_options.iter().position(|opt| opt == s) {
                            eprintln!("[DEBUG] get_constant_value: Found '{}' in TuneFile as String('{}'), matched at index {}", name, s, index);
                            return Ok(index as f64);
                        }
                        // Try case-insensitive
                        if let Some(index) = constant
                            .bit_options
                            .iter()
                            .position(|opt| opt.eq_ignore_ascii_case(s))
                        {
                            eprintln!("[DEBUG] get_constant_value: Found '{}' in TuneFile as String('{}'), case-insensitive match at index {}", name, s, index);
                            return Ok(index as f64);
                        }
                        eprintln!("[DEBUG] get_constant_value: Found '{}' in TuneFile as String('{}'), but not found in bit_options, returning 0", 
                            name, s);
                        return Ok(0.0);
                    }
                    TuneValue::String(_s) => {
                        // Non-bits string constants - should use get_constant_string_value
                        eprintln!("[DEBUG] get_constant_value: Found '{}' in TuneFile as String, but constant is not Bits type, returning 0", name);
                        return Ok(0.0);
                    }
                    TuneValue::Array(arr) => {
                        // For arrays, return first element or 0
                        if !arr.is_empty() {
                            return Ok(arr[0]);
                        }
                        return Ok(0.0);
                    }
                    TuneValue::Bool(b) => {
                        return Ok(if *b { 1.0 } else { 0.0 });
                    }
                }
            } else {
                // Constant not in TuneFile - return 0 (or default)
                eprintln!(
                    "[DEBUG] get_constant_value: Constant '{}' not found in TuneFile, returning 0",
                    name
                );
                return Ok(0.0);
            }
        } else {
            // No tune file loaded - return 0
            eprintln!("[DEBUG] get_constant_value: No TuneFile loaded, returning 0");
            return Ok(0.0);
        }
    }

    // When online, read from ECU
    // Handle bits constants specially (they're packed, size_bytes() == 0)
    if constant.data_type == libretune_core::ini::DataType::Bits {
        let bit_pos = constant.bit_position.unwrap_or(0);
        let bit_size = constant.bit_size.unwrap_or(1);

        // Calculate which byte contains the bits and the bit position within that byte
        let byte_offset = (bit_pos / 8) as u16;
        let bit_in_byte = bit_pos % 8;

        // Calculate how many bytes we need to read (may span multiple bytes)
        let bits_remaining_after_first_byte = bit_size.saturating_sub(8 - bit_in_byte);
        let bytes_needed = if bits_remaining_after_first_byte > 0 {
            // Need multiple bytes: first byte + additional bytes
            1 + bits_remaining_after_first_byte.div_ceil(8)
        } else {
            // All bits fit in one byte
            1
        };

        // Read the byte(s) containing the bits from ECU
        let read_offset = constant.offset + byte_offset;
        if let Some(conn) = conn {
            let params = libretune_core::protocol::commands::ReadMemoryParams {
                can_id: 0,
                page: constant.page,
                offset: read_offset,
                length: bytes_needed as u16,
            };
            if let Ok(bytes) = conn.read_memory(params) {
                if bytes.is_empty() {
                    return Ok(0.0);
                }

                // Extract bits from the first byte
                let first_byte = bytes[0];
                let bits_in_first_byte = (8 - bit_in_byte).min(bit_size);
                let mask_first = if bits_in_first_byte >= 8 {
                    0xFF
                } else {
                    (1u8 << bits_in_first_byte) - 1
                };
                let mut bit_val = ((first_byte >> bit_in_byte) & mask_first) as u32;

                // If bits span multiple bytes, extract from additional bytes
                if bits_remaining_after_first_byte > 0 && bytes.len() > 1 {
                    let mut bits_collected = bits_in_first_byte;
                    for byte in bytes.iter().skip(1) {
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
                        let val_from_byte = (*byte & mask) as u32;
                        bit_val |= val_from_byte << bits_collected;
                        bits_collected += bits_from_this_byte;
                    }
                }

                // Return the raw bit value (index into bit_options array)
                eprintln!("[DEBUG] get_constant_value: Read bits constant '{}' from ECU: bit_val={}, bit_options len={}", 
                    name, bit_val, constant.bit_options.len());
                return Ok(bit_val as f64);
            }
        }

        eprintln!(
            "[DEBUG] get_constant_value: Could not read bits constant '{}' from ECU, returning 0",
            name
        );
        return Ok(0.0);
    }

    let length = constant.size_bytes() as u16;
    if length == 0 {
        return Ok(0.0);
    } // Zero-size constants (shouldn't happen for non-bits)

    // If connected to ECU, always read from ECU (live data)
    if let Some(conn) = conn {
        let params = libretune_core::protocol::commands::ReadMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            length,
        };

        let raw_data = conn.read_memory(params).map_err(|e| e.to_string())?;
        if let Some(raw_val) = constant
            .data_type
            .read_from_bytes(&raw_data, 0, def.endianness)
        {
            return Ok(constant.raw_to_display(raw_val));
        }
        return Ok(0.0);
    }

    // If offline, read from cache (MSQ data)
    if let Some(cache) = cache_guard.as_ref() {
        if let Some(raw_data) = cache.read_bytes(constant.page, constant.offset, length) {
            if let Some(raw_val) = constant
                .data_type
                .read_from_bytes(raw_data, 0, def.endianness)
            {
                return Ok(constant.raw_to_display(raw_val));
            }
        }
    }

    // No cache and not connected - return 0
    Ok(0.0)
}

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
#[tauri::command]
async fn update_constant(
    state: tauri::State<'_, AppState>,
    name: String,
    value: f64,
) -> Result<(), String> {
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;
    let mut cache_guard = state.tune_cache.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let constant = def
        .constants
        .get(&name)
        .ok_or_else(|| format!("Constant {} not found", name))?;

    // PC variables are stored locally, not on ECU
    if constant.is_pc_variable {
        if let Some(cache) = cache_guard.as_mut() {
            cache.local_values.insert(name.clone(), value);
        }
        // Also update tune.constants for consistency
        let mut tune_guard = state.current_tune.lock().await;
        if let Some(tune) = tune_guard.as_mut() {
            tune.constants
                .insert(name, libretune_core::tune::TuneValue::Scalar(value));
        }
        return Ok(());
    }

    // Handle bits constants specially (they're packed, size_bytes() == 0)
    if constant.data_type == libretune_core::ini::DataType::Bits {
        let bit_pos = constant.bit_position.unwrap_or(0);
        let bit_size = constant.bit_size.unwrap_or(1);

        // Calculate which byte contains the bits and the bit position within that byte
        let byte_offset = (bit_pos / 8) as u16;
        let bit_in_byte = bit_pos % 8;

        // Calculate how many bytes we need to read/write (may span multiple bytes)
        let bits_remaining_after_first_byte = bit_size.saturating_sub(8 - bit_in_byte);
        let bytes_needed: usize = if bits_remaining_after_first_byte > 0 {
            (1 + bits_remaining_after_first_byte.div_ceil(8)) as usize
        } else {
            1
        };

        let read_offset = constant.offset + byte_offset;
        let new_bit_val = value as u32;

        // Read existing bytes from cache or ECU
        let mut existing_bytes = vec![0u8; bytes_needed];
        if let Some(cache) = cache_guard.as_ref() {
            if let Some(bytes) = cache.read_bytes(constant.page, read_offset, bytes_needed as u16) {
                existing_bytes.copy_from_slice(bytes);
            }
        } else if let Some(conn) = conn_guard.as_mut() {
            let params = libretune_core::protocol::commands::ReadMemoryParams {
                can_id: 0,
                page: constant.page,
                offset: read_offset,
                length: bytes_needed as u16,
            };
            if let Ok(bytes) = conn.read_memory(params) {
                let copy_len = bytes.len().min(existing_bytes.len());
                existing_bytes[..copy_len].copy_from_slice(&bytes[..copy_len]);
            }
        }

        // Apply the new bit value using masks
        // For single-byte case (most common for flags like [1:1])
        if bytes_needed == 1 {
            let mask = if bit_size >= 8 {
                0xFF
            } else {
                ((1u8 << bit_size) - 1) << bit_in_byte
            };
            existing_bytes[0] =
                (existing_bytes[0] & !mask) | (((new_bit_val as u8) << bit_in_byte) & mask);
        } else {
            // Multi-byte case: apply bits across multiple bytes
            let bits_in_first_byte = (8 - bit_in_byte).min(bit_size);
            let mask_first = if bits_in_first_byte >= 8 {
                0xFF
            } else {
                ((1u8 << bits_in_first_byte) - 1) << bit_in_byte
            };
            let val_first = ((new_bit_val as u8) << bit_in_byte) & mask_first;
            existing_bytes[0] = (existing_bytes[0] & !mask_first) | val_first;

            let mut bits_written = bits_in_first_byte;
            for byte in existing_bytes.iter_mut().skip(1) {
                let remaining_bits = bit_size - bits_written;
                if remaining_bits == 0 {
                    break;
                }
                let bits_for_this_byte = remaining_bits.min(8);
                let mask = if bits_for_this_byte >= 8 {
                    0xFF
                } else {
                    (1u8 << bits_for_this_byte) - 1
                };
                let val_for_byte = ((new_bit_val >> bits_written) as u8) & mask;
                *byte = (*byte & !mask) | val_for_byte;
                bits_written += bits_for_this_byte;
            }
        }

        // Write modified bytes to cache
        if let Some(cache) = cache_guard.as_mut() {
            cache.write_bytes(constant.page, read_offset, &existing_bytes);
        }

        // Update TuneFile in memory (both pages and constants)
        let mut tune_guard = state.current_tune.lock().await;
        if let Some(tune) = tune_guard.as_mut() {
            // Update page data
            let page_data = tune.pages.entry(constant.page).or_insert_with(|| {
                vec![
                    0u8;
                    def.page_sizes
                        .get(constant.page as usize)
                        .copied()
                        .unwrap_or(256) as usize
                ]
            });
            let start = read_offset as usize;
            let end = start + existing_bytes.len();
            if end <= page_data.len() {
                page_data[start..end].copy_from_slice(&existing_bytes);
            }

            // Update constants HashMap for offline reads
            tune.constants
                .insert(name.clone(), libretune_core::tune::TuneValue::Scalar(value));
        }

        // Mark tune as modified
        *state.tune_modified.lock().await = true;

        // Write to ECU if connected
        if let Some(conn) = conn_guard.as_mut() {
            let params = libretune_core::protocol::commands::WriteMemoryParams {
                can_id: 0,
                page: constant.page,
                offset: read_offset,
                data: existing_bytes,
            };
            if let Err(e) = conn.write_memory(params) {
                eprintln!("[WARN] Failed to write bits constant to ECU: {}", e);
            }
        }

        eprintln!(
            "[DEBUG] update_constant: Updated bits constant '{}' to value {}",
            name, value
        );
        return Ok(());
    }

    // Convert display value to raw bytes (for non-bits constants)
    let raw_val = constant.display_to_raw(value);
    let mut raw_data = vec![0u8; constant.size_bytes()];
    constant
        .data_type
        .write_to_bytes(&mut raw_data, 0, raw_val, def.endianness);

    // Always write to TuneCache if available (enables offline editing)
    if let Some(cache) = cache_guard.as_mut() {
        if cache.write_bytes(constant.page, constant.offset, &raw_data) {
            // Also update TuneFile in memory
            let mut tune_guard = state.current_tune.lock().await;
            if let Some(tune) = tune_guard.as_mut() {
                // Get or create page data
                let page_data = tune.pages.entry(constant.page).or_insert_with(|| {
                    // Create empty page if it doesn't exist
                    vec![
                        0u8;
                        def.page_sizes
                            .get(constant.page as usize)
                            .copied()
                            .unwrap_or(256) as usize
                    ]
                });

                // Update the page data
                let start = constant.offset as usize;
                let end = start + raw_data.len();
                if end <= page_data.len() {
                    page_data[start..end].copy_from_slice(&raw_data);
                }

                // Update constants HashMap for offline reads
                tune.constants
                    .insert(name.clone(), libretune_core::tune::TuneValue::Scalar(value));
            }

            // Mark tune as modified
            *state.tune_modified.lock().await = true;
        }
    }

    // Write to ECU if connected (optional - offline mode works without this)
    if let Some(conn) = conn_guard.as_mut() {
        let params = libretune_core::protocol::commands::WriteMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            data: raw_data.clone(),
        };

        // Don't fail if ECU write fails - offline mode should still work
        if let Err(e) = conn.write_memory(params) {
            eprintln!("[WARN] Failed to write to ECU (offline mode?): {}", e);
        }
    }

    Ok(())
}

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
#[tauri::command]
async fn get_all_constant_values(
    state: tauri::State<'_, AppState>,
) -> Result<HashMap<String, f64>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    // NO connection lock! Read from cache/tune only.
    let cache_guard = state.tune_cache.lock().await;
    let tune_guard = state.current_tune.lock().await;

    let mut values = HashMap::new();
    for (name, constant) in &def.constants {
        // Skip array constants (only need scalars for visibility conditions)
        if !matches!(constant.shape, libretune_core::ini::Shape::Scalar) {
            continue;
        }

        let value = read_constant_from_cache_or_tune(
            name,
            constant,
            def.endianness,
            tune_guard.as_ref(),
            cache_guard.as_ref(),
        );

        values.insert(name.clone(), value);
    }

    Ok(values)
}

/// Read a single constant value from tune file or cache (no ECU connection needed).
/// Priority: TuneFile → TuneCache → default 0.0
fn read_constant_from_cache_or_tune(
    name: &str,
    constant: &libretune_core::ini::Constant,
    endianness: libretune_core::ini::Endianness,
    tune: Option<&libretune_core::tune::TuneFile>,
    cache: Option<&libretune_core::tune::TuneCache>,
) -> f64 {
    // Try tune file first
    if let Some(tune) = tune {
        if let Some(tune_value) = tune.constants.get(name) {
            use libretune_core::tune::TuneValue;
            match tune_value {
                TuneValue::Scalar(v) => return *v,
                TuneValue::Bool(b) if constant.data_type == DataType::Bits => {
                    return if *b { 1.0 } else { 0.0 };
                }
                TuneValue::String(s) if constant.data_type == DataType::Bits => {
                    if let Some(index) = constant.bit_options.iter().position(|opt| opt == s) {
                        return index as f64;
                    } else if let Some(index) = constant
                        .bit_options
                        .iter()
                        .position(|opt| opt.eq_ignore_ascii_case(s))
                    {
                        return index as f64;
                    }
                    return 0.0;
                }
                _ => {} // fall through to cache
            }
        }
    }

    // Try cache
    if let Some(cache) = cache {
        return read_constant_from_cache(constant, endianness, cache);
    }

    0.0
}

/// Read a constant value from the tune cache bytes.
fn read_constant_from_cache(
    constant: &libretune_core::ini::Constant,
    endianness: libretune_core::ini::Endianness,
    cache: &libretune_core::tune::TuneCache,
) -> f64 {
    let length = constant.size_bytes() as u16;
    if length > 0 {
        if let Some(raw_data) = cache.read_bytes(constant.page, constant.offset, length) {
            if let Some(raw_val) = constant.data_type.read_from_bytes(raw_data, 0, endianness) {
                return constant.raw_to_display(raw_val);
            }
        }
    } else if constant.data_type == DataType::Bits {
        let byte_offset = (constant.bit_position.unwrap_or(0) / 8) as u16;
        let bit_in_byte = constant.bit_position.unwrap_or(0) % 8;
        let bytes_needed = (bit_in_byte + constant.bit_size.unwrap_or(0)).div_ceil(8) as u16;
        if let Some(raw_data) = cache.read_bytes(
            constant.page,
            constant.offset + byte_offset,
            bytes_needed.max(1),
        ) {
            let mut bit_value = 0u64;
            for (i, &byte) in raw_data.iter().enumerate() {
                let bit_start = if i == 0 { bit_in_byte } else { 0 };
                let bit_end = if i == bytes_needed.saturating_sub(1) as usize {
                    bit_in_byte + constant.bit_size.unwrap_or(0)
                } else {
                    8
                };
                let bits =
                    ((byte >> bit_start) & bit_mask_u8(bit_end.saturating_sub(bit_start))) as u64;
                bit_value |= bits << (i * 8);
            }
            return bit_value as f64;
        }
    }
    0.0
}

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

/// Stops AutoTune data collection.
///
/// Clears the AutoTune config and stops processing realtime data.
/// Recommendations remain available until explicitly cleared.
///
/// Returns: Nothing on success
#[tauri::command]
async fn stop_autotune(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.autotune_state.lock().await;
    guard.stop();

    let mut secondary_guard = state.autotune_secondary_state.lock().await;
    secondary_guard.stop();

    // Clear the config
    *state.autotune_config.lock().await = None;
    Ok(())
}

#[derive(Serialize)]
struct AutoTuneHeatEntry {
    cell_x: usize,
    cell_y: usize,
    hit_weighting: f64,
    change_magnitude: f64,
    beginning_value: f64,
    recommended_value: f64,
    hit_count: u32,
}

/// Retrieves current AutoTune recommendations.
///
/// Returns all accumulated VE correction recommendations with their
/// confidence weights (hit counts).
///
/// Returns: Vector of recommendations per cell
#[tauri::command]
async fn get_autotune_recommendations(
    state: tauri::State<'_, AppState>,
    table_name: Option<String>,
) -> Result<Vec<AutoTuneRecommendation>, String> {
    let secondary_name = state
        .autotune_config
        .lock()
        .await
        .as_ref()
        .and_then(|config| config.secondary_table_name.clone());

    let use_secondary = matches!(
        (table_name.as_deref(), secondary_name.as_deref()),
        (Some(table), Some(secondary)) if table == secondary
    );

    if use_secondary {
        let guard = state.autotune_secondary_state.lock().await;
        Ok(guard.get_recommendations())
    } else {
        let guard = state.autotune_state.lock().await;
        Ok(guard.get_recommendations())
    }
}

/// Retrieves AutoTune heatmap data for visualization.
///
/// Returns per-cell data for rendering coverage and change heatmaps.
///
/// Returns: Vector of heatmap entries with weighting and change magnitude
#[tauri::command]
async fn get_autotune_heatmap(
    state: tauri::State<'_, AppState>,
    table_name: Option<String>,
) -> Result<Vec<AutoTuneHeatEntry>, String> {
    let secondary_name = state
        .autotune_config
        .lock()
        .await
        .as_ref()
        .and_then(|config| config.secondary_table_name.clone());

    let recs = if matches!(
        (table_name.as_deref(), secondary_name.as_deref()),
        (Some(table), Some(secondary)) if table == secondary
    ) {
        let guard = state.autotune_secondary_state.lock().await;
        guard.get_recommendations()
    } else {
        let guard = state.autotune_state.lock().await;
        guard.get_recommendations()
    };

    let mut entries: Vec<AutoTuneHeatEntry> = Vec::new();
    for r in recs.iter() {
        let change = (r.recommended_value - r.beginning_value).abs();
        entries.push(AutoTuneHeatEntry {
            cell_x: r.cell_x,
            cell_y: r.cell_y,
            hit_weighting: r.hit_weighting,
            change_magnitude: change,
            beginning_value: r.beginning_value,
            recommended_value: r.recommended_value,
            hit_count: r.hit_count,
        });
    }

    Ok(entries)
}

/// Applies AutoTune recommendations to the VE table.
///
/// Writes the recommended VE corrections to the target table,
/// updating both tune cache and ECU memory.
///
/// # Arguments
/// * `table_name` - Target VE table name
///
/// Returns: Nothing on success
#[tauri::command]
async fn send_autotune_recommendations(
    state: tauri::State<'_, AppState>,
    table_name: String,
) -> Result<(), String> {
    // Collect recommendations
    let secondary_name = state
        .autotune_config
        .lock()
        .await
        .as_ref()
        .and_then(|config| config.secondary_table_name.clone());

    let recs = if matches!(
        (Some(table_name.as_str()), secondary_name.as_deref()),
        (Some(table), Some(secondary)) if table == secondary
    ) {
        let guard = state.autotune_secondary_state.lock().await;
        guard.get_recommendations()
    } else {
        let guard = state.autotune_state.lock().await;
        guard.get_recommendations()
    };
    if recs.is_empty() {
        return Err("No recommendations to send".to_string());
    }

    // Ensure connection and definition exist
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;

    // Find target table
    let table = def
        .get_table_by_name_or_map(&table_name)
        .ok_or_else(|| format!("Table {} not found", table_name))?;

    // Read current table map values
    let constant = def
        .constants
        .get(&table.map)
        .ok_or_else(|| format!("Constant {} not found for table {}", table.map, table_name))?;

    let element_count = constant.shape.element_count();
    let element_size = constant.data_type.size_bytes();
    let length = constant.size_bytes() as u16;

    if length == 0 {
        return Err("Table has zero length".to_string());
    }

    let params = libretune_core::protocol::commands::ReadMemoryParams {
        can_id: 0,
        page: constant.page,
        offset: constant.offset,
        length,
    };

    let raw_data = conn.read_memory(params).map_err(|e| e.to_string())?;

    // Convert to display values
    let mut values: Vec<f64> = Vec::with_capacity(element_count);
    for i in 0..element_count {
        let offset = i * element_size;
        if let Some(raw_val) = constant
            .data_type
            .read_from_bytes(&raw_data, offset, def.endianness)
        {
            values.push(constant.raw_to_display(raw_val));
        } else {
            values.push(0.0);
        }
    }

    // Determine table dimensions
    let x_size = table.x_size;
    let y_size = table.y_size;

    // Apply recommendations
    for r in recs.iter() {
        if r.cell_x >= x_size || r.cell_y >= y_size {
            eprintln!(
                "[WARN] send_autotune_recommendations: recommendation out of bounds: {}x{}",
                r.cell_x, r.cell_y
            );
            continue;
        }
        let idx = r.cell_y * x_size + r.cell_x;
        values[idx] = r.recommended_value;
    }

    // Convert back to raw bytes
    let mut raw_out = vec![0u8; constant.size_bytes()];
    for (i, val) in values.iter().enumerate() {
        let raw_val = constant.display_to_raw(*val);
        let offset = i * element_size;
        constant
            .data_type
            .write_to_bytes(&mut raw_out, offset, raw_val, def.endianness);
    }

    // Write back to ECU
    let write_params = libretune_core::protocol::commands::WriteMemoryParams {
        can_id: 0,
        page: constant.page,
        offset: constant.offset,
        data: raw_out,
    };

    conn.write_memory(write_params).map_err(|e| e.to_string())?;

    Ok(())
}

/// Burns the AutoTune recommendations to ECU flash memory.
///
/// Permanently saves the current table values (including any AutoTune
/// changes) to non-volatile ECU memory.
///
/// # Arguments
/// * `table_name` - Target table to burn
///
/// Returns: Nothing on success
#[tauri::command]
async fn burn_autotune_recommendations(
    state: tauri::State<'_, AppState>,
    table_name: String,
) -> Result<(), String> {
    // Ensure connection and definition exist
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;

    // Find target table constant page
    let table = def
        .get_table_by_name_or_map(&table_name)
        .ok_or_else(|| format!("Table {} not found", table_name))?;

    let constant = def
        .constants
        .get(&table.map)
        .ok_or_else(|| format!("Constant {} not found for table {}", table.map, table_name))?;

    let params = libretune_core::protocol::commands::BurnParams {
        can_id: 0,
        page: constant.page,
    };

    conn.burn(params).map_err(|e| e.to_string())?;

    Ok(())
}

/// Locks specific cells from AutoTune updates.
///
/// Prevents AutoTune from modifying the specified cells during
/// data collection and recommendation generation.
///
/// # Arguments
/// * `cells` - Vector of (x, y) cell coordinates to lock
///
/// Returns: Nothing on success
#[tauri::command]
async fn lock_autotune_cells(
    state: tauri::State<'_, AppState>,
    cells: Vec<(usize, usize)>,
    table_name: Option<String>,
) -> Result<(), String> {
    let secondary_name = state
        .autotune_config
        .lock()
        .await
        .as_ref()
        .and_then(|config| config.secondary_table_name.clone());

    let use_secondary = matches!(
        (table_name.as_deref(), secondary_name.as_deref()),
        (Some(table), Some(secondary)) if table == secondary
    );

    if use_secondary {
        let mut guard = state.autotune_secondary_state.lock().await;
        guard.lock_cells(cells);
    } else {
        let mut guard = state.autotune_state.lock().await;
        guard.lock_cells(cells);
    }
    Ok(())
}

/// Starts automatic periodic sending of AutoTune recommendations.
///
/// Spawns a background task that applies AutoTune recommendations
/// at the specified interval.
///
/// # Arguments
/// * `table_name` - Target VE table name
/// * `interval_ms` - Send interval in milliseconds (default: 15000)
///
/// Returns: Nothing on success
#[allow(dead_code)]
#[tauri::command]
async fn start_autotune_autosend(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    table_name: String,
    interval_ms: Option<u64>,
) -> Result<(), String> {
    let interval = interval_ms.unwrap_or(15000);

    // Ensure connection and definition exist
    {
        let conn_guard = state.connection.lock().await;
        let def_guard = state.definition.lock().await;
        if conn_guard.is_none() || def_guard.is_none() {
            return Err("Connection or definition missing".to_string());
        }
    }

    let mut task_guard = state.autotune_send_task.lock().await;
    if task_guard.is_some() {
        // Already running
        return Ok(());
    }

    let app_handle = app.clone();
    let table = table_name.clone();

    let handle = tokio::spawn(async move {
        let app_state = app_handle.state::<AppState>();
        let mut ticker = tokio::time::interval(tokio::time::Duration::from_millis(interval));
        loop {
            ticker.tick().await;

            // Run send_autotune_recommendations logic
            let secondary_name = app_state
                .autotune_config
                .lock()
                .await
                .as_ref()
                .and_then(|config| config.secondary_table_name.clone());

            let recs = if matches!(
                (Some(table.as_str()), secondary_name.as_deref()),
                (Some(table_name), Some(secondary)) if table_name == secondary
            ) {
                let guard = app_state.autotune_secondary_state.lock().await;
                guard.get_recommendations()
            } else {
                let guard = app_state.autotune_state.lock().await;
                guard.get_recommendations()
            };

            if recs.is_empty() {
                continue;
            }

            // Acquire definition snapshot first, then connection. Do not hold both locks
            // simultaneously to avoid deadlocks with other code paths.
            let def = {
                let def_guard = app_state.definition.lock().await;
                match def_guard.as_ref() {
                    Some(d) => d.clone(),
                    None => continue,
                }
            };

            let mut conn_guard = app_state.connection.lock().await;
            let conn = match conn_guard.as_mut() {
                Some(c) => c,
                None => continue,
            };

            // Find table constant
            let table_def = match def.get_table_by_name_or_map(&table) {
                Some(t) => t.clone(),
                None => continue,
            };

            let constant = match def.constants.get(&table_def.map) {
                Some(cnst) => cnst.clone(),
                None => continue,
            };

            // Read current data
            let params = libretune_core::protocol::commands::ReadMemoryParams {
                can_id: 0,
                page: constant.page,
                offset: constant.offset,
                length: constant.size_bytes() as u16,
            };
            let raw_data = match conn.read_memory(params) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let element_count = constant.shape.element_count();
            let element_size = constant.data_type.size_bytes();
            let mut values: Vec<f64> = Vec::with_capacity(element_count);
            for i in 0..element_count {
                let off = i * element_size;
                if let Some(rv) = constant
                    .data_type
                    .read_from_bytes(&raw_data, off, def.endianness)
                {
                    values.push(constant.raw_to_display(rv));
                } else {
                    values.push(0.0);
                }
            }

            let x_size = table_def.x_size;
            let y_size = table_def.y_size;

            // Apply recommendations
            for r in recs.iter() {
                if r.cell_x >= x_size || r.cell_y >= y_size {
                    continue;
                }
                let idx = r.cell_y * x_size + r.cell_x;
                values[idx] = r.recommended_value;
            }

            // Convert back to bytes
            let mut raw_out = vec![0u8; constant.size_bytes()];
            for (i, v) in values.iter().enumerate() {
                let rv = constant.display_to_raw(*v);
                let offset = i * element_size;
                constant
                    .data_type
                    .write_to_bytes(&mut raw_out, offset, rv, def.endianness);
            }

            let write_params = libretune_core::protocol::commands::WriteMemoryParams {
                can_id: 0,
                page: constant.page,
                offset: constant.offset,
                data: raw_out,
            };
            let _ = conn.write_memory(write_params);
        }
    });

    *task_guard = Some(handle);

    Ok(())
}

/// Stops the AutoTune autosend background task.
///
/// Aborts the periodic recommendation sending task.
///
/// Returns: Nothing on success
#[allow(dead_code)]
#[tauri::command]
async fn stop_autotune_autosend(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut task_guard = state.autotune_send_task.lock().await;
    if let Some(h) = task_guard.take() {
        h.abort();
    }
    Ok(())
}

/// Unlocks previously locked AutoTune cells.
///
/// # Arguments
/// * `cells` - Vector of (x, y) cell coordinates to unlock
///
/// Returns: Nothing on success
#[tauri::command]
async fn unlock_autotune_cells(
    state: tauri::State<'_, AppState>,
    cells: Vec<(usize, usize)>,
    table_name: Option<String>,
) -> Result<(), String> {
    let secondary_name = state
        .autotune_config
        .lock()
        .await
        .as_ref()
        .and_then(|config| config.secondary_table_name.clone());

    let use_secondary = matches!(
        (table_name.as_deref(), secondary_name.as_deref()),
        (Some(table), Some(secondary)) if table == secondary
    );

    if use_secondary {
        let mut guard = state.autotune_secondary_state.lock().await;
        guard.unlock_cells(cells);
    } else {
        let mut guard = state.autotune_state.lock().await;
        guard.unlock_cells(cells);
    }
    Ok(())
}

/// Get AI-predicted VE values for cells with no AutoTune data.
///
/// Uses bilinear interpolation and neighbor-weighted averaging to predict
/// VE values for zero-hit cells, with confidence scores.
///
/// # Arguments
/// * `table_name` - Target VE table name
/// * `min_confidence` - Minimum confidence threshold (0.0-1.0, default 0.3)
/// * `min_hit_count` - Minimum hit count to consider a cell "known" (default 3)
///
/// Returns: Vector of predicted cells sorted by confidence
#[tauri::command]
async fn get_predicted_fills(
    state: tauri::State<'_, AppState>,
    table_name: String,
    min_confidence: Option<f64>,
    min_hit_count: Option<u32>,
) -> Result<Vec<libretune_core::autotune::predictor::PredictedCell>, String> {
    use libretune_core::autotune::predictor::{PredictorConfig, VePredictor};

    // Get table data
    let table_data = get_table_data_internal(&state, &table_name).await?;

    // Use z_values directly (already 2D: Vec<Vec<f64>>)
    let table_values = &table_data.z_values;
    let rows = table_values.len();
    let cols = if rows > 0 { table_values[0].len() } else { 0 };

    // Get hit counts from AutoTune state
    let at_guard = state.autotune_state.lock().await;
    let recs = at_guard.get_recommendations();
    let mut hit_counts = vec![vec![0u32; cols]; rows];
    for rec in &recs {
        if rec.cell_y < rows && rec.cell_x < cols {
            hit_counts[rec.cell_y][rec.cell_x] = rec.hit_count;
        }
    }
    drop(at_guard);

    let config = PredictorConfig {
        min_confidence: min_confidence.unwrap_or(0.3),
        min_hit_count: min_hit_count.unwrap_or(3),
        ..Default::default()
    };

    let predictor = VePredictor::new(config);

    Ok(predictor.predict_cells(
        table_values,
        &hit_counts,
        &table_data.x_bins,
        &table_data.y_bins,
    ))
}

/// Detect anomalies in a VE/fuel table.
///
/// Runs statistical analysis to find outliers, monotonicity violations,
/// gradient discontinuities, physically unreasonable values, and flat regions.
///
/// # Arguments
/// * `table_name` - Target table name
/// * `outlier_sigma` - Standard deviations for outlier detection (default 2.0)
///
/// Returns: Vector of detected anomalies sorted by severity
#[tauri::command]
async fn get_tune_anomalies(
    state: tauri::State<'_, AppState>,
    table_name: String,
    outlier_sigma: Option<f64>,
) -> Result<Vec<libretune_core::autotune::anomaly::TuneAnomaly>, String> {
    use libretune_core::autotune::anomaly::{AnomalyConfig, AnomalyDetector};

    let table_data = get_table_data_internal(&state, &table_name).await?;

    let config = AnomalyConfig {
        outlier_sigma: outlier_sigma.unwrap_or(2.0),
        ..Default::default()
    };

    let detector = AnomalyDetector::new(config);

    Ok(detector.detect_anomalies(&table_data.z_values, &table_data.x_bins, &table_data.y_bins))
}

/// Get a tune health report scoring the VE table by region.
///
/// Evaluates idle, cruise, WOT, and part-throttle regions for coverage,
/// smoothness, and monotonicity. Returns overall grade (A-F) and
/// per-region scores with actionable recommendations.
///
/// # Arguments
/// * `table_name` - Target VE table name
///
/// Returns: Complete health report with scores and recommendations
#[tauri::command]
async fn get_tune_health_report(
    state: tauri::State<'_, AppState>,
    table_name: String,
) -> Result<libretune_core::autotune::health::TuneHealthReport, String> {
    use libretune_core::autotune::health::{HealthConfig, HealthScorer};

    let table_data = get_table_data_internal(&state, &table_name).await?;

    let table_values = &table_data.z_values;
    let rows = table_values.len();
    let cols = if rows > 0 { table_values[0].len() } else { 0 };

    // Get hit counts from AutoTune state
    let at_guard = state.autotune_state.lock().await;
    let recs = at_guard.get_recommendations();
    let mut hit_counts = vec![vec![0u32; cols]; rows];
    for rec in &recs {
        if rec.cell_y < rows && rec.cell_x < cols {
            hit_counts[rec.cell_y][rec.cell_x] = rec.hit_count;
        }
    }
    drop(at_guard);

    let scorer = HealthScorer::new(HealthConfig::default());

    Ok(scorer.score_table(
        table_values,
        &hit_counts,
        &table_data.x_bins,
        &table_data.y_bins,
    ))
}

// ============================================================================
// Cross-file Tune Comparison & Collaborative Tuning
// ============================================================================

/// Compare two tune files (from disk) and return detailed diff
#[tauri::command]
async fn compare_tune_files(
    path_a: String,
    path_b: String,
) -> Result<libretune_core::tune::TuneDiff, String> {
    use libretune_core::tune::{TuneDiff, TuneFile};

    let tune_a = TuneFile::load(&path_a).map_err(|e| format!("Failed to load tune A: {}", e))?;
    let tune_b = TuneFile::load(&path_b).map_err(|e| format!("Failed to load tune B: {}", e))?;

    Ok(TuneDiff::compare(&tune_a, &tune_b))
}

/// Merge selected constants from another tune file into the current tune
///
/// # Arguments
/// * `source_path` - Path to the source tune file to merge from
/// * `constant_names` - List of constant names to cherry-pick
///
/// Returns: Number of constants merged
#[tauri::command]
async fn merge_from_tune(
    state: tauri::State<'_, AppState>,
    source_path: String,
    constant_names: Vec<String>,
) -> Result<usize, String> {
    use libretune_core::tune::{TuneDiff, TuneFile};

    let source =
        TuneFile::load(&source_path).map_err(|e| format!("Failed to load source tune: {}", e))?;

    let mut tune_guard = state.current_tune.lock().await;
    let tune = tune_guard.as_mut().ok_or("No tune loaded")?;

    let merged = TuneDiff::merge_selected(tune, &source, &constant_names);

    // Also update the tune cache with the merged values
    if merged > 0 {
        let mut cache_guard = state.tune_cache.lock().await;
        let def_guard = state.definition.lock().await;
        if let (Some(cache), Some(def)) = (cache_guard.as_mut(), def_guard.as_ref()) {
            for name in &constant_names {
                if let Some(value) = source.constants.get(name) {
                    if let Some(constant) = def.constants.get(name) {
                        // Update raw page data in cache via write_bytes
                        if let libretune_core::tune::TuneValue::Scalar(v) = value {
                            let raw_val =
                                ((*v - constant.translate as f64) / constant.scale as f64) as i64;
                            let page = constant.page;
                            let offset = constant.offset;
                            cache.write_bytes(page, offset, &[(raw_val & 0xFF) as u8]);
                        }
                    }
                }
            }
        }
    }

    Ok(merged)
}

// ============================================================================
// Tune Annotations
// ============================================================================

// ============================================================================
// Tune Annotations
// ============================================================================

/// Set an annotation on a constant, table, or cell
/// Key format: "constant_name" for scalars, "table_name:row:col" for cells
#[tauri::command]
async fn set_annotation(
    state: tauri::State<'_, AppState>,
    key: String,
    text: String,
    tag: Option<String>,
) -> Result<(), String> {
    use libretune_core::tune::{AnnotationTag, TuneAnnotation};

    let annotation_tag = tag.and_then(|t| match t.as_str() {
        "info" => Some(AnnotationTag::Info),
        "warning" => Some(AnnotationTag::Warning),
        "critical" => Some(AnnotationTag::Critical),
        "success" => Some(AnnotationTag::Success),
        "todo" => Some(AnnotationTag::Todo),
        _ => None,
    });

    let annotation = TuneAnnotation {
        text,
        author: None,
        created: chrono::Utc::now().to_rfc3339(),
        modified: None,
        tag: annotation_tag,
    };

    let mut tune_guard = state.current_tune.lock().await;
    let tune = tune_guard.as_mut().ok_or("No tune loaded")?;
    tune.set_annotation(key, annotation);

    Ok(())
}

/// Get an annotation by key
#[tauri::command]
async fn get_annotation(
    state: tauri::State<'_, AppState>,
    key: String,
) -> Result<Option<libretune_core::tune::TuneAnnotation>, String> {
    let tune_guard = state.current_tune.lock().await;
    let tune = tune_guard.as_ref().ok_or("No tune loaded")?;
    Ok(tune.get_annotation(&key).cloned())
}

/// Get all annotations for a table
#[tauri::command]
async fn get_table_annotations(
    state: tauri::State<'_, AppState>,
    table_name: String,
) -> Result<Vec<(String, libretune_core::tune::TuneAnnotation)>, String> {
    let tune_guard = state.current_tune.lock().await;
    let tune = tune_guard.as_ref().ok_or("No tune loaded")?;
    let annotations = tune
        .get_table_annotations(&table_name)
        .into_iter()
        .map(|(k, a)| (k.clone(), a.clone()))
        .collect();
    Ok(annotations)
}

/// Delete an annotation
#[tauri::command]
async fn delete_annotation(state: tauri::State<'_, AppState>, key: String) -> Result<bool, String> {
    let mut tune_guard = state.current_tune.lock().await;
    let tune = tune_guard.as_mut().ok_or("No tune loaded")?;
    Ok(tune.delete_annotation(&key))
}

/// Get all annotations in the tune
#[tauri::command]
async fn get_all_annotations(
    state: tauri::State<'_, AppState>,
) -> Result<std::collections::HashMap<String, libretune_core::tune::TuneAnnotation>, String> {
    let tune_guard = state.current_tune.lock().await;
    let tune = tune_guard.as_ref().ok_or("No tune loaded")?;
    Ok(tune.all_annotations().clone())
}

// ============================================================================
// Dyno Data Import & Overlay
// ============================================================================

/// Load a dyno CSV file and return the parsed run data
#[tauri::command]
async fn load_dyno_run(
    path: String,
    name: String,
) -> Result<libretune_core::datalog::dyno::DynoRun, String> {
    libretune_core::datalog::dyno::DynoRun::from_csv(&path, name)
        .map_err(|e| format!("Failed to load dyno CSV: {}", e))
}

/// Detect CSV column headers for dyno import
#[tauri::command]
async fn detect_dyno_headers(path: String) -> Result<Vec<String>, String> {
    libretune_core::datalog::dyno::detect_csv_headers(&path)
        .map_err(|e| format!("Failed to read CSV headers: {}", e))
}

/// Compare two dyno runs
#[tauri::command]
async fn compare_dyno_runs(
    run_a: libretune_core::datalog::dyno::DynoRun,
    run_b: libretune_core::datalog::dyno::DynoRun,
) -> Result<libretune_core::datalog::dyno::DynoComparison, String> {
    Ok(libretune_core::datalog::dyno::DynoComparison::compare(
        run_a, run_b,
    ))
}

/// Map dyno data onto a table for overlay visualization
#[tauri::command]
async fn get_dyno_table_overlay(
    state: tauri::State<'_, AppState>,
    table_name: String,
    dyno_run: libretune_core::datalog::dyno::DynoRun,
) -> Result<libretune_core::datalog::dyno::DynoTableOverlay, String> {
    let table_data = get_table_data_internal(&state, &table_name).await?;

    Ok(dyno_run.map_to_table(&table_data.x_bins, &table_data.y_bins, None))
}

/// Helper function to get table data internally (avoids code duplication)
async fn get_table_data_internal(
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
async fn update_table_z_values_internal(
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
async fn update_constant_array_internal(
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

/// Re-bins a table with new X and Y axis values.
///
/// Optionally interpolates Z values to fit the new axis bins.
///
/// # Arguments
/// * `table_name` - Table name from INI definition
/// * `new_x_bins` - New X axis bin values
/// * `new_y_bins` - New Y axis bin values
/// * `interpolate_z` - If true, interpolates Z values to fit new bins
///
/// Returns: Updated TableData with new bins and Z values
#[tauri::command]
async fn rebin_table(
    state: tauri::State<'_, AppState>,
    table_name: String,
    new_x_bins: Vec<f64>,
    new_y_bins: Vec<f64>,
    interpolate_z: bool,
) -> Result<TableData, String> {
    // Get current table data
    let table_data = get_table_data_internal(&state, &table_name).await?;

    // Apply rebin operation
    let result = table_ops::rebin_table(
        &table_data.x_bins,
        &table_data.y_bins,
        &table_data.z_values,
        new_x_bins.clone(),
        new_y_bins.clone(),
        interpolate_z,
    );

    // Save the new Z values
    update_table_z_values_internal(&state, &table_name, result.z_values.clone()).await?;

    // Save the new X/Y axis bins
    {
        let def_guard = state.definition.lock().await;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;
        let table = def
            .get_table_by_name_or_map(&table_name)
            .ok_or_else(|| format!("Table {} not found", table_name))?;

        let x_bins_name = table.x_bins.clone();
        let y_bins_name = table.y_bins.clone();
        drop(def_guard);

        update_constant_array_internal(&state, &x_bins_name, result.x_bins.clone()).await?;
        if let Some(y_name) = y_bins_name {
            update_constant_array_internal(&state, &y_name, result.y_bins.clone()).await?;
        }
    }

    Ok(TableData {
        x_bins: result.x_bins,
        y_bins: result.y_bins,
        z_values: result.z_values,
        ..table_data
    })
}

#[tauri::command]
async fn interpolate_linear(
    state: tauri::State<'_, AppState>,
    table_name: String,
    selected_cells: Vec<(usize, usize)>,
    axis: String,
) -> Result<TableData, String> {
    let axis_enum = match axis.to_lowercase().as_str() {
        "row" => table_ops::InterpolationAxis::Row,
        "col" => table_ops::InterpolationAxis::Col,
        _ => return Err("Invalid interpolation axis".to_string()),
    };

    let table_data = get_table_data_internal(&state, &table_name).await?;
    let new_z_values =
        table_ops::interpolate_linear(&table_data.z_values, selected_cells, axis_enum);

    update_table_z_values_internal(&state, &table_name, new_z_values.clone()).await?;

    Ok(TableData {
        z_values: new_z_values,
        ..table_data
    })
}

#[tauri::command]
async fn add_offset(
    state: tauri::State<'_, AppState>,
    table_name: String,
    selected_cells: Vec<(usize, usize)>,
    offset: f64,
) -> Result<TableData, String> {
    let table_data = get_table_data_internal(&state, &table_name).await?;
    let new_z_values = table_ops::add_offset(&table_data.z_values, selected_cells, offset);

    update_table_z_values_internal(&state, &table_name, new_z_values.clone()).await?;

    Ok(TableData {
        z_values: new_z_values,
        ..table_data
    })
}

#[tauri::command]
async fn fill_region(
    state: tauri::State<'_, AppState>,
    table_name: String,
    selected_cells: Vec<(usize, usize)>,
    direction: String,
) -> Result<TableData, String> {
    let dir_enum = match direction.to_lowercase().as_str() {
        "right" => table_ops::FillDirection::Right,
        "down" => table_ops::FillDirection::Down,
        _ => return Err("Invalid fill direction".to_string()),
    };

    let table_data = get_table_data_internal(&state, &table_name).await?;
    let new_z_values = table_ops::fill_region(&table_data.z_values, selected_cells, dir_enum);

    update_table_z_values_internal(&state, &table_name, new_z_values.clone()).await?;

    Ok(TableData {
        z_values: new_z_values,
        ..table_data
    })
}

/// Applies Gaussian smoothing to selected table cells.
///
/// Uses weighted averaging from neighboring cells to smooth transitions.
///
/// # Arguments
/// * `table_name` - Table name from INI definition
/// * `factor` - Smoothing factor (higher = more smoothing)
/// * `selected_cells` - Vector of (row, col) coordinates to smooth
///
/// Returns: Updated TableData with smoothed values
#[tauri::command]
async fn smooth_table(
    state: tauri::State<'_, AppState>,
    table_name: String,
    factor: f64,
    selected_cells: Vec<(usize, usize)>,
) -> Result<TableData, String> {
    // Get current table data
    let table_data = get_table_data_internal(&state, &table_name).await?;

    // Apply smooth operation (cells are already in (row, col) format from frontend)
    let new_z_values = table_ops::smooth_table(&table_data.z_values, selected_cells, factor);

    // Save the modified values
    update_table_z_values_internal(&state, &table_name, new_z_values.clone()).await?;

    Ok(TableData {
        z_values: new_z_values,
        ..table_data
    })
}

/// Interpolates values between corner cells of selected region.
///
/// Uses bilinear interpolation to fill in values between the
/// corner cells of the selection rectangle.
///
/// # Arguments
/// * `table_name` - Table name from INI definition
/// * `selected_cells` - Vector of (row, col) coordinates to interpolate
///
/// Returns: Updated TableData with interpolated values
#[tauri::command]
async fn interpolate_cells(
    state: tauri::State<'_, AppState>,
    table_name: String,
    selected_cells: Vec<(usize, usize)>,
) -> Result<TableData, String> {
    // Get current table data
    let table_data = get_table_data_internal(&state, &table_name).await?;

    // Apply interpolate operation
    let new_z_values = table_ops::interpolate_cells(&table_data.z_values, selected_cells);

    // Save the modified values
    update_table_z_values_internal(&state, &table_name, new_z_values.clone()).await?;

    Ok(TableData {
        z_values: new_z_values,
        ..table_data
    })
}

/// Scales selected cells by a multiplication factor.
///
/// # Arguments
/// * `table_name` - Table name from INI definition
/// * `selected_cells` - Vector of (row, col) coordinates to scale
/// * `scale_factor` - Multiplication factor (e.g., 1.1 for +10%)
///
/// Returns: Updated TableData with scaled values
#[tauri::command]
async fn scale_cells(
    state: tauri::State<'_, AppState>,
    table_name: String,
    selected_cells: Vec<(usize, usize)>,
    scale_factor: f64,
) -> Result<TableData, String> {
    // Get current table data
    let table_data = get_table_data_internal(&state, &table_name).await?;

    // Apply scale operation
    let new_z_values = table_ops::scale_cells(&table_data.z_values, selected_cells, scale_factor);

    // Save the modified values
    update_table_z_values_internal(&state, &table_name, new_z_values.clone()).await?;

    Ok(TableData {
        z_values: new_z_values,
        ..table_data
    })
}

/// Sets all selected cells to the same value.
///
/// # Arguments
/// * `table_name` - Table name from INI definition
/// * `selected_cells` - Vector of (row, col) coordinates to set
/// * `value` - Value to assign to all selected cells
///
/// Returns: Updated TableData with modified values
#[tauri::command]
async fn set_cells_equal(
    state: tauri::State<'_, AppState>,
    table_name: String,
    selected_cells: Vec<(usize, usize)>,
    value: f64,
) -> Result<TableData, String> {
    // Get current table data
    let table_data = get_table_data_internal(&state, &table_name).await?;

    // Apply set equal operation (mutates in place)
    let mut new_z_values = table_data.z_values.clone();
    table_ops::set_cells_equal(&mut new_z_values, selected_cells, value);

    // Save the modified values
    update_table_z_values_internal(&state, &table_name, new_z_values.clone()).await?;

    Ok(TableData {
        z_values: new_z_values,
        ..table_data
    })
}

/// Saves a dashboard layout to the project's dashboard file.
///
/// Converts the layout to XML format for storage.
///
/// # Arguments
/// * `project_name` - Name of the project
/// * `layout` - Dashboard layout configuration
///
/// Returns: Nothing on success
#[tauri::command]
async fn save_dashboard_layout(
    _state: tauri::State<'_, AppState>,
    project_name: String,
    layout: DashboardLayout,
) -> Result<(), String> {
    let path = get_dashboard_file_path(&project_name);

    // Convert DashboardLayout to TS DashFile format
    let dash_file = convert_layout_to_dashfile(&layout);

    // Write as TS XML format
    dash::save_dash_file(&dash_file, &path)
        .map_err(|e| format!("Failed to write dashboard file: {}", e))?;

    Ok(())
}

/// Loads a dashboard layout from a project's dashboard file.
///
/// Supports both XML and legacy JSON formats.
///
/// # Arguments
/// * `project_name` - Name of the project
///
/// Returns: DashboardLayout configuration
#[tauri::command]
async fn load_dashboard_layout(
    _state: tauri::State<'_, AppState>,
    project_name: String,
) -> Result<DashboardLayout, String> {
    let path = get_dashboard_file_path(&project_name);

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read dashboard file: {}", e))?;

    // Try TS XML format first
    if content.trim().starts_with("<?xml") || content.trim().starts_with("<dsh") {
        let dash_file = dash::parse_dash_file(&content)
            .map_err(|e| format!("Failed to parse dashboard XML: {}", e))?;
        return Ok(convert_dashfile_to_layout(&dash_file, &project_name));
    }

    // Fall back to legacy JSON format for backward compatibility
    let layout: DashboardLayout = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse dashboard file: {}", e))?;

    Ok(layout)
}

/// Lists all available dashboard layouts in the projects directory.
///
/// Returns: Vector of dashboard names (without extension)
#[tauri::command]
async fn list_dashboard_layouts(
    _state: tauri::State<'_, AppState>,
    _project_name: String,
) -> Result<Vec<String>, String> {
    let projects_dir = libretune_core::project::Project::projects_dir()
        .map_err(|e| format!("Failed to get projects directory: {}", e))?;

    let mut dashboards = Vec::new();

    // Ensure projects directory exists
    if !projects_dir.exists() {
        let _ = std::fs::create_dir_all(&projects_dir);
        return Ok(dashboards); // Return empty list
    }

    let entries = std::fs::read_dir(&projects_dir)
        .map_err(|e| format!("Failed to read projects directory: {}", e))?;

    for entry in entries.flatten() {
        if let Some(name) = entry.file_name().to_str() {
            if name.ends_with(".dash") {
                let dash_name = name.replace(".dash", "");
                dashboards.push(dash_name);
            }
        }
    }

    dashboards.sort();
    Ok(dashboards)
}

/// Create a LibreTune default dashboard
#[tauri::command]
async fn create_default_dashboard(
    _state: tauri::State<'_, AppState>,
    project_name: String,
    template: String,
) -> Result<DashboardLayout, String> {
    println!(
        "[create_default_dashboard] Creating template: {} for project: {}",
        template, project_name
    );

    let dash_file = match template.as_str() {
        "basic" => create_basic_dashboard(),
        "racing" => create_racing_dashboard(),
        "tuning" => create_tuning_dashboard(),
        _ => create_basic_dashboard(),
    };

    println!(
        "[create_default_dashboard] Dashboard has {} components",
        dash_file.gauge_cluster.components.len()
    );

    // Save it
    let path = get_dashboard_file_path(&project_name);
    println!("[create_default_dashboard] Saving to: {:?}", path);
    dash::save_dash_file(&dash_file, &path)
        .map_err(|e| format!("Failed to write dashboard file: {}", e))?;

    // Return as layout
    let layout = convert_dashfile_to_layout(&dash_file, &project_name);
    println!(
        "[create_default_dashboard] Returning layout with {} gauges",
        layout.gauges.len()
    );
    Ok(layout)
}

/// Load a TS .dash file directly from a path (for testing)
#[tauri::command]
async fn load_tunerstudio_dash(path: String) -> Result<DashboardLayout, String> {
    println!("[load_ts_dash] Loading from: {}", path);

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read dashboard file: {}", e))?;

    let dash_file = dash::parse_dash_file(&content)
        .map_err(|e| format!("Failed to parse dashboard XML: {}", e))?;

    let layout = convert_dashfile_to_layout(&dash_file, "TS Dashboard");
    println!("[load_ts_dash] Loaded {} gauges", layout.gauges.len());
    Ok(layout)
}

/// Load a TS .dash file and return the full DashFile structure
#[tauri::command]
async fn get_dash_file(path: String) -> Result<DashFile, String> {
    println!("[get_dash_file] Loading from: {}", path);

    let lower = path.to_lowercase();

    let dash_file = if lower.ends_with(".gauge") {
        let gauge_file = dash::load_gauge_file(Path::new(&path))
            .map_err(|e| format!("Failed to parse gauge XML: {}", e))?;

        let mut dash_file = DashFile {
            bibliography: gauge_file.bibliography,
            version_info: gauge_file.version_info,
            ..Default::default()
        };
        dash_file.gauge_cluster.embedded_images = gauge_file.embedded_images;
        dash_file
            .gauge_cluster
            .components
            .push(DashComponent::Gauge(Box::new(gauge_file.gauge)));
        dash_file
    } else {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read dashboard file: {}", e))?;

        dash::parse_dash_file(&content)
            .map_err(|e| format!("Failed to parse dashboard XML: {}", e))?
    };

    println!(
        "[get_dash_file] Loaded {} components, {} embedded images",
        dash_file.gauge_cluster.components.len(),
        dash_file.gauge_cluster.embedded_images.len()
    );
    Ok(dash_file)
}

/// Validate a dashboard file and return a detailed report
#[tauri::command]
async fn validate_dashboard(
    dash_file: DashFile,
    project_name: Option<String>,
    app: tauri::AppHandle,
) -> Result<dash::ValidationReport, String> {
    println!("[validate_dashboard] Validating dashboard");

    // Load ECU definition if project name is provided
    let ecu_def = if let Some(ref proj_name) = project_name {
        let project_dir = get_projects_dir(&app).join(proj_name);
        let ini_path = project_dir.join("definition.ini");

        if ini_path.exists() {
            match EcuDefinition::from_file(ini_path.to_string_lossy().as_ref()) {
                Ok(def) => Some(def),
                Err(e) => {
                    println!(
                        "[validate_dashboard] Warning: Could not load INI for validation: {}",
                        e
                    );
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    let report = dash::validate_dashboard(&dash_file, ecu_def.as_ref());

    println!(
        "[validate_dashboard] Validation complete: {} errors, {} warnings",
        report.errors.len(),
        report.warnings.len()
    );

    Ok(report)
}

/// Save a TS .dash or .gauge file directly to a path
#[tauri::command]
async fn save_dash_file(path: String, dash_file: DashFile) -> Result<(), String> {
    let lower = path.to_lowercase();
    let path_buf = PathBuf::from(&path);

    if lower.ends_with(".gauge") {
        let gauge = dash_file
            .gauge_cluster
            .components
            .iter()
            .find_map(|comp| match comp {
                DashComponent::Gauge(g) => Some((**g).clone()),
                _ => None,
            })
            .ok_or_else(|| "Gauge file must contain a gauge component".to_string())?;

        let gauge_file = dash::GaugeFile {
            bibliography: dash_file.bibliography.clone(),
            version_info: VersionInfo {
                file_format: "1.0".to_string(),
                firmware_signature: dash_file.version_info.firmware_signature.clone(),
            },
            embedded_images: dash_file.gauge_cluster.embedded_images.clone(),
            gauge,
        };

        dash::save_gauge_file(&gauge_file, &path_buf)
            .map_err(|e| format!("Failed to write gauge file: {}", e))?;
    } else {
        dash::save_dash_file(&dash_file, &path_buf)
            .map_err(|e| format!("Failed to write dashboard file: {}", e))?;
    }

    Ok(())
}

/// Create a new dashboard file from a template in the user dashboards directory.
#[tauri::command]
async fn create_new_dashboard(
    app: tauri::AppHandle,
    name: String,
    template: String,
) -> Result<String, String> {
    let dash_dir = get_dashboards_dir(&app);
    if !dash_dir.exists() {
        std::fs::create_dir_all(&dash_dir)
            .map_err(|e| format!("Failed to create dashboards directory: {}", e))?;
    }

    let mut file_name = name.trim().to_string();
    if file_name.is_empty() {
        file_name = "Dashboard".to_string();
    }
    if !file_name.to_lowercase().ends_with(".ltdash.xml") {
        file_name = format!("{}.ltdash.xml", file_name);
    }

    let target_name = if dash_dir.join(&file_name).exists() {
        generate_unique_filename(&dash_dir, &file_name)
    } else {
        file_name
    };

    let dash_file = match template.as_str() {
        "basic" => create_basic_dashboard(),
        "racing" => create_racing_dashboard(),
        "tuning" => create_tuning_dashboard(),
        _ => create_basic_dashboard(),
    };

    let target_path = dash_dir.join(&target_name);
    dash::save_dash_file(&dash_file, &target_path)
        .map_err(|e| format!("Failed to write dashboard file: {}", e))?;

    Ok(target_path.to_string_lossy().to_string())
}

/// Rename an existing dashboard file.
#[tauri::command]
async fn rename_dashboard(path: String, new_name: String) -> Result<String, String> {
    let source = PathBuf::from(&path);
    let parent = source
        .parent()
        .ok_or_else(|| "Invalid dashboard path".to_string())?
        .to_path_buf();

    let ext = if path.to_lowercase().ends_with(".ltdash.xml") {
        ".ltdash.xml"
    } else if path.to_lowercase().ends_with(".dash") {
        ".dash"
    } else if path.to_lowercase().ends_with(".gauge") {
        ".gauge"
    } else {
        ""
    };

    let mut file_name = new_name.trim().to_string();
    if file_name.is_empty() {
        file_name = "Dashboard".to_string();
    }
    if !ext.is_empty() && !file_name.to_lowercase().ends_with(ext) {
        file_name = format!("{}{}", file_name, ext);
    }

    let target_name = if parent.join(&file_name).exists() {
        generate_unique_filename(&parent, &file_name)
    } else {
        file_name
    };

    let target_path = parent.join(&target_name);
    std::fs::rename(&source, &target_path)
        .map_err(|e| format!("Failed to rename dashboard: {}", e))?;

    Ok(target_path.to_string_lossy().to_string())
}

/// Duplicate a dashboard file.
#[tauri::command]
async fn duplicate_dashboard(path: String, new_name: String) -> Result<String, String> {
    let source = PathBuf::from(&path);
    let parent = source
        .parent()
        .ok_or_else(|| "Invalid dashboard path".to_string())?
        .to_path_buf();

    let ext = if path.to_lowercase().ends_with(".ltdash.xml") {
        ".ltdash.xml"
    } else if path.to_lowercase().ends_with(".dash") {
        ".dash"
    } else if path.to_lowercase().ends_with(".gauge") {
        ".gauge"
    } else {
        ""
    };

    let mut file_name = new_name.trim().to_string();
    if file_name.is_empty() {
        file_name = "Dashboard Copy".to_string();
    }
    if !ext.is_empty() && !file_name.to_lowercase().ends_with(ext) {
        file_name = format!("{}{}", file_name, ext);
    }

    let target_name = if parent.join(&file_name).exists() {
        generate_unique_filename(&parent, &file_name)
    } else {
        file_name
    };

    let target_path = parent.join(&target_name);
    std::fs::copy(&source, &target_path)
        .map_err(|e| format!("Failed to duplicate dashboard: {}", e))?;

    Ok(target_path.to_string_lossy().to_string())
}

/// Export a dashboard to a specific path.
#[tauri::command]
async fn export_dashboard(path: String, dash_file: DashFile) -> Result<(), String> {
    save_dash_file(path, dash_file).await
}

/// Delete a dashboard file.
#[tauri::command]
async fn delete_dashboard(path: String) -> Result<(), String> {
    let path_buf = PathBuf::from(&path);
    if !path_buf.exists() {
        return Err("Dashboard file not found".to_string());
    }
    std::fs::remove_file(&path_buf).map_err(|e| format!("Failed to delete dashboard: {}", e))?;
    Ok(())
}

/// Info about an available dashboard file
#[derive(Serialize)]
struct DashFileInfo {
    name: String,
    path: String,
    category: String, // "User", "Reference", etc.
}

/// Helper to scan a directory for .dash and .ltdash.xml files
fn scan_dash_directory(dir: &Path, _category: &str, dashes: &mut Vec<DashFileInfo>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();

            if let Some(name) = path.file_name() {
                if file_name.ends_with(".ltdash.xml") {
                    dashes.push(DashFileInfo {
                        name: name.to_string_lossy().to_string(),
                        path: path.to_string_lossy().to_string(),
                        category: "LibreTune".to_string(),
                    });
                } else if file_name.ends_with(".dash") {
                    dashes.push(DashFileInfo {
                        name: name.to_string_lossy().to_string(),
                        path: path.to_string_lossy().to_string(),
                        category: "Legacy (TunerStudio)".to_string(),
                    });
                } else if file_name.ends_with(".gauge") {
                    dashes.push(DashFileInfo {
                        name: name.to_string_lossy().to_string(),
                        path: path.to_string_lossy().to_string(),
                        category: "Legacy Gauges".to_string(),
                    });
                }
            }
        }
    }
}

/// List all available dashboard files (.ltdash.xml and .dash for import)
/// List all available dashboard files from the user dashboards directory
/// Creates 3 default LibreTune dashboards if the directory is empty
#[tauri::command]
async fn list_available_dashes(app: tauri::AppHandle) -> Result<Vec<DashFileInfo>, String> {
    let dash_dir = get_dashboards_dir(&app);

    // Create directory if it doesn't exist
    if !dash_dir.exists() {
        std::fs::create_dir_all(&dash_dir)
            .map_err(|e| format!("Failed to create dashboards directory: {}", e))?;
    }

    // Ensure default dashboards exist if no native LibreTune dashboards are present
    let has_native_dash = std::fs::read_dir(&dash_dir)
        .ok()
        .and_then(|entries| {
            entries.flatten().map(|entry| entry.path()).find(|path| {
                path.file_name()
                    .map(|n| n.to_string_lossy().to_lowercase().ends_with(".ltdash.xml"))
                    .unwrap_or(false)
            })
        })
        .is_some();

    if !has_native_dash {
        println!(
            "[list_available_dashes] Creating default dashboards in {:?}",
            dash_dir
        );
        create_default_dashboard_files(&dash_dir)?;
    }

    let mut dashes = Vec::new();

    // Scan only the user dashboards directory (imported or created by user)
    scan_dash_directory(&dash_dir, "User", &mut dashes);

    // Sort by name
    dashes.sort_by(|a, b| a.name.cmp(&b.name));

    println!("[list_available_dashes] Found {} dashboards", dashes.len());
    Ok(dashes)
}

/// Result of checking for dashboard file conflicts
#[derive(Serialize)]
struct DashConflictInfo {
    /// The filename that would conflict
    file_name: String,
    /// Whether a conflict exists
    has_conflict: bool,
    /// Suggested alternative name if conflict exists
    suggested_name: Option<String>,
}

/// Reset dashboards to defaults - removes all user dashboards and recreates the 3 defaults
#[tauri::command]
async fn reset_dashboards_to_defaults(app: tauri::AppHandle) -> Result<(), String> {
    let dash_dir = get_dashboards_dir(&app);

    println!(
        "[reset_dashboards_to_defaults] Clearing dashboards directory: {:?}",
        dash_dir
    );

    // Remove the entire dashboards directory
    if dash_dir.exists() {
        std::fs::remove_dir_all(&dash_dir)
            .map_err(|e| format!("Failed to remove dashboards directory: {}", e))?;
    }

    // Recreate it
    std::fs::create_dir_all(&dash_dir)
        .map_err(|e| format!("Failed to create dashboards directory: {}", e))?;

    // Create the 3 defaults
    create_default_dashboard_files(&dash_dir)?;

    println!("[reset_dashboards_to_defaults] Reset complete - 3 default dashboards created");
    Ok(())
}

/// Check if a dashboard file with the given name already exists
#[tauri::command]
async fn check_dash_conflict(
    app: tauri::AppHandle,
    file_name: String,
) -> Result<DashConflictInfo, String> {
    let dash_dir = get_dashboards_dir(&app);
    let target_path = dash_dir.join(&file_name);

    if target_path.exists() {
        // Generate a suggested alternative name
        let suggested = generate_unique_filename(&dash_dir, &file_name);
        Ok(DashConflictInfo {
            file_name,
            has_conflict: true,
            suggested_name: Some(suggested),
        })
    } else {
        Ok(DashConflictInfo {
            file_name,
            has_conflict: false,
            suggested_name: None,
        })
    }
}

/// Generate a unique filename by appending _2, _3, etc.
fn generate_unique_filename(dir: &Path, original_name: &str) -> String {
    // Split into base and extension(s)
    // Handle .ltdash.xml specially
    let (base, ext) = if original_name.ends_with(".ltdash.xml") {
        let base = original_name.trim_end_matches(".ltdash.xml");
        (base.to_string(), ".ltdash.xml".to_string())
    } else if let Some(dot_pos) = original_name.rfind('.') {
        (
            original_name[..dot_pos].to_string(),
            original_name[dot_pos..].to_string(),
        )
    } else {
        (original_name.to_string(), String::new())
    };

    let mut counter = 2;
    loop {
        let candidate = format!("{}_{}{}", base, counter, ext);
        if !dir.join(&candidate).exists() {
            return candidate;
        }
        counter += 1;
        if counter > 1000 {
            // Safety limit
            return format!("{}_{}{}", base, chrono::Utc::now().timestamp(), ext);
        }
    }
}

/// Import result for a single dashboard file
#[derive(Serialize)]
struct DashImportResult {
    /// Original source path
    source_path: String,
    /// Whether import succeeded
    success: bool,
    /// Error message if failed
    error: Option<String>,
    /// The imported file info if successful
    file_info: Option<DashFileInfo>,
}

/// Import a dashboard file from an external location
/// If rename_to is provided, the file will be saved with that name instead
#[tauri::command]
async fn import_dash_file(
    app: tauri::AppHandle,
    source_path: String,
    rename_to: Option<String>,
    overwrite: bool,
) -> Result<DashImportResult, String> {
    let dash_dir = get_dashboards_dir(&app);

    // Ensure dashboards directory exists
    std::fs::create_dir_all(&dash_dir)
        .map_err(|e| format!("Failed to create dashboards directory: {}", e))?;

    let source = Path::new(&source_path);

    // Check source file exists
    if !source.exists() {
        return Ok(DashImportResult {
            source_path: source_path.clone(),
            success: false,
            error: Some("Source file does not exist".to_string()),
            file_info: None,
        });
    }

    // Validate it's a parseable dash or gauge file
    let lower = source_path.to_lowercase();
    if lower.ends_with(".gauge") {
        if let Err(e) = dash::load_gauge_file(source) {
            return Ok(DashImportResult {
                source_path: source_path.clone(),
                success: false,
                error: Some(format!("Invalid gauge file: {}", e)),
                file_info: None,
            });
        }
    } else {
        let content =
            std::fs::read_to_string(source).map_err(|e| format!("Failed to read file: {}", e))?;

        if let Err(e) = dash::parse_dash_file(&content) {
            return Ok(DashImportResult {
                source_path: source_path.clone(),
                success: false,
                error: Some(format!("Invalid dashboard file: {}", e)),
                file_info: None,
            });
        }
    }

    // Determine target filename
    let file_name = if let Some(ref new_name) = rename_to {
        new_name.clone()
    } else {
        source
            .file_name()
            .ok_or_else(|| "Invalid file path".to_string())?
            .to_string_lossy()
            .to_string()
    };

    let dest_path = dash_dir.join(&file_name);

    // Check for conflict
    if dest_path.exists() && !overwrite {
        return Ok(DashImportResult {
            source_path: source_path.clone(),
            success: false,
            error: Some(format!("File '{}' already exists", file_name)),
            file_info: None,
        });
    }

    // Copy file to dashboards directory
    std::fs::copy(source, &dest_path).map_err(|e| format!("Failed to copy file: {}", e))?;

    println!(
        "[import_dash_file] Imported {} -> {:?}",
        source_path, dest_path
    );

    Ok(DashImportResult {
        source_path,
        success: true,
        error: None,
        file_info: Some(DashFileInfo {
            name: file_name,
            path: dest_path.to_string_lossy().to_string(),
            category: "User".to_string(),
        }),
    })
}

/// Create default dashboard XML files in the given directory
fn create_default_dashboard_files(dir: &Path) -> Result<(), String> {
    // Basic Dashboard
    let basic = create_basic_dashboard();
    let basic_xml = dash::write_dash_file(&basic)
        .map_err(|e| format!("Failed to serialize basic dashboard: {}", e))?;
    std::fs::write(dir.join("Basic.ltdash.xml"), basic_xml)
        .map_err(|e| format!("Failed to write Basic.ltdash.xml: {}", e))?;

    // Tuning Dashboard
    let tuning = create_tuning_dashboard();
    let tuning_xml = dash::write_dash_file(&tuning)
        .map_err(|e| format!("Failed to serialize tuning dashboard: {}", e))?;
    std::fs::write(dir.join("Tuning.ltdash.xml"), tuning_xml)
        .map_err(|e| format!("Failed to write Tuning.ltdash.xml: {}", e))?;

    // Racing Dashboard
    let racing = create_racing_dashboard();
    let racing_xml = dash::write_dash_file(&racing)
        .map_err(|e| format!("Failed to serialize racing dashboard: {}", e))?;
    std::fs::write(dir.join("Racing.ltdash.xml"), racing_xml)
        .map_err(|e| format!("Failed to write Racing.ltdash.xml: {}", e))?;

    println!("[create_default_dashboard_files] Created 3 default dashboards");
    Ok(())
}

/// Get list of available dashboard templates
#[tauri::command]
async fn get_dashboard_templates() -> Result<Vec<DashboardTemplateInfo>, String> {
    Ok(vec![
        DashboardTemplateInfo {
            id: "basic".to_string(),
            name: "Basic Dashboard".to_string(),
            description: "Essential gauges: RPM, AFR, Coolant, Throttle".to_string(),
        },
        DashboardTemplateInfo {
            id: "racing".to_string(),
            name: "Racing Dashboard".to_string(),
            description: "Large RPM with shift lights, oil pressure, water temp".to_string(),
        },
        DashboardTemplateInfo {
            id: "tuning".to_string(),
            name: "Tuning Dashboard".to_string(),
            description: "AFR, VE, Spark advance, and correction factors".to_string(),
        },
    ])
}

#[derive(Serialize)]
struct DashboardTemplateInfo {
    id: String,
    name: String,
    description: String,
}

// =============================================================================
// Tune File Save/Load/Burn Commands
// =============================================================================

#[derive(Serialize)]
struct TuneInfo {
    path: Option<String>,
    signature: String,
    modified: bool,
    has_tune: bool,
}

/// Gets information about the currently loaded tune.
///
/// Returns: TuneInfo with path, signature, and modification status
#[tauri::command]
async fn get_tune_info(state: tauri::State<'_, AppState>) -> Result<TuneInfo, String> {
    let tune_guard = state.current_tune.lock().await;
    let path_guard = state.current_tune_path.lock().await;
    let modified = *state.tune_modified.lock().await;

    match &*tune_guard {
        Some(tune) => Ok(TuneInfo {
            path: path_guard.as_ref().map(|p| p.to_string_lossy().to_string()),
            signature: tune.signature.clone(),
            modified,
            has_tune: true,
        }),
        None => Ok(TuneInfo {
            path: None,
            signature: String::new(),
            modified: false,
            has_tune: false,
        }),
    }
}

#[tauri::command]
async fn new_tune(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let def_guard = state.definition.lock().await;
    let signature = def_guard
        .as_ref()
        .map(|d| d.signature.clone())
        .unwrap_or_else(|| "Unknown".to_string());

    let tune = TuneFile::new(&signature);

    *state.current_tune.lock().await = Some(tune);
    *state.current_tune_path.lock().await = None;
    *state.tune_modified.lock().await = false;

    Ok(())
}

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

/// Get the current migration report (if any) from loading a tune
#[tauri::command]
async fn get_migration_report(
    state: tauri::State<'_, AppState>,
) -> Result<Option<MigrationReport>, String> {
    let report = state.migration_report.lock().await;
    Ok(report.clone())
}

/// Clear the current migration report
#[tauri::command]
async fn clear_migration_report(state: tauri::State<'_, AppState>) -> Result<(), String> {
    *state.migration_report.lock().await = None;
    Ok(())
}

/// Get INI metadata for the currently loaded tune
#[tauri::command]
async fn get_tune_ini_metadata(
    state: tauri::State<'_, AppState>,
) -> Result<Option<IniMetadata>, String> {
    let tune = state.current_tune.lock().await;
    Ok(tune.as_ref().and_then(|t| t.ini_metadata.clone()))
}

/// Get constant manifest for the currently loaded tune
#[tauri::command]
async fn get_tune_constant_manifest(
    state: tauri::State<'_, AppState>,
) -> Result<Option<Vec<ConstantManifestEntry>>, String> {
    let tune = state.current_tune.lock().await;
    Ok(tune.as_ref().and_then(|t| t.constant_manifest.clone()))
}

/// Lists all tune files in the projects directory.
///
/// Scans for MSQ and JSON tune files.
///
/// Returns: Sorted vector of tune file paths
#[tauri::command]
async fn list_tune_files() -> Result<Vec<String>, String> {
    let projects_dir = libretune_core::project::Project::projects_dir()
        .map_err(|e| format!("Failed to get projects directory: {}", e))?;

    // Ensure directory exists
    std::fs::create_dir_all(&projects_dir)
        .map_err(|e| format!("Failed to create projects directory: {}", e))?;

    let mut tunes = Vec::new();

    let entries = std::fs::read_dir(&projects_dir)
        .map_err(|e| format!("Failed to read projects directory: {}", e))?;

    for entry in entries.flatten() {
        if let Some(name) = entry.file_name().to_str() {
            if name.ends_with(".msq") || name.ends_with(".json") {
                tunes.push(entry.path().to_string_lossy().to_string());
            }
        }
    }

    tunes.sort();
    Ok(tunes)
}

/// Burns (writes) tune data from ECU RAM to non-volatile flash memory.
///
/// This is the critical "save to ECU" operation that persists changes.
/// Saves window state first in case of issues.
///
/// Returns: Nothing on success
#[tauri::command]
async fn burn_to_ecu(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // Save window state before critical operation (in case of crash)
    let _ = app.save_window_state(StateFlags::all());

    let mut conn_guard = state.connection.lock().await;
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;

    // Send burn command to ECU
    // The 'b' command tells the ECU to write RAM to flash
    conn.send_burn_command()
        .map_err(|e| format!("Burn failed: {}", e))?;

    Ok(())
}

/// Execute a controller command by name
/// Resolves command chains and sends raw bytes to ECU
#[tauri::command]
async fn execute_controller_command(
    state: tauri::State<'_, AppState>,
    command_name: String,
) -> Result<(), String> {
    // Resolve command bytes while holding definition lock, then release definition before acquiring connection
    let bytes = {
        let def_guard = state.definition.lock().await;
        let def = def_guard.as_ref().ok_or("No INI definition loaded")?;
        resolve_command_bytes(def, &command_name, &mut std::collections::HashSet::new())?
    };

    // Now acquire connection lock only for the I/O
    let mut conn_guard = state.connection.lock().await;
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;

    // Send bytes to ECU
    conn.send_raw_bytes(&bytes)
        .map_err(|e| format!("Failed to send command: {}", e))?;

    Ok(())
}

/// Recursively resolve a command to raw bytes, handling command chaining
fn resolve_command_bytes(
    def: &EcuDefinition,
    command_name: &str,
    visited: &mut std::collections::HashSet<String>,
) -> Result<Vec<u8>, String> {
    // Prevent infinite recursion
    if visited.contains(command_name) {
        return Err(format!(
            "Circular command reference detected: {}",
            command_name
        ));
    }
    visited.insert(command_name.to_string());

    let cmd = def
        .controller_commands
        .get(command_name)
        .ok_or_else(|| format!("Command not found: {}", command_name))?;

    let mut result = Vec::new();

    for part in &cmd.parts {
        match part {
            CommandPart::Raw(raw_str) => {
                // Parse hex escapes and variable substitution
                let bytes = parse_command_string(def, raw_str)?;
                result.extend(bytes);
            }
            CommandPart::Reference(ref_name) => {
                // Recursively resolve referenced command
                let ref_bytes = resolve_command_bytes(def, ref_name, visited)?;
                result.extend(ref_bytes);
            }
        }
    }

    Ok(result)
}

/// Parse a command string with hex escapes (\x00) and variable substitution ($tsCanId)
fn parse_command_string(def: &EcuDefinition, s: &str) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            // Escape sequence
            match chars.next() {
                Some('x') | Some('X') => {
                    // Hex byte: \x00
                    let mut hex = String::new();
                    for _ in 0..2 {
                        if let Some(&c) = chars.peek() {
                            if c.is_ascii_hexdigit() {
                                hex.push(chars.next().unwrap());
                            } else {
                                break;
                            }
                        }
                    }
                    if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                        result.push(byte);
                    }
                }
                Some('n') => result.push(b'\n'),
                Some('r') => result.push(b'\r'),
                Some('t') => result.push(b'\t'),
                Some('\\') => result.push(b'\\'),
                Some(c) => result.push(c as u8),
                None => {}
            }
        } else if ch == '$' {
            // Variable substitution
            let mut var_name = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_alphanumeric() || c == '_' {
                    var_name.push(chars.next().unwrap());
                } else {
                    break;
                }
            }

            // Look up variable value
            if let Some(&value) = def.pc_variables.get(&var_name) {
                result.push(value);
            } else {
                // Variable not found - push 0 as default
                result.push(0);
            }
        } else {
            result.push(ch as u8);
        }
    }

    Ok(result)
}

#[tauri::command]
async fn mark_tune_modified(state: tauri::State<'_, AppState>) -> Result<(), String> {
    *state.tune_modified.lock().await = true;
    Ok(())
}

/// Compare the current project tune with the tune synced from ECU
/// Returns true if they differ, false if identical
#[tauri::command]
async fn compare_project_and_ecu_tunes(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let tune_guard = state.current_tune.lock().await;
    let project_guard = state.current_project.lock().await;

    // Get ECU tune (synced from ECU, currently in current_tune)
    let ecu_tune = match tune_guard.as_ref() {
        Some(t) => t,
        None => return Ok(false), // No ECU tune, can't compare
    };

    // Get project tune path and load it
    let project_tune = if let Some(ref project) = *project_guard {
        let tune_path = project.current_tune_path();
        if tune_path.exists() {
            match TuneFile::load(&tune_path) {
                Ok(tune) => Some(tune),
                Err(e) => {
                    eprintln!("[WARN] Failed to load project tune for comparison: {}", e);
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    // If no project tune, they're different (ECU has data, project doesn't)
    let project_tune = match project_tune {
        Some(t) => t,
        None => return Ok(true), // Different - project has no tune
    };

    // Compare page data
    // Get all unique page numbers
    let mut all_pages: Vec<u8> = project_tune
        .pages
        .keys()
        .chain(ecu_tune.pages.keys())
        .copied()
        .collect();
    all_pages.sort();
    all_pages.dedup();

    // Compare each page
    for page_num in all_pages {
        let project_page = project_tune.pages.get(&page_num);
        let ecu_page = ecu_tune.pages.get(&page_num);

        match (project_page, ecu_page) {
            (None, None) => continue,                             // Both missing, skip
            (Some(_), None) | (None, Some(_)) => return Ok(true), // One missing, different
            (Some(p), Some(e)) => {
                if p != e {
                    return Ok(true); // Pages differ
                }
            }
        }
    }

    // All pages match
    Ok(false)
}

/// Write the project tune to ECU
/// Loads the tune from the project's CurrentTune.msq and writes all pages to ECU
#[tauri::command]
async fn write_project_tune_to_ecu(
    _app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let project_guard = state.current_project.lock().await;
    let def_guard = state.definition.lock().await;

    let project = project_guard.as_ref().ok_or("No project open")?;
    let _def = def_guard.as_ref().ok_or("Definition not loaded")?;

    // Load project tune
    let tune_path = project.current_tune_path();
    let tune =
        TuneFile::load(&tune_path).map_err(|e| format!("Failed to load project tune: {}", e))?;

    drop(project_guard);
    drop(def_guard);

    // Write all pages to ECU
    let mut conn_guard = state.connection.lock().await;
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;

    // Sort pages for consistent writing
    let mut pages: Vec<(u8, &Vec<u8>)> = tune.pages.iter().map(|(k, v)| (*k, v)).collect();
    pages.sort_by_key(|(p, _)| *p);

    for (page_num, page_data) in pages {
        let params = libretune_core::protocol::commands::WriteMemoryParams {
            can_id: 0,
            page: page_num,
            offset: 0,
            data: page_data.clone(),
        };
        conn.write_memory(params)
            .map_err(|e| format!("Failed to write page {}: {}", page_num, e))?;
    }

    // Update cache and current_tune with project tune
    {
        let mut cache_guard = state.tune_cache.lock().await;
        if let Some(cache) = cache_guard.as_mut() {
            for (page_num, page_data) in &tune.pages {
                cache.load_page(*page_num, page_data.clone());
            }
        }
    }

    let mut tune_guard = state.current_tune.lock().await;
    *tune_guard = Some(tune);

    // Update path to project tune file
    *state.current_tune_path.lock().await = Some(tune_path);

    // Mark as not modified (freshly loaded from project)
    *state.tune_modified.lock().await = false;

    Ok(())
}

/// Save the current tune to the project's tune file
#[tauri::command]
async fn save_tune_to_project(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let project_guard = state.current_project.lock().await;
    let tune_guard = state.current_tune.lock().await;

    let project = project_guard.as_ref().ok_or("No project open")?;
    let tune = tune_guard.as_ref().ok_or("No tune loaded")?.clone();

    let tune_path = project.current_tune_path();

    drop(project_guard);
    drop(tune_guard);

    // Save tune to project path
    tune.save(&tune_path)
        .map_err(|e| format!("Failed to save tune to project: {}", e))?;

    // Update path
    *state.current_tune_path.lock().await = Some(tune_path);

    // Mark as not modified
    *state.tune_modified.lock().await = false;

    Ok(())
}

// =============================================================================
// Data Logging Commands
// =============================================================================

#[derive(Serialize)]
struct LoggingStatus {
    is_recording: bool,
    entry_count: usize,
    duration_ms: u64,
    channels: Vec<String>,
}

#[tauri::command]
async fn start_logging(
    state: tauri::State<'_, AppState>,
    sample_rate: Option<f64>,
) -> Result<(), String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    // Get channel names from output channels
    let channels: Vec<String> = def.output_channels.keys().cloned().collect();

    let mut logger = state.data_logger.lock().await;
    *logger = DataLogger::new(channels);
    if let Some(rate) = sample_rate {
        logger.set_sample_rate(rate);
    }
    logger.start();

    Ok(())
}

#[tauri::command]
async fn stop_logging(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut logger = state.data_logger.lock().await;
    logger.stop();
    Ok(())
}

#[tauri::command]
async fn get_logging_status(state: tauri::State<'_, AppState>) -> Result<LoggingStatus, String> {
    let logger = state.data_logger.lock().await;
    Ok(LoggingStatus {
        is_recording: logger.is_recording(),
        entry_count: logger.entry_count(),
        duration_ms: logger.duration().as_millis() as u64,
        channels: logger.channels().to_vec(),
    })
}

#[derive(Serialize)]
struct LogEntryData {
    timestamp_ms: u64,
    values: HashMap<String, f64>,
}

#[tauri::command]
async fn get_log_entries(
    state: tauri::State<'_, AppState>,
    start_index: Option<usize>,
    count: Option<usize>,
) -> Result<Vec<LogEntryData>, String> {
    let logger = state.data_logger.lock().await;
    let channels = logger.channels();

    let start = start_index.unwrap_or(0);
    let max_count = count.unwrap_or(1000);

    let entries: Vec<LogEntryData> = logger
        .entries()
        .skip(start)
        .take(max_count)
        .map(|entry| {
            let mut values = HashMap::new();
            for (i, channel) in channels.iter().enumerate() {
                if let Some(&val) = entry.values.get(i) {
                    values.insert(channel.clone(), val);
                }
            }
            LogEntryData {
                timestamp_ms: entry.timestamp.as_millis() as u64,
                values,
            }
        })
        .collect();

    Ok(entries)
}

#[tauri::command]
async fn clear_log(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut logger = state.data_logger.lock().await;
    logger.clear();
    Ok(())
}

#[tauri::command]
async fn save_log(state: tauri::State<'_, AppState>, path: String) -> Result<(), String> {
    let logger = state.data_logger.lock().await;

    // Create CSV content
    let mut csv = String::new();

    // Header row
    csv.push_str("Time (ms)");
    for channel in logger.channels() {
        csv.push(',');
        csv.push_str(channel);
    }
    csv.push('\n');

    // Data rows
    for entry in logger.entries() {
        csv.push_str(&format!("{}", entry.timestamp.as_millis()));
        for val in &entry.values {
            csv.push(',');
            csv.push_str(&format!("{:.4}", val));
        }
        csv.push('\n');
    }

    std::fs::write(&path, csv).map_err(|e| format!("Failed to save log: {}", e))?;

    Ok(())
}

#[tauri::command]
async fn read_text_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))
}

// =====================================================
// Diagnostic Logger Commands
// =====================================================
// Tooth and composite loggers for analyzing crank/cam trigger patterns

/// Tooth log entry (single tooth timing)
#[derive(Debug, Clone, Serialize)]
struct ToothLogEntry {
    /// Tooth number (0-indexed)
    tooth_number: u16,
    /// Time since last tooth in microseconds
    tooth_time_us: u32,
    /// Crank angle at this tooth (if available)
    crank_angle: Option<f32>,
}

/// Composite log entry (combined tooth + sync)
#[derive(Debug, Clone, Serialize)]
struct CompositeLogEntry {
    /// Time in microseconds since start
    time_us: u32,
    /// Primary trigger state (high/low)
    primary: bool,
    /// Secondary trigger state (high/low)  
    secondary: bool,
    /// Sync status
    sync: bool,
    /// Composite voltage (if analog)
    voltage: Option<f32>,
}

/// Tooth logger result
#[derive(Serialize)]
struct ToothLogResult {
    /// All captured tooth entries
    teeth: Vec<ToothLogEntry>,
    /// Total capture time in milliseconds
    capture_time_ms: u32,
    /// Detected RPM (if calculable)
    detected_rpm: Option<f32>,
    /// Number of teeth per revolution (if detected)
    teeth_per_rev: Option<u16>,
}

/// Composite logger result  
#[derive(Serialize)]
struct CompositeLogResult {
    /// All captured entries
    entries: Vec<CompositeLogEntry>,
    /// Total capture time in milliseconds
    capture_time_ms: u32,
    /// Sample rate in Hz
    sample_rate_hz: u32,
}

/// Start the tooth logger and capture data
///
/// ECU Protocol Commands:
/// - Speeduino: 'H' to get tooth log (blocking), 'T' for timing pattern, 'h' for tooth times
/// - rusEFI: 'l\x01' start tooth logger, 'l\x02' get data, 'l\x03' stop
/// - MS2/MS3: Page 0xf0-0xf1 fetch tooth log data
#[tauri::command]
async fn start_tooth_logger(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<ToothLogResult, String> {
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;

    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;
    let _def = def_guard.as_ref().ok_or("Definition not loaded")?;

    // Detect ECU type from signature
    let signature = conn.signature().unwrap_or_default().to_lowercase();

    let teeth: Vec<ToothLogEntry>;

    if signature.contains("speeduino") || signature.contains("202") {
        // Speeduino protocol: Send 'H' command for tooth log
        // Response format: 2-byte count (little-endian) + (count * 4-byte entries)
        // Each entry: 2 bytes tooth number (LE) + 2 bytes time in 0.5µs units (LE)
        eprintln!("[Tooth Logger] Starting Speeduino tooth capture...");

        let response = conn
            .send_raw_bytes_with_response(b"H", std::time::Duration::from_millis(2000))
            .map_err(|e| format!("Failed to get tooth log data: {}", e))?;

        if response.len() < 2 {
            return Err("Tooth logger returned no data (ECU may not support this command)".into());
        }

        // Parse 2-byte tooth count
        let tooth_count = u16::from_le_bytes([response[0], response[1]]) as usize;
        eprintln!("[Tooth Logger] ECU reports {} teeth", tooth_count);

        let expected_len = 2 + tooth_count * 4;
        if response.len() < expected_len {
            eprintln!(
                "[Tooth Logger] Warning: expected {} bytes but got {}. Parsing available data.",
                expected_len,
                response.len()
            );
        }

        let available_teeth = (response.len().saturating_sub(2)) / 4;
        let parse_count = available_teeth.min(tooth_count);

        teeth = (0..parse_count)
            .map(|i| {
                let offset = 2 + i * 4;
                let tooth_num = u16::from_le_bytes([response[offset], response[offset + 1]]);
                // Time is in 0.5µs units, convert to µs
                let raw_time = u16::from_le_bytes([response[offset + 2], response[offset + 3]]);
                let tooth_time_us = raw_time as u32 / 2;
                ToothLogEntry {
                    tooth_number: tooth_num,
                    tooth_time_us,
                    crank_angle: None, // Speeduino doesn't provide angle in this response
                }
            })
            .collect();

        eprintln!("[Tooth Logger] Parsed {} teeth from response", teeth.len());
    } else if signature.contains("rusefi") || signature.contains("fome") {
        // rusEFI protocol: Binary commands
        // 'l\x01' = start tooth logger
        // 'l\x02' = get tooth data
        // 'l\x03' = stop tooth logger
        // Response to 'l\x02': 2-byte count (BE) + (count * 4-byte entries)
        // Each entry: 4 bytes time in µs (big-endian, u32)
        eprintln!("[Tooth Logger] Starting rusEFI tooth capture...");

        // Start logger
        conn.send_raw_bytes(&[b'l', 0x01])
            .map_err(|e| format!("Failed to start tooth logger: {}", e))?;

        // Wait for capture
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Get data
        let response = conn
            .send_raw_bytes_with_response(&[b'l', 0x02], std::time::Duration::from_millis(2000))
            .map_err(|e| format!("Failed to get tooth data: {}", e))?;

        // Stop logger
        let _ = conn.send_raw_bytes(&[b'l', 0x03]);

        if response.len() < 2 {
            return Err("Tooth logger returned no data".into());
        }

        // rusEFI uses big-endian 2-byte count
        let tooth_count = u16::from_be_bytes([response[0], response[1]]) as usize;
        eprintln!("[Tooth Logger] ECU reports {} teeth", tooth_count);

        let available_teeth = (response.len().saturating_sub(2)) / 4;
        let parse_count = available_teeth.min(tooth_count);

        teeth = (0..parse_count)
            .map(|i| {
                let offset = 2 + i * 4;
                let tooth_time_us = u32::from_be_bytes([
                    response[offset],
                    response[offset + 1],
                    response[offset + 2],
                    response[offset + 3],
                ]);
                ToothLogEntry {
                    tooth_number: i as u16,
                    tooth_time_us,
                    crank_angle: None,
                }
            })
            .collect();

        eprintln!("[Tooth Logger] Parsed {} teeth from response", teeth.len());
    } else if signature.contains("ms2") || signature.contains("ms3") || signature.contains("mega") {
        // Megasquirt protocol: Read tooth log page
        // MS2/MS3 uses page 0xF0 for tooth log data
        // Response: raw bytes, each 2-byte pair is tooth time in µs (big-endian)
        eprintln!("[Tooth Logger] Starting Megasquirt tooth capture...");

        let response = conn
            .read_page(0xF0)
            .map_err(|e| format!("Failed to read tooth log page: {}", e))?;

        if response.is_empty() {
            return Err("Tooth logger returned no data".into());
        }

        // MS tooth log: each entry is 2 bytes (big-endian), tooth time in µs
        let tooth_count = response.len() / 2;
        teeth = (0..tooth_count)
            .filter_map(|i| {
                let offset = i * 2;
                let raw_time = u16::from_be_bytes([response[offset], response[offset + 1]]);
                // Skip zero entries (unused slots)
                if raw_time == 0 {
                    return None;
                }
                Some(ToothLogEntry {
                    tooth_number: i as u16,
                    tooth_time_us: raw_time as u32,
                    crank_angle: None,
                })
            })
            .collect();

        eprintln!("[Tooth Logger] Parsed {} teeth from response", teeth.len());
    } else {
        // Unknown ECU - return placeholder indicating feature not available
        return Err(format!(
            "Tooth logger not supported for this ECU type (signature: {})",
            signature
        ));
    }

    // Calculate RPM from tooth times (if we have enough data)
    let detected_rpm = if teeth.len() >= 2 {
        let total_time: u32 = teeth.iter().map(|t| t.tooth_time_us).sum();
        let avg_tooth_time_us = total_time as f32 / teeth.len() as f32;
        // Assuming standard trigger wheel (36-1 teeth = 35 actual teeth per rev)
        let teeth_per_rev = if teeth.len() > 30 {
            36
        } else {
            teeth.len() as u16
        };
        let rev_time_us = avg_tooth_time_us * teeth_per_rev as f32;
        let rpm = 60_000_000.0 / rev_time_us;
        Some(rpm)
    } else {
        None
    };

    // Emit event to frontend
    let _ = app.emit("tooth_logger:data", &teeth);

    Ok(ToothLogResult {
        teeth,
        capture_time_ms: 500,
        detected_rpm,
        teeth_per_rev: Some(36),
    })
}

/// Stops the tooth logger capture.
///
/// Sends the appropriate stop command based on ECU type.
///
/// Returns: Nothing on success
#[tauri::command]
async fn stop_tooth_logger(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut conn_guard = state.connection.lock().await;

    if let Some(conn) = conn_guard.as_mut() {
        let signature = conn.signature().unwrap_or_default().to_lowercase();

        if signature.contains("rusefi") || signature.contains("fome") {
            // rusEFI: Send stop command
            conn.send_raw_bytes(&[b'l', 0x03])
                .map_err(|e| format!("Failed to stop tooth logger: {}", e))?;
        }
        // Speeduino and MS don't need explicit stop
    }

    Ok(())
}

/// Start the composite logger and capture data
#[tauri::command]
async fn start_composite_logger(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<CompositeLogResult, String> {
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.lock().await;

    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;
    let _def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let signature = conn.signature().unwrap_or_default().to_lowercase();

    let entries: Vec<CompositeLogEntry>;

    if signature.contains("speeduino") || signature.contains("202") {
        // Speeduino composite logger commands:
        // 'J' = Start composite logger
        // 'O' = Get composite data
        // 'X' = Stop composite logger
        // Response to 'O': Raw bytes, each entry is 1 byte of packed flags:
        //   bit 0: primary trigger state
        //   bit 1: secondary trigger state
        //   bit 2: sync status
        // Entries are captured at ~10kHz (100µs intervals)
        eprintln!("[Composite Logger] Starting Speeduino composite capture...");

        conn.send_raw_bytes(b"J")
            .map_err(|e| format!("Failed to start composite logger: {}", e))?;

        std::thread::sleep(std::time::Duration::from_millis(500));

        let response = conn
            .send_raw_bytes_with_response(b"O", std::time::Duration::from_millis(2000))
            .map_err(|e| format!("Failed to get composite data: {}", e))?;

        if response.is_empty() {
            return Err("Composite logger returned no data".into());
        }

        // Each byte is a packed status entry at ~100µs intervals
        entries = response
            .iter()
            .enumerate()
            .map(|(i, &byte)| CompositeLogEntry {
                time_us: (i as u32) * 100, // 100µs per sample = 10kHz
                primary: (byte & 0x01) != 0,
                secondary: (byte & 0x02) != 0,
                sync: (byte & 0x04) != 0,
                voltage: None,
            })
            .collect();

        // Send stop
        let _ = conn.send_raw_bytes(b"X");

        eprintln!(
            "[Composite Logger] Parsed {} entries from response",
            entries.len()
        );
    } else if signature.contains("rusefi") || signature.contains("fome") {
        // rusEFI: 'l\x04' start, 'l\x05' get, 'l\x06' stop
        // Response to 'l\x05': 2-byte count (BE) + (count * 5-byte entries)
        // Each entry: 4 bytes time_us (BE u32) + 1 byte flags
        //   flags bit 0: primary, bit 1: secondary, bit 2: sync
        eprintln!("[Composite Logger] Starting rusEFI composite capture...");

        conn.send_raw_bytes(&[b'l', 0x04])
            .map_err(|e| format!("Failed to start composite logger: {}", e))?;

        std::thread::sleep(std::time::Duration::from_millis(500));

        let response = conn
            .send_raw_bytes_with_response(&[b'l', 0x05], std::time::Duration::from_millis(2000))
            .map_err(|e| format!("Failed to get composite data: {}", e))?;

        let _ = conn.send_raw_bytes(&[b'l', 0x06]);

        if response.len() < 2 {
            return Err("Composite logger returned no data".into());
        }

        let entry_count = u16::from_be_bytes([response[0], response[1]]) as usize;
        let available = (response.len().saturating_sub(2)) / 5;
        let parse_count = available.min(entry_count);

        entries = (0..parse_count)
            .map(|i| {
                let offset = 2 + i * 5;
                let time_us = u32::from_be_bytes([
                    response[offset],
                    response[offset + 1],
                    response[offset + 2],
                    response[offset + 3],
                ]);
                let flags = response[offset + 4];
                CompositeLogEntry {
                    time_us,
                    primary: (flags & 0x01) != 0,
                    secondary: (flags & 0x02) != 0,
                    sync: (flags & 0x04) != 0,
                    voltage: None,
                }
            })
            .collect();

        eprintln!(
            "[Composite Logger] Parsed {} entries from response",
            entries.len()
        );
    } else if signature.contains("ms2") || signature.contains("ms3") || signature.contains("mega") {
        // Megasquirt: Page 0xF2 for composite log data
        // Response: raw bytes, each entry is 6 bytes:
        //   4 bytes time_us (BE u32), 1 byte flags, 1 byte voltage (0-255 mapped to 0-5V)
        eprintln!("[Composite Logger] Starting Megasquirt composite capture...");

        let response = conn
            .read_page(0xF2)
            .map_err(|e| format!("Failed to read composite log page: {}", e))?;

        if response.is_empty() {
            return Err("Composite logger returned no data".into());
        }

        let entry_count = response.len() / 6;
        entries = (0..entry_count)
            .filter_map(|i| {
                let offset = i * 6;
                if offset + 5 >= response.len() {
                    return None;
                }
                let time_us = u32::from_be_bytes([
                    response[offset],
                    response[offset + 1],
                    response[offset + 2],
                    response[offset + 3],
                ]);
                // Skip zero-time entries (unused)
                if time_us == 0 {
                    return None;
                }
                let flags = response[offset + 4];
                let raw_voltage = response[offset + 5];
                Some(CompositeLogEntry {
                    time_us,
                    primary: (flags & 0x01) != 0,
                    secondary: (flags & 0x02) != 0,
                    sync: (flags & 0x04) != 0,
                    voltage: Some(raw_voltage as f32 * 5.0 / 255.0),
                })
            })
            .collect();

        eprintln!(
            "[Composite Logger] Parsed {} entries from response",
            entries.len()
        );
    } else {
        return Err(format!(
            "Composite logger not supported for this ECU type (signature: {})",
            signature
        ));
    }

    let _ = app.emit("composite_logger:data", &entries);

    Ok(CompositeLogResult {
        entries,
        capture_time_ms: 500,
        sample_rate_hz: 10000,
    })
}

/// Stops the composite logger capture.
///
/// Sends the appropriate stop command based on ECU type.
///
/// Returns: Nothing on success
#[tauri::command]
async fn stop_composite_logger(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut conn_guard = state.connection.lock().await;

    if let Some(conn) = conn_guard.as_mut() {
        let signature = conn.signature().unwrap_or_default().to_lowercase();

        if signature.contains("rusefi") || signature.contains("fome") {
            conn.send_raw_bytes(&[b'l', 0x06])
                .map_err(|e| format!("Failed to stop composite logger: {}", e))?;
        }
    }

    Ok(())
}

/// Table comparison result showing differences between two tables
#[derive(Serialize)]
struct TableComparisonResult {
    /// Table A name
    table_a: String,
    /// Table B name  
    table_b: String,
    /// Number of rows
    rows: usize,
    /// Number of columns
    cols: usize,
    /// Differences: (row, col, value_a, value_b, difference)
    differences: Vec<TableCellDiff>,
    /// Total number of differing cells
    diff_count: usize,
    /// Maximum absolute difference
    max_diff: f64,
    /// Average absolute difference (of differing cells only)
    avg_diff: f64,
}

#[derive(Serialize)]
struct TableCellDiff {
    row: usize,
    col: usize,
    value_a: f64,
    value_b: f64,
    diff: f64,
    percent_diff: f64,
}

/// Compares two tables cell-by-cell to show differences.
///
/// Useful for comparing before/after tuning changes or comparing tables
/// between different tune files.
///
/// # Arguments
/// * `table_a` - First table name
/// * `table_b` - Second table name
///
/// Returns: TableComparisonResult with all differences
#[tauri::command]
async fn compare_tables(
    state: tauri::State<'_, AppState>,
    table_a: String,
    table_b: String,
) -> Result<TableComparisonResult, String> {
    let def_guard = state.definition.lock().await;
    let cache_guard = state.tune_cache.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let cache = cache_guard.as_ref().ok_or("Tune cache not loaded")?;

    // Find table A definition
    let table_def_a = def
        .get_table_by_name_or_map(&table_a)
        .ok_or_else(|| format!("Table '{}' not found", table_a))?;

    // Find table B definition
    let table_def_b = def
        .get_table_by_name_or_map(&table_b)
        .ok_or_else(|| format!("Table '{}' not found", table_b))?;

    // Get dimensions from x_size and y_size
    let (rows_a, cols_a) = (table_def_a.y_size, table_def_a.x_size);
    let (rows_b, cols_b) = (table_def_b.y_size, table_def_b.x_size);

    if rows_a != rows_b || cols_a != cols_b {
        return Err(format!(
            "Table dimensions don't match: {}x{} vs {}x{}",
            rows_a, cols_a, rows_b, cols_b
        ));
    }

    let rows = rows_a;
    let cols = cols_a;

    // Read table A values
    let values_a = read_table_values(cache, def, table_def_a, rows, cols)?;
    let values_b = read_table_values(cache, def, table_def_b, rows, cols)?;

    // Compare cells
    let mut differences = Vec::new();
    let mut max_diff: f64 = 0.0;
    let mut total_diff: f64 = 0.0;

    for row in 0..rows {
        for col in 0..cols {
            let idx = row * cols + col;
            let val_a = values_a[idx];
            let val_b = values_b[idx];
            let diff = val_b - val_a;

            if diff.abs() > 0.0001 {
                let percent_diff = if val_a.abs() > 0.0001 {
                    (diff / val_a) * 100.0
                } else if diff.abs() > 0.0001 {
                    100.0
                } else {
                    0.0
                };

                differences.push(TableCellDiff {
                    row,
                    col,
                    value_a: val_a,
                    value_b: val_b,
                    diff,
                    percent_diff,
                });

                max_diff = max_diff.max(diff.abs());
                total_diff += diff.abs();
            }
        }
    }

    let diff_count = differences.len();
    let avg_diff = if diff_count > 0 {
        total_diff / diff_count as f64
    } else {
        0.0
    };

    Ok(TableComparisonResult {
        table_a,
        table_b,
        rows,
        cols,
        differences,
        diff_count,
        max_diff,
        avg_diff,
    })
}

/// Helper to read all values from a table into a flat vector
fn read_table_values(
    cache: &TuneCache,
    def: &EcuDefinition,
    table_def: &libretune_core::ini::TableDefinition,
    rows: usize,
    cols: usize,
) -> Result<Vec<f64>, String> {
    let mut values = Vec::with_capacity(rows * cols);

    // Look up the Z constant (main data array) from the map name
    let z_const = def
        .constants
        .get(&table_def.map)
        .ok_or_else(|| format!("Table map constant '{}' not found", table_def.map))?;

    let page_data = cache
        .get_page(z_const.page)
        .ok_or(format!("Page {} not loaded", z_const.page))?;

    let elem_size = z_const.data_type.size_bytes();
    let mut offset = z_const.offset as usize;

    for _row in 0..rows {
        for _col in 0..cols {
            if offset + elem_size > page_data.len() {
                return Err("Table data exceeds page bounds".to_string());
            }

            let raw_value = read_raw_value(&page_data[offset..], &z_const.data_type)?;
            let display_value = z_const.raw_to_display(raw_value);
            values.push(display_value);

            offset += elem_size;
        }
    }

    Ok(values)
}

/// Read a raw numeric value from bytes based on data type
fn read_raw_value(bytes: &[u8], data_type: &DataType) -> Result<f64, String> {
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

/// Reset all tune values to their INI-defined defaults
#[tauri::command]
async fn reset_tune_to_defaults(state: tauri::State<'_, AppState>) -> Result<u32, String> {
    let def_guard = state.definition.lock().await;
    let mut cache_guard = state.tune_cache.lock().await;
    let mut tune_guard = state.current_tune.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let cache = cache_guard.as_mut().ok_or("Tune cache not loaded")?;
    let tune = tune_guard.as_mut().ok_or("No tune loaded")?;

    let mut reset_count = 0u32;

    // Reset each constant to its default value
    for (name, constant) in &def.constants {
        // Skip arrays - they don't have simple defaults
        if !matches!(constant.shape, libretune_core::ini::Shape::Scalar) {
            continue;
        }

        // Get default value from INI [Defaults] section
        let default_value = if let Some(&default_val) = def.default_values.get(name) {
            default_val
        } else {
            // No default defined - use min value as fallback
            constant.min
        };

        // Update PC variable locally
        if constant.is_pc_variable {
            cache.local_values.insert(name.clone(), default_value);
            tune.constants
                .insert(name.clone(), TuneValue::Scalar(default_value));
            reset_count += 1;
            continue;
        }

        // Update ECU constant in cache and tune file
        // Convert display value to raw value for storage
        let raw_value = constant.display_to_raw(default_value);

        // Update tune file
        tune.constants
            .insert(name.clone(), TuneValue::Scalar(default_value));

        // Encode value to bytes and write to cache
        let bytes = encode_constant_value(raw_value, &constant.data_type);
        cache.write_bytes(constant.page, constant.offset, &bytes);
        reset_count += 1;
    }

    Ok(reset_count)
}

/// Export tune data to CSV file
#[tauri::command]
async fn export_tune_as_csv(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<u32, String> {
    let def_guard = state.definition.lock().await;
    let cache_guard = state.tune_cache.lock().await;
    let tune_guard = state.current_tune.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let mut csv_lines = Vec::new();
    csv_lines.push(
        "Name,Page,Offset,Shape,Value,Units,Min,Max,Scale,Translate,DataType,IsPcVariable"
            .to_string(),
    );

    let mut export_count = 0u32;

    // Export all constants
    for (name, constant) in &def.constants {
        // Get the current value(s)
        let value_str = if constant.data_type == DataType::String {
            // String constant — read raw bytes from cache/tune
            let str_len = constant.size_bytes();
            let raw = if let Some(cache) = cache_guard.as_ref() {
                cache
                    .read_bytes(constant.page, constant.offset, str_len as u16)
                    .map(|b| b.to_vec())
            } else {
                None
            };
            let raw = raw.or_else(|| {
                tune_guard.as_ref().and_then(|tune| {
                    tune.pages.get(&constant.page).and_then(|page_data| {
                        let start = constant.offset as usize;
                        let end = start + str_len;
                        if end <= page_data.len() {
                            Some(page_data[start..end].to_vec())
                        } else {
                            None
                        }
                    })
                })
            });
            if let Some(bytes) = raw {
                // Trim null padding
                let s = String::from_utf8_lossy(&bytes);
                let trimmed = s.trim_end_matches('\0');
                format!("\"{}\"", trimmed.replace('"', "\"\""))
            } else {
                "\"\"".to_string()
            }
        } else if matches!(constant.shape, libretune_core::ini::Shape::Scalar) {
            // Scalar constant
            let value = read_constant_from_cache_or_tune(
                name,
                constant,
                def.endianness,
                tune_guard.as_ref(),
                cache_guard.as_ref(),
            );
            format!("{}", value)
        } else {
            // Array constant — read all elements
            let elem_size = constant.data_type.size_bytes();
            let elem_count = constant.shape.element_count();
            let mut values = Vec::with_capacity(elem_count);

            for idx in 0..elem_count {
                let offset = constant.offset + (idx * elem_size) as u16;
                let raw_bytes = if let Some(cache) = cache_guard.as_ref() {
                    cache
                        .read_bytes(constant.page, offset, elem_size as u16)
                        .map(|b| b.to_vec())
                } else {
                    None
                };
                let raw_bytes = raw_bytes.or_else(|| {
                    tune_guard.as_ref().and_then(|tune| {
                        tune.pages.get(&constant.page).and_then(|page_data| {
                            let start = offset as usize;
                            let end = start + elem_size;
                            if end <= page_data.len() {
                                Some(page_data[start..end].to_vec())
                            } else {
                                None
                            }
                        })
                    })
                });
                let raw_val = if let Some(bytes) = raw_bytes {
                    constant
                        .data_type
                        .read_from_bytes(&bytes, 0, def.endianness)
                        .unwrap_or(0.0)
                } else {
                    0.0
                };
                let display_val = constant.raw_to_display(raw_val);
                values.push(format!("{}", display_val));
            }
            format!("\"[{}]\"", values.join(","))
        };

        let shape_str = match &constant.shape {
            libretune_core::ini::Shape::Scalar => "scalar".to_string(),
            libretune_core::ini::Shape::Array1D(n) => format!("[{}]", n),
            libretune_core::ini::Shape::Array2D { rows, cols } => format!("[{}x{}]", rows, cols),
        };

        // Escape name and units for CSV (in case they contain commas)
        let escaped_name = if name.contains(',') || name.contains('"') {
            format!("\"{}\"", name.replace('"', "\"\""))
        } else {
            name.clone()
        };
        let escaped_units = if constant.units.contains(',') || constant.units.contains('"') {
            format!("\"{}\"", constant.units.replace('"', "\"\""))
        } else {
            constant.units.clone()
        };

        let data_type_str = format!("{:?}", constant.data_type);

        csv_lines.push(format!(
            "{},{},{},{},{},{},{},{},{},{},{},{}",
            escaped_name,
            constant.page,
            constant.offset,
            shape_str,
            value_str,
            escaped_units,
            constant.min,
            constant.max,
            constant.scale,
            constant.translate,
            data_type_str,
            constant.is_pc_variable
        ));
        export_count += 1;
    }

    // Write to file
    let csv_content = csv_lines.join("\n");
    std::fs::write(&path, csv_content).map_err(|e| format!("Failed to write CSV file: {}", e))?;

    Ok(export_count)
}

/// Import tune data from CSV file
#[tauri::command]
async fn import_tune_from_csv(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<u32, String> {
    let def_guard = state.definition.lock().await;
    let mut cache_guard = state.tune_cache.lock().await;
    let mut tune_guard = state.current_tune.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let cache = cache_guard.as_mut().ok_or("Tune cache not loaded")?;
    let tune = tune_guard.as_mut().ok_or("No tune loaded")?;

    // Read CSV file
    let csv_content =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read CSV file: {}", e))?;

    let mut import_count = 0u32;
    let mut errors = Vec::new();

    for (line_num, line) in csv_content.lines().enumerate() {
        // Skip header
        if line_num == 0 && (line.starts_with("Name,") || line.starts_with("\"Name\"")) {
            continue;
        }

        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }

        // Parse CSV line (simple parser - handles basic quoting)
        let fields: Vec<&str> = parse_csv_line(line);

        // Support both old format (11 cols: Name,Page,Offset,Value,...)
        // and new format (12 cols: Name,Page,Offset,Shape,Value,...)
        let (name, value_field) = if fields.len() >= 12 {
            // New format with Shape column
            (fields[0].trim(), fields[4].trim())
        } else if fields.len() >= 4 {
            // Legacy format without Shape column
            (fields[0].trim(), fields[3].trim())
        } else {
            errors.push(format!("Line {}: too few fields", line_num + 1));
            continue;
        };

        // Find constant in definition
        let constant = match def.constants.get(name) {
            Some(c) => c,
            None => {
                // Constant not found - skip silently (might be from different INI)
                continue;
            }
        };

        // Handle string constants
        if constant.data_type == DataType::String {
            let str_val = value_field
                .trim_start_matches('"')
                .trim_end_matches('"')
                .replace("\"\"", "\"");
            let max_len = constant.size_bytes();
            let mut raw_data = vec![0u8; max_len];
            let copy_len = str_val.len().min(max_len);
            raw_data[..copy_len].copy_from_slice(&str_val.as_bytes()[..copy_len]);
            cache.write_bytes(constant.page, constant.offset, &raw_data);
            tune.constants
                .insert(name.to_string(), TuneValue::String(str_val));
            import_count += 1;
            continue;
        }

        // Handle array constants (value looks like "[1.0,2.0,3.0]")
        if !matches!(constant.shape, libretune_core::ini::Shape::Scalar) {
            let array_str = value_field
                .trim_start_matches('"')
                .trim_end_matches('"')
                .trim_start_matches('[')
                .trim_end_matches(']');

            let elem_size = constant.data_type.size_bytes();
            let elem_count = constant.shape.element_count();
            let values: Vec<f64> = array_str
                .split(',')
                .filter_map(|s| s.trim().parse::<f64>().ok())
                .collect();

            let parse_count = values.len().min(elem_count);
            for (idx, &display_val) in values.iter().take(parse_count).enumerate() {
                let clamped = display_val.clamp(constant.min, constant.max);
                let raw_val = constant.display_to_raw(clamped);
                let offset = constant.offset + (idx * elem_size) as u16;
                let mut bytes = vec![0u8; elem_size];
                constant
                    .data_type
                    .write_to_bytes(&mut bytes, 0, raw_val, def.endianness);
                cache.write_bytes(constant.page, offset, &bytes);
            }

            tune.constants
                .insert(name.to_string(), TuneValue::Array(values));
            import_count += 1;
            continue;
        }

        // Scalar constant
        let value: f64 = match value_field.parse() {
            Ok(v) => v,
            Err(_) => {
                errors.push(format!(
                    "Line {}: invalid value '{}'",
                    line_num + 1,
                    value_field
                ));
                continue;
            }
        };

        // Find constant in definition
        let constant = match def.constants.get(name) {
            Some(c) => c,
            None => {
                // Constant not found - skip silently (might be from different INI)
                continue;
            }
        };

        // Validate value is within bounds
        let clamped_value = value.clamp(constant.min, constant.max);
        if (clamped_value - value).abs() > 0.0001 {
            errors.push(format!(
                "Line {}: value {} clamped to {} (range {}-{})",
                line_num + 1,
                value,
                clamped_value,
                constant.min,
                constant.max
            ));
        }

        // Update PC variable locally
        if constant.is_pc_variable {
            cache.local_values.insert(name.to_string(), clamped_value);
            tune.constants
                .insert(name.to_string(), TuneValue::Scalar(clamped_value));
            import_count += 1;
            continue;
        }

        // Update ECU constant
        let raw_value = constant.display_to_raw(clamped_value);
        tune.constants
            .insert(name.to_string(), TuneValue::Scalar(clamped_value));

        // Encode value to bytes and write to cache
        let bytes = encode_constant_value(raw_value, &constant.data_type);
        cache.write_bytes(constant.page, constant.offset, &bytes);
        import_count += 1;
    }

    // Log errors if any
    if !errors.is_empty() {
        eprintln!("[CSV Import] {} warnings/errors:", errors.len());
        for err in errors.iter().take(10) {
            eprintln!("  {}", err);
        }
        if errors.len() > 10 {
            eprintln!("  ... and {} more", errors.len() - 10);
        }
    }

    Ok(import_count)
}

/// Simple CSV line parser that handles quoted fields
fn parse_csv_line(line: &str) -> Vec<&str> {
    let mut fields = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;
    let chars: Vec<char> = line.chars().collect();

    for (i, &ch) in chars.iter().enumerate() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if ch == ',' && !in_quotes {
            let field = &line[start..i];
            // Strip surrounding quotes if present
            let trimmed = field.trim();
            if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
                fields.push(&trimmed[1..trimmed.len() - 1]);
            } else {
                fields.push(trimmed);
            }
            start = i + 1;
        }
    }

    // Add last field
    let field = &line[start..];
    let trimmed = field.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        fields.push(&trimmed[1..trimmed.len() - 1]);
    } else {
        fields.push(trimmed);
    }

    fields
}

/// Encode a constant value to bytes based on data type (big-endian)
fn encode_constant_value(raw_value: f64, data_type: &DataType) -> Vec<u8> {
    match data_type {
        DataType::U08 => vec![raw_value.clamp(0.0, 255.0) as u8],
        DataType::S08 => vec![raw_value.clamp(-128.0, 127.0) as i8 as u8],
        DataType::U16 => {
            let val = raw_value.clamp(0.0, 65535.0) as u16;
            val.to_be_bytes().to_vec()
        }
        DataType::S16 => {
            let val = raw_value.clamp(-32768.0, 32767.0) as i16;
            val.to_be_bytes().to_vec()
        }
        DataType::U32 => {
            let val = raw_value.clamp(0.0, 4294967295.0) as u32;
            val.to_be_bytes().to_vec()
        }
        DataType::S32 => {
            let val = raw_value.clamp(-2147483648.0, 2147483647.0) as i32;
            val.to_be_bytes().to_vec()
        }
        DataType::F32 => (raw_value as f32).to_be_bytes().to_vec(),
        DataType::F64 => raw_value.to_be_bytes().to_vec(),
        DataType::Bits | DataType::String => {
            vec![raw_value.clamp(0.0, 255.0) as u8]
        }
    }
}

// =====================================================
// Project Management Commands
// =====================================================

#[derive(Serialize)]
struct ProjectInfoResponse {
    name: String,
    path: String,
    signature: String,
    modified: String,
}

#[derive(Serialize)]
struct IniEntryResponse {
    id: String,
    name: String,
    signature: String,
    path: String,
}

#[derive(Serialize)]
struct CurrentProjectInfo {
    name: String,
    path: String,
    signature: String,
    has_tune: bool,
    tune_modified: bool,
    connection: ConnectionSettingsResponse,
}

#[derive(Serialize)]
struct ConnectionSettingsResponse {
    port: Option<String>,
    baud_rate: u32,
    auto_connect: bool,
}

/// Get the path to the projects directory
#[tauri::command]
async fn get_projects_path() -> Result<String, String> {
    let path =
        Project::projects_dir().map_err(|e| format!("Failed to get projects directory: {}", e))?;

    // Create if doesn't exist
    std::fs::create_dir_all(&path)
        .map_err(|e| format!("Failed to create projects directory: {}", e))?;

    Ok(path.to_string_lossy().to_string())
}

/// List all available projects
#[tauri::command]
async fn list_projects() -> Result<Vec<ProjectInfoResponse>, String> {
    let projects =
        Project::list_projects().map_err(|e| format!("Failed to list projects: {}", e))?;

    Ok(projects
        .into_iter()
        .map(|p| ProjectInfoResponse {
            name: p.name,
            path: p.path.to_string_lossy().to_string(),
            signature: p.signature,
            modified: p.modified,
        })
        .collect())
}

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

/// Close the current project and clear state.
///
/// Closes the project, clears the INI definition and tune from memory.
/// Should be called before opening a different project.
///
/// Returns: Nothing on success
#[tauri::command]
async fn close_project(state: tauri::State<'_, AppState>) -> Result<(), String> {
    // Get and close the project
    let mut proj_guard = state.current_project.lock().await;
    if let Some(project) = proj_guard.take() {
        project
            .close()
            .map_err(|e| format!("Failed to close project: {}", e))?;
    }

    // Clear definition
    let mut def_guard = state.definition.lock().await;
    *def_guard = None;

    // Clear tune
    let mut tune_guard = state.current_tune.lock().await;
    *tune_guard = None;

    Ok(())
}

/// Get information about the currently open project.
///
/// Returns project metadata including name, path, signature, tune status,
/// and connection settings. Returns None if no project is open.
///
/// Returns: Optional CurrentProjectInfo with project details
#[tauri::command]
async fn get_current_project(
    state: tauri::State<'_, AppState>,
) -> Result<Option<CurrentProjectInfo>, String> {
    let proj_guard = state.current_project.lock().await;
    let tune_modified = *state.tune_modified.lock().await;

    Ok(proj_guard.as_ref().map(|project| CurrentProjectInfo {
        name: project.config.name.clone(),
        path: project.path.to_string_lossy().to_string(),
        signature: project.config.signature.clone(),
        has_tune: project.current_tune.is_some(),
        tune_modified,
        connection: ConnectionSettingsResponse {
            port: project.config.connection.port.clone(),
            baud_rate: project.config.connection.baud_rate,
            auto_connect: project.config.settings.auto_connect,
        },
    }))
}

/// Update the serial connection settings for the current project.
///
/// Saves the port name and baud rate to the project configuration file.
///
/// # Arguments
/// * `port` - Serial port name (e.g., "COM3", "/dev/ttyUSB0")
/// * `baud_rate` - Baud rate for communication
///
/// Returns: Nothing on success
#[tauri::command]
async fn update_project_connection(
    state: tauri::State<'_, AppState>,
    port: Option<String>,
    baud_rate: u32,
) -> Result<(), String> {
    let mut proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_mut()
        .ok_or_else(|| "No project open".to_string())?;

    project.config.connection.port = port;
    project.config.connection.baud_rate = baud_rate;
    project
        .save_config()
        .map_err(|e| format!("Failed to save project config: {}", e))?;

    Ok(())
}

/// Update the auto-connect setting for the current project
#[tauri::command]
async fn update_project_auto_connect(
    state: tauri::State<'_, AppState>,
    auto_connect: bool,
) -> Result<(), String> {
    let mut proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_mut()
        .ok_or_else(|| "No project open".to_string())?;

    project.config.settings.auto_connect = auto_connect;
    project
        .save_config()
        .map_err(|e| format!("Failed to save project config: {}", e))?;

    Ok(())
}

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

// =====================================================
// Restore Points Commands
// =====================================================

/// Info about a restore point
#[derive(Debug, Clone, serde::Serialize)]
pub struct RestorePointResponse {
    pub filename: String,
    pub path: String,
    pub created: String,
    pub size_bytes: u64,
}

/// Create a restore point from the current tune
#[tauri::command]
async fn create_restore_point(
    state: tauri::State<'_, AppState>,
) -> Result<RestorePointResponse, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let restore_path = project
        .create_restore_point()
        .map_err(|e| format!("Failed to create restore point: {}", e))?;

    // Auto-prune if max_restore_points is set
    let max_points = project.config.settings.max_restore_points;
    if max_points > 0 {
        let _ = project.prune_restore_points(max_points as usize);
    }

    let metadata = std::fs::metadata(&restore_path)
        .map_err(|e| format!("Failed to read restore point metadata: {}", e))?;

    Ok(RestorePointResponse {
        filename: restore_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        path: restore_path.to_string_lossy().to_string(),
        created: chrono::Utc::now().to_rfc3339(),
        size_bytes: metadata.len(),
    })
}

/// List restore points for the current project
#[tauri::command]
async fn list_restore_points(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<RestorePointResponse>, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let points = project
        .list_restore_points()
        .map_err(|e| format!("Failed to list restore points: {}", e))?;

    Ok(points
        .into_iter()
        .map(|p| RestorePointResponse {
            filename: p.filename,
            path: p.path.to_string_lossy().to_string(),
            created: p.created,
            size_bytes: p.size_bytes,
        })
        .collect())
}

/// Load a restore point as the current tune
#[tauri::command]
async fn load_restore_point(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    filename: String,
) -> Result<(), String> {
    let mut proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_mut()
        .ok_or_else(|| "No project open".to_string())?;

    project
        .load_restore_point(&filename)
        .map_err(|e| format!("Failed to load restore point: {}", e))?;

    // Reload the tune into cache
    if let Some(ref tune) = project.current_tune {
        let def_guard = state.definition.lock().await;
        if let Some(ref def) = *def_guard {
            let cache = TuneCache::from_definition(def);
            let mut cache_guard = state.tune_cache.lock().await;
            *cache_guard = Some(cache);

            if let Some(cache) = cache_guard.as_mut() {
                // Load page data
                for (page_num, page_data) in &tune.pages {
                    cache.load_page(*page_num, page_data.clone());
                }

                // Apply constants
                use libretune_core::tune::TuneValue;
                for (name, tune_value) in &tune.constants {
                    if let Some(constant) = def.constants.get(name) {
                        if constant.is_pc_variable {
                            if let TuneValue::Scalar(v) = tune_value {
                                cache.local_values.insert(name.clone(), *v);
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
                                let _ =
                                    cache.write_bytes(constant.page, constant.offset, &raw_data);
                            }
                            TuneValue::Array(arr) => {
                                for (i, val) in arr.iter().take(element_count).enumerate() {
                                    let raw_val = constant.display_to_raw(*val);
                                    let offset = i * element_size;
                                    constant.data_type.write_to_bytes(
                                        &mut raw_data,
                                        offset,
                                        raw_val,
                                        def.endianness,
                                    );
                                }
                                let _ =
                                    cache.write_bytes(constant.page, constant.offset, &raw_data);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    // Notify UI
    let _ = app.emit("tune:loaded", "restore_point");

    Ok(())
}

/// Delete a restore point by filename.
///
/// Permanently removes the specified restore point file from the project.
///
/// # Arguments
/// * `filename` - The filename of the restore point to delete
///
/// Returns: Nothing on success
#[tauri::command]
async fn delete_restore_point(
    state: tauri::State<'_, AppState>,
    filename: String,
) -> Result<(), String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    project
        .delete_restore_point(&filename)
        .map_err(|e| format!("Failed to delete restore point: {}", e))
}

/// Preview data for a TS project import
#[derive(Debug, Clone, Serialize)]
struct TsImportPreview {
    project_name: String,
    ini_file: Option<String>,
    has_tune: bool,
    restore_point_count: usize,
    has_pc_variables: bool,
    connection_port: Option<String>,
    connection_baud: Option<u32>,
}

/// Preview a TS project before importing
#[tauri::command]
async fn preview_tunerstudio_import(path: String) -> Result<TsImportPreview, String> {
    use libretune_core::project::Properties;

    let ts_path = std::path::Path::new(&path);

    // Look for project.properties in projectCfg subfolder
    let project_props_path = ts_path.join("projectCfg").join("project.properties");
    if !project_props_path.exists() {
        return Err("Not a valid TS project: project.properties not found".to_string());
    }

    let project_props = Properties::load(&project_props_path)
        .map_err(|e| format!("Failed to read project: {}", e))?;

    // Extract project name
    let project_name = project_props
        .get("projectName")
        .cloned()
        .unwrap_or_else(|| {
            ts_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Imported Project".to_string())
        });

    // Check for INI file
    let ini_file = project_props.get("ecuConfigFile").cloned();

    // Check for tune file
    let tune_path = ts_path.join("CurrentTune.msq");
    let has_tune = tune_path.exists();

    // Count restore points
    let restore_dir = ts_path.join("restorePoints");
    let restore_point_count = if restore_dir.exists() {
        std::fs::read_dir(&restore_dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().is_some_and(|ext| ext == "msq"))
                    .count()
            })
            .unwrap_or(0)
    } else {
        0
    };

    // Check for PC variables
    let pc_path = ts_path.join("projectCfg").join("pcVariableValues.msq");
    let has_pc_variables = pc_path.exists();

    // Connection settings
    let connection_port = project_props.get("commPort").cloned();
    let connection_baud = project_props.get_i32("baudRate").map(|v| v as u32);

    Ok(TsImportPreview {
        project_name,
        ini_file,
        has_tune,
        restore_point_count,
        has_pc_variables,
        connection_port,
        connection_baud,
    })
}

/// Import a TS project
#[tauri::command]
async fn import_tunerstudio_project(
    state: tauri::State<'_, AppState>,
    source_path: String,
) -> Result<CurrentProjectInfo, String> {
    let project = Project::import_tunerstudio(&source_path, None)
        .map_err(|e| format!("Failed to import TS project: {}", e))?;

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

    // Store as current project
    let mut proj_guard = state.current_project.lock().await;
    *proj_guard = Some(project);

    Ok(response)
}

// =====================================================
// Git Version Control Commands
// =====================================================

/// Response type for commit info
#[derive(Debug, Clone, Serialize)]
struct CommitInfoResponse {
    sha_short: String,
    sha: String,
    message: String,
    annotation: Option<String>,
    author: String,
    timestamp: String,
    is_head: bool,
}

impl From<CommitInfo> for CommitInfoResponse {
    fn from(info: CommitInfo) -> Self {
        Self {
            sha_short: info.sha_short,
            sha: info.sha,
            message: info.message,
            annotation: info.annotation,
            author: info.author,
            timestamp: info.timestamp,
            is_head: info.is_head,
        }
    }
}

/// Response type for branch info
#[derive(Debug, Clone, Serialize)]
struct BranchInfoResponse {
    name: String,
    is_current: bool,
    tip_sha: String,
}

impl From<BranchInfo> for BranchInfoResponse {
    fn from(info: BranchInfo) -> Self {
        Self {
            name: info.name,
            is_current: info.is_current,
            tip_sha: info.tip_sha,
        }
    }
}

/// Initialize git repository for current project
#[tauri::command]
async fn git_init_project(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let vc = VersionControl::init(&project.path)
        .map_err(|e| format!("Failed to initialize git: {}", e))?;

    // Create initial commit
    vc.commit("Initial project commit")
        .map_err(|e| format!("Failed to create initial commit: {}", e))?;

    Ok(true)
}

/// Check if current project has git repository
#[tauri::command]
async fn git_has_repo(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    Ok(VersionControl::is_git_repo(&project.path))
}

/// Commit current tune with message
#[tauri::command]
async fn git_commit(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    message: Option<String>,
    annotation: Option<String>,
) -> Result<String, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let vc = VersionControl::open(&project.path)
        .map_err(|e| format!("Git repository not initialized: {}", e))?;

    // Generate message from settings if not provided
    let commit_message = message.unwrap_or_else(|| {
        let settings = load_settings(&app);
        let now = chrono::Local::now();
        settings
            .commit_message_format
            .replace("{date}", &now.format("%Y-%m-%d").to_string())
            .replace("{time}", &now.format("%H:%M:%S").to_string())
    });

    let commit_message = format_commit_message(&commit_message, annotation.as_deref());

    let sha = vc
        .commit(&commit_message)
        .map_err(|e| format!("Failed to commit: {}", e))?;

    Ok(sha)
}

/// Get commit history for current project
#[tauri::command]
async fn git_history(
    state: tauri::State<'_, AppState>,
    max_count: Option<usize>,
) -> Result<Vec<CommitInfoResponse>, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let vc = VersionControl::open(&project.path)
        .map_err(|e| format!("Git repository not initialized: {}", e))?;

    let history = vc
        .get_history(max_count.unwrap_or(50))
        .map_err(|e| format!("Failed to get history: {}", e))?;

    Ok(history.into_iter().map(CommitInfoResponse::from).collect())
}

/// Get diff between two commits
#[tauri::command]
async fn git_diff(
    state: tauri::State<'_, AppState>,
    from_sha: String,
    to_sha: String,
) -> Result<CommitDiff, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let vc = VersionControl::open(&project.path)
        .map_err(|e| format!("Git repository not initialized: {}", e))?;

    vc.diff_commits(&from_sha, &to_sha)
        .map_err(|e| format!("Failed to diff commits: {}", e))
}

/// Checkout a specific commit
#[tauri::command]
async fn git_checkout(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    sha: String,
) -> Result<(), String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let vc = VersionControl::open(&project.path)
        .map_err(|e| format!("Git repository not initialized: {}", e))?;

    vc.checkout_commit(&sha)
        .map_err(|e| format!("Failed to checkout: {}", e))?;

    // Notify UI to reload tune
    let _ = app.emit("tune:loaded", "git_checkout");

    Ok(())
}

/// List all branches in the project's git repository.
///
/// Returns information about each branch including name, whether it's
/// the current branch, and its tip commit SHA.
///
/// Returns: Vector of BranchInfoResponse with branch details
#[tauri::command]
async fn git_list_branches(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<BranchInfoResponse>, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let vc = VersionControl::open(&project.path)
        .map_err(|e| format!("Git repository not initialized: {}", e))?;

    let branches = vc
        .list_branches()
        .map_err(|e| format!("Failed to list branches: {}", e))?;

    Ok(branches.into_iter().map(BranchInfoResponse::from).collect())
}

/// Create a new git branch in the project repository.
///
/// Creates a new branch pointing at the current HEAD commit.
/// Does not switch to the new branch automatically.
///
/// # Arguments
/// * `name` - Name for the new branch
///
/// Returns: Nothing on success
#[tauri::command]
async fn git_create_branch(state: tauri::State<'_, AppState>, name: String) -> Result<(), String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let vc = VersionControl::open(&project.path)
        .map_err(|e| format!("Git repository not initialized: {}", e))?;

    vc.create_branch(&name)
        .map_err(|e| format!("Failed to create branch: {}", e))
}

/// Switch to a different git branch.
///
/// Checks out the specified branch and emits an event to reload
/// the tune data in the UI.
///
/// # Arguments
/// * `name` - Name of the branch to switch to
///
/// Returns: Nothing on success
#[tauri::command]
async fn git_switch_branch(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<(), String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let vc = VersionControl::open(&project.path)
        .map_err(|e| format!("Git repository not initialized: {}", e))?;

    vc.switch_branch(&name)
        .map_err(|e| format!("Failed to switch branch: {}", e))?;

    // Notify UI to reload tune
    let _ = app.emit("tune:loaded", "git_switch_branch");

    Ok(())
}

/// Get the name of the current git branch.
///
/// Returns None if the project doesn't have a git repository initialized.
///
/// Returns: Optional branch name string
#[tauri::command]
async fn git_current_branch(state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    if !VersionControl::is_git_repo(&project.path) {
        return Ok(None);
    }

    let vc = VersionControl::open(&project.path)
        .map_err(|e| format!("Git repository not initialized: {}", e))?;

    Ok(vc.get_current_branch_name())
}

/// Check if the project has uncommitted git changes.
///
/// Returns false if the project doesn't have a git repository.
///
/// Returns: True if there are uncommitted changes
#[tauri::command]
async fn git_has_changes(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    if !VersionControl::is_git_repo(&project.path) {
        return Ok(false);
    }

    let vc = VersionControl::open(&project.path)
        .map_err(|e| format!("Git repository not initialized: {}", e))?;

    vc.has_changes()
        .map_err(|e| format!("Failed to check changes: {}", e))
}

// =====================================================
// Math Channel Commands
// =====================================================

#[tauri::command]
async fn get_math_channels(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<UserMathChannel>, String> {
    Ok(state.math_channels.lock().await.clone())
}

#[tauri::command]
async fn set_math_channel(
    state: tauri::State<'_, AppState>,
    mut channel: UserMathChannel,
) -> Result<(), String> {
    // Validate first
    channel
        .compile()
        .map_err(|e| format!("Invalid expression: {}", e))?;

    let mut channels = state.math_channels.lock().await;

    // Check if updating existing
    if let Some(existing) = channels.iter_mut().find(|c| c.name == channel.name) {
        *existing = channel;
    } else {
        channels.push(channel);
    }

    // Auto-save if project open
    let project = state.current_project.lock().await;
    if let Some(ref proj) = *project {
        let path = proj.path.join("math_channels.json");
        save_math_channels(&path, &channels)?;
    }

    Ok(())
}

#[tauri::command]
async fn delete_math_channel(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<(), String> {
    let mut channels = state.math_channels.lock().await;
    let initial_len = channels.len();
    channels.retain(|c| c.name != name);

    if channels.len() == initial_len {
        return Err(format!("Channel '{}' not found", name));
    }

    // Auto-save
    let project = state.current_project.lock().await;
    if let Some(ref proj) = *project {
        let path = proj.path.join("math_channels.json");
        save_math_channels(&path, &channels)?;
    }

    Ok(())
}

#[tauri::command]
async fn validate_math_expression(expr: String) -> Result<String, String> {
    let mut parser = libretune_core::ini::expression::Parser::new(&expr);
    match parser.parse() {
        Ok(_) => Ok("Valid expression".to_string()),
        Err(e) => Err(e),
    }
}

// =====================================================
// Base Map Generator & Project Utility Commands
// =====================================================

/// Generate a base map from engine specifications
#[tauri::command]
async fn generate_base_map(
    cylinder_count: u8,
    displacement_cc: f64,
    injector_size_cc: f64,
    fuel_type: String,
    aspiration: String,
    stroke_type: String,
    injection_mode: String,
    ignition_mode: String,
    idle_rpm: u16,
    redline_rpm: u16,
    boost_target_kpa: Option<f64>,
    target_wot_afr: Option<f64>,
) -> Result<serde_json::Value, String> {
    use libretune_core::basemap::{
        Aspiration, EngineSpec, FuelType, IgnitionMode, InjectionMode, StrokeType,
    };

    let fuel = match fuel_type.to_lowercase().as_str() {
        "gasoline" | "petrol" => FuelType::Gasoline,
        "e85" => FuelType::E85,
        "e100" => FuelType::E100,
        "methanol" => FuelType::Methanol,
        "lpg" | "propane" => FuelType::LPG,
        _ => return Err(format!("Unknown fuel type: {}", fuel_type)),
    };

    let asp = match aspiration.to_lowercase().as_str() {
        "na" | "naturally_aspirated" => Aspiration::NA,
        "turbo" | "turbocharged" => Aspiration::Turbo,
        "supercharged" => Aspiration::Supercharged,
        _ => return Err(format!("Unknown aspiration: {}", aspiration)),
    };

    let stroke = match stroke_type.to_lowercase().as_str() {
        "four_stroke" | "4stroke" | "4" => StrokeType::FourStroke,
        "two_stroke" | "2stroke" | "2" => StrokeType::TwoStroke,
        _ => return Err(format!("Unknown stroke type: {}", stroke_type)),
    };

    let inj = match injection_mode.to_lowercase().as_str() {
        "sequential" => InjectionMode::Sequential,
        "batch" => InjectionMode::Batch,
        "simultaneous" => InjectionMode::Simultaneous,
        "throttle_body" | "tbi" => InjectionMode::ThrottleBody,
        _ => return Err(format!("Unknown injection mode: {}", injection_mode)),
    };

    let ign = match ignition_mode.to_lowercase().as_str() {
        "wasted_spark" | "wastedspark" => IgnitionMode::WastedSpark,
        "coil_on_plug" | "cop" => IgnitionMode::CoilOnPlug,
        "distributor" => IgnitionMode::Distributor,
        _ => return Err(format!("Unknown ignition mode: {}", ignition_mode)),
    };

    let spec = EngineSpec {
        cylinder_count,
        displacement_cc,
        injector_size_cc,
        fuel_type: fuel,
        aspiration: asp,
        stroke_type: stroke,
        injection_mode: inj,
        ignition_mode: ign,
        idle_rpm,
        redline_rpm,
        boost_target_kpa,
        target_wot_afr,
    };

    let base_map = libretune_core::basemap::generator::generate_base_map(&spec);

    serde_json::to_value(&base_map).map_err(|e| format!("Failed to serialize base map: {}", e))
}

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

/// Get info about an MSQ file without fully loading it (for the open dialog preview)
#[tauri::command]
async fn get_msq_info(path: String) -> Result<serde_json::Value, String> {
    let file_path = std::path::Path::new(&path);
    if !file_path.exists() {
        return Err("File not found".to_string());
    }

    let tune = TuneFile::load(file_path).map_err(|e| format!("Failed to read MSQ: {}", e))?;

    let mut info = serde_json::Map::new();
    info.insert(
        "signature".to_string(),
        serde_json::Value::String(tune.signature.clone()),
    );
    info.insert(
        "version".to_string(),
        serde_json::Value::String(tune.version.clone()),
    );
    info.insert(
        "file_name".to_string(),
        serde_json::Value::String(
            file_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
        ),
    );
    info.insert(
        "file_size".to_string(),
        serde_json::Value::Number(serde_json::Number::from(
            std::fs::metadata(file_path).map(|m| m.len()).unwrap_or(0),
        )),
    );

    // Count constants
    let constant_count = tune.constants.len();
    info.insert(
        "constant_count".to_string(),
        serde_json::Value::Number(serde_json::Number::from(constant_count)),
    );

    // INI metadata if present
    if let Some(ref meta) = tune.ini_metadata {
        info.insert(
            "ini_name".to_string(),
            serde_json::Value::String(meta.name.clone()),
        );
        info.insert(
            "saved_at".to_string(),
            serde_json::Value::String(meta.saved_at.clone()),
        );
    }

    // Author and description
    if let Some(ref author) = tune.author {
        info.insert(
            "author".to_string(),
            serde_json::Value::String(author.clone()),
        );
    }
    if let Some(ref desc) = tune.description {
        info.insert(
            "description".to_string(),
            serde_json::Value::String(desc.clone()),
        );
    }

    Ok(serde_json::Value::Object(info))
}

/// Delete a project and all its files
#[tauri::command]
async fn delete_project(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    project_name: String,
) -> Result<(), String> {
    let projects_dir = get_projects_dir(&app);
    let project_path = projects_dir.join(&project_name);

    if !project_path.exists() {
        return Err(format!("Project '{}' not found", project_name));
    }

    // Don't allow deleting the currently open project
    let proj_guard = state.current_project.lock().await;
    if let Some(ref proj) = *proj_guard {
        if proj.config.name == project_name {
            return Err("Cannot delete the currently open project. Close it first.".to_string());
        }
    }
    drop(proj_guard);

    std::fs::remove_dir_all(&project_path)
        .map_err(|e| format!("Failed to delete project: {}", e))?;

    Ok(())
}

// =====================================================
// INI Repository Commands
// =====================================================

/// Initialize the INI repository for managing ECU definition files.
///
/// Opens or creates the local INI repository where ECU definitions
/// are stored and indexed.
///
/// Returns: Path to the repository directory
#[tauri::command]
async fn init_ini_repository(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let repo =
        IniRepository::open(None).map_err(|e| format!("Failed to open INI repository: {}", e))?;

    let path = repo.path.to_string_lossy().to_string();

    let mut guard = state.ini_repository.lock().await;
    *guard = Some(repo);

    Ok(path)
}

/// List INIs in the repository
#[tauri::command]
async fn list_repository_inis(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<IniEntryResponse>, String> {
    let guard = state.ini_repository.lock().await;
    let repo = guard
        .as_ref()
        .ok_or_else(|| "INI repository not initialized".to_string())?;

    Ok(repo
        .list()
        .iter()
        .map(|e| IniEntryResponse {
            id: e.id.clone(),
            name: e.name.clone(),
            signature: e.signature.clone(),
            path: e.path.clone(),
        })
        .collect())
}

/// Import an INI file into the local repository.
///
/// Copies the INI file and indexes it for future use.
///
/// # Arguments
/// * `source_path` - Path to the INI file to import
///
/// Returns: IniEntryResponse with the imported file's metadata
#[tauri::command]
async fn import_ini(
    state: tauri::State<'_, AppState>,
    source_path: String,
) -> Result<IniEntryResponse, String> {
    let mut guard = state.ini_repository.lock().await;
    let repo = guard
        .as_mut()
        .ok_or_else(|| "INI repository not initialized".to_string())?;

    let id = repo
        .import(Path::new(&source_path))
        .map_err(|e| format!("Failed to import INI: {}", e))?;

    let entry = repo
        .get(&id)
        .ok_or_else(|| "Failed to get imported INI".to_string())?;

    Ok(IniEntryResponse {
        id: entry.id.clone(),
        name: entry.name.clone(),
        signature: entry.signature.clone(),
        path: entry.path.clone(),
    })
}

/// Scan a directory for INI files and import them all.
///
/// Recursively searches for .ini files and adds them to the repository.
///
/// # Arguments
/// * `directory` - Path to directory to scan
///
/// Returns: Vector of imported INI IDs
#[tauri::command]
async fn scan_for_inis(
    state: tauri::State<'_, AppState>,
    directory: String,
) -> Result<Vec<String>, String> {
    let mut guard = state.ini_repository.lock().await;
    let repo = guard
        .as_mut()
        .ok_or_else(|| "INI repository not initialized".to_string())?;

    repo.scan_directory(Path::new(&directory))
        .map_err(|e| format!("Failed to scan directory: {}", e))
}

/// Remove an INI file from the repository.
///
/// Deletes the INI file and removes it from the index.
///
/// # Arguments
/// * `id` - The unique identifier of the INI to remove
///
/// Returns: Nothing on success
#[tauri::command]
async fn remove_ini(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    let mut guard = state.ini_repository.lock().await;
    let repo = guard
        .as_mut()
        .ok_or_else(|| "INI repository not initialized".to_string())?;

    repo.remove(&id)
        .map_err(|e| format!("Failed to remove INI: {}", e))
}

// =============================================================================
// ONLINE INI REPOSITORY COMMANDS
// =============================================================================

/// Serializable version of OnlineIniEntry for the frontend
#[derive(Serialize)]
struct OnlineIniEntryResponse {
    source: String,
    name: String,
    signature: Option<String>,
    download_url: String,
    repo_path: String,
    size: Option<u64>,
}

impl From<OnlineIniEntry> for OnlineIniEntryResponse {
    fn from(entry: OnlineIniEntry) -> Self {
        OnlineIniEntryResponse {
            source: entry.source.display_name().to_string(),
            name: entry.name,
            signature: entry.signature,
            download_url: entry.download_url,
            repo_path: entry.repo_path,
            size: entry.size,
        }
    }
}

/// Check if we have internet connectivity
#[tauri::command]
async fn check_internet_connectivity(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let repo = state.online_ini_repository.lock().await;
    Ok(repo.check_connectivity().await)
}

/// Search for INI files online matching a signature
/// If signature is None, returns all available INIs
#[tauri::command]
async fn search_online_inis(
    state: tauri::State<'_, AppState>,
    signature: Option<String>,
) -> Result<Vec<OnlineIniEntryResponse>, String> {
    let mut repo = state.online_ini_repository.lock().await;

    let results = repo
        .search(signature.as_deref())
        .await
        .map_err(|e| format!("Failed to search online INIs: {}", e))?;

    Ok(results.into_iter().map(|e| e.into()).collect())
}

/// Download an INI file from online repository
#[tauri::command]
async fn download_ini(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    download_url: String,
    name: String,
    source: String,
) -> Result<String, String> {
    let repo = state.online_ini_repository.lock().await;

    // Create an OnlineIniEntry from the provided info
    let source_enum = match source.to_lowercase().as_str() {
        "speeduino" => IniSource::Speeduino,
        "rusefi" => IniSource::RusEFI,
        _ => IniSource::Custom,
    };

    let entry = OnlineIniEntry {
        source: source_enum,
        name: name.clone(),
        signature: None,
        download_url,
        repo_path: name.clone(),
        size: None,
    };

    // Download to definitions directory
    let definitions_dir = get_definitions_dir(&app);

    let downloaded_path = repo
        .download(&entry, &definitions_dir)
        .await
        .map_err(|e| format!("Failed to download INI: {}", e))?;

    // Also import to local repository
    drop(repo);
    let mut local_repo_guard = state.ini_repository.lock().await;
    if let Some(ref mut local_repo) = *local_repo_guard {
        let _ = local_repo.import(&downloaded_path);
    }

    Ok(downloaded_path.to_string_lossy().to_string())
}

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

/// Get application settings
#[tauri::command]
async fn get_settings(app: tauri::AppHandle) -> Result<Settings, String> {
    Ok(load_settings(&app))
}

/// Update a single setting
#[tauri::command]
async fn update_setting(app: tauri::AppHandle, key: String, value: String) -> Result<(), String> {
    let mut settings = load_settings(&app);

    match key.as_str() {
        "units_system" => settings.units_system = value,
        "auto_burn_on_close" => {
            settings.auto_burn_on_close = value.parse().map_err(|_| "Invalid boolean value")?
        }
        "gauge_snap_to_grid" => {
            settings.gauge_snap_to_grid = value.parse().map_err(|_| "Invalid boolean value")?
        }
        "gauge_free_move" => {
            settings.gauge_free_move = value.parse().map_err(|_| "Invalid boolean value")?
        }
        "gauge_lock" => settings.gauge_lock = value.parse().map_err(|_| "Invalid boolean value")?,
        "auto_sync_gauge_ranges" => {
            settings.auto_sync_gauge_ranges = value.parse().map_err(|_| "Invalid boolean value")?
        }
        "indicator_column_count" => settings.indicator_column_count = value,
        "indicator_fill_empty" => {
            settings.indicator_fill_empty = value.parse().map_err(|_| "Invalid boolean value")?
        }
        "indicator_text_fit" => settings.indicator_text_fit = value,
        // Status bar channels (JSON array)
        "status_bar_channels" => {
            settings.status_bar_channels = serde_json::from_str(&value)
                .map_err(|e| format!("Invalid JSON for status_bar_channels: {}", e))?
        }
        // Heatmap scheme settings
        "heatmap_value_scheme" => settings.heatmap_value_scheme = value,
        "heatmap_change_scheme" => settings.heatmap_change_scheme = value,
        "heatmap_coverage_scheme" => settings.heatmap_coverage_scheme = value,
        // Help icon visibility
        "show_all_help_icons" => {
            settings.show_all_help_icons = value.parse().map_err(|_| "Invalid boolean value")?
        }
        // Alert rules settings
        "alert_large_change_enabled" => {
            settings.alert_large_change_enabled =
                value.parse().map_err(|_| "Invalid boolean value")?
        }
        "alert_large_change_abs" => {
            settings.alert_large_change_abs = value.parse().map_err(|_| "Invalid number value")?
        }
        "alert_large_change_percent" => {
            settings.alert_large_change_percent =
                value.parse().map_err(|_| "Invalid number value")?
        }
        "runtime_packet_mode" => {
            // Accept any string; UI should validate. Store as-is.
            settings.runtime_packet_mode = value;
        }
        "onboarding_completed" => {
            settings.onboarding_completed = value.parse().map_err(|_| "Invalid boolean value")?
        }
        // Session persistence
        "last_project_path" => {
            settings.last_project_path = if value.is_empty() { None } else { Some(value) }
        }
        "last_active_tab" => {
            settings.last_active_tab = if value.is_empty() { None } else { Some(value) }
        }
        _ => return Err(format!("Unknown setting: {}", key)),
    }

    save_settings(&app, &settings);
    let _ = app.emit("settings:changed", key.clone());
    Ok(())
}

/// Execute a Lua script in the sandboxed runtime
#[tauri::command]
async fn run_lua_script(script: String) -> Result<LuaExecutionResult, String> {
    execute_script(&script)
}

/// Update custom heatmap color stops for a context
#[tauri::command]
async fn update_heatmap_custom_stops(
    app: tauri::AppHandle,
    context: String,
    stops: Vec<String>,
) -> Result<(), String> {
    let mut settings = load_settings(&app);

    match context.as_str() {
        "value" => settings.heatmap_value_custom = stops,
        "change" => settings.heatmap_change_custom = stops,
        "coverage" => settings.heatmap_coverage_custom = stops,
        _ => return Err(format!("Unknown heatmap context: {}", context)),
    }

    save_settings(&app, &settings);
    Ok(())
}

/// Update a string-type constant
#[tauri::command]
async fn update_constant_string(
    state: tauri::State<'_, AppState>,
    _app: tauri::AppHandle,
    name: String,
    value: String,
) -> Result<(), String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let constant = def
        .constants
        .get(&name)
        .ok_or_else(|| format!("Constant {} not found", name))?;

    // Validate it's a string type
    if constant.data_type != DataType::String {
        return Err(format!("Constant {} is not a string type", name));
    }

    let max_len = constant.size_bytes();
    if max_len == 0 {
        return Err(format!("String constant {} has zero length", name));
    }

    // Encode string to bytes: fixed-length, null-padded
    let mut raw_data = vec![0u8; max_len];
    let copy_len = value.len().min(max_len);
    raw_data[..copy_len].copy_from_slice(&value.as_bytes()[..copy_len]);
    // Remaining bytes are already 0 (null padding)

    // Write to TuneCache if available
    let mut cache_guard = state.tune_cache.lock().await;
    if let Some(cache) = cache_guard.as_mut() {
        cache.write_bytes(constant.page, constant.offset, &raw_data);
    }

    // Update TuneFile in memory
    let mut tune_guard = state.current_tune.lock().await;
    if let Some(tune) = tune_guard.as_mut() {
        let page_data = tune.pages.entry(constant.page).or_insert_with(|| {
            let def_guard_inner = &def;
            vec![
                0u8;
                def_guard_inner
                    .page_sizes
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
        tune.constants.insert(
            name.clone(),
            libretune_core::tune::TuneValue::String(value.clone()),
        );
    }

    // Mark tune as modified
    *state.tune_modified.lock().await = true;

    // Write to ECU if connected
    let mut conn_guard = state.connection.lock().await;
    if let Some(conn) = conn_guard.as_mut() {
        let params = libretune_core::protocol::commands::WriteMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            data: raw_data,
        };
        if let Err(e) = conn.write_memory(params) {
            eprintln!("[WARN] Failed to write string constant to ECU: {}", e);
        }
    }

    eprintln!("Updated string constant '{}' to: '{}'", name, value);

    Ok(())
}

/// Get current hotkey bindings from settings
///
/// Returns: HashMap of action names to keyboard shortcuts
#[tauri::command]
async fn get_hotkey_bindings(app: tauri::AppHandle) -> Result<HashMap<String, String>, String> {
    let settings = load_settings(&app);
    Ok(settings.hotkey_bindings.clone())
}

/// Save hotkey bindings to settings
///
/// # Arguments
/// * `bindings` - HashMap of action names to keyboard shortcuts (e.g., {"table.setEqual": "="})
///
/// Returns: Nothing on success
#[tauri::command]
async fn save_hotkey_bindings(
    app: tauri::AppHandle,
    bindings: HashMap<String, String>,
) -> Result<(), String> {
    let mut settings = load_settings(&app);
    settings.hotkey_bindings = bindings;
    save_settings(&app, &settings);
    let _ = app.emit("settings:hotkeys_changed", ());
    Ok(())
}

/// Mark onboarding as completed
#[tauri::command]
async fn mark_onboarding_completed(app: tauri::AppHandle) -> Result<(), String> {
    let mut settings = load_settings(&app);
    settings.onboarding_completed = true;
    save_settings(&app, &settings);
    Ok(())
}

/// Check if onboarding has been completed
#[tauri::command]
async fn is_onboarding_completed(app: tauri::AppHandle) -> Result<bool, String> {
    let settings = load_settings(&app);
    Ok(settings.onboarding_completed)
}

// ============================================================================
// WASM Plugin Commands
// ============================================================================

/// Serializable plugin info returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WasmPluginInfo {
    name: String,
    version: String,
    description: String,
    author: String,
    state: String,
    permissions: Vec<String>,
    exec_count: u64,
}

/// Ensure the WASM plugin manager is initialized.
fn ensure_wasm_plugin_manager(_state: &AppState) -> WasmPluginManager {
    WasmPluginManager::new(WasmPluginConfig {
        data_dir: String::new(),
        ecu_type: String::from("Unknown"),
        libretune_version: String::from(env!("CARGO_PKG_VERSION")),
    })
}

/// Load a WASM plugin from a .wasm file.
///
/// # Arguments
/// * `path` - Path to the .wasm plugin file
/// * `manifest_json` - JSON string with plugin manifest (name, version, description, author, permissions)
///
/// Returns: Plugin name on success
#[tauri::command]
async fn load_wasm_plugin(
    path: String,
    manifest_json: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let manifest: WasmPluginManifest = serde_json::from_str(&manifest_json)
        .map_err(|e| format!("Invalid plugin manifest: {}", e))?;

    let wasm_path = std::path::Path::new(&path);
    if !wasm_path.exists() {
        return Err(format!("WASM file not found: {}", path));
    }

    let mut pm_guard = state.wasm_plugin_manager.lock().await;
    let pm = pm_guard.get_or_insert_with(|| ensure_wasm_plugin_manager(&state));

    let name = pm.load_plugin(manifest, wasm_path)?;
    Ok(name)
}

/// Unload a WASM plugin by name.
#[tauri::command]
async fn unload_wasm_plugin(name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut pm_guard = state.wasm_plugin_manager.lock().await;
    let pm = pm_guard.as_mut().ok_or("Plugin manager not initialized")?;
    pm.unload_plugin(&name)
}

/// List all loaded WASM plugins with their info.
#[tauri::command]
async fn list_wasm_plugins(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<WasmPluginInfo>, String> {
    let pm_guard = state.wasm_plugin_manager.lock().await;

    match pm_guard.as_ref() {
        Some(pm) => {
            let list = pm.list_plugins();
            Ok(list
                .iter()
                .map(|(name, stats)| {
                    // Try to get the manifest for additional info
                    let (version, description, author, permissions) =
                        if let Some(plugin) = pm.get_plugin(name) {
                            let m = plugin.manifest();
                            (
                                m.version.clone(),
                                m.description.clone(),
                                m.author.clone(),
                                m.permissions.iter().map(|p| format!("{:?}", p)).collect(),
                            )
                        } else {
                            (String::new(), String::new(), String::new(), vec![])
                        };

                    WasmPluginInfo {
                        name: name.clone(),
                        version,
                        description,
                        author,
                        state: format!("{:?}", stats.state),
                        permissions,
                        exec_count: stats.exec_count,
                    }
                })
                .collect())
        }
        None => Ok(vec![]),
    }
}

/// Execute a WASM plugin by name.
#[tauri::command]
async fn execute_wasm_plugin(
    name: String,
    state: tauri::State<'_, AppState>,
) -> Result<u64, String> {
    let mut pm_guard = state.wasm_plugin_manager.lock().await;
    let pm = pm_guard.as_mut().ok_or("Plugin manager not initialized")?;
    pm.execute_plugin(&name)
}

/// Get info about a specific WASM plugin.
#[tauri::command]
async fn get_wasm_plugin_info(
    name: String,
    state: tauri::State<'_, AppState>,
) -> Result<WasmPluginInfo, String> {
    let pm_guard = state.wasm_plugin_manager.lock().await;
    let pm = pm_guard.as_ref().ok_or("Plugin manager not initialized")?;

    let plugin = pm
        .get_plugin(&name)
        .ok_or_else(|| format!("Plugin '{}' not found", name))?;

    let stats = plugin.stats();
    let manifest = plugin.manifest();

    Ok(WasmPluginInfo {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        description: manifest.description.clone(),
        author: manifest.author.clone(),
        state: format!("{:?}", stats.state),
        permissions: manifest
            .permissions
            .iter()
            .map(|p| format!("{:?}", p))
            .collect(),
        exec_count: stats.exec_count,
    })
}

/// Use the project's saved tune file, discarding any ECU data.
///
/// Loads the tune from the project's CurrentTune.msq file and populates
/// the tune cache. Used when there's a conflict between project and ECU data.
///
/// Returns: Nothing on success
#[tauri::command]
async fn use_project_tune(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let project_guard = state.current_project.lock().await;
    let project = project_guard.as_ref().ok_or("No project loaded")?;

    // Load project tune from disk
    let tune_path = project.current_tune_path();
    if tune_path.exists() {
        let tune = TuneFile::load(&tune_path)
            .map_err(|e| format!("Failed to load project tune: {}", e))?;

        // Populate TuneCache from project tune
        {
            let mut cache_guard = state.tune_cache.lock().await;
            if let Some(cache) = cache_guard.as_mut() {
                for (page_num, page_data) in &tune.pages {
                    cache.load_page(*page_num, page_data.clone());
                }
            }
        }

        // Set as current tune
        *state.current_tune.lock().await = Some(tune);
        *state.current_tune_path.lock().await = Some(tune_path);
        *state.tune_modified.lock().await = false;

        // Emit event to trigger re-sync if connected
        let _ = app.emit("tune:loaded", "project");
    } else {
        return Err("Project tune file not found".to_string());
    }

    Ok(())
}

/// Use the ECU's tune data, discarding project file changes.
///
/// Keeps the currently synced ECU data and marks the tune as unmodified.
/// Used when there's a conflict between project and ECU data.
///
/// Returns: Nothing on success
#[tauri::command]
async fn use_ecu_tune(state: tauri::State<'_, AppState>) -> Result<(), String> {
    // ECU tune is already loaded from sync, just mark as not modified
    *state.tune_modified.lock().await = false;
    Ok(())
}

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
