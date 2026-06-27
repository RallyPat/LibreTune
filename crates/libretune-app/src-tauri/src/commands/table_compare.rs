//! Table comparison Tauri command.

use libretune_core::ini::EcuDefinition;
use libretune_core::tune::TuneCache;
use serde::Serialize;

use crate::read_raw_value;
use crate::state::AppState;

/// Single cell difference between two tables.
#[derive(Serialize)]
pub struct TableCellDiff {
    pub row: usize,
    pub col: usize,
    pub value_a: f64,
    pub value_b: f64,
    pub diff: f64,
    pub percent_diff: f64,
}

/// Table comparison result showing differences between two tables.
#[derive(Serialize)]
pub struct TableComparisonResult {
    pub table_a: String,
    pub table_b: String,
    pub rows: usize,
    pub cols: usize,
    pub differences: Vec<TableCellDiff>,
    pub diff_count: usize,
    pub max_diff: f64,
    pub avg_diff: f64,
}

/// Compares two tables cell-by-cell to show differences.
#[tauri::command]
pub async fn compare_tables(
    state: tauri::State<'_, AppState>,
    table_a: String,
    table_b: String,
) -> Result<TableComparisonResult, String> {
    let def_guard = state.definition.read().await;
    let cache_guard = state.tune_cache.lock().await;

    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let cache = cache_guard.as_ref().ok_or("Tune cache not loaded")?;

    let table_def_a = def
        .get_table_by_name_or_map(&table_a)
        .ok_or_else(|| format!("Table '{}' not found", table_a))?;
    let table_def_b = def
        .get_table_by_name_or_map(&table_b)
        .ok_or_else(|| format!("Table '{}' not found", table_b))?;

    let (rows_a, cols_a) = (table_def_a.y_size, table_def_a.x_size);
    let (rows_b, cols_b) = (table_def_b.y_size, table_def_b.x_size);

    if rows_a != rows_b || cols_a != cols_b {
        return Err(format!(
            "Table dimensions don't match: {}x{} vs {}x{}",
            rows_a, cols_a, rows_b, cols_b
        ));
    }

    let rows = rows_a;
    let cols = cols_a;

    let values_a = read_table_values(cache, def, table_def_a, rows, cols)?;
    let values_b = read_table_values(cache, def, table_def_b, rows, cols)?;

    let mut differences = Vec::new();
    let mut max_diff: f64 = 0.0;
    let mut total_diff: f64 = 0.0;

    for row in 0..rows {
        for col in 0..cols {
            let idx = row * cols + col;
            let val_a = values_a[idx];
            let val_b = values_b[idx];
            let diff = val_b - val_a;

            if diff.abs() > 0.0001 {
                let percent_diff = if val_a.abs() > 0.0001 {
                    (diff / val_a) * 100.0
                } else if diff.abs() > 0.0001 {
                    100.0
                } else {
                    0.0
                };

                differences.push(TableCellDiff {
                    row,
                    col,
                    value_a: val_a,
                    value_b: val_b,
                    diff,
                    percent_diff,
                });

                max_diff = max_diff.max(diff.abs());
                total_diff += diff.abs();
            }
        }
    }

    let diff_count = differences.len();
    let avg_diff = if diff_count > 0 {
        total_diff / diff_count as f64
    } else {
        0.0
    };

    Ok(TableComparisonResult {
        table_a,
        table_b,
        rows,
        cols,
        differences,
        diff_count,
        max_diff,
        avg_diff,
    })
}

/// Helper to read all values from a table into a flat vector.
fn read_table_values(
    cache: &TuneCache,
    def: &EcuDefinition,
    table_def: &libretune_core::ini::TableDefinition,
    rows: usize,
    cols: usize,
) -> Result<Vec<f64>, String> {
    let mut values = Vec::with_capacity(rows * cols);

    let z_const = def
        .constants
        .get(&table_def.map)
        .ok_or_else(|| format!("Table map constant '{}' not found", table_def.map))?;

    let page_data = cache
        .get_page(z_const.page)
        .ok_or(format!("Page {} not loaded", z_const.page))?;

    let elem_size = z_const.data_type.size_bytes();
    let mut offset = z_const.offset as usize;

    for _row in 0..rows {
        for _col in 0..cols {
            if offset + elem_size > page_data.len() {
                return Err("Table data exceeds page bounds".to_string());
            }

            let raw_value = read_raw_value(&page_data[offset..], &z_const.data_type)?;
            let display_value = z_const.raw_to_display(raw_value);
            values.push(display_value);

            offset += elem_size;
        }
    }

    Ok(values)
}
