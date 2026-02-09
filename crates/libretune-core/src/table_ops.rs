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
    let _old_x_len = old_x_bins.len();
    let _old_y_len = old_y_bins.len();
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
    }

    TableOperationResult {
        table_name: "".to_string(),
        x_bins: new_x_bins,
        y_bins: new_y_bins,
        z_values: new_z_values,
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

/// Interpolate selected cells between their corners
pub fn interpolate_cells(z_values: &[Vec<f64>], selected_cells: Vec<TableCell>) -> Vec<Vec<f64>> {
    let mut result = z_values.to_vec();

    if selected_cells.len() < 4 {
        return result;
    }

    let mut x_indices: Vec<usize> = Vec::new();
    let mut y_indices: Vec<usize> = Vec::new();

    for (y, x) in selected_cells.iter() {
        x_indices.push(*x);
        y_indices.push(*y);
    }

    let min_x = *x_indices.iter().min().unwrap();
    let max_x = *x_indices.iter().max().unwrap();
    let min_y = *y_indices.iter().min().unwrap();
    let max_y = *y_indices.iter().max().unwrap();

    let mut z_values_mut = z_values.to_vec();

    let corners = [
        get_cell_value(&mut z_values_mut, min_y, min_x),
        get_cell_value(&mut z_values_mut, min_y, max_x),
        get_cell_value(&mut z_values_mut, max_y, min_x),
        get_cell_value(&mut z_values_mut, max_y, max_x),
    ];

    for (y_idx, row) in result
        .iter_mut()
        .enumerate()
        .skip(min_y)
        .take(max_y - min_y + 1)
    {
        let y = y_idx;
        for (x_idx, cell) in row
            .iter_mut()
            .enumerate()
            .skip(min_x)
            .take(max_x - min_x + 1)
        {
            let x = x_idx;
            if corners.iter().all(|c| c.is_some()) {
                let y_ratio = (y - min_y) as f64 / (max_y - min_y) as f64;
                let x_ratio = (x - min_x) as f64 / (max_x - min_x) as f64;

                let top_left = corners[0].unwrap() * (1.0f64 - y_ratio) * (1.0f64 - x_ratio);
                let top_right = corners[1].unwrap() * (1.0f64 - y_ratio) * x_ratio;
                let bottom_left = corners[2].unwrap() * y_ratio * (1.0f64 - x_ratio);
                let bottom_right = corners[3].unwrap() * y_ratio * x_ratio;

                let interpolated = top_left + top_right + bottom_left + bottom_right;

                *cell = interpolated;
            }
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
pub fn fill_region(
    z_values: &[Vec<f64>],
    selected_cells: Vec<TableCell>,
    direction: FillDirection,
) -> Vec<Vec<f64>> {
    let mut result = z_values.to_vec();
    if selected_cells.is_empty() { return result; }
    
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
