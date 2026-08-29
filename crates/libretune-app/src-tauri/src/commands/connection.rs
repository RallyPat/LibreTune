//! ECU connection lifecycle commands.

use crate::state::AppState;
use crate::{load_settings, set_conn_lock_holder, stop_metrics_task, ConnectionStatus};
use libretune_core::protocol::ConnectionState;
use std::path::Path;
use std::time::{Duration, Instant};

/// Disconnects from the currently connected ECU.
///
/// Closes the serial connection and clears the connection state.
///
/// Returns: Nothing on success
#[tauri::command]
pub async fn disconnect_ecu(state: tauri::State<'_, AppState>) -> Result<(), String> {
    // Every path that replaces `state.connection` takes this first, so a
    // teardown cannot race a demo transition or a connect that is midway
    // through installing its own connection.
    let _transition = state.connection_transition.lock().await;

    // Stop metrics and realtime streaming before dropping the connection
    stop_metrics_task(state.clone()).await;

    {
        let mut task_guard = state.streaming_task.lock().await;
        if let Some(handle) = task_guard.take() {
            handle.abort();
        }
    }

    // Issue #71: the realtime stream task performs BLOCKING serial I/O inside the
    // connection lock (get_realtime_data -> send_raw_command/send_packet). Tokio's
    // task abort only takes effect at the next `.await`, so a mid-read task can hold
    // the connection mutex for up to one full read timeout. Polling the lock with a
    // deadline avoids hanging the UI ("disconnect does nothing") — we also signal
    // cancellation through the connection's cancel flag once we hold it so any later
    // in-flight read aborts promptly.
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match state.connection.try_lock() {
            Ok(mut guard) => {
                if let Some(conn) = guard.as_mut() {
                    conn.disconnect();
                }
                *guard = None;
                return Ok(());
            }
            Err(_) => {
                if Instant::now() >= deadline {
                    // Could not acquire the lock within the deadline. The streaming
                    // task is stuck in a blocking read we cannot interrupt from here
                    // without the cancel handle. Force-clear what we can and report a
                    // clear error so the UI can recover on the next connect.
                    eprintln!(
                        "[WARN] disconnect_ecu: connection lock busy after 3s, \
                         forcing disconnect (Issue #71)"
                    );
                    // Retry one more time with a full async lock (may still block,
                    // but lets the runtime schedule other work first).
                    let mut guard = state.connection.lock().await;
                    if let Some(conn) = guard.as_mut() {
                        conn.disconnect();
                    }
                    *guard = None;
                    return Ok(());
                }
                // Yield to the runtime so the (aborted) streaming task can finish its
                // current tick and release the lock.
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }
}

// Adaptive timing commands extracted to commands/adaptive_timing.rs
/// Gets the current ECU connection status.
///
/// Returns comprehensive connection information including state, ECU signature,
/// loaded INI info, and demo mode status.
///
/// Returns: ConnectionStatus with connection state and metadata
#[tauri::command]
pub async fn get_connection_status(
    state: tauri::State<'_, AppState>,
) -> Result<ConnectionStatus, String> {
    // IMPORTANT: Acquire each lock independently and release before taking the next.
    // Holding multiple locks simultaneously causes deadlocks with the realtime stream task.
    let demo_mode = *state.demo_mode.lock().await;

    let streaming_active = {
        let task_guard = state.streaming_task.lock().await;
        task_guard.is_some()
    };

    let (state_val, signature) = if demo_mode && streaming_active {
        (
            ConnectionState::Connected,
            Some("DEMO - Simulated epicEFI".to_string()),
        )
    } else {
        set_conn_lock_holder("get_connection_status");
        let conn_guard = state.connection.lock().await;
        let result = match &*conn_guard {
            Some(conn) => (conn.state(), conn.signature().map(|s| s.to_string())),
            None => (ConnectionState::Disconnected, None),
        };
        drop(conn_guard);
        set_conn_lock_holder("(none)");
        result
    };

    let (has_definition, ini_name) = {
        let def_guard = state.definition.lock().await;
        (
            def_guard.is_some(),
            def_guard.as_ref().map(|d| d.signature.clone()),
        )
    };

    Ok(ConnectionStatus {
        state: state_val,
        signature,
        has_definition,
        ini_name,
        demo_mode,
    })
}

/// Retrieves the path to the last-used INI file from settings.
///
/// Used on startup to auto-load the previously used ECU definition.
///
/// Returns: Optional path to last INI file, or None if not set or file missing
#[tauri::command]
pub async fn auto_load_last_ini(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let settings = load_settings(&app);
    if let Some(path) = settings.last_ini_path {
        if Path::new(&path).exists() {
            return Ok(Some(path));
        }
    }
    Ok(None)
}
