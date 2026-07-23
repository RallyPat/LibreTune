//! sync_ecu_data command (extracted from lib.rs).

use crate::commands::tune_apply::materialize_project_pages;
use crate::state::TuneMismatchSnapshot;
use crate::{set_conn_lock_holder, AppState, SyncProgress, SyncResult};
use libretune_core::tune::TuneFile;
use std::collections::HashMap;
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

/// Compare only non-empty pages that were successfully read from the ECU.
fn pages_with_differences(
    baseline: &HashMap<u8, Vec<u8>>,
    ecu: &HashMap<u8, Vec<u8>>,
    n_pages: u8,
    page_sizes: &[u32],
) -> Vec<u8> {
    let mut diff_pages = Vec::new();
    for page_num in 0..n_pages {
        let page_size = page_sizes.get(page_num as usize).copied().unwrap_or(0);
        if page_size == 0 {
            continue;
        }
        match (baseline.get(&page_num), ecu.get(&page_num)) {
            (Some(b), Some(e)) if b != e => diff_pages.push(page_num),
            (None, Some(_)) => diff_pages.push(page_num),
            // Missing ECU page (read failure) is not treated as a diff.
            _ => {}
        }
    }
    diff_pages
}

async fn restore_baseline_pages(state: &AppState, baseline: &HashMap<u8, Vec<u8>>) {
    {
        let mut cache_guard = state.tune_cache.lock().await;
        if let Some(cache) = cache_guard.as_mut() {
            for (page_num, data) in baseline {
                cache.load_page(*page_num, data.clone());
            }
        }
    }

    let signature = {
        let def_guard = state.definition.lock().await;
        def_guard
            .as_ref()
            .map(|d| d.signature.clone())
            .unwrap_or_default()
    };

    if !signature.is_empty() {
        let mut tune = TuneFile::new(&signature);
        for (page_num, data) in baseline {
            tune.pages.insert(*page_num, data.clone());
        }
        let mut tune_guard = state.current_tune.lock().await;
        *tune_guard = Some(tune);
    }
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

    let was_modified = *state.tune_modified.lock().await;

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
            pages_synced += 1;
            tune.pages.insert(page, vec![]);
            {
                let mut cache_guard = state.tune_cache.lock().await;
                if let Some(cache) = cache_guard.as_mut() {
                    cache.load_page(page, vec![]);
                }
            }
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

    // Project side for compare/write must be ECU base + MSQ constants — never the
    // zero-padded cache from Load Tune (that would look like hundreds of fake diffs
    // and corrupt the ECU if written).
    let project_msq = {
        let project_guard = state.current_project.lock().await;
        project_guard.as_ref().and_then(|project| {
            let path = project.current_tune_path();
            if path.exists() {
                TuneFile::load(&path).ok()
            } else {
                None
            }
        })
    };

    let project_pages = {
        let def_guard = state.definition.lock().await;
        if let (Some(def), Some(msq)) = (def_guard.as_ref(), project_msq.as_ref()) {
            materialize_project_pages(def, msq, &ecu_tune.pages)
        } else {
            baseline_pages.clone()
        }
    };

    let diff_pages = pages_with_differences(&project_pages, &ecu_tune.pages, n_pages, &page_sizes);
    let should_emit_mismatch = pages_failed == 0 && !diff_pages.is_empty();

    if should_emit_mismatch {
        let project_page_nums: Vec<u8> = project_pages.keys().copied().collect();
        let ecu_page_nums: Vec<u8> = ecu_tune.pages.keys().copied().collect();
        {
            let mut snapshot_guard = state.tune_mismatch_snapshot.lock().await;
            *snapshot_guard = Some(TuneMismatchSnapshot {
                ecu_pages: ecu_tune.pages.clone(),
            });
        }
        let _ = app.emit(
            "tune:mismatch",
            &serde_json::json!({
                "ecu_pages": ecu_page_nums,
                "project_pages": project_page_nums,
                "diff_pages": diff_pages,
            }),
        );
    } else if pages_failed > 0 && !was_modified {
        // Partial read with no local edits — restore pre-sync cache instead of leaving drift.
        restore_baseline_pages(state.inner(), &baseline_pages).await;
        *state.tune_mismatch_snapshot.lock().await = None;
    } else if pages_failed == 0 {
        *state.tune_modified.lock().await = false;
        *state.tune_mismatch_snapshot.lock().await = None;
        // Keep cache as ECU pages (authoritative after a clean match).
        {
            let mut cache_guard = state.tune_cache.lock().await;
            if let Some(cache) = cache_guard.as_mut() {
                for (page_num, page_data) in &ecu_tune.pages {
                    cache.load_page(*page_num, page_data.clone());
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_with_differences_detects_mismatch() {
        let mut baseline = HashMap::new();
        baseline.insert(0, vec![1, 2, 3]);
        let mut ecu = HashMap::new();
        ecu.insert(0, vec![1, 2, 4]);
        assert_eq!(pages_with_differences(&baseline, &ecu, 2, &[3, 0]), vec![0]);
    }

    #[test]
    fn pages_with_differences_ignores_matching_pages() {
        let mut baseline = HashMap::new();
        baseline.insert(0, vec![1, 2, 3]);
        baseline.insert(1, vec![9, 9]);
        let mut ecu = HashMap::new();
        ecu.insert(0, vec![1, 2, 3]);
        ecu.insert(1, vec![9, 9]);
        assert!(pages_with_differences(&baseline, &ecu, 2, &[3, 2]).is_empty());
    }

    #[test]
    fn pages_with_differences_ignores_failed_ecu_reads() {
        let mut baseline = HashMap::new();
        baseline.insert(0, vec![1, 2, 3]);
        baseline.insert(1, vec![9, 9]);
        let mut ecu = HashMap::new();
        ecu.insert(0, vec![1, 2, 3]);
        assert!(pages_with_differences(&baseline, &ecu, 2, &[3, 2]).is_empty());
    }

    #[test]
    fn pages_with_differences_skips_zero_size_pages() {
        let mut baseline = HashMap::new();
        baseline.insert(0, vec![]);
        baseline.insert(1, vec![1]);
        let mut ecu = HashMap::new();
        ecu.insert(1, vec![2]);
        assert_eq!(pages_with_differences(&baseline, &ecu, 2, &[0, 1]), vec![1]);
    }
}
