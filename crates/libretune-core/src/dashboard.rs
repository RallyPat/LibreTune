//! Dashboard Module
//!
//! Dashboard persistence and configuration.
//! Supports saving/loading dashboard layouts with gauge configurations.

use serde::{Serialize, Deserialize};

pub use std::collections::HashMap;

/// Gauge types supported by dashboard
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GaugeType {
    #[serde(rename = "analog_dial")]
    AnalogDial,
    #[serde(rename = "digital_readout")]
    DigitalReadout,
    #[serde(rename = "bar_gauge")]
    BarGauge,
    #[serde(rename = "sweep_gauge")]
    SweepGauge,
    #[serde(rename = "led_indicator")]
    LEDIndicator,
    /// Simple warning light - on/off based on boolean condition
    #[serde(rename = "warning_light")]
    WarningLight,
}

/// Configuration for a single gauge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaugeConfig {
    pub id: String,
    pub gauge_type: GaugeType,
    pub channel: String,
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub z_index: u32,
    pub min_value: f64,
    pub max_value: f64,
    pub low_warning: Option<f64>,
    pub high_warning: Option<f64>,
    pub high_critical: Option<f64>,
    pub decimals: u32,
    pub units: String,
    pub font_color: String,
    pub needle_color: String,
    pub trim_color: String,
    pub show_history: bool,
    pub show_min_max: bool,
    
    // Warning light specific fields
    /// Expression that evaluates to true when light should be on (e.g., "afr < 12")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_condition: Option<String>,
    /// Color when light is on (default: red)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_color: Option<String>,
    /// Color when light is off (default: dark gray)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub off_color: Option<String>,
    /// Whether to blink when on
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blink: Option<bool>,
}

impl Default for GaugeConfig {
    fn default() -> Self {
        Self {
            id: "gauge_1".to_string(),
            gauge_type: GaugeType::DigitalReadout,
            channel: "rpm".to_string(),
            label: "Engine Speed".to_string(),
            x: 0.1f64,
            y: 0.1f64,
            width: 0.2f64,
            height: 0.2f64,
            z_index: 0,
            min_value: 0.0f64,
            max_value: 8000.0f64,
            low_warning: Some(600.0f64),
            high_warning: Some(6500.0f64),
            high_critical: Some(7200.0f64),
            decimals: 0,
            units: "RPM".to_string(),
            font_color: "#FFFFFF".to_string(),
            needle_color: "#FF6600".to_string(),
            trim_color: "#666999".to_string(),
            show_history: true,
            show_min_max: true,
            // Warning light defaults
            on_condition: None,
            on_color: None,
            off_color: None,
            blink: None,
        }
    }
}

/// Complete dashboard layout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardLayout {
    pub name: String,
    pub gauges: Vec<GaugeConfig>,
    pub is_fullscreen: bool,
    pub background_image: Option<String>,
}

impl Default for DashboardLayout {
    fn default() -> Self {
        Self {
            name: "Default Dashboard".to_string(),
            gauges: vec![
                GaugeConfig {
                    id: "rpm_gauge".to_string(),
                    gauge_type: GaugeType::AnalogDial,
                    channel: "rpm".to_string(),
                    label: "Engine Speed".to_string(),
                    x: 0.1f64,
                    y: 0.1f64,
                    width: 0.3f64,
                    height: 0.3f64,
                    z_index: 0,
                    min_value: 0.0f64,
                    max_value: 8000.0f64,
                    low_warning: Some(600.0f64),
                    high_warning: Some(6500.0f64),
                    high_critical: Some(7200.0f64),
                    decimals: 0,
                    units: "RPM".to_string(),
                    font_color: "#FFFFFF".to_string(),
                    needle_color: "#FF6600".to_string(),
                    trim_color: "#666999".to_string(),
                    show_history: true,
                    show_min_max: true,
                    ..GaugeConfig::default()
                },
                GaugeConfig {
                    id: "afr_gauge".to_string(),
                    gauge_type: GaugeType::DigitalReadout,
                    channel: "afr".to_string(),
                    label: "AFR".to_string(),
                    x: 0.4f64,
                    y: 0.1f64,
                    width: 0.15f64,
                    height: 0.15f64,
                    z_index: 1,
                    min_value: 10.0f64,
                    max_value: 20.0f64,
                    low_warning: Some(13.0f64),
                    high_warning: Some(15.0f64),
                    high_critical: Some(16.0f64),
                    decimals: 2,
                    units: "".to_string(),
                    font_color: "#FFFFFF".to_string(),
                    needle_color: "#000000".to_string(),
                    trim_color: "#666999".to_string(),
                    show_history: false,
                    show_min_max: false,
                    ..GaugeConfig::default()
                },
                GaugeConfig {
                    id: "clt_gauge".to_string(),
                    gauge_type: GaugeType::BarGauge,
                    channel: "clt".to_string(),
                    label: "Coolant".to_string(),
                    x: 0.7f64,
                    y: 0.1f64,
                    width: 0.25f64,
                    height: 0.15f64,
                    z_index: 2,
                    min_value: -40.0f64,
                    max_value: 120.0f64,
                    low_warning: None,
                    high_warning: Some(100.0f64),
                    high_critical: Some(110.0f64),
                    decimals: 1,
                    units: "°C".to_string(),
                    font_color: "#FFFFFF".to_string(),
                    needle_color: "#00FF00".to_string(),
                    trim_color: "#666999".to_string(),
                    show_history: false,
                    show_min_max: false,
                    ..GaugeConfig::default()
                },
            ],
            is_fullscreen: false,
            background_image: None,
        }
    }
}

/// Helper function to get dashboard file name
pub fn get_dashboard_file(project_name: &str) -> String {
    format!("{}.dash", project_name)
}

/// Helper function to get dashboard file path
/// Note: This uses the libretune_core project module for cross-platform path resolution
pub fn get_dashboard_file_path(project_name: &str) -> std::path::PathBuf {
    // Use the cross-platform projects_dir from Project module
    crate::project::Project::projects_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(get_dashboard_file(project_name))
}
