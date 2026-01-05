//! Dashboard Components
//!
//! Implements dashboard system with:
//! - Multiple gauge types (analog, digital, bar, sweep, LED)
//! - Tabbed dashboard support
//! - Dashboard designer mode (drag & drop gauge layout)
//! - Full-screen mode
//! - GPS-based gauges (speed, distance, fuel economy calculations)

use crate::types::Expr;

/// Gauge types
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GaugeType {
    #[serde(rename = "analog")]
    Analog { min: f64, max: f64, value: f64, unit: String },
    #[serde(rename = "digital")]
    Digital { value: String, decimals: u8 },
    #[serde(rename = "bar")]
    Bar { min: f64, max: f64, value: f64, unit: String },
    #[serde(rename = "sweep")]
    Sweep { min: f64, max: f64, value: f64, unit: String },
    #[serde(rename = "led")]
    Led { is_on: bool, color_on: String, color_off: String },
}

/// Single gauge definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gauge {
    pub id: String,
    pub gauge_type: GaugeType,
    pub channel: String,
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub z_index: u8,
    pub min_value: f64,
    pub max_value: f64,
    pub low_warning: Option<f64>,
    pub high_warning: Option<f64>,
    pub high_critical: Option<f64>,
    pub decimals: u8,
    pub units: String,
    pub style: String,
    pub background_color: String,
    pub font_color: String,
    pub needle_color: String,
    pub trim_color: String,
    pub show_history: bool,
    pub show_min_max: bool,
}

/// Dashboard layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardLayout {
    pub name: String,
    pub is_fullscreen: bool,
    pub background_image: Option<String>,
    pub gauges: Vec<Gauge>,
}

/// Dashboard designer state
#[derive(Debug, Clone, Default)]
pub struct DashboardDesigner {
    pub selected_tool: DesignerTool,
    pub gauge_to_edit: Option<String>,
    pub dashboard: DashboardLayout,
}

/// Designer tools
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DesignerTool {
    Select,
    Move,
    Resize,
    AddGauge,
    AddLabel,
    AddImage,
    Properties,
    Delete,
    Undo,
    Redo,
    Save,
}
