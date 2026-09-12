//! INI-dialog view of a tune mismatch (TunerStudio Difference Report).
//!
//! Compare named constants only. Unused/padding bytes never create a page.
//! Values are decoded from the mismatch snapshot — never live ECU reads.

use crate::commands::constant_values::collect_scalar_constant_values;
use crate::commands::curve_ops::CurveData;
use crate::commands::menu::evaluate_visibility;
use crate::commands::table_internals::{read_const_values, TableData};
use crate::state::AppState;
use libretune_core::ini::expression::StringContext;
use libretune_core::ini::{
    Constant, DataType, DialogComponent, DialogDefinition, EcuDefinition, MenuItem,
};
use libretune_core::tune::{TuneCache, TuneFile};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Serialize)]
pub struct TuneMismatchDialogIndexEntry {
    pub name: String,
    pub title: String,
    pub changed_count: u32,
}

#[derive(Serialize)]
pub struct TuneMismatchDialogView {
    pub name: String,
    pub title: String,
    pub definition: DialogDefinition,
    pub changed_names: Vec<String>,
    pub project_numbers: HashMap<String, f64>,
    pub ecu_numbers: HashMap<String, f64>,
    pub project_strings: HashMap<String, String>,
    pub ecu_strings: HashMap<String, String>,
    pub project_tables: HashMap<String, TableData>,
    pub ecu_tables: HashMap<String, TableData>,
    pub project_curves: HashMap<String, CurveData>,
    pub ecu_curves: HashMap<String, CurveData>,
}

fn cache_from_pages(def: &EcuDefinition, pages: &HashMap<u8, Vec<u8>>) -> TuneCache {
    let mut cache = TuneCache::from_definition(def);
    for (page, data) in pages {
        cache.load_page(*page, data.clone());
    }
    cache
}

fn tune_from_pages(signature: &str, pages: &HashMap<u8, Vec<u8>>) -> TuneFile {
    let mut tune = TuneFile::new(signature);
    tune.pages = pages.clone();
    tune
}

fn lookup_dialog(def: &EcuDefinition, name: &str) -> Option<DialogDefinition> {
    def.dialogs
        .get(name)
        .cloned()
        .or_else(|| def.std_panel_definition(name))
}

fn collect_setting_names(
    def: &EcuDefinition,
    dialog_name: &str,
    out: &mut HashSet<String>,
    seen: &mut HashSet<String>,
) {
    if !seen.insert(dialog_name.to_string()) {
        return;
    }
    let Some(dialog) = lookup_dialog(def, dialog_name) else {
        if let Some(table) = def.get_table_by_name_or_map(dialog_name) {
            out.insert(dialog_name.to_string());
            out.insert(table.name.clone());
            out.insert(table.map.clone());
            out.insert(table.x_bins.clone());
            if let Some(y) = &table.y_bins {
                out.insert(y.clone());
            }
        }
        if let Some(curve) = def.get_curve_by_name_or_map(dialog_name) {
            out.insert(dialog_name.to_string());
            out.insert(curve.name.clone());
            out.insert(curve.x_bins.clone());
            out.insert(curve.y_bins.clone());
        }
        return;
    };
    walk_components(def, &dialog.components, out, seen);
}

fn walk_components(
    def: &EcuDefinition,
    components: &[DialogComponent],
    out: &mut HashSet<String>,
    seen: &mut HashSet<String>,
) {
    for comp in components {
        match comp {
            DialogComponent::Field { name, .. } => {
                if def.constants.contains_key(name) {
                    out.insert(name.clone());
                }
            }
            DialogComponent::Table { name } => {
                if let Some(table) = def.get_table_by_name_or_map(name) {
                    out.insert(name.clone());
                    out.insert(table.name.clone());
                    out.insert(table.map.clone());
                    out.insert(table.x_bins.clone());
                    if let Some(y) = &table.y_bins {
                        out.insert(y.clone());
                    }
                }
            }
            DialogComponent::Panel { name, .. } => {
                collect_setting_names(def, name, out, seen);
            }
            _ => {}
        }
    }
}

fn constant_changed(constant: &Constant, project: Option<&Vec<u8>>, ecu: Option<&Vec<u8>>) -> bool {
    let offset = usize::from(constant.offset);
    match constant.data_type {
        DataType::Bits => {
            let pos = u32::from(constant.bit_position.unwrap_or(0));
            let width = u32::from(constant.bit_size.unwrap_or(1)).clamp(1, 8);
            let mask = ((1u32 << width) - 1) << pos;
            let p = project.and_then(|v| v.get(offset)).copied().unwrap_or(0) as u32;
            let e = ecu.and_then(|v| v.get(offset)).copied().unwrap_or(0) as u32;
            (p & mask) != (e & mask)
        }
        _ => {
            let len = if constant.data_type == DataType::String {
                constant.shape.element_count()
            } else {
                constant.size_bytes()
            };
            if len == 0 {
                return false;
            }
            for i in 0..len {
                let p = project
                    .and_then(|v| v.get(offset + i))
                    .copied()
                    .unwrap_or(0);
                let e = ecu.and_then(|v| v.get(offset + i)).copied().unwrap_or(0);
                if p != e {
                    return true;
                }
            }
            false
        }
    }
}

fn changed_settings(
    def: &EcuDefinition,
    names: &HashSet<String>,
    project_pages: &HashMap<u8, Vec<u8>>,
    ecu_pages: &HashMap<u8, Vec<u8>>,
) -> Vec<String> {
    let mut changed = Vec::new();
    for name in names {
        let Some(constant) = def.constants.get(name) else {
            continue;
        };
        if constant.is_pc_variable {
            continue;
        }
        if constant_changed(
            constant,
            project_pages.get(&constant.page),
            ecu_pages.get(&constant.page),
        ) {
            changed.push(name.clone());
        }
    }
    changed.sort();
    changed
}

fn menu_visible(
    visible: bool,
    condition: &Option<String>,
    project: &HashMap<String, f64>,
    ecu: &HashMap<String, f64>,
    strings: &StringContext,
) -> bool {
    if !visible {
        return false;
    }
    evaluate_visibility(condition, project, strings) || evaluate_visibility(condition, ecu, strings)
}

/// TunerStudio pages through menu *dialogs*. Tables/curves stay on the dialog
/// that embeds them. A table menu item becomes its own page only when no
/// visible dialog already contains it.
fn visit_menu_targets(
    def: &EcuDefinition,
    items: &[MenuItem],
    project: &HashMap<String, f64>,
    ecu: &HashMap<String, f64>,
    strings: &StringContext,
    dialogs: &mut Vec<(String, String)>,
    tables: &mut Vec<(String, String)>,
) {
    for item in items {
        match item {
            MenuItem::Dialog {
                label,
                target,
                visibility_condition,
                visible,
                ..
            } => {
                if menu_visible(*visible, visibility_condition, project, ecu, strings) {
                    dialogs.push((target.clone(), label.clone()));
                }
            }
            MenuItem::Table {
                label,
                target,
                visibility_condition,
                visible,
                ..
            } => {
                if menu_visible(*visible, visibility_condition, project, ecu, strings) {
                    tables.push((target.clone(), label.clone()));
                }
            }
            MenuItem::Std {
                label,
                target,
                visibility_condition,
                visible,
                ..
            } => {
                if menu_visible(*visible, visibility_condition, project, ecu, strings)
                    && (lookup_dialog(def, target).is_some()
                        || def.std_panel_definition(target).is_some())
                {
                    dialogs.push((target.clone(), label.clone()));
                }
            }
            MenuItem::SubMenu {
                items,
                visibility_condition,
                visible,
                ..
            } => {
                if menu_visible(*visible, visibility_condition, project, ecu, strings) {
                    visit_menu_targets(def, items, project, ecu, strings, dialogs, tables);
                }
            }
            _ => {}
        }
    }
}

fn covered_table_names(def: &EcuDefinition, dialog_name: &str) -> HashSet<String> {
    let mut settings = HashSet::new();
    collect_setting_names(def, dialog_name, &mut settings, &mut HashSet::new());
    let mut out = HashSet::new();
    for name in settings {
        if let Some(table) = def.get_table_by_name_or_map(&name) {
            out.insert(table.name.clone());
        }
    }
    out
}

fn dialog_definition_or_table(
    def: &EcuDefinition,
    name: &str,
    title: &str,
) -> Option<DialogDefinition> {
    if let Some(d) = lookup_dialog(def, name) {
        return Some(d);
    }
    if def.get_table_by_name_or_map(name).is_some() {
        return Some(DialogDefinition {
            name: name.to_string(),
            title: title.to_string(),
            components: vec![DialogComponent::Table {
                name: name.to_string(),
            }],
            layout_hint: None,
        });
    }
    None
}

fn decode_strings(def: &EcuDefinition, pages: &HashMap<u8, Vec<u8>>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (name, constant) in &def.constants {
        if constant.data_type != DataType::String {
            continue;
        }
        let page = pages.get(&constant.page);
        let offset = usize::from(constant.offset);
        let len = constant.shape.element_count();
        let mut bytes = Vec::with_capacity(len);
        for i in 0..len {
            bytes.push(page.and_then(|v| v.get(offset + i)).copied().unwrap_or(0));
        }
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        out.insert(
            name.clone(),
            String::from_utf8_lossy(&bytes[..end]).to_string(),
        );
    }
    out
}

fn decode_tables(
    def: &EcuDefinition,
    names: &HashSet<String>,
    pages: &HashMap<u8, Vec<u8>>,
) -> HashMap<String, TableData> {
    let tune = tune_from_pages(&def.signature, pages);
    let mut out = HashMap::new();
    for name in names {
        let Some(table) = def.get_table_by_name_or_map(name) else {
            continue;
        };
        if !out.contains_key(&table.name) {
            if let Ok(data) = table_data_from_tune(def, &table.name, &tune) {
                out.insert(table.name.clone(), data);
            } else {
                continue;
            }
        }
        if let Some(data) = out.get(&table.name).cloned() {
            out.insert(name.clone(), data.clone());
            out.insert(table.map.clone(), data);
        }
    }
    out
}

fn decode_curves(
    def: &EcuDefinition,
    names: &HashSet<String>,
    pages: &HashMap<u8, Vec<u8>>,
) -> HashMap<String, CurveData> {
    let tune = tune_from_pages(&def.signature, pages);
    let mut out = HashMap::new();
    for name in names {
        let Some(curve) = def.get_curve_by_name_or_map(name) else {
            continue;
        };
        if !out.contains_key(&curve.name) {
            let Some(x_const) = def.constants.get(&curve.x_bins) else {
                continue;
            };
            let Some(y_const) = def.constants.get(&curve.y_bins) else {
                continue;
            };
            let Ok(x_bins) = read_const_values(x_const, Some(&tune), def.endianness) else {
                continue;
            };
            let Ok(y_bins) = read_const_values(y_const, Some(&tune), def.endianness) else {
                continue;
            };
            out.insert(
                curve.name.clone(),
                CurveData {
                    name: curve.name.clone(),
                    title: curve.title.clone(),
                    x_bins,
                    y_bins,
                    x_label: curve.column_labels.0.clone(),
                    y_label: curve.column_labels.1.clone(),
                    x_axis: curve.x_axis,
                    y_axis: curve.y_axis,
                    x_output_channel: curve.x_output_channel.clone(),
                    gauge: curve.gauge.clone(),
                },
            );
        }
        if let Some(data) = out.get(&curve.name).cloned() {
            out.insert(name.clone(), data);
        }
    }
    out
}

fn table_data_from_tune(
    def: &EcuDefinition,
    table_name: &str,
    tune: &TuneFile,
) -> Result<TableData, String> {
    let table = def
        .get_table_by_name_or_map(table_name)
        .ok_or_else(|| format!("Table {table_name} not found"))?;
    let endianness = def.endianness;
    let x_const = def
        .constants
        .get(&table.x_bins)
        .ok_or_else(|| format!("Constant {} not found", table.x_bins))?;
    let y_const = table
        .y_bins
        .as_ref()
        .and_then(|name| def.constants.get(name));
    let z_const = def
        .constants
        .get(&table.map)
        .ok_or_else(|| format!("Constant {} not found", table.map))?;

    let x_bins = read_const_values(x_const, Some(tune), endianness)?;
    let y_bins = if let Some(y) = y_const {
        read_const_values(y, Some(tune), endianness)?
    } else {
        vec![0.0]
    };
    let z_flat = read_const_values(z_const, Some(tune), endianness)?;
    let is_3d = table.is_3d();
    let x_size = x_bins.len();
    let y_size = if is_3d { y_bins.len() } else { 1 };
    let mut z_values = Vec::with_capacity(y_size);
    for y in 0..y_size {
        let mut row = Vec::with_capacity(x_size);
        for x in 0..x_size {
            row.push(*z_flat.get(y * x_size + x).unwrap_or(&0.0));
        }
        z_values.push(row);
    }
    Ok(TableData {
        name: table.name.clone(),
        title: table.title.clone(),
        x_bins,
        y_bins,
        z_values,
        x_axis_name: table
            .x_label
            .clone()
            .unwrap_or_else(|| table.x_bins.clone()),
        y_axis_name: table
            .y_label
            .clone()
            .unwrap_or_else(|| table.y_bins.clone().unwrap_or_default()),
        x_output_channel: table.x_output_channel.clone(),
        y_output_channel: table.y_output_channel.clone(),
        size_info: None,
    })
}

fn index_from_snapshot(
    def: &EcuDefinition,
    project_pages: &HashMap<u8, Vec<u8>>,
    ecu_pages: &HashMap<u8, Vec<u8>>,
) -> Vec<TuneMismatchDialogIndexEntry> {
    let project_ctx =
        collect_scalar_constant_values(def, None, Some(&cache_from_pages(def, project_pages)));
    let ecu_ctx =
        collect_scalar_constant_values(def, None, Some(&cache_from_pages(def, ecu_pages)));
    let strings = StringContext::default();

    let mut dialog_targets = Vec::new();
    let mut table_targets = Vec::new();
    for menu in &def.menus {
        visit_menu_targets(
            def,
            &menu.items,
            &project_ctx,
            &ecu_ctx,
            &strings,
            &mut dialog_targets,
            &mut table_targets,
        );
    }
    if def.menus.is_empty() {
        for (name, dialog) in &def.dialogs {
            dialog_targets.push((name.clone(), dialog.title.clone()));
        }
    }

    let mut covered_tables = HashSet::new();
    for (name, _) in &dialog_targets {
        covered_tables.extend(covered_table_names(def, name));
    }

    let mut targets = dialog_targets;
    for (name, title) in table_targets {
        let Some(table) = def.get_table_by_name_or_map(&name) else {
            continue;
        };
        if covered_tables.contains(&table.name) {
            continue;
        }
        covered_tables.insert(table.name.clone());
        targets.push((name, title));
    }

    let mut seen = HashSet::new();
    let mut index = Vec::new();
    for (name, title) in targets {
        if !seen.insert(name.clone()) {
            continue;
        }
        let mut settings = HashSet::new();
        collect_setting_names(def, &name, &mut settings, &mut HashSet::new());
        let changed = changed_settings(def, &settings, project_pages, ecu_pages);
        if changed.is_empty() {
            continue;
        }
        let title = lookup_dialog(def, &name).map(|d| d.title).unwrap_or(title);
        index.push(TuneMismatchDialogIndexEntry {
            name,
            title,
            changed_count: changed.len() as u32,
        });
    }
    index
}

#[tauri::command]
pub async fn get_tune_mismatch_dialog_index(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<TuneMismatchDialogIndexEntry>, String> {
    let snapshot_guard = state.tune_mismatch_snapshot.lock().await;
    let snapshot = snapshot_guard
        .as_ref()
        .ok_or("No tune mismatch snapshot available. Re-sync ECU first.")?;
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    Ok(index_from_snapshot(
        def,
        &snapshot.project_pages,
        &snapshot.ecu_pages,
    ))
}

#[tauri::command]
pub async fn get_tune_mismatch_dialog_view(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<TuneMismatchDialogView, String> {
    let snapshot_guard = state.tune_mismatch_snapshot.lock().await;
    let snapshot = snapshot_guard
        .as_ref()
        .ok_or("No tune mismatch snapshot available. Re-sync ECU first.")?;
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let definition = dialog_definition_or_table(def, &name, &name)
        .ok_or_else(|| format!("Dialog {name} not found"))?;
    let mut settings = HashSet::new();
    collect_setting_names(def, &name, &mut settings, &mut HashSet::new());
    let changed_names =
        changed_settings(def, &settings, &snapshot.project_pages, &snapshot.ecu_pages);

    let project_cache = cache_from_pages(def, &snapshot.project_pages);
    let ecu_cache = cache_from_pages(def, &snapshot.ecu_pages);

    Ok(TuneMismatchDialogView {
        name: name.clone(),
        title: definition.title.clone(),
        definition,
        changed_names,
        project_numbers: collect_scalar_constant_values(def, None, Some(&project_cache)),
        ecu_numbers: collect_scalar_constant_values(def, None, Some(&ecu_cache)),
        project_strings: decode_strings(def, &snapshot.project_pages),
        ecu_strings: decode_strings(def, &snapshot.ecu_pages),
        project_tables: decode_tables(def, &settings, &snapshot.project_pages),
        ecu_tables: decode_tables(def, &settings, &snapshot.ecu_pages),
        project_curves: decode_curves(def, &settings, &snapshot.project_pages),
        ecu_curves: decode_curves(def, &settings, &snapshot.ecu_pages),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use libretune_core::ini::{
        Constant, CurveDefinition, DataType, DialogComponent, DialogDefinition, Menu, Shape,
        TableDefinition,
    };

    fn def_with_dialog() -> EcuDefinition {
        let mut def = EcuDefinition {
            n_pages: 1,
            page_sizes: vec![8],
            signature: "test".into(),
            ..EcuDefinition::default()
        };
        let mut pulse = Constant::new("onTime", 0, 0, DataType::U16);
        pulse.scale = 0.001;
        def.constants.insert("onTime".into(), pulse);
        def.dialogs.insert(
            "hardwareTest".into(),
            DialogDefinition {
                name: "hardwareTest".into(),
                title: "Hardware test".into(),
                components: vec![DialogComponent::Field {
                    label: "On Time".into(),
                    name: "onTime".into(),
                    visibility_condition: None,
                    enabled_condition: None,
                }],
                layout_hint: None,
            },
        );
        def.menus.push(Menu {
            name: "tools".into(),
            title: "Tools".into(),
            items: vec![MenuItem::Dialog {
                label: "Hardware test".into(),
                target: "hardwareTest".into(),
                visibility_condition: None,
                enabled_condition: None,
                visible: true,
                enabled: true,
            }],
        });
        def
    }

    #[test]
    fn index_lists_only_dialogs_with_named_diffs() {
        let def = def_with_dialog();
        let mut project = HashMap::new();
        project.insert(0u8, vec![0x00, 0x20, 0, 0, 0, 0, 0, 0]);
        let mut ecu = HashMap::new();
        ecu.insert(0u8, vec![0x00, 0x10, 0, 0, 0, 0, 0, 0]);
        let index = index_from_snapshot(&def, &project, &ecu);
        assert_eq!(index.len(), 1);
        assert_eq!(index[0].name, "hardwareTest");
        assert_eq!(index[0].changed_count, 1);
    }

    #[test]
    fn padding_only_diff_is_not_a_dialog_page() {
        let def = def_with_dialog();
        let mut project = HashMap::new();
        project.insert(0u8, vec![0x00, 0x20, 0, 0, 0, 0, 0, 9]);
        let mut ecu = HashMap::new();
        ecu.insert(0u8, vec![0x00, 0x20, 0, 0, 0, 0, 0, 3]);
        let index = index_from_snapshot(&def, &project, &ecu);
        assert!(index.is_empty());
    }

    #[test]
    fn table_menu_item_does_not_duplicate_dialog_page() {
        let mut def = def_with_dialog();
        let mut map = Constant::new("veMap", 0, 4, DataType::U08);
        map.shape = Shape::Array1D(2);
        def.constants.insert("veMap".into(), map);
        let mut bins = Constant::new("veBins", 0, 6, DataType::U08);
        bins.shape = Shape::Array1D(2);
        def.constants.insert("veBins".into(), bins);
        def.tables.insert(
            "veTbl".into(),
            TableDefinition::new_2d("veTbl", "veMap", "veBins", 2),
        );
        def.dialogs
            .get_mut("hardwareTest")
            .unwrap()
            .components
            .push(DialogComponent::Table {
                name: "veTbl".into(),
            });
        def.menus[0].items.push(MenuItem::Table {
            label: "VE".into(),
            target: "veTbl".into(),
            visibility_condition: None,
            enabled_condition: None,
            visible: true,
            enabled: true,
        });
        let mut project = HashMap::new();
        project.insert(0u8, vec![0x00, 0x20, 0, 0, 1, 2, 3, 4]);
        let mut ecu = HashMap::new();
        ecu.insert(0u8, vec![0x00, 0x10, 0, 0, 1, 2, 3, 4]);
        let index = index_from_snapshot(&def, &project, &ecu);
        assert_eq!(index.len(), 1);
        assert_eq!(index[0].name, "hardwareTest");
    }

    #[test]
    fn hidden_menu_dialog_is_not_a_page() {
        let mut def = def_with_dialog();
        if let MenuItem::Dialog { visible, .. } = &mut def.menus[0].items[0] {
            *visible = false;
        }
        let mut project = HashMap::new();
        project.insert(0u8, vec![0x00, 0x20, 0, 0, 0, 0, 0, 0]);
        let mut ecu = HashMap::new();
        ecu.insert(0u8, vec![0x00, 0x10, 0, 0, 0, 0, 0, 0]);
        let index = index_from_snapshot(&def, &project, &ecu);
        assert!(index.is_empty());
    }

    #[test]
    fn unused_constant_is_ignored_when_not_on_dialog() {
        let mut def = def_with_dialog();
        def.constants
            .insert("other".into(), Constant::new("other", 0, 4, DataType::U08));
        let mut project = HashMap::new();
        project.insert(0u8, vec![0x00, 0x20, 0, 0, 7, 0, 0, 0]);
        let mut ecu = HashMap::new();
        ecu.insert(0u8, vec![0x00, 0x20, 0, 0, 1, 0, 0, 0]);
        let index = index_from_snapshot(&def, &project, &ecu);
        assert!(index.is_empty());
    }

    #[test]
    fn curves_decode_from_snapshot_pages() {
        let mut def = EcuDefinition {
            n_pages: 1,
            page_sizes: vec![16],
            signature: "test".into(),
            ..EcuDefinition::default()
        };
        let mut x = Constant::new("warmX", 0, 0, DataType::U08);
        x.shape = Shape::Array1D(4);
        let mut y = Constant::new("warmY", 0, 4, DataType::U08);
        y.shape = Shape::Array1D(4);
        def.constants.insert("warmX".into(), x);
        def.constants.insert("warmY".into(), y);
        def.curves.insert(
            "warmup".into(),
            CurveDefinition::new("warmup", "warmX", "warmY"),
        );
        let mut names = HashSet::new();
        names.insert("warmup".into());
        let mut pages = HashMap::new();
        pages.insert(
            0u8,
            vec![1, 2, 3, 4, 10, 20, 30, 40, 0, 0, 0, 0, 0, 0, 0, 0],
        );
        let curves = decode_curves(&def, &names, &pages);
        assert_eq!(curves["warmup"].x_bins, vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(curves["warmup"].y_bins, vec![10.0, 20.0, 30.0, 40.0]);
    }
}
