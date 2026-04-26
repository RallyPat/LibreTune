//! Find matching INIs command (trivial wrapper around lib helper).

use crate::{find_matching_inis_internal, AppState, MatchingIniInfo};

#[tauri::command]
pub async fn find_matching_inis(
    state: tauri::State<'_, AppState>,
    ecu_signature: String,
) -> Result<Vec<MatchingIniInfo>, String> {
    Ok(find_matching_inis_internal(&state, &ecu_signature).await)
}
