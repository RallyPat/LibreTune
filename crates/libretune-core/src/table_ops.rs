//! Table Operations Module
//!
//! Advanced table editing operations.
//! Features: re-binning, smoothing, interpolation, scaling, equalizing.

use serde::{Deserialize, Serialize};

/// Represents a cell coordinate in a table
pub type TableCell = (usize, usize);

/// Result of a table operation
#[derive(Debug, Serialize, Deserialize)]
pub struct TableOperationResult {
    pub table_name: String,
    pub x_bins: Vec<f64>,
    pub y_bins: Vec<f64>,
    pub z_values: Vec<Vec<f64>>,
}

/// Re-bin a table with new X/Y axis bins
pub fn rebin_table(
    old_x_bins: &[f64],
    old_y_bins: &[f64],
    old_z_values: &[Vec<f64>],
    new_x_bins: Vec<f64>,
    new_y_bins: Vec<f64>,
    interpolate_z: bool,
) -> TableOperationResult {
    let new_x_len = new_x_bins.len();
    let new_y_len = new_y_bins.len();

    let mut new_z_values = vec![vec![0.0f64; new_x_len]; new_y_len];

    if interpolate_z {
        for y in 0..new_y_len {
            for x in 0..new_x_len {
                let target_x = new_x_bins[x];
                let target_y = new_y_bins[y];

                new_z_values[y][x] =
                    interpolate_value(target_x, target_y, old_x_bins, old_y_bins, old_z_values);
            }
        }
    } else {
        copy_z_by_index(old_z_values, &mut new_z_values);
    }

    TableOperationResult {
        table_name: "".to_string(),
        x_bins: new_x_bins,
        y_bins: new_y_bins,
        z_values: new_z_values,
    }
}

/// Copy old Z values into the new grid by cell index, with no interpolation.
///
/// Used for `rebin_table(..., interpolate_z: false)`: re-binning without
/// interpolation should keep values in place while only the axes change, so
/// `new_z[y][x] = old_z[y][x]` whenever both indices exist in the old grid.
/// Cells beyond the old grid's bounds (e.g. when growing the table) fall back
/// to the nearest existing old cell by clamping the index, so nothing is
/// zeroed out. An empty old grid leaves `new_z_values` untouched (zeros).
fn copy_z_by_index(old_z_values: &[Vec<f64>], new_z_values: &mut [Vec<f64>]) {
    let old_y_len = old_z_values.len();
    if old_y_len == 0 {
        return;
    }

    for (y, new_row) in new_z_values.iter_mut().enumerate() {
        let old_row = &old_z_values[y.min(old_y_len - 1)];
        let old_x_len = old_row.len();
        if old_x_len == 0 {
            continue;
        }

        for (x, cell) in new_row.iter_mut().enumerate() {
            *cell = old_row[x.min(old_x_len - 1)];
        }
    }
}

/// Bilinear interpolation for a point in a table
fn interpolate_value(
    target_x: f64,
    target_y: f64,
    x_bins: &[f64],
    y_bins: &[f64],
    z_values: &[Vec<f64>],
) -> f64 {
    let (x0, x1, tx) = find_surrounding_indices(target_x, x_bins);
    let (y0, y1, ty) = find_surrounding_indices(target_y, y_bins);

    let v00 = get_value(z_values, y0, x0);
    let v10 = get_value(z_values, y0, x1);
    let v01 = get_value(z_values, y1, x0);
    let v11 = get_value(z_values, y1, x1);

    let top = v00 + (v10 - v00) * tx;
    let bottom = v01 + (v11 - v01) * tx;

    top + (bottom - top) * ty
}

/// Find surrounding bin indices and interpolation ratio (clamped to edges)
fn find_surrounding_indices(value: f64, bins: &[f64]) -> (usize, usize, f64) {
    if bins.is_empty() {
        return (0, 0, 0.0);
    }

    // Clamp below first bin
    if value <= bins[0] {
        return (0, 0, 0.0);
    }

    // Clamp above last bin
    let last_idx = bins.len() - 1;
    if value >= bins[last_idx] {
        return (last_idx, last_idx, 0.0);
    }

    for window in bins.windows(2).enumerate() {
        let (i, pair) = window;
        let left = pair[0];
        let right = pair[1];

        if value >= left && value <= right {
            let span = right - left;
            let ratio = if span.abs() < f64::EPSILON {
                0.0
            } else {
                (value - left) / span
            };
            return (i, i + 1, ratio);
        }
    }

    // Fallback (should not reach here due to early clamps)
    (last_idx, last_idx, 0.0)
}

/// Safe value fetch with bounds checks
fn get_value(z_values: &[Vec<f64>], y: usize, x: usize) -> f64 {
    z_values
        .get(y)
        .and_then(|row| row.get(x))
        .copied()
        .unwrap_or(0.0)
}

/// Smooth table values using 2D Gaussian weighted average
///
/// Each selected cell is replaced with a weighted average of itself and its
/// 8 neighbors (3×3 kernel). Weights are calculated using a 2D Gaussian:
/// `weight = exp(-distance² / (2 × σ²))` where σ = factor.
///
/// - `factor <= 0`: No smoothing, returns original values
/// - `factor = 1.0`: Standard smoothing (center weighted ~1.0, neighbors ~0.6-0.37)
/// - Higher factor: More aggressive smoothing (neighbors weighted closer to center)
pub fn smooth_table(
    z_values: &[Vec<f64>],
    selected_cells: Vec<TableCell>,
    factor: f64,
) -> Vec<Vec<f64>> {
    let rows = z_values.len();
    let cols = if rows > 0 { z_values[0].len() } else { 0 };

    let mut result = z_values.to_vec();

    // No smoothing if factor <= 0
    if factor <= 0.0 {
        return result;
    }

    // Reject (no-op) rather than panic if the selection references cells
    // outside the table's current dimensions — the same guard
    // `interpolate_linear` and `fill_region` carry. "Set Table Size" resizes
    // the grid without clearing the frontend's `selectionRange`, so the next
    // "Smooth" arrives with coordinates that no longer exist; a cell one row
    // past the edge still has an in-bounds `(-1,-1)` neighbour, so
    // `weight_sum > 0.0` and the unchecked `result[y][x]` write panicked.
    if selected_cells.iter().any(|&(y, x)| y >= rows || x >= cols) {
        return result;
    }

    let sigma = factor;
    let two_sigma_sq = 2.0 * sigma * sigma;

    for &(y, x) in selected_cells.iter() {
        let mut sum = 0.0;
        let mut weight_sum = 0.0;

        // Iterate over 3×3 neighborhood including center
        for dy in -1i32..=1i32 {
            for dx in -1i32..=1i32 {
                let ny = y as i32 + dy;
                let nx = x as i32 + dx;

                // Bounds check
                if ny >= 0 && ny < rows as i32 && nx >= 0 && nx < cols as i32 {
                    let val = z_values[ny as usize][nx as usize];
                    // 2D Gaussian weight based on distance from center
                    let dist_sq = (dy * dy + dx * dx) as f64;
                    let weight = (-dist_sq / two_sigma_sq).exp();
                    sum += val * weight;
                    weight_sum += weight;
                }
            }
        }

        if weight_sum > 0.0 {
            result[y][x] = sum / weight_sum;
        }
    }

    result
}

/// Get a cell value safely
fn get_cell_value(z_values: &mut [Vec<f64>], y: usize, x: usize) -> Option<f64> {
    z_values.get(y).and_then(|row| row.get(x).copied())
}

/// Scale cell values by a factor
pub fn scale_cells(
    z_values: &[Vec<f64>],
    selected_cells: Vec<TableCell>,
    scale_factor: f64,
) -> Vec<Vec<f64>> {
    let mut result = z_values.to_vec();

    for &(y, x) in selected_cells.iter() {
        if let Some(val) = get_cell_value(&mut result, y, x) {
            result[y][x] = val * scale_factor;
        }
    }

    result
}

/// Interpolate the selection's bounding box by bilinearly blending its four
/// corners — the equivalent of TunerStudio's "Interpolate" table operation.
///
/// When the selection collapses to a single row or a single column the span
/// along that axis is zero. Previously the ratio was computed as
/// `(y - min_y) / (max_y - min_y)`, i.e. `0.0 / 0.0`, which in Rust evaluates
/// to `NaN` (not a panic) and silently poisoned every cell in the selection —
/// a NaN could then be written straight to the ECU. A degenerate axis is now
/// pinned to ratio `0.0`, which reduces the bilinear blend to a clean linear
/// interpolation along the remaining axis (matching TunerStudio).
pub fn interpolate_cells(z_values: &[Vec<f64>], selected_cells: Vec<TableCell>) -> Vec<Vec<f64>> {
    let mut result = z_values.to_vec();

    if selected_cells.len() < 4 {
        return result;
    }

    let min_x = selected_cells.iter().map(|(_, x)| *x).min().unwrap();
    let max_x = selected_cells.iter().map(|(_, x)| *x).max().unwrap();
    let min_y = selected_cells.iter().map(|(y, _)| *y).min().unwrap();
    let max_y = selected_cells.iter().map(|(y, _)| *y).max().unwrap();

    // Read the four corners with bounds checks. If any corner falls outside the
    // table (e.g. a stale selection after rebin_table shrank the grid) bail out
    // rather than corrupt the data.
    let corner = |y: usize, x: usize| z_values.get(y).and_then(|row| row.get(x)).copied();
    let (top_left, top_right, bottom_left, bottom_right) = match (
        corner(min_y, min_x),
        corner(min_y, max_x),
        corner(max_y, min_x),
        corner(max_y, max_x),
    ) {
        (Some(tl), Some(tr), Some(bl), Some(br)) => (tl, tr, bl, br),
        _ => return result,
    };

    let y_span = (max_y - min_y) as f64;
    let x_span = (max_x - min_x) as f64;

    for (y_idx, row) in result
        .iter_mut()
        .enumerate()
        .skip(min_y)
        .take(max_y - min_y + 1)
    {
        let y_ratio = if y_span > 0.0 {
            (y_idx - min_y) as f64 / y_span
        } else {
            0.0
        };
        for (x_idx, cell) in row
            .iter_mut()
            .enumerate()
            .skip(min_x)
            .take(max_x - min_x + 1)
        {
            let x_ratio = if x_span > 0.0 {
                (x_idx - min_x) as f64 / x_span
            } else {
                0.0
            };

            *cell = top_left * (1.0 - y_ratio) * (1.0 - x_ratio)
                + top_right * (1.0 - y_ratio) * x_ratio
                + bottom_left * y_ratio * (1.0 - x_ratio)
                + bottom_right * y_ratio * x_ratio;
        }
    }

    result
}

/// Set selected cells to a value
pub fn set_cells_equal(z_values: &mut [Vec<f64>], selected_cells: Vec<TableCell>, value: f64) {
    for &(y, x) in selected_cells.iter() {
        if get_cell_value(z_values, y, x).is_some() {
            z_values[y][x] = value;
        }
    }
}

/// Axis for linear interpolation
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum InterpolationAxis {
    Row,
    Col,
}

/// Direction for fill operations
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FillDirection {
    Right,
    Down,
}

/// Add an offset to selected cells
pub fn add_offset(
    z_values: &[Vec<f64>],
    selected_cells: Vec<TableCell>,
    offset: f64,
) -> Vec<Vec<f64>> {
    let mut result = z_values.to_vec();

    for &(y, x) in selected_cells.iter() {
        if let Some(val) = get_cell_value(&mut result, y, x) {
            result[y][x] = val + offset;
        }
    }

    result
}

/// Interpolate selected cells linearly along an axis
#[allow(clippy::needless_range_loop)]
pub fn interpolate_linear(
    z_values: &[Vec<f64>],
    selected_cells: Vec<TableCell>,
    axis: InterpolationAxis,
) -> Vec<Vec<f64>> {
    let mut result = z_values.to_vec();
    if selected_cells.is_empty() {
        return result;
    }

    // Determine bounds
    let mut min_x = usize::MAX;
    let mut max_x = usize::MIN;
    let mut min_y = usize::MAX;
    let mut max_y = usize::MIN;

    for &(y, x) in &selected_cells {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }

    // Reject (no-op) rather than panic if the selection references cells
    // outside the table's current dimensions — e.g. a stale selection left
    // over from before rebin_table shrank the table. Every other function
    // in this module bounds-checks via get_cell_value(); this one and
    // fill_region index directly below, so the check has to happen here.
    let rows = result.len();
    let cols = if rows > 0 { result[0].len() } else { 0 };
    if max_y >= rows || max_x >= cols {
        return result;
    }

    match axis {
        InterpolationAxis::Row => {
            // Horizontal interpolation: for each row y from min_y to max_y
            // Interpolate between value at min_x and max_x
            for y in min_y..=max_y {
                let start_val = result[y][min_x];
                let end_val = result[y][max_x];
                let span = (max_x - min_x) as f64;

                if span > 0.0 {
                    for x in min_x..=max_x {
                        if selected_cells.contains(&(y, x)) {
                            let ratio = (x - min_x) as f64 / span;
                            result[y][x] = start_val + (end_val - start_val) * ratio;
                        }
                    }
                }
            }
        }
        InterpolationAxis::Col => {
            // Vertical interpolation
            for x in min_x..=max_x {
                let start_val = result[min_y][x];
                let end_val = result[max_y][x];
                let span = (max_y - min_y) as f64;

                if span > 0.0 {
                    for y in min_y..=max_y {
                        if selected_cells.contains(&(y, x)) {
                            let ratio = (y - min_y) as f64 / span;
                            result[y][x] = start_val + (end_val - start_val) * ratio;
                        }
                    }
                }
            }
        }
    }

    result
}

/// Fill region from edges
#[allow(clippy::needless_range_loop)]
pub fn fill_region(
    z_values: &[Vec<f64>],
    selected_cells: Vec<TableCell>,
    direction: FillDirection,
) -> Vec<Vec<f64>> {
    let mut result = z_values.to_vec();
    if selected_cells.is_empty() {
        return result;
    }

    // Bounds
    let mut min_x = usize::MAX;
    let mut max_x = usize::MIN;
    let mut min_y = usize::MAX;
    let mut max_y = usize::MIN;

    for &(y, x) in &selected_cells {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }

    // Reject (no-op) rather than panic if the selection references cells
    // outside the table's current dimensions — see interpolate_linear.
    let rows = result.len();
    let cols = if rows > 0 { result[0].len() } else { 0 };
    if max_y >= rows || max_x >= cols {
        return result;
    }

    match direction {
        FillDirection::Right => {
            // Take values from min_x column and propagate right
            for y in min_y..=max_y {
                let source_val = result[y][min_x];
                for x in min_x..=max_x {
                    if selected_cells.contains(&(y, x)) {
                        result[y][x] = source_val;
                    }
                }
            }
        }
        FillDirection::Down => {
            // Take values from min_y row and propagate down
            for x in min_x..=max_x {
                let source_val = result[min_y][x];
                for y in min_y..=max_y {
                    if selected_cells.contains(&(y, x)) {
                        result[y][x] = source_val;
                    }
                }
            }
        }
    }

    result
}
