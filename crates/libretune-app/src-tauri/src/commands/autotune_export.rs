//! Saving a tuning session's result so it outlives the session.
//!
//! Recommendations live in memory and nowhere else. Closing the app, or
//! clearing them, throws away a whole drive's collection — which makes the
//! decision to apply them a decision to make *now*, in a car park, on a table
//! nobody has looked at properly. Writing them to a file turns that into a
//! decision that can be made later at a desk, or not at all.

use crate::AppState;
use libretune_core::autotune::{proposal_summary, proposed_ve_table, AutoTuneRecommendation};
use serde::Serialize;

/// A saved proposal: the table as it would be, alongside what it came from.
#[derive(Debug, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedProposal {
    /// Which table this proposes to change.
    pub table_name: String,
    /// The VE table with every recommendation applied, `[row][col]`.
    pub proposed: Vec<Vec<f64>>,
    /// The table as it is now, so the file is a complete record rather than
    /// half of a comparison whose other half may have moved on.
    pub current: Vec<Vec<f64>>,
    /// Cells that would actually change. Not the same as the recommendation
    /// count: the confidence ramp and the change threshold both produce
    /// deliberate no-ops, so counting recommendations overstates the result.
    pub cells_changed: usize,
    /// Largest single change, signed — a fuel cut must not read as a gain.
    pub largest_delta: f64,
    /// Every recommendation, so the file can be re-examined rather than just
    /// applied on trust.
    pub recommendations: Vec<AutoTuneRecommendation>,
    /// ECU signature, so a proposal cannot be quietly applied to another car.
    pub definition_signature: String,
}

/// Build the proposal for `table_name` from the session's recommendations.
#[tauri::command]
pub async fn build_autotune_proposal(
    state: tauri::State<'_, AppState>,
    table_name: String,
) -> Result<SavedProposal, String> {
    let recommendations = {
        let guard = state.autotune_state.lock().await;
        guard.get_recommendations()
    };
    if recommendations.is_empty() {
        return Err("No recommendations to save — the session has collected nothing".into());
    }

    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let cache_guard = state.tune_cache.lock().await;
    let table = def
        .get_table_by_name_or_map(&table_name)
        .ok_or_else(|| format!("Table {table_name} not in the definition"))?;
    let current = crate::commands::start_autotune::read_table_z_values(
        def,
        cache_guard.as_ref(),
        table.map.as_str(),
        table.x_size,
        table.y_size,
    )
    .ok_or("Could not read the current table — is the tune synced?")?;

    let proposed = proposed_ve_table(&current, &recommendations);
    let (cells_changed, largest_delta) = proposal_summary(&current, &recommendations);

    Ok(SavedProposal {
        table_name,
        proposed,
        current,
        cells_changed,
        largest_delta,
        recommendations,
        definition_signature: def.signature.clone(),
    })
}

/// Write the proposal to `path`.
///
/// The caller supplies the path so this cannot quietly overwrite a tune; the
/// UI puts a timestamp in the name. JSON rather than `.msq` deliberately: an
/// `.msq` is a thing a tool will load and burn, and a proposal is not yet a
/// tune. It carries the current values and the per-cell hit counts alongside,
/// which an `.msq` has no room for and which are exactly what is needed to
/// judge it later.
#[tauri::command]
pub async fn save_autotune_proposal(
    state: tauri::State<'_, AppState>,
    table_name: String,
    path: String,
) -> Result<String, String> {
    let proposal = build_autotune_proposal(state, table_name).await?;
    let text = serde_json::to_string_pretty(&proposal).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("write {path}: {e}"))?;
    tracing::info!(
        path = %path,
        cells = proposal.cells_changed,
        largest = proposal.largest_delta,
        "autotune proposal saved"
    );
    Ok(format!(
        "Saved {} changed cell(s), largest {:+.1}, to {}",
        proposal.cells_changed, proposal.largest_delta, path
    ))
}
