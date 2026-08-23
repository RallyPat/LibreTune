//! The filters an INI declares for VE Analyze, and how the session compares.
//!
//! `[VeAnalyze]` states its own filters, already resolved for the project's unit
//! system by the preprocessor:
//!
//! ```text
//! #if CELSIUS
//!      filter = minCltFilter, "Minimum CLT", coolant, <, 71,  , true
//! #else
//!      filter = minCltFilter, "Minimum CLT", coolant, <, 160, , true
//! #endif
//!      filter = accelFilter, "Accel Flag", engine, &, 16, , false
//!      filter = overrunFilter, "Overrun", pulseWidth, =, 0, , false
//! ```
//!
//! LibreTune ignored all of it and used hardcoded defaults — 160 in the Rust
//! struct, 60 in the UI — which is how a Celsius project ended up with a
//! coolant threshold above boiling that rejected every sample of a session.
//! The INI had the right number the whole time.
//!
//! Note the operators: `<` and `>` and `=` are what you would expect, but `&`
//! is a **bitmask test** against a status byte, which is how the accel and ASE
//! flags are read. A filter engine that treats `&` as equality silently accepts
//! every sample those filters exist to reject.

use crate::AppState;
use libretune_core::ini::{AnalysisFilter, FilterOperator};
use serde::Serialize;

/// One declared filter, plus what the session is currently doing about it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeclaredFilter {
    /// Identifier from the INI, e.g. `minCltFilter`.
    pub name: String,
    /// Label the INI gives it, for the UI.
    pub display_name: String,
    /// Output channel it tests.
    pub channel: String,
    /// `<`, `>`, `=` or `&` (bitmask).
    pub operator: String,
    /// The value the INI declares — already unit-correct.
    pub ini_value: f64,
    /// Whether the INI lets the user change it.
    pub user_adjustable: bool,
    /// What this session is using, where the setting maps onto one of ours.
    pub session_value: Option<f64>,
    /// True when the two disagree and the INI's should probably win.
    pub differs: bool,
}

fn op_str(op: &FilterOperator) -> &'static str {
    match op {
        FilterOperator::LessThan => "<",
        FilterOperator::GreaterThan => ">",
        FilterOperator::Equal => "=",
        FilterOperator::BitwiseAnd => "&",
        _ => "?",
    }
}

/// Map an INI filter onto the session setting that implements it, if any.
///
/// Deliberately narrow: only filters LibreTune actually applies are paired up.
/// Claiming a pairing that does not exist would report agreement where the
/// filter is in truth being ignored.
fn session_value_for(
    name: &str,
    filters: &libretune_core::autotune::AutoTuneFilters,
) -> Option<f64> {
    match name {
        "minCltFilter" => Some(filters.min_clt),
        "minRPMFilter" => Some(filters.min_rpm),
        _ => None,
    }
}

/// List the INI's declared VE Analyze filters against the session's settings.
#[tauri::command]
pub async fn get_declared_analyze_filters(
    state: tauri::State<'_, AppState>,
    filters: Option<libretune_core::autotune::AutoTuneFilters>,
) -> Result<Vec<DeclaredFilter>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let cfg = def
        .ve_analyze
        .as_ref()
        .ok_or("This INI declares no [VeAnalyze] section")?;

    let session = filters.unwrap_or_default();
    let out = cfg
        .filters
        .iter()
        .map(|f: &AnalysisFilter| {
            let session_value = session_value_for(&f.name, &session);
            DeclaredFilter {
                name: f.name.clone(),
                display_name: if f.display_name.is_empty() {
                    f.name.clone()
                } else {
                    f.display_name.clone()
                },
                channel: f.channel.clone(),
                operator: op_str(&f.operator).to_string(),
                ini_value: f.default_value,
                user_adjustable: f.user_adjustable,
                session_value,
                differs: session_value
                    .map(|v| (v - f.default_value).abs() > 0.5)
                    .unwrap_or(false),
            }
        })
        .collect();
    Ok(out)
}
