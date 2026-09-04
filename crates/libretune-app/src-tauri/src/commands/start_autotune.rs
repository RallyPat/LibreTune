//! start_autotune command and read_axis_bins helper (extracted from lib.rs).

use crate::read_raw_value;
use crate::state::{
    algorithm_selects_tps_load, is_maf_channel_name, is_tps_channel_name, AppState, AutoTuneConfig,
    AutoTuneLoadSource, AxisHint,
};
use libretune_core::autotune::{
    AutoTuneAuthorityLimits, AutoTuneFilters, AutoTuneReferenceTables, AutoTuneSettings,
};
use libretune_core::ini::{Constant, EcuDefinition, TableRole};
use libretune_core::tune::TuneCache;

#[tauri::command]
#[allow(clippy::too_many_arguments)]
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
    tracing::info!(
        table = %table_name,
        secondary = ?secondary_table_name,
        requested_load_source = ?load_source,
        target_afr = settings.target_afr,
        target_afr_table = ?target_afr_table_name,
        lambda_delay_table = ?lambda_delay_table_name,
        "start_autotune: requested"
    );

    // Get the table definition to extract bin values
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or_else(|| {
        tracing::warn!("start_autotune: no ECU definition loaded — cannot start");
        "No ECU definition loaded".to_string()
    })?;
    // This session's corrections are `value * measured/target AFR`, which only
    // means anything on a fuel table. Refused here rather than in the picker,
    // because this path writes to the ECU and a UI is not where a rule that
    // expensive belongs.
    if let Some(why) = libretune_core::autotune::fuel_tune_refusal(def, &table_name) {
        tracing::warn!(table = %table_name, "start_autotune: refused, not a fuel table");
        return Err(why);
    }
    if let Some(secondary) = secondary_table_name.as_deref() {
        if let Some(why) = libretune_core::autotune::fuel_tune_refusal(def, secondary) {
            tracing::warn!(table = %secondary, "start_autotune: refused secondary");
            return Err(why);
        }
    }

    let definition_signature = def.signature.clone();
    let cache_guard = state.tune_cache.lock().await;
    let cache = cache_guard.as_ref();

    let mut resolved_load_source = load_source.unwrap_or(AutoTuneLoadSource::Map);

    // Find the table and extract bins
    let (x_bins, y_bins) = if let Some(table) = def.get_table_by_name_or_map(&table_name) {
        let y_output_channel = table.y_output_channel.clone();
        // Auto-detect the load source from the table's Y-axis output channel
        // when the caller left it at the default (MAP). This is what makes
        // TPS/Alpha-N (ITB) tunes work out of the box: a VE table whose Y axis
        // is a throttle channel is treated as a TPS load, so live data is
        // attributed to the correct cells instead of being dropped or matched
        // against the wrong axis (issue #132).
        if resolved_load_source == AutoTuneLoadSource::Map {
            if let Some(ref channel) = y_output_channel {
                if is_tps_channel_name(channel) {
                    resolved_load_source = AutoTuneLoadSource::Tps;
                } else if is_maf_channel_name(channel) {
                    resolved_load_source = AutoTuneLoadSource::Maf;
                }
            }
            // Speeduino names its VE load-axis output channel `fuelLoad`
            // regardless of the fuel algorithm, so channel-name detection
            // cannot fire there. Fall back to the `algorithm` constant, which
            // is authoritative: 1 = TPS / Alpha-N on Speeduino and MS2/MS3
            // alike (issue #132).
            if resolved_load_source == AutoTuneLoadSource::Map {
                if let Some(algorithm) = def.constants.get("algorithm") {
                    let value = crate::commands::constant_values::read_constant_from_cache_or_tune(
                        "algorithm",
                        algorithm,
                        def.endianness,
                        state.current_tune.lock().await.as_ref(),
                        cache,
                    );
                    if algorithm_selects_tps_load(value) {
                        tracing::info!(
                            value,
                            "start_autotune: algorithm constant selects TPS load"
                        );
                        resolved_load_source = AutoTuneLoadSource::Tps;
                    }
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
            // Alpha-N / ITB: the load axis is throttle opening, 0–100 %.
            AutoTuneLoadSource::Tps => {
                vec![0.0, 10.0, 20.0, 30.0, 40.0, 50.0, 65.0, 80.0, 100.0]
            }
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
    // `min_clt` compares against the coolant channel, and the INI declares the
    // right number for its own unit system:
    //
    //     #if CELSIUS
    //          filter = minCltFilter, "Minimum CLT", coolant, <, 71,  , true
    //     #else
    //          filter = minCltFilter, "Minimum CLT", coolant, <, 160, , true
    //
    // The struct default is a bare 160, sensible in Fahrenheit and above
    // boiling in Celsius - where it rejects every sample a warm engine can
    // produce, and a whole session collects nothing. Warning about that after
    // the fact was treating the symptom; the INI has had the answer all along,
    // so take it when the caller has not chosen a value of their own.
    let mut filters = filters;
    if (filters.min_clt - AutoTuneFilters::default().min_clt).abs() < f64::EPSILON {
        if let Some(declared) = def
            .ve_analyze
            .as_ref()
            .and_then(|c| c.filters.iter().find(|f| f.name == "minCltFilter"))
        {
            tracing::info!(
                was = filters.min_clt,
                now = declared.default_value,
                "min_clt left at its default; taking the value this INI declares"
            );
            filters.min_clt = declared.default_value;
        }
    }

    let (mut reference_tables, target_afr_source) = resolve_reference_tables(
        def,
        cache,
        &table_name,
        target_afr_table_name.as_deref(),
        lambda_delay_table_name.as_deref(),
    );

    // Say which target the corrections are actually chasing. A flat target is a
    // legitimate mode, but silently substituting one for a table the tuner
    // believed was auto-discovered is how a session ends up leaning every
    // full-load cell to stoich.
    match &target_afr_source {
        TargetAfrSource::FlatSetting => tracing::warn!(
            flat_target_afr = settings.target_afr,
            "{}",
            target_afr_source.describe(settings.target_afr)
        ),
        _ => tracing::info!(
            table = target_afr_source.table_name().unwrap_or("-"),
            "{}",
            target_afr_source.describe(settings.target_afr)
        ),
    }

    // Flow-scaled lambda-delay table: when requested and no explicit per-cell
    // delay table was found (the Speeduino case — no lambdaDelay in the INI),
    // synthesise one from the VE table. Transport delay ≈ exhaust volume /
    // flow, and flow ∝ rpm·load·VE, so the delay is long at idle and short at
    // high load. lambda_delay_ms anchors the low-flow end.
    if settings.lambda_delay_flow_scaled && reference_tables.lambda_delay_table.is_empty() {
        if let Some(table) = def.get_table_by_name_or_map(&table_name) {
            if let Some(ve_z) =
                read_table_z_values(def, cache, table.map.as_str(), table.x_size, table.y_size)
            {
                reference_tables.lambda_delay_table =
                    libretune_core::autotune::compute_flow_scaled_delay_table(
                        &ve_z,
                        &x_bins,
                        &y_bins,
                        settings.lambda_delay_ms,
                        settings.lambda_delay_floor_ms,
                    );
            }
        }
    }

    tracing::info!(
        resolved_load_source = ?resolved_load_source,
        x_bins = x_bins.len(),
        y_bins = y_bins.len(),
        table_in_ini = def.get_table_by_name_or_map(&table_name).is_some(),
        cache_present = cache.is_some(),
        flow_scaled = settings.lambda_delay_flow_scaled,
        target_afr_table_resolved = !reference_tables.target_afr_table.is_empty(),
        lambda_delay_table_resolved = !reference_tables.lambda_delay_table.is_empty(),
        "start_autotune: resolved session (empty AFR/delay tables fall back to \
         settings.target_afr / RPM-based delay)"
    );

    drop(cache_guard);
    drop(def_guard);

    // Store the config for realtime stream to use
    let strict = strict_lambda_match.unwrap_or(true);
    let config = AutoTuneConfig {
        table_name: table_name.clone(),
        definition_signature,
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
    tracing::info!(
        table = %table_name,
        secondary_running = secondary_table_name.is_some(),
        "start_autotune: session started"
    );
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
            // Alpha-N / ITB: throttle opening 0–100 %.
            AxisHint::Load(AutoTuneLoadSource::Tps) => (0..size)
                .map(|i| 0.0 + (i as f64 * 100.0 / steps))
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

    // If we have cached tune data, read from it.
    //
    // All `size` bins or none. The old loop skipped a bin it could not read but
    // carried on - a short page returned a truncated axis, and a single failed
    // decode dropped one bin and shifted every bin after it down one position.
    // Either way AutoTune ends up with fewer axis bins than the table has
    // cells, so `nearest_bin` attributes a sample to the wrong RPM or load cell
    // and the recommendation is filed against a cell the engine was never in.
    // `read_table_z_values` already refuses a partial table for the same
    // reason; an axis is no different. Falling through to `fallback_bins` keeps
    // the existing contract for "cannot read this constant" rather than adding
    // a new fabrication path.
    if let Some(cache) = cache {
        if let Some(page_data) = cache.get_page(constant.page) {
            let elem_size = constant.data_type.size_bytes();
            if elem_size == 0 {
                tracing::warn!(
                    axis = const_name,
                    "axis constant has a zero-width data type; using generated bins"
                );
            } else {
                let mut bins = Vec::with_capacity(size);
                let mut offset = constant.offset as usize;

                for i in 0..size {
                    if offset + elem_size > page_data.len() {
                        tracing::warn!(
                            axis = const_name,
                            page = constant.page,
                            bin = i,
                            of = size,
                            need = offset + elem_size,
                            have = page_data.len(),
                            "axis read ran past the end of the page"
                        );
                        bins.clear();
                        break;
                    }
                    match read_raw_value(&page_data[offset..], &constant.data_type, def.endianness)
                    {
                        Ok(raw) => bins.push(constant.raw_to_display(raw)),
                        Err(e) => {
                            tracing::warn!(
                                axis = const_name,
                                bin = i,
                                offset,
                                error = %e,
                                "axis bin failed to decode"
                            );
                            bins.clear();
                            break;
                        }
                    }
                    offset += elem_size;
                }

                if bins.len() == size {
                    return Ok(bins);
                }
                // The page is there but does not hold the axis. Falling through
                // to fallback_bins would answer with a fixed 500-6500 rpm (or
                // 20-100 kPa) ramp that has nothing to do with this table, and
                // AutoTune would file every recommendation against a cell
                // chosen by that invented axis - across the full table width
                // rather than part of it. read_table_z_values refuses the
                // identical condition; the fallback below is for an axis the
                // INI never declared, which is a different thing.
                return Err(format!(
                    "Axis '{const_name}' could not be read from page {}: the page is                      present but does not hold {size} complete bins. Re-sync the ECU                      and try again.",
                    constant.page
                ));
            }
        }
    }

    // Last resort: generate linear bins based on axis hint
    Ok(fallback_bins(axis_hint, size))
}

/// The AFR/lambda target table the INI itself declares, if any.
///
/// Speeduino states the pairing outright rather than leaving it to be guessed:
///
/// ```text
/// #if LAMBDA
///   veAnalyzeMap = veTable1Tbl, lambdaTable1Tbl, lambda, egoCorrection
/// #else
///   veAnalyzeMap = veTable1Tbl, afrTable1Tbl,    afr,    egoCorrection
/// ```
///
/// The parser already turns that into [`TableRole::AfrTarget`], so the answer
/// is sitting in the definition — it was simply never consulted here.
fn ini_declared_afr_target(def: &EcuDefinition) -> Option<&str> {
    // `[VeAnalyze]` names the primary target outright, so take it. Scanning for
    // the role is a fallback, and a poor primary: `infer_table_roles` stamps
    // `AfrTarget` on every entry of `lambdaTargetTables` and on the WUE target
    // too, and `def.tables` is a `HashMap`, whose iteration order is
    // unspecified and varies between runs of the same binary. On this car that
    // means `afrTable1Tbl` and `afrTSCustom` both carry the role, so a scan can
    // silently tune against a custom table one launch and the real one the
    // next, with nothing on screen to distinguish the two sessions.
    if let Some(cfg) = def.ve_analyze.as_ref() {
        let name = cfg.target_table_name.trim();
        if !name.is_empty() && def.get_table_by_name_or_map(name).is_some() {
            return Some(name);
        }
    }

    // No `[VeAnalyze]`: fall back to the role, but deterministically. Sorting
    // by name is arbitrary in meaning yet stable in effect, which is the point
    // - two runs must agree even when neither can be sure it is right.
    let mut candidates: Vec<&str> = def
        .tables
        .values()
        .filter(|t| t.role == TableRole::AfrTarget)
        .map(|t| t.name.as_str())
        .collect();
    candidates.sort_unstable();
    if candidates.len() > 1 {
        tracing::warn!(
            ?candidates,
            chosen = candidates[0],
            "several tables claim the AFR-target role and the INI names no \
             primary; picking the first by name so the choice is at least the \
             same every run"
        );
    }
    candidates.first().copied()
}

/// Resolve the per-cell Target AFR and lambda-delay reference tables for an
/// AutoTune session (bug #14).
///
/// Lookup order for the AFR target:
/// 1. The explicit name passed by the caller (UI override).
/// 2. The table the INI declares as [`TableRole::AfrTarget`].
/// 3. Best-effort name matching, for INIs that declare nothing.
///
/// Step 2 is why this is not merely tidier than guessing. The candidate list
/// held `afrTable1` while every Speeduino names the table **`afrTable1Tbl`**,
/// so auto-discovery missed on every Speeduino project and fell through to a
/// flat `settings.target_afr` of 14.7 — stoich at wide-open throttle, where the
/// car's own table asked for 12.7. On a real drive that produced 154
/// recommendations, 117 of them removing fuel, the worst cutting VE 91 -> 78 at
/// full load. Nothing surfaced the substitution: the UI offers "Auto-discover"
/// and reports success either way.
///
/// The candidate list also spanned both kinds of table (`afrTable` *and*
/// `lambdaTable`), so on a lambda INI it could match a target of ~0.88 against
/// a measured AFR of ~13. `lambdaTable` is gone from it: the role lookup picks
/// the lambda table correctly on a lambda INI, where the measured channel is
/// lambda too, so the pair stays consistent.
///
/// A failure still yields an empty table and the `settings.target_afr`
/// fallback, because a flat target is a documented mode ("blank = use Target
/// AFR setting"). `target_afr_source` records which of the three applied so the
/// caller can say so out loud instead of silently substituting stoich.
pub(crate) fn resolve_reference_tables(
    def: &EcuDefinition,
    cache: Option<&TuneCache>,
    ve_table_name: &str,
    target_afr_table_name: Option<&str>,
    lambda_delay_table_name: Option<&str>,
) -> (AutoTuneReferenceTables, TargetAfrSource) {
    let declared = ini_declared_afr_target(def);
    let requested = target_afr_table_name.or(declared);

    let target_afr_table = resolve_named_table(
        def,
        cache,
        requested,
        &[
            "afrTable",
            "afr_target",
            "afrTarget",
            "targetAfr",
            "afrTable1",
        ],
    )
    .unwrap_or_default();

    // `resolve_target_afr` indexes the target with the VE table's own (x, y),
    // so the two must be the same shape. Nothing checked that. The AFR target
    // has independent axes by design — Speeduino gives it `afrRpmBins`/
    // `afrLoadBins` against the VE table's `veRpmBins`/`veLoadBins` — so a
    // smaller target simply returns `None` for the rows and columns past its
    // end, and those cells fall back to the flat target with no symptom. Once
    // again that is the top-load, high-rpm corner.
    //
    // Equal dimensions are necessary but not sufficient: two 16x16 tables on
    // different bin values line up index-for-index while meaning different
    // things. On this Speeduino the VE rpm axis ends 6000, 6500, 7200, 8000
    // and the AFR target's ends 5300, 5800, 6300, 6800 — so cell 12 asked for
    // the 6300 rpm target while the sample came from 6000 rpm. Measured against
    // the real tables that reaches 0.52 AFR (3.7% VE) at 6000 rpm, and it errs
    // *lean* at the top of the map, which is the direction that costs pistons.
    //
    // So resample the target onto the VE table's own axes rather than trusting
    // the indices to mean the same thing. Only when an axis cannot be read do
    // we fall back to the old shape test, which is still better than nothing.
    let ve_table = def.get_table_by_name_or_map(ve_table_name);
    let ve_shape = ve_table.map(|t| (t.y_size, t.x_size));
    let target_afr_table = match (&ve_shape, target_afr_table.is_empty()) {
        (Some((rows, cols)), false) => {
            let got = (
                target_afr_table.len(),
                target_afr_table.first().map(|r| r.len()).unwrap_or(0),
            );
            let axes = ve_table
                .zip(requested.and_then(|n| def.get_table_by_name_or_map(n)))
                .and_then(|(ve, tgt)| {
                    Some((
                        read_table_axes(def, cache, ve)?,
                        read_table_axes(def, cache, tgt)?,
                    ))
                });

            match axes {
                Some(((ve_x, ve_y), (tgt_x, tgt_y))) if ve_x != tgt_x || ve_y != tgt_y => {
                    tracing::info!(
                        ve_table = ve_table_name,
                        "AFR target table has different axes to the VE table; \
                         resampling it onto the VE axes"
                    );
                    libretune_core::table_ops::rebin_table(
                        &tgt_x,
                        &tgt_y,
                        &target_afr_table,
                        ve_x,
                        ve_y,
                        true,
                    )
                    .z_values
                }
                // Axes already agree: index-for-index is correct as it stands.
                Some(_) => target_afr_table,
                // Axes unreadable — fall back to the shape check.
                None if got == (*rows, *cols) => target_afr_table,
                None => {
                    tracing::warn!(
                        ve_table = ve_table_name,
                        expected = ?(*rows, *cols),
                        got = ?got,
                        "AFR target table is a different shape to the VE table and its \
                         axes could not be read; ignoring it rather than applying it to \
                         part of the map"
                    );
                    Vec::new()
                }
            }
        }
        _ => target_afr_table,
    };

    let target_afr_source = if target_afr_table.is_empty() {
        TargetAfrSource::FlatSetting
    } else if target_afr_table_name.is_some() {
        TargetAfrSource::Explicit(requested.unwrap_or_default().to_string())
    } else if declared.is_some() {
        TargetAfrSource::IniDeclared(requested.unwrap_or_default().to_string())
    } else {
        TargetAfrSource::NameMatch
    };

    // Lambda-delay tables are uncommon; only attempt when named explicitly or
    // via the most common Speeduino/rusEFI identifier.
    let lambda_delay_table = resolve_named_table(
        def,
        cache,
        lambda_delay_table_name,
        &["lambdaDelay", "egoDelay"],
    )
    .unwrap_or_default();

    // Suppress unused warning for the VE table name; it's available for future
    // INI cross-referencing (e.g. walking the VE table's own reference field).
    let _ = ve_table_name;

    (
        AutoTuneReferenceTables {
            lambda_delay_table,
            target_afr_table,
            // The table being tuned. A cell's correction is anchored to its
            // value here rather than to the interpolated `veCurr` of whichever
            // sample hit it first — see `add_data_point`. Empty on a read
            // failure, which restores the old sample-anchored behaviour rather
            // than correcting every cell against zero.
            ve_table: ve_table
                .and_then(|t| read_table_z_values(def, cache, t.map.as_str(), t.x_size, t.y_size))
                .unwrap_or_default(),
        },
        target_afr_source,
    )
}

/// Where a session's per-cell AFR target came from.
///
/// Exists so the answer can be shown to the user. The dangerous case is
/// [`Self::FlatSetting`]: it means every cell is being tuned to one number,
/// which is only ever right if that is what the tuner intended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TargetAfrSource {
    /// Caller named the table explicitly.
    Explicit(String),
    /// Taken from the INI's own `TableRole::AfrTarget` declaration.
    IniDeclared(String),
    /// Matched by name from the candidate list (INI declared nothing).
    NameMatch,
    /// No table resolved — every cell uses the flat `settings.target_afr`.
    FlatSetting,
}

impl TargetAfrSource {
    /// Table name, when a table was actually resolved.
    pub(crate) fn table_name(&self) -> Option<&str> {
        match self {
            Self::Explicit(n) | Self::IniDeclared(n) => Some(n.as_str()),
            Self::NameMatch | Self::FlatSetting => None,
        }
    }

    /// A line fit to show a tuner, naming what the corrections are chasing.
    pub(crate) fn describe(&self, flat_target: f64) -> String {
        match self {
            Self::Explicit(n) => format!("per-cell AFR target from '{n}' (chosen)"),
            Self::IniDeclared(n) => format!("per-cell AFR target from '{n}' (declared by the INI)"),
            Self::NameMatch => "per-cell AFR target from a name-matched table".to_string(),
            Self::FlatSetting => format!(
                "NO AFR target table resolved - every cell targets a flat {flat_target}. \
                 Set 'Target AFR Table' if the engine runs richer under load."
            ),
        }
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
            if let Some(rows) =
                read_table_z_values(def, cache, table.map.as_str(), table.x_size, table.y_size)
            {
                return Some(rows);
            }
        }
    }
    None
}

/// Read a table's X and Y axis values.
///
/// An axis is a 1xN constant, so it goes through the same reader as the Z data
/// — including its refusal to pad a short read, which matters just as much
/// here: a truncated axis would place cells at fabricated rpm.
///
/// `None` if either axis is missing or unreadable, which sends the caller back
/// to comparing shapes rather than silently resampling against a wrong axis.
pub(crate) fn read_table_axes(
    def: &EcuDefinition,
    cache: Option<&TuneCache>,
    table: &libretune_core::ini::TableDefinition,
) -> Option<(Vec<f64>, Vec<f64>)> {
    let y_name = table.y_bins.as_deref()?;
    let x = read_table_z_values(def, cache, &table.x_bins, table.x_size, 1)?.pop()?;
    let y = read_table_z_values(def, cache, y_name, table.y_size, 1)?.pop()?;
    (x.len() == table.x_size && y.len() == table.y_size).then_some((x, y))
}

/// Read the Z (data) values of a table constant and reshape into row-major
/// `[row][col]`. Returns `None` on any read failure or zero-size table.
pub(crate) fn read_table_z_values(
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

    // A short or undecodable page must fail, not pad. Padding produced a table
    // that reported as resolved while its tail was zeros — and because
    // `resolve_target_afr` ignores values <= 0.1, exactly those cells silently
    // reverted to the flat target. The tail of a row-major table is the
    // high-load, high-rpm corner, so the cells that got the fabricated target
    // were the ones where being wrong matters most. `None` costs the caller the
    // whole table, which is the honest outcome: a partially-read table cannot
    // be told apart from a fully-read one by anything downstream.
    let mut out = Vec::with_capacity(rows);
    for _ in 0..rows {
        let mut row = Vec::with_capacity(cols);
        for _ in 0..cols {
            if offset + elem_size > page_data.len() {
                tracing::warn!(
                    map = map_name,
                    page = constant.page,
                    need = offset + elem_size,
                    have = page_data.len(),
                    "table read ran past the end of the page; refusing a partial table"
                );
                return None;
            }
            let raw =
                match read_raw_value(&page_data[offset..], &constant.data_type, def.endianness) {
                    Ok(raw) => raw,
                    Err(e) => {
                        tracing::warn!(
                            map = map_name,
                            offset,
                            error = %e,
                            "table value failed to decode; refusing a partial table"
                        );
                        return None;
                    }
                };
            row.push(constant.raw_to_display(raw));
            offset += elem_size;
        }
        out.push(row);
    }
    Some(out)
}

#[cfg(test)]
mod axis_bin_tests {
    use super::*;
    use libretune_core::ini::DataType;
    use libretune_core::tune::TuneCache;

    /// One 8-entry U16 RPM axis at page offset 0. `page_bytes` decides whether
    /// the page is long enough to hold it.
    fn def_and_cache(page_bytes: usize) -> (EcuDefinition, TuneCache) {
        let mut def = EcuDefinition {
            page_sizes: vec![page_bytes as u16],
            n_pages: 1,
            ..Default::default()
        };
        def.constants.insert(
            "rpmBins".to_string(),
            Constant {
                name: "rpmBins".to_string(),
                page: 0,
                offset: 0,
                data_type: DataType::U16,
                scale: 1.0,
                translate: 0.0,
                ..Default::default()
            },
        );
        let mut cache = TuneCache::from_definition(&def);
        // 500, 1500, 2500 ... U16, big-endian as the INI reader decodes them.
        let mut page = Vec::with_capacity(page_bytes);
        for i in 0..(page_bytes / 2) {
            let rpm = 500u16 + (i as u16 * 1000);
            page.extend_from_slice(&rpm.to_be_bytes());
        }
        page.resize(page_bytes, 0);
        cache.load_page(0, page);
        (def, cache)
    }

    #[test]
    fn a_full_page_reads_the_real_axis() {
        let (def, cache) = def_and_cache(16);
        let bins = read_axis_bins(&def, Some(&cache), "rpmBins", 8, AxisHint::Rpm).unwrap();
        assert_eq!(bins.len(), 8);
        assert_eq!(bins[0], 500.0);
        assert_eq!(bins[7], 7500.0);
    }

    /// The bug: a page holding only 5 of the 8 bins used to return a 5-entry
    /// axis. AutoTune then had fewer bins than the table has cells, so samples
    /// were attributed to the wrong RPM cell entirely - and nothing downstream
    /// could tell a short axis from a complete one.
    #[test]
    fn a_short_page_refuses_rather_than_truncating_or_inventing() {
        let (def, cache) = def_and_cache(10); // 5 x U16, need 8
        let err = read_axis_bins(&def, Some(&cache), "rpmBins", 8, AxisHint::Rpm)
            .expect_err("a page that cannot hold the axis must not answer");
        assert!(
            err.contains("rpmBins") && err.contains("page 0"),
            "the error must name the axis and page: {err}"
        );
    }

    /// Refusing is only correct when the page *is* there and falls short.
    /// An axis the INI never declared has no page to be short, and generated
    /// bins are the only answer available - that path must keep working.
    #[test]
    fn a_missing_constant_still_generates_bins() {
        let (def, cache) = def_and_cache(16);
        let bins = read_axis_bins(&def, Some(&cache), "notThere", 8, AxisHint::Rpm).unwrap();
        assert_eq!(bins.len(), 8);
        assert!(bins.windows(2).all(|w| w[1] > w[0]));
    }
}

#[cfg(test)]
mod target_afr_resolution_tests {
    use super::*;
    use libretune_core::ini::{DataType, TableDefinition};
    use libretune_core::tune::TuneCache;

    /// A definition holding one 2x2 AFR-target table, optionally declaring the
    /// `AfrTarget` role the way a real Speeduino INI does.
    fn def_with_afr_table(table_name: &str, role: TableRole) -> EcuDefinition {
        let mut def = EcuDefinition {
            page_sizes: vec![16],
            n_pages: 1,
            ..Default::default()
        };

        def.constants.insert(
            "afrTable".to_string(),
            Constant {
                name: "afrTable".to_string(),
                page: 0,
                offset: 0,
                data_type: DataType::U08,
                scale: 0.1,
                translate: 0.0,
                ..Default::default()
            },
        );
        def.tables.insert(
            table_name.to_string(),
            TableDefinition {
                name: table_name.to_string(),
                map: "afrTable".to_string(),
                x_size: 2,
                y_size: 2,
                role,
                ..Default::default()
            },
        );
        def
    }

    /// A VE table and an AFR target table on *different* rpm axes, the way a
    /// real Speeduino ships them.
    ///
    /// AFR rpm bins are 1000/4000 against the VE table's 1000/2000, and the
    /// target rises 12.0 -> 15.0 across them. Index-for-index the VE table's
    /// 2000 rpm cell therefore picks up the 4000 rpm target of 15.0, when the
    /// value actually asked for at 2000 rpm is 13.0.
    fn def_with_offset_axes() -> (EcuDefinition, TuneCache) {
        let mut def = EcuDefinition {
            page_sizes: vec![16],
            n_pages: 1,
            ..Default::default()
        };
        let mut konst = |name: &str, offset: u16, scale: f64| {
            def.constants.insert(
                name.to_string(),
                Constant {
                    name: name.to_string(),
                    page: 0,
                    offset,
                    data_type: DataType::U08,
                    scale,
                    translate: 0.0,
                    ..Default::default()
                },
            );
        };
        konst("afrTable", 0, 0.1); // 4 bytes
        konst("afrRpmBins", 4, 100.0); // 2 bytes
        konst("afrLoadBins", 6, 1.0);
        konst("veRpmBins", 8, 100.0);
        konst("veLoadBins", 10, 1.0);
        konst("veTable", 12, 1.0);

        // Key, `name` and `map` are distinct in a real INI: the table is keyed
        // and named `afrTable1Tbl` while its data constant is `afrTable`.
        let mut table = |name: &str, map: &str, x: &str, y: &str, role: TableRole| {
            def.tables.insert(
                name.to_string(),
                TableDefinition {
                    name: name.to_string(),
                    map: map.to_string(),
                    x_bins: x.to_string(),
                    y_bins: Some(y.to_string()),
                    x_size: 2,
                    y_size: 2,
                    role,
                    ..Default::default()
                },
            );
        };
        table(
            "afrTable1Tbl",
            "afrTable",
            "afrRpmBins",
            "afrLoadBins",
            TableRole::AfrTarget,
        );
        table(
            "veTable1Tbl",
            "veTable",
            "veRpmBins",
            "veLoadBins",
            TableRole::Ve,
        );

        let mut cache = TuneCache::from_definition(&def);
        cache.load_page(
            0,
            vec![
                120, 150, 120, 150, // afrTable: 12.0, 15.0 per row
                10, 40, // afrRpmBins: 1000, 4000
                20, 80, // afrLoadBins
                10, 20, // veRpmBins: 1000, 2000
                20, 80, // veLoadBins
                0, 0, 0, 0, // veTable z, unread here
            ],
        );
        (def, cache)
    }

    /// The target must be resampled onto the VE table's axes, not indexed by
    /// its cell numbers.
    ///
    /// Both tables are 2x2, so the shape check that used to guard this passes
    /// and the mismatch went through silently. On the real NA6 tables the error
    /// reached 0.52 AFR at 6000 rpm and always in the lean direction, because
    /// the AFR axis is compressed relative to the VE axis at the top end.
    #[test]
    fn a_target_on_different_axes_is_resampled_not_indexed() {
        let (def, cache) = def_with_offset_axes();
        let (tables, source) =
            resolve_reference_tables(&def, Some(&cache), "veTable1Tbl", None, None);

        assert!(
            matches!(source, TargetAfrSource::IniDeclared(_)),
            "got {source:?}"
        );
        let got = tables.target_afr_table;
        assert_eq!(got.len(), 2);
        // 2000 rpm sits a third of the way from 1000 to 4000: 12 + (15-12)/3.
        assert!(
            (got[0][1] - 13.0).abs() < 1e-6,
            "VE cell at 2000 rpm should target 13.0, got {} \
             (15.0 means it was indexed rather than resampled)",
            got[0][1]
        );
        assert!(
            (got[0][0] - 12.0).abs() < 1e-6,
            "1000 rpm is shared by both axes"
        );
    }

    /// Axes that already agree must be left exactly as they are - resampling a
    /// table onto its own axis should be a no-op, not a slow rounding error.
    #[test]
    fn a_target_on_matching_axes_is_untouched() {
        let (mut def, mut cache) = def_with_offset_axes();
        // Make the VE rpm axis identical to the AFR one.
        if let Some(c) = def.constants.get_mut("veRpmBins") {
            c.offset = 4;
        }
        cache.load_page(
            0,
            vec![
                120, 150, 120, 150, 10, 40, 20, 80, 10, 40, 20, 80, 0, 0, 0, 0,
            ],
        );
        let (tables, _) = resolve_reference_tables(&def, Some(&cache), "veTable1Tbl", None, None);
        assert_eq!(tables.target_afr_table[0], vec![12.0, 15.0]);
    }

    /// Page bytes for a 2x2 table of 13.0 AFR at scale 0.1 (raw 130).
    fn cache_with_rich_targets(def: &EcuDefinition) -> TuneCache {
        let mut cache = TuneCache::from_definition(def);
        cache.load_page(0, vec![130u8; 16]);
        cache
    }

    #[test]
    fn ini_declaration_is_consulted_for_the_real_speeduino_table_name() {
        // `afrTable1Tbl` is what every Speeduino calls it, and it was NOT in the
        // old candidate list ("afrTable1" was). Before the fix this resolved to
        // nothing and the session silently targeted a flat 14.7.
        let def = def_with_afr_table("afrTable1Tbl", TableRole::AfrTarget);
        assert_eq!(ini_declared_afr_target(&def), Some("afrTable1Tbl"));

        let cache = cache_with_rich_targets(&def);
        let (tables, source) =
            resolve_reference_tables(&def, Some(&cache), "veTable1Tbl", None, None);

        assert_eq!(
            source,
            TargetAfrSource::IniDeclared("afrTable1Tbl".to_string()),
            "auto-discovery must take the table the INI declares"
        );
        // The values must actually arrive, not just the name: a resolved-but-empty
        // table falls through to the flat target just as invisibly.
        assert_eq!(
            tables.target_afr_table,
            vec![vec![13.0, 13.0], vec![13.0, 13.0]]
        );
    }

    #[test]
    fn an_undeclared_ini_still_falls_back_to_name_matching() {
        // `afrTable` is a candidate name, so this resolves even with no role.
        let def = def_with_afr_table("afrTable", TableRole::Other);
        assert_eq!(ini_declared_afr_target(&def), None);

        let cache = cache_with_rich_targets(&def);
        let (tables, source) =
            resolve_reference_tables(&def, Some(&cache), "veTable1Tbl", None, None);
        assert_eq!(source, TargetAfrSource::NameMatch);
        assert!(!tables.target_afr_table.is_empty());
    }

    #[test]
    fn a_lambda_table_is_never_name_matched_as_an_afr_target() {
        // The old candidate list ended in "lambdaTable", so an undeclared lambda
        // INI could match a target of ~0.88 against a measured AFR of ~13.
        let def = def_with_afr_table("lambdaTable", TableRole::Other);
        let cache = cache_with_rich_targets(&def);
        let (tables, source) =
            resolve_reference_tables(&def, Some(&cache), "veTable1Tbl", None, None);

        assert_eq!(source, TargetAfrSource::FlatSetting);
        assert!(
            tables.target_afr_table.is_empty(),
            "a lambda table must not be picked up as an AFR target by name"
        );
    }

    #[test]
    fn a_declared_lambda_target_is_still_used_when_the_ini_says_so() {
        // On a real lambda INI the role points at the lambda table. Resolution
        // must accept it - but see the unit test below: the VALUES then need
        // normalising, because the measured side reports AFR either way. An
        // earlier version of this test asserted only that resolution happened
        // and so would have passed while the units were mismatched.
        let def = def_with_afr_table("lambdaTable1Tbl", TableRole::AfrTarget);
        let cache = cache_with_rich_targets(&def);
        let (tables, source) =
            resolve_reference_tables(&def, Some(&cache), "veTable1Tbl", None, None);
        assert_eq!(
            source,
            TargetAfrSource::IniDeclared("lambdaTable1Tbl".to_string())
        );
        assert!(!tables.target_afr_table.is_empty());
    }

    #[test]
    fn a_lambda_target_is_normalised_before_it_reaches_the_correction() {
        // The whole point: a lambda table holds ~0.88 while the measured value
        // arriving at the correction has already been converted to AFR (~13.0).
        // Divided un-normalised that is a 14.8x correction on every cell, every
        // pass - the authority ceiling, forever, re-anchored higher each session.
        use libretune_core::autotune::normalise_to_afr;

        let lambda_target = 0.88_f64;
        let normalised = normalise_to_afr(lambda_target);
        assert!(
            (normalised - 12.936).abs() < 1e-6,
            "0.88 lambda must become {} AFR, got {normalised}",
            0.88 * 14.7
        );

        // An AFR target must pass through untouched.
        assert_eq!(normalise_to_afr(12.7), 12.7);
        assert_eq!(normalise_to_afr(14.7), 14.7);

        // And the correction ratio must be sane rather than enormous.
        let measured = 13.0_f64;
        let ratio = measured / normalised;
        assert!(
            (0.9..1.1).contains(&ratio),
            "a lambda target of 0.88 against a measured 13.0 AFR should be a              near-unity correction, got {ratio}x"
        );
    }

    #[test]
    fn an_explicit_name_outranks_the_ini_declaration() {
        let def = def_with_afr_table("afrTable1Tbl", TableRole::AfrTarget);
        let cache = cache_with_rich_targets(&def);
        let (_, source) = resolve_reference_tables(
            &def,
            Some(&cache),
            "veTable1Tbl",
            Some("afrTable1Tbl"),
            None,
        );
        assert_eq!(
            source,
            TargetAfrSource::Explicit("afrTable1Tbl".to_string())
        );
    }

    /// Give the definition a VE table so `resolve_reference_tables` can compare
    /// shapes. `rows`/`cols` are the VE table's, which the target must match.
    fn with_ve_table(mut def: EcuDefinition, rows: usize, cols: usize) -> EcuDefinition {
        def.constants.insert(
            "veTable".to_string(),
            Constant {
                name: "veTable".to_string(),
                page: 0,
                offset: 0,
                data_type: DataType::U08,
                scale: 1.0,
                translate: 0.0,
                ..Default::default()
            },
        );
        def.tables.insert(
            "veTable1Tbl".to_string(),
            TableDefinition {
                name: "veTable1Tbl".to_string(),
                map: "veTable".to_string(),
                x_size: cols,
                y_size: rows,
                role: TableRole::Ve,
                ..Default::default()
            },
        );
        def
    }

    #[test]
    fn a_target_table_of_a_different_shape_is_refused() {
        // `resolve_target_afr` indexes the target with the VE table's own
        // (x, y). A smaller target returns None for the rows and columns past
        // its end - the high-load, high-rpm corner - and those cells silently
        // revert to the flat target. Half-applying is worse than not applying.
        let def = def_with_afr_table("afrTable1Tbl", TableRole::AfrTarget);
        let def = with_ve_table(def, 4, 4); // VE is 4x4, the target is 2x2
        let cache = cache_with_rich_targets(&def);

        let (tables, source) =
            resolve_reference_tables(&def, Some(&cache), "veTable1Tbl", None, None);
        assert!(
            tables.target_afr_table.is_empty(),
            "a 2x2 target must not be applied to a 4x4 VE table"
        );
        assert_eq!(source, TargetAfrSource::FlatSetting, "and it must say so");
    }

    #[test]
    fn a_matching_shape_is_accepted() {
        let def = def_with_afr_table("afrTable1Tbl", TableRole::AfrTarget);
        let def = with_ve_table(def, 2, 2); // same shape as the target
        let cache = cache_with_rich_targets(&def);

        let (tables, source) =
            resolve_reference_tables(&def, Some(&cache), "veTable1Tbl", None, None);
        assert_eq!(tables.target_afr_table.len(), 2);
        assert_eq!(
            source,
            TargetAfrSource::IniDeclared("afrTable1Tbl".to_string())
        );
    }

    #[test]
    fn a_short_page_yields_no_table_rather_than_zeros() {
        // Padding a short read with 0.0 produced a table that reported as
        // resolved with a zero tail; `resolve_target_afr` ignores values <= 0.1,
        // so exactly those cells fell back to the flat target invisibly.
        let def = def_with_afr_table("afrTable1Tbl", TableRole::AfrTarget);
        let mut cache = TuneCache::from_definition(&def);
        cache.load_page(0, vec![130u8; 2]); // 2 bytes for a 2x2 table

        let z = read_table_z_values(&def, Some(&cache), "afrTable", 2, 2);
        assert!(
            z.is_none(),
            "a partial read must fail, not return a zero-padded table: {z:?}"
        );

        let (tables, source) =
            resolve_reference_tables(&def, Some(&cache), "veTable1Tbl", None, None);
        assert!(tables.target_afr_table.is_empty());
        assert_eq!(source, TargetAfrSource::FlatSetting);
    }

    #[test]
    fn a_full_page_still_reads_every_cell() {
        // The padding fix must not make a healthy read fail.
        let def = def_with_afr_table("afrTable1Tbl", TableRole::AfrTarget);
        let cache = cache_with_rich_targets(&def);
        let z = read_table_z_values(&def, Some(&cache), "afrTable", 2, 2).expect("full page reads");
        assert_eq!(z, vec![vec![13.0, 13.0], vec![13.0, 13.0]]);
    }

    /// `infer_table_roles` stamps AfrTarget on several tables, and `def.tables`
    /// is a HashMap. Without a deterministic rule the session can tune against
    /// a different table each launch, silently.
    #[test]
    fn several_role_claimants_resolve_the_same_way_every_time() {
        let mut def = def_with_afr_table("afrTable1Tbl", TableRole::AfrTarget);
        // A second claimant, as a real Speeduino INI produces.
        def.tables.insert(
            "afrTSCustom".to_string(),
            TableDefinition {
                name: "afrTSCustom".to_string(),
                map: "afrTable".to_string(),
                x_size: 2,
                y_size: 2,
                role: TableRole::AfrTarget,
                ..Default::default()
            },
        );
        // Repeat: a HashMap scan would eventually disagree with itself.
        let picks: std::collections::HashSet<_> = (0..50)
            .map(|_| ini_declared_afr_target(&def).unwrap().to_string())
            .collect();
        assert_eq!(
            picks.len(),
            1,
            "must pick the same table every time, got {picks:?}"
        );
    }

    /// When the INI names a primary, that outranks any role scan.
    #[test]
    fn the_ve_analyze_declaration_outranks_the_role_scan() {
        use libretune_core::ini::VeAnalyzeConfig;
        let mut def = def_with_afr_table("afrTSCustom", TableRole::AfrTarget);
        def.tables.insert(
            "afrTable1Tbl".to_string(),
            TableDefinition {
                name: "afrTable1Tbl".to_string(),
                map: "afrTable".to_string(),
                x_size: 2,
                y_size: 2,
                role: TableRole::AfrTarget,
                ..Default::default()
            },
        );
        def.ve_analyze = Some(VeAnalyzeConfig {
            ve_table_name: "veTable1Tbl".to_string(),
            target_table_name: "afrTable1Tbl".to_string(),
            ..Default::default()
        });
        assert_eq!(
            ini_declared_afr_target(&def),
            Some("afrTable1Tbl"),
            "the INI's own [VeAnalyze] naming must win"
        );
    }

    #[test]
    fn an_unresolved_target_says_so_loudly() {
        // The failure that bit a real car: no table, every cell at 14.7, and
        // nothing on screen to say so. The message must name the number.
        let def = EcuDefinition::default();
        let (tables, source) = resolve_reference_tables(&def, None, "veTable1Tbl", None, None);
        assert_eq!(source, TargetAfrSource::FlatSetting);
        assert!(tables.target_afr_table.is_empty());

        let msg = source.describe(14.7);
        assert!(msg.contains("14.7"), "must name the flat target: {msg}");
        assert!(
            msg.contains("NO AFR target table"),
            "must be unmistakable, got: {msg}"
        );
        assert_eq!(source.table_name(), None);
    }
}
