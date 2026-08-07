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

    pub fn clamp_to_budget(&mut self) {
        while self.active_cols * self.active_rows > self.max_elements
            && self.active_cols > self.min_cols
        {
            self.active_cols -= 1;
        }
        while self.active_cols * self.active_rows > self.max_elements
            && self.active_rows > self.min_rows
        {
            self.active_rows -= 1;
        }
    }
}

/// Resolve one resizable-table axis count.
///
/// Values below INI `min` (including 0 from never-migrated firmware/MSQ) use
/// `defaultValue`, matching TunerStudio — not a clamp-to-min that yields 1×1.
pub fn resolve_axis_count(raw: Option<f64>, min: usize, max: usize, default: Option<f64>) -> usize {
    let fallback = default
        .map(|v| v.round() as usize)
        .unwrap_or(min)
        .clamp(min, max);
    match raw {
        Some(v) => {
            let n = v.round() as i64;
            if n < min as i64 {
                fallback
            } else {
                (n as usize).clamp(min, max)
            }
        }
        None => fallback,
    }
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

    let mut info = TableSizeInfo {
        active_cols: resolve_axis_count(
            get_value(&cols_const),
            min_cols,
            max_cols,
            def.default_values.get(&cols_const).copied(),
        ),
        active_rows: resolve_axis_count(
            get_value(&rows_const),
            min_rows,
            max_rows,
            def.default_values.get(&rows_const).copied(),
        ),
        cols_const,
        rows_const,
        min_cols,
        max_cols,
        min_rows,
        max_rows,
        max_elements,
    };
    info.clamp_to_budget();
    Some(info)
}

/// Names of scalars used as dynamic table row/col counts.
pub fn size_const_names(def: &EcuDefinition) -> Vec<String> {
    let mut names = Vec::new();
    for c in def.constants.values() {
        let Some(d) = c.dynamic_size.as_ref() else {
            continue;
        };
        if let Some(cols) = &d.cols_const {
            names.push(cols.clone());
        }
        names.push(d.rows_const.clone());
    }
    names.sort();
    names.dedup();
    names
}

/// Write INI `defaultValue` into page bytes when a size scalar is out of range.
/// Returns how many scalars were repaired (so callers can mark the tune dirty).
pub fn patch_invalid_size_scalars(
    def: &EcuDefinition,
    pages: &mut std::collections::HashMap<u8, Vec<u8>>,
) -> usize {
    let mut patched = 0;
    for name in size_const_names(def) {
        let Some(c) = def.constants.get(&name) else {
            continue;
        };
        if c.data_type.size_bytes() != 1 {
            continue;
        }
        let Some(page) = pages.get_mut(&c.page) else {
            continue;
        };
        let off = c.offset as usize;
        if off >= page.len() {
            continue;
        }
        let min = c.min.round().max(1.0) as u8;
        let max = c.max.round().max(min as f64) as u8;
        let default = def
            .default_values
            .get(&name)
            .map(|v| v.round() as u8)
            .unwrap_or(min)
            .clamp(min, max);
        let cur = page[off];
        if cur < min || cur > max {
            page[off] = default;
            patched += 1;
        }
    }
    patched
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

    #[test]
    fn resolve_axis_uses_default_when_below_min() {
        // Uninitialized ECU/MSQ bytes (0 or 1) must not become a 1×1 table.
        assert_eq!(resolve_axis_count(Some(0.0), 8, 64, Some(32.0)), 32);
        assert_eq!(resolve_axis_count(Some(1.0), 8, 64, Some(32.0)), 32);
        assert_eq!(resolve_axis_count(None, 8, 64, Some(32.0)), 32);
        assert_eq!(resolve_axis_count(Some(24.0), 8, 64, Some(32.0)), 24);
        assert_eq!(resolve_axis_count(Some(100.0), 8, 64, Some(32.0)), 64);
    }
}
