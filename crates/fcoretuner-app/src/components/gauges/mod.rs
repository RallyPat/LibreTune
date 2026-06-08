//! Gauge Components
//!
//! Individual gauge implementations: Analog, Digital, Bar, Sweep, LED.

use crate::dashboards::{Gauge, GaugeType};

/// Gauge component props
pub struct GaugeProps {
    pub gauge: Gauge,
    pub value: f64,
}

/// Analog needle gauge
#[derive(Debug, Clone)]
pub struct AnalogGauge {
    pub min: f64,
    pub max: f64,
    pub start_angle: f64,
    pub sweep_angle: f64,
    pub needle_color: String,
    pub tick_color: String,
    pub subdivisions: u8,
}

/// Digital readout gauge
#[derive(Debug, Clone)]
pub struct DigitalGauge {
    pub decimals: u8,
    pub font: String,
}

/// Bar gauge
#[derive(Debug, Clone)]
pub struct BarGauge {
    pub orientation: 'horizontal' | 'vertical',
}

/// Sweep gauge
#[derive(Debug, Clone)]
pub struct SweepGauge {
    pub from_angle: f64,
    pub to_angle: f64,
}

/// LED indicator
#[derive(Debug, Clone)]
pub struct LEDIndicator {
    pub is_on: bool,
    pub color_on: String,
    pub color_off: String,
}
