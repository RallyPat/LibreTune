//! Tune file I/O commands: list tune files, burn to ECU, execute controller commands.

use crate::state::AppState;
use libretune_core::ini::{CommandPart, EcuDefinition};
use tauri_plugin_window_state::{AppHandleExt, StateFlags};

/// Lists all tune files in the projects directory.
///
/// Scans for MSQ and JSON tune files.
///
/// Returns: Sorted vector of tune file paths
#[tauri::command]
pub async fn list_tune_files() -> Result<Vec<String>, String> {
    let projects_dir = libretune_core::project::Project::projects_dir()
        .map_err(|e| format!("Failed to get projects directory: {}", e))?;

    // Ensure directory exists
    std::fs::create_dir_all(&projects_dir)
        .map_err(|e| format!("Failed to create projects directory: {}", e))?;

    let mut tunes = Vec::new();

    let entries = std::fs::read_dir(&projects_dir)
        .map_err(|e| format!("Failed to read projects directory: {}", e))?;

    for entry in entries.flatten() {
        if let Some(name) = entry.file_name().to_str() {
            if name.ends_with(".msq") || name.ends_with(".json") {
                tunes.push(entry.path().to_string_lossy().to_string());
            }
        }
    }

    tunes.sort();
    Ok(tunes)
}

/// Burns (writes) tune data from ECU RAM to non-volatile flash memory.
///
/// This is the critical "save to ECU" operation that persists changes.
/// Saves window state first in case of issues.
///
/// Returns: Nothing on success
#[tauri::command]
pub async fn burn_to_ecu(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // Save window state before critical operation (in case of crash)
    let _ = app.save_window_state(StateFlags::all());

    let mut conn_guard = state.connection.lock().await;
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;

    // Send burn command to ECU
    // The 'b' command tells the ECU to write RAM to flash
    conn.send_burn_command()
        .map_err(|e| format!("Burn failed: {}", e))?;

    *state.tune_modified.lock().await = false;

    Ok(())
}

/// Execute a controller command by name
/// Resolves command chains and sends raw bytes to ECU
#[tauri::command]
pub async fn execute_controller_command(
    state: tauri::State<'_, AppState>,
    command_name: String,
) -> Result<(), String> {
    let bytes = resolve_controller_command(&state, &command_name).await?;
    send_controller_command_bytes(&state, &bytes).await
}

/// Resolve a controller command name to raw bytes (for reuse by firmware update, etc.)
pub async fn resolve_controller_command(
    state: &AppState,
    command_name: &str,
) -> Result<Vec<u8>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("No INI definition loaded")?;
    resolve_command_bytes(def, command_name, &mut std::collections::HashSet::new())
}

/// Send pre-resolved controller command bytes to the connected ECU.
pub async fn send_controller_command_bytes(state: &AppState, bytes: &[u8]) -> Result<(), String> {
    let mut conn_guard = state.connection.lock().await;
    let conn = conn_guard.as_mut().ok_or("Not connected to ECU")?;
    conn.send_raw_bytes(bytes)
        .map_err(|e| format!("Failed to send command: {}", e))
}

/// Recursively resolve a command to raw bytes, handling command chaining
fn resolve_command_bytes(
    def: &EcuDefinition,
    command_name: &str,
    visited: &mut std::collections::HashSet<String>,
) -> Result<Vec<u8>, String> {
    // Prevent infinite recursion
    if visited.contains(command_name) {
        return Err(format!(
            "Circular command reference detected: {}",
            command_name
        ));
    }
    visited.insert(command_name.to_string());

    let cmd = def
        .controller_commands
        .get(command_name)
        .ok_or_else(|| format!("Command not found: {}", command_name))?;

    let mut result = Vec::new();

    for part in &cmd.parts {
        match part {
            CommandPart::Raw(raw_str) => {
                // Parse hex escapes and variable substitution
                let bytes = parse_command_string(def, raw_str)?;
                result.extend(bytes);
            }
            CommandPart::Reference(ref_name) => {
                // Recursively resolve referenced command
                let ref_bytes = resolve_command_bytes(def, ref_name, visited)?;
                result.extend(ref_bytes);
            }
        }
    }

    Ok(result)
}

/// Parse a command string with hex escapes (\x00) and variable substitution ($tsCanId)
fn parse_command_string(def: &EcuDefinition, s: &str) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            // Escape sequence
            match chars.next() {
                Some('x') | Some('X') => {
                    // Hex byte: \x00
                    let mut hex = String::new();
                    for _ in 0..2 {
                        if let Some(&c) = chars.peek() {
                            if c.is_ascii_hexdigit() {
                                hex.push(chars.next().unwrap());
                            } else {
                                break;
                            }
                        }
                    }
                    if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                        result.push(byte);
                    }
                }
                Some('n') => result.push(b'\n'),
                Some('r') => result.push(b'\r'),
                Some('t') => result.push(b'\t'),
                Some('\\') => result.push(b'\\'),
                Some(c) => result.push(c as u8),
                None => {}
            }
        } else if ch == '$' {
            // Variable substitution
            let mut var_name = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_alphanumeric() || c == '_' {
                    var_name.push(chars.next().unwrap());
                } else {
                    break;
                }
            }

            // Look up variable value
            if let Some(&value) = def.pc_variables.get(&var_name) {
                result.push(value);
            } else {
                // Variable not found - push 0 as default
                result.push(0);
            }
        } else {
            result.push(ch as u8);
        }
    }

    Ok(result)
}
