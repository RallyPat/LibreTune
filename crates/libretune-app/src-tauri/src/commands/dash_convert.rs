//! Dashboard format conversion helpers (legacy DashboardLayout <-> TS DashFile).

use libretune_core::dash::{
    self, Bibliography, DashComponent, DashFile, GaugePainter, TsColor, VersionInfo,
};
use libretune_core::dash::layout::{DashboardLayout, GaugeConfig as DashboardGaugeConfig};

/// Convert legacy DashboardLayout to TS DashFile format
pub(crate) fn convert_layout_to_dashfile(layout: &DashboardLayout) -> DashFile {
    use libretune_core::dash::{BackgroundStyle, GaugeCluster};
    use libretune_core::dash::layout::GaugeType;

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
            cluster_background_color: TsColor {
                alpha: 255,
                red: 30,
                green: 30,
                blue: 30,
            },
            background_dither_color: None,
            cluster_background_image_file_name: layout.background_image.clone(),
            cluster_background_image_style: BackgroundStyle::Stretch,
            embedded_images: Vec::new(),
            components: Vec::new(),
        },
    };

    for gauge in &layout.gauges {
        let painter = match gauge.gauge_type {
            GaugeType::AnalogDial => GaugePainter::AnalogGauge,
            GaugeType::DigitalReadout => GaugePainter::BasicReadout,
            GaugeType::BarGauge => GaugePainter::HorizontalBarGauge,
            GaugeType::SweepGauge => GaugePainter::AsymmetricSweepGauge,
            GaugeType::LEDIndicator | GaugeType::WarningLight => GaugePainter::BasicReadout,
        };

        let ts_gauge = dash::GaugeConfig {
            id: gauge.id.clone(),
            title: gauge.label.clone(),
            units: gauge.units.clone(),
            output_channel: gauge.channel.clone(),
            min: gauge.min_value,
            max: gauge.max_value,
            low_warning: gauge.low_warning,
            high_warning: gauge.high_warning,
            high_critical: gauge.high_critical,
            value_digits: gauge.decimals as i32,
            relative_x: gauge.x,
            relative_y: gauge.y,
            relative_width: gauge.width,
            relative_height: gauge.height,
            gauge_painter: painter,
            font_color: parse_hex_color(&gauge.font_color),
            needle_color: parse_hex_color(&gauge.needle_color),
            trim_color: parse_hex_color(&gauge.trim_color),
            show_history: gauge.show_history,
            ..Default::default()
        };

        dash.gauge_cluster
            .components
            .push(DashComponent::Gauge(Box::new(ts_gauge)));
    }

    dash
}

/// Convert TS DashFile to legacy DashboardLayout format
pub(crate) fn convert_dashfile_to_layout(dash: &DashFile, name: &str) -> DashboardLayout {
    use libretune_core::dash::layout::GaugeType;

    let mut layout = DashboardLayout {
        name: name.to_string(),
        gauges: Vec::new(),
        is_fullscreen: false,
        background_image: dash
            .gauge_cluster
            .cluster_background_image_file_name
            .clone(),
    };

    for (idx, component) in dash.gauge_cluster.components.iter().enumerate() {
        if let DashComponent::Gauge(ref g) = component {
            let gauge_type = match g.gauge_painter {
                GaugePainter::AnalogGauge
                | GaugePainter::BasicAnalogGauge
                | GaugePainter::CircleAnalogGauge
                | GaugePainter::RoundGauge
                | GaugePainter::RoundDashedGauge
                | GaugePainter::FuelMeter
                | GaugePainter::Tachometer => GaugeType::AnalogDial,
                GaugePainter::BasicReadout => GaugeType::DigitalReadout,
                GaugePainter::HorizontalBarGauge
                | GaugePainter::HorizontalDashedBar
                | GaugePainter::VerticalBarGauge
                | GaugePainter::HorizontalLineGauge
                | GaugePainter::VerticalDashedBar
                | GaugePainter::AnalogBarGauge
                | GaugePainter::AnalogMovingBarGauge
                | GaugePainter::Histogram => GaugeType::BarGauge,
                GaugePainter::AsymmetricSweepGauge => GaugeType::SweepGauge,
                GaugePainter::LineGraph => GaugeType::DigitalReadout, // Deferred
            };

            let config = DashboardGaugeConfig {
                id: if g.id.is_empty() {
                    format!("gauge_{}", idx)
                } else {
                    g.id.clone()
                },
                gauge_type,
                channel: g.output_channel.clone(),
                label: g.title.clone(),
                x: g.relative_x,
                y: g.relative_y,
                width: g.relative_width,
                height: g.relative_height,
                z_index: idx as u32,
                min_value: g.min,
                max_value: g.max,
                low_warning: g.low_warning,
                high_warning: g.high_warning,
                high_critical: g.high_critical,
                decimals: g.value_digits.max(0) as u32,
                units: g.units.clone(),
                font_color: g.font_color.to_css_hex(),
                needle_color: g.needle_color.to_css_hex(),
                trim_color: g.trim_color.to_css_hex(),
                show_history: g.show_history,
                show_min_max: false,
                on_condition: None,
                on_color: None,
                off_color: None,
                blink: None,
            };

            layout.gauges.push(config);
        }
    }

    layout
}

/// Parse a CSS hex color string to TsColor
fn parse_hex_color(hex: &str) -> TsColor {
    let hex = hex.trim_start_matches('#');
    if hex.len() >= 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
        TsColor {
            alpha: 255,
            red: r,
            green: g,
            blue: b,
        }
    } else {
        TsColor::default()
    }
}
