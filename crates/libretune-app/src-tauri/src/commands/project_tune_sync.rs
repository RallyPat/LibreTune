//! Project<->ECU tune sync commands.

use crate::state::AppState;
use libretune_core::tune::TuneFile;
use tokio::time::{sleep, Duration};

#[tauri::command]
pub async fn mark_tune_modified(state: tauri::State<'_, AppState>) -> Result<(), String> {
    *state.tune_modified.lock().await = true;
    Ok(())
}

/// Compare the current project tune with the tune synced from ECU
/// Returns true if they differ, false if identical
#[tauri::command]
pub async fn compare_project_and_ecu_tunes(
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    let tune_guard = state.current_tune.lock().await;
    let project_guard = state.current_project.lock().await;

    // Get ECU tune (synced from ECU, currently in current_tune)
    let ecu_tune = match tune_guard.as_ref() {
        Some(t) => t,
        None => return Ok(false), // No ECU tune, can't compare
    };

    // Get project tune path and load it
    let project_tune = if let Some(ref project) = *project_guard {
        let tune_path = project.current_tune_path();
        if tune_path.exists() {
            match TuneFile::load(&tune_path) {
                Ok(tune) => Some(tune),
                Err(e) => {
                    eprintln!("[WARN] Failed to load project tune for comparison: {}", e);
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    // If no project tune, they're different (ECU has data, project doesn't)
    let project_tune = match project_tune {
        Some(t) => t,
        None => return Ok(true), // Different - project has no tune
    };

    // Compare page data
    // Get all unique page numbers
    let mut all_pages: Vec<u8> = project_tune
        .pages
        .keys()
        .chain(ecu_tune.pages.keys())
        .copied()
        .collect();
    all_pages.sort();
    all_pages.dedup();

    // Compare each page
    for page_num in all_pages {
        let project_page = project_tune.pages.get(&page_num);
        let ecu_page = ecu_tune.pages.get(&page_num);

        match (project_page, ecu_page) {
            (None, None) => continue,                             // Both missing, skip
            (Some(_), None) | (None, Some(_)) => return Ok(true), // One missing, different
            (Some(p), Some(e)) => {
                if p != e {
                    return Ok(true); // Pages differ
                }
            }
        }
    }

    // All pages match
    Ok(false)
}

/// Pause realtime streaming so bulk page writes don't race the OCH poller.
/// The stream treats write responses as bad realtime frames and disconnects after 3 errors.
async fn pause_realtime_stream(state: &tauri::State<'_, AppState>) {
    let mut task_guard = state.streaming_task.lock().await;
    if let Some(handle) = task_guard.take() {
        handle.abort();
    }
    drop(task_guard);
    // Let the aborted task release the connection lock.
    sleep(Duration::from_millis(80)).await;
}

/// The slice of [`libretune_core::protocol::Connection`] that the bulk page
/// write needs. Exists so the write-then-burn sequence can be exercised
/// without a live ECU.
pub(crate) trait PageWriteTarget {
    fn set_auto_burn_on_page_change(&mut self, enabled: bool);
    fn clear_rx_buffer(&mut self);
    fn write_page(&mut self, page: u8, data: &[u8]) -> Result<(), String>;
    fn send_burn_command(&mut self) -> Result<(), String>;
}

impl PageWriteTarget for libretune_core::protocol::Connection {
    fn set_auto_burn_on_page_change(&mut self, enabled: bool) {
        libretune_core::protocol::Connection::set_auto_burn_on_page_change(self, enabled)
    }
    fn clear_rx_buffer(&mut self) {
        libretune_core::protocol::Connection::clear_rx_buffer(self)
    }
    fn write_page(&mut self, page: u8, data: &[u8]) -> Result<(), String> {
        libretune_core::protocol::Connection::write_page(self, page, data)
            .map_err(|e| e.to_string())
    }
    fn send_burn_command(&mut self) -> Result<(), String> {
        libretune_core::protocol::Connection::send_burn_command(self).map_err(|e| e.to_string())
    }
}

/// Write every page to ECU RAM, then commit them with a single burn.
///
/// Auto-burn is off for the duration: a burn between each page costs a 2 s
/// sleep per page change and freezes the link. The one burn at the end walks
/// the connection's dirty-page set, so it covers every page written here —
/// without it the tune lives in RAM only and a power cycle reverts it.
pub(crate) fn write_pages_and_burn<C: PageWriteTarget + ?Sized>(
    conn: &mut C,
    pages: &[(u8, Vec<u8>)],
) -> Result<(), String> {
    conn.set_auto_burn_on_page_change(false);
    conn.clear_rx_buffer();

    let mut result = Ok(());
    for (page_num, page_data) in pages {
        if let Err(e) = conn.write_page(*page_num, page_data) {
            result = Err(format!("Failed to write page {}: {}", page_num, e));
            break;
        }
    }

    // A partial write is never committed to flash — leaving it in RAM lets a
    // power cycle restore the last good tune.
    if result.is_ok() {
        result = conn
            .send_burn_command()
            .map_err(|e| format!("Failed to burn tune to ECU: {}", e));
    }

    conn.clear_rx_buffer();
    conn.set_auto_burn_on_page_change(true);
    result
}

/// Write the project tune to ECU
///
/// Loads the tune from the project's CurrentTune.msq, writes all pages, burns
/// once, and restarts the realtime stream it paused — on the failure path too,
/// so a failed write never leaves the app without realtime data. Callers must
/// not burn or restart the stream again.
#[tauri::command]
pub async fn write_project_tune_to_ecu(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // Lock order: definition before current_project, matching apply_base_map —
    // avoids an AB-BA deadlock against it.
    let def_guard = state.definition.lock().await;
    let project_guard = state.current_project.lock().await;

    let project = project_guard.as_ref().ok_or("No project open")?;
    let _def = def_guard.as_ref().ok_or("Definition not loaded")?;

    // Load project tune
    let tune_path = project.current_tune_path();
    let tune =
        TuneFile::load(&tune_path).map_err(|e| format!("Failed to load project tune: {}", e))?;

    drop(project_guard);
    drop(def_guard);

    pause_realtime_stream(&state).await;

    let mut pages: Vec<(u8, Vec<u8>)> = tune.pages.iter().map(|(k, v)| (*k, v.clone())).collect();
    pages.sort_by_key(|(p, _)| *p);

    let write_result = {
        let mut conn_guard = state.connection.lock().await;
        match conn_guard.as_mut() {
            Some(conn) => write_pages_and_burn(conn, &pages),
            None => Err("Not connected to ECU".to_string()),
        }
    };

    if let Err(e) = write_result {
        // The stream was paused for the write; bring it back before surfacing
        // the failure.
        let _ =
            crate::commands::realtime_stream::start_realtime_stream(app, state.clone(), Some(50))
                .await;
        return Err(e);
    }

    // Update cache and current_tune with project tune
    {
        let mut cache_guard = state.tune_cache.lock().await;
        if let Some(cache) = cache_guard.as_mut() {
            for (page_num, page_data) in &tune.pages {
                cache.load_page(*page_num, page_data.clone());
            }
        }
    }

    let mut tune_guard = state.current_tune.lock().await;
    *tune_guard = Some(tune);

    // Update path to project tune file
    *state.current_tune_path.lock().await = Some(tune_path);

    // Mark as not modified (freshly loaded from project)
    *state.tune_modified.lock().await = false;
    drop(tune_guard);

    let _ =
        crate::commands::realtime_stream::start_realtime_stream(app, state.clone(), Some(50)).await;

    Ok(())
}

/// Save the current tune to the project's tune file
#[tauri::command]
pub async fn save_tune_to_project(state: tauri::State<'_, AppState>) -> Result<(), String> {
    save_tune_to_project_internal(&state).await
}

/// Internal (non-command) project-tune save, shared with the AI-assistant
/// apply path: syncs the cache into the in-memory tune, stamps the INI
/// signature, writes the project's tune file, and clears the modified flag.
pub(crate) async fn save_tune_to_project_internal(state: &AppState) -> Result<(), String> {
    let project_guard = state.current_project.lock().await;
    let project = project_guard.as_ref().ok_or("No project open")?;
    let tune_path = project.current_tune_path();
    drop(project_guard);

    let ini_signature = {
        let def_guard = state.definition.lock().await;
        def_guard.as_ref().map(|d| d.signature.clone())
    };

    // Sync cache pages into the in-memory tune before writing disk.
    let mut tune = {
        let tune_guard = state.current_tune.lock().await;
        tune_guard.as_ref().ok_or("No tune loaded")?.clone()
    };
    {
        let cache_guard = state.tune_cache.lock().await;
        if let Some(cache) = cache_guard.as_ref() {
            for page_num in 0..cache.page_count() {
                if let Some(page_data) = cache.get_page(page_num) {
                    tune.pages.insert(page_num, page_data.to_vec());
                }
            }
        }
    }
    if let Some(sig) = ini_signature {
        tune.signature = sig;
    }

    tune.save(&tune_path)
        .map_err(|e| format!("Failed to save tune to project: {}", e))?;

    *state.current_tune.lock().await = Some(tune);
    *state.current_tune_path.lock().await = Some(tune_path);
    *state.tune_modified.lock().await = false;

    Ok(())
}

#[cfg(test)]
mod write_pages_and_burn_tests {
    use super::{write_pages_and_burn, PageWriteTarget};

    #[derive(Debug, PartialEq, Eq)]
    enum Op {
        AutoBurn(bool),
        ClearRx,
        Write(u8),
        Burn,
    }

    #[derive(Default)]
    struct MockConn {
        ops: Vec<Op>,
        fail_on_page: Option<u8>,
    }

    impl PageWriteTarget for MockConn {
        fn set_auto_burn_on_page_change(&mut self, enabled: bool) {
            self.ops.push(Op::AutoBurn(enabled));
        }
        fn clear_rx_buffer(&mut self) {
            self.ops.push(Op::ClearRx);
        }
        fn write_page(&mut self, page: u8, _data: &[u8]) -> Result<(), String> {
            self.ops.push(Op::Write(page));
            if self.fail_on_page == Some(page) {
                return Err("link died".to_string());
            }
            Ok(())
        }
        fn send_burn_command(&mut self) -> Result<(), String> {
            self.ops.push(Op::Burn);
            Ok(())
        }
    }

    fn pages() -> Vec<(u8, Vec<u8>)> {
        vec![(0, vec![1, 2]), (1, vec![3, 4]), (2, vec![5, 6])]
    }

    #[test]
    fn burns_once_after_every_page_is_written() {
        let mut conn = MockConn::default();

        write_pages_and_burn(&mut conn, &pages()).expect("write succeeds");

        assert_eq!(
            conn.ops,
            vec![
                Op::AutoBurn(false),
                Op::ClearRx,
                Op::Write(0),
                Op::Write(1),
                Op::Write(2),
                Op::Burn,
                Op::ClearRx,
                Op::AutoBurn(true),
            ],
            "expected exactly one burn, issued after all pages are in RAM"
        );
    }

    #[test]
    fn does_not_burn_when_a_page_write_fails() {
        let mut conn = MockConn {
            fail_on_page: Some(1),
            ..MockConn::default()
        };

        let err = write_pages_and_burn(&mut conn, &pages()).expect_err("write fails");

        assert!(err.contains("page 1"), "unexpected error: {err}");
        assert!(
            !conn.ops.contains(&Op::Burn),
            "a partial write must not be committed to flash"
        );
        // Auto-burn is restored even on the failure path.
        assert_eq!(conn.ops.last(), Some(&Op::AutoBurn(true)));
    }
}
