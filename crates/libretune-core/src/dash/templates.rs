//! Default dashboard templates for LibreTune.
//!
//! This module provides pre-configured dashboard layouts that match
//! common ECU tuning workflows with professional visual design.

use super::{
    BackgroundStyle, Bibliography, DashComponent, DashFile, GaugeCluster, GaugeConfig,
    GaugePainter, TsColor, VersionInfo,
};
use chrono;

// LibreTune brand colors - consistent dark theme with vibrant accents
const LT_DARKER_BG: TsColor = TsColor {
    alpha: 255,
    red: 12,
    green: 14,
    blue: 20,
};
const LT_GAUGE_BG: TsColor = TsColor {
    alpha: 255,
    red: 28,
    green: 32,
    blue: 40,
};
const LT_ACCENT_BLUE: TsColor = TsColor {
    alpha: 255,
    red: 74,
    green: 158,
    blue: 248,
};
const LT_ACCENT_TEAL: TsColor = TsColor {
    alpha: 255,
    red: 56,
    green: 189,
    blue: 248,
};
const LT_ACCENT_AMBER: TsColor = TsColor {
    alpha: 255,
    red: 251,
    green: 191,
    blue: 36,
};
const LT_ACCENT_GREEN: TsColor = TsColor {
    alpha: 255,
    red: 34,
    green: 197,
    blue: 94,
};
const LT_ACCENT_RED: TsColor = TsColor {
    alpha: 255,
    red: 239,
    green: 68,
    blue: 68,
};
const LT_TEXT_PRIMARY: TsColor = TsColor {
    alpha: 255,
    red: 255,
    green: 255,
    blue: 255,
};
const LT_TEXT_SECONDARY: TsColor = TsColor {
    alpha: 255,
    red: 148,
    green: 163,
    blue: 184,
};
const LT_WARN_COLOR: TsColor = TsColor {
    alpha: 255,
    red: 234,
    green: 179,
    blue: 8,
};
const LT_CRITICAL_COLOR: TsColor = TsColor {
    alpha: 255,
    red: 239,
    green: 68,
    blue: 68,
};

// Data Logger template — dense, Grafana/ops-console inspired palette: dark
// slate background + a distinct multi-series color per channel so many
// overlaid lines stay readable.
const LT_LOG_BG: TsColor = TsColor {
    alpha: 255,
    red: 15,
    green: 17,
    blue: 20,
};
const LT_LOG_PANEL_BG: TsColor = TsColor {
    alpha: 255,
    red: 26,
    green: 28,
    blue: 33,
};
const LT_LOG_WHITE: TsColor = TsColor {
    alpha: 255,
    red: 230,
    green: 230,
    blue: 235,
};
const LT_LOG_GRAY: TsColor = TsColor {
    alpha: 255,
    red: 140,
    green: 145,
    blue: 155,
};
const LT_LOG_BLUE: TsColor = TsColor {
    alpha: 255,
    red: 87,
    green: 148,
    blue: 242,
};
const LT_LOG_GREEN: TsColor = TsColor {
    alpha: 255,
    red: 115,
    green: 191,
    blue: 105,
};
const LT_LOG_ORANGE: TsColor = TsColor {
    alpha: 255,
    red: 255,
    green: 152,
    blue: 48,
};
const LT_LOG_RED: TsColor = TsColor {
    alpha: 255,
    red: 242,
    green: 73,
    blue: 92,
};
const LT_LOG_PURPLE: TsColor = TsColor {
    alpha: 255,
    red: 184,
    green: 119,
    blue: 217,
};
const LT_LOG_YELLOW: TsColor = TsColor {
    alpha: 255,
    red: 250,
    green: 222,
    blue: 42,
};
const LT_LOG_CYAN: TsColor = TsColor {
    alpha: 255,
    red: 51,
    green: 178,
    blue: 255,
};

/// Create a basic dashboard layout - LibreTune default
/// Clean 4x2 grid: Large RPM + AFR in center, supporting gauges around edges
/// Perfect for general monitoring and everyday driving
pub fn create_basic_dashboard() -> DashFile {
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
            cluster_background_color: LT_DARKER_BG,
            background_dither_color: None,
            cluster_background_image_file_name: None,
            cluster_background_image_style: BackgroundStyle::Stretch,
            embedded_images: Vec::new(),
            components: Vec::new(),
            cluster_layout: None,
            enabled_condition: None,
            extra_attrs: std::collections::BTreeMap::new(),
        },
        additional_clusters: Vec::new(),
        extra_attrs: std::collections::BTreeMap::new(),
    };

    // CENTER LEFT: Large RPM tachometer
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "rpm".to_string(),
            title: "ENGINE RPM".to_string(),
            units: "".to_string(),
            output_channel: "rpm".to_string(),
            min: 0.0,
            max: 8000.0,
            high_warning: Some(6500.0),
            high_critical: Some(7200.0),
            gauge_painter: GaugePainter::Tachometer,
            start_angle: 135,
            sweep_angle: 270,
            major_ticks: 8.0,
            minor_ticks: 4.0,
            value_digits: 0,
            relative_x: 0.02,
            relative_y: 0.10,
            relative_width: 0.45,
            relative_height: 0.80,
            back_color: LT_GAUGE_BG,
            font_color: LT_TEXT_PRIMARY,
            needle_color: LT_ACCENT_AMBER,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: 2,
            ..Default::default()
        })));

    // CENTER RIGHT: Large AFR gauge
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "afr".to_string(),
            title: "AIR/FUEL RATIO".to_string(),
            units: ":1".to_string(),
            output_channel: "afr".to_string(),
            min: 10.0,
            max: 20.0,
            low_warning: Some(11.5),
            low_critical: Some(10.5),
            high_warning: Some(16.0),
            value_digits: 1,
            gauge_painter: GaugePainter::AnalogGauge,
            start_angle: 225,
            sweep_angle: 270,
            major_ticks: 10.0,
            minor_ticks: 5.0,
            relative_x: 0.52,
            relative_y: 0.10,
            relative_width: 0.46,
            relative_height: 0.80,
            back_color: LT_GAUGE_BG,
            font_color: LT_ACCENT_GREEN,
            needle_color: LT_ACCENT_GREEN,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: 1,
            ..Default::default()
        })));

    // TOP LEFT: Coolant temp bar
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "coolant".to_string(),
            title: "COOLANT".to_string(),
            units: "°C".to_string(),
            output_channel: "coolant".to_string(),
            min: -40.0,
            max: 120.0,
            high_warning: Some(100.0),
            high_critical: Some(110.0),
            value_digits: 0,
            gauge_painter: GaugePainter::HorizontalBarGauge,
            relative_x: 0.02,
            relative_y: 0.02,
            relative_width: 0.23,
            relative_height: 0.06,
            back_color: LT_GAUGE_BG,
            font_color: LT_ACCENT_BLUE,
            needle_color: LT_ACCENT_BLUE,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            ..Default::default()
        })));

    // TOP CENTER: MAP bar
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "map".to_string(),
            title: "MAP".to_string(),
            units: "kPa".to_string(),
            output_channel: "map".to_string(),
            min: 0.0,
            max: 250.0,
            high_warning: Some(200.0),
            value_digits: 0,
            gauge_painter: GaugePainter::HorizontalBarGauge,
            relative_x: 0.27,
            relative_y: 0.02,
            relative_width: 0.23,
            relative_height: 0.06,
            back_color: LT_GAUGE_BG,
            font_color: LT_ACCENT_TEAL,
            needle_color: LT_ACCENT_TEAL,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            ..Default::default()
        })));

    // TOP RIGHT: TPS bar
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "tps".to_string(),
            title: "THROTTLE".to_string(),
            units: "%".to_string(),
            output_channel: "tps".to_string(),
            min: 0.0,
            max: 100.0,
            value_digits: 0,
            gauge_painter: GaugePainter::HorizontalBarGauge,
            relative_x: 0.52,
            relative_y: 0.02,
            relative_width: 0.23,
            relative_height: 0.06,
            back_color: LT_GAUGE_BG,
            font_color: LT_ACCENT_AMBER,
            needle_color: LT_ACCENT_AMBER,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            ..Default::default()
        })));

    // TOP FAR RIGHT: Battery readout
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "battery".to_string(),
            title: "BATT".to_string(),
            units: "V".to_string(),
            output_channel: "battery".to_string(),
            min: 10.0,
            max: 16.0,
            low_warning: Some(11.5),
            low_critical: Some(11.0),
            value_digits: 1,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.77,
            relative_y: 0.02,
            relative_width: 0.21,
            relative_height: 0.06,
            back_color: LT_GAUGE_BG,
            font_color: LT_TEXT_PRIMARY,
            needle_color: LT_ACCENT_GREEN,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: -1,
            ..Default::default()
        })));

    // BOTTOM LEFT: IAT readout
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "iat".to_string(),
            title: "INTAKE TEMP".to_string(),
            units: "°C".to_string(),
            output_channel: "iat".to_string(),
            min: -40.0,
            max: 80.0,
            high_warning: Some(50.0),
            value_digits: 0,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.02,
            relative_y: 0.92,
            relative_width: 0.23,
            relative_height: 0.06,
            back_color: LT_GAUGE_BG,
            font_color: LT_TEXT_PRIMARY,
            needle_color: LT_ACCENT_AMBER,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: -1,
            ..Default::default()
        })));

    // BOTTOM CENTER: Ignition advance
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "advance".to_string(),
            title: "TIMING".to_string(),
            units: "°".to_string(),
            output_channel: "advance".to_string(),
            min: -10.0,
            max: 50.0,
            value_digits: 1,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.27,
            relative_y: 0.92,
            relative_width: 0.23,
            relative_height: 0.06,
            back_color: LT_GAUGE_BG,
            font_color: LT_TEXT_PRIMARY,
            needle_color: LT_ACCENT_TEAL,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: -1,
            ..Default::default()
        })));

    // BOTTOM CENTER-RIGHT: VE percentage
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "ve".to_string(),
            title: "VE".to_string(),
            units: "%".to_string(),
            output_channel: "ve".to_string(),
            min: 0.0,
            max: 150.0,
            value_digits: 0,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.52,
            relative_y: 0.92,
            relative_width: 0.23,
            relative_height: 0.06,
            back_color: LT_GAUGE_BG,
            font_color: LT_TEXT_PRIMARY,
            needle_color: LT_ACCENT_GREEN,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: -1,
            ..Default::default()
        })));

    // BOTTOM RIGHT: Pulse width
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "pw".to_string(),
            title: "PULSE".to_string(),
            units: "ms".to_string(),
            output_channel: "pulseWidth".to_string(),
            min: 0.0,
            max: 25.0,
            value_digits: 2,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.77,
            relative_y: 0.92,
            relative_width: 0.21,
            relative_height: 0.06,
            back_color: LT_GAUGE_BG,
            font_color: LT_TEXT_PRIMARY,
            needle_color: LT_ACCENT_BLUE,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: -1,
            ..Default::default()
        })));

    dash
}


/// Create a tuning-focused dashboard
/// Create a tuning-focused dashboard
/// Professional layout optimized for live tuning sessions
/// Shows all critical metrics for VE/fuel table tuning
pub fn create_tuning_dashboard() -> DashFile {
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
            cluster_background_color: LT_DARKER_BG,
            background_dither_color: None,
            cluster_background_image_file_name: None,
            cluster_background_image_style: BackgroundStyle::Stretch,
            embedded_images: Vec::new(),
            components: Vec::new(),
            cluster_layout: None,
            enabled_condition: None,
            extra_attrs: std::collections::BTreeMap::new(),
        },
        additional_clusters: Vec::new(),
        extra_attrs: std::collections::BTreeMap::new(),
    };

    // TOP ROW: Primary tuning metrics

    // RPM - sweep gauge (top left)
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "rpm".to_string(),
            title: "RPM".to_string(),
            units: "".to_string(),
            output_channel: "rpm".to_string(),
            min: 0.0,
            max: 8000.0,
            high_warning: Some(6500.0),
            high_critical: Some(7200.0),
            value_digits: 0,
            gauge_painter: GaugePainter::AsymmetricSweepGauge,
            start_angle: 180,
            sweep_angle: 180,
            relative_x: 0.02,
            relative_y: 0.02,
            relative_width: 0.30,
            relative_height: 0.30,
            back_color: LT_GAUGE_BG,
            font_color: LT_TEXT_PRIMARY,
            needle_color: LT_ACCENT_TEAL,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: 1,
            ..Default::default()
        })));

    // AFR - analog gauge (top center)
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "afr".to_string(),
            title: "AFR".to_string(),
            units: ":1".to_string(),
            output_channel: "afr".to_string(),
            min: 10.0,
            max: 20.0,
            low_warning: Some(11.5),
            low_critical: Some(10.5),
            high_warning: Some(16.0),
            value_digits: 2,
            gauge_painter: GaugePainter::RoundGauge,
            start_angle: 135,
            sweep_angle: 270,
            major_ticks: 10.0,
            minor_ticks: 5.0,
            relative_x: 0.34,
            relative_y: 0.02,
            relative_width: 0.30,
            relative_height: 0.30,
            back_color: LT_GAUGE_BG,
            font_color: LT_ACCENT_GREEN,
            needle_color: LT_ACCENT_GREEN,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: 1,
            ..Default::default()
        })));

    // MAP - horizontal bar (top right)
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "map".to_string(),
            title: "MAP".to_string(),
            units: "kPa".to_string(),
            output_channel: "map".to_string(),
            min: 0.0,
            max: 250.0,
            high_warning: Some(200.0),
            value_digits: 0,
            gauge_painter: GaugePainter::HorizontalBarGauge,
            relative_x: 0.66,
            relative_y: 0.02,
            relative_width: 0.32,
            relative_height: 0.12,
            back_color: LT_GAUGE_BG,
            font_color: LT_ACCENT_TEAL,
            needle_color: LT_ACCENT_TEAL,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: 0,
            ..Default::default()
        })));

    // TPS - horizontal bar (top right, below MAP)
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "tps".to_string(),
            title: "TPS".to_string(),
            units: "%".to_string(),
            output_channel: "tps".to_string(),
            min: 0.0,
            max: 100.0,
            value_digits: 0,
            gauge_painter: GaugePainter::HorizontalBarGauge,
            relative_x: 0.66,
            relative_y: 0.16,
            relative_width: 0.32,
            relative_height: 0.12,
            back_color: LT_GAUGE_BG,
            font_color: LT_ACCENT_AMBER,
            needle_color: LT_ACCENT_AMBER,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: 0,
            ..Default::default()
        })));

    // MIDDLE ROW: Temperature monitoring

    // Coolant - vertical bar (left)
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "coolant".to_string(),
            title: "COOLANT".to_string(),
            units: "°C".to_string(),
            output_channel: "coolant".to_string(),
            min: -40.0,
            max: 120.0,
            high_warning: Some(100.0),
            high_critical: Some(110.0),
            value_digits: 0,
            gauge_painter: GaugePainter::VerticalBarGauge,
            relative_x: 0.02,
            relative_y: 0.35,
            relative_width: 0.14,
            relative_height: 0.38,
            back_color: LT_GAUGE_BG,
            font_color: LT_ACCENT_BLUE,
            needle_color: LT_ACCENT_BLUE,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: 0,
            ..Default::default()
        })));

    // IAT - vertical bar (next to coolant)
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "iat".to_string(),
            title: "IAT".to_string(),
            units: "°C".to_string(),
            output_channel: "iat".to_string(),
            min: -40.0,
            max: 80.0,
            high_warning: Some(50.0),
            value_digits: 0,
            gauge_painter: GaugePainter::VerticalBarGauge,
            relative_x: 0.18,
            relative_y: 0.35,
            relative_width: 0.14,
            relative_height: 0.38,
            back_color: LT_GAUGE_BG,
            font_color: LT_ACCENT_AMBER,
            needle_color: LT_ACCENT_AMBER,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: 0,
            ..Default::default()
        })));

    // Lambda trend - line graph (center section)
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "lambda_hist".to_string(),
            title: "LAMBDA TREND".to_string(),
            units: "λ".to_string(),
            output_channel: "lambda".to_string(),
            min: 0.7,
            max: 1.3,
            low_warning: Some(0.75),
            high_warning: Some(1.1),
            value_digits: 3,
            gauge_painter: GaugePainter::LineGraph,
            relative_x: 0.34,
            relative_y: 0.35,
            relative_width: 0.64,
            relative_height: 0.38,
            back_color: LT_GAUGE_BG,
            font_color: LT_ACCENT_GREEN,
            needle_color: LT_ACCENT_GREEN,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            show_history: true,
            ..Default::default()
        })));

    // BOTTOM ROW: Tuning-specific metrics (all digital readouts)

    // VE percentage
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "ve".to_string(),
            title: "VE".to_string(),
            units: "%".to_string(),
            output_channel: "ve".to_string(),
            min: 0.0,
            max: 150.0,
            value_digits: 0,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.02,
            relative_y: 0.76,
            relative_width: 0.14,
            relative_height: 0.11,
            back_color: LT_GAUGE_BG,
            font_color: LT_TEXT_PRIMARY,
            needle_color: LT_ACCENT_GREEN,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: 1,
            ..Default::default()
        })));

    // Pulse width
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "pw".to_string(),
            title: "PULSE".to_string(),
            units: "ms".to_string(),
            output_channel: "pulseWidth".to_string(),
            min: 0.0,
            max: 25.0,
            high_warning: Some(20.0),
            value_digits: 2,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.18,
            relative_y: 0.76,
            relative_width: 0.14,
            relative_height: 0.11,
            back_color: LT_GAUGE_BG,
            font_color: LT_TEXT_PRIMARY,
            needle_color: LT_ACCENT_BLUE,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: 1,
            ..Default::default()
        })));

    // Injector duty cycle
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "duty".to_string(),
            title: "DUTY".to_string(),
            units: "%".to_string(),
            output_channel: "dutyCycle".to_string(),
            min: 0.0,
            max: 100.0,
            high_warning: Some(85.0),
            high_critical: Some(95.0),
            value_digits: 0,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.34,
            relative_y: 0.76,
            relative_width: 0.14,
            relative_height: 0.11,
            back_color: LT_GAUGE_BG,
            font_color: LT_TEXT_PRIMARY,
            needle_color: LT_ACCENT_AMBER,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: 1,
            ..Default::default()
        })));

    // Ignition advance
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "advance".to_string(),
            title: "TIMING".to_string(),
            units: "°".to_string(),
            output_channel: "advance".to_string(),
            min: -10.0,
            max: 50.0,
            value_digits: 1,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.50,
            relative_y: 0.76,
            relative_width: 0.14,
            relative_height: 0.11,
            back_color: LT_GAUGE_BG,
            font_color: LT_TEXT_PRIMARY,
            needle_color: LT_ACCENT_TEAL,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: 1,
            ..Default::default()
        })));

    // Battery voltage
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "battery".to_string(),
            title: "BATT".to_string(),
            units: "V".to_string(),
            output_channel: "battery".to_string(),
            min: 10.0,
            max: 16.0,
            low_warning: Some(11.5),
            low_critical: Some(11.0),
            value_digits: 1,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.66,
            relative_y: 0.76,
            relative_width: 0.14,
            relative_height: 0.11,
            back_color: LT_GAUGE_BG,
            font_color: LT_TEXT_PRIMARY,
            needle_color: LT_ACCENT_GREEN,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: 1,
            ..Default::default()
        })));

    // EGT or knock count
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "egt".to_string(),
            title: "EGT".to_string(),
            units: "°C".to_string(),
            output_channel: "egt".to_string(),
            min: 0.0,
            max: 1000.0,
            high_warning: Some(850.0),
            high_critical: Some(950.0),
            value_digits: 0,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.82,
            relative_y: 0.76,
            relative_width: 0.16,
            relative_height: 0.11,
            back_color: LT_GAUGE_BG,
            font_color: LT_TEXT_PRIMARY,
            needle_color: LT_ACCENT_RED,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: 1,
            ..Default::default()
        })));

    // AFR target readout (bottom second row, left)
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "afrtarget".to_string(),
            title: "AFR TARGET".to_string(),
            units: ":1".to_string(),
            output_channel: "afrTarget".to_string(),
            min: 10.0,
            max: 20.0,
            value_digits: 1,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.02,
            relative_y: 0.89,
            relative_width: 0.14,
            relative_height: 0.09,
            back_color: LT_GAUGE_BG,
            font_color: LT_TEXT_SECONDARY,
            needle_color: LT_TEXT_SECONDARY,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: -1,
            ..Default::default()
        })));

    // Correction factor readout
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "corr".to_string(),
            title: "CORRECTION".to_string(),
            units: "%".to_string(),
            output_channel: "correction".to_string(),
            min: 0.0,
            max: 200.0,
            value_digits: 0,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.18,
            relative_y: 0.89,
            relative_width: 0.14,
            relative_height: 0.09,
            back_color: LT_GAUGE_BG,
            font_color: LT_TEXT_SECONDARY,
            needle_color: LT_TEXT_SECONDARY,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: -1,
            ..Default::default()
        })));

    dash
}

// ---------------------------------------------------------------------------
// Telemetry Live — dense Grafana-style live data dashboard
// ---------------------------------------------------------------------------

/// Compact live-value tile for the telemetry grid.
struct LogStatSpec {
    id: &'static str,
    title: &'static str,
    channel: &'static str,
    units: &'static str,
    min: f64,
    max: f64,
    digits: i32,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    color: TsColor,
}

/// Single-channel scrolling sparkline panel.
struct LogSparkSpec {
    id: &'static str,
    title: &'static str,
    channel: &'static str,
    units: &'static str,
    min: f64,
    max: f64,
    digits: i32,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    color: TsColor,
}

/// One overlay series on a [`MultiChannelTrend`] panel (series 2+).
struct LogSeriesEntry {
    channel: &'static str,
    label: &'static str,
    color: &'static str,
    min: f64,
    max: f64,
}

fn log_stat_tile(spec: LogStatSpec) -> DashComponent {
    DashComponent::Gauge(Box::new(GaugeConfig {
        id: spec.id.to_string(),
        title: spec.title.to_string(),
        units: spec.units.to_string(),
        output_channel: spec.channel.to_string(),
        min: spec.min,
        max: spec.max,
        value_digits: spec.digits,
        gauge_painter: GaugePainter::TelemetryStat,
        relative_x: spec.x,
        relative_y: spec.y,
        relative_width: spec.w,
        relative_height: spec.h,
        back_color: LT_LOG_PANEL_BG,
        font_color: spec.color.clone(),
        needle_color: spec.color,
        trim_color: LT_LOG_GRAY,
        warn_color: LT_WARN_COLOR,
        critical_color: LT_CRITICAL_COLOR,
        border_width: 1,
        font_size_adjustment: -1,
        ..Default::default()
    }))
}

fn log_sparkline(spec: LogSparkSpec) -> DashComponent {
    DashComponent::Gauge(Box::new(GaugeConfig {
        id: spec.id.to_string(),
        title: spec.title.to_string(),
        units: spec.units.to_string(),
        output_channel: spec.channel.to_string(),
        min: spec.min,
        max: spec.max,
        value_digits: spec.digits,
        gauge_painter: GaugePainter::LineGraph,
        relative_x: spec.x,
        relative_y: spec.y,
        relative_width: spec.w,
        relative_height: spec.h,
        back_color: LT_LOG_PANEL_BG,
        font_color: spec.color.clone(),
        needle_color: spec.color,
        trim_color: LT_LOG_GRAY,
        warn_color: LT_WARN_COLOR,
        critical_color: LT_CRITICAL_COLOR,
        show_history: true,
        border_width: 1,
        ..Default::default()
    }))
}

fn log_series_attrs(extra: &[LogSeriesEntry]) -> std::collections::BTreeMap<String, String> {
    let mut attrs = std::collections::BTreeMap::new();
    for (idx, entry) in extra.iter().enumerate() {
        let n = idx + 2;
        attrs.insert(format!("lt_series{n}_channel"), entry.channel.to_string());
        attrs.insert(format!("lt_series{n}_label"), entry.label.to_string());
        attrs.insert(format!("lt_series{n}_color"), entry.color.to_string());
        attrs.insert(format!("lt_series{n}_min"), entry.min.to_string());
        attrs.insert(format!("lt_series{n}_max"), entry.max.to_string());
    }
    attrs
}

#[allow(clippy::too_many_arguments)]
fn log_multi_trend(
    id: &str,
    title: &str,
    primary: LogSeriesEntry,
    extra: &[LogSeriesEntry],
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> DashComponent {
    let color = TsColor::from_css_hex(primary.color).unwrap_or(LT_LOG_BLUE);
    DashComponent::Gauge(Box::new(GaugeConfig {
        id: id.to_string(),
        title: title.to_string(),
        units: String::new(),
        output_channel: primary.channel.to_string(),
        min: primary.min,
        max: primary.max,
        value_digits: 1,
        gauge_painter: GaugePainter::MultiChannelTrend,
        relative_x: x,
        relative_y: y,
        relative_width: w,
        relative_height: h,
        back_color: LT_LOG_PANEL_BG,
        font_color: color.clone(),
        needle_color: color,
        trim_color: LT_LOG_GRAY,
        warn_color: LT_WARN_COLOR,
        critical_color: LT_CRITICAL_COLOR,
        extra_attrs: log_series_attrs(extra),
        border_width: 1,
        ..Default::default()
    }))
}

/// Dense live telemetry dashboard — Grafana / ops-console inspired layout
/// showing dozens of channels at once: compact stat tiles, multi-series trend
/// overlays, and a wall of scrolling sparkline charts (like viewing a log live).
pub fn create_telemetry_live_dashboard() -> DashFile {
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
            force_aspect: true,
            force_aspect_width: 16.0,
            force_aspect_height: 10.0,
            cluster_background_color: LT_LOG_BG,
            background_dither_color: None,
            cluster_background_image_file_name: None,
            cluster_background_image_style: BackgroundStyle::Stretch,
            embedded_images: Vec::new(),
            components: Vec::new(),
            cluster_layout: None,
            enabled_condition: None,
            extra_attrs: std::collections::BTreeMap::new(),
        },
        additional_clusters: Vec::new(),
        extra_attrs: std::collections::BTreeMap::new(),
    };

    // --- Top KPI strip (10 channels) -----------------------------------------
    let top_stats = [
        LogStatSpec {
            id: "tl_rpm",
            title: "RPM",
            channel: "rpm",
            units: "",
            min: 0.0,
            max: 9000.0,
            digits: 0,
            x: 0.005,
            y: 0.005,
            w: 0.095,
            h: 0.048,
            color: LT_LOG_BLUE,
        },
        LogStatSpec {
            id: "tl_map",
            title: "MAP",
            channel: "map",
            units: "kPa",
            min: 0.0,
            max: 250.0,
            digits: 0,
            x: 0.105,
            y: 0.005,
            w: 0.095,
            h: 0.048,
            color: LT_LOG_CYAN,
        },
        LogStatSpec {
            id: "tl_tps",
            title: "TPS",
            channel: "tps",
            units: "%",
            min: 0.0,
            max: 100.0,
            digits: 0,
            x: 0.205,
            y: 0.005,
            w: 0.095,
            h: 0.048,
            color: LT_LOG_ORANGE,
        },
        LogStatSpec {
            id: "tl_afr",
            title: "AFR",
            channel: "afr",
            units: ":1",
            min: 10.0,
            max: 20.0,
            digits: 1,
            x: 0.305,
            y: 0.005,
            w: 0.095,
            h: 0.048,
            color: LT_LOG_GREEN,
        },
        LogStatSpec {
            id: "tl_lam",
            title: "LAM",
            channel: "lambda",
            units: "λ",
            min: 0.7,
            max: 1.3,
            digits: 3,
            x: 0.405,
            y: 0.005,
            w: 0.095,
            h: 0.048,
            color: LT_LOG_GREEN,
        },
        LogStatSpec {
            id: "tl_clt",
            title: "CLT",
            channel: "coolant",
            units: "°C",
            min: -20.0,
            max: 120.0,
            digits: 0,
            x: 0.505,
            y: 0.005,
            w: 0.095,
            h: 0.048,
            color: LT_LOG_BLUE,
        },
        LogStatSpec {
            id: "tl_iat",
            title: "IAT",
            channel: "iat",
            units: "°C",
            min: -20.0,
            max: 80.0,
            digits: 0,
            x: 0.605,
            y: 0.005,
            w: 0.095,
            h: 0.048,
            color: LT_LOG_ORANGE,
        },
        LogStatSpec {
            id: "tl_spd",
            title: "SPD",
            channel: "speed",
            units: "km/h",
            min: 0.0,
            max: 260.0,
            digits: 0,
            x: 0.705,
            y: 0.005,
            w: 0.095,
            h: 0.048,
            color: LT_LOG_WHITE,
        },
        LogStatSpec {
            id: "tl_bat",
            title: "BATT",
            channel: "battery",
            units: "V",
            min: 10.0,
            max: 16.0,
            digits: 1,
            x: 0.805,
            y: 0.005,
            w: 0.095,
            h: 0.048,
            color: LT_LOG_YELLOW,
        },
        LogStatSpec {
            id: "tl_duty",
            title: "DUTY",
            channel: "dutyCycle",
            units: "%",
            min: 0.0,
            max: 100.0,
            digits: 0,
            x: 0.905,
            y: 0.005,
            w: 0.090,
            h: 0.048,
            color: LT_LOG_PURPLE,
        },
    ];
    for spec in top_stats {
        dash.gauge_cluster.components.push(log_stat_tile(spec));
    }

    // --- Left stat column (12 channels) --------------------------------------
    let left_stats = [
        LogStatSpec {
            id: "tl_ve",
            title: "VE",
            channel: "ve",
            units: "%",
            min: 0.0,
            max: 150.0,
            digits: 0,
            x: 0.005,
            y: 0.060,
            w: 0.118,
            h: 0.034,
            color: LT_LOG_GREEN,
        },
        LogStatSpec {
            id: "tl_pw",
            title: "PW",
            channel: "pulseWidth",
            units: "ms",
            min: 0.0,
            max: 25.0,
            digits: 2,
            x: 0.005,
            y: 0.097,
            w: 0.118,
            h: 0.034,
            color: LT_LOG_BLUE,
        },
        LogStatSpec {
            id: "tl_adv",
            title: "ADV",
            channel: "advance",
            units: "°",
            min: -10.0,
            max: 50.0,
            digits: 1,
            x: 0.005,
            y: 0.134,
            w: 0.118,
            h: 0.034,
            color: LT_LOG_CYAN,
        },
        LogStatSpec {
            id: "tl_bst",
            title: "BOOST",
            channel: "boost",
            units: "kPa",
            min: 0.0,
            max: 300.0,
            digits: 0,
            x: 0.005,
            y: 0.171,
            w: 0.118,
            h: 0.034,
            color: LT_LOG_ORANGE,
        },
        LogStatSpec {
            id: "tl_baro",
            title: "BARO",
            channel: "baro",
            units: "kPa",
            min: 70.0,
            max: 110.0,
            digits: 0,
            x: 0.005,
            y: 0.208,
            w: 0.118,
            h: 0.034,
            color: LT_LOG_GRAY,
        },
        LogStatSpec {
            id: "tl_oilp",
            title: "OIL P",
            channel: "oilPressure",
            units: "kPa",
            min: 0.0,
            max: 700.0,
            digits: 0,
            x: 0.005,
            y: 0.245,
            w: 0.118,
            h: 0.034,
            color: LT_LOG_YELLOW,
        },
        LogStatSpec {
            id: "tl_oilt",
            title: "OIL T",
            channel: "oilTemp",
            units: "°C",
            min: 0.0,
            max: 150.0,
            digits: 0,
            x: 0.005,
            y: 0.282,
            w: 0.118,
            h: 0.034,
            color: LT_LOG_ORANGE,
        },
        LogStatSpec {
            id: "tl_egt",
            title: "EGT",
            channel: "egt",
            units: "°C",
            min: 0.0,
            max: 1000.0,
            digits: 0,
            x: 0.005,
            y: 0.319,
            w: 0.118,
            h: 0.034,
            color: LT_LOG_RED,
        },
        LogStatSpec {
            id: "tl_corr",
            title: "CORR",
            channel: "correction",
            units: "%",
            min: 50.0,
            max: 150.0,
            digits: 0,
            x: 0.005,
            y: 0.356,
            w: 0.118,
            h: 0.034,
            color: LT_LOG_GRAY,
        },
        LogStatSpec {
            id: "tl_sync",
            title: "SYNC",
            channel: "sync",
            units: "",
            min: 0.0,
            max: 1.0,
            digits: 0,
            x: 0.005,
            y: 0.393,
            w: 0.118,
            h: 0.034,
            color: LT_LOG_GREEN,
        },
        LogStatSpec {
            id: "tl_fuel",
            title: "FUEL",
            channel: "fuelLevel",
            units: "%",
            min: 0.0,
            max: 100.0,
            digits: 0,
            x: 0.005,
            y: 0.430,
            w: 0.118,
            h: 0.034,
            color: LT_LOG_YELLOW,
        },
        LogStatSpec {
            id: "tl_tgt",
            title: "AFR TGT",
            channel: "afrTarget",
            units: ":1",
            min: 10.0,
            max: 20.0,
            digits: 1,
            x: 0.005,
            y: 0.467,
            w: 0.118,
            h: 0.034,
            color: LT_LOG_GRAY,
        },
    ];
    for spec in left_stats {
        dash.gauge_cluster.components.push(log_stat_tile(spec));
    }

    // --- Multi-series trend panels (Grafana-style dense overlays) ------------
    dash.gauge_cluster.components.push(log_multi_trend(
        "tl_trend_engine",
        "ENGINE DYNAMICS",
        LogSeriesEntry {
            channel: "rpm",
            label: "RPM",
            color: "#5794f2",
            min: 0.0,
            max: 9000.0,
        },
        &[
            LogSeriesEntry {
                channel: "map",
                label: "MAP",
                color: "#33b5ff",
                min: 0.0,
                max: 250.0,
            },
            LogSeriesEntry {
                channel: "tps",
                label: "TPS",
                color: "#ff9830",
                min: 0.0,
                max: 100.0,
            },
            LogSeriesEntry {
                channel: "speed",
                label: "SPD",
                color: "#e6e6eb",
                min: 0.0,
                max: 260.0,
            },
            LogSeriesEntry {
                channel: "dutyCycle",
                label: "DUTY",
                color: "#b877d9",
                min: 0.0,
                max: 100.0,
            },
            LogSeriesEntry {
                channel: "advance",
                label: "ADV",
                color: "#73bf69",
                min: -10.0,
                max: 50.0,
            },
            LogSeriesEntry {
                channel: "boost",
                label: "BOOST",
                color: "#f2495c",
                min: 0.0,
                max: 300.0,
            },
            LogSeriesEntry {
                channel: "ve",
                label: "VE",
                color: "#fade2a",
                min: 0.0,
                max: 150.0,
            },
        ],
        0.130,
        0.060,
        0.430,
        0.210,
    ));

    dash.gauge_cluster.components.push(log_multi_trend(
        "tl_trend_fuel",
        "FUEL & AFR",
        LogSeriesEntry {
            channel: "afr",
            label: "AFR",
            color: "#73bf69",
            min: 10.0,
            max: 20.0,
        },
        &[
            LogSeriesEntry {
                channel: "lambda",
                label: "LAM",
                color: "#56a64b",
                min: 0.7,
                max: 1.3,
            },
            LogSeriesEntry {
                channel: "pulseWidth",
                label: "PW",
                color: "#5794f2",
                min: 0.0,
                max: 25.0,
            },
            LogSeriesEntry {
                channel: "ve",
                label: "VE",
                color: "#fade2a",
                min: 0.0,
                max: 150.0,
            },
            LogSeriesEntry {
                channel: "correction",
                label: "CORR",
                color: "#8e8e93",
                min: 50.0,
                max: 150.0,
            },
            LogSeriesEntry {
                channel: "afrTarget",
                label: "TGT",
                color: "#b877d9",
                min: 10.0,
                max: 20.0,
            },
        ],
        0.570,
        0.060,
        0.425,
        0.210,
    ));

    dash.gauge_cluster.components.push(log_multi_trend(
        "tl_trend_temp",
        "TEMPERATURES",
        LogSeriesEntry {
            channel: "coolant",
            label: "CLT",
            color: "#5794f2",
            min: -20.0,
            max: 120.0,
        },
        &[
            LogSeriesEntry {
                channel: "iat",
                label: "IAT",
                color: "#ff9830",
                min: -20.0,
                max: 80.0,
            },
            LogSeriesEntry {
                channel: "egt",
                label: "EGT",
                color: "#f2495c",
                min: 0.0,
                max: 1000.0,
            },
            LogSeriesEntry {
                channel: "oilTemp",
                label: "OILT",
                color: "#fade2a",
                min: 0.0,
                max: 150.0,
            },
        ],
        0.130,
        0.278,
        0.430,
        0.223,
    ));

    dash.gauge_cluster.components.push(log_multi_trend(
        "tl_trend_pressure",
        "PRESSURES & LOAD",
        LogSeriesEntry {
            channel: "map",
            label: "MAP",
            color: "#33b5ff",
            min: 0.0,
            max: 250.0,
        },
        &[
            LogSeriesEntry {
                channel: "baro",
                label: "BARO",
                color: "#8e8e93",
                min: 70.0,
                max: 110.0,
            },
            LogSeriesEntry {
                channel: "oilPressure",
                label: "OILP",
                color: "#fade2a",
                min: 0.0,
                max: 700.0,
            },
            LogSeriesEntry {
                channel: "boost",
                label: "BOOST",
                color: "#f2495c",
                min: 0.0,
                max: 300.0,
            },
            LogSeriesEntry {
                channel: "battery",
                label: "BATT",
                color: "#73bf69",
                min: 10.0,
                max: 16.0,
            },
        ],
        0.570,
        0.278,
        0.425,
        0.223,
    ));

    // --- Sparkline wall (16 scrolling single-channel charts) -----------------
    let spark_w = 0.242;
    let spark_h = 0.115;
    let spark_y0 = 0.510;
    let sparks = [
        LogSparkSpec {
            id: "tl_sp_rpm",
            title: "RPM",
            channel: "rpm",
            units: "",
            min: 0.0,
            max: 9000.0,
            digits: 0,
            x: 0.005,
            y: spark_y0,
            w: spark_w,
            h: spark_h,
            color: LT_LOG_BLUE,
        },
        LogSparkSpec {
            id: "tl_sp_map",
            title: "MAP",
            channel: "map",
            units: "kPa",
            min: 0.0,
            max: 250.0,
            digits: 0,
            x: 0.005 + spark_w + 0.006,
            y: spark_y0,
            w: spark_w,
            h: spark_h,
            color: LT_LOG_CYAN,
        },
        LogSparkSpec {
            id: "tl_sp_tps",
            title: "TPS",
            channel: "tps",
            units: "%",
            min: 0.0,
            max: 100.0,
            digits: 0,
            x: 0.005 + (spark_w + 0.006) * 2.0,
            y: spark_y0,
            w: spark_w,
            h: spark_h,
            color: LT_LOG_ORANGE,
        },
        LogSparkSpec {
            id: "tl_sp_spd",
            title: "SPEED",
            channel: "speed",
            units: "km/h",
            min: 0.0,
            max: 260.0,
            digits: 0,
            x: 0.005 + (spark_w + 0.006) * 3.0,
            y: spark_y0,
            w: spark_w,
            h: spark_h,
            color: LT_LOG_WHITE,
        },
        LogSparkSpec {
            id: "tl_sp_afr",
            title: "AFR",
            channel: "afr",
            units: ":1",
            min: 10.0,
            max: 20.0,
            digits: 2,
            x: 0.005,
            y: spark_y0 + spark_h + 0.008,
            w: spark_w,
            h: spark_h,
            color: LT_LOG_GREEN,
        },
        LogSparkSpec {
            id: "tl_sp_lam",
            title: "LAMBDA",
            channel: "lambda",
            units: "λ",
            min: 0.7,
            max: 1.3,
            digits: 3,
            x: 0.005 + spark_w + 0.006,
            y: spark_y0 + spark_h + 0.008,
            w: spark_w,
            h: spark_h,
            color: LT_LOG_GREEN,
        },
        LogSparkSpec {
            id: "tl_sp_ve",
            title: "VE",
            channel: "ve",
            units: "%",
            min: 0.0,
            max: 150.0,
            digits: 0,
            x: 0.005 + (spark_w + 0.006) * 2.0,
            y: spark_y0 + spark_h + 0.008,
            w: spark_w,
            h: spark_h,
            color: LT_LOG_YELLOW,
        },
        LogSparkSpec {
            id: "tl_sp_pw",
            title: "PULSE",
            channel: "pulseWidth",
            units: "ms",
            min: 0.0,
            max: 25.0,
            digits: 2,
            x: 0.005 + (spark_w + 0.006) * 3.0,
            y: spark_y0 + spark_h + 0.008,
            w: spark_w,
            h: spark_h,
            color: LT_LOG_BLUE,
        },
        LogSparkSpec {
            id: "tl_sp_clt",
            title: "COOLANT",
            channel: "coolant",
            units: "°C",
            min: -20.0,
            max: 120.0,
            digits: 0,
            x: 0.005,
            y: spark_y0 + (spark_h + 0.008) * 2.0,
            w: spark_w,
            h: spark_h,
            color: LT_LOG_BLUE,
        },
        LogSparkSpec {
            id: "tl_sp_iat",
            title: "IAT",
            channel: "iat",
            units: "°C",
            min: -20.0,
            max: 80.0,
            digits: 0,
            x: 0.005 + spark_w + 0.006,
            y: spark_y0 + (spark_h + 0.008) * 2.0,
            w: spark_w,
            h: spark_h,
            color: LT_LOG_ORANGE,
        },
        LogSparkSpec {
            id: "tl_sp_adv",
            title: "TIMING",
            channel: "advance",
            units: "°",
            min: -10.0,
            max: 50.0,
            digits: 1,
            x: 0.005 + (spark_w + 0.006) * 2.0,
            y: spark_y0 + (spark_h + 0.008) * 2.0,
            w: spark_w,
            h: spark_h,
            color: LT_LOG_CYAN,
        },
        LogSparkSpec {
            id: "tl_sp_duty",
            title: "DUTY",
            channel: "dutyCycle",
            units: "%",
            min: 0.0,
            max: 100.0,
            digits: 0,
            x: 0.005 + (spark_w + 0.006) * 3.0,
            y: spark_y0 + (spark_h + 0.008) * 2.0,
            w: spark_w,
            h: spark_h,
            color: LT_LOG_PURPLE,
        },
        LogSparkSpec {
            id: "tl_sp_bst",
            title: "BOOST",
            channel: "boost",
            units: "kPa",
            min: 0.0,
            max: 300.0,
            digits: 0,
            x: 0.005,
            y: spark_y0 + (spark_h + 0.008) * 3.0,
            w: spark_w,
            h: spark_h,
            color: LT_LOG_RED,
        },
        LogSparkSpec {
            id: "tl_sp_oilp",
            title: "OIL P",
            channel: "oilPressure",
            units: "kPa",
            min: 0.0,
            max: 700.0,
            digits: 0,
            x: 0.005 + spark_w + 0.006,
            y: spark_y0 + (spark_h + 0.008) * 3.0,
            w: spark_w,
            h: spark_h,
            color: LT_LOG_YELLOW,
        },
        LogSparkSpec {
            id: "tl_sp_egt",
            title: "EGT",
            channel: "egt",
            units: "°C",
            min: 0.0,
            max: 1000.0,
            digits: 0,
            x: 0.005 + (spark_w + 0.006) * 2.0,
            y: spark_y0 + (spark_h + 0.008) * 3.0,
            w: spark_w,
            h: spark_h,
            color: LT_LOG_RED,
        },
        LogSparkSpec {
            id: "tl_sp_bat",
            title: "BATTERY",
            channel: "battery",
            units: "V",
            min: 10.0,
            max: 16.0,
            digits: 1,
            x: 0.005 + (spark_w + 0.006) * 3.0,
            y: spark_y0 + (spark_h + 0.008) * 3.0,
            w: spark_w,
            h: spark_h,
            color: LT_LOG_GREEN,
        },
    ];
    for spec in sparks {
        dash.gauge_cluster.components.push(log_sparkline(spec));
    }

    dash
}

/// Backward-compatible alias — the old "F1 Telemetry" name pointed here.
pub fn create_f1_telemetry_dashboard() -> DashFile {
    create_telemetry_live_dashboard()
}
