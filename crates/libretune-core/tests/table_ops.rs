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
}
