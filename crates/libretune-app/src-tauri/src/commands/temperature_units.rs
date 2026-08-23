//! Asking which temperature units an INI should use, once, and remembering it.
//!
//! An INI selects Celsius through `#if CELSIUS`, and the tuning software that
//! created the project defines that symbol from its `ecuSettings`. A project carrying no such
//! declaration silently takes the `#else` arm, so every temperature renders in
//! Fahrenheit while wearing the INI's generic "TEMP" label — a 15 °C morning
//! reads 59 and nothing says why.
//!
//! It is worse than cosmetic. AutoTune's `min_clt` filter is compared against
//! that same channel, so a project that quietly decided it was Fahrenheit will
//! reject or accept entire sessions on a threshold meaning the wrong thing.
//!
//! So: ask once, when there is a real question to ask, and write the answer
//! where both LibreTune and other tuning software will find it.

use crate::commands::app_settings::seed_symbols_from_project;
use crate::AppState;
use libretune_core::project::Properties;

/// Whether the user needs to be asked about temperature units, and why not.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitsStatus {
    /// Show the picker. Only true when the question is real and unanswered.
    pub needs_choice: bool,
    /// What the INI is currently parsed as.
    pub celsius: bool,
    /// Where that came from: `project`, `preference`, or `default`.
    pub source: String,
    /// Does this INI test `CELSIUS` at all? An INI that never asks has no
    /// answer worth storing.
    pub ini_uses_celsius: bool,
}

/// Report whether this project has decided its temperature units.
#[tauri::command]
pub async fn get_temperature_units_status(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<UnitsStatus, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let ini_uses_celsius = def.tests_symbol("CELSIUS");
    let celsius = def.symbol_is_active("CELSIUS");
    drop(def_guard);

    let declared = {
        let proj_guard = state.current_project.lock().await;
        proj_guard
            .as_ref()
            .map(|p| p.ini_path())
            .and_then(|ini| ini.parent().map(|d| d.join("project.properties")))
            .filter(|p| p.exists())
            .and_then(|p| Properties::load(&p).ok())
            .and_then(|props| props.get("ecuSettings").cloned())
            .is_some()
    };

    // A units preference the user has actually set is an answer, even though
    // it is not the project's. Prompting over it would be nagging about a
    // question already decided.
    let preference_set = !crate::commands::app_settings::load_settings(&app)
        .units_system
        .trim()
        .is_empty();

    let source = if declared {
        "project"
    } else if preference_set {
        "preference"
    } else {
        // Nothing declared and no preference: the empty default resolves to
        // Fahrenheit without ever saying so. This is the case worth asking about.
        "default"
    };

    Ok(UnitsStatus {
        // Only ask when the INI actually branches on it AND nothing has
        // answered. An INI with no `#if CELSIUS` has no question to put, and a
        // user who has set a preference has already put theirs.
        needs_choice: ini_uses_celsius && !declared && !preference_set,
        celsius,
        source: source.to_string(),
        ini_uses_celsius,
    })
}

/// Record the choice in the project, where it will outlive this session.
///
/// Written to `projectCfg/project.properties` as `ecuSettings`, the same key
/// and format other popular tuning software uses, so the tools agree rather than
/// each keeping a private opinion. An existing `ecuSettings` has only its `CELSIUS` token
/// adjusted — the other tokens belong to whoever wrote them.
///
/// Symbols are resolved while the INI is parsed, so this cannot change the
/// definition already in memory. `reload_required` says so plainly rather than
/// letting the caller assume it took effect.
#[tauri::command]
pub async fn set_temperature_units(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    celsius: bool,
) -> Result<ReloadHint, String> {
    let proj_guard = state.current_project.lock().await;
    let project = proj_guard.as_ref().ok_or("No project open")?;
    let ini = project.ini_path();
    let dir = ini
        .parent()
        .ok_or("Project INI has no parent directory")?
        .to_path_buf();
    let props_path = dir.join("project.properties");
    drop(proj_guard);

    let mut props = if props_path.exists() {
        Properties::load(&props_path).map_err(|e| format!("read {props_path:?}: {e}"))?
    } else {
        Properties::new()
    };

    // Preserve every other token; only CELSIUS is ours to decide.
    let mut tokens: Vec<String> = props
        .get("ecuSettings")
        .map(|raw| {
            raw.split('|')
                .map(str::trim)
                .filter(|t| !t.is_empty() && !t.eq_ignore_ascii_case("CELSIUS"))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if celsius {
        tokens.push("CELSIUS".to_string());
    }
    // Trailing separator matches the conventional formatting.
    let value = format!("{}|", tokens.join("|"));
    props.set("ecuSettings".to_string(), value.clone());
    props
        .save(&props_path)
        .map_err(|e| format!("write {props_path:?}: {e}"))?;

    // Seed the parser for every subsequent parse. The definition currently in
    // memory keeps whatever it was parsed with.
    let settings = crate::commands::app_settings::load_settings(&app);
    seed_symbols_from_project(&ini, &settings);

    tracing::info!(
        celsius,
        ecu_settings = %value,
        path = ?props_path,
        "temperature units recorded in the project"
    );

    Ok(ReloadHint {
        saved_to: props_path.to_string_lossy().to_string(),
        ecu_settings: value,
        reload_required: true,
    })
}

/// What was written, and the fact that it will not apply until a re-parse.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReloadHint {
    pub saved_to: String,
    pub ecu_settings: String,
    /// Symbols are resolved during parsing, so the open definition is unchanged
    /// until the project is reopened.
    pub reload_required: bool,
}
