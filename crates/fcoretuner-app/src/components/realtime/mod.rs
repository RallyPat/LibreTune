//! Realtime Data Components
//!
//! Implements live gauge data display and performance calculations.
//! Based on standard ECU tuning performance and economy fields.

use serde::{Serialize, Deserialize};

/// Realtime gauge data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeChannel {
    pub channel: String,
    pub value: f64,
    pub timestamp: u64,
    pub is_valid: bool,
}

/// Performance calculations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceCalcs {
    /// Vehicle specifications
    pub injector_size_cc: Option<u32>,
    pub weight_lbs: Option<f64>,
    pub frontal_area_sqft: Option<f64>,
    pub frontal_area_sqm: Option<f64>,
    pub tire_pressure_psi: Option<f64>,
    
    /// Calculated values
    pub speed_mph: Option<f64>,
    pub speed_kmh: Option<f64>,
    pub fuel_gallons: Option<f64>,
    pub fuel_liters: Option<f64>,
    pub fuel_gallons_us: Option<f64>,
    pub fuel_kmpl: Option<f64>,
    pub fuel_liters: Option<f64>,
    pub fuel_liters_us: Option<f64>,
    
    /// Calculated performance
    pub hp: Option<f64>,
    pub tq: Option<f64>,
    pub drag: Option<f64>,
    pub rolling_drag: Option<f64>,
    
    /// Calculated economy
    pub mpg: Option<f64>,
    pub mpl: Option<f64>,
    pub kpl: Option<f64>,
}
