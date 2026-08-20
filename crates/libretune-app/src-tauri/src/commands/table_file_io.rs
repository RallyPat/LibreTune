//! Per-table import/export to TunerStudio's `.table` file format.
//!
//! Distinct from csv_io.rs, which dumps/restores the *entire* tune as a flat
//! CSV — this is the TunerStudio-compatible "Save Table to File" / "Load
//! Table from File" a user reaches from a single table's toolbar.

use crate::state::AppState;
use crate::{
    get_table_data_internal, update_constant_array_internal, update_table_z_values_internal,
};
use libretune_core::table_file::{parse_table_file, write_table_file};

/// Export one table's current X/Y bins and Z-value grid to a `.table` file.
#[tauri::command]
pub async fn export_table_to_file(
    state: tauri::State<'_, AppState>,
    table_name: String,
    path: String,
) -> Result<(), String> {
    let data = get_table_data_internal(&state, &table_name).await?;

    let (x_bins_name, y_bins_name) = {
        let def_guard = state.definition.lock().await;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;
        let table = def
            .get_table_by_name_or_map(&table_name)
            .ok_or_else(|| format!("Table {} not found", table_name))?;
        (
            table.x_bins.clone(),
            table.y_bins.clone().unwrap_or_default(),
        )
    };

    let xml = write_table_file(
        &x_bins_name,
        &y_bins_name,
        &data.x_bins,
        &data.y_bins,
        &data.z_values,
    )
    .map_err(|e| e.to_string())?;

    std::fs::write(&path, xml).map_err(|e| format!("Failed to write {}: {}", path, e))
}

/// Import a `.table` file into the named table. The file's dimensions must
/// match the table's *current* size exactly — unlike TunerStudio, this does
/// not resample/interpolate a mismatched grid onto the table's axes, so a
/// resized table's file won't silently apply at the wrong scale.
#[tauri::command]
pub async fn import_table_from_file(
    state: tauri::State<'_, AppState>,
    table_name: String,
    path: String,
) -> Result<crate::TableData, String> {
    let xml =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read {}: {}", path, e))?;
    let parsed = parse_table_file(&xml).map_err(|e| e.to_string())?;

    let current = get_table_data_internal(&state, &table_name).await?;
    if parsed.cols != current.x_bins.len() || parsed.rows != current.y_bins.len() {
        return Err(format!(
            "{} is {}x{} in this project, but the file is {}x{}. Resize the table to match before importing.",
            table_name,
            current.x_bins.len(),
            current.y_bins.len(),
            parsed.cols,
            parsed.rows,
        ));
    }

    let (x_bins_name, y_bins_name, is_3d) = {
        let def_guard = state.definition.lock().await;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;
        let table = def
            .get_table_by_name_or_map(&table_name)
            .ok_or_else(|| format!("Table {} not found", table_name))?;
        (table.x_bins.clone(), table.y_bins.clone(), table.is_3d())
    };

    update_constant_array_internal(&state, &x_bins_name, parsed.x_bins).await?;
    if is_3d {
        if let Some(y_bins_name) = y_bins_name {
            update_constant_array_internal(&state, &y_bins_name, parsed.y_bins).await?;
        }
    }
    update_table_z_values_internal(&state, &table_name, parsed.z_values).await?;

    get_table_data_internal(&state, &table_name).await
}
