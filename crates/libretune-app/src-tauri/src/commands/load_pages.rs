//! Background page loader command.

use crate::AppState;
use tauri::Emitter;

/// Load all ECU pages into the cache (background operation)
#[tauri::command]
pub async fn load_all_pages(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    // Get pages to load and their sizes
    let pages_to_load: Vec<(u8, u16)>;
    {
        let cache_guard = state.tune_cache.lock().await;
        let def_guard = state.definition.read().await;

        let cache = cache_guard.as_ref().ok_or("TuneCache not initialized")?;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;

        pages_to_load = cache
            .pages_to_load()
            .into_iter()
            .filter_map(|p| def.page_sizes.get(p as usize).map(|size| (p, *size)))
            .collect();
    }

    if pages_to_load.is_empty() {
        return Ok(());
    }

    // Mark pages as loading
    {
        let mut cache_guard = state.tune_cache.lock().await;
        if let Some(cache) = cache_guard.as_mut() {
            for (page, _) in &pages_to_load {
                cache.mark_loading(*page);
            }
        }
    }

    // Emit loading started event
    let _ = app.emit(
        "cache:loading",
        serde_json::json!({
            "pages": pages_to_load.len(),
            "status": "started"
        }),
    );

    // Load pages one at a time to avoid blocking
    for (page, size) in pages_to_load {
        // Read page from ECU
        let page_data: Result<Vec<u8>, String> = {
            let mut conn_guard = state.connection.lock().await;
            if let Some(conn) = conn_guard.as_mut() {
                let params = libretune_core::protocol::commands::ReadMemoryParams {
                    can_id: 0,
                    page,
                    offset: 0,
                    length: size,
                };
                conn.read_memory(params).map_err(|e| e.to_string())
            } else {
                Err("Not connected".to_string())
            }
        };

        // Update cache with result
        {
            let mut cache_guard = state.tune_cache.lock().await;
            if let Some(cache) = cache_guard.as_mut() {
                match page_data {
                    Ok(data) => {
                        cache.load_page(page, data);
                        let _ = app.emit(
                            "cache:page_loaded",
                            serde_json::json!({
                                "page": page,
                                "success": true
                            }),
                        );
                    }
                    Err(e) => {
                        cache.mark_error(page);
                        let _ = app.emit(
                            "cache:page_loaded",
                            serde_json::json!({
                                "page": page,
                                "success": false,
                                "error": e
                            }),
                        );
                    }
                }
            }
        }

        // Small delay between pages to avoid overwhelming the ECU
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // Emit loading complete event
    let _ = app.emit(
        "cache:loading",
        serde_json::json!({
            "status": "complete"
        }),
    );

    Ok(())
}
