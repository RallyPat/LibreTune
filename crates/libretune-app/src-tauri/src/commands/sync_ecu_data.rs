//! sync_ecu_data command (extracted from lib.rs).

use crate::{set_conn_lock_holder, AppState, SyncProgress, SyncResult};
use libretune_core::tune::TuneFile;
use tauri::Emitter;

#[tauri::command]
pub async fn sync_ecu_data(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<SyncResult, String> {
    // Get definition to know page sizes
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let signature = def.signature.clone();
    let n_pages = def.n_pages;
    let page_sizes: Vec<u32> = def.protocol.page_sizes.clone();
    let total_bytes: usize = page_sizes.iter().map(|&s| s as usize).sum();
    let def_clone = def.clone();
    drop(def_guard);

    crate::commands::tune_persist::ensure_tune_cache(&state, &def_clone).await;

    // Create new tune file
    let mut tune = TuneFile::new(&signature);
    let mut bytes_read: usize = 0;
    let mut pages_synced: u8 = 0;
    let mut pages_failed: u8 = 0;
    let mut errors: Vec<String> = Vec::new();

    for page in 0..n_pages {
        let page_size = page_sizes.get(page as usize).copied().unwrap_or(0);

        // Emit progress
        let progress = SyncProgress {
            current_page: page,
            total_pages: n_pages,
            bytes_read,
            total_bytes,
            complete: false,
            failed_page: None,
        };
        let _ = app.emit("sync:progress", &progress);

        if page_size == 0 {
            // Empty page, skip but count as success
            pages_synced += 1;
            continue;
        }

        // Read page data - wrapped in error handling for resilience
        let page_num = page;
        set_conn_lock_holder("sync_ecu_data");
        let mut conn_guard = state.connection.lock().await;
        let conn = match conn_guard.as_mut() {
            Some(c) => c,
            None => {
                set_conn_lock_holder("(none)");
                errors.push(format!("Page {}: Not connected", page_num));
                pages_failed += 1;
                continue;
            }
        };

        // Try to read page - continue on failure
        match conn.read_page(page_num) {
            Ok(page_data) => {
                bytes_read += page_data.len();
                pages_synced += 1;

                // Store in TuneFile
                tune.pages.insert(page_num, page_data.clone());

                // Also populate TuneCache
                {
                    let mut cache_guard = state.tune_cache.lock().await;
                    if let Some(cache) = cache_guard.as_mut() {
                        cache.load_page(page_num, page_data);
                    } else {
                        eprintln!("[WARN] sync_ecu_data: tune cache missing after ensure");
                    }
                }
            }
            Err(e) => {
                let error_msg = format!("Page {}: {}", page_num, e);
                eprintln!("[WARN] sync_ecu_data: {}", error_msg);
                errors.push(error_msg);
                pages_failed += 1;

                // Emit progress with failed page indicator
                let progress = SyncProgress {
                    current_page: page,
                    total_pages: n_pages,
                    bytes_read,
                    total_bytes,
                    complete: false,
                    failed_page: Some(page_num),
                };
                let _ = app.emit("sync:progress", &progress);
            }
        }

        drop(conn_guard);
        set_conn_lock_holder("(none)");
    }

    // Store tune file in state (even if partial)
    {
        let def_guard = state.definition.read().await;
        if let Some(def) = def_guard.as_ref() {
            crate::commands::constant_values::refresh_tune_constants_from_pages(&mut tune, def);
        }
    }

    let mut tune_guard = state.current_tune.lock().await;
    let project_tune = tune_guard.clone(); // Keep copy for comparison
    let ecu_tune = tune.clone(); // Keep copy for comparison
    *tune_guard = Some(tune);

    // Mark as not modified (freshly synced from ECU)
    let mut modified_guard = state.tune_modified.lock().await;
    *modified_guard = false;
    drop(modified_guard);
    drop(tune_guard);

    // Emit complete
    let progress = SyncProgress {
        current_page: n_pages,
        total_pages: n_pages,
        bytes_read,
        total_bytes,
        complete: true,
        failed_page: None,
    };
    let _ = app.emit("sync:progress", &progress);

    if pages_synced > 0 {
        let _ = app.emit("tune:loaded", "ecu_sync");
    }

    // Check if project tune exists and differs from ECU tune
    if let Some(ref project) = project_tune {
        if project.signature == ecu_tune.signature {
            // Compare page data
            let mut has_differences = false;
            let mut diff_pages: Vec<u8> = Vec::new();

            // Check all pages that exist in either tune
            let all_pages: std::collections::HashSet<u8> = project
                .pages
                .keys()
                .chain(ecu_tune.pages.keys())
                .copied()
                .collect();

            for page_num in all_pages {
                let project_page = project.pages.get(&page_num);
                let ecu_page = ecu_tune.pages.get(&page_num);

                match (project_page, ecu_page) {
                    (Some(p), Some(e)) if p != e => {
                        has_differences = true;
                        diff_pages.push(page_num);
                    }
                    (Some(_), None) | (None, Some(_)) => {
                        has_differences = true;
                        diff_pages.push(page_num);
                    }
                    _ => {}
                }
            }

            if has_differences {
                // Emit event for frontend to show dialog
                let ecu_page_nums: Vec<u8> = ecu_tune.pages.keys().copied().collect();
                let project_page_nums: Vec<u8> = project.pages.keys().copied().collect();
                let _ = app.emit(
                    "tune:mismatch",
                    &serde_json::json!({
                        "ecu_pages": ecu_page_nums,
                        "project_pages": project_page_nums,
                        "diff_pages": diff_pages,
                    }),
                );
            }
        }
    }

    // Log detailed errors for debugging
    if !errors.is_empty() {
        eprintln!(
            "[WARN] sync_ecu_data completed with {} errors:",
            errors.len()
        );
        for err in &errors {
            eprintln!("  - {}", err);
        }
    }

    Ok(SyncResult {
        success: pages_failed == 0,
        pages_synced,
        pages_failed,
        total_pages: n_pages,
        errors,
    })
}
