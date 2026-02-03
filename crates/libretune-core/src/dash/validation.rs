//! Dashboard validation and error reporting.
//!
//! Provides comprehensive validation of dashboard files with detailed error reporting.

use crate::dash::{DashComponent, DashFile, GaugeConfig, IndicatorConfig};
use crate::ini::EcuDefinition;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

/// Validation errors that can occur when checking dashboards.
#[derive(Debug, Clone, Serialize, Deserialize, Error)]
pub enum ValidationError {
    #[error("Gauge '{gauge_id}' references unknown output channel '{channel}'")]
    UnknownOutputChannel { gauge_id: String, channel: String },

    #[error("Gauge '{gauge_id}' has invalid range: min={min} >= max={max}")]
    InvalidRange {
        gauge_id: String,
        min: f64,
        max: f64,
    },

    #[error("Gauge '{gauge_id}' references missing embedded image '{image_name}'")]
    MissingEmbeddedImage {
        gauge_id: String,
        image_name: String,
    },

    #[error("Gauge '{gauge_id}' has overlapping position with '{other_id}'")]
    OverlappingGauges { gauge_id: String, other_id: String },

    #[error("Indicator '{indicator_id}' references unknown output channel '{channel}'")]
    UnknownIndicatorChannel {
        indicator_id: String,
        channel: String,
    },

    #[error("Dashboard has no gauges or indicators")]
    EmptyDashboard,

    #[error("Gauge '{gauge_id}' uses unsupported painter type (will render as BasicReadout)")]
    UnsupportedPainter { gauge_id: String },
}

/// Warning messages for non-critical issues.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationWarning {
    /// Gauge uses a color that may be hard to see
    PoorContrast { gauge_id: String },
    /// Gauge is very small and may be hard to read
    TinyGauge { gauge_id: String },
    /// Gauge extends beyond dashboard bounds
    OutOfBounds { gauge_id: String },
    /// Gauge overlaps with another gauge
    OverlappingGauges { gauge_id: String, other_id: String },
    /// Dashboard has many components and may be slow
    PerformanceWarning { component_count: usize },
}

/// Complete validation report for a dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Critical errors that prevent proper rendering
    pub errors: Vec<ValidationError>,
    /// Non-critical warnings
    pub warnings: Vec<ValidationWarning>,
    /// Statistics about the dashboard
    pub stats: DashboardStats,
}

/// Statistics about a dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub gauge_count: usize,
    pub indicator_count: usize,
    pub unique_channels: usize,
    pub embedded_image_count: usize,
    pub has_embedded_fonts: bool,
}

impl ValidationReport {
    /// Returns true if there are any critical errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Returns true if there are any warnings.
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Returns true if the dashboard is valid (no errors).
    pub fn is_valid(&self) -> bool {
        !self.has_errors()
    }
}

/// Validate a dashboard file against an INI configuration.
pub fn validate_dashboard(dash: &DashFile, ecu_def: Option<&EcuDefinition>) -> ValidationReport {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Collect available output channels from INI if provided
    let available_channels: Option<HashSet<String>> =
        ecu_def.map(|def| def.output_channels.keys().cloned().collect());

    // Collect all embedded image names for reference validation
    let embedded_images: HashSet<String> = dash
        .gauge_cluster
        .embedded_images
        .iter()
        .map(|img| img.file_name.clone())
        .collect();

    let components = &dash.gauge_cluster.components;

    // Check for empty dashboard
    if components.is_empty() {
        errors.push(ValidationError::EmptyDashboard);
    }

    // Track unique channels and check each component
    let mut unique_channels = HashSet::new();
    let mut gauge_positions: Vec<(String, f64, f64, f64, f64)> = Vec::new();

    for component in components {
        match component {
            DashComponent::Gauge(gauge) => {
                validate_gauge(
                    gauge,
                    &available_channels,
                    &embedded_images,
                    &mut errors,
                    &mut warnings,
                    &mut unique_channels,
                );

                // Track position for overlap detection
                gauge_positions.push((
                    gauge.id.clone(),
                    gauge.relative_x,
                    gauge.relative_y,
                    gauge.relative_width,
                    gauge.relative_height,
                ));
            }
            DashComponent::Indicator(indicator) => {
                validate_indicator(
                    indicator.as_ref(),
                    &available_channels,
                    &mut errors,
                    &mut warnings,
                    &mut unique_channels,
                );
            }
        }
    }

    // Check for overlapping gauges (warning, not error)
    check_overlaps(&gauge_positions, &mut warnings);

    // Performance warning for large dashboards
    if components.len() > 50 {
        warnings.push(ValidationWarning::PerformanceWarning {
            component_count: components.len(),
        });
    }

    // Build statistics
    let stats = DashboardStats {
        gauge_count: components
            .iter()
            .filter(|c| matches!(c, DashComponent::Gauge(_)))
            .count(),
        indicator_count: components
            .iter()
            .filter(|c| matches!(c, DashComponent::Indicator(_)))
            .count(),
        unique_channels: unique_channels.len(),
        embedded_image_count: dash.gauge_cluster.embedded_images.len(),
        has_embedded_fonts: dash
            .gauge_cluster
            .embedded_images
            .iter()
            .any(|img| img.resource_type == crate::dash::ResourceType::Ttf),
    };

    ValidationReport {
        errors,
        warnings,
        stats,
    }
}

fn validate_gauge(
    gauge: &GaugeConfig,
    available_channels: &Option<HashSet<String>>,
    embedded_images: &HashSet<String>,
    errors: &mut Vec<ValidationError>,
    warnings: &mut Vec<ValidationWarning>,
    unique_channels: &mut HashSet<String>,
) {
    // Check output channel exists
    if !gauge.output_channel.is_empty() {
        unique_channels.insert(gauge.output_channel.clone());

        if let Some(ref channels) = available_channels {
            if !channels.contains(&gauge.output_channel) {
                errors.push(ValidationError::UnknownOutputChannel {
                    gauge_id: gauge.id.clone(),
                    channel: gauge.output_channel.clone(),
                });
            }
        }
    }

    // Check min/max range
    if gauge.min >= gauge.max {
        errors.push(ValidationError::InvalidRange {
            gauge_id: gauge.id.clone(),
            min: gauge.min,
            max: gauge.max,
        });
    }

    // Check embedded image references
    if let Some(ref img_name) = gauge.background_image_file_name {
        if !embedded_images.contains(img_name) {
            errors.push(ValidationError::MissingEmbeddedImage {
                gauge_id: gauge.id.clone(),
                image_name: img_name.clone(),
            });
        }
    }
    if let Some(ref img_name) = gauge.needle_image_file_name {
        if !embedded_images.contains(img_name) {
            errors.push(ValidationError::MissingEmbeddedImage {
                gauge_id: gauge.id.clone(),
                image_name: img_name.clone(),
            });
        }
    }

    // Check gauge size
    if gauge.relative_width < 0.05 || gauge.relative_height < 0.05 {
        warnings.push(ValidationWarning::TinyGauge {
            gauge_id: gauge.id.clone(),
        });
    }

    // Check if gauge extends beyond bounds
    if gauge.relative_x + gauge.relative_width > 1.0
        || gauge.relative_y + gauge.relative_height > 1.0
    {
        warnings.push(ValidationWarning::OutOfBounds {
            gauge_id: gauge.id.clone(),
        });
    }
}

fn validate_indicator(
    indicator: &IndicatorConfig,
    available_channels: &Option<HashSet<String>>,
    errors: &mut Vec<ValidationError>,
    _warnings: &mut Vec<ValidationWarning>,
    unique_channels: &mut HashSet<String>,
) {
    // Check output channel exists
    if !indicator.output_channel.is_empty() {
        unique_channels.insert(indicator.output_channel.clone());

        if let Some(ref channels) = available_channels {
            if !channels.contains(&indicator.output_channel) {
                errors.push(ValidationError::UnknownIndicatorChannel {
                    indicator_id: indicator.id.clone(),
                    channel: indicator.output_channel.clone(),
                });
            }
        }
    }
}

fn check_overlaps(
    positions: &[(String, f64, f64, f64, f64)],
    warnings: &mut Vec<ValidationWarning>,
) {
    for i in 0..positions.len() {
        for j in (i + 1)..positions.len() {
            let (id1, x1, y1, w1, h1) = &positions[i];
            let (id2, x2, y2, w2, h2) = &positions[j];

            let rect1 = Rect {
                x: *x1,
                y: *y1,
                w: *w1,
                h: *h1,
            };
            let rect2 = Rect {
                x: *x2,
                y: *y2,
                w: *w2,
                h: *h2,
            };

            // Check for rectangle overlap
            if rectangles_overlap(rect1, rect2) {
                warnings.push(ValidationWarning::OverlappingGauges {
                    gauge_id: id1.clone(),
                    other_id: id2.clone(),
                });
            }
        }
    }
}

#[derive(Copy, Clone)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

fn rectangles_overlap(a: Rect, b: Rect) -> bool {
    !(a.x + a.w < b.x || b.x + b.w < a.x || a.y + a.h < b.y || b.y + b.h < a.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dash::{GaugePainter, TsColor};

    #[test]
    fn test_empty_dashboard_error() {
        let dash = DashFile::default();
        let report = validate_dashboard(&dash, None);

        assert!(report.has_errors());
        assert!(matches!(
            report.errors.first(),
            Some(ValidationError::EmptyDashboard)
        ));
    }

    #[test]
    fn test_invalid_range_error() {
        let mut dash = DashFile::default();
        let mut gauge = GaugeConfig::default();
        gauge.id = "test_gauge".to_string();
        gauge.min = 100.0;
        gauge.max = 50.0; // Invalid: min > max

        dash.gauge_cluster
            .components
            .push(DashComponent::Gauge(Box::new(gauge)));

        let report = validate_dashboard(&dash, None);

        assert!(report.has_errors());
        assert!(report
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::InvalidRange { .. })));
    }
}
