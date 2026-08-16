//! Application-level state held by Tauri.
//!
//! `AppState` is `manage()`d on startup and accessed by every Tauri command
//! via `tauri::State<AppState>`. All fields are `pub` so the (still large) set
//! of command implementations in `lib.rs` (and future submodules) can read and
//! lock them directly.

use libretune_core::autotune::{
    AutoTuneAuthorityLimits, AutoTuneFilters, AutoTuneReferenceTables, AutoTuneSettings,
    AutoTuneState,
};
use libretune_core::datalog::DataLogger;
use libretune_core::ini::{
    EcuDefinition, Endianness, IncTableCache, OutputChannel, ProtocolSettings,
};
use libretune_core::plugin_system::PluginManager as WasmPluginManager;
use libretune_core::project::{IniRepository, OnlineIniRepository, Project, UserMathChannel};
use libretune_core::protocol::{Connection, ConnectionConfig};
use libretune_core::realtime::Evaluator;
use libretune_core::tune::{MigrationReport, TuneCache, TuneFile};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Optional test seam: factory to produce a signature without opening real serial ports.
pub type ConnectionFactory = dyn Fn(ConnectionConfig, Option<ProtocolSettings>, Endianness) -> Result<String, String>
    + Send
    + Sync;

/// Tracks RPM state for key-on/off detection
pub struct RpmStateTracker {
    pub current_state: RpmState,
    pub pending_off_start: Option<std::time::Instant>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RpmState {
    On,
    Off,
}

impl RpmStateTracker {
    pub fn new() -> Self {
        Self {
            current_state: RpmState::Off,
            pending_off_start: None,
        }
    }

    /// Update RPM and check for state transitions.
    /// Returns Some(new_state) if state changed, None otherwise.
    pub fn update(&mut self, rpm: f64, threshold_rpm: f64, timeout_sec: u32) -> Option<RpmState> {
        let rpm_above_threshold = rpm >= threshold_rpm;

        match self.current_state {
            RpmState::Off => {
                if rpm_above_threshold {
                    self.current_state = RpmState::On;
                    self.pending_off_start = None;
                    return Some(RpmState::On);
                }
            }
            RpmState::On => {
                if rpm_above_threshold {
                    self.pending_off_start = None;
                } else {
                    match self.pending_off_start {
                        None => {
                            self.pending_off_start = Some(std::time::Instant::now());
                        }
                        Some(start_time) => {
                            if start_time.elapsed().as_secs() >= timeout_sec as u64 {
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

/// Live statistics about the realtime output-channel stream.
/// Updated by the streaming task on every tick and read by the
/// `get_output_channel_status` command.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StreamStats {
    pub ticks_total: u64,
    pub ticks_success: u64,
    pub ticks_skipped: u64,
    pub ticks_error: u64,
    pub transfer_mode: String,
    pub transfer_reason: String,
    pub interval_ms: u64,
    pub started_at_ms: i64,
    /// Realtime snapshots that could not be handed to the data logger because
    /// its lock was held elsewhere while a recording was active. A nonzero
    /// value means the saved log is missing samples (see [`LOGGER_SAMPLES_DROPPED`]).
    pub samples_dropped: u64,
}

/// Count of realtime samples dropped because the data-logger lock was busy on a
/// stream tick while recording (D10). The stream tick feeds the logger with
/// `try_lock` so it never stalls; before this counter those drops were silent.
/// Read into [`StreamStats::samples_dropped`] and reset when a recording starts.
pub static LOGGER_SAMPLES_DROPPED: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Whether a data-logging recording is currently active. Set by the start/stop
/// logging commands so the stream tick can tell a genuinely dropped sample
/// (lock busy *while recording*) from ordinary idle contention, without paying
/// for the data-logger lock on the hot path.
pub static LOGGER_RECORDING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AutoTuneLoadSource {
    Map,
    Maf,
    /// Throttle Position Sensor — used by Alpha-N / ITB (individual throttle
    /// body) fuelling strategies where the VE table's load (Y) axis is indexed
    /// by throttle opening rather than manifold pressure or mass airflow.
    /// See GitHub issue #132.
    Tps,
}

#[derive(Clone, Copy, Debug)]
pub enum AxisHint {
    Rpm,
    Load(AutoTuneLoadSource),
    #[allow(dead_code)]
    Unknown,
}

pub fn is_maf_channel_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("maf") || lower.contains("airmass") || lower.contains("airflow")
}

/// Detect a throttle-position (Alpha-N) load channel from an INI channel name
/// or label. Mirrors [`is_maf_channel_name`]. Typical Speeduino / rusEFI
/// channel names: `tps`, `tpsValue`, `throttle`, `throttlePos`, `tp` (and
/// `tpsAccel`/`tpsDot`, which are rate channels but still indicate a
/// TPS-based tune — we match on the `tps`/`throttle` root).
pub fn is_tps_channel_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    // Match `tps`/`throttle` roots but avoid `map`/`maf` false positives and
    // avoid bare `tp` matching substrings like `output`/`stopt`.
    lower == "tps"
        || lower == "tp"
        || lower == "throttle"
        || lower.contains("tps")
        || lower.contains("throttle")
}

/// AutoTune configuration stored when tuning session starts
#[derive(Clone)]
pub struct AutoTuneConfig {
    pub table_name: String,
    /// Signature of the ECU definition this session's bins/tables were
    /// resolved against. If the loaded definition changes (e.g. reconnect to
    /// a different ECU/INI) without stopping AutoTune first, recommendations
    /// computed against the old table layout must not be applied to
    /// whatever same-named table exists in the new one — checked at
    /// apply/burn time in autotune_misc.rs.
    pub definition_signature: String,
    pub secondary_table_name: Option<String>,
    pub settings: AutoTuneSettings,
    pub filters: AutoTuneFilters,
    pub authority_limits: AutoTuneAuthorityLimits,
    pub load_source: AutoTuneLoadSource,
    pub x_bins: Vec<f64>,
    pub y_bins: Vec<f64>,
    pub secondary_x_bins: Option<Vec<f64>>,
    pub secondary_y_bins: Option<Vec<f64>>,
    pub last_tps: Option<f64>,
    pub last_timestamp_ms: Option<u64>,
    /// Per-cell Target AFR / lambda delay reference tables for the session.
    /// Empty by default → AutoTune falls back to settings.target_afr and the
    /// RPM-based delay curve. See bug #14.
    ///
    /// Retained on the config for inspection; the live copy lives on
    /// `AutoTuneState` (set via `set_reference_tables` at start).
    #[allow(dead_code)]
    pub reference_tables: AutoTuneReferenceTables,
    /// When true (default), samples with no delayed-buffer match are dropped
    /// instead of being attributed to the current cell. See bug #2.
    #[allow(dead_code)]
    pub strict_lambda_match: bool,
}

pub struct AppState {
    pub connection: Mutex<Option<Connection>>,
    pub definition: Mutex<Option<EcuDefinition>>,
    pub autotune_state: Mutex<AutoTuneState>,
    pub autotune_secondary_state: Mutex<AutoTuneState>,
    pub connection_factory: Mutex<Option<Arc<ConnectionFactory>>>,
    pub autotune_config: Mutex<Option<AutoTuneConfig>>,
    pub streaming_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    #[allow(dead_code)]
    pub autotune_send_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    pub metrics_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    pub current_tune: Mutex<Option<TuneFile>>,
    pub current_tune_path: Mutex<Option<PathBuf>>,
    pub tune_modified: Mutex<bool>,
    pub data_logger: Mutex<DataLogger>,
    pub current_project: Mutex<Option<Project>>,
    pub ini_repository: Mutex<Option<IniRepository>>,
    pub online_ini_repository: Mutex<OnlineIniRepository>,
    pub tune_cache: Mutex<Option<TuneCache>>,
    /// Snapshot of project vs ECU pages while a tune-mismatch dialog is open.
    pub tune_mismatch_snapshot: Mutex<Option<TuneMismatchSnapshot>>,
    pub demo_mode: Mutex<bool>,
    pub wasm_plugin_manager: Mutex<Option<WasmPluginManager>>,
    pub migration_report: Mutex<Option<MigrationReport>>,
    pub evaluator: Mutex<Option<Evaluator>>,
    pub cached_output_channels: Mutex<Option<Arc<HashMap<String, OutputChannel>>>>,
    pub console_history: Mutex<Vec<String>>,
    pub rpm_state_tracker: Mutex<RpmStateTracker>,
    pub math_channels: Mutex<Vec<UserMathChannel>>,
    pub stream_stats: Mutex<StreamStats>,
    /// JoinHandle for the currently-running AI assistant turn (if any).
    /// Aborted by `agent_stop` to cancel an in-flight LLM request.
    pub agent_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Process-start epoch seconds for INI `timeNow()`.
    pub app_start_epoch: f64,
    /// Cached `.inc` table files for INI `table()` expressions.
    pub inc_table_cache: Arc<std::sync::Mutex<IncTableCache>>,
}

#[cfg(test)]
mod tests {
    use super::{is_maf_channel_name, is_tps_channel_name};

    #[test]
    fn detects_tps_load_channels() {
        // The names a Speeduino / rusEFI INI actually uses for the throttle
        // channel on an Alpha-N / ITB tune. All must be recognised so a TPS
        // Y-axis is auto-detected and the load source switches off MAP.
        for name in [
            "tps",
            "TPS",
            "tpsValue",
            "throttle",
            "throttlePos",
            "tp",
            "tpsDot",
        ] {
            assert!(is_tps_channel_name(name), "{name:?} should be TPS");
        }
    }

    #[test]
    fn does_not_false_positive_tps() {
        // Channels that must NOT be treated as throttle load sources.
        for name in ["map", "maf", "rpm", "afr", "clt", "boost", "dwell"] {
            assert!(!is_tps_channel_name(name), "{name:?} should not be TPS");
        }
    }

    #[test]
    fn tps_detection_independent_of_maf() {
        // The two detectors are orthogonal: a MAF channel is not a TPS channel
        // and vice-versa, so auto-detection picks the right load source.
        assert!(is_maf_channel_name("maf") && !is_tps_channel_name("maf"));
        assert!(is_tps_channel_name("tps") && !is_maf_channel_name("tps"));
    }
}

impl AppState {
    /// Epoch seconds at process start (for `timeNow()`).
    pub fn process_start_epoch() -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }

    /// Empty `.inc` table cache for AppState construction.
    pub fn new_inc_table_cache() -> Arc<std::sync::Mutex<IncTableCache>> {
        Arc::new(std::sync::Mutex::new(IncTableCache::default()))
    }
}

#[derive(Clone, Debug)]
pub struct TuneMismatchSnapshot {
    /// ECU page images captured at mismatch time (base for safe project merge).
    pub ecu_pages: HashMap<u8, Vec<u8>>,
}
