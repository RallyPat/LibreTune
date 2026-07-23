//! start_autotune command and read_axis_bins helper (extracted from lib.rs).

use crate::read_raw_value;
use crate::state::{is_maf_channel_name, AppState, AutoTuneConfig, AutoTuneLoadSource, AxisHint};
use libretune_core::autotune::{
    AutoTuneAuthorityLimits, AutoTuneFilters, AutoTuneReferenceTables, AutoTuneSettings,
};
use libretune_core::ini::{Constant, EcuDefinition};
use libretune_core::tune::TuneCache;

#[tauri::command]
pub async fn start_autotune(
    state: tauri::State<'_, AppState>,
    table_name: String,
    secondary_table_name: Option<String>,
    load_source: Option<AutoTuneLoadSource>,
    settings: AutoTuneSettings,
    filters: AutoTuneFilters,
    authority_limits: AutoTuneAuthorityLimits,
    target_afr_table_name: Option<String>,
    lambda_delay_table_name: Option<String>,
    strict_lambda_match: Option<bool>,
) -> Result<(), String> {
    // Get the table definition to extract bin values
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("No ECU definition loaded")?;
    let cache_guard = state.tune_cache.lock().await;
    let cache = cache_guard.as_ref();

    let mut resolved_load_source = load_source.unwrap_or(AutoTuneLoadSource::Map);

    // Find the table and extract bins
    let (x_bins, y_bins) = if let Some(table) = def.get_table_by_name_or_map(&table_name) {
        let y_output_channel = table.y_output_channel.clone();
        if resolved_load_source == AutoTuneLoadSource::Map {
            if let Some(ref channel) = y_output_channel {
                if is_maf_channel_name(channel) {
                    resolved_load_source = AutoTuneLoadSource::Maf;
                }
            }
        }

        // Read X bins from the constant
        let x_bins = read_axis_bins(def, cache, &table.x_bins, table.x_size, AxisHint::Rpm)?;

        // Read Y bins from the constant (if it's a 3D table)
        let y_bins = if let Some(ref y_bins_name) = table.y_bins {
            read_axis_bins(
                def,
                cache,
                y_bins_name,
                table.y_size,
                AxisHint::Load(resolved_load_source),
            )?
        } else {
            vec![0.0] // 2D table has single Y bin
        };

        (x_bins, y_bins)
    } else {
        // Use default bins if table not found
        let default_y_bins = match resolved_load_source {
            AutoTuneLoadSource::Maf => {
                vec![0.0, 25.0, 50.0, 75.0, 100.0, 150.0, 200.0, 250.0, 300.0]
            }
            AutoTuneLoadSource::Map => vec![20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0],
        };

        (
            vec![
                500.0, 1000.0, 1500.0, 2000.0, 2500.0, 3000.0, 3500.0, 4000.0, 4500.0, 5000.0,
                5500.0, 6000.0,
            ],
            default_y_bins,
        )
    };

    if resolved_load_source == AutoTuneLoadSource::Maf {
        let has_maf_channel = def
            .output_channels
            .keys()
            .any(|name| is_maf_channel_name(name));
        if !has_maf_channel {
            resolved_load_source = AutoTuneLoadSource::Map;
        }
    }

    let (secondary_x_bins, secondary_y_bins) = if let Some(ref secondary_name) =
        secondary_table_name
    {
        if let Some(table) = def.get_table_by_name_or_map(secondary_name) {
            let x_bins = read_axis_bins(def, cache, &table.x_bins, table.x_size, AxisHint::Rpm)?;
            let y_bins = if let Some(ref y_bins_name) = table.y_bins {
                read_axis_bins(
                    def,
                    cache,
                    y_bins_name,
                    table.y_size,
                    AxisHint::Load(resolved_load_source),
                )?
            } else {
                vec![0.0]
            };

            (Some(x_bins), Some(y_bins))
        } else {
            return Err(format!("Secondary table {} not found", secondary_name));
        }
    } else {
        (None, None)
    };

    // Resolve the per-cell Target AFR and lambda-delay reference tables
    // (bug #14). The caller may name them explicitly; otherwise we attempt
    // best-effort auto-discovery from the INI by common table/map names. Any
    // lookup failure falls back to an empty table, which AutoTune handles by
    // reverting to settings.target_afr and the RPM-based delay curve.
    let reference_tables = resolve_reference_tables(
        def,
        cache,
        &table_name,
        target_afr_table_name.as_deref(),
        lambda_delay_table_name.as_deref(),
    );

    drop(cache_guard);
    drop(def_guard);

    // Store the config for realtime stream to use
    let strict = strict_lambda_match.unwrap_or(true);
    let config = AutoTuneConfig {
        table_name: table_name.clone(),
        secondary_table_name: secondary_table_name.clone(),
        settings: settings.clone(),
        filters: filters.clone(),
        authority_limits: authority_limits.clone(),
        load_source: resolved_load_source,
        x_bins,
        y_bins,
        secondary_x_bins,
        secondary_y_bins,
        last_tps: None,
        last_timestamp_ms: None,
        reference_tables: reference_tables.clone(),
        strict_lambda_match: strict,
    };

    *state.autotune_config.lock().await = Some(config);

    let mut guard = state.autotune_state.lock().await;
    guard.set_reference_tables(reference_tables.clone());
    guard.set_strict_lambda_match(strict);
    guard.start();

    let mut secondary_guard = state.autotune_secondary_state.lock().await;
    secondary_guard.set_reference_tables(reference_tables);
    secondary_guard.set_strict_lambda_match(strict);
    if secondary_table_name.is_some() {
        secondary_guard.start();
    } else {
        secondary_guard.stop();
    }
    Ok(())
}
/// Read axis bin values from a constant definition
pub(crate) fn read_axis_bins(
    def: &EcuDefinition,
    cache: Option<&TuneCache>,
    const_name: &str,
    size: usize,
    axis_hint: AxisHint,
) -> Result<Vec<f64>, String> {
    let fallback_bins = |hint: AxisHint, size: usize| -> Vec<f64> {
        let steps = (size.saturating_sub(1)).max(1) as f64;
        match hint {
            AxisHint::Rpm => (0..size)
                .map(|i| 500.0 + (i as f64 * 6000.0 / steps))
                .collect(),
            AxisHint::Load(AutoTuneLoadSource::Maf) => (0..size)
                .map(|i| 0.0 + (i as f64 * 300.0 / steps))
                .collect(),
            AxisHint::Load(AutoTuneLoadSource::Map) => (0..size)
                .map(|i| 20.0 + (i as f64 * 80.0 / steps))
                .collect(),
            AxisHint::Unknown => {
                if size > 8 {
                    (0..size)
                        .map(|i| 500.0 + (i as f64 * 6000.0 / steps))
                        .collect()
                } else {
                    (0..size)
                        .map(|i| 20.0 + (i as f64 * 80.0 / steps))
                        .collect()
                }
            }
        }
    };

    // Try to get the constant
    let constant = match def.constants.get(const_name) {
        Some(c) => c,
        None => {
            // Constant not found, generate linear bins
            return Ok(fallback_bins(axis_hint, size));
        }
    };

    // If we have cached tune data, read from it
    if let Some(cache) = cache {
        if let Some(page_data) = cache.get_page(constant.page) {
            let elem_size = constant.data_type.size_bytes();
            let mut bins = Vec::with_capacity(size);
            let mut offset = constant.offset as usize;

            for _ in 0..size {
                if offset + elem_size <= page_data.len() {
                    if let Ok(raw) = read_raw_value(&page_data[offset..], &constant.data_type) {
                        bins.push(constant.raw_to_display(raw));
                    }
                    offset += elem_size;
                }
            }

            if !bins.is_empty() {
                return Ok(bins);
            }
        }
    }

    // Last resort: generate linear bins based on axis hint
    Ok(fallback_bins(axis_hint, size))
}

/// Resolve the per-cell Target AFR and lambda-delay reference tables for an
/// AutoTune session (bug #14).
///
/// Lookup order for each table:
/// 1. The explicit name passed by the caller (UI override).
/// 2. Best-effort auto-discovery from the INI by common table/map names.
///
/// Any failure returns an empty table for that slot, which AutoTune handles by
/// falling back to `settings.target_afr` (for AFR) or the RPM-based delay curve
/// (for lambda delay). This never fails the whole `start_autotune` call.
fn resolve_reference_tables(
    def: &EcuDefinition,
    cache: Option<&TuneCache>,
    ve_table_name: &str,
    target_afr_table_name: Option<&str>,
    lambda_delay_table_name: Option<&str>,
) -> AutoTuneReferenceTables {
    let target_afr_table = resolve_named_table(def, cache, target_afr_table_name, &[
        "afrTable",
        "afr_target",
        "afrTarget",
        "targetAfr",
        "afrTable1",
        "lambdaTable",
    ])
    .unwrap_or_default();

    // Lambda-delay tables are uncommon; only attempt when named explicitly or
    // via the most common Speeduino/rusEFI identifier.
    let lambda_delay_table = resolve_named_table(def, cache, lambda_delay_table_name, &[
        "lambdaDelay",
        "egoDelay",
    ])
    .unwrap_or_default();

    // Suppress unused warning for the VE table name; it's available for future
    // INI cross-referencing (e.g. walking the VE table's own reference field).
    let _ = ve_table_name;

    AutoTuneReferenceTables {
        lambda_delay_table,
        target_afr_table,
    }
}

/// Look up a 2D table by an explicit name first, then by a list of candidate
/// names. Reads the table's Z (data) constant from the tune cache and reshapes
/// it to row-major `[row][col]` matching the VE table layout. Returns
/// `None` if no candidate resolves to a known table or the data cannot be read.
fn resolve_named_table(
    def: &EcuDefinition,
    cache: Option<&TuneCache>,
    explicit: Option<&str>,
    candidates: &[&str],
) -> Option<Vec<Vec<f64>>> {
    // Build the ordered list of names to try. Explicit name first.
    let mut names: Vec<&str> = Vec::new();
    if let Some(n) = explicit {
        names.push(n);
    }
    names.extend_from_slice(candidates);

    for name in names {
        if let Some(table) = def.get_table_by_name_or_map(name) {
            if let Some(rows) = read_table_z_values(def, cache, table.map.as_str(), table.x_size, table.y_size) {
                return Some(rows);
            }
        }
    }
    None
}

/// Read the Z (data) values of a table constant and reshape into row-major
/// `[row][col]`. Returns `None` on any read failure or zero-size table.
fn read_table_z_values(
    def: &EcuDefinition,
    cache: Option<&TuneCache>,
    map_name: &str,
    cols: usize,
    rows: usize,
) -> Option<Vec<Vec<f64>>> {
    if rows == 0 || cols == 0 {
        return None;
    }
    let constant: &Constant = def.constants.get(map_name)?;
    let cache = cache?;
    let page_data = cache.get_page(constant.page)?;
    let elem_size = constant.data_type.size_bytes();
    if elem_size == 0 {
        return None;
    }
    let mut offset = constant.offset as usize;

    let mut out = Vec::with_capacity(rows);
    for _ in 0..rows {
        let mut row = Vec::with_capacity(cols);
        for _ in 0..cols {
            if offset + elem_size <= page_data.len() {
                if let Ok(raw) = read_raw_value(&page_data[offset..], &constant.data_type) {
                    row.push(constant.raw_to_display(raw));
                } else {
                    row.push(0.0);
                }
                offset += elem_size;
            } else {
                row.push(0.0);
            }
        }
        out.push(row);
    }
    Some(out)
}
