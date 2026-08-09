//! Build a snapshot-based [`StringContext`] for INI expression evaluation.

use crate::state::AppState;
use libretune_core::ini::expression::StringContext;
use libretune_core::ini::{DataType, EcuDefinition, IncTableCache};
use libretune_core::project::Project;
use libretune_core::protocol::ConnectionState;
use libretune_core::tune::{TuneFile, TuneValue};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Refresh `.inc` search paths from the current project + definitions dirs.
pub fn refresh_inc_table_paths(
    cache: &Mutex<IncTableCache>,
    project_path: Option<&PathBuf>,
    definitions_dir: Option<PathBuf>,
) {
    let Ok(mut cache) = cache.lock() else {
        return;
    };
    cache.clear();
    if let Some(path) = project_path {
        cache.add_search_path(path.clone());
    }
    if let Some(dir) = definitions_dir {
        cache.add_search_path(dir);
    }
}

/// Build a numeric evaluation context from tune scalar/bool constants.
pub fn numeric_context_from_tune(tune: Option<&TuneFile>) -> HashMap<String, f64> {
    let mut context = HashMap::new();
    let Some(tune) = tune else {
        return context;
    };
    for (name, value) in &tune.constants {
        match value {
            TuneValue::Scalar(n) => {
                context.insert(name.clone(), *n);
            }
            TuneValue::Bool(b) => {
                context.insert(name.clone(), if *b { 1.0 } else { 0.0 });
            }
            TuneValue::Array(arr) if !arr.is_empty() => {
                // Index expressions often reference the constant name for the first bin.
                context.insert(name.clone(), arr[0]);
            }
            _ => {}
        }
    }
    for (name, value) in &tune.pc_variables {
        match value {
            TuneValue::Scalar(n) => {
                context.insert(name.clone(), *n);
            }
            TuneValue::Bool(b) => {
                context.insert(name.clone(), if *b { 1.0 } else { 0.0 });
            }
            _ => {}
        }
    }
    context
}

fn string_map_from_tune(
    tune: Option<&TuneFile>,
    def: Option<&EcuDefinition>,
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Some(tune) = tune else {
        return map;
    };
    for (name, value) in &tune.constants {
        if let TuneValue::String(s) = value {
            let is_string_const = def
                .and_then(|d| d.constants.get(name))
                .map(|c| c.data_type == DataType::String)
                .unwrap_or(false);
            if is_string_const {
                map.insert(name.clone(), s.clone());
            }
        }
    }
    map
}

fn bit_options_map(def: Option<&EcuDefinition>) -> HashMap<String, Vec<String>> {
    let mut map = HashMap::new();
    let Some(def) = def else {
        return map;
    };
    for (name, constant) in &def.constants {
        if !constant.bit_options.is_empty() {
            map.insert(name.clone(), constant.bit_options.clone());
        }
    }
    map
}

fn array_map_from_tune(tune: Option<&TuneFile>) -> HashMap<String, Vec<f64>> {
    let mut map = HashMap::new();
    let Some(tune) = tune else {
        return map;
    };
    for (name, value) in &tune.constants {
        if let TuneValue::Array(arr) = value {
            map.insert(name.clone(), arr.clone());
        }
    }
    map
}

fn interpolate_array(values: &[f64], index: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    if index <= 0.0 {
        return Some(values[0]);
    }
    let max_idx = (values.len() - 1) as f64;
    if index >= max_idx {
        return Some(values[values.len() - 1]);
    }
    let lo = index.floor() as usize;
    let hi = lo + 1;
    let t = index - lo as f64;
    Some(values[lo] + t * (values[hi] - values[lo]))
}

/// Snapshot AppState into a [`StringContext`] (closures hold owned data / Arcs).
pub async fn build_string_context(state: &AppState) -> StringContext {
    let def_guard = state.definition.lock().await;
    let tune_guard = state.current_tune.lock().await;
    let project_guard = state.current_project.lock().await;
    let conn_guard = state.connection.lock().await;
    let demo = *state.demo_mode.lock().await;
    let streaming = state.streaming_task.lock().await.is_some();

    let string_values = string_map_from_tune(tune_guard.as_ref(), def_guard.as_ref());
    let bit_options = bit_options_map(def_guard.as_ref());
    let arrays = array_map_from_tune(tune_guard.as_ref());

    let projects_dir = Project::projects_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let working_dir = project_guard
        .as_ref()
        .map(|p| p.path.display().to_string())
        .unwrap_or_default();

    let is_online = demo && streaming
        || conn_guard
            .as_ref()
            .map(|c| c.state() == ConnectionState::Connected)
            .unwrap_or(false);

    let start_time = state.app_start_epoch;
    let cache = Arc::clone(&state.inc_table_cache);

    drop(conn_guard);
    drop(project_guard);
    drop(tune_guard);
    drop(def_guard);

    let mut ctx = StringContext {
        start_time: Some(start_time),
        ..Default::default()
    };

    ctx.get_string_value = Some(Box::new(move |name| string_values.get(name).cloned()));
    ctx.get_bit_options = Some(Box::new(move |name| bit_options.get(name).cloned()));
    ctx.get_projects_dir = Some(Box::new(move || projects_dir.clone()));
    ctx.get_working_dir = Some(Box::new(move || working_dir.clone()));
    ctx.is_online = Some(Box::new(move || is_online));
    ctx.array_value = Some(Box::new(move |name, index| {
        arrays.get(name).and_then(|v| interpolate_array(v, index))
    }));
    ctx.table_lookup = Some(Box::new(move |filename, lookup| {
        let Ok(mut cache) = cache.lock() else {
            return None;
        };
        cache
            .get_or_load(filename)
            .and_then(|table| table.lookup(lookup))
    }));

    ctx
}

/// Build context from already-held definition/tune snapshots (no AppState locks).
#[allow(dead_code)]
pub fn build_string_context_from_parts(
    def: Option<&EcuDefinition>,
    tune: Option<&TuneFile>,
    working_dir: String,
    is_online: bool,
    start_time: f64,
    cache: Arc<Mutex<IncTableCache>>,
) -> StringContext {
    let string_values = string_map_from_tune(tune, def);
    let bit_options = bit_options_map(def);
    let arrays = array_map_from_tune(tune);
    let projects_dir = Project::projects_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    let mut ctx = StringContext {
        start_time: Some(start_time),
        ..Default::default()
    };
    ctx.get_string_value = Some(Box::new(move |name| string_values.get(name).cloned()));
    ctx.get_bit_options = Some(Box::new(move |name| bit_options.get(name).cloned()));
    ctx.get_projects_dir = Some(Box::new(move || projects_dir.clone()));
    ctx.get_working_dir = Some(Box::new(move || working_dir.clone()));
    ctx.is_online = Some(Box::new(move || is_online));
    ctx.array_value = Some(Box::new(move |name, index| {
        arrays.get(name).and_then(|v| interpolate_array(v, index))
    }));
    ctx.table_lookup = Some(Box::new(move |filename, lookup| {
        let Ok(mut cache) = cache.lock() else {
            return None;
        };
        cache
            .get_or_load(filename)
            .and_then(|table| table.lookup(lookup))
    }));
    ctx
}
