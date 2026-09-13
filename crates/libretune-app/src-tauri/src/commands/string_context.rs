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

/// Build a numeric evaluation context from tune scalar/bool/bits constants.
pub fn numeric_context_from_tune(tune: Option<&TuneFile>) -> HashMap<String, f64> {
    numeric_context_from_tune_def(tune, None)
}

/// Like [`numeric_context_from_tune`], resolving quoted bitfield labels to indices.
pub fn numeric_context_from_tune_def(
    tune: Option<&TuneFile>,
    def: Option<&EcuDefinition>,
) -> HashMap<String, f64> {
    let mut context = HashMap::new();
    let Some(tune) = tune else {
        return context;
    };
    let resolve = |name: &str, value: &TuneValue, dest: &mut HashMap<String, f64>| match value {
        TuneValue::Scalar(n) => {
            dest.insert(name.to_string(), *n);
        }
        TuneValue::Bool(b) => {
            dest.insert(name.to_string(), if *b { 1.0 } else { 0.0 });
        }
        TuneValue::Array(arr) if !arr.is_empty() => {
            dest.insert(name.to_string(), arr[0]);
        }
        TuneValue::String(s) => {
            if let Some(idx) = def.and_then(|d| d.constants.get(name)).and_then(|c| {
                c.bit_options.iter().position(|opt| opt == s).or_else(|| {
                    c.bit_options
                        .iter()
                        .position(|opt| opt.eq_ignore_ascii_case(s))
                })
            }) {
                dest.insert(name.to_string(), idx as f64);
            }
        }
        _ => {}
    };
    for (name, value) in &tune.constants {
        resolve(name, value, &mut context);
    }
    for (name, value) in &tune.pc_variables {
        resolve(name, value, &mut context);
    }
    context
}

/// Identifiers an expression mentions.
///
/// Used to build only the lookup maps an expression can actually reach. The
/// scan is deliberately over-inclusive - it returns every bare word, including
/// function names and keywords - because a spurious extra entry costs one map
/// insertion while a missing one would change the result.
pub(crate) fn referenced_identifiers(expression: &str) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    let mut word = String::new();
    // A name cannot start with a digit, so a run beginning with one is a
    // numeric literal and is dropped rather than inserted as junk.
    let keep = |w: &str| w.starts_with(|c: char| c.is_alphabetic() || c == '_');
    for ch in expression.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            word.push(ch);
        } else if !word.is_empty() {
            let w = std::mem::take(&mut word);
            if keep(&w) {
                out.insert(w);
            }
        }
    }
    if !word.is_empty() && keep(&word) {
        out.insert(word);
    }
    out
}

type NameFilter<'a> = Option<&'a std::collections::HashSet<String>>;

fn wanted(filter: NameFilter, name: &str) -> bool {
    filter.is_none_or(|f| f.contains(name))
}

fn string_map_from_tune(
    tune: Option<&TuneFile>,
    def: Option<&EcuDefinition>,
    filter: NameFilter,
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Some(tune) = tune else {
        return map;
    };
    for (name, value) in &tune.constants {
        if !wanted(filter, name) {
            continue;
        }
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

fn bit_options_map(
    def: Option<&EcuDefinition>,
    filter: NameFilter,
) -> HashMap<String, Vec<String>> {
    let mut map = HashMap::new();
    let Some(def) = def else {
        return map;
    };
    for (name, constant) in &def.constants {
        if !wanted(filter, name) {
            continue;
        }
        if !constant.bit_options.is_empty() {
            map.insert(name.clone(), constant.bit_options.clone());
        }
    }
    map
}

fn array_map_from_tune(tune: Option<&TuneFile>, filter: NameFilter) -> HashMap<String, Vec<f64>> {
    let mut map = HashMap::new();
    let Some(tune) = tune else {
        return map;
    };
    for (name, value) in &tune.constants {
        if !wanted(filter, name) {
            continue;
        }
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
    build_string_context_filtered(state, None).await
}

/// As [`build_string_context`], but only populating entries an expression can
/// reach.
///
/// The unfiltered build clones every string constant, every array, and the
/// option list of every bitfield: on a real Speeduino INI that is 910
/// constants, 507 of them bitfields and 144 arrays, rebuilt on every call.
/// Measured against a live ECU it costs ~100 ms per call, and it is CPU rather
/// than lock contention (100.6 ms with the realtime stream stopped, 115.7 ms
/// with it running). The dashboard evaluates a visibility expression every
/// 250 ms per conditioned gauge, so a handful of them saturates the backend -
/// which is what made raising the realtime poll rate unusable.
///
/// An expression names a few identifiers, so building the rest is pure waste.
pub async fn build_string_context_filtered(
    state: &AppState,
    filter: NameFilter<'_>,
) -> StringContext {
    let def_guard = state.definition.lock().await;
    let tune_guard = state.current_tune.lock().await;
    let project_guard = state.current_project.lock().await;
    let demo = *state.demo_mode.lock().await;
    let streaming = state.streaming_task.lock().await.is_some();

    // The connection is consulted for exactly one boolean, so never wait on
    // its mutex here. This context builds while table writes hold the
    // connection for seconds (paced, verified serial I/O), and blocking on it
    // with `current_tune` held deadlocks those writes — they hold the
    // connection and then take `current_tune` — until something tears one
    // side down. A busy connection mutex means serial I/O is in flight,
    // which is itself proof of "online".
    let conn_online = match state.connection.try_lock() {
        Ok(g) => g
            .as_ref()
            .map(|c| c.state() == ConnectionState::Connected)
            .unwrap_or(false),
        Err(_) => true,
    };

    let string_values = string_map_from_tune(tune_guard.as_ref(), def_guard.as_ref(), filter);
    let bit_options = bit_options_map(def_guard.as_ref(), filter);
    let arrays = array_map_from_tune(tune_guard.as_ref(), filter);

    let projects_dir = Project::projects_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let working_dir = project_guard
        .as_ref()
        .map(|p| p.path.display().to_string())
        .unwrap_or_default();

    let is_online = demo && streaming || conn_online;

    let start_time = state.app_start_epoch;
    let cache = Arc::clone(&state.inc_table_cache);

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
    let string_values = string_map_from_tune(tune, def, None);
    let bit_options = bit_options_map(def, None);
    let arrays = array_map_from_tune(tune, None);
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

#[cfg(test)]
mod filter_tests {
    use super::referenced_identifiers;

    /// The filter decides which of the INI's ~910 constants get cloned into the
    /// evaluation context, so it must never miss a name the expression can
    /// reach. Being over-inclusive is free; being under-inclusive changes the
    /// answer.
    #[test]
    fn finds_every_name_an_expression_can_reach() {
        let ids = referenced_identifiers("{ nCylinders > 4 && injType == 1 }");
        assert!(ids.contains("nCylinders"));
        assert!(ids.contains("injType"));

        // Underscores and digits are part of a name, not separators.
        let ids = referenced_identifiers("std_injection + rtc_trim * page2Value");
        assert!(ids.contains("std_injection"));
        assert!(ids.contains("rtc_trim"));
        assert!(ids.contains("page2Value"));

        // Names butted against punctuation must still be seen.
        let ids = referenced_identifiers("table(fuelLoadBins,rpm)>=veTable1Tbl");
        for want in ["table", "fuelLoadBins", "rpm", "veTable1Tbl"] {
            assert!(ids.contains(want), "missed {want}");
        }
    }

    #[test]
    fn an_expression_naming_nothing_yields_nothing() {
        assert!(referenced_identifiers("1 + 2 * (3 - 4)").is_empty());
    }
}

#[cfg(test)]
mod numeric_context_tests {
    use super::numeric_context_from_tune_def;
    use libretune_core::ini::{Constant, DataType, EcuDefinition};
    use libretune_core::tune::{TuneFile, TuneValue};

    #[test]
    fn bits_string_becomes_option_index() {
        let mut etb = Constant::new("etbFunctions1", 0, 0, DataType::Bits);
        etb.bit_options = vec!["None".into(), "Throttle 1".into()];
        let mut def = EcuDefinition::default();
        def.constants.insert("etbFunctions1".into(), etb);

        let mut tune = TuneFile::new("test");
        tune.constants.insert(
            "etbFunctions1".into(),
            TuneValue::String("Throttle 1".into()),
        );

        assert!(super::numeric_context_from_tune(Some(&tune))
            .get("etbFunctions1")
            .is_none());
        assert_eq!(
            numeric_context_from_tune_def(Some(&tune), Some(&def)).get("etbFunctions1"),
            Some(&1.0)
        );
    }
}
