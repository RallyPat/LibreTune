//! get_table_data command (extracted from lib.rs).

use crate::{get_table_data_internal, AppState, TableData};

#[tauri::command]
pub async fn get_table_data(
    state: tauri::State<'_, AppState>,
    table_name: String,
) -> Result<TableData, String> {
    get_table_data_internal(&state, &table_name).await
}
