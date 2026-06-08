//! Gauge configuration parser
//!
//! Parses [GaugeConfigurations] section for dashboard gauge definitions.

use serde::{Deserialize, Serialize};

/// A gauge configuration for dashboard display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaugeConfig {
    /// Gauge name/identifier
    pub name: String,

    /// Output channel to display
    pub channel: String,

    /// Display title
    pub title: String,

    /// Unit label
    pub units: String,

    /// Low warning threshold
    pub low_warning: f64,

    /// Low danger threshold  
    pub low_danger: f64,

    /// High warning threshold
    pub high_warning: f64,

    /// High danger threshold
    pub high_danger: f64,

    /// Minimum display value
    pub lo: f64,

    /// Maximum display value
    pub hi: f64,

    /// Decimal digits for display
    pub digits: u8,
}

impl GaugeConfig {
    /// Create a new gauge configuration
    pub fn new(name: impl Into<String>, channel: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            channel: channel.into(),
            title: String::new(),
            units: String::new(),
            low_warning: 0.0,
            low_danger: 0.0,
            high_warning: 100.0,
            high_danger: 100.0,
            lo: 0.0,
            hi: 100.0,
            digits: 0,
        }
    }

    /// Check if a value is in the danger zone
    pub fn is_danger(&self, value: f64) -> bool {
        value <= self.low_danger || value >= self.high_danger
    }

    /// Check if a value is in the warning zone
    pub fn is_warning(&self, value: f64) -> bool {
        (value <= self.low_warning && value > self.low_danger)
            || (value >= self.high_warning && value < self.high_danger)
    }

    /// Check if a value is in the normal range
    pub fn is_normal(&self, value: f64) -> bool {
        value > self.low_warning && value < self.high_warning
    }
}

impl Default for GaugeConfig {
    fn default() -> Self {
        Self::new("", "")
    }
}

/// Parse a gauge configuration line
///
/// Format: name = channel, title, units, lo, hi, loD, loW, hiW, hiD, digits
pub fn parse_gauge_line(name: &str, value: &str) -> Option<GaugeConfig> {
    let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();

    if parts.is_empty() {
        return None;
    }

    let mut gauge = GaugeConfig::new(name, parts[0].trim_matches('"'));

    if parts.len() > 1 {
        gauge.title = parts[1].trim_matches('"').to_string();
    }
    if parts.len() > 2 {
        gauge.units = parts[2].trim_matches('"').to_string();
    }
    if parts.len() > 3 {
        gauge.lo = parts[3].parse().unwrap_or(0.0);
    }
    if parts.len() > 4 {
        gauge.hi = parts[4].parse().unwrap_or(100.0);
    }
    if parts.len() > 5 {
        gauge.low_danger = parts[5].parse().unwrap_or(0.0);
    }
    if parts.len() > 6 {
        gauge.low_warning = parts[6].parse().unwrap_or(0.0);
    }
    if parts.len() > 7 {
        gauge.high_warning = parts[7].parse().unwrap_or(100.0);
    }
    if parts.len() > 8 {
        gauge.high_danger = parts[8].parse().unwrap_or(100.0);
    }
    if parts.len() > 9 {
        gauge.digits = parts[9].parse().unwrap_or(0);
    }

    Some(gauge)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gauge_zones() {
        let mut gauge = GaugeConfig::new("rpm", "rpm");
        gauge.low_danger = 500.0;
        gauge.low_warning = 800.0;
        gauge.high_warning = 6500.0;
        gauge.high_danger = 7000.0;

        assert!(gauge.is_danger(400.0));
        assert!(gauge.is_danger(7500.0));
        assert!(gauge.is_warning(600.0));
        assert!(gauge.is_warning(6800.0));
        assert!(gauge.is_normal(3000.0));
    }

    #[test]
    fn test_parse_gauge_line() {
        let gauge = parse_gauge_line(
            "rpmGauge",
            "rpm, \"Engine Speed\", \"RPM\", 0, 8000, 300, 600, 6500, 7000, 0",
        );
        assert!(gauge.is_some());
        let gauge = gauge.unwrap();
        assert_eq!(gauge.channel, "rpm");
        assert_eq!(gauge.title, "Engine Speed");
        assert_eq!(gauge.hi, 8000.0);
    }
}
