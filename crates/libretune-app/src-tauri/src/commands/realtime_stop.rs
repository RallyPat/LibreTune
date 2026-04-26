//! Stop realtime streaming command.

use crate::{stream_log, AppState};

/// Stops the realtime data streaming task.
#[tauri::command]
pub async fn stop_realtime_stream(state: tauri::State<'_, AppState>) -> Result<(), String> {
    stream_log("stop called");
    let mut task_guard = state.streaming_task.lock().await;
    if let Some(handle) = task_guard.take() {
        stream_log("stop: aborting task");
        handle.abort();
    } else {
        stream_log("stop: no task to abort");
    }
    Ok(())
}
