//! Dialog Components
//!
//! Specific dialog implementations following standard ECU tuning patterns.

pub mod dialog_window;
pub use dialog_window::DialogWindow;

use crate::types::Expr;

/// Dialog types
pub enum DialogType {
    NewProject,
    BrowseProjects,
    SaveDialogTune,
    LoadDialogTune,
    BurnToEcu,
    AutoTuneDialog,
    NewDashboard,
    SettingsDialog,
}

/// Save dialog tune settings
#[derive(Debug, Clone)]
pub struct SaveDialogTune {
    pub tables: Option<Vec<String>>,
    pub settings: Option<Vec<String>>,
    pub save_format: String,
    pub auto_burn_on_close: bool,
    pub auto_burn_on_page_change: bool,
}

/// Load dialog tune
#[derive(Debug, Clone)]
pub struct LoadDialogTune {
    pub full_tune_only: bool,
    pub dialog_tune: Option<String>,
}

/// Auto-tune dialog (VE Analyze Live!)
#[derive(Debug, Clone)]
pub struct AutoTuneDialog {
    pub target_table: String,
    pub update_controller: bool,
    pub auto_send_updates: bool,
    pub send_button: Box<dyn Fn() + SendSync>,
    pub burn_button: Box<dyn Fn() + SendSync>,
    pub start_auto_tune: Box<dyn Fn() + SendSync>,
    pub stop_auto_tune: Box<dyn Fn() + SendSync>,
}

/// Settings dialog
#[derive(Debug, Clone)]
pub struct SettingsDialog {
    pub general_settings: SettingsCategory,
    pub fuel_settings: FuelSettingsCategory,
    pub ignition_settings: IgnitionSettingsCategory,
    pub startup_settings: StartupSettingsCategory,
}

/// Settings categories
#[derive(Debug, Clone)]
pub enum SettingsCategory {
    EngineSequential,
    RevLimiter,
    ShiftLight,
    CalibrationTPS,
    CalibrateMAPBaro,
    BatteryVoltage,
    UnLockCalibrations,
    CalibrateLambdaSensor,
    KnockSensor,
}

/// Dashboard configuration
#[derive(Debug, Clone)]
pub struct DashboardConfig {
    pub gauge_types: Vec<String>,
    pub auto_save_layout: bool,
    pub show_fps: bool,
}
