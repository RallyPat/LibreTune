//! ECU connection lifecycle commands.

use crate::state::AppState;
use crate::{load_settings, set_conn_lock_holder, stop_metrics_task, ConnectionStatus};
use libretune_core::protocol::ConnectionState;
use std::path::Path;

/// Disconnects from the currently connected ECU.
///
/// Closes the serial connection and clears the connection state.
///
/// Returns: Nothing on success
#[tauri::command]
pub async fn disconnect_ecu(state: tauri::State<'_, AppState>) -> Result<(), String> {
    // Stop metrics task first
    stop_metrics_task(state.clone()).await;

    let mut guard = state.connection.lock().await;
    *guard = None;
    Ok(())
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

    let (state_val, signature) = if demo_mode {
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
        let def_guard = state.definition.read().await;
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
