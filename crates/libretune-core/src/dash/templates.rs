//! Default dashboard templates for LibreTune.
//!
//! This module provides pre-configured dashboard layouts that match
//! common ECU tuning workflows and TunerStudio-style layouts.

use super::{
    BackgroundStyle, Bibliography, DashComponent, DashFile, GaugeCluster, GaugeConfig,
    GaugePainter, TsColor, VersionInfo,
};
use chrono;

/// Create a basic dashboard layout - LibreTune default
/// 2x4 grid of analog gauges: RPM/Coolant/TPS/Map top row, AFR/Battery/Dwell/Advance bottom row
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
            cluster_background_color: TsColor {
                alpha: 255,
                red: 25,
                green: 25,
                blue: 30,
            },
            background_dither_color: None,
            cluster_background_image_file_name: None,
            cluster_background_image_style: BackgroundStyle::Stretch,
            embedded_images: Vec::new(),
            components: Vec::new(),
        },
    };

    // Helper function to create analog gauge config
    let create_analog_gauge = |id: &str,
                               title: &str,
                               units: &str,
                               channel: &str,
                               min: f64,
                               max: f64,
                               x: f64,
                               y: f64|
     -> GaugeConfig {
        GaugeConfig {
            id: id.to_string(),
            title: title.to_string(),
            units: units.to_string(),
            output_channel: channel.to_string(),
            min,
            max,
            gauge_painter: GaugePainter::AnalogGauge,
            start_angle: 225,
            sweep_angle: 270,
            major_ticks: 10.0,
            minor_ticks: 5.0,
            relative_x: x,
            relative_y: y,
            relative_width: 0.24,
            relative_height: 0.48,
            back_color: TsColor {
                alpha: 255,
                red: 40,
                green: 40,
                blue: 45,
            },
            font_color: TsColor {
                alpha: 255,
                red: 255,
                green: 255,
                blue: 255,
            },
            needle_color: TsColor {
                alpha: 255,
                red: 255,
                green: 80,
                blue: 0,
            },
            trim_color: TsColor {
                alpha: 255,
                red: 100,
                green: 100,
                blue: 110,
            },
            warn_color: TsColor {
                alpha: 255,
                red: 255,
                green: 200,
                blue: 0,
            },
            critical_color: TsColor {
                alpha: 255,
                red: 255,
                green: 50,
                blue: 50,
            },
            value_digits: 0,
            ..Default::default()
        }
    };

    // Top row: RPM, Coolant Temp, Throttle Position, Map
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(create_analog_gauge(
            "rpm", "RPM", "", "rpm", 0.0, 8000.0, 0.01, 0.01,
        ))));
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(create_analog_gauge(
            "coolant", "COOLANT", "°C", "coolant", -40.0, 120.0, 0.26, 0.01,
        ))));
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(create_analog_gauge(
            "tps", "TPS", "%", "tps", 0.0, 100.0, 0.51, 0.01,
        ))));
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(create_analog_gauge(
            "map", "MAP", "kPa", "map", 0.0, 250.0, 0.76, 0.01,
        ))));

    // Bottom row: Air/Fuel Ratio, Battery, Dwell, Ignition Timing
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(create_analog_gauge(
            "afr", "AFR", "", "afr", 10.0, 20.0, 0.01, 0.51,
        ))));
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(create_analog_gauge(
            "battery", "BATTERY", "V", "battery", 10.0, 16.0, 0.26, 0.51,
        ))));
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(create_analog_gauge(
            "dwell", "DWELL", "°", "dwell", 0.0, 10.0, 0.51, 0.51,
        ))));
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(create_analog_gauge(
            "advance", "ADVANCE", "°", "advance", -10.0, 50.0, 0.76, 0.51,
        ))));

    dash
}

/// Create a racing-focused dashboard
/// Large center RPM with oil pressure, water temp, speed, AFR, boost, fuel
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
            cluster_background_color: TsColor {
                alpha: 255,
                red: 15,
                green: 15,
                blue: 20,
            },
            background_dither_color: None,
            cluster_background_image_file_name: None,
            cluster_background_image_style: BackgroundStyle::Stretch,
            embedded_images: Vec::new(),
            components: Vec::new(),
        },
    };

    // Giant center RPM
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "rpm".to_string(),
            title: "RPM".to_string(),
            units: "".to_string(),
            output_channel: "rpm".to_string(),
            min: 0.0,
            max: 10000.0,
            high_warning: Some(8000.0),
            high_critical: Some(9000.0),
            gauge_painter: GaugePainter::AnalogGauge,
            relative_x: 0.15,
            relative_y: 0.05,
            relative_width: 0.70,
            relative_height: 0.70,
            back_color: TsColor {
                alpha: 255,
                red: 25,
                green: 25,
                blue: 30,
            },
            font_color: TsColor {
                alpha: 255,
                red: 255,
                green: 255,
                blue: 255,
            },
            needle_color: TsColor {
                alpha: 255,
                red: 255,
                green: 0,
                blue: 0,
            },
            trim_color: TsColor {
                alpha: 255,
                red: 80,
                green: 80,
                blue: 90,
            },
            warn_color: TsColor {
                alpha: 255,
                red: 255,
                green: 255,
                blue: 0,
            },
            critical_color: TsColor {
                alpha: 255,
                red: 255,
                green: 0,
                blue: 0,
            },
            ..Default::default()
        })));

    // Oil pressure (left)
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "oilpres".to_string(),
            title: "OIL".to_string(),
            units: "psi".to_string(),
            output_channel: "oilPressure".to_string(),
            min: 0.0,
            max: 100.0,
            low_warning: Some(20.0),
            low_critical: Some(10.0),
            value_digits: 0,
            gauge_painter: GaugePainter::VerticalBarGauge,
            relative_x: 0.02,
            relative_y: 0.05,
            relative_width: 0.10,
            relative_height: 0.55,
            back_color: TsColor {
                alpha: 255,
                red: 30,
                green: 30,
                blue: 35,
            },
            font_color: TsColor {
                alpha: 255,
                red: 255,
                green: 200,
                blue: 100,
            },
            ..Default::default()
        })));

    // Water temp (right)
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "coolant".to_string(),
            title: "H2O".to_string(),
            units: "°C".to_string(),
            output_channel: "coolant".to_string(),
            min: 0.0,
            max: 130.0,
            high_warning: Some(105.0),
            high_critical: Some(115.0),
            value_digits: 0,
            gauge_painter: GaugePainter::VerticalBarGauge,
            relative_x: 0.88,
            relative_y: 0.05,
            relative_width: 0.10,
            relative_height: 0.55,
            back_color: TsColor {
                alpha: 255,
                red: 30,
                green: 30,
                blue: 35,
            },
            font_color: TsColor {
                alpha: 255,
                red: 100,
                green: 200,
                blue: 255,
            },
            ..Default::default()
        })));

    // Speed (bottom left)
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "speed".to_string(),
            title: "SPEED".to_string(),
            units: "km/h".to_string(),
            output_channel: "speed".to_string(),
            min: 0.0,
            max: 300.0,
            value_digits: 0,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.02,
            relative_y: 0.78,
            relative_width: 0.23,
            relative_height: 0.20,
            back_color: TsColor {
                alpha: 255,
                red: 25,
                green: 25,
                blue: 30,
            },
            font_color: TsColor {
                alpha: 255,
                red: 255,
                green: 255,
                blue: 255,
            },
            font_size_adjustment: 4,
            ..Default::default()
        })));

    // AFR (bottom center-left)
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "afr".to_string(),
            title: "AFR".to_string(),
            units: "".to_string(),
            output_channel: "afr".to_string(),
            min: 10.0,
            max: 18.0,
            low_warning: Some(11.0),
            high_warning: Some(15.0),
            value_digits: 1,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.27,
            relative_y: 0.78,
            relative_width: 0.22,
            relative_height: 0.20,
            back_color: TsColor {
                alpha: 255,
                red: 25,
                green: 25,
                blue: 30,
            },
            font_color: TsColor {
                alpha: 255,
                red: 0,
                green: 255,
                blue: 128,
            },
            font_size_adjustment: 4,
            ..Default::default()
        })));

    // Boost (bottom center-right)
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "boost".to_string(),
            title: "BOOST".to_string(),
            units: "psi".to_string(),
            output_channel: "boost".to_string(),
            min: -15.0,
            max: 30.0,
            high_warning: Some(22.0),
            high_critical: Some(26.0),
            value_digits: 1,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.51,
            relative_y: 0.78,
            relative_width: 0.22,
            relative_height: 0.20,
            back_color: TsColor {
                alpha: 255,
                red: 25,
                green: 25,
                blue: 30,
            },
            font_color: TsColor {
                alpha: 255,
                red: 100,
                green: 200,
                blue: 255,
            },
            font_size_adjustment: 4,
            ..Default::default()
        })));

    // Fuel level (bottom right)
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
            gauge_painter: GaugePainter::HorizontalBarGauge,
            relative_x: 0.75,
            relative_y: 0.78,
            relative_width: 0.23,
            relative_height: 0.20,
            back_color: TsColor {
                alpha: 255,
                red: 25,
                green: 25,
                blue: 30,
            },
            font_color: TsColor {
                alpha: 255,
                red: 255,
                green: 200,
                blue: 0,
            },
            ..Default::default()
        })));

    dash
}

/// Create a tuning-focused dashboard
/// Mixed layout with sweep gauge, analog gauge, bars, line graph, dashed bars, correction factors
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
            cluster_background_color: TsColor {
                alpha: 255,
                red: 20,
                green: 22,
                blue: 28,
            },
            background_dither_color: None,
            cluster_background_image_file_name: None,
            cluster_background_image_style: BackgroundStyle::Stretch,
            embedded_images: Vec::new(),
            components: Vec::new(),
        },
    };

    // Top row: RPM sweep gauge + AFR analog + Coolant bar

    // RPM - large sweep gauge
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
            gauge_painter: GaugePainter::AsymmetricSweepGauge,
            start_angle: 200,
            sweep_angle: 140,
            relative_x: 0.02,
            relative_y: 0.02,
            relative_width: 0.38,
            relative_height: 0.32,
            back_color: TsColor {
                alpha: 255,
                red: 25,
                green: 28,
                blue: 35,
            },
            font_color: TsColor {
                alpha: 255,
                red: 255,
                green: 255,
                blue: 255,
            },
            needle_color: TsColor {
                alpha: 255,
                red: 0,
                green: 200,
                blue: 100,
            },
            trim_color: TsColor {
                alpha: 255,
                red: 80,
                green: 90,
                blue: 100,
            },
            warn_color: TsColor {
                alpha: 255,
                red: 255,
                green: 180,
                blue: 0,
            },
            critical_color: TsColor {
                alpha: 255,
                red: 255,
                green: 50,
                blue: 50,
            },
            ..Default::default()
        })));

    // AFR - analog gauge
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "afr".to_string(),
            title: "AFR".to_string(),
            units: "".to_string(),
            output_channel: "afr".to_string(),
            min: 10.0,
            max: 18.0,
            low_warning: Some(11.5),
            low_critical: Some(10.5),
            high_warning: Some(15.5),
            high_critical: Some(16.5),
            value_digits: 2,
            gauge_painter: GaugePainter::AnalogGauge,
            relative_x: 0.42,
            relative_y: 0.02,
            relative_width: 0.32,
            relative_height: 0.32,
            back_color: TsColor {
                alpha: 255,
                red: 30,
                green: 35,
                blue: 40,
            },
            font_color: TsColor {
                alpha: 255,
                red: 0,
                green: 255,
                blue: 128,
            },
            needle_color: TsColor {
                alpha: 255,
                red: 0,
                green: 255,
                blue: 100,
            },
            trim_color: TsColor {
                alpha: 255,
                red: 60,
                green: 80,
                blue: 60,
            },
            warn_color: TsColor {
                alpha: 255,
                red: 255,
                green: 200,
                blue: 0,
            },
            critical_color: TsColor {
                alpha: 255,
                red: 255,
                green: 50,
                blue: 50,
            },
            ..Default::default()
        })));

    // Coolant - vertical bar
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "coolant".to_string(),
            title: "CLT".to_string(),
            units: "°C".to_string(),
            output_channel: "coolant".to_string(),
            min: -40.0,
            max: 120.0,
            high_warning: Some(100.0),
            high_critical: Some(110.0),
            value_digits: 0,
            gauge_painter: GaugePainter::VerticalBarGauge,
            relative_x: 0.76,
            relative_y: 0.02,
            relative_width: 0.10,
            relative_height: 0.32,
            back_color: TsColor {
                alpha: 255,
                red: 28,
                green: 30,
                blue: 35,
            },
            font_color: TsColor {
                alpha: 255,
                red: 100,
                green: 200,
                blue: 255,
            },
            needle_color: TsColor {
                alpha: 255,
                red: 0,
                green: 150,
                blue: 255,
            },
            trim_color: TsColor {
                alpha: 255,
                red: 60,
                green: 80,
                blue: 100,
            },
            warn_color: TsColor {
                alpha: 255,
                red: 255,
                green: 200,
                blue: 0,
            },
            critical_color: TsColor {
                alpha: 255,
                red: 255,
                green: 50,
                blue: 50,
            },
            ..Default::default()
        })));

    // IAT - vertical bar
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
            high_critical: Some(65.0),
            value_digits: 0,
            gauge_painter: GaugePainter::VerticalBarGauge,
            relative_x: 0.88,
            relative_y: 0.02,
            relative_width: 0.10,
            relative_height: 0.32,
            back_color: TsColor {
                alpha: 255,
                red: 28,
                green: 30,
                blue: 35,
            },
            font_color: TsColor {
                alpha: 255,
                red: 255,
                green: 180,
                blue: 80,
            },
            needle_color: TsColor {
                alpha: 255,
                red: 255,
                green: 150,
                blue: 0,
            },
            trim_color: TsColor {
                alpha: 255,
                red: 100,
                green: 80,
                blue: 50,
            },
            warn_color: TsColor {
                alpha: 255,
                red: 255,
                green: 200,
                blue: 0,
            },
            critical_color: TsColor {
                alpha: 255,
                red: 255,
                green: 50,
                blue: 50,
            },
            ..Default::default()
        })));

    // Middle row: MAP bar + VE digital + Advance digital + TPS bar + Duty bar

    // MAP - horizontal bar
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "map".to_string(),
            title: "MAP".to_string(),
            units: "kPa".to_string(),
            output_channel: "map".to_string(),
            min: 0.0,
            max: 250.0,
            value_digits: 0,
            gauge_painter: GaugePainter::HorizontalBarGauge,
            relative_x: 0.02,
            relative_y: 0.36,
            relative_width: 0.30,
            relative_height: 0.14,
            back_color: TsColor {
                alpha: 255,
                red: 25,
                green: 28,
                blue: 35,
            },
            font_color: TsColor {
                alpha: 255,
                red: 180,
                green: 180,
                blue: 255,
            },
            needle_color: TsColor {
                alpha: 255,
                red: 100,
                green: 100,
                blue: 255,
            },
            trim_color: TsColor {
                alpha: 255,
                red: 70,
                green: 70,
                blue: 120,
            },
            ..Default::default()
        })));

    // VE - digital readout
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "ve".to_string(),
            title: "VE".to_string(),
            units: "%".to_string(),
            output_channel: "ve".to_string(),
            min: 0.0,
            max: 150.0,
            value_digits: 1,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.34,
            relative_y: 0.36,
            relative_width: 0.20,
            relative_height: 0.14,
            back_color: TsColor {
                alpha: 255,
                red: 25,
                green: 30,
                blue: 25,
            },
            font_color: TsColor {
                alpha: 255,
                red: 100,
                green: 255,
                blue: 100,
            },
            trim_color: TsColor {
                alpha: 255,
                red: 60,
                green: 100,
                blue: 60,
            },
            ..Default::default()
        })));

    // Advance - digital readout
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "advance".to_string(),
            title: "ADV".to_string(),
            units: "°".to_string(),
            output_channel: "advance".to_string(),
            min: -10.0,
            max: 50.0,
            value_digits: 1,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.56,
            relative_y: 0.36,
            relative_width: 0.20,
            relative_height: 0.14,
            back_color: TsColor {
                alpha: 255,
                red: 30,
                green: 25,
                blue: 20,
            },
            font_color: TsColor {
                alpha: 255,
                red: 255,
                green: 180,
                blue: 80,
            },
            trim_color: TsColor {
                alpha: 255,
                red: 100,
                green: 80,
                blue: 50,
            },
            ..Default::default()
        })));

    // TPS - horizontal line gauge
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "tps".to_string(),
            title: "TPS".to_string(),
            units: "%".to_string(),
            output_channel: "tps".to_string(),
            min: 0.0,
            max: 100.0,
            value_digits: 1,
            gauge_painter: GaugePainter::HorizontalLineGauge,
            relative_x: 0.78,
            relative_y: 0.36,
            relative_width: 0.20,
            relative_height: 0.14,
            back_color: TsColor {
                alpha: 255,
                red: 25,
                green: 28,
                blue: 35,
            },
            font_color: TsColor {
                alpha: 255,
                red: 200,
                green: 200,
                blue: 200,
            },
            needle_color: TsColor {
                alpha: 255,
                red: 200,
                green: 200,
                blue: 200,
            },
            trim_color: TsColor {
                alpha: 255,
                red: 80,
                green: 80,
                blue: 90,
            },
            ..Default::default()
        })));

    // Bottom row: PW bar + Lambda histogram + EGT dashed bar + Duty dashed bar

    // Pulse Width - horizontal bar
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "pw".to_string(),
            title: "PW".to_string(),
            units: "ms".to_string(),
            output_channel: "pulseWidth1".to_string(),
            min: 0.0,
            max: 25.0,
            value_digits: 2,
            gauge_painter: GaugePainter::HorizontalBarGauge,
            relative_x: 0.02,
            relative_y: 0.52,
            relative_width: 0.30,
            relative_height: 0.14,
            back_color: TsColor {
                alpha: 255,
                red: 25,
                green: 25,
                blue: 30,
            },
            font_color: TsColor {
                alpha: 255,
                red: 200,
                green: 200,
                blue: 200,
            },
            needle_color: TsColor {
                alpha: 255,
                red: 150,
                green: 150,
                blue: 180,
            },
            trim_color: TsColor {
                alpha: 255,
                red: 80,
                green: 80,
                blue: 100,
            },
            ..Default::default()
        })));

    // Lambda correction - line graph (simulates historical view)
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "lambda_hist".to_string(),
            title: "λ HISTORY".to_string(),
            units: "".to_string(),
            output_channel: "lambda".to_string(),
            min: 0.7,
            max: 1.3,
            low_warning: Some(0.8),
            high_warning: Some(1.15),
            value_digits: 3,
            gauge_painter: GaugePainter::LineGraph,
            relative_x: 0.34,
            relative_y: 0.52,
            relative_width: 0.30,
            relative_height: 0.46,
            back_color: TsColor {
                alpha: 255,
                red: 20,
                green: 25,
                blue: 30,
            },
            font_color: TsColor {
                alpha: 255,
                red: 0,
                green: 200,
                blue: 100,
            },
            needle_color: TsColor {
                alpha: 255,
                red: 0,
                green: 255,
                blue: 128,
            },
            trim_color: TsColor {
                alpha: 255,
                red: 60,
                green: 80,
                blue: 70,
            },
            warn_color: TsColor {
                alpha: 255,
                red: 255,
                green: 200,
                blue: 0,
            },
            critical_color: TsColor {
                alpha: 255,
                red: 255,
                green: 50,
                blue: 50,
            },
            ..Default::default()
        })));

    // EGT - vertical dashed bar
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "egt".to_string(),
            title: "EGT".to_string(),
            units: "°C".to_string(),
            output_channel: "egt".to_string(),
            min: 0.0,
            max: 1000.0,
            high_warning: Some(800.0),
            high_critical: Some(900.0),
            value_digits: 0,
            gauge_painter: GaugePainter::VerticalDashedBar,
            relative_x: 0.66,
            relative_y: 0.52,
            relative_width: 0.15,
            relative_height: 0.46,
            back_color: TsColor {
                alpha: 255,
                red: 28,
                green: 25,
                blue: 25,
            },
            font_color: TsColor {
                alpha: 255,
                red: 255,
                green: 150,
                blue: 80,
            },
            needle_color: TsColor {
                alpha: 255,
                red: 255,
                green: 100,
                blue: 50,
            },
            trim_color: TsColor {
                alpha: 255,
                red: 100,
                green: 70,
                blue: 50,
            },
            warn_color: TsColor {
                alpha: 255,
                red: 255,
                green: 200,
                blue: 0,
            },
            critical_color: TsColor {
                alpha: 255,
                red: 255,
                green: 50,
                blue: 50,
            },
            ..Default::default()
        })));

    // Injector Duty - vertical dashed bar
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "duty".to_string(),
            title: "DUTY".to_string(),
            units: "%".to_string(),
            output_channel: "injDuty".to_string(),
            min: 0.0,
            max: 100.0,
            high_warning: Some(85.0),
            high_critical: Some(95.0),
            value_digits: 0,
            gauge_painter: GaugePainter::VerticalDashedBar,
            relative_x: 0.83,
            relative_y: 0.52,
            relative_width: 0.15,
            relative_height: 0.46,
            back_color: TsColor {
                alpha: 255,
                red: 25,
                green: 25,
                blue: 30,
            },
            font_color: TsColor {
                alpha: 255,
                red: 150,
                green: 200,
                blue: 255,
            },
            needle_color: TsColor {
                alpha: 255,
                red: 100,
                green: 180,
                blue: 255,
            },
            trim_color: TsColor {
                alpha: 255,
                red: 60,
                green: 80,
                blue: 100,
            },
            warn_color: TsColor {
                alpha: 255,
                red: 255,
                green: 200,
                blue: 0,
            },
            critical_color: TsColor {
                alpha: 255,
                red: 255,
                green: 50,
                blue: 50,
            },
            ..Default::default()
        })));

    // Battery - small readout at bottom
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
            low_critical: Some(10.5),
            high_warning: Some(15.0),
            value_digits: 1,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.02,
            relative_y: 0.68,
            relative_width: 0.15,
            relative_height: 0.14,
            back_color: TsColor {
                alpha: 255,
                red: 28,
                green: 28,
                blue: 25,
            },
            font_color: TsColor {
                alpha: 255,
                red: 255,
                green: 220,
                blue: 100,
            },
            trim_color: TsColor {
                alpha: 255,
                red: 100,
                green: 90,
                blue: 50,
            },
            warn_color: TsColor {
                alpha: 255,
                red: 255,
                green: 200,
                blue: 0,
            },
            critical_color: TsColor {
                alpha: 255,
                red: 255,
                green: 50,
                blue: 50,
            },
            ..Default::default()
        })));

    // Fuel Correction - small readout at bottom
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "fuelcorr".to_string(),
            title: "FUEL%".to_string(),
            units: "%".to_string(),
            output_channel: "fuelCorrection".to_string(),
            min: -25.0,
            max: 25.0,
            value_digits: 1,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.19,
            relative_y: 0.68,
            relative_width: 0.13,
            relative_height: 0.14,
            back_color: TsColor {
                alpha: 255,
                red: 25,
                green: 28,
                blue: 25,
            },
            font_color: TsColor {
                alpha: 255,
                red: 150,
                green: 255,
                blue: 150,
            },
            trim_color: TsColor {
                alpha: 255,
                red: 70,
                green: 100,
                blue: 70,
            },
            ..Default::default()
        })));

    // CLT Correction - small readout at bottom
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "cltcorr".to_string(),
            title: "CLT%".to_string(),
            units: "%".to_string(),
            output_channel: "cltCorrection".to_string(),
            min: 0.0,
            max: 200.0,
            value_digits: 0,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.02,
            relative_y: 0.84,
            relative_width: 0.15,
            relative_height: 0.14,
            back_color: TsColor {
                alpha: 255,
                red: 25,
                green: 28,
                blue: 30,
            },
            font_color: TsColor {
                alpha: 255,
                red: 100,
                green: 200,
                blue: 255,
            },
            trim_color: TsColor {
                alpha: 255,
                red: 60,
                green: 90,
                blue: 120,
            },
            ..Default::default()
        })));

    // IAT Correction - small readout at bottom
    dash.gauge_cluster
        .components
        .push(DashComponent::Gauge(Box::new(GaugeConfig {
            id: "iatcorr".to_string(),
            title: "IAT%".to_string(),
            units: "%".to_string(),
            output_channel: "iatCorrection".to_string(),
            min: 0.0,
            max: 200.0,
            value_digits: 0,
            gauge_painter: GaugePainter::BasicReadout,
            relative_x: 0.19,
            relative_y: 0.84,
            relative_width: 0.13,
            relative_height: 0.14,
            back_color: TsColor {
                alpha: 255,
                red: 30,
                green: 28,
                blue: 25,
            },
            font_color: TsColor {
                alpha: 255,
                red: 255,
                green: 200,
                blue: 120,
            },
            trim_color: TsColor {
                alpha: 255,
                red: 120,
                green: 90,
                blue: 60,
            },
            ..Default::default()
        })));

    dash
}
