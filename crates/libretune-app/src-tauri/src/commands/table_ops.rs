//! Table editing operations (rebin, interpolate, scale, smooth, set-equal, fill, offset).

use crate::commands::constant_update::update_constant;
use crate::state::AppState;
use crate::{
    get_table_data_internal, update_constant_array_internal, update_table_z_values_internal,
    TableData,
};
use libretune_core::dynamic_table;
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
        let def_guard = state.definition.lock().await;
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

/// Resize a TunerStudio dynamically sized table (and all tables sharing its
/// row/col count scalars). Works offline (tune RAM/file only); burns when connected.
/// Blocked when a connected ECU signature fully mismatches the loaded INI.
#[tauri::command]
pub async fn resize_table_size(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    table_name: String,
    new_cols: usize,
    new_rows: usize,
) -> Result<TableData, String> {
    crate::commands::signature_helpers::assert_resize_allowed(&state).await?;

    let (cols_const, rows_const, shared_tables, x_first, y_first) = {
        let def_guard = state.definition.lock().await;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;
        let table = def
            .get_table_by_name_or_map(&table_name)
            .ok_or_else(|| format!("Table {} not found", table_name))?;
        let info = dynamic_table::table_size_info(def, table, &|_| None)
            .ok_or("This table is not resizable in the INI")?;
        info.allows(new_cols, new_rows)?;
        let shared =
            dynamic_table::tables_sharing_size_consts(def, &info.cols_const, &info.rows_const);
        let x_bins = table.x_bins.clone();
        let y_bins = table.y_bins.clone();
        (info.cols_const, info.rows_const, shared, x_bins, y_bins)
    };

    // Primary table: build new linearly spaced axes from current endpoints, interpolate Z.
    let primary = get_table_data_internal(&state, &table_name).await?;
    let x0 = *primary.x_bins.first().unwrap_or(&0.0);
    let x1 = *primary.x_bins.last().unwrap_or(&8000.0);
    let y0 = *primary.y_bins.first().unwrap_or(&0.0);
    let y1 = *primary.y_bins.last().unwrap_or(&100.0);
    let new_x = dynamic_table::linspace_bins(x0, x1, new_cols);
    let new_y = dynamic_table::linspace_bins(y0, y1, new_rows);

    let mut tables_to_resize = shared_tables;
    if tables_to_resize.is_empty() {
        tables_to_resize.push(primary.name.clone());
    }

    // Shared axes: write bins once from the primary table's endpoints.
    let mut wrote_bins = false;
    for name in &tables_to_resize {
        let current = get_table_data_internal(&state, name).await?;
        let result = table_ops::rebin_table(
            &current.x_bins,
            &current.y_bins,
            &current.z_values,
            new_x.clone(),
            new_y.clone(),
            true,
        );
        update_table_z_values_internal(&state, name, result.z_values).await?;

        if !wrote_bins {
            let (x_name, y_name) = {
                let def_guard = state.definition.lock().await;
                let def = def_guard.as_ref().ok_or("Definition not loaded")?;
                let t = def
                    .get_table_by_name_or_map(name)
                    .ok_or_else(|| format!("Table {} not found", name))?;
                (t.x_bins.clone(), t.y_bins.clone())
            };
            // Prefer primary axis names when available.
            let x_name = if x_first.is_empty() {
                x_name
            } else {
                x_first.clone()
            };
            let y_name = y_first.clone().or(y_name);
            update_constant_array_internal(&state, &x_name, result.x_bins).await?;
            if let Some(y) = y_name {
                update_constant_array_internal(&state, &y, result.y_bins).await?;
            }
            wrote_bins = true;
        }
    }

    update_constant(state.clone(), cols_const, new_cols as f64).await?;
    update_constant(state.clone(), rows_const, new_rows as f64).await?;

    let connected = state.connection.lock().await.is_some();
    if connected {
        crate::commands::tune_io::burn_to_ecu(app, state.clone(), None)
            .await
            .map_err(|e| format!("Resized in RAM but burn failed: {}", e))?;
    }

    get_table_data_internal(&state, &table_name).await
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
