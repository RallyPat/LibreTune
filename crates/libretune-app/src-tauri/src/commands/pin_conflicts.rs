//! Pin assignment conflict checks (pre-burn / before pin constant writes).

use crate::commands::constant_values::read_constant_from_cache_or_tune;
use crate::AppState;
use libretune_core::pin_conflict::{
    conflict_if_assigning, detect_pin_conflicts, PinConflictReport,
};

/// Scan the loaded tune for pins claimed by more than one constant.
pub(crate) async fn scan_pin_conflicts(state: &AppState) -> Result<PinConflictReport, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let cache_guard = state.tune_cache.lock().await;
    let tune_guard = state.current_tune.lock().await;
    let endianness = def.endianness;

    Ok(detect_pin_conflicts(def, |name, constant| {
        let v = read_constant_from_cache_or_tune(
            name,
            constant,
            endianness,
            tune_guard.as_ref(),
            cache_guard.as_ref(),
        );
        Some(v as usize)
    }))
}

/// Scan the loaded tune for pins claimed by more than one constant.
#[tauri::command]
pub async fn check_pin_conflicts(
    state: tauri::State<'_, AppState>,
) -> Result<PinConflictReport, String> {
    scan_pin_conflicts(&state).await
}

/// Shared helper: reject assigning `name` to bits index `value` when that pin is taken.
pub(crate) async fn deny_if_pin_conflict(
    state: &AppState,
    name: &str,
    value: f64,
) -> Result<(), String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let cache_guard = state.tune_cache.lock().await;
    let tune_guard = state.current_tune.lock().await;
    let endianness = def.endianness;
    let new_index = value as usize;

    if let Some(conflict) = conflict_if_assigning(def, name, new_index, |other_name, constant| {
        if other_name == name {
            return None;
        }
        let v = read_constant_from_cache_or_tune(
            other_name,
            constant,
            endianness,
            tune_guard.as_ref(),
            cache_guard.as_ref(),
        );
        Some(v as usize)
    }) {
        return Err(format!(
            "Pin '{}' is already used by {}. Clear that assignment first.",
            conflict.pin_label,
            conflict
                .constants
                .iter()
                .filter(|c| c.as_str() != name)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    Ok(())
}
