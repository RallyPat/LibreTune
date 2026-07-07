//! sync_ecu_data command (extracted from lib.rs).

use crate::{set_conn_lock_holder, AppState, SyncProgress, SyncResult};
use libretune_core::tune::TuneFile;
use std::collections::{HashMap, HashSet};
use tauri::Emitter;

async fn snapshot_baseline_pages(state: &AppState) -> HashMap<u8, Vec<u8>> {
    let cache_guard = state.tune_cache.lock().await;
    if let Some(cache) = cache_guard.as_ref() {
        let mut snapshot = HashMap::new();
        for page in 0..cache.page_count() {
            if let Some(data) = cache.get_page(page) {
                snapshot.insert(page, data.to_vec());
            }
        }
        return snapshot;
    }
    HashMap::new()
}

fn pages_with_differences(
    baseline: &HashMap<u8, Vec<u8>>,
    ecu: &HashMap<u8, Vec<u8>>,
    n_pages: u8,
) -> Vec<u8> {
    let all_pages: HashSet<u8> = baseline
        .keys()
        .chain(ecu.keys())
        .copied()
        .filter(|p| *p < n_pages)
        .collect();

    let mut diff_pages = Vec::new();
    for page_num in all_pages {
        match (baseline.get(&page_num), ecu.get(&page_num)) {
            (Some(b), Some(e)) if b == e => {}
            (None, None) => {}
            _ => diff_pages.push(page_num),
        }
    }
    diff_pages.sort_unstable();
    diff_pages
}

#[tauri::command]
pub async fn sync_ecu_data(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<SyncResult, String> {
    // Get definition to know page sizes
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let signature = def.signature.clone();
    let n_pages = def.n_pages;
    let page_sizes: Vec<u32> = def.protocol.page_sizes.clone();
    let total_bytes: usize = page_sizes.iter().map(|&s| s as usize).sum();
    drop(def_guard);

    // Compare against the in-memory tune cache (authoritative editing state),
    // not the raw TuneFile which may have empty pages for MSQ-based projects.
    let baseline_pages = snapshot_baseline_pages(state.inner()).await;

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
    let ecu_tune = tune.clone();
    {
        let mut tune_guard = state.current_tune.lock().await;
        *tune_guard = Some(tune);
    }

    // Mark as not modified (freshly synced from ECU)
    *state.tune_modified.lock().await = false;

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

    // Compare baseline (pre-sync cache) with ECU read — only prompt when bytes differ.
    let diff_pages = pages_with_differences(&baseline_pages, &ecu_tune.pages, n_pages);
    if !diff_pages.is_empty() && pages_synced > 0 {
        let baseline_page_nums: Vec<u8> = baseline_pages.keys().copied().collect();
        let ecu_page_nums: Vec<u8> = ecu_tune.pages.keys().copied().collect();
        let _ = app.emit(
            "tune:mismatch",
            &serde_json::json!({
                "ecu_pages": ecu_page_nums,
                "project_pages": baseline_page_nums,
                "diff_pages": diff_pages,
            }),
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_with_differences_detects_mismatch() {
        let mut baseline = HashMap::new();
        baseline.insert(0, vec![1, 2, 3]);
        let mut ecu = HashMap::new();
        ecu.insert(0, vec![1, 2, 4]);
        assert_eq!(pages_with_differences(&baseline, &ecu, 2), vec![0]);
    }

    #[test]
    fn pages_with_differences_ignores_matching_pages() {
        let mut baseline = HashMap::new();
        baseline.insert(0, vec![1, 2, 3]);
        baseline.insert(1, vec![9, 9]);
        let mut ecu = HashMap::new();
        ecu.insert(0, vec![1, 2, 3]);
        ecu.insert(1, vec![9, 9]);
        assert!(pages_with_differences(&baseline, &ecu, 2).is_empty());
    }
}
