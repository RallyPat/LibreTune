//! Table Editor Components
//!
//! Implements 2D and 3D table editors following standard ECU tuning patterns.
//! Supports:
//! - Re-bin feature for axis adjustment
//! - History trail overlay
//! - Color coding for richer/leaner visualization
//! - Keyboard navigation
//! - Cell selection and editing

use crate::types::{BinOp, UnaryOp};

/// Table axis (X or Y)
pub struct TableAxis {
    pub name: String,
    pub bins: Vec<f64>,
    pub label: String,
    pub read_only: bool,
}

/// Table cell value
pub struct TableCell {
    pub x: usize,
    pub y: usize,
    pub value: f64,
    pub is_changed: bool,
    pub is_active: bool,
    pub hit_count: Option<u32>,
    pub hit_weight: Option<f64>,
}

/// Cell selection
#[derive(Debug, Clone, Default)]
pub struct CellSelection {
    pub cells: Vec<(usize, usize)>,
}

/// Re-bin dialog state
#[derive(Debug, Clone)]
pub struct ReBinState {
    pub x_bins: Vec<f64>,
    pub y_bins: Vec<f64>,
    pub current_x_value: f64,
    pub current_y_values: Vec<f64>,
}

/// History trail settings
#[derive(Debug, Clone, Default)]
pub struct HistoryTrailSettings {
    pub show_trail: bool,
    pub trail_length: usize,
    pub trail_color: String,
}

/// Table editor toolbar operations
pub enum TableToolbarOp {
    SetEqual,
    IncreasePct,
    DecreasePct,
    IncreaseE,
    DecreaseE,
    Scale,
    Interpolate,
    Smooth,
    Rebin,
    Copy,
    Paste,
    Undo,
    Redo,
    Reset,
    LockCells,
    UnlockCells,
    ShowHistory,
    HideHistory,
    ColorShade,
}

/// 3D table view settings
#[derive(Debug, Clone, Default)]
pub struct TableView3D {
    pub yaw_angle: i32,
    pub roll_angle: i32,
    pub z_scale: f32,
    pub show_color_shade: bool,
    pub follow_mode: bool,
    pub show_active_values: bool,
    pub show_selected_xy: bool,
    pub increment_amount: usize,
}
