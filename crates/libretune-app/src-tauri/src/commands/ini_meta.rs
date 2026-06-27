//! INI metadata commands (tables, curves, frontpage, gauges).

use crate::state::AppState;
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
    let def_guard = state.definition.read().await;
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
    let def_guard = state.definition.read().await;
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
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    Ok(def.frontpage.as_ref().map(|fp| FrontPageInfo {
        gauges: fp.gauges.clone(),
        indicators: fp
            .indicators
            .iter()
            .map(|ind| FrontPageIndicatorInfo {
                expression: ind.expression.clone(),
                label_off: ind.label_off.clone(),
                label_on: ind.label_on.clone(),
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
#[tauri::command]
pub async fn get_gauge_configs(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<GaugeInfo>, String> {
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let gauges: Vec<GaugeInfo> = def
        .gauges
        .values()
        .map(|g| GaugeInfo {
            name: g.name.clone(),
            channel: g.channel.clone(),
            title: g.title.clone(),
            units: g.units.clone(),
            lo: g.lo,
            hi: g.hi,
            low_warning: g.low_warning,
            high_warning: g.high_warning,
            low_danger: g.low_danger,
            high_danger: g.high_danger,
            digits: g.digits,
        })
        .collect();
    Ok(gauges)
}

/// Get a single gauge configuration by name
#[tauri::command]
pub async fn get_gauge_config(
    state: tauri::State<'_, AppState>,
    gauge_name: String,
) -> Result<GaugeInfo, String> {
    let def_guard = state.definition.read().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let gauge = def
        .gauges
        .get(&gauge_name)
        .ok_or_else(|| format!("Gauge {} not found", gauge_name))?;

    Ok(GaugeInfo {
        name: gauge.name.clone(),
        channel: gauge.channel.clone(),
        title: gauge.title.clone(),
        units: gauge.units.clone(),
        lo: gauge.lo,
        hi: gauge.hi,
        low_warning: gauge.low_warning,
        high_warning: gauge.high_warning,
        low_danger: gauge.low_danger,
        high_danger: gauge.high_danger,
        digits: gauge.digits,
    })
}
