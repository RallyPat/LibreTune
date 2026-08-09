//! INI metadata commands (tables, curves, frontpage, gauges).

use crate::commands::string_context::{build_string_context, numeric_context_from_tune};
use crate::state::AppState;
use libretune_core::ini::expression::evaluate_display_string;
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct TableInfo {
    pub name: String,
    pub title: String,
}

#[derive(Serialize)]
pub(crate) struct CurveInfo {
    pub name: String,
    pub title: String,
}

/// Lists all available tables from the loaded INI definition.
///
/// Returns basic info (name and title) for all tables defined in the INI.
/// Used to populate menus and table selection UI.
///
/// Returns: Sorted vector of TableInfo with name and title
#[tauri::command]
pub async fn get_tables(state: tauri::State<'_, AppState>) -> Result<Vec<TableInfo>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let mut tables: Vec<TableInfo> = def
        .tables
        .values()
        .map(|t| TableInfo {
            name: t.name.clone(),
            title: t.title.clone(),
        })
        .collect();
    tables.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(tables)
}

/// Lists all available curves from the loaded INI definition.
///
/// Returns basic info (name and title) for all curves defined in the INI.
/// Used to populate sidebar curve list and search UI.
///
/// Returns: Sorted vector of CurveInfo with name and title
#[tauri::command]
pub async fn get_curves(state: tauri::State<'_, AppState>) -> Result<Vec<CurveInfo>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let mut curves: Vec<CurveInfo> = def
        .curves
        .values()
        .map(|c| CurveInfo {
            name: c.name.clone(),
            title: c.title.clone(),
        })
        .collect();
    curves.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(curves)
}

/// Gauge configuration info returned to frontend
#[derive(Serialize)]
pub(crate) struct GaugeInfo {
    pub name: String,
    pub channel: String,
    pub title: String,
    pub units: String,
    pub lo: f64,
    pub hi: f64,
    pub low_warning: f64,
    pub high_warning: f64,
    pub low_danger: f64,
    pub high_danger: f64,
    pub digits: u8,
}

/// FrontPage indicator info returned to frontend
#[derive(Serialize)]
pub(crate) struct FrontPageIndicatorInfo {
    pub expression: String,
    pub label_off: String,
    pub label_on: String,
    pub bg_off: String,
    pub fg_off: String,
    pub bg_on: String,
    pub fg_on: String,
}

/// FrontPage configuration info returned to frontend
#[derive(Serialize)]
pub(crate) struct FrontPageInfo {
    /// Gauge names for gauge1-gauge8 (references to [GaugeConfigurations])
    pub gauges: Vec<String>,
    /// Status indicators
    pub indicators: Vec<FrontPageIndicatorInfo>,
}

/// Get the FrontPage definition from the INI file.
///
/// FrontPage defines the default dashboard layout including which gauges
/// and status indicators to show when the app first loads.
///
/// Returns: Optional FrontPageInfo with gauge references and indicators
#[tauri::command]
pub async fn get_frontpage(
    state: tauri::State<'_, AppState>,
) -> Result<Option<FrontPageInfo>, String> {
    let string_ctx = build_string_context(&state).await;
    let numeric = {
        let tune = state.current_tune.lock().await;
        numeric_context_from_tune(tune.as_ref())
    };

    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    Ok(def.frontpage.as_ref().map(|fp| FrontPageInfo {
        gauges: fp.gauges.clone(),
        indicators: fp
            .indicators
            .iter()
            .map(|ind| FrontPageIndicatorInfo {
                expression: ind.expression.clone(),
                label_off: evaluate_display_string(&ind.label_off, &numeric, Some(&string_ctx)),
                label_on: evaluate_display_string(&ind.label_on, &numeric, Some(&string_ctx)),
                bg_off: libretune_core::ini::FrontPageIndicator::color_to_css(&ind.bg_off),
                fg_off: libretune_core::ini::FrontPageIndicator::color_to_css(&ind.fg_off),
                bg_on: libretune_core::ini::FrontPageIndicator::color_to_css(&ind.bg_on),
                fg_on: libretune_core::ini::FrontPageIndicator::color_to_css(&ind.fg_on),
            })
            .collect(),
    }))
}

/// Get all gauge configurations from the INI file.
///
/// Returns complete gauge definitions including channel bindings,
/// min/max ranges, warning thresholds, and display settings.
/// Used to configure dashboard gauges.
///
/// Returns: Vector of GaugeInfo for all defined gauges
/// Resolve one gauge numeric field: literal value, or its `{expression}`
/// evaluated against the current tune/default values. An expression that
/// cannot be resolved yields NaN (serialized as `null`), NOT the parser's
/// placeholder fallback — a bogus 100 here is what pegged RPM gauges at 100
/// after a range sync. The frontend treats non-finite as "keep the
/// dashboard's own value".
fn resolve_gauge_field(
    literal: f64,
    expr: Option<&str>,
    ctx: &std::collections::HashMap<String, f64>,
) -> f64 {
    let Some(expr) = expr else {
        return literal;
    };
    let parsed = match libretune_core::ini::expression::Parser::new(expr).parse() {
        Ok(p) => p,
        Err(_) => return f64::NAN,
    };
    match libretune_core::ini::expression::evaluate(&parsed, ctx, None) {
        Ok(v) => {
            let f = v.as_f64();
            if f.is_finite() {
                f
            } else {
                f64::NAN
            }
        }
        Err(_) => f64::NAN,
    }
}

/// Build the expression context for gauge ranges: scalar constant values from
/// the tune/cache, plus INI `defaultValue` entries (PcVariables like
/// `rpmhigh` live there) filling any gaps.
fn gauge_expr_context(
    def: &libretune_core::ini::EcuDefinition,
    tune: Option<&libretune_core::tune::TuneFile>,
    cache: Option<&libretune_core::tune::TuneCache>,
) -> std::collections::HashMap<String, f64> {
    let mut ctx = super::constant_values::collect_scalar_constant_values(def, tune, cache);
    for (name, value) in &def.default_values {
        ctx.entry(name.clone()).or_insert(*value);
    }
    ctx
}

fn gauge_to_info(
    g: &libretune_core::ini::GaugeConfig,
    ctx: &std::collections::HashMap<String, f64>,
    string_ctx: &libretune_core::ini::expression::StringContext,
) -> GaugeInfo {
    GaugeInfo {
        name: g.name.clone(),
        channel: g.channel.clone(),
        title: evaluate_display_string(&g.title, ctx, Some(string_ctx)),
        units: evaluate_display_string(&g.units, ctx, Some(string_ctx)),
        lo: resolve_gauge_field(g.lo, g.lo_expr.as_deref(), ctx),
        hi: resolve_gauge_field(g.hi, g.hi_expr.as_deref(), ctx),
        low_warning: resolve_gauge_field(g.low_warning, g.low_warning_expr.as_deref(), ctx),
        high_warning: resolve_gauge_field(g.high_warning, g.high_warning_expr.as_deref(), ctx),
        low_danger: resolve_gauge_field(g.low_danger, g.low_danger_expr.as_deref(), ctx),
        high_danger: resolve_gauge_field(g.high_danger, g.high_danger_expr.as_deref(), ctx),
        digits: g.digits,
    }
}

#[tauri::command]
pub async fn get_gauge_configs(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<GaugeInfo>, String> {
    let string_ctx = build_string_context(&state).await;

    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    // Same lock order as get_all_constant_values: definition → cache → tune.
    let cache_guard = state.tune_cache.lock().await;
    let tune_guard = state.current_tune.lock().await;
    let ctx = gauge_expr_context(def, tune_guard.as_ref(), cache_guard.as_ref());

    let gauges: Vec<GaugeInfo> = def
        .gauges
        .values()
        .map(|g| gauge_to_info(g, &ctx, &string_ctx))
        .collect();
    Ok(gauges)
}

/// Get a single gauge configuration by name
#[tauri::command]
pub async fn get_gauge_config(
    state: tauri::State<'_, AppState>,
    gauge_name: String,
) -> Result<GaugeInfo, String> {
    let string_ctx = build_string_context(&state).await;

    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let gauge = def
        .gauges
        .get(&gauge_name)
        .ok_or_else(|| format!("Gauge {} not found", gauge_name))?;

    // Same lock order as get_all_constant_values: definition → cache → tune.
    let cache_guard = state.tune_cache.lock().await;
    let tune_guard = state.current_tune.lock().await;
    let ctx = gauge_expr_context(def, tune_guard.as_ref(), cache_guard.as_ref());

    Ok(gauge_to_info(gauge, &ctx, &string_ctx))
}
