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

/// Smooth table values using weighted average
pub fn smooth_table(
    z_values: &[Vec<f64>],
    selected_cells: Vec<TableCell>,
    factor: f64,
) -> Vec<Vec<f64>> {
    let rows = z_values.len();
    let cols = if rows > 0 { z_values[0].len() } else { 0 };

    let mut result = z_values.to_vec();

    let weights = calculate_smoothing_weights(factor);
    let weight_sum: f64 = weights.iter().sum();

    for (y, x) in selected_cells.iter() {
        let neighbors = get_neighbors(*y, *x, rows, cols);
        let sum: f64 = neighbors
            .iter()
            .enumerate()
            .map(|(i, (ny, nx))| {
                if let Some(val) = get_cell_value(&mut result, *ny, *nx) {
                    val * weights[i]
                } else {
                    0.0f64
                }
            })
            .sum();

        if weight_sum > 0.0f64 {
            result[*y][*x] = sum / weight_sum;
        }
    }

    result
}

/// Calculate smoothing weights based on factor
fn calculate_smoothing_weights(factor: f64) -> Vec<f64> {
    let kernel_size = (factor * 2.0f64) as usize + 1;
    let sigma = factor;
    let mut weights = Vec::with_capacity(kernel_size);

    let center = kernel_size / 2;
    let two_sigma_sq = 2.0f64 * sigma * sigma;

    for i in 0..kernel_size {
        let x = i as f64 - center as f64;
        let weight = (-x * x / two_sigma_sq).exp();
        weights.push(weight);
    }

    weights
}

/// Get neighboring cell coordinates
fn get_neighbors(y: usize, x: usize, rows: usize, cols: usize) -> Vec<(usize, usize)> {
    let mut neighbors = Vec::new();

    for dy in -1i32..=1i32 {
        for dx in -1i32..=1i32 {
            if dy == 0 && dx == 0 {
                continue;
            }

            let ny = (y as i32 + dy) as usize;
            let nx = (x as i32 + dx) as usize;

            if ny < rows && nx < cols {
                neighbors.push((ny, nx));
            }
        }
    }

    neighbors
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

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            if corners.iter().all(|c| c.is_some()) {
                let y_ratio = (y - min_y) as f64 / (max_y - min_y) as f64;
                let x_ratio = (x - min_x) as f64 / (max_x - min_x) as f64;

                let top_left = corners[0].unwrap() * (1.0f64 - y_ratio) * (1.0f64 - x_ratio);
                let top_right = corners[1].unwrap() * (1.0f64 - y_ratio) * x_ratio;
                let bottom_left = corners[2].unwrap() * y_ratio * (1.0f64 - x_ratio);
                let bottom_right = corners[3].unwrap() * y_ratio * x_ratio;

                let interpolated = top_left + top_right + bottom_left + bottom_right;

                result[y][x] = interpolated;
            }
        }
    }

    result
}

/// Set selected cells to a value
pub fn set_cells_equal(z_values: &mut [Vec<f64>], selected_cells: Vec<TableCell>, value: f64) {
    for &(y, x) in selected_cells.iter() {
        if let Some(_) = get_cell_value(z_values, y, x) {
            z_values[y][x] = value;
        }
    }
}
