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
    /// Tolerance for considering two adjacent cell values "identical" when
    /// detecting flat (untuned) regions. Bug #13: previously hardcoded to
    /// 0.01, which was too strict for scaled/rounded tables. Default 0.1 VE.
    pub flat_tolerance: f64,
    /// Load (kPa) below which monotonicity violations are ignored. Bug #10:
    /// very low-load cells can legitimately have non-monotonic VE. Default
    /// 30 kPa.
    pub monotonicity_load_floor_kpa: f64,
    /// RPM below which monotonicity violations are ignored. Default 0 (no floor).
    pub monotonicity_min_rpm: f64,
}

impl Default for AnomalyConfig {
    fn default() -> Self {
        Self {
            outlier_sigma: 2.0,
            gradient_threshold: 2.5,
            min_reasonable_ve: 5.0,
            max_reasonable_ve: 180.0,
            min_flat_region_size: 4,
            flat_tolerance: 0.1,
            monotonicity_load_floor_kpa: 30.0,
            monotonicity_min_rpm: 0.0,
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
        x_bins: &[f64],
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
        self.detect_statistical_outliers(table_values, rows, cols, x_bins, y_bins, &mut anomalies);
        self.detect_monotonicity_violations(
            table_values,
            rows,
            cols,
            x_bins,
            y_bins,
            &mut anomalies,
        );
        self.detect_gradient_discontinuities(table_values, rows, cols, &mut anomalies);
        self.detect_physically_unreasonable(table_values, rows, cols, &mut anomalies);
        self.detect_flat_regions(table_values, rows, cols, &mut anomalies);

        // Sort by severity (highest first)
        anomalies.sort_by(|a, b| {
            b.severity
                .partial_cmp(&a.severity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // De-duplicate: keep highest severity per cell. Bug #18: the old key
        // included the anomaly type, so a single cell could emit multiple
        // anomalies of different types and drown out the most severe one.
        // Since anomalies are already sorted by severity descending, retaining
        // the first entry per (row, col) keeps the highest-severity issue.
        let mut seen = std::collections::HashSet::new();
        anomalies.retain(|a| seen.insert((a.row, a.col)));

        anomalies
    }

    /// Detect cells that are statistical outliers compared to their neighbors.
    ///
    /// Bug #9: previously this compared each cell to the *scalar* mean of its
    /// neighbors, which flagged legitimate ramped surfaces (where neighbors
    /// span a real value range) and corner cells (fewer neighbors). It now
    /// fits a local plane `z = a·rpm + b·load + c` over the neighbors and
    /// flags the cell when its residual from the plane exceeds
    /// `outlier_sigma` times the residual standard deviation.
    #[allow(clippy::needless_range_loop)]
    fn detect_statistical_outliers(
        &self,
        table: &[Vec<f64>],
        rows: usize,
        cols: usize,
        x_bins: &[f64],
        y_bins: &[f64],
        anomalies: &mut Vec<TuneAnomaly>,
    ) {
        for r in 0..rows {
            for c in 0..cols {
                let val = table[r][c];

                // Gather neighbors with their physical (rpm, load) coordinates.
                let mut pts: Vec<(f64, f64, f64)> = Vec::new(); // (X=rpm, Y=load, Z=ve)
                let r_start = r.saturating_sub(1);
                let r_end = (r + 2).min(rows);
                let c_start = c.saturating_sub(1);
                let c_end = (c + 2).min(cols);
                for nr in r_start..r_end {
                    for nc in c_start..c_end {
                        if nr == r && nc == c {
                            continue;
                        }
                        let x = x_bins.get(nc).copied().unwrap_or(nc as f64);
                        let y = y_bins.get(nr).copied().unwrap_or(nr as f64);
                        pts.push((x, y, table[nr][nc]));
                    }
                }

                // Need at least 3 non-collinear points for a plane fit.
                if pts.len() < 3 {
                    continue;
                }

                // Least-squares plane fit z = a·X + b·Y + c.
                // Normal equations (center coordinates to improve conditioning):
                let n = pts.len() as f64;
                let mean_x = pts.iter().map(|p| p.0).sum::<f64>() / n;
                let mean_y = pts.iter().map(|p| p.1).sum::<f64>() / n;
                let sxx = pts.iter().map(|p| (p.0 - mean_x).powi(2)).sum::<f64>();
                let syy = pts.iter().map(|p| (p.1 - mean_y).powi(2)).sum::<f64>();
                let sxy = pts
                    .iter()
                    .map(|p| (p.0 - mean_x) * (p.1 - mean_y))
                    .sum::<f64>();
                let sxz = pts.iter().map(|p| (p.0 - mean_x) * p.2).sum::<f64>();
                let syz = pts.iter().map(|p| (p.1 - mean_y) * p.2).sum::<f64>();
                let mean_z = pts.iter().map(|p| p.2).sum::<f64>() / n;

                // Solve 2x2 for (a, b):  [sxx sxy][a]   [sxz]
                //                       [sxy syy][b] = [syz]
                let det = sxx * syy - sxy * sxy;
                let (expected, residual_std) = if det.abs() > 1e-9 {
                    let a = (syy * sxz - sxy * syz) / det;
                    let b = (sxx * syz - sxy * sxz) / det;
                    let cc = mean_z - a * mean_x - b * mean_y;
                    let cell_x = x_bins.get(c).copied().unwrap_or(c as f64);
                    let cell_y = y_bins.get(r).copied().unwrap_or(r as f64);
                    let expected = a * cell_x + b * cell_y + cc;
                    // Residual std of the fit over the neighbors.
                    let resid: Vec<f64> = pts
                        .iter()
                        .map(|(x, y, z)| z - (a * x + b * y + cc))
                        .collect();
                    let rstd = (resid.iter().map(|v| v.powi(2)).sum::<f64>()
                        / resid.len().max(1) as f64)
                        .sqrt();
                    (expected, rstd)
                } else {
                    // Degenerate (collinear) neighborhood — fall back to mean.
                    (mean_z, 0.0)
                };

                if residual_std < 0.01 {
                    // Neighbors lie on (nearly) a perfect plane. We can't form a
                    // meaningful z-score from a near-zero denominator, but a cell
                    // deviating far from that clean plane is a strong outlier
                    // signal — flag it by an absolute residual threshold.
                    let residual = (val - expected).abs();
                    const CLEAN_PLANE_RESIDUAL_THRESHOLD: f64 = 5.0;
                    if residual > CLEAN_PLANE_RESIDUAL_THRESHOLD {
                        let z_score = residual / 0.01; // residual_std was ~0 → very high
                        let severity =
                            ((z_score - self.config.outlier_sigma) / 3.0).clamp(0.0, 1.0);
                        anomalies.push(TuneAnomaly {
                            row: r,
                            col: c,
                            value: val,
                            expected_value: expected,
                            anomaly_type: AnomalyType::StatisticalOutlier,
                            severity,
                            description: format!(
                                "Cell value {:.1} deviates {:.1} from a clean local plane ({:.1})",
                                val, residual, expected
                            ),
                        });
                    }
                    continue;
                }

                let residual = (val - expected).abs();
                let z_score = residual / residual_std;
                if z_score > self.config.outlier_sigma {
                    let severity = ((z_score - self.config.outlier_sigma) / 3.0).min(1.0);
                    anomalies.push(TuneAnomaly {
                        row: r,
                        col: c,
                        value: val,
                        expected_value: expected,
                        anomaly_type: AnomalyType::StatisticalOutlier,
                        severity,
                        description: format!(
                            "Cell value {:.1} is {:.1}σ from local plane fit ({:.1})",
                            val, z_score, expected
                        ),
                    });
                }
            }
        }
    }

    /// Detect monotonicity violations: VE should generally increase with load.
    /// Bug #10: skips cells below the configured load floor / RPM floor so
    /// idle/decay areas don't generate false positives.
    fn detect_monotonicity_violations(
        &self,
        table: &[Vec<f64>],
        rows: usize,
        cols: usize,
        x_bins: &[f64],
        y_bins: &[f64],
        anomalies: &mut Vec<TuneAnomaly>,
    ) {
        if y_bins.len() < 2 {
            return;
        }

        // Check column-wise (increasing load/MAP should generally increase VE)
        #[allow(clippy::needless_range_loop)]
        for c in 0..cols {
            let rpm = x_bins.get(c).copied().unwrap_or(0.0);
            if rpm < self.config.monotonicity_min_rpm {
                continue;
            }
            for r in 1..rows {
                let load = y_bins.get(r - 1).copied().unwrap_or(0.0);
                if load < self.config.monotonicity_load_floor_kpa {
                    continue;
                }

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
        #[allow(clippy::needless_range_loop)]
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
                    if (table[cr][cc] - val).abs() > self.config.flat_tolerance {
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
    #[allow(dead_code)]
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

        #[allow(clippy::needless_range_loop)]
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

        let table = vec![
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
        // Use a huge outlier sigma, gradient threshold, and monotonicity floor
        // so the extreme cells are reported as PhysicallyUnreasonable rather
        // than being outranked by other anomaly types after the #18 per-cell dedup.
        let config = AnomalyConfig {
            outlier_sigma: 1e9,
            gradient_threshold: 1e9,
            monotonicity_load_floor_kpa: 1e9,
            ..Default::default()
        };
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
    fn test_no_false_outliers_on_ramped_surface() {
        // Bug #9: scalar-neighbor-mean outlier detection flagged ramped table
        // corners. A plane-fit based detector should accept a smooth linear
        // surface with no local deviations.
        let config = AnomalyConfig {
            outlier_sigma: 2.0,
            ..Default::default()
        };
        let detector = AnomalyDetector::new(config);

        // Perfect plane: 48 + 2*rpm_idx + 2*load_idx.
        let table: Vec<Vec<f64>> = (0..4)
            .map(|r| {
                (0..4)
                    .map(|c| 48.0 + 2.0 * r as f64 + 2.0 * c as f64)
                    .collect()
            })
            .collect();
        let x_bins = vec![1000.0, 2000.0, 3000.0, 4000.0];
        let y_bins = vec![20.0, 40.0, 60.0, 80.0];

        let anomalies = detector.detect_anomalies(&table, &x_bins, &y_bins);
        let outliers: Vec<_> = anomalies
            .iter()
            .filter(|a| a.anomaly_type == AnomalyType::StatisticalOutlier)
            .collect();
        assert!(
            outliers.is_empty(),
            "Smooth ramped surface should have no statistical outliers, got {:?}",
            outliers
        );
    }

    #[test]
    fn test_flat_tolerance_config() {
        // Bug #13: flat tolerance should be configurable. With a tolerance of
        // 1.0, a checkerboard of 50.0/50.5 cells is still considered flat; with
        // 0.0, every cell is isolated and below the min_flat_region_size, so
        // no flat flags. A huge outlier sigma prevents the #18 dedup from
        // masking flat flags with StatisticalOutliers.
        let loose_config = AnomalyConfig {
            outlier_sigma: 1e9,
            min_flat_region_size: 4,
            flat_tolerance: 1.0,
            ..Default::default()
        };
        let strict_config = AnomalyConfig {
            outlier_sigma: 1e9,
            min_flat_region_size: 4,
            flat_tolerance: 0.0,
            ..Default::default()
        };

        let table: Vec<Vec<f64>> = (0..4)
            .map(|r| {
                (0..4)
                    .map(|c| if (r + c) % 2 == 0 { 50.0 } else { 50.5 })
                    .collect()
            })
            .collect();
        let x_bins = vec![1000.0, 2000.0, 3000.0, 4000.0];
        let y_bins = vec![20.0, 40.0, 60.0, 80.0];

        let loose = AnomalyDetector::new(loose_config).detect_anomalies(&table, &x_bins, &y_bins);
        let strict = AnomalyDetector::new(strict_config).detect_anomalies(&table, &x_bins, &y_bins);

        let loose_flat = loose
            .iter()
            .filter(|a| a.anomaly_type == AnomalyType::FlatRegion)
            .count();
        let strict_flat = strict
            .iter()
            .filter(|a| a.anomaly_type == AnomalyType::FlatRegion)
            .count();

        assert!(
            loose_flat >= 16,
            "loose tolerance should flag the whole grid as flat, got {}",
            loose_flat
        );
        assert!(
            strict_flat < loose_flat,
            "strict tolerance should produce fewer flat flags ({} vs {})",
            strict_flat,
            loose_flat
        );
    }

    #[test]
    fn test_monotonicity_floor_ignores_low_load_in_anomaly_detector() {
        // Bug #10: a VE drop below the configured load floor should not be
        // flagged as a monotonicity violation.
        let config = AnomalyConfig {
            monotonicity_load_floor_kpa: 35.0,
            ..Default::default()
        };
        let detector = AnomalyDetector::new(config);

        let table = vec![
            vec![50.0, 50.0, 50.0, 50.0],
            vec![30.0, 30.0, 30.0, 30.0], // drop below floor: ignored
            vec![55.0, 55.0, 55.0, 55.0],
            vec![25.0, 25.0, 25.0, 25.0], // drop above floor: flagged
        ];
        let x_bins = vec![1000.0, 2000.0, 3000.0, 4000.0];
        let y_bins = vec![20.0, 40.0, 60.0, 80.0];

        let anomalies = detector.detect_anomalies(&table, &x_bins, &y_bins);
        let mono: Vec<_> = anomalies
            .iter()
            .filter(|a| a.anomaly_type == AnomalyType::MonotonicityViolation)
            .collect();
        // Only the 55 -> 25 drop at row 3 (80 kPa) is above the 35 kPa floor.
        assert!(!mono.is_empty(), "drop above load floor should be flagged");
        assert!(
            mono.iter().all(|a| a.row == 3),
            "only row 3 (80 kPa) violations expected, got {:?}",
            mono
        );
    }

    #[test]
    fn test_dedup_keeps_highest_severity_per_cell() {
        // Bug #18: a cell with multiple anomaly types should emit exactly one
        // anomaly after dedup — the highest-severity one.
        let config = AnomalyConfig {
            outlier_sigma: 1.0,       // make the outlier fire
            gradient_threshold: 1.0,  // make the gradient discontinuity fire
            min_flat_region_size: 16, // disable flat detection for this test
            ..Default::default()
        };
        let detector = AnomalyDetector::new(config);

        let table = vec![
            vec![50.0, 52.0, 54.0, 56.0],
            vec![52.0, 54.0, 56.0, 58.0],
            vec![54.0, 56.0, 200.0, 60.0], // (2,2) is both an outlier and a sharp gradient
            vec![56.0, 58.0, 60.0, 62.0],
        ];
        let x_bins = vec![1000.0, 2000.0, 3000.0, 4000.0];
        let y_bins = vec![20.0, 40.0, 60.0, 80.0];

        let anomalies = detector.detect_anomalies(&table, &x_bins, &y_bins);
        let cell_2_2: Vec<_> = anomalies
            .iter()
            .filter(|a| a.row == 2 && a.col == 2)
            .collect();
        assert_eq!(
            cell_2_2.len(),
            1,
            "cell (2,2) should have exactly one anomaly after dedup, got {:?}",
            cell_2_2
        );
        // The retained anomaly should be the highest severity type for this cell.
        let retained = &cell_2_2[0];
        assert!(
            retained.severity >= 0.9,
            "retained anomaly should be high severity, got {}",
            retained.severity
        );
    }
}
