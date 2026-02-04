# Table Operations

LibreTune provides several mathematical operations for manipulating 2D and 3D table data. These operations are implemented in the core library for consistency and tested for correctness.

## Interpolation

### Bilinear Interpolation (2D Tables)

Used for calculating values between table cells, essential for re-binning and cursor tracking.

**Algorithm**:
Given a 2D table with X axis (columns) and Y axis (rows), to find value at arbitrary (x, y):

```
1. Find surrounding cells:
   x1 < x < x2  (column indices i1, i2)
   y1 < y < y2  (row indices j1, j2)

2. Normalize position within cell:
   tx = (x - x1) / (x2 - x1)
   ty = (y - y1) / (y2 - y1)

3. Interpolate X direction (for each Y):
   v_top = lerp(Z[j1][i1], Z[j1][i2], tx)
   v_bot = lerp(Z[j2][i1], Z[j2][i2], tx)

4. Interpolate Y direction:
   result = lerp(v_top, v_bot, ty)
```

Where `lerp(a, b, t) = a + (b - a) * t`

**Implementation**:
```rust
pub fn interpolate_2d(
    x_bins: &[f64],
    y_bins: &[f64],
    z_values: &[Vec<f64>],
    x: f64,
    y: f64,
) -> Option<f64> {
    // Find X bracket
    let x_idx = x_bins.iter().position(|&bx| bx > x)?;
    if x_idx == 0 { return None; }
    let i1 = x_idx - 1;
    let i2 = x_idx;
    
    // Find Y bracket
    let y_idx = y_bins.iter().position(|&by| by > y)?;
    if y_idx == 0 { return None; }
    let j1 = y_idx - 1;
    let j2 = y_idx;
    
    // Normalize
    let tx = (x - x_bins[i1]) / (x_bins[i2] - x_bins[i1]);
    let ty = (y - y_bins[j1]) / (y_bins[j2] - y_bins[j1]);
    
    // Bilinear
    let v11 = z_values[j1][i1];
    let v12 = z_values[j1][i2];
    let v21 = z_values[j2][i1];
    let v22 = z_values[j2][i2];
    
    let v_top = v11 + (v12 - v11) * tx;
    let v_bot = v21 + (v22 - v21) * tx;
    
    Some(v_top + (v_bot - v_top) * ty)
}
```

**Complexity**: O(log n + log m) if using binary search, O(n + m) with linear search

**Use Cases**:
- Re-binning tables to new axis values
- Cursor tracking in 3D visualization
- Smoothing operation (samples between cells)

### Edge Case Handling

- **Outside Table Bounds**: Returns None (or clamps to nearest edge)
- **Coincident Points**: If x == x_bin, uses exact value (no interpolation)
- **Single Row/Column**: Falls back to 1D linear interpolation

## Smoothing

### Gaussian Weighted Averaging

Smooths table values by averaging with neighbors, weighted by distance.

**Algorithm**:
```
For each selected cell (i, j):
  1. Get neighbors within kernel_size radius
  2. Calculate Gaussian weight for each neighbor:
     distance = sqrt((i - ni)² + (j - nj)²)
     weight = exp(-(distance²) / (2 * sigma²))
  3. Weighted average:
     new_value = Σ(neighbor_value * weight) / Σ(weight)
```

**Parameters**:
- `kernel_size`: Radius of smoothing (1 = 3×3, 2 = 5×5, etc.)
- `sigma`: Gaussian standard deviation (default: kernel_size / 2)

**Implementation**:
```rust
pub fn smooth_table(
    z_values: &[Vec<f64>],
    selection: &[(usize, usize)],
    kernel_size: usize,
) -> Result<Vec<Vec<f64>>, String> {
    let mut result = z_values.to_vec();
    let sigma = (kernel_size as f64) / 2.0;
    let weights = calculate_gaussian_weights(kernel_size, sigma);
    
    for &(row, col) in selection {
        let neighbors = get_neighbors(z_values, row, col, kernel_size);
        let weighted_sum: f64 = neighbors.iter()
            .map(|(val, dist)| val * weights[*dist])
            .sum();
        let weight_sum: f64 = neighbors.iter()
            .map(|(_, dist)| weights[*dist])
            .sum();
        result[row][col] = weighted_sum / weight_sum;
    }
    
    Ok(result)
}

fn calculate_gaussian_weights(kernel_size: usize, sigma: f64) -> Vec<f64> {
    let max_dist = kernel_size as f64 * 1.415; // sqrt(2) * kernel_size
    (0..=(max_dist as usize))
        .map(|d| {
            let dist = d as f64;
            (-dist.powi(2) / (2.0 * sigma.powi(2))).exp()
        })
        .collect()
}
```

**Complexity**: O(k² * n) where k = kernel_size, n = selection size

**Use Cases**:
- Removing noise from logged data
- Blending transitions between tuning regions
- Creating smooth gradients after manual edits

**Visual Example**:
```
Before:          After (kernel_size=1):
80  85  90       80.0  85.0  90.0
85  95  85   →   84.6  88.1  86.4
90  85  80       88.1  85.0  82.5
```

## Scaling

Multiplies selected cells by a constant factor.

**Algorithm**:
```
For each selected cell (i, j):
  new_value = old_value * factor
```

**Implementation**:
```rust
pub fn scale_cells(
    z_values: &[Vec<f64>],
    selection: &[(usize, usize)],
    factor: f64,
) -> Result<Vec<Vec<f64>>, String> {
    let mut result = z_values.to_vec();
    for &(row, col) in selection {
        result[row][col] *= factor;
    }
    Ok(result)
}
```

**Complexity**: O(n) where n = selection size

**Use Cases**:
- Quick percentage adjustments (multiply by 1.05 = +5%)
- Compensating for fuel injector size changes
- Batch adjustments after hardware changes

**Example**:
```
Selection: [(0,0), (0,1), (1,0), (1,1)]
Factor: 1.10 (+10%)

Before:      After:
80  85  90   88   93.5  90
85  90  85   93.5 99    85
90  85  80   90   85    80
```

## Set Equal

Sets all selected cells to the average value of the selection.

**Algorithm**:
```
average = sum(selected_values) / count(selected_values)
For each selected cell (i, j):
  new_value = average
```

**Implementation**:
```rust
pub fn set_cells_equal(
    z_values: &[Vec<f64>],
    selection: &[(usize, usize)],
) -> Result<Vec<Vec<f64>>, String> {
    let sum: f64 = selection.iter()
        .map(|&(row, col)| z_values[row][col])
        .sum();
    let avg = sum / selection.len() as f64;
    
    let mut result = z_values.to_vec();
    for &(row, col) in selection {
        result[row][col] = avg;
    }
    Ok(result)
}
```

**Complexity**: O(n) where n = selection size

**Use Cases**:
- Creating flat regions (e.g., idle areas)
- Setting consistent base values before fine-tuning
- Removing spikes or outliers from a region

**Example**:
```
Selection: [(0,0), (0,1), (1,0)]
Average: (80 + 85 + 85) / 3 = 83.33

Before:      After:
80  85  90   83.33  83.33  90
85  90  85   83.33  90     85
90  85  80   90     85     80
```

## Re-binning

Changes table axis values and interpolates Z values for the new grid.

**Algorithm**:
```
1. For each new axis point (x_new, y_new):
   a. Find position in old axis bins
   b. Use bilinear interpolation to get Z value
   c. Store in new table
```

**Implementation**:
```rust
pub fn rebin_table(
    old_x_bins: &[f64],
    old_y_bins: &[f64],
    old_z_values: &[Vec<f64>],
    new_x_bins: &[f64],
    new_y_bins: &[f64],
) -> Result<Vec<Vec<f64>>, String> {
    let mut new_z_values = vec![vec![0.0; new_x_bins.len()]; new_y_bins.len()];
    
    for (j, &y) in new_y_bins.iter().enumerate() {
        for (i, &x) in new_x_bins.iter().enumerate() {
            new_z_values[j][i] = interpolate_2d(
                old_x_bins,
                old_y_bins,
                old_z_values,
                x,
                y,
            ).unwrap_or_else(|| {
                // Fallback: nearest edge value
                extrapolate_edge(old_x_bins, old_y_bins, old_z_values, x, y)
            });
        }
    }
    
    Ok(new_z_values)
}
```

**Complexity**: O(n * m * log(n_old * m_old))

**Use Cases**:
- Increasing resolution in critical areas (e.g., 12×12 → 16×16)
- Changing RPM range after cam swap
- Consolidating sparse data into denser table

**Example**:
```
Old X bins: [1000, 2000, 3000]
New X bins: [1000, 1500, 2000, 2500, 3000]

Old table:
      1000  2000  3000
2000  80    85    90
3000  85    90    95

New table:
      1000  1500  2000  2500  3000
2000  80    82.5  85    87.5  90
3000  85    87.5  90    92.5  95
```

## Interpolate Cells

Linear interpolation between corner cells of a selection.

**Algorithm**:
```
1. Find min/max row and col in selection
2. For each cell (i, j) in selection:
   tx = (col - col_min) / (col_max - col_min)
   ty = (row - row_min) / (row_max - row_min)
   
   value = bilinear_interpolate(
     Z[row_min][col_min],  // top-left
     Z[row_min][col_max],  // top-right
     Z[row_max][col_min],  // bottom-left
     Z[row_max][col_max],  // bottom-right
     tx, ty
   )
```

**Implementation**:
```rust
pub fn interpolate_cells(
    z_values: &[Vec<f64>],
    selection: &[(usize, usize)],
) -> Result<Vec<Vec<f64>>, String> {
    let mut result = z_values.to_vec();
    
    let rows: Vec<usize> = selection.iter().map(|&(r, _)| r).collect();
    let cols: Vec<usize> = selection.iter().map(|&(_, c)| c).collect();
    
    let r_min = *rows.iter().min().unwrap();
    let r_max = *rows.iter().max().unwrap();
    let c_min = *cols.iter().min().unwrap();
    let c_max = *cols.iter().max().unwrap();
    
    let v11 = z_values[r_min][c_min];
    let v12 = z_values[r_min][c_max];
    let v21 = z_values[r_max][c_min];
    let v22 = z_values[r_max][c_max];
    
    for &(row, col) in selection {
        let ty = if r_max == r_min { 0.0 } else {
            (row - r_min) as f64 / (r_max - r_min) as f64
        };
        let tx = if c_max == c_min { 0.0 } else {
            (col - c_min) as f64 / (c_max - c_min) as f64
        };
        
        let v_top = v11 + (v12 - v11) * tx;
        let v_bot = v21 + (v22 - v21) * tx;
        result[row][col] = v_top + (v_bot - v_top) * ty;
    }
    
    Ok(result)
}
```

**Complexity**: O(n) where n = selection size

**Use Cases**:
- Creating smooth transitions between two known-good values
- Filling in missing data regions
- Building initial base maps

**Example**:
```
Selection: all cells in 3×3 region
Corner values: TL=80, TR=100, BL=70, BR=90

Before:      After:
80  ?   100  80   90   100
?   ?   ?    75   85   95
70  ?   90   70   80   90
```

## Performance Optimization

### Memory Layout

Tables are stored as `Vec<Vec<f64>>` (row-major order) for cache-friendly access:
```
[row0[col0, col1, col2, ...],
 row1[col0, col1, col2, ...],
 ...]
```

### Vectorization

Operations on large tables use iterator chains for potential SIMD optimization:
```rust
let sum: f64 = z_values.iter()
    .flat_map(|row| row.iter())
    .sum();
```

### In-Place Operations

Where possible, operations modify existing arrays instead of allocating new ones:
```rust
// Good (in-place)
result[row][col] *= factor;

// Avoid (allocates)
result = result.iter().map(|v| v * factor).collect();
```

## Error Handling

All table operations return `Result<Vec<Vec<f64>>, String>`:

Common error cases:
- **Out of bounds**: `"Row X out of bounds (table has Y rows)"`
- **Empty selection**: `"No cells selected"`
- **Invalid dimensions**: `"New X bins must match table columns"`
- **Division by zero**: `"Cannot interpolate with coincident axis points"`

## Source Code Reference

- Implementation: `crates/libretune-core/src/table_ops.rs`
- Tests: `crates/libretune-core/tests/table_ops.rs`
- Tauri commands: `crates/libretune-app/src-tauri/src/lib.rs` (smooth_table, rebin_table, etc.)
- UI integration: `crates/libretune-app/src/components/tables/TableEditor2D.tsx`

## See Also

- [2D Table Editing](../features/table-editing/2d-tables.md) - User guide for table operations
- [3D Visualization](../features/table-editing/3d-visualization.md) - Visualizing interpolated surfaces
- [AutoTune Algorithm](./autotune-algorithm.md) - Uses interpolation for data correlation
