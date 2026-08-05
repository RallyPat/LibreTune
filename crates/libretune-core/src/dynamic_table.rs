//! TunerStudio-style dynamically sized (resizable) tables.
//!
//! INI arrays may be declared as `[{cols}x{rows}]` / `[{count}]`. The allocated
//! footprint is fixed (see `maximumElements` / axis scalar max); the active size
//! lives in tune scalars and packs values row-major with stride = current cols.

use crate::ini::{Constant, EcuDefinition, Shape, TableDefinition};

/// Limits and active size for a resizable table.
#[derive(Debug, Clone)]
pub struct TableSizeInfo {
    pub cols_const: String,
    pub rows_const: String,
    pub min_cols: usize,
    pub max_cols: usize,
    pub min_rows: usize,
    pub max_rows: usize,
    pub max_elements: usize,
    pub active_cols: usize,
    pub active_rows: usize,
}

impl TableSizeInfo {
    pub fn is_resizable(&self) -> bool {
        true
    }

    pub fn allows(&self, cols: usize, rows: usize) -> Result<(), String> {
        if cols < self.min_cols || cols > self.max_cols {
            return Err(format!(
                "Columns {} out of range {}..{}",
                cols, self.min_cols, self.max_cols
            ));
        }
        if rows < self.min_rows || rows > self.max_rows {
            return Err(format!(
                "Rows {} out of range {}..{}",
                rows, self.min_rows, self.max_rows
            ));
        }
        if cols.saturating_mul(rows) > self.max_elements {
            return Err(format!(
                "{}x{} exceeds cell budget {}",
                rows, cols, self.max_elements
            ));
        }
        Ok(())
    }
}

/// Read a scalar constant as usize (tune page data / named value / default).
pub fn read_size_scalar(
    def: &EcuDefinition,
    name: &str,
    get_value: &dyn Fn(&str) -> Option<f64>,
) -> usize {
    if let Some(v) = get_value(name) {
        return v.round().max(0.0) as usize;
    }
    if let Some(v) = def.default_values.get(name) {
        return v.round().max(0.0) as usize;
    }
    0
}

/// Size metadata for a table whose Z map uses dynamic `{const}` dimensions.
pub fn table_size_info(
    def: &EcuDefinition,
    table: &TableDefinition,
    get_value: &dyn Fn(&str) -> Option<f64>,
) -> Option<TableSizeInfo> {
    let map = def.constants.get(&table.map)?;
    let dyn_refs = map.dynamic_size.as_ref()?;
    let cols_const = dyn_refs.cols_const.clone()?;
    let rows_const = dyn_refs.rows_const.clone();

    let cols_c = def.constants.get(&cols_const)?;
    let rows_c = def.constants.get(&rows_const)?;

    let min_cols = cols_c.min.round().max(1.0) as usize;
    let max_cols = cols_c.max.round().max(1.0) as usize;
    let min_rows = rows_c.min.round().max(1.0) as usize;
    let max_rows = rows_c.max.round().max(1.0) as usize;
    let max_elements = def
        .maximum_elements
        .get(&table.map)
        .copied()
        .unwrap_or_else(|| map.shape.element_count())
        .max(1);

    let mut active_cols = read_size_scalar(def, &cols_const, get_value);
    let mut active_rows = read_size_scalar(def, &rows_const, get_value);
    if active_cols == 0 {
        active_cols = def
            .default_values
            .get(&cols_const)
            .map(|v| v.round() as usize)
            .unwrap_or(min_cols)
            .max(min_cols);
    }
    if active_rows == 0 {
        active_rows = def
            .default_values
            .get(&rows_const)
            .map(|v| v.round() as usize)
            .unwrap_or(min_rows)
            .max(min_rows);
    }

    // Clamp like firmware helpers — never exceed axis max or cell budget.
    active_cols = active_cols.clamp(min_cols, max_cols);
    active_rows = active_rows.clamp(min_rows, max_rows);
    while active_cols * active_rows > max_elements && active_cols > min_cols {
        active_cols -= 1;
    }
    while active_cols * active_rows > max_elements && active_rows > min_rows {
        active_rows -= 1;
    }

    Some(TableSizeInfo {
        cols_const,
        rows_const,
        min_cols,
        max_cols,
        min_rows,
        max_rows,
        max_elements,
        active_cols,
        active_rows,
    })
}

/// All table names that share the same row/col count scalars (must resize together).
pub fn tables_sharing_size_consts(
    def: &EcuDefinition,
    cols_const: &str,
    rows_const: &str,
) -> Vec<String> {
    def.tables
        .iter()
        .filter_map(|(name, table)| {
            let map = def.constants.get(&table.map)?;
            let d = map.dynamic_size.as_ref()?;
            let cols = d.cols_const.as_deref()?;
            if cols == cols_const && d.rows_const == rows_const {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect()
}

/// Slice a full allocated axis to the active length.
pub fn slice_bins(bins: &[f64], active: usize) -> Vec<f64> {
    if bins.is_empty() {
        return vec![0.0; active.max(1)];
    }
    if bins.len() >= active {
        return bins[..active].to_vec();
    }
    let mut out = bins.to_vec();
    let last = *bins.last().unwrap_or(&0.0);
    while out.len() < active {
        out.push(last);
    }
    out
}

/// Reshape a flat Z buffer using the *active* column stride (TS packing).
pub fn unpack_z(flat: &[f64], cols: usize, rows: usize) -> Vec<Vec<f64>> {
    let mut z = Vec::with_capacity(rows);
    for y in 0..rows {
        let mut row = Vec::with_capacity(cols);
        for x in 0..cols {
            row.push(*flat.get(y * cols + x).unwrap_or(&0.0));
        }
        z.push(row);
    }
    z
}

/// Pack active Z into an allocated flat buffer (preserves unused tail).
pub fn pack_z_into(allocated: &mut [f64], z: &[Vec<f64>]) {
    let cols = z.first().map(|r| r.len()).unwrap_or(0);
    for (y, row) in z.iter().enumerate() {
        for (x, &val) in row.iter().enumerate().take(cols) {
            let idx = y * cols + x;
            if let Some(slot) = allocated.get_mut(idx) {
                *slot = val;
            }
        }
    }
}

/// Linearly spaced bins from first..last over `count` points.
pub fn linspace_bins(first: f64, last: f64, count: usize) -> Vec<f64> {
    if count == 0 {
        return Vec::new();
    }
    if count == 1 {
        return vec![first];
    }
    let step = (last - first) / (count as f64 - 1.0);
    (0..count).map(|i| first + step * i as f64).collect()
}

/// Whether a constant is a dynamically sized array.
pub fn is_dynamic_array(constant: &Constant) -> bool {
    constant.dynamic_size.is_some()
}

/// Allocated element count for reads/writes (full ECU footprint).
pub fn allocated_elements(constant: &Constant) -> usize {
    match &constant.shape {
        Shape::Scalar => 1,
        Shape::Array1D(n) => (*n).max(1),
        Shape::Array2D { rows, cols } => rows.saturating_mul(*cols).max(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_unpack_roundtrip_active_region() {
        let z = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let mut flat = vec![9.0; 16];
        pack_z_into(&mut flat, &z);
        assert_eq!(&flat[..6], &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(flat[6], 9.0);
        let back = unpack_z(&flat, 3, 2);
        assert_eq!(back, z);
    }

    #[test]
    fn linspace_bins_endpoints() {
        let bins = linspace_bins(0.0, 100.0, 5);
        assert_eq!(bins, vec![0.0, 25.0, 50.0, 75.0, 100.0]);
    }
}
