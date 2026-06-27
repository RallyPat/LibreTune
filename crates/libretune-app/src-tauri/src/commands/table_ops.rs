//! Table editing operations (rebin, interpolate, scale, smooth, set-equal, fill, offset).

use crate::state::AppState;
use crate::{
    get_table_data_internal, update_constant_array_internal, update_table_z_values_internal,
    TableData,
};
use libretune_core::table_ops;

/// Re-bins a table with new X and Y axis values.
///
/// Optionally interpolates Z values to fit the new axis bins.
///
/// # Arguments
/// * `table_name` - Table name from INI definition
/// * `new_x_bins` - New X axis bin values
/// * `new_y_bins` - New Y axis bin values
/// * `interpolate_z` - If true, interpolates Z values to fit new bins
///
/// Returns: Updated TableData with new bins and Z values
#[tauri::command]
pub async fn rebin_table(
    state: tauri::State<'_, AppState>,
    table_name: String,
    new_x_bins: Vec<f64>,
    new_y_bins: Vec<f64>,
    interpolate_z: bool,
) -> Result<TableData, String> {
    // Get current table data
    let table_data = get_table_data_internal(&state, &table_name).await?;

    // Apply rebin operation
    let result = table_ops::rebin_table(
        &table_data.x_bins,
        &table_data.y_bins,
        &table_data.z_values,
        new_x_bins.clone(),
        new_y_bins.clone(),
        interpolate_z,
    );

    // Save the new Z values
    update_table_z_values_internal(&state, &table_name, result.z_values.clone()).await?;

    // Save the new X/Y axis bins
    {
        let def_guard = state.definition.read().await;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;
        let table = def
            .get_table_by_name_or_map(&table_name)
            .ok_or_else(|| format!("Table {} not found", table_name))?;

        let x_bins_name = table.x_bins.clone();
        let y_bins_name = table.y_bins.clone();
        drop(def_guard);

        update_constant_array_internal(&state, &x_bins_name, result.x_bins.clone()).await?;
        if let Some(y_name) = y_bins_name {
            update_constant_array_internal(&state, &y_name, result.y_bins.clone()).await?;
        }
    }

    Ok(TableData {
        x_bins: result.x_bins,
        y_bins: result.y_bins,
        z_values: result.z_values,
        ..table_data
    })
}

#[tauri::command]
pub async fn interpolate_linear(
    state: tauri::State<'_, AppState>,
    table_name: String,
    selected_cells: Vec<(usize, usize)>,
    axis: String,
) -> Result<TableData, String> {
    let axis_enum = match axis.to_lowercase().as_str() {
        "row" => table_ops::InterpolationAxis::Row,
        "col" => table_ops::InterpolationAxis::Col,
        _ => return Err("Invalid interpolation axis".to_string()),
    };

    let table_data = get_table_data_internal(&state, &table_name).await?;
    let new_z_values =
        table_ops::interpolate_linear(&table_data.z_values, selected_cells, axis_enum);

    update_table_z_values_internal(&state, &table_name, new_z_values.clone()).await?;

    Ok(TableData {
        z_values: new_z_values,
        ..table_data
    })
}

#[tauri::command]
pub async fn add_offset(
    state: tauri::State<'_, AppState>,
    table_name: String,
    selected_cells: Vec<(usize, usize)>,
    offset: f64,
) -> Result<TableData, String> {
    let table_data = get_table_data_internal(&state, &table_name).await?;
    let new_z_values = table_ops::add_offset(&table_data.z_values, selected_cells, offset);

    update_table_z_values_internal(&state, &table_name, new_z_values.clone()).await?;

    Ok(TableData {
        z_values: new_z_values,
        ..table_data
    })
}

#[tauri::command]
pub async fn fill_region(
    state: tauri::State<'_, AppState>,
    table_name: String,
    selected_cells: Vec<(usize, usize)>,
    direction: String,
) -> Result<TableData, String> {
    let dir_enum = match direction.to_lowercase().as_str() {
        "right" => table_ops::FillDirection::Right,
        "down" => table_ops::FillDirection::Down,
        _ => return Err("Invalid fill direction".to_string()),
    };

    let table_data = get_table_data_internal(&state, &table_name).await?;
    let new_z_values = table_ops::fill_region(&table_data.z_values, selected_cells, dir_enum);

    update_table_z_values_internal(&state, &table_name, new_z_values.clone()).await?;

    Ok(TableData {
        z_values: new_z_values,
        ..table_data
    })
}

/// Applies Gaussian smoothing to selected table cells.
///
/// Uses weighted averaging from neighboring cells to smooth transitions.
///
/// # Arguments
/// * `table_name` - Table name from INI definition
/// * `factor` - Smoothing factor (higher = more smoothing)
/// * `selected_cells` - Vector of (row, col) coordinates to smooth
///
/// Returns: Updated TableData with smoothed values
#[tauri::command]
pub async fn smooth_table(
    state: tauri::State<'_, AppState>,
    table_name: String,
    factor: f64,
    selected_cells: Vec<(usize, usize)>,
) -> Result<TableData, String> {
    // Get current table data
    let table_data = get_table_data_internal(&state, &table_name).await?;

    // Apply smooth operation (cells are already in (row, col) format from frontend)
    let new_z_values = table_ops::smooth_table(&table_data.z_values, selected_cells, factor);

    // Save the modified values
    update_table_z_values_internal(&state, &table_name, new_z_values.clone()).await?;

    Ok(TableData {
        z_values: new_z_values,
        ..table_data
    })
}

/// Interpolates values between corner cells of selected region.
///
/// Uses bilinear interpolation to fill in values between the
/// corner cells of the selection rectangle.
///
/// # Arguments
/// * `table_name` - Table name from INI definition
/// * `selected_cells` - Vector of (row, col) coordinates to interpolate
///
/// Returns: Updated TableData with interpolated values
#[tauri::command]
pub async fn interpolate_cells(
    state: tauri::State<'_, AppState>,
    table_name: String,
    selected_cells: Vec<(usize, usize)>,
) -> Result<TableData, String> {
    // Get current table data
    let table_data = get_table_data_internal(&state, &table_name).await?;

    // Apply interpolate operation
    let new_z_values = table_ops::interpolate_cells(&table_data.z_values, selected_cells);

    // Save the modified values
    update_table_z_values_internal(&state, &table_name, new_z_values.clone()).await?;

    Ok(TableData {
        z_values: new_z_values,
        ..table_data
    })
}

/// Scales selected cells by a multiplication factor.
///
/// # Arguments
/// * `table_name` - Table name from INI definition
/// * `selected_cells` - Vector of (row, col) coordinates to scale
/// * `scale_factor` - Multiplication factor (e.g., 1.1 for +10%)
///
/// Returns: Updated TableData with scaled values
#[tauri::command]
pub async fn scale_cells(
    state: tauri::State<'_, AppState>,
    table_name: String,
    selected_cells: Vec<(usize, usize)>,
    scale_factor: f64,
) -> Result<TableData, String> {
    // Get current table data
    let table_data = get_table_data_internal(&state, &table_name).await?;

    // Apply scale operation
    let new_z_values = table_ops::scale_cells(&table_data.z_values, selected_cells, scale_factor);

    // Save the modified values
    update_table_z_values_internal(&state, &table_name, new_z_values.clone()).await?;

    Ok(TableData {
        z_values: new_z_values,
        ..table_data
    })
}

/// Sets all selected cells to the same value.
///
/// # Arguments
/// * `table_name` - Table name from INI definition
/// * `selected_cells` - Vector of (row, col) coordinates to set
/// * `value` - Value to assign to all selected cells
///
/// Returns: Updated TableData with modified values
#[tauri::command]
pub async fn set_cells_equal(
    state: tauri::State<'_, AppState>,
    table_name: String,
    selected_cells: Vec<(usize, usize)>,
    value: f64,
) -> Result<TableData, String> {
    // Get current table data
    let table_data = get_table_data_internal(&state, &table_name).await?;

    // Apply set equal operation (mutates in place)
    let mut new_z_values = table_data.z_values.clone();
    table_ops::set_cells_equal(&mut new_z_values, selected_cells, value);

    // Save the modified values
    update_table_z_values_internal(&state, &table_name, new_z_values.clone()).await?;

    Ok(TableData {
        z_values: new_z_values,
        ..table_data
    })
}
