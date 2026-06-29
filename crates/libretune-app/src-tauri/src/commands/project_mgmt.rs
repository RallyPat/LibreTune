//! Project management commands (close, get, update settings).

use crate::state::AppState;
use crate::{ConnectionSettingsResponse, CurrentProjectInfo};

/// Close the current project and clear state.
///
/// Closes the project, clears the INI definition and tune from memory.
/// Should be called before opening a different project.
///
/// Returns: Nothing on success
#[tauri::command]
pub async fn close_project(state: tauri::State<'_, AppState>) -> Result<(), String> {
    // Get and close the project
    let mut proj_guard = state.current_project.lock().await;
    if let Some(project) = proj_guard.take() {
        project
            .close()
            .map_err(|e| format!("Failed to close project: {}", e))?;
    }

    // Clear definition — drop write lock before acquiring current_tune to avoid ABBA deadlock
    // (get_table_data holds current_tune then tries definition.read; we must not hold
    // definition.write while waiting for current_tune)
    {
        let mut def_guard = state.definition.write().await;
        *def_guard = None;
    }

    // Clear tune cache
    {
        let mut cache_guard = state.tune_cache.lock().await;
        *cache_guard = None;
    }

    // Clear tune
    let mut tune_guard = state.current_tune.lock().await;
    *tune_guard = None;

    Ok(())
}

/// Get information about the currently open project.
///
/// Returns project metadata including name, path, signature, tune status,
/// and connection settings. Returns None if no project is open.
///
/// Returns: Optional CurrentProjectInfo with project details
#[tauri::command]
pub async fn get_current_project(
    state: tauri::State<'_, AppState>,
) -> Result<Option<CurrentProjectInfo>, String> {
    let proj_guard = state.current_project.lock().await;
    let tune_modified = *state.tune_modified.lock().await;

    Ok(proj_guard.as_ref().map(|project| CurrentProjectInfo {
        name: project.config.name.clone(),
        path: project.path.to_string_lossy().to_string(),
        signature: project.config.signature.clone(),
        has_tune: project.current_tune.is_some(),
        tune_modified,
        connection: ConnectionSettingsResponse {
            port: project.config.connection.port.clone(),
            baud_rate: project.config.connection.baud_rate,
            auto_connect: project.config.settings.auto_connect,
        },
    }))
}

/// Update the serial connection settings for the current project.
///
/// Saves the port name and baud rate to the project configuration file.
///
/// # Arguments
/// * `port` - Serial port name (e.g., "COM3", "/dev/ttyUSB0")
/// * `baud_rate` - Baud rate for communication
///
/// Returns: Nothing on success
#[tauri::command]
pub async fn update_project_connection(
    state: tauri::State<'_, AppState>,
    port: Option<String>,
    baud_rate: u32,
) -> Result<(), String> {
    let mut proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_mut()
        .ok_or_else(|| "No project open".to_string())?;

    project.config.connection.port = port;
    project.config.connection.baud_rate = baud_rate;
    project
        .save_config()
        .map_err(|e| format!("Failed to save project config: {}", e))?;

    Ok(())
}

/// Update the auto-connect setting for the current project
#[tauri::command]
pub async fn update_project_auto_connect(
    state: tauri::State<'_, AppState>,
    auto_connect: bool,
) -> Result<(), String> {
    let mut proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_mut()
        .ok_or_else(|| "No project open".to_string())?;

    project.config.settings.auto_connect = auto_connect;
    project
        .save_config()
        .map_err(|e| format!("Failed to save project config: {}", e))?;

    Ok(())
}
