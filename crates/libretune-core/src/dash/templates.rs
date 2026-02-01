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
const LT_DARKER_BG: TsColor = TsColor { alpha: 255, red: 12, green: 14, blue: 20 };
const LT_GAUGE_BG: TsColor = TsColor { alpha: 255, red: 28, green: 32, blue: 40 };
const LT_ACCENT_BLUE: TsColor = TsColor { alpha: 255, red: 74, green: 158, blue: 248 };
const LT_ACCENT_TEAL: TsColor = TsColor { alpha: 255, red: 56, green: 189, blue: 248 };
const LT_ACCENT_AMBER: TsColor = TsColor { alpha: 255, red: 251, green: 191, blue: 36 };
const LT_ACCENT_GREEN: TsColor = TsColor { alpha: 255, red: 34, green: 197, blue: 94 };
const LT_ACCENT_RED: TsColor = TsColor { alpha: 255, red: 239, green: 68, blue: 68 };
const LT_TEXT_PRIMARY: TsColor = TsColor { alpha: 255, red: 255, green: 255, blue: 255 };
const LT_TEXT_SECONDARY: TsColor = TsColor { alpha: 255, red: 148, green: 163, blue: 184 };
const LT_WARN_COLOR: TsColor = TsColor { alpha: 255, red: 234, green: 179, blue: 8 };
const LT_CRITICAL_COLOR: TsColor = TsColor { alpha: 255, red: 239, green: 68, blue: 68 };

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
        },
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

/// Create a racing-focused dashboard
/// Massive center RPM tachometer with critical racing metrics in bold readouts
/// Optimized for quick glances while driving at speed
pub fn create_racing_dashboard() -> DashFile {
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
        },
    };

    // MASSIVE CENTER: RPM Tachometer - the star of the show
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "rpm".to_string(),
            title: "".to_string(),  // No title - let the gauge speak for itself
            units: "RPM".to_string(),
            output_channel: "rpm".to_string(),
            min: 0.0,
            max: 10000.0,
            high_warning: Some(8000.0),
            high_critical: Some(9000.0),
            gauge_painter: GaugePainter::Tachometer,
            start_angle: 135,
            sweep_angle: 270,
            major_ticks: 10.0,
            minor_ticks: 5.0,
            value_digits: 0,
            relative_x: 0.20,
            relative_y: 0.08,
            relative_width: 0.60,
            relative_height: 0.65,
            back_color: LT_GAUGE_BG,
            font_color: LT_TEXT_PRIMARY,
            needle_color: LT_ACCENT_RED,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: 4,
            ..Default::default()
        })));

    // LEFT SIDE: Oil pressure vertical bar - critical for racing
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "oilpres".to_string(),
            title: "OIL".to_string(),
            units: "PSI".to_string(),
            output_channel: "oilPressure".to_string(),
            min: 0.0,
            max: 100.0,
            low_warning: Some(20.0),
            low_critical: Some(10.0),
            value_digits: 0,
            gauge_painter: GaugePainter::VerticalBarGauge,
            relative_x: 0.02,
            relative_y: 0.08,
            relative_width: 0.14,
            relative_height: 0.50,
            back_color: LT_GAUGE_BG,
            font_color: LT_ACCENT_AMBER,
            needle_color: LT_ACCENT_AMBER,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: 1,
            ..Default::default()
        })));

    // RIGHT SIDE: Water temp vertical bar
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "coolant".to_string(),
            title: "WATER".to_string(),
            units: "°C".to_string(),
            output_channel: "coolant".to_string(),
            min: 0.0,
            max: 130.0,
            high_warning: Some(105.0),
            high_critical: Some(115.0),
            value_digits: 0,
            gauge_painter: GaugePainter::VerticalBarGauge,
            relative_x: 0.84,
            relative_y: 0.08,
            relative_width: 0.14,
            relative_height: 0.50,
            back_color: LT_GAUGE_BG,
            font_color: LT_ACCENT_BLUE,
            needle_color: LT_ACCENT_BLUE,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: 1,
            ..Default::default()
        })));

    // BOTTOM ROW: Large digital readouts for quick glances

    // Speed - bottom far left
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "speed".to_string(),
            title: "SPEED".to_string(),
            units: "KM/H".to_string(),
            output_channel: "speed".to_string(),
            min: 0.0,
            max: 300.0,
            value_digits: 0,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.02,
            relative_y: 0.78,
            relative_width: 0.22,
            relative_height: 0.20,
            back_color: LT_GAUGE_BG,
            font_color: LT_TEXT_PRIMARY,
            needle_color: LT_ACCENT_TEAL,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: 6,
            ..Default::default()
        })));

    // AFR - bottom center-left
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "afr".to_string(),
            title: "AFR".to_string(),
            units: ":1".to_string(),
            output_channel: "afr".to_string(),
            min: 10.0,
            max: 20.0,
            low_warning: Some(11.0),
            low_critical: Some(10.5),
            high_warning: Some(16.0),
            value_digits: 1,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.26,
            relative_y: 0.78,
            relative_width: 0.22,
            relative_height: 0.20,
            back_color: LT_GAUGE_BG,
            font_color: LT_ACCENT_GREEN,
            needle_color: LT_ACCENT_GREEN,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: 6,
            ..Default::default()
        })));

    // Boost - bottom center-right
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "boost".to_string(),
            title: "BOOST".to_string(),
            units: "PSI".to_string(),
            output_channel: "boost".to_string(),
            min: -15.0,
            max: 30.0,
            high_warning: Some(22.0),
            high_critical: Some(26.0),
            value_digits: 1,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.50,
            relative_y: 0.78,
            relative_width: 0.22,
            relative_height: 0.20,
            back_color: LT_GAUGE_BG,
            font_color: LT_ACCENT_TEAL,
            needle_color: LT_ACCENT_TEAL,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: 6,
            ..Default::default()
        })));

    // Fuel - bottom far right
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "fuel".to_string(),
            title: "FUEL".to_string(),
            units: "%".to_string(),
            output_channel: "fuelLevel".to_string(),
            min: 0.0,
            max: 100.0,
            low_warning: Some(20.0),
            low_critical: Some(10.0),
            value_digits: 0,
            gauge_painter: GaugePainter::FuelMeter,
            relative_x: 0.74,
            relative_y: 0.78,
            relative_width: 0.24,
            relative_height: 0.20,
            back_color: LT_GAUGE_BG,
            font_color: LT_ACCENT_AMBER,
            needle_color: LT_ACCENT_AMBER,
            trim_color: LT_TEXT_SECONDARY,
            warn_color: LT_WARN_COLOR,
            critical_color: LT_CRITICAL_COLOR,
            font_size_adjustment: 3,
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
        },
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
