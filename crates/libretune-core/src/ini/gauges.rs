//! Gauge configuration parser
//!
//! Parses [GaugeConfigurations] section for dashboard gauge definitions.

use serde::{Deserialize, Serialize};

use super::split_ini_line;

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

    /// Raw `{expression}` text for each numeric field that was not a literal
    /// number in the INI (e.g. the Speeduino tachometer's `hi = {rpmhigh}`).
    /// The numeric fields above then hold only a placeholder fallback, and
    /// callers with access to tune/default values must resolve the expression
    /// to get the real value — parse time has no tune loaded to resolve
    /// against. Silently using the fallback is what pegged RPM gauges at 100.
    #[serde(default)]
    pub lo_expr: Option<String>,
    #[serde(default)]
    pub hi_expr: Option<String>,
    #[serde(default)]
    pub low_danger_expr: Option<String>,
    #[serde(default)]
    pub low_warning_expr: Option<String>,
    #[serde(default)]
    pub high_warning_expr: Option<String>,
    #[serde(default)]
    pub high_danger_expr: Option<String>,
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
            lo_expr: None,
            hi_expr: None,
            low_danger_expr: None,
            low_warning_expr: None,
            high_warning_expr: None,
            high_danger_expr: None,
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

/// Parse a numeric gauge field that the INI allows to be either a literal
/// number or an `{expression}`. Returns the literal (or `fallback`) plus the
/// captured expression text (braces stripped) when it isn't a plain number.
fn parse_num_or_expr(raw: &str, fallback: f64) -> (f64, Option<String>) {
    match raw.parse::<f64>() {
        Ok(v) => (v, None),
        Err(_) => {
            let t = raw.trim();
            if let Some(inner) = t.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                (fallback, Some(inner.trim().to_string()))
            } else {
                (fallback, None)
            }
        }
    }
}

/// Parse a gauge configuration line
///
/// Format: name = channel, title, units, lo, hi, loD, loW, hiW, hiD, digits
pub fn parse_gauge_line(name: &str, value: &str) -> Option<GaugeConfig> {
    let parts = split_ini_line(value);

    if parts.is_empty() || parts[0].is_empty() {
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
        (gauge.lo, gauge.lo_expr) = parse_num_or_expr(&parts[3], 0.0);
    }
    if parts.len() > 4 {
        (gauge.hi, gauge.hi_expr) = parse_num_or_expr(&parts[4], 100.0);
    }
    if parts.len() > 5 {
        (gauge.low_danger, gauge.low_danger_expr) = parse_num_or_expr(&parts[5], 0.0);
    }
    if parts.len() > 6 {
        (gauge.low_warning, gauge.low_warning_expr) = parse_num_or_expr(&parts[6], 0.0);
    }
    if parts.len() > 7 {
        (gauge.high_warning, gauge.high_warning_expr) = parse_num_or_expr(&parts[7], 100.0);
    }
    if parts.len() > 8 {
        (gauge.high_danger, gauge.high_danger_expr) = parse_num_or_expr(&parts[8], 100.0);
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
    fn expression_fields_are_captured_not_silently_defaulted() {
        // Real Speeduino 202501 tachometer line: hi/hiW/hiD are {expressions}
        // referencing PcVariables (rpmhigh etc.), not literals. The old parser
        // silently fell back to 100, which pegged RPM gauges at 100 after a
        // range sync. The expression text must be captured so callers can
        // resolve the real value against tune/default values.
        let gauge = parse_gauge_line(
            "tachometer",
            "rpm, \"Engine Speed\", \"RPM\", 0, {rpmhigh}, 300, 600, {rpmwarn}, {rpmdang}, 0, 0",
        )
        .unwrap();
        assert_eq!(gauge.hi_expr.as_deref(), Some("rpmhigh"));
        assert_eq!(gauge.high_warning_expr.as_deref(), Some("rpmwarn"));
        assert_eq!(gauge.high_danger_expr.as_deref(), Some("rpmdang"));
        // Literal fields have no expression captured.
        assert_eq!(gauge.lo_expr, None);
        assert_eq!(gauge.low_danger_expr, None);
        assert_eq!(gauge.lo, 0.0);
        assert_eq!(gauge.low_danger, 300.0);
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

    #[test]
    fn test_parse_gauge_line_braced_title_with_comma() {
        let gauge = parse_gauge_line(
            "gppwmGauge1",
            "gppwmOutput1, { bitStringValue(pwmAxisLabels, gppwm1_loadAxis) }, \"%\", 0, 100, 0, 0, 100, 100, 1",
        )
        .expect("parse");
        assert_eq!(gauge.channel, "gppwmOutput1");
        assert_eq!(
            gauge.title,
            "{ bitStringValue(pwmAxisLabels, gppwm1_loadAxis) }"
        );
        assert_eq!(gauge.units, "%");
        assert_eq!(gauge.hi, 100.0);
        assert_eq!(gauge.digits, 1);
    }
}
