//! TunerStudio dashboard format data types.
//!
//! These structures match the TunerStudio XML schema exactly for full compatibility.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// TunerStudio color format (ARGB as signed 32-bit integer).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TsColor {
    pub alpha: u8,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl TsColor {
    /// Create from ARGB integer (TunerStudio's format)
    pub fn from_argb_int(value: i32) -> Self {
        let bytes = value.to_be_bytes();
        Self {
            alpha: bytes[0],
            red: bytes[1],
            green: bytes[2],
            blue: bytes[3],
        }
    }

    /// Convert to ARGB integer
    pub fn to_argb_int(&self) -> i32 {
        i32::from_be_bytes([self.alpha, self.red, self.green, self.blue])
    }

    /// Convert to CSS hex color
    pub fn to_css_hex(&self) -> String {
        if self.alpha == 255 {
            format!("#{:02x}{:02x}{:02x}", self.red, self.green, self.blue)
        } else {
            format!(
                "#{:02x}{:02x}{:02x}{:02x}",
                self.red, self.green, self.blue, self.alpha
            )
        }
    }

    /// Create from CSS hex color
    pub fn from_css_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');
        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Self {
                    alpha: 255,
                    red: r,
                    green: g,
                    blue: b,
                })
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Self {
                    alpha: a,
                    red: r,
                    green: g,
                    blue: b,
                })
            }
            _ => None,
        }
    }

    /// Common colors
    pub fn black() -> Self {
        Self {
            alpha: 255,
            red: 0,
            green: 0,
            blue: 0,
        }
    }
    pub fn white() -> Self {
        Self {
            alpha: 255,
            red: 255,
            green: 255,
            blue: 255,
        }
    }
    pub fn red() -> Self {
        Self {
            alpha: 255,
            red: 255,
            green: 0,
            blue: 0,
        }
    }
    pub fn yellow() -> Self {
        Self {
            alpha: 255,
            red: 255,
            green: 255,
            blue: 0,
        }
    }
    pub fn green() -> Self {
        Self {
            alpha: 255,
            red: 0,
            green: 255,
            blue: 0,
        }
    }
    pub fn transparent() -> Self {
        Self {
            alpha: 0,
            red: 0,
            green: 0,
            blue: 0,
        }
    }
}

/// Bibliography metadata for dashboard files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Bibliography {
    pub author: String,
    pub company: String,
    pub write_date: String,
}

/// Version information for dashboard files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VersionInfo {
    pub file_format: String,
    pub firmware_signature: Option<String>,
}

/// Embedded image/font resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedImage {
    pub file_name: String,
    pub image_id: String,
    pub resource_type: ResourceType,
    pub data: String, // Base64 encoded
}

/// Type of embedded resource.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum ResourceType {
    #[default]
    Png,
    Gif,
    Ttf,
}

/// Background image display style.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub enum BackgroundStyle {
    #[default]
    Tile,
    Stretch,
    Center,
    Fit,
}

/// Gauge painter type - determines how the gauge is rendered.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum GaugePainter {
    /// Standard analog dial gauge
    AnalogGauge,
    /// Simple analog dial (fewer features)
    BasicAnalogGauge,
    /// Circular analog gauge
    CircleAnalogGauge,
    /// Asymmetric sweep arc gauge
    AsymmetricSweepGauge,
    /// Digital numeric readout
    #[default]
    BasicReadout,
    /// Horizontal progress bar
    HorizontalBarGauge,
    /// Vertical progress bar
    VerticalBarGauge,
    /// Thin horizontal line indicator
    HorizontalLineGauge,
    /// Segmented vertical bar
    VerticalDashedBar,
    /// Analog-style bar gauge
    AnalogBarGauge,
    /// Analog moving bar (high CPU)
    AnalogMovingBarGauge,
    /// Bar histogram
    Histogram,
    /// Scrolling line graph (deferred - not yet implemented)
    LineGraph,
}

impl GaugePainter {
    /// Parse from TunerStudio's GaugePainter string
    pub fn from_ts_string(s: &str) -> Self {
        match s.trim() {
            "Analog Gauge" | "AnalogGaugePainter" => Self::AnalogGauge,
            "Basic Analog Gauge" => Self::BasicAnalogGauge,
            "Circle Analog Gauge" | "CircleAnalogGaugePainter" => Self::CircleAnalogGauge,
            "Asymetric Sweep Gauge" | "AsymetricSweepGaugePainter" => Self::AsymmetricSweepGauge,
            "Basic Readout" | "BasicReadoutGaugePainter" => Self::BasicReadout,
            "Horizontal Bar Gauge" | "HorizontalBarPainter" => Self::HorizontalBarGauge,
            "Vertical Bar Gauge" | "VerticalBarPainter" => Self::VerticalBarGauge,
            "Horizontal Line Gauge" | "HorizontalLinePainter" => Self::HorizontalLineGauge,
            "Vertical Dashed Bar Gauge" | "VerticalDashedBarPainter" => Self::VerticalDashedBar,
            "Analog Bar Gauge" | "AnalogBarPainter" => Self::AnalogBarGauge,
            "Analog Moving Bar Gauge" | "AnalogMovingBarGaugePainter" => Self::AnalogMovingBarGauge,
            "Histogram" | "HistogramPainter" => Self::Histogram,
            "Line Graph" | "LineGraphPainter" => Self::LineGraph,
            _ => Self::BasicReadout, // Default fallback
        }
    }

    /// Convert to TunerStudio's GaugePainter string
    pub fn to_ts_string(&self) -> &'static str {
        match self {
            Self::AnalogGauge => "Analog Gauge",
            Self::BasicAnalogGauge => "Basic Analog Gauge",
            Self::CircleAnalogGauge => "Circle Analog Gauge",
            Self::AsymmetricSweepGauge => "Asymetric Sweep Gauge",
            Self::BasicReadout => "Basic Readout",
            Self::HorizontalBarGauge => "Horizontal Bar Gauge",
            Self::VerticalBarGauge => "Vertical Bar Gauge",
            Self::HorizontalLineGauge => "Horizontal Line Gauge",
            Self::VerticalDashedBar => "Vertical Dashed Bar Gauge",
            Self::AnalogBarGauge => "Analog Bar Gauge",
            Self::AnalogMovingBarGauge => "Analog Moving Bar Gauge",
            Self::Histogram => "Histogram",
            Self::LineGraph => "Line Graph",
        }
    }
}

/// Indicator painter type for boolean indicators.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum IndicatorPainter {
    #[default]
    BasicRectangleIndicator,
}

impl IndicatorPainter {
    pub fn from_ts_string(s: &str) -> Self {
        match s.trim() {
            "Basic Rectangle Indicator" => Self::BasicRectangleIndicator,
            _ => Self::BasicRectangleIndicator,
        }
    }

    pub fn to_ts_string(&self) -> &'static str {
        match self {
            Self::BasicRectangleIndicator => "Basic Rectangle Indicator",
        }
    }
}

/// A gauge component in the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaugeConfig {
    // Identification
    pub id: String,

    // Gauge type
    pub gauge_painter: GaugePainter,
    pub gauge_style: String, // Original style name from file

    // Data binding
    pub output_channel: String,
    pub title: String,
    pub units: String,

    // Current value (for preview/demo)
    pub value: f64,

    // Range
    pub min: f64,
    pub max: f64,
    pub min_vp: Option<String>, // ValueProvider - can be ECU variable name
    pub max_vp: Option<String>,
    pub default_min: Option<f64>,
    pub default_max: Option<f64>,
    pub peg_limits: bool,

    // Warning/critical thresholds
    pub low_warning: Option<f64>,
    pub high_warning: Option<f64>,
    pub low_critical: Option<f64>,
    pub high_critical: Option<f64>,
    pub low_warning_vp: Option<String>,
    pub high_warning_vp: Option<String>,
    pub low_critical_vp: Option<String>,
    pub high_critical_vp: Option<String>,

    // Colors
    pub back_color: TsColor,
    pub font_color: TsColor,
    pub trim_color: TsColor,
    pub warn_color: TsColor,
    pub critical_color: TsColor,
    pub needle_color: TsColor,

    // Display
    pub value_digits: i32,
    pub label_digits: i32,
    pub font_family: String,
    pub font_size_adjustment: i32,
    pub italic_font: bool,

    // Geometry (for analog/sweep gauges)
    pub sweep_angle: i32,
    pub start_angle: i32,
    pub face_angle: i32,
    pub sweep_begin_degree: i32,
    pub counter_clockwise: bool,

    // Tick marks
    pub major_ticks: f64, // -1.0 = auto
    pub minor_ticks: f64,

    // Layout (relative 0.0-1.0)
    pub relative_x: f64,
    pub relative_y: f64,
    pub relative_width: f64,
    pub relative_height: f64,

    // Appearance
    pub border_width: i32,
    pub shortest_size: i32,
    pub shape_locked_to_aspect: bool,
    pub antialiasing_on: bool,

    // Custom images
    pub background_image_file_name: Option<String>,
    pub needle_image_file_name: Option<String>,

    // History/tracking
    pub show_history: bool,
    pub history_value: f64,
    pub history_delay: i32,
    pub needle_smoothing: i32,

    // Interaction
    pub short_click_action: Option<String>,
    pub long_click_action: Option<String>,

    // Display options
    pub display_value_at_180: bool,

    // Runtime state (not persisted)
    #[serde(skip)]
    pub run_demo: bool,
    #[serde(skip)]
    pub must_paint: bool,
    #[serde(skip)]
    pub invalid_state: bool,
    #[serde(skip)]
    pub dirty: bool,
}

impl Default for GaugeConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            gauge_painter: GaugePainter::BasicReadout,
            gauge_style: "Basic Readout".to_string(),
            output_channel: String::new(),
            title: String::new(),
            units: String::new(),
            value: 0.0,
            min: 0.0,
            max: 100.0,
            min_vp: None,
            max_vp: None,
            default_min: None,
            default_max: None,
            peg_limits: true,
            low_warning: None,
            high_warning: None,
            low_critical: None,
            high_critical: None,
            low_warning_vp: None,
            high_warning_vp: None,
            low_critical_vp: None,
            high_critical_vp: None,
            back_color: TsColor::black(),
            font_color: TsColor::white(),
            trim_color: TsColor {
                alpha: 255,
                red: 102,
                green: 102,
                blue: 153,
            },
            warn_color: TsColor::yellow(),
            critical_color: TsColor::red(),
            needle_color: TsColor {
                alpha: 255,
                red: 255,
                green: 102,
                blue: 0,
            },
            value_digits: 0,
            label_digits: 0,
            font_family: String::new(),
            font_size_adjustment: 0,
            italic_font: false,
            sweep_angle: 270,
            start_angle: 135,
            face_angle: 270,
            sweep_begin_degree: 135,
            counter_clockwise: false,
            major_ticks: -1.0,
            minor_ticks: -1.0,
            relative_x: 0.0,
            relative_y: 0.0,
            relative_width: 0.25,
            relative_height: 0.25,
            border_width: 3,
            shortest_size: 50,
            shape_locked_to_aspect: false,
            antialiasing_on: true,
            background_image_file_name: None,
            needle_image_file_name: None,
            show_history: false,
            history_value: 0.0,
            history_delay: 15000,
            needle_smoothing: 1,
            short_click_action: None,
            long_click_action: None,
            display_value_at_180: false,
            run_demo: false,
            must_paint: false,
            invalid_state: false,
            dirty: false,
        }
    }
}

/// A boolean indicator component (warning light).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorConfig {
    // Identification
    pub id: String,
    pub indicator_painter: IndicatorPainter,

    // Data binding
    pub output_channel: String,

    // Current value
    pub value: f64,

    // Text labels
    pub on_text: String,
    pub off_text: String,

    // Text colors
    pub on_text_color: TsColor,
    pub off_text_color: TsColor,

    // Background colors
    pub on_background_color: TsColor,
    pub off_background_color: TsColor,

    // Image files (optional - can replace colors)
    pub on_image_file_name: Option<String>,
    pub off_image_file_name: Option<String>,

    // Layout (relative 0.0-1.0)
    pub relative_x: f64,
    pub relative_y: f64,
    pub relative_width: f64,
    pub relative_height: f64,

    // Display
    pub font_family: String,
    pub italic_font: bool,
    pub antialiasing_on: bool,

    // Interaction
    pub short_click_action: Option<String>,
    pub long_click_action: Option<String>,

    // ECU configuration reference
    pub ecu_configuration_name: Option<String>,

    // Runtime state
    #[serde(skip)]
    pub run_demo: bool,
    #[serde(skip)]
    pub must_paint: bool,
    #[serde(skip)]
    pub invalid_state: bool,
    #[serde(skip)]
    pub dirty: bool,
}

impl Default for IndicatorConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            indicator_painter: IndicatorPainter::BasicRectangleIndicator,
            output_channel: String::new(),
            value: 0.0,
            on_text: "ON".to_string(),
            off_text: "OFF".to_string(),
            on_text_color: TsColor::black(),
            off_text_color: TsColor {
                alpha: 255,
                red: 51,
                green: 51,
                blue: 51,
            },
            on_background_color: TsColor::green(),
            off_background_color: TsColor::transparent(),
            on_image_file_name: None,
            off_image_file_name: None,
            relative_x: 0.0,
            relative_y: 0.0,
            relative_width: 0.1,
            relative_height: 0.05,
            font_family: String::new(),
            italic_font: false,
            antialiasing_on: true,
            short_click_action: None,
            long_click_action: None,
            ecu_configuration_name: None,
            run_demo: false,
            must_paint: false,
            invalid_state: false,
            dirty: false,
        }
    }
}

/// A dashboard component - either a gauge or an indicator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DashComponent {
    Gauge(GaugeConfig),
    Indicator(IndicatorConfig),
}

/// Gauge cluster - container for all dashboard components.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaugeCluster {
    /// Enable anti-aliasing for all gauges
    pub anti_aliasing: bool,
    /// Background dither color
    pub background_dither_color: Option<TsColor>,
    /// Cluster background color
    pub cluster_background_color: TsColor,
    /// Background image file name (reference to embedded or external)
    pub cluster_background_image_file_name: Option<String>,
    /// Background image display style
    pub cluster_background_image_style: BackgroundStyle,
    /// Embedded image/font resources
    pub embedded_images: Vec<EmbeddedImage>,
    /// Dashboard components (gauges and indicators)
    pub components: Vec<DashComponent>,
}

impl Default for GaugeCluster {
    fn default() -> Self {
        Self {
            anti_aliasing: true,
            background_dither_color: None,
            cluster_background_color: TsColor::black(),
            cluster_background_image_file_name: None,
            cluster_background_image_style: BackgroundStyle::Tile,
            embedded_images: Vec::new(),
            components: Vec::new(),
        }
    }
}

/// Top-level dashboard file structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashFile {
    /// File metadata
    pub bibliography: Bibliography,
    /// Version information
    pub version_info: VersionInfo,
    /// Gauge cluster (main content)
    pub gauge_cluster: GaugeCluster,
}

impl Default for DashFile {
    fn default() -> Self {
        Self {
            bibliography: Bibliography {
                author: "LibreTune".to_string(),
                company: "LibreTune Project".to_string(),
                write_date: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            },
            version_info: VersionInfo {
                file_format: "3.0".to_string(),
                firmware_signature: None,
            },
            gauge_cluster: GaugeCluster::default(),
        }
    }
}

/// Top-level gauge template file structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaugeFile {
    /// File metadata
    pub bibliography: Bibliography,
    /// Version information
    pub version_info: VersionInfo,
    /// Embedded resources
    pub embedded_images: Vec<EmbeddedImage>,
    /// The gauge template
    pub gauge: GaugeConfig,
}

impl Default for GaugeFile {
    fn default() -> Self {
        Self {
            bibliography: Bibliography {
                author: "LibreTune".to_string(),
                company: "LibreTune Project".to_string(),
                write_date: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            },
            version_info: VersionInfo {
                file_format: "1.0".to_string(),
                firmware_signature: None,
            },
            embedded_images: Vec::new(),
            gauge: GaugeConfig::default(),
        }
    }
}

/// Lookup map for embedded images by ID.
pub type EmbeddedImageMap = HashMap<String, EmbeddedImage>;
