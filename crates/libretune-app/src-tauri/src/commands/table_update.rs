//! Table z-values update command.

use crate::commands::tune_persist::{
    ensure_tune_cache, maybe_auto_save_project_tune, write_bytes_to_tune_pages,
};
use crate::AppState;

#[tauri::command]
pub async fn update_table_data(
    state: tauri::State<'_, AppState>,
    table_name: String,
    z_values: Vec<Vec<f64>>,
) -> Result<(), String> {
    let mut conn_guard = state.connection.lock().await;
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    ensure_tune_cache(&state, def).await;

    let mut cache_guard = state.tune_cache.lock().await;

    let table = def
        .get_table_by_name_or_map(&table_name)
        .ok_or_else(|| format!("Table {} not found", table_name))?;

    let constant = def
        .constants
        .get(&table.map)
        .ok_or_else(|| format!("Constant {} not found for table {}", table.map, table_name))?;

    let flat_values: Vec<f64> = z_values.into_iter().flatten().collect();

    if flat_values.len() != constant.shape.element_count() {
        return Err(format!(
            "Invalid data size: expected {}, got {}",
            constant.shape.element_count(),
            flat_values.len()
        ));
    }

    let element_size = constant.data_type.size_bytes();
    let mut raw_data = vec![0u8; constant.size_bytes()];

    for (i, val) in flat_values.iter().enumerate() {
        let raw_val = constant.display_to_raw(*val);
        let offset = i * element_size;
        constant
            .data_type
            .write_to_bytes(&mut raw_data, offset, raw_val, def.endianness);
    }

    let mut persisted = false;

    if let Some(cache) = cache_guard.as_mut() {
        persisted = cache.write_bytes(constant.page, constant.offset, &raw_data);
    }

    {
        let mut tune_guard = state.current_tune.lock().await;
        if let Some(tune) = tune_guard.as_mut() {
            write_bytes_to_tune_pages(tune, def, constant.page, constant.offset, &raw_data);
            persisted = true;
        }
    }

    if !persisted {
        return Err("Failed to persist table data — no tune loaded".to_string());
    }

    *state.tune_modified.lock().await = true;

    if let Some(conn) = conn_guard.as_mut() {
        let params = libretune_core::protocol::commands::WriteMemoryParams {
            can_id: 0,
            page: constant.page,
            offset: constant.offset,
            data: raw_data.clone(),
        };

        if let Err(e) = conn.write_memory(params) {
            eprintln!("[WARN] Failed to write to ECU (offline mode?): {}", e);
        }
    }

    drop(conn_guard);
    drop(cache_guard);
    drop(def_guard);

    maybe_auto_save_project_tune(&state).await;

    Ok(())
}
