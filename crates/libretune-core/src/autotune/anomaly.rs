//! Anomaly Detection for ECU Tune Tables
//!
//! Identifies problems in VE/fuel/ignition tables using statistical analysis:
//! - Cells with AFR variance significantly different from neighbors
//! - Monotonicity violations (VE should generally increase with load)
//! - Suspect sensor data (impossible values, stuck readings)
//! - Gradient discontinuities (sharp jumps between adjacent cells)

use serde::{Deserialize, Serialize};

/// A detected anomaly in the tune
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuneAnomaly {
    /// Row index in the table
    pub row: usize,
    /// Column index in the table
    pub col: usize,
    /// Current cell value
    pub value: f64,
    /// Expected value based on neighbors
    pub expected_value: f64,
    /// Type of anomaly detected
    pub anomaly_type: AnomalyType,
    /// Severity 0.0–1.0 (higher = more severe)
    pub severity: f64,
    /// Human-readable description
    pub description: String,
}

/// Categories of anomalies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AnomalyType {
    /// Cell value is a statistical outlier compared to neighbors
    StatisticalOutlier,
    /// VE decreases where it should increase (with load)
    MonotonicityViolation,
    /// Sharp gradient jump between adjacent cells
    GradientDiscontinuity,
    /// Cell value outside physically reasonable range
    PhysicallyUnreasonable,
    /// Flat region: several adjacent cells have identical values (likely untuned)
    FlatRegion,
}

/// Configuration for anomaly detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyConfig {
    /// Number of standard deviations to consider an outlier (default: 2.0)
    pub outlier_sigma: f64,
    /// Minimum gradient ratio to flag as discontinuity (default: 2.5)
    pub gradient_threshold: f64,
    /// Minimum VE value considered physically reasonable
    pub min_reasonable_ve: f64,
    /// Maximum VE value considered physically reasonable
    pub max_reasonable_ve: f64,
    /// Minimum region size to flag as flat (default: 4)
    pub min_flat_region_size: usize,
}

impl Default for AnomalyConfig {
    fn default() -> Self {
        Self {
            outlier_sigma: 2.0,
            gradient_threshold: 2.5,
            min_reasonable_ve: 5.0,
            max_reasonable_ve: 180.0,
            min_flat_region_size: 4,
        }
    }
}

/// Anomaly detector for VE/fuel tables
pub struct AnomalyDetector {
    config: AnomalyConfig,
}

impl AnomalyDetector {
    pub fn new(config: AnomalyConfig) -> Self {
        Self { config }
    }

    /// Analyze a table for anomalies
    ///
    /// # Arguments
    /// * `table_values` - Table values (row-major: `[row][col]`)
    /// * `x_bins` - RPM axis bins
    /// * `y_bins` - Load axis bins
    ///
    /// # Returns
    /// Vector of detected anomalies, sorted by severity (highest first)
    pub fn detect_anomalies(
        &self,
        table_values: &[Vec<f64>],
        _x_bins: &[f64],
        y_bins: &[f64],
    ) -> Vec<TuneAnomaly> {
        let rows = table_values.len();
        if rows == 0 {
            return Vec::new();
        }
        let cols = table_values[0].len();
        if cols == 0 {
            return Vec::new();
        }

        let mut anomalies = Vec::new();

        // Run all detection passes
        self.detect_statistical_outliers(table_values, rows, cols, &mut anomalies);
        self.detect_monotonicity_violations(table_values, rows, cols, y_bins, &mut anomalies);
        self.detect_gradient_discontinuities(table_values, rows, cols, &mut anomalies);
        self.detect_physically_unreasonable(table_values, rows, cols, &mut anomalies);
        self.detect_flat_regions(table_values, rows, cols, &mut anomalies);

        // Sort by severity (highest first)
        anomalies.sort_by(|a, b| {
            b.severity
                .partial_cmp(&a.severity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // De-duplicate: keep highest severity per cell
        let mut seen = std::collections::HashSet::new();
        anomalies.retain(|a| seen.insert((a.row, a.col, format!("{:?}", a.anomaly_type))));

        anomalies
    }

    /// Detect cells that are statistical outliers compared to their neighbors
    fn detect_statistical_outliers(
        &self,
        table: &[Vec<f64>],
        rows: usize,
        cols: usize,
        anomalies: &mut Vec<TuneAnomaly>,
    ) {
        for r in 0..rows {
            for c in 0..cols {
                let val = table[r][c];
                let neighbors = self.get_neighbor_values(table, r, c, rows, cols);
                if neighbors.len() < 3 {
                    continue;
                }

                let mean = neighbors.iter().sum::<f64>() / neighbors.len() as f64;
                let variance = neighbors.iter().map(|n| (n - mean).powi(2)).sum::<f64>()
                    / neighbors.len() as f64;
                let std_dev = variance.sqrt();

                if std_dev < 0.01 {
                    continue; // All neighbors are the same
                }

                let z_score = (val - mean).abs() / std_dev;
                if z_score > self.config.outlier_sigma {
                    let severity = ((z_score - self.config.outlier_sigma) / 3.0).min(1.0);
                    anomalies.push(TuneAnomaly {
                        row: r,
                        col: c,
                        value: val,
                        expected_value: mean,
                        anomaly_type: AnomalyType::StatisticalOutlier,
                        severity,
                        description: format!(
                            "Cell value {:.1} is {:.1}σ from neighbor mean {:.1}",
                            val, z_score, mean
                        ),
                    });
                }
            }
        }
    }

    /// Detect monotonicity violations: VE should generally increase with load
    fn detect_monotonicity_violations(
        &self,
        table: &[Vec<f64>],
        rows: usize,
        cols: usize,
        y_bins: &[f64],
        anomalies: &mut Vec<TuneAnomaly>,
    ) {
        if y_bins.len() < 2 {
            return;
        }

        // Check column-wise (increasing load/MAP should generally increase VE)
        for c in 0..cols {
            for r in 1..rows {
                let prev = table[r - 1][c];
                let curr = table[r][c];
                let load_increasing = y_bins
                    .get(r)
                    .zip(y_bins.get(r - 1))
                    .map(|(a, b)| a > b)
                    .unwrap_or(true);

                if load_increasing && curr < prev * 0.8 && prev > 10.0 {
                    // VE dropped by >20% despite load increasing
                    let drop_pct = ((prev - curr) / prev * 100.0).abs();
                    let severity = (drop_pct / 50.0).min(1.0);
                    anomalies.push(TuneAnomaly {
                        row: r,
                        col: c,
                        value: curr,
                        expected_value: prev,
                        anomaly_type: AnomalyType::MonotonicityViolation,
                        severity,
                        description: format!(
                            "VE dropped {:.0}% ({:.1}→{:.1}) with increasing load",
                            drop_pct, prev, curr
                        ),
                    });
                }
            }
        }
    }

    /// Detect sharp gradient discontinuities between adjacent cells
    fn detect_gradient_discontinuities(
        &self,
        table: &[Vec<f64>],
        rows: usize,
        cols: usize,
        anomalies: &mut Vec<TuneAnomaly>,
    ) {
        // Calculate average local gradient
        let mut gradients = Vec::new();
        for r in 0..rows {
            for c in 0..cols {
                if c + 1 < cols {
                    gradients.push((table[r][c + 1] - table[r][c]).abs());
                }
                if r + 1 < rows {
                    gradients.push((table[r + 1][c] - table[r][c]).abs());
                }
            }
        }

        if gradients.is_empty() {
            return;
        }

        let avg_gradient = gradients.iter().sum::<f64>() / gradients.len() as f64;
        if avg_gradient < 0.01 {
            return;
        }

        let threshold = avg_gradient * self.config.gradient_threshold;

        for r in 0..rows {
            for c in 0..cols {
                let mut max_local_gradient = 0.0f64;
                let mut max_neighbor_val = table[r][c];

                if c + 1 < cols {
                    let g = (table[r][c + 1] - table[r][c]).abs();
                    if g > max_local_gradient {
                        max_local_gradient = g;
                        max_neighbor_val = table[r][c + 1];
                    }
                }
                if c > 0 {
                    let g = (table[r][c - 1] - table[r][c]).abs();
                    if g > max_local_gradient {
                        max_local_gradient = g;
                        max_neighbor_val = table[r][c - 1];
                    }
                }
                if r + 1 < rows {
                    let g = (table[r + 1][c] - table[r][c]).abs();
                    if g > max_local_gradient {
                        max_local_gradient = g;
                        max_neighbor_val = table[r + 1][c];
                    }
                }
                if r > 0 {
                    let g = (table[r - 1][c] - table[r][c]).abs();
                    if g > max_local_gradient {
                        max_local_gradient = g;
                        max_neighbor_val = table[r - 1][c];
                    }
                }

                if max_local_gradient > threshold {
                    let severity = ((max_local_gradient / threshold - 1.0) / 2.0).min(1.0);
                    anomalies.push(TuneAnomaly {
                        row: r,
                        col: c,
                        value: table[r][c],
                        expected_value: max_neighbor_val,
                        anomaly_type: AnomalyType::GradientDiscontinuity,
                        severity,
                        description: format!(
                            "Sharp gradient: {:.1} change vs {:.1} average",
                            max_local_gradient, avg_gradient
                        ),
                    });
                }
            }
        }
    }

    /// Detect physically unreasonable VE values
    fn detect_physically_unreasonable(
        &self,
        table: &[Vec<f64>],
        rows: usize,
        cols: usize,
        anomalies: &mut Vec<TuneAnomaly>,
    ) {
        for r in 0..rows {
            for c in 0..cols {
                let val = table[r][c];
                if val < self.config.min_reasonable_ve {
                    anomalies.push(TuneAnomaly {
                        row: r,
                        col: c,
                        value: val,
                        expected_value: self.config.min_reasonable_ve,
                        anomaly_type: AnomalyType::PhysicallyUnreasonable,
                        severity: 0.8,
                        description: format!(
                            "VE {:.1} below minimum reasonable value {:.1}",
                            val, self.config.min_reasonable_ve
                        ),
                    });
                } else if val > self.config.max_reasonable_ve {
                    anomalies.push(TuneAnomaly {
                        row: r,
                        col: c,
                        value: val,
                        expected_value: self.config.max_reasonable_ve,
                        anomaly_type: AnomalyType::PhysicallyUnreasonable,
                        severity: 0.9,
                        description: format!(
                            "VE {:.1} above maximum reasonable value {:.1}",
                            val, self.config.max_reasonable_ve
                        ),
                    });
                }
            }
        }
    }

    /// Detect flat regions where many adjacent cells have identical values (untuned areas)
    fn detect_flat_regions(
        &self,
        table: &[Vec<f64>],
        rows: usize,
        cols: usize,
        anomalies: &mut Vec<TuneAnomaly>,
    ) {
        let mut visited = vec![vec![false; cols]; rows];

        for r in 0..rows {
            for c in 0..cols {
                if visited[r][c] {
                    continue;
                }

                let val = table[r][c];
                let mut region = Vec::new();
                let mut stack = vec![(r, c)];

                while let Some((cr, cc)) = stack.pop() {
                    if cr >= rows || cc >= cols || visited[cr][cc] {
                        continue;
                    }
                    if (table[cr][cc] - val).abs() > 0.01 {
                        continue;
                    }

                    visited[cr][cc] = true;
                    region.push((cr, cc));

                    // 4-connected neighbors
                    if cr > 0 {
                        stack.push((cr - 1, cc));
                    }
                    if cr + 1 < rows {
                        stack.push((cr + 1, cc));
                    }
                    if cc > 0 {
                        stack.push((cr, cc - 1));
                    }
                    if cc + 1 < cols {
                        stack.push((cr, cc + 1));
                    }
                }

                if region.len() >= self.config.min_flat_region_size {
                    let severity = (region.len() as f64 / (rows * cols) as f64).min(0.8);
                    for (pr, pc) in &region {
                        anomalies.push(TuneAnomaly {
                            row: *pr,
                            col: *pc,
                            value: val,
                            expected_value: val,
                            anomaly_type: AnomalyType::FlatRegion,
                            severity,
                            description: format!(
                                "Part of {} identical cells (value {:.1}) — likely untuned",
                                region.len(),
                                val
                            ),
                        });
                    }
                }
            }
        }
    }

    /// Get neighbor values within 1-cell radius
    fn get_neighbor_values(
        &self,
        table: &[Vec<f64>],
        row: usize,
        col: usize,
        rows: usize,
        cols: usize,
    ) -> Vec<f64> {
        let mut neighbors = Vec::new();
        let r_start = row.saturating_sub(1);
        let r_end = (row + 2).min(rows);
        let c_start = col.saturating_sub(1);
        let c_end = (col + 2).min(cols);

        for r in r_start..r_end {
            for c in c_start..c_end {
                if r == row && c == col {
                    continue;
                }
                neighbors.push(table[r][c]);
            }
        }
        neighbors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_anomalies_in_smooth_table() {
        let config = AnomalyConfig {
            outlier_sigma: 3.0, // Higher threshold to avoid false positives on corners
            ..Default::default()
        };
        let detector = AnomalyDetector::new(config);

        // Smooth table increasing with load (small increments to avoid corner effects)
        let table = vec![
            vec![48.0, 50.0, 52.0, 54.0],
            vec![50.0, 52.0, 54.0, 56.0],
            vec![52.0, 54.0, 56.0, 58.0],
            vec![54.0, 56.0, 58.0, 60.0],
        ];
        let x_bins = vec![1000.0, 2000.0, 3000.0, 4000.0];
        let y_bins = vec![20.0, 40.0, 60.0, 80.0];

        let anomalies = detector.detect_anomalies(&table, &x_bins, &y_bins);
        // No outliers, monotonicity, or gradient issues expected
        let non_flat: Vec<_> = anomalies
            .iter()
            .filter(|a| a.anomaly_type != AnomalyType::FlatRegion)
            .collect();
        assert!(
            non_flat.is_empty(),
            "Smooth table should have no anomalies (except maybe flat), got {:?}",
            non_flat
        );
    }

    #[test]
    fn test_detect_outlier() {
        let config = AnomalyConfig {
            outlier_sigma: 2.0,
            ..Default::default()
        };
        let detector = AnomalyDetector::new(config);

        let mut table = vec![
            vec![50.0, 52.0, 54.0, 56.0],
            vec![52.0, 120.0, 56.0, 58.0], // 120 is a huge outlier
            vec![54.0, 56.0, 58.0, 60.0],
            vec![56.0, 58.0, 60.0, 62.0],
        ];
        let x_bins = vec![1000.0, 2000.0, 3000.0, 4000.0];
        let y_bins = vec![20.0, 40.0, 60.0, 80.0];

        let anomalies = detector.detect_anomalies(&table, &x_bins, &y_bins);
        let outliers: Vec<_> = anomalies
            .iter()
            .filter(|a| {
                a.anomaly_type == AnomalyType::StatisticalOutlier && a.row == 1 && a.col == 1
            })
            .collect();
        assert!(!outliers.is_empty(), "Should detect the outlier at (1,1)");
    }

    #[test]
    fn test_detect_monotonicity_violation() {
        let config = AnomalyConfig::default();
        let detector = AnomalyDetector::new(config);

        // VE drops dramatically at row 2 despite increasing load
        let table = vec![
            vec![40.0, 42.0, 44.0, 46.0],
            vec![50.0, 52.0, 54.0, 56.0],
            vec![20.0, 22.0, 24.0, 26.0], // Big drop!
            vec![60.0, 62.0, 64.0, 66.0],
        ];
        let x_bins = vec![1000.0, 2000.0, 3000.0, 4000.0];
        let y_bins = vec![20.0, 40.0, 60.0, 80.0];

        let anomalies = detector.detect_anomalies(&table, &x_bins, &y_bins);
        let monotonic: Vec<_> = anomalies
            .iter()
            .filter(|a| a.anomaly_type == AnomalyType::MonotonicityViolation)
            .collect();
        assert!(
            !monotonic.is_empty(),
            "Should detect monotonicity violations"
        );
    }

    #[test]
    fn test_detect_flat_region() {
        let config = AnomalyConfig {
            min_flat_region_size: 4,
            ..Default::default()
        };
        let detector = AnomalyDetector::new(config);

        // Large flat region (untuned)
        let table = vec![
            vec![50.0, 50.0, 50.0, 50.0],
            vec![50.0, 50.0, 50.0, 50.0],
            vec![50.0, 50.0, 50.0, 50.0],
            vec![50.0, 50.0, 50.0, 50.0],
        ];
        let x_bins = vec![1000.0, 2000.0, 3000.0, 4000.0];
        let y_bins = vec![20.0, 40.0, 60.0, 80.0];

        let anomalies = detector.detect_anomalies(&table, &x_bins, &y_bins);
        let flat: Vec<_> = anomalies
            .iter()
            .filter(|a| a.anomaly_type == AnomalyType::FlatRegion)
            .collect();
        assert!(!flat.is_empty(), "Should detect flat region");
        assert_eq!(flat.len(), 16, "All 16 cells should be flagged as flat");
    }

    #[test]
    fn test_detect_physically_unreasonable() {
        let config = AnomalyConfig::default();
        let detector = AnomalyDetector::new(config);

        let table = vec![
            vec![50.0, 52.0, 54.0, 250.0], // 250 is above max
            vec![52.0, 54.0, 56.0, 58.0],
            vec![1.0, 56.0, 58.0, 60.0], // 1.0 is below min
            vec![56.0, 58.0, 60.0, 62.0],
        ];
        let x_bins = vec![1000.0, 2000.0, 3000.0, 4000.0];
        let y_bins = vec![20.0, 40.0, 60.0, 80.0];

        let anomalies = detector.detect_anomalies(&table, &x_bins, &y_bins);
        let unreasonable: Vec<_> = anomalies
            .iter()
            .filter(|a| a.anomaly_type == AnomalyType::PhysicallyUnreasonable)
            .collect();
        assert!(
            unreasonable.len() >= 2,
            "Should detect at least 2 physically unreasonable values"
        );
    }

    #[test]
    fn test_empty_table() {
        let detector = AnomalyDetector::new(AnomalyConfig::default());
        let anomalies = detector.detect_anomalies(&[], &[], &[]);
        assert!(anomalies.is_empty());
    }
}
