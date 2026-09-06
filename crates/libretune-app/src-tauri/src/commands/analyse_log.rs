//! Offline log analysis: run a recorded drive through AutoTune.
//!
//! The backend for the Log Analyze view. Reference tables are resolved through
//! the same [`resolve_reference_tables`] the live session uses, so the target
//! table is discovered, axis-corrected and anchored identically — an offline
//! answer that disagreed with the live one would be worse than none.
//!
//! `.ltlog` is streamed from disk (named columns only). Other formats still
//! arrive pre-parsed from the frontend, where `.msl` preamble handling lives.

use crate::state::AppState;
use libretune_core::autotune::replay::{replay, LogChannels, ReplayConfig, ReplayReport};
use serde::Deserialize;

use super::start_autotune::resolve_reference_tables;

/// Channel names in the file that map onto AutoTune's required columns.
#[derive(Debug, Deserialize)]
pub struct LogColumnMap {
    pub rpm: String,
    pub load: String,
    pub afr: String,
    #[serde(default)]
    pub ve: Option<String>,
    #[serde(default)]
    pub clt: Option<String>,
    #[serde(default)]
    pub tps: Option<String>,
    #[serde(default)]
    pub tps_rate: Option<String>,
    #[serde(default)]
    pub fuel_cut: Option<String>,
    #[serde(default)]
    pub accel_enrich: Option<String>,
}

#[tauri::command]
pub async fn analyse_log(
    state: tauri::State<'_, AppState>,
    table_name: String,
    log: LogChannels,
    config: ReplayConfig,
    target_afr_table_name: Option<String>,
    lambda_delay_table_name: Option<String>,
) -> Result<ReplayReport, String> {
    analyse_channels(
        &state,
        table_name,
        log,
        config,
        target_afr_table_name,
        lambda_delay_table_name,
    )
    .await
}

/// Stream a `.ltlog` and run AutoTune on it. Only the named columns are
/// kept in RAM — not the whole 100-channel file.
#[tauri::command]
pub async fn analyse_log_file(
    state: tauri::State<'_, AppState>,
    path: String,
    table_name: String,
    columns: LogColumnMap,
    config: ReplayConfig,
    target_afr_table_name: Option<String>,
    lambda_delay_table_name: Option<String>,
) -> Result<ReplayReport, String> {
    let names = [
        columns.rpm.as_str(),
        columns.load.as_str(),
        columns.afr.as_str(),
        columns.ve.as_deref().unwrap_or(""),
        columns.clt.as_deref().unwrap_or(""),
        columns.tps.as_deref().unwrap_or(""),
        columns.tps_rate.as_deref().unwrap_or(""),
        columns.fuel_cut.as_deref().unwrap_or(""),
        columns.accel_enrich.as_deref().unwrap_or(""),
    ];
    let wanted: Vec<&str> = names.iter().copied().filter(|s| !s.is_empty()).collect();
    let (_, mut time_ms, cols) =
        libretune_core::datalog::ltlog::extract_ltlog_columns(&path, &wanted)
            .map_err(|e| format!("Failed to read log: {e}"))?;
    if time_ms.is_empty() {
        return Err("The log has no samples with rpm, load and AFR.".into());
    }
    let t0 = time_ms[0];
    for t in &mut time_ms {
        *t -= t0;
    }
    let mut by_name = std::collections::HashMap::new();
    for (name, col) in wanted.iter().zip(cols) {
        by_name.insert(*name, col);
    }
    let mut take = |n: &str| by_name.remove(n).unwrap_or_default();
    let log = LogChannels {
        time_ms,
        rpm: take(&columns.rpm),
        load: take(&columns.load),
        afr: take(&columns.afr),
        ve: take(columns.ve.as_deref().unwrap_or("")),
        clt: take(columns.clt.as_deref().unwrap_or("")),
        tps: take(columns.tps.as_deref().unwrap_or("")),
        tps_rate: take(columns.tps_rate.as_deref().unwrap_or("")),
        fuel_cut: take(columns.fuel_cut.as_deref().unwrap_or("")),
        accel_enrich: take(columns.accel_enrich.as_deref().unwrap_or("")),
    };
    analyse_channels(
        &state,
        table_name,
        log,
        config,
        target_afr_table_name,
        lambda_delay_table_name,
    )
    .await
}

async fn analyse_channels(
    state: &AppState,
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
