//! Pre-start checks, surfaced to the user before a session is wasted.

use crate::commands::start_autotune::{read_table_z_values, resolve_reference_tables};
use crate::AppState;
use libretune_core::autotune::preflight::{check, has_blocker, Finding, PreflightInput};
use libretune_core::autotune::{AutoTuneAuthorityLimits, AutoTuneFilters, AutoTuneSettings};
use libretune_core::ini::TableRole;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightReport {
    pub findings: Vec<Finding>,
    /// Delay model fitted to this session's own measurements, when there are
    /// enough of them. The UI offers it as the delay setting.
    pub delay_fit: Option<libretune_core::autotune::delay_measure::FlowDelayFit>,
    pub has_blocker: bool,
    /// Tables that could serve as an AFR target, so the UI can offer a choice
    /// rather than only a complaint.
    pub candidate_target_tables: Vec<String>,
    /// The table that resolved, if any.
    pub resolved_target_table: Option<String>,
}

/// Check a session before starting it.
///
/// Takes the same arguments `start_autotune` will, so what is checked is what
/// will run - a preflight against different values would be worse than none.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn preflight_autotune(
    state: tauri::State<'_, AppState>,
    table_name: String,
    settings: AutoTuneSettings,
    filters: AutoTuneFilters,
    authority_limits: AutoTuneAuthorityLimits,
    target_afr_table_name: Option<String>,
    lambda_delay_table_name: Option<String>,
    will_write_to_ecu: Option<bool>,
) -> Result<PreflightReport, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let cache_guard = state.tune_cache.lock().await;
    let cache = cache_guard.as_ref();

    let (tables, source) = resolve_reference_tables(
        def,
        cache,
        &table_name,
        target_afr_table_name.as_deref(),
        lambda_delay_table_name.as_deref(),
    );

    let ve_shape = def
        .get_table_by_name_or_map(&table_name)
        .map(|t| (t.y_size, t.x_size))
        .unwrap_or((0, 0));
    let ve_values = def
        .get_table_by_name_or_map(&table_name)
        .and_then(|t| read_table_z_values(def, cache, t.map.as_str(), t.x_size, t.y_size))
        .unwrap_or_default();

    // Anything the INI marks as an AFR target, plus anything named like one -
    // the user may need to pick when the INI declares nothing.
    let mut candidates: Vec<String> = def
        .tables
        .values()
        .filter(|t| {
            t.role == TableRole::AfrTarget
                || t.name.to_lowercase().contains("afr")
                || t.name.to_lowercase().contains("lambda")
        })
        .map(|t| t.name.clone())
        .collect();
    candidates.sort();
    candidates.dedup();

    let celsius = def.symbol_is_active("CELSIUS");

    let input = PreflightInput {
        target_table: source.table_name(),
        target_values: &tables.target_afr_table,
        ve_shape,
        ve_values: &ve_values,
        celsius,
        candidate_target_tables: candidates.clone(),
        will_write_to_ecu: will_write_to_ecu.unwrap_or(false),
    };

    let mut findings = check(&input, &settings, &filters, &authority_limits);

    // Turn the generic "no delay set" warning into a number, when this session
    // has actually measured one. A recommendation drawn from the user's own
    // exhaust beats any default, and the model is what makes a handful of
    // scattered cells usable: it fits all of them at once instead of trusting
    // whichever cell they happen to be tuning in.
    // Live samples first; a model saved from previous sessions otherwise. The
    // delay is a property of the exhaust, not of today - once measured well it
    // should not have to be measured again, and requiring a fresh run every
    // session is why the setting sat at its useless default for so long.
    let live = libretune_core::autotune::delay_measure::fit_flow_delay(
        &crate::commands::afr_delay_test::delay_samples_snapshot(),
    );
    let from_live = live.is_some();
    let fit = live.or_else(|| {
        let path = {
            let proj = state.current_project.try_lock().ok()?;
            proj.as_ref()?.path.join("afr_delay_model.json")
        };
        let text = std::fs::read_to_string(&path).ok()?;
        match serde_json::from_str::<libretune_core::autotune::delay_measure::FlowDelayFit>(&text) {
            Ok(f) => {
                tracing::info!(?path, "delay model loaded from the project");
                Some(f)
            }
            Err(e) => {
                tracing::warn!(?path, error = %e, "saved delay model could not be read");
                None
            }
        }
    });
    if let Some(f) = &fit {
        if let Some(entry) = findings
            .iter_mut()
            .find(|x| x.code == "delay_default_curve")
        {
            // Say where the model came from. "This session measured N" is
            // false for a model read off disk, and a preflight that misreports
            // its own evidence is worse than one that says nothing.
            let provenance = if from_live {
                format!("Measured {} samples this session", f.samples)
            } else {
                format!(
                    "Using the delay model saved with this project ({} measurements,                      no test run needed today)",
                    f.samples
                )
            };
            entry.detail = format!(
                "{provenance}. Fitted to flow (delay = floor + k/(rpm x load)): a floor                  of {:.0} ms and {:.0} ms at the idle anchor, RMS residual {:.0} ms.                  The built-in curve assumes 200 ms falling to 50 ms, far short of that,                  and credits each reading to a cell already left behind.",
                f.floor_ms, f.anchor_ms, f.rms_ms
            );
            entry.suggested = Some(format!(
                "flow-scaled, floor {:.0} ms, anchor {:.0} ms",
                f.floor_ms, f.anchor_ms
            ));
        }
    }
    tracing::info!(
        blockers = findings
            .iter()
            .filter(|f| f.severity == libretune_core::autotune::preflight::Severity::Blocker)
            .count(),
        total = findings.len(),
        target = source.table_name().unwrap_or("-"),
        "preflight_autotune"
    );

    Ok(PreflightReport {
        delay_fit: fit,
        has_blocker: has_blocker(&findings),
        findings,
        candidate_target_tables: candidates,
        resolved_target_table: source.table_name().map(str::to_string),
    })
}

/// Save the delay model fitted from this session's measurements into the
/// project, so later sessions inherit it without re-running the test.
///
/// The delay is a property of the exhaust and the sensor's position in it. It
/// does not change between drives, and asking for it to be re-measured every
/// session is why the setting sat at a useless default: the cost of getting it
/// right was paid over and over, so it was not paid at all.
#[tauri::command]
pub async fn save_delay_model(
    state: tauri::State<'_, AppState>,
    model: Option<libretune_core::autotune::delay_measure::FlowDelayFit>,
) -> Result<String, String> {
    let fit = match model {
        Some(m) => m,
        None => libretune_core::autotune::delay_measure::fit_flow_delay(
            &crate::commands::afr_delay_test::delay_samples_snapshot(),
        )
        .ok_or("No delay measurements this session, and no model supplied")?,
    };

    let path = {
        let proj = state.current_project.lock().await;
        proj.as_ref()
            .ok_or("No project open")?
            .path
            .join("afr_delay_model.json")
    };
    let text = serde_json::to_string_pretty(&fit).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("write {path:?}: {e}"))?;
    tracing::info!(
        ?path,
        floor = fit.floor_ms,
        anchor = fit.anchor_ms,
        "delay model saved"
    );
    Ok(path.to_string_lossy().to_string())
}
