//! Offline log analysis: run a recorded drive through AutoTune.
//!
//! The backend for the Log Analyze view. Reference tables are resolved through
//! the same [`resolve_reference_tables`] the live session uses, so the target
//! table is discovered, axis-corrected and anchored identically — an offline
//! answer that disagreed with the live one would be worse than none.
//!
//! The log arrives already parsed, as one array per channel. Parsing stays in
//! the frontend because that is where the `.msl` preamble handling and its
//! tests already live, and re-implementing it here would give two parsers to
//! disagree with each other.

use crate::state::AppState;
use libretune_core::autotune::replay::{replay, LogChannels, ReplayConfig, ReplayReport};

use super::start_autotune::resolve_reference_tables;

#[tauri::command]
pub async fn analyse_log(
    state: tauri::State<'_, AppState>,
    table_name: String,
    log: LogChannels,
    config: ReplayConfig,
    target_afr_table_name: Option<String>,
    lambda_delay_table_name: Option<String>,
) -> Result<ReplayReport, String> {
    if log.is_empty() {
        return Err("The log has no samples with rpm, load and AFR.".into());
    }

    let def_guard = state.definition.lock().await;
    let def = def_guard
        .as_ref()
        .ok_or_else(|| "No ECU definition loaded".to_string())?;
    let cache_guard = state.tune_cache.lock().await;
    let cache = cache_guard.as_ref();

    let table = def
        .get_table_by_name_or_map(&table_name)
        .ok_or_else(|| format!("Table {table_name} not found in the definition"))?;

    // The table's own axes. Analysing against invented bins would place every
    // sample in the wrong cell while looking perfectly successful, so a failure
    // to read them is fatal here rather than something to guess around.
    let (x_bins, y_bins) = super::start_autotune::read_table_axes(def, cache, table)
        .ok_or_else(|| format!("Could not read the axes of {table_name}"))?;

    let (tables, source) = resolve_reference_tables(
        def,
        cache,
        &table_name,
        target_afr_table_name.as_deref(),
        lambda_delay_table_name.as_deref(),
    );

    tracing::info!(
        table = %table_name,
        samples = log.len(),
        target_afr_source = ?source,
        min_steady_ms = config.filters.min_steady_ms,
        "analyse_log: replaying"
    );

    let report = replay(&log, &x_bins, &y_bins, &tables, &config);
    Ok(report)
}
