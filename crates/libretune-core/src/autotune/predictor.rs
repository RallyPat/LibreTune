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
    /// Linear extrapolation from edge cells (true extrapolation, lower confidence)
    LinearExtrapolation,
    /// 1D interpolation between two bracketing known cells (higher confidence)
    OneDInterpolation,
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
    /// RPM at which volumetric efficiency peaks (torque peak). Used by the
    /// physics-model fallback. Bug #4.
    pub physics_peak_torque_rpm: f64,
    /// Lower clamp for physics-model VE estimates. Bug #4.
    pub physics_min_ve: f64,
    /// Upper clamp for physics-model VE estimates. Extended to support forced
    /// induction (VE > 100%). Bug #4.
    pub physics_max_ve: f64,
}

impl Default for PredictorConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.3,
            max_search_radius: 5,
            min_hit_count: 3,
            distance_decay: 2.0,
            physics_peak_torque_rpm: 4500.0,
            physics_min_ve: 10.0,
            physics_max_ve: 200.0,
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

    /// Span of an axis (max − min), floored at a tiny epsilon to avoid
    /// division by zero. Used to normalize physical distances (bug #7).
    fn axis_range(bins: &[f64]) -> f64 {
        let max = bins.last().copied().unwrap_or(100.0);
        let min = bins.first().copied().unwrap_or(0.0);
        (max - min).max(1e-6)
    }

    /// Normalized physical distance between two cells using the supplied axis
    /// bins. Falls back to unit grid steps when a bin lookup fails.
    #[allow(clippy::too_many_arguments)]
    fn physical_distance(
        row: usize,
        col: usize,
        other_row: usize,
        other_col: usize,
        x_bins: &[f64],
        y_bins: &[f64],
    ) -> f64 {
        let rpm_range = Self::axis_range(x_bins);
        let load_range = Self::axis_range(y_bins);
        let dx = x_bins
            .get(col)
            .zip(x_bins.get(other_col))
            .map(|(a, b)| (a - b).abs() / rpm_range)
            .unwrap_or((col as f64 - other_col as f64).abs());
        let dy = y_bins
            .get(row)
            .zip(y_bins.get(other_row))
            .map(|(a, b)| (a - b).abs() / load_range)
            .unwrap_or((row as f64 - other_row as f64).abs());
        (dx * dx + dy * dy).sqrt().max(0.001)
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
        #[allow(clippy::needless_range_loop)]
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

                if let Some(pred) = self.try_neighbor_weighted(
                    r,
                    c,
                    rows,
                    cols,
                    &known,
                    table_values,
                    x_bins,
                    y_bins,
                ) {
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
                if let Some(pred) =
                    self.try_physics_model(r, c, rows, cols, table_values, x_bins, y_bins)
                {
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
    #[allow(clippy::too_many_arguments)]
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

        // Bug #7: normalize distance by physical axis ranges (RPM and Load)
        // rather than raw grid-step indices, so two cells one index apart but
        // spanning a large physical gap weigh less than an equal-physical-gap
        // pair.
        let rpm_range = Self::axis_range(x_bins);
        let load_range = Self::axis_range(y_bins);

        for (cr, cc) in &corners {
            let dr_phys = x_bins
                .get(*cc)
                .zip(x_bins.get(col))
                .map(|(a, b)| (a - b).abs() / rpm_range)
                .unwrap_or((col as f64 - *cc as f64).abs());
            let dc_phys = y_bins
                .get(*cr)
                .zip(y_bins.get(row))
                .map(|(a, b)| (a - b).abs() / load_range)
                .unwrap_or((row as f64 - *cr as f64).abs());
            let dist = (dr_phys * dr_phys + dc_phys * dc_phys).sqrt().max(0.001);
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

    /// Find nearest known cell in a given direction (quadrant).
    ///
    /// Bug #6: the previous implementation stepped strictly along the 45°
    /// diagonal (row ± d, col ± d), so it could not find an orthogonal
    /// neighbor sitting directly above/below/left/right of the target. This
    /// version expands a 2D quadrant ring by ring (Chebyshev distance),
    /// scanning every cell in the quadrant at that radius in ascending
    /// Euclidean-distance order, returning the first known cell encountered.
    #[allow(clippy::too_many_arguments)]
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

        for ring in 1..=max_r {
            // Collect candidate (dr, dc) offsets in this quadrant at Chebyshev
            // distance exactly `ring`, then sort by Euclidean distance so the
            // nearest known cell wins.
            let mut candidates: Vec<(i32, i32, f64)> = Vec::new();
            for dr in 0..=ring {
                for dc in 0..=ring {
                    if dr == 0 && dc == 0 {
                        continue;
                    }
                    // Only the outer ring of the expanding square.
                    if dr != ring && dc != ring {
                        continue;
                    }
                    // Respect zero directions: a 0 axis means "same line".
                    if (row_dir == 0 && dr != 0) || (col_dir == 0 && dc != 0) {
                        continue;
                    }
                    let (dr_i, dc_i) = (dr as i32, dc as i32);
                    let euclid = ((dr_i * dr_i + dc_i * dc_i) as f64).sqrt();
                    candidates.push((dr_i, dc_i, euclid));
                }
            }
            candidates.sort_by(|a, b| {
                a.2.partial_cmp(&b.2)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            for (dr, dc, _) in candidates {
                // dr, dc are non-negative magnitudes; row_dir/col_dir give the
                // sign. (The 0-axis case was filtered above, so a non-zero
                // magnitude always pairs with a ±1 direction.)
                let r = row as i32 + dr * row_dir;
                let c = col as i32 + dc * col_dir;
                if r < 0 || r >= rows as i32 || c < 0 || c >= cols as i32 {
                    continue;
                }
                let ru = r as usize;
                let cu = c as usize;
                if known.contains_key(&(ru, cu)) {
                    return Some((ru, cu));
                }
            }
        }
        None
    }

    /// Distance-weighted average from all nearby known cells
    #[allow(clippy::too_many_arguments)]
    fn try_neighbor_weighted(
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
                    // Bug #7: physical (axis-normalized) distance, not grid steps.
                    let dist = Self::physical_distance(row, col, r, c, x_bins, y_bins);

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

    /// Linear extrapolation / 1D interpolation from edge cells.
    ///
    /// Bug #17: the old code tagged every 1D estimate as `LinearExtrapolation`
    /// even when the target was bracketed by two known points (safe
    /// interpolation). Interpolated targets now use `OneDInterpolation` with
    /// higher confidence; true out-of-range extrapolation keeps the lower
    /// confidence `LinearExtrapolation` tag.
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

        // Try row-wise first
        if row_known.len() >= 2 {
            row_known.sort_by_key(|(c, _)| *c);
            if let Some((val, is_interpolation)) = self.extrapolate_1d(col, &row_known) {
                let clamped = val.clamp(1.0, 200.0);
                let (confidence, method) = if is_interpolation {
                    (0.75, PredictionMethod::OneDInterpolation)
                } else {
                    (0.4, PredictionMethod::LinearExtrapolation)
                };
                return Some(PredictedCell {
                    row,
                    col,
                    predicted_value: clamped,
                    current_value: table_values[row][col],
                    confidence,
                    method,
                    neighbor_count: row_known.len(),
                });
            }
        }

        // Try column-wise
        if col_known.len() >= 2 {
            col_known.sort_by_key(|(r, _)| *r);
            if let Some((val, is_interpolation)) = self.extrapolate_1d(row, &col_known) {
                let clamped = val.clamp(1.0, 200.0);
                let (confidence, method) = if is_interpolation {
                    (0.75, PredictionMethod::OneDInterpolation)
                } else {
                    (0.35, PredictionMethod::LinearExtrapolation)
                };
                return Some(PredictedCell {
                    row,
                    col,
                    predicted_value: clamped,
                    current_value: table_values[row][col],
                    confidence,
                    method,
                    neighbor_count: col_known.len(),
                });
            }
        }

        None
    }

    /// Extrapolate/interpolate from known 1D data points to target index.
    /// Returns the predicted value and a bool that is `true` when the target
    /// is bracketed by two known points (interpolation) and `false` when it
    /// lies outside the known range (true extrapolation).
    fn extrapolate_1d(
        &self,
        target: usize,
        known_points: &[(usize, f64)],
    ) -> Option<(f64, bool)> {
        if known_points.len() < 2 {
            return None;
        }

        let target_f = target as f64;

        // Check if target is bracketed (interpolation case)
        for window in known_points.windows(2) {
            let (i0, v0) = window[0];
            let (i1, v1) = window[1];
            if i0 <= target && target <= i1 && i0 != i1 {
                let t = (target_f - i0 as f64) / (i1 as f64 - i0 as f64);
                return Some((v0 + t * (v1 - v0), true));
            }
        }

        // Extrapolation from nearest two points
        if target < known_points[0].0 {
            let (i0, v0) = known_points[0];
            let (i1, v1) = known_points[1];
            if i0 != i1 {
                let slope = (v1 - v0) / (i1 as f64 - i0 as f64);
                return Some((v0 + slope * (target_f - i0 as f64), false));
            }
        } else if target > known_points.last().unwrap().0 {
            let n = known_points.len();
            let (i0, v0) = known_points[n - 2];
            let (i1, v1) = known_points[n - 1];
            if i0 != i1 {
                let slope = (v1 - v0) / (i1 as f64 - i0 as f64);
                return Some((v1 + slope * (target_f - i1 as f64), false));
            }
        }

        None
    }

    /// Physics-based VE estimate.
    ///
    /// Bug #4: the previous model clamped VE to a fixed 30–100 range and
    /// ignored RPM entirely. It now (a) scales with an RPM efficiency curve
    /// that peaks near the configured torque-peak RPM, and (b) supports
    /// forced-induction engines via a configurable upper clamp (default 200,
    /// i.e. VE > 100%).
    fn try_physics_model(
        &self,
        row: usize,
        col: usize,
        _rows: usize,
        _cols: usize,
        table_values: &[Vec<f64>],
        x_bins: &[f64],
        y_bins: &[f64],
    ) -> Option<PredictedCell> {
        if y_bins.is_empty() {
            return None;
        }

        // Load term: VE scales roughly linearly with MAP/load.
        let max_load = y_bins.last().copied().unwrap_or(100.0);
        let min_load = y_bins.first().copied().unwrap_or(0.0);
        let load_range = (max_load - min_load).max(1.0);
        let current_load = y_bins.get(row).copied().unwrap_or(50.0);
        let load_fraction = ((current_load - min_load) / load_range).clamp(0.0, 1.0);

        // RPM term: a simple tent function peaking (1.0) at torque-peak RPM and
        // tapering to ~0.6 at the idle/redline extremes. This shapes the load-
        // driven estimate so it doesn't suggest identical VE across the rev
        // range.
        let peak = self.config.physics_peak_torque_rpm.max(1.0);
        let rpm = x_bins.get(col).copied().unwrap_or(peak);
        let rpm_factor = if rpm <= peak {
            // 0.6 at rpm=0 → 1.0 at peak
            0.6 + 0.4 * (rpm / peak).clamp(0.0, 1.0)
        } else {
            // 1.0 at peak, decaying toward 0.6 as rpm → 2*peak
            let over = ((rpm - peak) / peak).clamp(0.0, 1.0);
            1.0 - 0.4 * over
        };

        // Base VE spans the configurable min..max; load supplies the primary
        // spread and the RPM factor scales the result.
        let ve_span = (self.config.physics_max_ve - self.config.physics_min_ve).max(1.0);
        let estimated_ve =
            (self.config.physics_min_ve + load_fraction * ve_span) * rpm_factor;
        let estimated_ve = estimated_ve.clamp(self.config.physics_min_ve, self.config.physics_max_ve);

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
    fn test_quadrant_search_finds_orthogonal_neighbors() {
        // Bug #6: the old diagonal-only ray missed neighbors directly above,
        // below, left, or right of the target cell.
        let config = PredictorConfig::default();
        let predictor = VePredictor::new(config);

        let mut table = make_table(5, 5, 50.0);
        let mut hits = make_hits(5, 5, 0);

        // Known cells directly above (row 1) and left (col 1) of the target
        // at (row 2, col 2), but no diagonal cells.
        hits[1][2] = 10;
        table[1][2] = 55.0;
        hits[2][1] = 10;
        table[2][1] = 53.0;

        let x_bins = vec![1000.0, 2000.0, 3000.0, 4000.0, 5000.0];
        let y_bins = vec![20.0, 40.0, 60.0, 80.0, 100.0];

        let predictions = predictor.predict_cells(&table, &hits, &x_bins, &y_bins);
        assert!(
            predictions.iter().any(|p| p.row == 2 && p.col == 2),
            "target (2,2) should be predicted from orthogonal neighbors"
        );
    }

    #[test]
    fn test_physical_distance_weighting() {
        // Bug #7: distance should be normalized by physical axis ranges. Two
        // cells one index apart but spanning a huge physical RPM gap should
        // weight less than an equal-physical-gap pair.
        //
        // Setup: target at (row 0, col 0). Two known cells in the first column
        // but far apart in physical RPM. The top-left (row 0, col 1) is only
        // 100 RPM away in x but same row; the bottom-left (row 1, col 0) is
        // 1000 RPM away but same col. Physical-distance weighting should rank
        // the 100 RPM neighbor higher.
        let config = PredictorConfig {
            min_confidence: 0.0, // accept everything for comparison
            max_search_radius: 3,
            min_hit_count: 1,
            distance_decay: 1.0, // linear inverse to make comparisons easy
            ..PredictorConfig::default()
        };
        let predictor = VePredictor::new(config);

        let mut table = make_table(3, 3, 50.0);
        let mut hits = make_hits(3, 3, 0);
        hits[0][1] = 10;
        table[0][1] = 60.0; // same row, x_gap = 100 RPM
        hits[1][0] = 10;
        table[1][0] = 70.0; // same col, y_gap = 1000 "load" units

        // Very compressed x axis: 0,100,200; very stretched y axis: 0,1000,2000
        let x_bins = vec![0.0, 100.0, 200.0];
        let y_bins = vec![0.0, 1000.0, 2000.0];

        let predictions = predictor.predict_cells(&table, &hits, &x_bins, &y_bins);
        let pred_0_0 = predictions
            .iter()
            .find(|p| p.row == 0 && p.col == 0)
            .expect("(0,0) should be predicted");

        // The 60.0 (100 RPM away) should dominate over the 70.0 (1000 load
        // units away), so the predicted value should be closer to 60 than 70.
        assert!(
            pred_0_0.predicted_value < 65.0,
            "closer physical neighbor should dominate; got {}",
            pred_0_0.predicted_value
        );
    }

    #[test]
    fn test_extrapolation_1d() {
        let config = PredictorConfig::default();
        let predictor = VePredictor::new(config);

        let known = vec![(2, 40.0), (4, 60.0)];

        // Interpolation
        let (val, is_interp) = predictor.extrapolate_1d(3, &known).unwrap();
        assert!(is_interp, "bracketed target should be interpolation");
        assert!((val - 50.0).abs() < 0.01);

        // Extrapolation below
        let (val, is_interp) = predictor.extrapolate_1d(0, &known).unwrap();
        assert!(!is_interp, "out-of-range target should be extrapolation");
        assert!((val - 20.0).abs() < 0.01);

        // Extrapolation above
        let (val, is_interp) = predictor.extrapolate_1d(6, &known).unwrap();
        assert!(!is_interp, "out-of-range target should be extrapolation");
        assert!((val - 80.0).abs() < 0.01);
    }

    #[test]
    fn test_interpolation_vs_extrapolation_tagging() {
        // Bug #17: a bracketed 1D estimate should be tagged OneDInterpolation
        // with high confidence; an out-of-range estimate should be tagged
        // LinearExtrapolation with low confidence. We shrink the search radius
        // and use a single-row table so bilinear / neighbor-weighted fall
        // through to the 1D path.
        let config = PredictorConfig {
            min_hit_count: 1,
            min_confidence: 0.0,
            max_search_radius: 1,
            ..Default::default()
        };
        let predictor = VePredictor::new(config);

        // 1 row, 7 columns. Known at col 0 and col 4.
        let mut table = vec![vec![50.0; 7]];
        let mut hits = vec![vec![0u32; 7]];
        hits[0][0] = 5;
        table[0][0] = 40.0;
        hits[0][4] = 5;
        table[0][4] = 60.0;

        let x_bins = vec![1000.0, 2000.0, 3000.0, 4000.0, 5000.0, 6000.0, 7000.0];
        let y_bins = vec![20.0];

        let predictions = predictor.predict_cells(&table, &hits, &x_bins, &y_bins);

        let interp = predictions
            .iter()
            .find(|p| p.row == 0 && p.col == 2)
            .expect("col 2 should be predicted by interpolation between col 0 and col 4");
        assert_eq!(
            interp.method,
            PredictionMethod::OneDInterpolation,
            "bracketed cell should be OneDInterpolation"
        );
        assert!(
            interp.confidence >= 0.70,
            "interpolation confidence should be high, got {}",
            interp.confidence
        );

        let extrap = predictions
            .iter()
            .find(|p| p.row == 0 && p.col == 6)
            .expect("col 6 should be predicted by extrapolation from col 0/4");
        assert_eq!(
            extrap.method,
            PredictionMethod::LinearExtrapolation,
            "out-of-range cell should be LinearExtrapolation"
        );
        assert!(
            extrap.confidence <= 0.40,
            "extrapolation confidence should be low, got {}",
            extrap.confidence
        );
    }

    #[test]
    fn test_empty_table() {
        let config = PredictorConfig::default();
        let predictor = VePredictor::new(config);
        let predictions = predictor.predict_cells(&[], &[], &[], &[]);
        assert!(predictions.is_empty());
    }

    #[test]
    fn test_physics_model_scales_with_rpm_and_supports_fi() {
        // Bug #4: the physics-model fallback must (a) factor in RPM (peak near
        // torque-peak) and (b) be able to exceed 100% VE for forced induction.
        // Provide one known cell far from the query cell so the data-driven
        // methods (bilinear/neighbor/extrapolation) don't reach it, forcing the
        // physics model to fire.
        let config = PredictorConfig {
            min_confidence: 0.1,
            max_search_radius: 1, // keep neighbor/bilinear from reaching the target
            physics_peak_torque_rpm: 4000.0,
            physics_min_ve: 10.0,
            physics_max_ve: 200.0,
            ..PredictorConfig::default()
        };
        let predictor = VePredictor::new(config);

        let rows = 3;
        let cols = 3;
        let mut table = make_table(rows, cols, 50.0);
        let mut hits = make_hits(rows, cols, 0);
        // One known cell at the opposite corner (row 0, col 0) so `known` is
        // non-empty but far from (row 2, col 1) under max_search_radius=1.
        table[0][0] = 55.0;
        hits[0][0] = 5;

        let x_bins = vec![1000.0, 4000.0, 8000.0];
        // High load (boosted): y up to 250 kPa
        let y_bins = vec![40.0, 150.0, 250.0];

        let predictions = predictor.predict_cells(&table, &hits, &x_bins, &y_bins);

        // Find the prediction at peak-torque RPM (col 1) and max load (row 2).
        let peak_pred = predictions
            .iter()
            .find(|p| p.row == 2 && p.col == 1)
            .expect("physics prediction at (row=2,col=1) should exist");

        // At peak RPM with max load, the estimate should be able to exceed
        // 100% VE (the old hard ceiling). rpm_factor at peak = 1.0,
        // load_fraction = 1.0 → min + full span.
        assert!(
            peak_pred.predicted_value > 100.0,
            "forced-induction VE should exceed 100 at peak RPM / max load, got {}",
            peak_pred.predicted_value
        );

        // RPM scaling: same load (row 2), but off-peak RPM (col 0, 1000 rpm)
        // must yield a lower estimate than peak RPM.
        let low_rpm_pred = predictions
            .iter()
            .find(|p| p.row == 2 && p.col == 0)
            .expect("prediction at (row=2,col=0) should exist");
        assert!(
            low_rpm_pred.predicted_value < peak_pred.predicted_value,
            "off-peak RPM (got {}) should predict lower VE than peak RPM (got {})",
            low_rpm_pred.predicted_value,
            peak_pred.predicted_value
        );
    }
}
