//! ECU console (text-command) Tauri commands.
//!
//! Used by rusEFI/FOME/epicEFI ECUs that support a text-based console
//! interface alongside the binary tuning protocol.

use crate::state::AppState;

const MAX_HISTORY: usize = 1000;

/// Get the current ECU type (for console and other ECU-specific features).
///
/// Returns the stylized display name: "Speeduino", "rusEFI", "FOME",
/// "epicEFI", "MegaSquirt 2", "MegaSquirt 3", or "Unknown".
#[tauri::command]
pub async fn get_ecu_type(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("No INI definition loaded")?;

    Ok(def.ecu_type.display_name().to_string())
}

/// Send a console command to the ECU (rusEFI/FOME/epicEFI only).
///
/// For FOME ECUs with `fome_fast_comms_enabled` setting:
/// - Attempts a faster protocol path first (if available)
/// - Falls back to standard console protocol on error
/// - No error propagation for fallback (transparent to user)
///
/// Returns the response from the ECU with trailing whitespace trimmed.
#[tauri::command]
pub async fn send_console_command(
    state: tauri::State<'_, AppState>,
    _app: tauri::AppHandle,
    command: String,
) -> Result<String, String> {
    let mut conn_guard = state.connection.lock().await;
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;

    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("No INI definition loaded")?;

    if !def.ecu_type.supports_console() {
        return Err(format!(
            "ECU type {:?} does not support text-based console",
            def.ecu_type
        ));
    }

    let response = conn
        .send_console_command(&libretune_core::protocol::ConsoleCommand::new(&command))
        .map_err(|e| format!("Console command failed: {}", e))?;

    let mut history = state.console_history.lock().await;
    history.push(format!("{}: {}", command, &response));
    if history.len() > MAX_HISTORY {
        history.remove(0);
    }

    Ok(response)
}

/// Get full console command history.
#[tauri::command]
pub async fn get_console_history(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    let history = state.console_history.lock().await;
    Ok(history.clone())
}

/// Clear console command history.
#[tauri::command]
pub async fn clear_console_history(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut history = state.console_history.lock().await;
    history.clear();
    Ok(())
}
