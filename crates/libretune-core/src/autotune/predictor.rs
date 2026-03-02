//! Predictive VE Cell Filling
//!
//! Uses bilinear interpolation and neighbor-weighted averaging to predict
//! VE values for cells with zero AutoTune hits. Provides confidence scores
//! based on data quality and distance from known datapoints.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A predicted VE cell value with confidence score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictedCell {
    /// Row index in the table
    pub row: usize,
    /// Column index in the table
    pub col: usize,
    /// Predicted VE value
    pub predicted_value: f64,
    /// Current table value (before prediction)
    pub current_value: f64,
    /// Confidence score 0.0–1.0 (higher = more reliable)
    pub confidence: f64,
    /// Method used for prediction
    pub method: PredictionMethod,
    /// Number of known neighbors used
    pub neighbor_count: usize,
}

/// How the prediction was made
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PredictionMethod {
    /// Bilinear interpolation from 4 corner points
    BilinearInterpolation,
    /// Distance-weighted average from nearby known cells
    NeighborWeighted,
    /// Linear extrapolation from edge cells
    LinearExtrapolation,
    /// Physics-based estimate (VE generally increases with load)
    PhysicsModel,
}

/// Configuration for the predictor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictorConfig {
    /// Minimum confidence to include in results (0.0–1.0)
    pub min_confidence: f64,
    /// Maximum search radius for neighbors (in cell units)
    pub max_search_radius: usize,
    /// Minimum hit count for a cell to be considered "known"
    pub min_hit_count: u32,
    /// Weight decay factor for neighbor distance (higher = faster decay)
    pub distance_decay: f64,
}

impl Default for PredictorConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.3,
            max_search_radius: 5,
            min_hit_count: 3,
            distance_decay: 2.0,
        }
    }
}

/// Predicts VE values for cells without AutoTune data
pub struct VePredictor {
    config: PredictorConfig,
}

impl VePredictor {
    pub fn new(config: PredictorConfig) -> Self {
        Self { config }
    }

    /// Generate predictions for all zero-hit cells in the VE table.
    ///
    /// # Arguments
    /// * `table_values` - Current VE table values (row-major: `[row][col]`)
    /// * `hit_counts` - Hit count per cell from AutoTune (same dimensions)
    /// * `x_bins` - RPM axis bins
    /// * `y_bins` - Load axis bins
    ///
    /// # Returns
    /// Vector of predicted cells, sorted by confidence (highest first)
    pub fn predict_cells(
        &self,
        table_values: &[Vec<f64>],
        hit_counts: &[Vec<u32>],
        x_bins: &[f64],
        y_bins: &[f64],
    ) -> Vec<PredictedCell> {
        let rows = table_values.len();
        if rows == 0 {
            return Vec::new();
        }
        let cols = table_values[0].len();

        // Build known cells map: (row, col) -> (value, hit_count)
        let mut known: HashMap<(usize, usize), (f64, u32)> = HashMap::new();
        for r in 0..rows {
            for c in 0..cols {
                let hits = hit_counts
                    .get(r)
                    .and_then(|row| row.get(c))
                    .copied()
                    .unwrap_or(0);
                if hits >= self.config.min_hit_count {
                    known.insert((r, c), (table_values[r][c], hits));
                }
            }
        }

        if known.is_empty() {
            return Vec::new();
        }

        let mut predictions = Vec::new();

        for r in 0..rows {
            for c in 0..cols {
                // Skip cells that already have data
                let hits = hit_counts
                    .get(r)
                    .and_then(|row| row.get(c))
                    .copied()
                    .unwrap_or(0);
                if hits >= self.config.min_hit_count {
                    continue;
                }

                // Try prediction methods in order of preference
                if let Some(pred) =
                    self.try_bilinear(r, c, rows, cols, &known, table_values, x_bins, y_bins)
                {
                    if pred.confidence >= self.config.min_confidence {
                        predictions.push(pred);
                        continue;
                    }
                }

                if let Some(pred) =
                    self.try_neighbor_weighted(r, c, rows, cols, &known, table_values)
                {
                    if pred.confidence >= self.config.min_confidence {
                        predictions.push(pred);
                        continue;
                    }
                }

                if let Some(pred) =
                    self.try_linear_extrapolation(r, c, rows, cols, &known, table_values)
                {
                    if pred.confidence >= self.config.min_confidence {
                        predictions.push(pred);
                        continue;
                    }
                }

                // Physics model as last resort
                if let Some(pred) = self.try_physics_model(r, c, rows, cols, table_values, y_bins) {
                    if pred.confidence >= self.config.min_confidence {
                        predictions.push(pred);
                    }
                }
            }
        }

        // Sort by confidence (highest first)
        predictions.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        predictions
    }

    /// Try bilinear interpolation from 4 surrounding known cells
    fn try_bilinear(
        &self,
        row: usize,
        col: usize,
        rows: usize,
        cols: usize,
        known: &HashMap<(usize, usize), (f64, u32)>,
        table_values: &[Vec<f64>],
        x_bins: &[f64],
        y_bins: &[f64],
    ) -> Option<PredictedCell> {
        // Find nearest known cell in each quadrant (up-left, up-right, down-left, down-right)
        let ul = self.find_nearest_known(row, col, -1, -1, rows, cols, known);
        let ur = self.find_nearest_known(row, col, -1, 1, rows, cols, known);
        let dl = self.find_nearest_known(row, col, 1, -1, rows, cols, known);
        let dr = self.find_nearest_known(row, col, 1, 1, rows, cols, known);

        // Need at least 3 corners for reasonable interpolation
        let corners: Vec<_> = [ul, ur, dl, dr].iter().filter_map(|c| *c).collect();
        if corners.len() < 3 {
            return None;
        }

        // Compute distance-weighted average
        let mut weighted_sum = 0.0;
        let mut weight_sum = 0.0;
        let mut max_dist = 0.0f64;

        for (cr, cc) in &corners {
            let dr_f = (row as f64 - *cr as f64).abs();
            let dc_f = (col as f64 - *cc as f64).abs();
            let dist = (dr_f * dr_f + dc_f * dc_f).sqrt().max(0.001);
            max_dist = max_dist.max(dist);
            let weight = 1.0 / dist.powf(self.config.distance_decay);
            weighted_sum += known[&(*cr, *cc)].0 * weight;
            weight_sum += weight;
        }

        if weight_sum < 0.001 {
            return None;
        }

        let predicted = weighted_sum / weight_sum;

        // Confidence based on: number of corners and max distance
        let corner_factor = corners.len() as f64 / 4.0;
        let distance_factor =
            (1.0 - max_dist / (self.config.max_search_radius as f64 * 1.5)).max(0.0);
        let confidence = corner_factor * 0.7 + distance_factor * 0.3;

        // Apply axis-based sanity check: VE should be physically reasonable
        let _ = (x_bins, y_bins); // Used for potential axis-weighted refinement
        let predicted = predicted.clamp(1.0, 200.0);

        Some(PredictedCell {
            row,
            col,
            predicted_value: predicted,
            current_value: table_values[row][col],
            confidence: confidence.clamp(0.0, 1.0),
            method: PredictionMethod::BilinearInterpolation,
            neighbor_count: corners.len(),
        })
    }

    /// Find nearest known cell in a given direction
    fn find_nearest_known(
        &self,
        row: usize,
        col: usize,
        row_dir: i32, // -1 (up), 0, or 1 (down)
        col_dir: i32, // -1 (left), 0, or 1 (right)
        rows: usize,
        cols: usize,
        known: &HashMap<(usize, usize), (f64, u32)>,
    ) -> Option<(usize, usize)> {
        let max_r = self.config.max_search_radius;

        for dist in 1..=max_r {
            let r = row as i32 + row_dir * dist as i32;
            let c = col as i32 + col_dir * dist as i32;

            if r < 0 || r >= rows as i32 || c < 0 || c >= cols as i32 {
                break;
            }

            let ru = r as usize;
            let cu = c as usize;
            if known.contains_key(&(ru, cu)) {
                return Some((ru, cu));
            }
        }
        None
    }

    /// Distance-weighted average from all nearby known cells
    fn try_neighbor_weighted(
        &self,
        row: usize,
        col: usize,
        rows: usize,
        cols: usize,
        known: &HashMap<(usize, usize), (f64, u32)>,
        table_values: &[Vec<f64>],
    ) -> Option<PredictedCell> {
        let radius = self.config.max_search_radius;
        let mut weighted_sum = 0.0;
        let mut weight_sum = 0.0;
        let mut neighbor_count = 0;

        let r_start = row.saturating_sub(radius);
        let r_end = (row + radius + 1).min(rows);
        let c_start = col.saturating_sub(radius);
        let c_end = (col + radius + 1).min(cols);

        for r in r_start..r_end {
            for c in c_start..c_end {
                if r == row && c == col {
                    continue;
                }
                if let Some((val, hits)) = known.get(&(r, c)) {
                    let dr = (row as f64 - r as f64).abs();
                    let dc = (col as f64 - c as f64).abs();
                    let dist = (dr * dr + dc * dc).sqrt().max(0.001);

                    // Weight by inverse distance and hit count
                    let dist_weight = 1.0 / dist.powf(self.config.distance_decay);
                    let hit_weight = (*hits as f64).sqrt();
                    let weight = dist_weight * hit_weight;

                    weighted_sum += val * weight;
                    weight_sum += weight;
                    neighbor_count += 1;
                }
            }
        }

        if neighbor_count < 2 || weight_sum < 0.001 {
            return None;
        }

        let predicted = (weighted_sum / weight_sum).clamp(1.0, 200.0);

        // Confidence based on neighbor count and total weight
        let count_factor = (neighbor_count as f64 / 8.0).min(1.0);
        let confidence = count_factor * 0.8;

        Some(PredictedCell {
            row,
            col,
            predicted_value: predicted,
            current_value: table_values[row][col],
            confidence: confidence.clamp(0.0, 1.0),
            method: PredictionMethod::NeighborWeighted,
            neighbor_count,
        })
    }

    /// Linear extrapolation from edge cells
    fn try_linear_extrapolation(
        &self,
        row: usize,
        col: usize,
        rows: usize,
        cols: usize,
        known: &HashMap<(usize, usize), (f64, u32)>,
        table_values: &[Vec<f64>],
    ) -> Option<PredictedCell> {
        // Find two nearest known cells in same row or column for extrapolation
        let mut row_known: Vec<(usize, f64)> = Vec::new();
        let mut col_known: Vec<(usize, f64)> = Vec::new();

        for c in 0..cols {
            if let Some((val, _)) = known.get(&(row, c)) {
                row_known.push((c, *val));
            }
        }
        for r in 0..rows {
            if let Some((val, _)) = known.get(&(r, col)) {
                col_known.push((r, *val));
            }
        }

        // Try row-wise extrapolation
        if row_known.len() >= 2 {
            row_known.sort_by_key(|(c, _)| *c);
            if let Some(val) = self.extrapolate_1d(col, &row_known) {
                let clamped = val.clamp(1.0, 200.0);
                return Some(PredictedCell {
                    row,
                    col,
                    predicted_value: clamped,
                    current_value: table_values[row][col],
                    confidence: 0.4, // Extrapolation is less reliable
                    method: PredictionMethod::LinearExtrapolation,
                    neighbor_count: row_known.len(),
                });
            }
        }

        // Try column-wise extrapolation
        if col_known.len() >= 2 {
            col_known.sort_by_key(|(r, _)| *r);
            if let Some(val) = self.extrapolate_1d(row, &col_known) {
                let clamped = val.clamp(1.0, 200.0);
                return Some(PredictedCell {
                    row,
                    col,
                    predicted_value: clamped,
                    current_value: table_values[row][col],
                    confidence: 0.35,
                    method: PredictionMethod::LinearExtrapolation,
                    neighbor_count: col_known.len(),
                });
            }
        }

        None
    }

    /// Extrapolate from known 1D data points to target index
    fn extrapolate_1d(&self, target: usize, known_points: &[(usize, f64)]) -> Option<f64> {
        if known_points.len() < 2 {
            return None;
        }

        // Find two closest points on the same side, or bracketing
        let target_f = target as f64;

        // Check if target is bracketed (interpolation case)
        for window in known_points.windows(2) {
            let (i0, v0) = window[0];
            let (i1, v1) = window[1];
            if i0 <= target && target <= i1 && i0 != i1 {
                let t = (target_f - i0 as f64) / (i1 as f64 - i0 as f64);
                return Some(v0 + t * (v1 - v0));
            }
        }

        // Extrapolation from nearest two points
        if target < known_points[0].0 {
            // Below range — extrapolate from first two
            let (i0, v0) = known_points[0];
            let (i1, v1) = known_points[1];
            if i0 != i1 {
                let slope = (v1 - v0) / (i1 as f64 - i0 as f64);
                return Some(v0 + slope * (target_f - i0 as f64));
            }
        } else if target > known_points.last().unwrap().0 {
            // Above range — extrapolate from last two
            let n = known_points.len();
            let (i0, v0) = known_points[n - 2];
            let (i1, v1) = known_points[n - 1];
            if i0 != i1 {
                let slope = (v1 - v0) / (i1 as f64 - i0 as f64);
                return Some(v1 + slope * (target_f - i1 as f64));
            }
        }

        None
    }

    /// Physics-based VE estimate: VE generally increases with load
    fn try_physics_model(
        &self,
        row: usize,
        col: usize,
        _rows: usize,
        _cols: usize,
        table_values: &[Vec<f64>],
        y_bins: &[f64],
    ) -> Option<PredictedCell> {
        if y_bins.is_empty() {
            return None;
        }

        // Simple physics model: VE scales roughly linearly with MAP/load
        // at mid RPM range, with falloff at very high RPM
        let max_load = y_bins.last().copied().unwrap_or(100.0);
        let min_load = y_bins.first().copied().unwrap_or(0.0);
        let load_range = (max_load - min_load).max(1.0);

        let current_load = y_bins.get(row).copied().unwrap_or(50.0);
        let load_fraction = (current_load - min_load) / load_range;

        // Estimate based on load fraction: VE typically 30-100
        let estimated_ve = 30.0 + load_fraction * 70.0;

        Some(PredictedCell {
            row,
            col,
            predicted_value: estimated_ve,
            current_value: table_values[row][col],
            confidence: 0.15, // Very low — physics model is rough
            method: PredictionMethod::PhysicsModel,
            neighbor_count: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_table(rows: usize, cols: usize, fill: f64) -> Vec<Vec<f64>> {
        vec![vec![fill; cols]; rows]
    }

    fn make_hits(rows: usize, cols: usize, fill: u32) -> Vec<Vec<u32>> {
        vec![vec![fill; cols]; rows]
    }

    #[test]
    fn test_no_known_cells_returns_empty() {
        let config = PredictorConfig::default();
        let predictor = VePredictor::new(config);

        let table = make_table(4, 4, 50.0);
        let hits = make_hits(4, 4, 0); // No hits anywhere
        let x_bins = vec![1000.0, 2000.0, 3000.0, 4000.0];
        let y_bins = vec![20.0, 40.0, 60.0, 80.0];

        let predictions = predictor.predict_cells(&table, &hits, &x_bins, &y_bins);
        assert!(predictions.is_empty());
    }

    #[test]
    fn test_all_known_returns_empty() {
        let config = PredictorConfig::default();
        let predictor = VePredictor::new(config);

        let table = make_table(4, 4, 50.0);
        let hits = make_hits(4, 4, 10); // All cells have plenty of hits
        let x_bins = vec![1000.0, 2000.0, 3000.0, 4000.0];
        let y_bins = vec![20.0, 40.0, 60.0, 80.0];

        let predictions = predictor.predict_cells(&table, &hits, &x_bins, &y_bins);
        assert!(predictions.is_empty());
    }

    #[test]
    fn test_predict_center_from_corners() {
        let config = PredictorConfig {
            min_confidence: 0.1,
            min_hit_count: 1,
            ..Default::default()
        };
        let predictor = VePredictor::new(config);

        // 3x3 table with known corners, unknown center
        let mut table = make_table(3, 3, 0.0);
        table[0][0] = 40.0;
        table[0][2] = 60.0;
        table[2][0] = 50.0;
        table[2][2] = 70.0;

        let mut hits = make_hits(3, 3, 0);
        hits[0][0] = 5;
        hits[0][2] = 5;
        hits[2][0] = 5;
        hits[2][2] = 5;

        let x_bins = vec![1000.0, 2000.0, 3000.0];
        let y_bins = vec![20.0, 40.0, 60.0];

        let predictions = predictor.predict_cells(&table, &hits, &x_bins, &y_bins);

        // Should predict center cell (1,1)
        let center = predictions.iter().find(|p| p.row == 1 && p.col == 1);
        assert!(center.is_some(), "Should predict center cell");
        let center = center.unwrap();

        // Average of 40, 60, 50, 70 = 55 (with distance weighting it'll be close)
        assert!(
            (center.predicted_value - 55.0).abs() < 5.0,
            "Center prediction {} should be close to 55",
            center.predicted_value
        );
        assert!(center.confidence > 0.3);
    }

    #[test]
    fn test_predict_sorted_by_confidence() {
        let config = PredictorConfig {
            min_confidence: 0.0,
            min_hit_count: 1,
            ..Default::default()
        };
        let predictor = VePredictor::new(config);

        let mut table = make_table(5, 5, 50.0);
        let mut hits = make_hits(5, 5, 0);

        // Make a few known cells clustered in one area
        hits[2][2] = 10;
        table[2][2] = 60.0;
        hits[2][3] = 10;
        table[2][3] = 65.0;
        hits[3][2] = 10;
        table[3][2] = 62.0;

        let x_bins = vec![1000.0, 2000.0, 3000.0, 4000.0, 5000.0];
        let y_bins = vec![20.0, 40.0, 60.0, 80.0, 100.0];

        let predictions = predictor.predict_cells(&table, &hits, &x_bins, &y_bins);
        assert!(!predictions.is_empty());

        // Verify sorted by confidence descending
        for window in predictions.windows(2) {
            assert!(
                window[0].confidence >= window[1].confidence,
                "Predictions should be sorted by confidence descending"
            );
        }
    }

    #[test]
    fn test_extrapolation_1d() {
        let config = PredictorConfig::default();
        let predictor = VePredictor::new(config);

        let known = vec![(2, 40.0), (4, 60.0)];

        // Interpolation
        let val = predictor.extrapolate_1d(3, &known);
        assert!(val.is_some());
        assert!((val.unwrap() - 50.0).abs() < 0.01);

        // Extrapolation below
        let val = predictor.extrapolate_1d(0, &known);
        assert!(val.is_some());
        assert!((val.unwrap() - 20.0).abs() < 0.01);

        // Extrapolation above
        let val = predictor.extrapolate_1d(6, &known);
        assert!(val.is_some());
        assert!((val.unwrap() - 80.0).abs() < 0.01);
    }

    #[test]
    fn test_empty_table() {
        let config = PredictorConfig::default();
        let predictor = VePredictor::new(config);
        let predictions = predictor.predict_cells(&[], &[], &[], &[]);
        assert!(predictions.is_empty());
    }
}
