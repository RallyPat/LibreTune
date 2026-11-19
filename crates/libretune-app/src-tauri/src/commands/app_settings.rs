//! App-level Settings struct, defaults, and load/save helpers.

use crate::paths::get_settings_path;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

#[derive(Serialize, Deserialize, Default, Clone)]
pub(crate) struct Settings {
    #[serde(default)]
    pub(crate) last_ini_path: Option<String>,
    #[serde(default)]
    pub(crate) units_system: String, // "metric" or "imperial"
    #[serde(default)]
    pub(crate) auto_burn_on_close: bool, // Auto-burn toggle
    #[serde(default)]
    pub(crate) gauge_snap_to_grid: bool, // Dashboard gauge snap to grid
    #[serde(default)]
    pub(crate) gauge_free_move: bool, // Dashboard gauge free move
    #[serde(default)]
    pub(crate) gauge_lock: bool, // Dashboard gauge lock in place
    #[serde(default = "default_true")]
    pub(crate) auto_sync_gauge_ranges: bool, // Auto-sync gauge ranges from INI
    /// Dashboard gauge redraw cap in Hz (allowed: 10, 15, 20, 25, 30).
    /// Lower values cut CPU/battery use on dashboards with many gauges.
    #[serde(default = "default_dashboard_refresh_hz")]
    pub(crate) dashboard_refresh_hz: u32,
    /// Right-align gauge numeric value text in a fixed region so digit-count
    /// and sign changes don't shift the layout ("jumping text" fix, issue #82).
    #[serde(default)]
    pub(crate) gauge_right_align_values: bool,
    #[serde(default)]
    pub(crate) indicator_column_count: String, // "auto" or number like "12"
    #[serde(default)]
    pub(crate) indicator_fill_empty: bool, // Fill empty cells in last row
    #[serde(default)]
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

    // --- UI layout state (restored on launch) ---
    /// Whether the left sidebar is visible.
    #[serde(default = "default_true")]
    pub(crate) sidebar_visible: bool,
    /// Whether ECU-derived (INI) tuning menus are shown in the top menu bar.
    /// When disabled, those menus are still reachable from the sidebar.
    #[serde(default = "default_true")]
    pub(crate) show_ecu_menus_in_menubar: bool,
    /// Whether the AI assistant side panel is visible.
    #[serde(default)]
    pub(crate) agent_panel_visible: bool,
    /// Expanded sidebar folder IDs (JSON array of strings) for session restore.
    #[serde(default)]
    pub(crate) sidebar_expanded_ids: Option<String>,
    /// The selected dashboard file name (e.g. "Telemetry Live.ltdash.xml").
    #[serde(default)]
    pub(crate) selected_dashboard: Option<String>,
    /// Serialized open tabs (id, title, icon, type, data) for session restore.
    #[serde(default)]
    pub(crate) open_tabs: Option<String>,

    /// Render table Y axis with the origin at the bottom-left (lowest load
    /// row at the bottom) instead of the top-left.
    #[serde(default)]
    pub(crate) table_y_axis_bottom: bool,

    /// Custom color for the live cursor marker (empty = theme default)
    #[serde(default)]
    pub(crate) table_cursor_color: String,

    /// Custom color for the operating-point trail (empty = default blue)
    #[serde(default)]
    pub(crate) table_trail_color: String,

    /// Seconds before trail points expire (0 = never)
    #[serde(default = "default_trail_fade_sec")]
    pub(crate) table_trail_fade_sec: f64,

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

    // --- AI Assistant (bring-your-own LLM) -------------------------------
    // All gated behind `ai_assistant_enabled`, which itself requires
    // `ai_risk_acknowledged`. The model only ever *proposes* changes; nothing
    // burns to the ECU automatically.
    /// Master enable for the AI assistant. Must be paired with a risk ack.
    #[serde(default = "default_false")]
    pub(crate) ai_assistant_enabled: bool,
    /// User has acknowledged the "at your own risk" warning.
    #[serde(default = "default_false")]
    pub(crate) ai_risk_acknowledged: bool,
    /// Provider protocol: "openai" | "anthropic" | "google".
    #[serde(default = "default_ai_provider")]
    pub(crate) ai_provider: String,
    /// Base URL (empty = provider default; or local endpoint like Ollama).
    #[serde(default)]
    pub(crate) ai_base_url: String,
    /// API key (plaintext v1; OS keychain hardening is a planned follow-up).
    #[serde(default)]
    pub(crate) ai_api_key: String,
    /// Model identifier (e.g. "gpt-4o", "claude-3-5-sonnet-...").
    #[serde(default)]
    pub(crate) ai_model: String,
    /// Capability the assistant is unlocked for: "read" | "tune" | "config".
    #[serde(default = "default_ai_capability")]
    pub(crate) ai_capability_tier: String,

    // --- Local MCP server (see `crate::mcp`) ---
    /// Expose the read-only tune tools to external MCP clients on loopback.
    /// Off by default: turning it on opens a socket, so it stays an explicit
    /// choice rather than something an update can switch on for the user.
    #[serde(default = "default_false")]
    pub(crate) mcp_enabled: bool,
    /// Loopback port for the MCP server.
    #[serde(default = "default_mcp_port")]
    pub(crate) mcp_port: u16,
}

/// Default MCP port. Same 8765 OpenTune used, so a client config ported over
/// only needs its URL changed.
pub(crate) fn default_mcp_port() -> u16 {
    8765
}

pub(crate) fn default_runtime_packet_mode() -> String {
    "Auto".to_string()
}

fn default_ai_provider() -> String {
    "openai".to_string()
}

fn default_ai_capability() -> String {
    "read".to_string()
}

fn default_heatmap_scheme() -> String {
    "tunerstudio".to_string()
}

fn default_trail_fade_sec() -> f64 {
    8.0
}

fn default_dashboard_refresh_hz() -> u32 {
    30
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

/// Persist `settings`. Expected to be called only while holding
/// [`SETTINGS_IO_LOCK`] via [`with_settings`], which also serializes the
/// read half — otherwise two in-flight commands can lose each other's
/// updates.
fn save_settings_locked(app: &tauri::AppHandle, settings: &Settings) {
    apply_unit_symbols(settings);
    let settings_path = get_settings_path(app);
    // Ensure parent directory exists
    if let Some(parent) = settings_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // The AI API key lives in the OS keychain when one is available; the
    // settings file then keeps an empty string so the secret never rests on
    // disk. (When no keychain backend exists the plaintext fallback is
    // intentionally preserved — losing the key would lock the user out.)
    let mut to_write = settings.clone();
    if !to_write.ai_api_key.is_empty() && crate::commands::ai_keychain::load().is_some() {
        to_write.ai_api_key = String::new();
    }
    if let Err(e) = write_settings_atomic(&settings_path, &to_write) {
        eprintln!("[WARN] Failed to save settings: {}", e);
    }
}

/// Process-wide lock serializing ALL settings read-modify-write cycles.
///
/// Why: Tauri commands run concurrently on the async runtime, and settings
/// updates used to be unsynchronized `load_settings` → mutate → `save_settings`
/// sequences. Two races resulted:
///
/// 1. **Corrupt/failed atomic writes.** Concurrent saves both wrote to the
///    SAME sibling `.tmp` file; when the first save renamed it away, the
///    second save's rename failed with "The system cannot find the file
///    specified. (os error 2)" (seen as repeated WARN spam), and truncated
///    interleaved writes to the shared tmp handle could corrupt the file.
/// 2. **Lost updates.** Two concurrent `update_setting` calls each loaded
///    their own snapshot; the second save silently reverted the first call's
///    change.
///
/// `std::sync::Mutex` (not tokio's) is correct here: the critical section is
/// pure synchronous file I/O — no `.await` — so holding it cannot deadlock
/// the executor's async tasks.
static SETTINGS_IO_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Run `f` against the current settings and persist the result, with the
/// whole load → mutate → save cycle serialized against every other settings
/// writer in the process.
///
/// The settings file is written even when `f` leaves the struct unchanged or
/// returns `Err` (mirroring the previous save-always semantics of
/// `update_settings`, which persists partially-applied batches).
pub(crate) fn with_settings<R>(app: &tauri::AppHandle, f: impl FnOnce(&mut Settings) -> R) -> R {
    let _guard = SETTINGS_IO_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut settings = load_settings(app);
    let result = f(&mut settings);
    save_settings_locked(app, &settings);
    result
}

/// Write `settings` to `path` atomically: serialize to a sibling `.tmp` file,
/// flush it to disk, then rename it over the real path. A crash, kill, or
/// power loss mid-write leaves either the old file or the fully-written new
/// one — never a truncated/partial file — because `std::fs::write`'s
/// truncate-then-write-in-place was exactly what could corrupt the file that
/// every setting (AI keys, hotkeys, layout, everything) lives in, with no
/// backup anywhere. `rename` onto an existing path is atomic on both
/// Windows and Unix as long as source and destination are on the same
/// volume, which a sibling file in the same directory always is.
fn write_settings_atomic(path: &Path, settings: &Settings) -> io::Result<()> {
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let tmp_path = path.with_extension("json.tmp");
    {
        let mut file = File::create(&tmp_path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp_path, path)
}

/// Helper exposed to extracted command modules: returns the commit message format string.
pub(crate) fn get_commit_message_format(app: &tauri::AppHandle) -> String {
    load_settings(app).commit_message_format
}

/// Construct the default `Settings` using the SAME values serde applies when a
/// field is missing during deserialization.
///
/// This is intentionally **not** the derived `Default` impl (which sets every
/// `bool` to `false` and every `String` to `""`, ignoring the `#[serde(default =
/// "...")]` attributes). Using `Settings::default()` as the no-file fallback
/// was the root cause of `sidebar_visible` (and every other `default_true`
/// field) silently becoming `false` on first run, because the all-`false`
/// struct was then written back to disk. Keep this in sync with the
/// `#[serde(default = "...")]` attributes on the struct fields.
fn default_settings() -> Settings {
    Settings {
        last_ini_path: None,
        units_system: String::new(),
        auto_burn_on_close: false,
        gauge_snap_to_grid: false,
        gauge_free_move: false,
        gauge_lock: false,
        auto_sync_gauge_ranges: default_true(),
        dashboard_refresh_hz: default_dashboard_refresh_hz(),
        gauge_right_align_values: false,
        indicator_column_count: String::new(),
        indicator_fill_empty: false,
        indicator_text_fit: String::new(),
        status_bar_channels: Vec::new(),
        show_all_help_icons: default_true(),
        last_project_path: None,
        last_active_tab: None,
        sidebar_visible: default_true(),
        show_ecu_menus_in_menubar: default_true(),
        agent_panel_visible: false,
        sidebar_expanded_ids: None,
        selected_dashboard: None,
        open_tabs: None,
        table_y_axis_bottom: false,
        table_cursor_color: String::new(),
        table_trail_color: String::new(),
        table_trail_fade_sec: default_trail_fade_sec(),
        heatmap_value_scheme: default_heatmap_scheme(),
        heatmap_change_scheme: default_heatmap_scheme(),
        heatmap_coverage_scheme: default_heatmap_scheme(),
        heatmap_value_custom: Vec::new(),
        heatmap_change_custom: Vec::new(),
        heatmap_coverage_custom: Vec::new(),
        auto_commit_on_save: default_auto_commit(),
        commit_message_format: default_commit_message_format(),
        runtime_packet_mode: default_runtime_packet_mode(),
        last_serial_port: None,
        auto_reconnect_after_controller_command: default_true(),
        auto_reconnect_after_firmware: default_true(),
        fome_fast_comms_enabled: default_true(),
        auto_record_enabled: default_false(),
        key_on_threshold_rpm: default_key_on_rpm(),
        key_off_timeout_sec: default_key_off_timeout(),
        alert_large_change_enabled: default_true(),
        alert_large_change_abs: default_alert_large_change_abs(),
        alert_large_change_percent: default_alert_large_change_percent(),
        hotkey_bindings: HashMap::new(),
        onboarding_completed: false,
        language: None,
        ai_assistant_enabled: false,
        ai_risk_acknowledged: false,
        ai_provider: default_ai_provider(),
        ai_base_url: String::new(),
        ai_api_key: String::new(),
        ai_model: String::new(),
        ai_capability_tier: default_ai_capability(),
        mcp_enabled: false,
        mcp_port: default_mcp_port(),
    }
}

/// Symbols declared by the loaded project, if it declared any. These outrank
/// any inference from the app's units preference, because they are the tune's
/// own statement of how it was built rather than a guess about the user.
static PROJECT_SYMBOLS: std::sync::RwLock<Option<Vec<String>>> = std::sync::RwLock::new(None);

/// Read `ecuSettings` from the project.properties beside `ini_path` and seed
/// the INI parser's conditional symbols from it.
///
/// TunerStudio stores, e.g.,
/// `ecuSettings=AFR|CELSIUS|enablehardware_test_OFF|` in
/// `projectCfg/project.properties`, the same folder as the controller INI, and
/// those tokens are exactly the symbols its `#if` blocks test.
///
/// Must run before the INI is parsed: `#if CELSIUS` is resolved during parsing.
pub(crate) fn seed_symbols_from_project(ini_path: &Path, settings: &Settings) {
    match project_declared_symbols(ini_path) {
        Some(symbols) => {
            tracing::info!(?symbols, "INI symbols taken from the project's ecuSettings");
            if let Ok(mut guard) = PROJECT_SYMBOLS.write() {
                *guard = Some(symbols.clone());
            }
            libretune_core::ini::set_default_symbols(symbols);
        }
        None => {
            // This project makes no declaration, so the previous one's must be
            // dropped: PROJECT_SYMBOLS outranks the units preference, and
            // leaving it set means opening a Celsius project and then an
            // undeclared one silently keeps Celsius - with no setting able to
            // override it for the rest of the session.
            if let Ok(mut guard) = PROJECT_SYMBOLS.write() {
                *guard = None;
            }
            apply_unit_symbols(settings);
        }
    }
}

/// The symbols a project declares in `ecuSettings`, if it declares any.
///
/// Split out from the seeding so the parsing rules - pipe-separated, trailing
/// empty token discarded - can be tested without a settings fixture.
fn project_declared_symbols(ini_path: &Path) -> Option<Vec<String>> {
    ini_path
        .parent()
        .map(|dir| dir.join("project.properties"))
        .filter(|p| p.exists())
        .and_then(|p| libretune_core::project::Properties::load(&p).ok())
        .and_then(|props| props.get("ecuSettings").cloned())
        .map(|raw| {
            raw.split('|')
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
}

/// Seed the INI preprocessor's conditional symbols.
///
/// An INI selects metric units through `#if CELSIUS`, and TunerStudio defines
/// that symbol from the project's `ecuSettings`. LibreTune never carried it, so
/// the Fahrenheit `#else` arm always won: a 23 degC cold start showed 73 on the
/// gauge, labelled with the INI's generic "TEMP".
///
/// A project that declares its own `ecuSettings` decides this outright. Only
/// when there is no declaration does the units preference stand in, and then an
/// explicit "metric" is required: the preference defaults to an empty string,
/// so treating "not imperial" as metric would switch every existing install to
/// Celsius whether or not it wanted that.
pub(crate) fn apply_unit_symbols(settings: &Settings) {
    if let Ok(guard) = PROJECT_SYMBOLS.read() {
        if let Some(symbols) = guard.as_ref() {
            libretune_core::ini::set_default_symbols(symbols.clone());
            return;
        }
    }

    let mut symbols: Vec<String> = Vec::new();
    if settings.units_system.eq_ignore_ascii_case("metric") {
        symbols.push("CELSIUS".to_string());
    }
    libretune_core::ini::set_default_symbols(symbols);
}

pub(crate) fn load_settings(app: &tauri::AppHandle) -> Settings {
    let settings_path = get_settings_path(app);
    if let Ok(content) = std::fs::read_to_string(&settings_path) {
        if let Some(mut settings) = parse_settings_or_backup(&settings_path, &content) {
            if settings.runtime_packet_mode.trim().is_empty() {
                settings.runtime_packet_mode = default_runtime_packet_mode();
            }
            migrate_ai_key_to_keychain(&mut settings);
            apply_unit_symbols(&settings);
            return settings;
        }
    }
    // Ensure default runtime mode is set when no file exists
    let mut s = default_settings();
    if s.runtime_packet_mode.trim().is_empty() {
        s.runtime_packet_mode = default_runtime_packet_mode();
    }
    apply_unit_symbols(&s);
    s
}

/// Move a plaintext AI API key into the OS keychain (one-time migration),
/// and fill the in-memory key from the keychain when the file has none.
/// Graceful on every failure path: no keychain → keep plaintext as-is, so
/// the assistant keeps working exactly as before this hardening existed.
fn migrate_ai_key_to_keychain(settings: &mut Settings) {
    if !settings.ai_api_key.is_empty() {
        // Plaintext found in the file: try to move it to the keychain. On
        // success the in-memory value stays (callers use it at runtime) but
        // the next save writes an empty string in its place.
        if crate::commands::ai_keychain::store(&settings.ai_api_key).is_ok() {
            tracing::info!("migrated AI API key from settings.json to the OS keychain");
        }
        return; // keep the in-memory value regardless
    }
    // File has no key: the authoritative copy (if any) is the keychain.
    if let Some(key) = crate::commands::ai_keychain::load() {
        settings.ai_api_key = key;
    }
}

/// Parse `content` (the text of `path`) as `Settings`. On failure, renames
/// `path` aside to `<path>.corrupt` instead of leaving the caller to
/// silently fall back to all-defaults and then overwrite the unreadable
/// original on the next save — that used to discard a corrupt settings file
/// (and every preference in it) with no warning and no way to recover it.
fn parse_settings_or_backup(path: &Path, content: &str) -> Option<Settings> {
    match serde_json::from_str::<Settings>(content) {
        Ok(settings) => Some(settings),
        Err(e) => {
            eprintln!(
                "[WARN] settings.json failed to parse ({}); backing it up to {}.corrupt \
                 instead of discarding it, and resetting to defaults for this session.",
                e,
                path.display()
            );
            let backup_path = path.with_extension("json.corrupt");
            let _ = std::fs::rename(path, &backup_path);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn default_settings_matches_serde_defaults_for_missing_fields() {
        // The whole point of default_settings() is to mirror what serde
        // produces when every field is absent (see its doc comment on the
        // sidebar_visible incident). Parsing "{}" is the ground truth for
        // that; if a field's #[serde(default...)] ever drifts from its
        // hand-written value here, this catches it instead of silently
        // shipping a divergence.
        let from_empty_json: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(
            serde_json::to_string(&default_settings()).unwrap(),
            serde_json::to_string(&from_empty_json).unwrap()
        );
    }

    #[test]
    fn write_settings_atomic_round_trips() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");

        let mut settings = default_settings();
        settings.units_system = "imperial".to_string();

        write_settings_atomic(&path, &settings).expect("write should succeed");

        // The .tmp sibling must not be left behind after a successful write.
        assert!(!path.with_extension("json.tmp").exists());

        let content = std::fs::read_to_string(&path).unwrap();
        let loaded: Settings = serde_json::from_str(&content).unwrap();
        assert_eq!(loaded.units_system, "imperial");
    }

    #[test]
    fn parse_settings_or_backup_returns_settings_on_valid_json() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"units_system":"metric"}"#).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();

        let result = parse_settings_or_backup(&path, &content);
        assert!(result.is_some());
        assert_eq!(result.unwrap().units_system, "metric");
        // A successful parse must not touch the original file.
        assert!(path.exists());
    }

    #[test]
    fn parse_settings_or_backup_preserves_corrupt_file_instead_of_discarding_it() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        // Simulates a crash/kill mid-write leaving truncated JSON.
        std::fs::write(&path, r#"{"units_system":"metr"#).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();

        let result = parse_settings_or_backup(&path, &content);
        assert!(result.is_none());

        // The corrupt file must survive under a backup name, not vanish.
        assert!(!path.exists());
        let backup_path = path.with_extension("json.corrupt");
        assert!(backup_path.exists());
        assert_eq!(
            std::fs::read_to_string(&backup_path).unwrap(),
            r#"{"units_system":"metr"#
        );
    }

    /// The project states its own units, so nothing is inferred from a UI
    /// preference. This is the declaration from a real NA6 Speeduino project
    /// whose `units_system` is the empty default - the case where either
    /// direction of guess is wrong for somebody.
    #[test]
    fn project_ecu_settings_decide_the_ini_symbols() {
        let dir = TempDir::new().unwrap();
        let cfg = dir.path().join("projectCfg");
        std::fs::create_dir_all(&cfg).unwrap();
        std::fs::write(
            cfg.join("project.properties"),
            "projectName=NA6_SPEEDUINO
             ecuSettings=AFR|CELSIUS|enablehardware_test_OFF|resetcontrol_standard|
",
        )
        .unwrap();

        seed_symbols_from_project(&cfg.join("mainController.ini"), &Settings::default());

        let symbols = PROJECT_SYMBOLS.read().unwrap().clone().unwrap();
        assert!(symbols.iter().any(|s| s == "CELSIUS"));
        assert_eq!(symbols.len(), 4, "the trailing empty token is not a symbol");

        // The declaration outranks the preference, so a user who never opened
        // Settings still gets the units the tune was built in.
        apply_unit_symbols(&Settings {
            units_system: String::new(),
            ..Default::default()
        });
        assert!(PROJECT_SYMBOLS
            .read()
            .unwrap()
            .as_ref()
            .unwrap()
            .iter()
            .any(|s| s == "CELSIUS"));

        *PROJECT_SYMBOLS.write().unwrap() = None;
    }

    #[test]
    fn parse_settings_or_backup_tolerates_missing_originally_non_default_fields() {
        // Before these fields got #[serde(default)], a settings.json missing
        // any one of them (e.g. saved by an older version, or with the key
        // manually removed) failed deserialization entirely and silently
        // reset every other setting too. Confirm each now degrades to just
        // that field's own default instead.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"auto_burn_on_close":true}"#).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();

        let result = parse_settings_or_backup(&path, &content);
        let settings = result.expect("missing fields should no longer break the whole parse");
        assert!(settings.auto_burn_on_close);
        assert_eq!(settings.last_ini_path, None);
        assert_eq!(settings.units_system, "");
    }
}
