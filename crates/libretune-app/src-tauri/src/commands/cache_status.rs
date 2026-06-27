//! Lightweight read-only commands: get_table_info, get_tune_cache_status.

use crate::commands::ini_meta::{CurveInfo, TableInfo};
use crate::state::AppState;
use libretune_core::tune::PageState;
use serde::Serialize;

/// Lightweight command to check if a table exists in the definition
/// This is used by the frontend to determine if a panel should render as a table button
#[tauri::command]
pub async fn get_table_info(
    state: tauri::State<'_, AppState>,
    table_name: String,
) -> Result<TableInfo, String> {
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or_else(|| {
        eprintln!(
            "[WARN] get_table_info: Definition not loaded when looking for '{}'",
            table_name
        );
        "Definition not loaded".to_string()
    })?;

    // Diagnostic logging
    eprintln!(
        "[DEBUG] get_table_info: Looking for '{}' in {} tables ({} map entries)",
        table_name,
        def.tables.len(),
        def.table_map_to_name.len()
    );

    if let Some(table) = def.get_table_by_name_or_map(&table_name) {
        eprintln!(
            "[DEBUG] get_table_info: Found table '{}' (title: {})",
            table.name, table.title
        );
        Ok(TableInfo {
            name: table.name.clone(),
            title: table.title.clone(),
        })
    } else {
        // Log available tables for debugging
        let available: Vec<_> = def.tables.keys().take(10).cloned().collect();
        eprintln!(
            "[WARN] get_table_info: Table '{}' not found. Available tables (first 10): {:?}",
            table_name, available
        );
        Err(format!(
            "Table '{}' not found (checked {} tables, {} map entries)",
            table_name,
            def.tables.len(),
            def.table_map_to_name.len()
        ))
    }
}

/// Lightweight command to check if a curve exists in the definition.
#[tauri::command]
pub async fn get_curve_info(
    state: tauri::State<'_, AppState>,
    curve_name: String,
) -> Result<CurveInfo, String> {
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or_else(|| {
        eprintln!(
            "[WARN] get_curve_info: Definition not loaded when looking for '{}'",
            curve_name
        );
        "Definition not loaded".to_string()
    })?;

    if let Some(curve) = def.get_curve_by_name_or_map(&curve_name) {
        Ok(CurveInfo {
            name: curve.name.clone(),
            title: curve.title.clone(),
        })
    } else {
        Err(format!(
            "Curve '{}' not found (checked {} curves, {} map entries)",
            curve_name,
            def.curves.len(),
            def.curve_map_to_name.len()
        ))
    }
}

// INI metadata commands extracted to commands/ini_metadata.rs
/// Status of the tune cache for UI display
#[derive(Serialize)]
pub struct TuneCacheStatus {
    /// Total number of pages
    pub total_pages: u8,
    /// Number of pages loaded
    pub loaded_pages: u8,
    /// Whether all pages are loaded
    pub fully_loaded: bool,
    /// Whether currently loading
    pub is_loading: bool,
    /// Whether there are unsaved changes
    pub has_dirty_data: bool,
    /// Whether there are pending burns
    pub has_pending_burn: bool,
    /// Count of dirty bytes
    pub dirty_byte_count: usize,
    /// Pages with dirty data
    pub dirty_pages: Vec<u8>,
}

/// Get the current status of the tune data cache.
///
/// Returns information about loaded pages, dirty data that needs saving,
/// and pending burns. Used to show sync/save status in the UI.
///
/// Returns: TuneCacheStatus with page loading and modification info
#[tauri::command]
pub async fn get_tune_cache_status(
    state: tauri::State<'_, AppState>,
) -> Result<TuneCacheStatus, String> {
    let cache_guard = state.tune_cache.lock().await;
    let cache = cache_guard.as_ref().ok_or("TuneCache not initialized")?;

    let total_pages = cache.page_count();
    let mut loaded_pages = 0u8;
    for page in 0..total_pages {
        match cache.page_state(page) {
            PageState::Clean | PageState::Dirty | PageState::Pending => loaded_pages += 1,
            _ => {}
        }
    }

    Ok(TuneCacheStatus {
        total_pages,
        loaded_pages,
        fully_loaded: cache.is_fully_loaded(),
        is_loading: cache.is_loading(),
        has_dirty_data: cache.has_dirty_data(),
        has_pending_burn: cache.has_pending_burn(),
        dirty_byte_count: cache.dirty_byte_count(),
        dirty_pages: cache.dirty_pages(),
    })
}
