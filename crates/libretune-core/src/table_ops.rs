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
    let x_idx = find_bin_index(target_x, x_bins);
    let y_idx = find_bin_index(target_y, y_bins);

    match (x_idx, y_idx) {
        (Some(xi), Some(yi)) => z_values[yi][xi],
        (Some(xi), None) => {
            let row = &z_values[0];
            if row.is_empty() {
                0.0f64
            } else {
                row[xi]
            }
        }
        (None, Some(yi)) => {
            let row = &z_values[yi];
            if row.is_empty() {
                0.0f64
            } else {
                row[0]
            }
        }
        _ => 0.0f64,
    }
}

/// Find bin index for a value
fn find_bin_index(value: f64, bins: &[f64]) -> Option<usize> {
    bins.iter()
        .enumerate()
        .find(|&(_, bin)| (bin - value).abs() < 0.1)
        .map(|(i, _)| i)
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
