//! Application-level state held by Tauri.
//!
//! `AppState` is `manage()`d on startup and accessed by every Tauri command
//! via `tauri::State<AppState>`. All fields are `pub` so the (still large) set
//! of command implementations in `lib.rs` (and future submodules) can read and
//! lock them directly.

use libretune_core::autotune::{
    AutoTuneAuthorityLimits, AutoTuneFilters, AutoTuneSettings, AutoTuneState,
};
use libretune_core::datalog::DataLogger;
use libretune_core::ini::{EcuDefinition, Endianness, OutputChannel, ProtocolSettings};
use libretune_core::plugin_system::PluginManager as WasmPluginManager;
use libretune_core::project::{IniRepository, OnlineIniRepository, Project, UserMathChannel};
use libretune_core::protocol::{Connection, ConnectionConfig};
use libretune_core::realtime::Evaluator;
use libretune_core::tune::{MigrationReport, TuneCache, TuneFile};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

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
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AutoTuneLoadSource {
    Map,
    Maf,
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

/// AutoTune configuration stored when tuning session starts
#[derive(Clone)]
pub struct AutoTuneConfig {
    #[allow(dead_code)]
    pub table_name: String,
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
}

pub struct AppState {
    pub connection: Mutex<Option<Connection>>,
    pub definition: RwLock<Option<EcuDefinition>>,
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
    pub demo_mode: Mutex<bool>,
    pub wasm_plugin_manager: Mutex<Option<WasmPluginManager>>,
    pub migration_report: Mutex<Option<MigrationReport>>,
    pub evaluator: Mutex<Option<Evaluator>>,
    pub cached_output_channels: Mutex<Option<Arc<HashMap<String, OutputChannel>>>>,
    pub console_history: Mutex<Vec<String>>,
    pub rpm_state_tracker: Mutex<RpmStateTracker>,
    pub math_channels: Mutex<Vec<UserMathChannel>>,
    pub stream_stats: Mutex<StreamStats>,
    pub autosave_generation: Mutex<u64>,
}
