//! INI dialog/indicator/port-editor/help/expression query commands.

use crate::port_editor::{load_port_editor_store, save_port_editor_store, PortEditorAssignment};
use crate::state::AppState;
use libretune_core::ini::{DialogDefinition, HelpTopic};
use std::collections::HashMap;

/// Evaluates an INI expression (visibility condition) with given context values.
///
/// Used to determine if menu items, dialogs, or fields should be shown
/// based on current constant values.
///
/// # Arguments
/// * `expression` - INI expression string (e.g., "{ nCylinders > 4 }")
/// * `context` - HashMap of variable names to current values
///
/// Returns: Boolean result of expression evaluation
#[tauri::command]
pub async fn evaluate_expression(
    _state: tauri::State<'_, AppState>,
    expression: String,
    context: HashMap<String, f64>,
) -> Result<bool, String> {
    let mut parser = libretune_core::ini::expression::Parser::new(&expression);
    let expr = parser.parse()?;
    let val = libretune_core::ini::expression::evaluate_simple(&expr, &context)?;
    Ok(val.as_bool())
}

/// Retrieves a dialog definition from the INI file.
///
/// Gets the complete dialog structure including panels, fields, and layout
/// for rendering settings dialogs.
///
/// # Arguments
/// * `name` - Dialog name from INI definition
///
/// Returns: Complete DialogDefinition structure
#[tauri::command]
pub async fn get_dialog_definition(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<DialogDefinition, String> {
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    def.dialogs
        .get(&name)
        .cloned()
        .ok_or_else(|| format!("Dialog {} not found", name))
}

/// Retrieves an indicator panel definition from the INI file.
///
/// # Arguments
/// * `name` - Indicator panel name from INI definition
///
/// Returns: IndicatorPanel structure with LED/indicator configurations
#[tauri::command]
pub async fn get_indicator_panel(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<libretune_core::ini::IndicatorPanel, String> {
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    def.indicator_panels
        .get(&name)
        .cloned()
        .ok_or_else(|| format!("IndicatorPanel {} not found", name))
}

/// Retrieves a port editor configuration from the INI file.
///
/// # Arguments
/// * `name` - Port editor name from INI definition
///
/// Returns: PortEditorConfig for I/O pin assignment UI
#[tauri::command]
pub async fn get_port_editor(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<libretune_core::ini::PortEditorConfig, String> {
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    // First try to get from INI definition
    if let Some(config) = def.port_editors.get(&name) {
        return Ok(config.clone());
    }

    // For built-in std_port_edit, provide a default if not explicitly defined
    if name == "std_port_edit" {
        return Ok(libretune_core::ini::PortEditorConfig {
            name: "std_port_edit".to_string(),
            label: "Output Port Settings".to_string(),
            enable_condition: None,
        });
    }

    Err(format!("PortEditor {} not found", name))
}

/// Retrieves saved port editor assignments for the current project.
#[tauri::command]
pub async fn get_port_editor_assignments(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<Vec<PortEditorAssignment>, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let store = load_port_editor_store(project)?;
    Ok(store.assignments.get(&name).cloned().unwrap_or_default())
}

/// Saves port editor assignments for the current project.
#[tauri::command]
pub async fn save_port_editor_assignments(
    state: tauri::State<'_, AppState>,
    name: String,
    assignments: Vec<PortEditorAssignment>,
) -> Result<(), String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard
        .as_ref()
        .ok_or_else(|| "No project open".to_string())?;

    let mut store = load_port_editor_store(project)?;
    store.assignments.insert(name, assignments);
    save_port_editor_store(project, &store)
}

/// Retrieves a help topic from the INI file.
///
/// # Arguments
/// * `name` - Help topic name from INI definition
///
/// Returns: HelpTopic with title and content
#[tauri::command]
pub async fn get_help_topic(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<HelpTopic, String> {
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    def.help_topics
        .get(&name)
        .cloned()
        .ok_or_else(|| format!("Help topic {} not found", name))
}
