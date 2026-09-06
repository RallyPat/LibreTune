//! Tests for table operations

use libretune_core::table_ops::{
    interpolate_cells, rebin_table, scale_cells, set_cells_equal, smooth_table,
};

#[test]
fn test_rebin_table_same_size() {
    let old_x_bins = vec![500.0, 1000.0, 2000.0, 3000.0];
    let old_y_bins = vec![20.0, 40.0, 60.0, 80.0];
    let old_z_values = vec![
        vec![10.0, 15.0, 20.0, 25.0],
        vec![20.0, 25.0, 30.0, 35.0],
        vec![30.0, 35.0, 40.0, 45.0],
        vec![40.0, 45.0, 50.0, 55.0],
    ];

    let result = rebin_table(
        &old_x_bins,
        &old_y_bins,
        &old_z_values,
        old_x_bins.clone(),
        old_y_bins.clone(),
        true,
    );

    assert_eq!(result.x_bins, old_x_bins);
    assert_eq!(result.y_bins, old_y_bins);
    // With same bins and interpolation, values should match
    for (y, row) in result.z_values.iter().enumerate() {
        for (x, &val) in row.iter().enumerate() {
            assert!(
                (val - old_z_values[y][x]).abs() < 0.01,
                "Mismatch at [{}, {}]: {} vs {}",
                x,
                y,
                val,
                old_z_values[y][x]
            );
        }
    }
}

#[test]
fn test_rebin_table_smaller() {
    let old_x_bins = vec![500.0, 1000.0, 2000.0, 3000.0];
    let old_y_bins = vec![20.0, 40.0, 60.0, 80.0];
    let old_z_values = vec![
        vec![10.0, 15.0, 20.0, 25.0],
        vec![20.0, 25.0, 30.0, 35.0],
        vec![30.0, 35.0, 40.0, 45.0],
        vec![40.0, 45.0, 50.0, 55.0],
    ];

    let new_x_bins = vec![500.0, 3000.0];
    let new_y_bins = vec![20.0, 80.0];

    let result = rebin_table(
        &old_x_bins,
        &old_y_bins,
        &old_z_values,
        new_x_bins.clone(),
        new_y_bins.clone(),
        true,
    );

    assert_eq!(result.x_bins.len(), 2);
    assert_eq!(result.y_bins.len(), 2);
    assert_eq!(result.z_values.len(), 2);
    assert_eq!(result.z_values[0].len(), 2);
}

#[test]
fn test_rebin_table_bin_values_persisted() {
    let old_x_bins = vec![1000.0, 2000.0, 3000.0];
    let old_y_bins = vec![20.0, 40.0, 60.0];
    let old_z_values = vec![
        vec![10.0, 20.0, 30.0],
        vec![15.0, 25.0, 35.0],
        vec![20.0, 30.0, 40.0],
    ];

    let new_x_bins = vec![1200.0, 2400.0, 3600.0, 4800.0];
    let new_y_bins = vec![25.0, 50.0];

    let result = rebin_table(
        &old_x_bins,
        &old_y_bins,
        &old_z_values,
        new_x_bins.clone(),
        new_y_bins.clone(),
        true,
    );

    assert_eq!(result.x_bins, new_x_bins);
    assert_eq!(result.y_bins, new_y_bins);
    assert_eq!(result.z_values.len(), 2);
    assert_eq!(result.z_values[0].len(), 4);
}

#[test]
fn test_rebin_table_interpolates_shifted_bins() {
    // z = 10*y + x across bins for predictable bilinear interpolation
    let old_x_bins = vec![0.0, 10.0];
    let old_y_bins = vec![0.0, 10.0];
    let old_z_values = vec![
        vec![0.0, 10.0],    // y = 0
        vec![100.0, 110.0], // y = 10
    ];

    let new_x_bins = vec![5.0];
    let new_y_bins = vec![5.0];

    let result = rebin_table(
        &old_x_bins,
        &old_y_bins,
        &old_z_values,
        new_x_bins,
        new_y_bins,
        true,
    );

    assert_eq!(result.z_values.len(), 1);
    assert_eq!(result.z_values[0].len(), 1);
    let interpolated = result.z_values[0][0];

    // Expected bilinear interpolation at (5,5): 55.0
    assert!(
        (interpolated - 55.0).abs() < 0.001,
        "Expected ~55.0 at (5,5), got {}",
        interpolated
    );
}

#[test]
fn test_rebin_table_clamps_outside_range() {
    let old_x_bins = vec![0.0, 10.0];
    let old_y_bins = vec![0.0, 10.0];
    let old_z_values = vec![vec![0.0, 10.0], vec![100.0, 110.0]];

    let new_x_bins = vec![-5.0, 0.0, 15.0];
    let new_y_bins = vec![-2.0, 0.0, 12.0];

    let result = rebin_table(
        &old_x_bins,
        &old_y_bins,
        &old_z_values,
        new_x_bins,
        new_y_bins,
        true,
    );

    assert_eq!(result.z_values.len(), 3);
    assert_eq!(result.z_values[0].len(), 3);

    // Clamped to first bin -> top-left value
    assert!((result.z_values[0][0] - 0.0).abs() < 0.001);
    // Exact bin matches should be preserved
    assert!((result.z_values[1][1] - 0.0).abs() < 0.001);
    // Clamped to last bin -> bottom-right value
    assert!((result.z_values[2][2] - 110.0).abs() < 0.001);
}

/// Regression: `interpolate_z: false` must not zero out the table. It should
/// copy old Z values by cell index, preserving them when dims are unchanged.
#[test]
fn test_rebin_table_no_interpolate_same_size_preserves_values() {
    let old_x_bins = vec![500.0, 1000.0, 2000.0, 3000.0];
    let old_y_bins = vec![20.0, 40.0, 60.0, 80.0];
    let old_z_values = vec![
        vec![10.0, 15.0, 20.0, 25.0],
        vec![20.0, 25.0, 30.0, 35.0],
        vec![30.0, 35.0, 40.0, 45.0],
        vec![40.0, 45.0, 50.0, 55.0],
    ];

    let result = rebin_table(
        &old_x_bins,
        &old_y_bins,
        &old_z_values,
        old_x_bins.clone(),
        old_y_bins.clone(),
        false,
    );

    assert_eq!(result.z_values, old_z_values);
}

/// Growing the grid with `interpolate_z: false` must preserve the original
/// cells by index and fill new edge cells with the nearest (clamped) old
/// cell rather than zeros.
#[test]
fn test_rebin_table_no_interpolate_grow_clamps_to_nearest_old_cell() {
    let old_x_bins = vec![1.0, 2.0, 3.0];
    let old_y_bins = vec![1.0, 2.0, 3.0];
    let old_z_values = vec![
        vec![1.0, 2.0, 3.0],
        vec![4.0, 5.0, 6.0],
        vec![7.0, 8.0, 9.0],
    ];

    let new_x_bins = vec![1.0, 2.0, 3.0, 4.0];
    let new_y_bins = vec![1.0, 2.0, 3.0, 4.0];

    let result = rebin_table(
        &old_x_bins,
        &old_y_bins,
        &old_z_values,
        new_x_bins,
        new_y_bins,
        false,
    );

    // Original 3x3 block preserved by index.
    for (y, old_row) in old_z_values.iter().enumerate() {
        assert_eq!(&result.z_values[y][..3], &old_row[..]);
    }

    // New row/col at index 3 has no old counterpart, so it clamps to the
    // nearest existing old cell (index 2, the old grid's last row/col).
    assert_eq!(result.z_values[3][0], old_z_values[2][0]);
    assert_eq!(result.z_values[0][3], old_z_values[0][2]);
    assert_eq!(result.z_values[3][3], old_z_values[2][2]);
}

/// Shrinking the grid with `interpolate_z: false` must keep the top-left
/// block of old values (every new index still exists in the old grid).
#[test]
fn test_rebin_table_no_interpolate_shrink_keeps_top_left() {
    let old_x_bins = vec![1.0, 2.0, 3.0, 4.0];
    let old_y_bins = vec![1.0, 2.0, 3.0, 4.0];
    let old_z_values = vec![
        vec![1.0, 2.0, 3.0, 4.0],
        vec![5.0, 6.0, 7.0, 8.0],
        vec![9.0, 10.0, 11.0, 12.0],
        vec![13.0, 14.0, 15.0, 16.0],
    ];

    let new_x_bins = vec![1.0, 2.0];
    let new_y_bins = vec![1.0, 2.0];

    let result = rebin_table(
        &old_x_bins,
        &old_y_bins,
        &old_z_values,
        new_x_bins,
        new_y_bins,
        false,
    );

    assert_eq!(result.z_values, vec![vec![1.0, 2.0], vec![5.0, 6.0]]);
}

/// An empty old grid (e.g. a brand-new table) must not panic and should
/// return zeros for the new grid.
#[test]
fn test_rebin_table_no_interpolate_empty_old_grid_returns_zeros_no_panic() {
    let old_x_bins: Vec<f64> = vec![];
    let old_y_bins: Vec<f64> = vec![];
    let old_z_values: Vec<Vec<f64>> = vec![];

    let new_x_bins = vec![1.0, 2.0];
    let new_y_bins = vec![1.0, 2.0];

    let result = rebin_table(
        &old_x_bins,
        &old_y_bins,
        &old_z_values,
        new_x_bins,
        new_y_bins,
        false,
    );

    assert_eq!(result.z_values, vec![vec![0.0, 0.0], vec![0.0, 0.0]]);
}

#[test]
fn test_smooth_table() {
    let z_values = vec![
        vec![10.0, 10.0, 10.0],
        vec![10.0, 50.0, 10.0], // Center cell is an outlier
        vec![10.0, 10.0, 10.0],
    ];

    let selected_cells = vec![(1, 1)]; // Select the center cell
    let smoothed = smooth_table(&z_values, selected_cells, 1.0);

    // The center cell should be smoothed toward neighbors
    assert!(
        smoothed[1][1] < 50.0,
        "Smoothed value {} should be less than original outlier 50.0",
        smoothed[1][1]
    );
    assert!(
        smoothed[1][1] > 10.0,
        "Smoothed value {} should be greater than neighbors 10.0",
        smoothed[1][1]
    );
}

#[test]
fn test_smooth_table_corner_cell() {
    // Test smoothing a corner cell (only 4 neighbors + center = 4 cells in bounds)
    let z_values = vec![
        vec![50.0, 10.0, 10.0],
        vec![10.0, 10.0, 10.0],
        vec![10.0, 10.0, 10.0],
    ];

    let selected_cells = vec![(0, 0)]; // Top-left corner
    let smoothed = smooth_table(&z_values, selected_cells, 1.0);

    // Corner should be smoothed toward its available neighbors
    assert!(
        smoothed[0][0] < 50.0,
        "Corner value {} should be smoothed down from 50.0",
        smoothed[0][0]
    );
    assert!(
        smoothed[0][0] > 10.0,
        "Corner value {} should still be above neighbor values 10.0",
        smoothed[0][0]
    );
}

#[test]
fn test_smooth_table_edge_cell() {
    // Test smoothing an edge cell (6 neighbors + center = 6 cells in bounds)
    let z_values = vec![
        vec![10.0, 50.0, 10.0],
        vec![10.0, 10.0, 10.0],
        vec![10.0, 10.0, 10.0],
    ];

    let selected_cells = vec![(0, 1)]; // Top edge, middle column
    let smoothed = smooth_table(&z_values, selected_cells, 1.0);

    // Edge cell should be smoothed toward its available neighbors
    assert!(
        smoothed[0][1] < 50.0,
        "Edge value {} should be smoothed down from 50.0",
        smoothed[0][1]
    );
    assert!(
        smoothed[0][1] > 10.0,
        "Edge value {} should still be above neighbor values 10.0",
        smoothed[0][1]
    );
}

#[test]
fn test_smooth_table_zero_factor() {
    // factor=0 should return unchanged values
    let z_values = vec![
        vec![10.0, 10.0, 10.0],
        vec![10.0, 50.0, 10.0],
        vec![10.0, 10.0, 10.0],
    ];

    let selected_cells = vec![(1, 1)];
    let smoothed = smooth_table(&z_values, selected_cells, 0.0);

    assert!(
        (smoothed[1][1] - 50.0).abs() < 0.001,
        "With factor=0, value should be unchanged. Got {}",
        smoothed[1][1]
    );
}

#[test]
fn test_smooth_table_high_factor() {
    // Higher factor = more aggressive smoothing (neighbors weighted more equally)
    let z_values = vec![
        vec![10.0, 10.0, 10.0],
        vec![10.0, 50.0, 10.0],
        vec![10.0, 10.0, 10.0],
    ];

    let selected_cells = vec![(1, 1)];
    let smoothed_low = smooth_table(&z_values, selected_cells.clone(), 0.5);
    let smoothed_high = smooth_table(&z_values, selected_cells, 2.0);

    // Higher factor should result in more smoothing (closer to neighbor average)
    // With 8 neighbors at 10.0 and center at 50.0:
    // - Low factor: center weighted heavily, result closer to 50
    // - High factor: neighbors weighted more, result closer to 10
    assert!(
        smoothed_high[1][1] < smoothed_low[1][1],
        "Higher factor {} should smooth more than lower factor {}",
        smoothed_high[1][1],
        smoothed_low[1][1]
    );
}

#[test]
fn test_scale_cells() {
    let z_values = vec![vec![10.0, 20.0, 30.0], vec![40.0, 50.0, 60.0]];

    let selected_cells = vec![(0, 0), (0, 1)]; // First two cells of first row (y, x)
    let scaled = scale_cells(&z_values, selected_cells, 2.0);

    assert!((scaled[0][0] - 20.0).abs() < 0.01);
    assert!((scaled[0][1] - 40.0).abs() < 0.01);
    assert!((scaled[0][2] - 30.0).abs() < 0.01); // Unselected, unchanged
}

#[test]
fn test_set_cells_equal() {
    let mut z_values = vec![vec![10.0, 20.0, 30.0], vec![40.0, 50.0, 60.0]];

    let selected_cells = vec![(0, 0), (0, 1), (0, 2)]; // First row (y, x)
    set_cells_equal(&mut z_values, selected_cells, 25.0);

    assert!((z_values[0][0] - 25.0).abs() < 0.01);
    assert!((z_values[0][1] - 25.0).abs() < 0.01);
    assert!((z_values[0][2] - 25.0).abs() < 0.01);
    // Second row unchanged
    assert!((z_values[1][0] - 40.0).abs() < 0.01);
}

#[test]
fn test_interpolate_cells_2d() {
    let z_values = vec![
        vec![10.0, 0.0, 40.0],
        vec![0.0, 0.0, 0.0],
        vec![20.0, 0.0, 80.0],
    ];

    // Select all cells in the 3x3 grid (need at least 4 for corners)
    let selected_cells = vec![
        (0, 0),
        (0, 1),
        (0, 2),
        (1, 0),
        (1, 1),
        (1, 2),
        (2, 0),
        (2, 1),
        (2, 2),
    ];
    let result = interpolate_cells(&z_values, selected_cells);

    // Corners should stay the same
    assert!((result[0][0] - 10.0).abs() < 0.01);
    assert!((result[0][2] - 40.0).abs() < 0.01);
    assert!((result[2][0] - 20.0).abs() < 0.01);
    assert!((result[2][2] - 80.0).abs() < 0.01);

    // Center should be interpolated (bilinear interpolation of corners)
    // Expected: (10 + 40 + 20 + 80) / 4 = 37.5 if uniform, but bilinear will be different
    assert!(
        result[1][1] > 10.0 && result[1][1] < 80.0,
        "Center should be between corner values"
    );
    // Bilinear centre of the four corners is exactly their mean here.
    assert!((result[1][1] - 37.5).abs() < 0.01);
}

/// Regression: a single-row selection has a zero-height bounding box. The old
/// implementation computed `0.0 / 0.0 = NaN` and poisoned the whole row; it
/// must instead degrade to a clean horizontal linear interpolation.
#[test]
fn test_interpolate_cells_single_row_is_linear_not_nan() {
    let z_values = vec![vec![10.0, 0.0, 0.0, 40.0]];
    let selected_cells = vec![(0, 0), (0, 1), (0, 2), (0, 3)];

    let result = interpolate_cells(&z_values, selected_cells);

    assert!(
        result[0].iter().all(|v| v.is_finite()),
        "no NaN/Inf allowed"
    );
    assert!((result[0][0] - 10.0).abs() < 1e-9);
    assert!((result[0][1] - 20.0).abs() < 1e-9);
    assert!((result[0][2] - 30.0).abs() < 1e-9);
    assert!((result[0][3] - 40.0).abs() < 1e-9);
}

/// Regression: same defect for a single-column (zero-width) selection.
#[test]
fn test_interpolate_cells_single_col_is_linear_not_nan() {
    let z_values = vec![vec![10.0], vec![0.0], vec![0.0], vec![40.0]];
    let selected_cells = vec![(0, 0), (1, 0), (2, 0), (3, 0)];

    let result = interpolate_cells(&z_values, selected_cells);

    let col: Vec<f64> = result.iter().map(|r| r[0]).collect();
    assert!(col.iter().all(|v| v.is_finite()), "no NaN/Inf allowed");
    assert!((col[0] - 10.0).abs() < 1e-9);
    assert!((col[1] - 20.0).abs() < 1e-9);
    assert!((col[2] - 30.0).abs() < 1e-9);
    assert!((col[3] - 40.0).abs() < 1e-9);
}

/// A selection whose corners fall outside the current table (e.g. left over
/// after a rebin shrank the grid) must be a no-op, never a panic or a write.
#[test]
fn test_interpolate_cells_out_of_bounds_selection_is_noop() {
    let z_values = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
    let selected_cells = vec![(0, 0), (0, 5), (9, 0), (9, 5)];

    let result = interpolate_cells(&z_values, selected_cells);

    assert_eq!(result, z_values, "out-of-bounds selection must not mutate");
}

/// A selection left over from before "Set Table Size" shrank the grid must be
/// a no-op in `smooth_table`, not an out-of-bounds write.
///
/// `handleSetTableSize` in `TableEditor2D.tsx` resizes the table without
/// clearing `selectionRange`, so the next "Smooth" sends coordinates that no
/// longer exist. A cell exactly one row/col past the edge still has an
/// in-bounds `(-1,-1)` neighbour, so `weight_sum > 0.0` and the unchecked
/// `result[y][x]` write panicked.
#[test]
fn test_smooth_table_out_of_bounds_selection_is_noop() {
    let z_values = vec![vec![10.0, 20.0, 30.0]; 3];

    // (3, 3) is one past the last row and column of a 3x3 grid.
    let result = smooth_table(&z_values, vec![(3, 3)], 1.0);

    assert_eq!(result, z_values, "out-of-bounds selection must not mutate");
}

/// Same guard, but with a partially-stale multi-cell selection: one valid cell
/// and one past the edge. The whole operation is rejected rather than half
/// applied, matching `interpolate_linear`/`fill_region`.
#[test]
fn test_smooth_table_partially_stale_selection_is_noop() {
    let z_values = vec![vec![10.0, 90.0], vec![10.0, 10.0]];

    let result = smooth_table(&z_values, vec![(0, 1), (2, 2)], 1.0);

    assert_eq!(result, z_values, "stale selection must not mutate");
}
