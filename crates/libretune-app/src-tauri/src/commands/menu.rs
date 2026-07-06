//! Menu tree and searchable index commands.

use crate::state::AppState;
use libretune_core::ini::{Menu, MenuItem};
use std::collections::HashMap;

#[tauri::command]
pub async fn get_menu_tree(
    state: tauri::State<'_, AppState>,
    filter_context: Option<HashMap<String, f64>>,
) -> Result<Vec<Menu>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    // Always return all menu items - visibility conditions are evaluated but items are never filtered out
    // This allows the frontend to show all items (grayed out if disabled) and enables search to find everything
    if let Some(context) = filter_context {
        let mut all_menus = Vec::new();
        for menu in &def.menus {
            let items_with_flags = add_visibility_flags(&menu.items, &context);
            all_menus.push(Menu {
                name: menu.name.clone(),
                title: menu.title.clone(),
                items: items_with_flags,
            });
        }
        Ok(all_menus)
    } else {
        Ok(def.menus.clone())
    }
}

/// Recursively add visibility/enabled flags to menu items without filtering them out
fn add_visibility_flags(items: &[MenuItem], context: &HashMap<String, f64>) -> Vec<MenuItem> {
    items
        .iter()
        .map(|item| {
            match item {
                MenuItem::Dialog {
                    label,
                    target,
                    visibility_condition,
                    enabled_condition,
                    ..
                } => {
                    let visible = evaluate_visibility(visibility_condition, context);
                    let enabled = evaluate_visibility(enabled_condition, context);
                    MenuItem::Dialog {
                        label: label.clone(),
                        target: target.clone(),
                        visibility_condition: visibility_condition.clone(),
                        enabled_condition: enabled_condition.clone(),
                        visible,
                        enabled,
                    }
                }
                MenuItem::Table {
                    label,
                    target,
                    visibility_condition,
                    enabled_condition,
                    ..
                } => {
                    let visible = evaluate_visibility(visibility_condition, context);
                    let enabled = evaluate_visibility(enabled_condition, context);
                    MenuItem::Table {
                        label: label.clone(),
                        target: target.clone(),
                        visibility_condition: visibility_condition.clone(),
                        enabled_condition: enabled_condition.clone(),
                        visible,
                        enabled,
                    }
                }
                MenuItem::SubMenu {
                    label,
                    items: sub_items,
                    visibility_condition,
                    enabled_condition,
                    ..
                } => {
                    let visible = evaluate_visibility(visibility_condition, context);
                    let enabled = evaluate_visibility(enabled_condition, context);
                    // Recursively process children
                    let children_with_flags = add_visibility_flags(sub_items, context);
                    MenuItem::SubMenu {
                        label: label.clone(),
                        items: children_with_flags,
                        visibility_condition: visibility_condition.clone(),
                        enabled_condition: enabled_condition.clone(),
                        visible,
                        enabled,
                    }
                }
                MenuItem::Std {
                    label,
                    target,
                    visibility_condition,
                    enabled_condition,
                    ..
                } => {
                    let visible = evaluate_visibility(visibility_condition, context);
                    let enabled = evaluate_visibility(enabled_condition, context);
                    MenuItem::Std {
                        label: label.clone(),
                        target: target.clone(),
                        visibility_condition: visibility_condition.clone(),
                        enabled_condition: enabled_condition.clone(),
                        visible,
                        enabled,
                    }
                }
                MenuItem::Help {
                    label,
                    target,
                    visibility_condition,
                    enabled_condition,
                    ..
                } => {
                    let visible = evaluate_visibility(visibility_condition, context);
                    let enabled = evaluate_visibility(enabled_condition, context);
                    MenuItem::Help {
                        label: label.clone(),
                        target: target.clone(),
                        visibility_condition: visibility_condition.clone(),
                        enabled_condition: enabled_condition.clone(),
                        visible,
                        enabled,
                    }
                }
                MenuItem::Separator => MenuItem::Separator,
            }
        })
        .collect()
}

/// Evaluate visibility condition - returns true if visible (or on error/missing condition)
fn evaluate_visibility(condition: &Option<String>, context: &HashMap<String, f64>) -> bool {
    if let Some(cond) = condition {
        let mut parser = libretune_core::ini::expression::Parser::new(cond);
        if let Ok(expr) = parser.parse() {
            if let Ok(val) = libretune_core::ini::expression::evaluate_simple(&expr, context) {
                return val.as_bool();
            }
        }
    }
    true // Default to visible
}

/// Builds a searchable index of all menu targets and their content.
/// Maps target names to searchable terms (field labels, panel titles, etc.)
/// This enables deep search - finding dialogs by their field contents.
#[tauri::command]
pub async fn get_searchable_index(
    state: tauri::State<'_, AppState>,
) -> Result<HashMap<String, Vec<String>>, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;

    let mut index: HashMap<String, Vec<String>> = HashMap::new();

    // Recursively collect searchable terms from a dialog and its nested panels
    fn collect_dialog_terms(
        dialog_name: &str,
        dialogs: &HashMap<String, libretune_core::ini::DialogDefinition>,
        visited: &mut std::collections::HashSet<String>,
        terms: &mut Vec<String>,
    ) {
        if !visited.insert(dialog_name.to_string()) {
            return; // Already visited, avoid cycles
        }
        let dialog = match dialogs.get(dialog_name) {
            Some(d) => d,
            None => return,
        };

        terms.push(dialog.title.clone());

        for component in &dialog.components {
            match component {
                libretune_core::ini::DialogComponent::Label { text } => {
                    terms.push(text.clone());
                }
                libretune_core::ini::DialogComponent::Field { label, name, .. } => {
                    terms.push(label.clone());
                    terms.push(name.clone());
                }
                libretune_core::ini::DialogComponent::RuntimeValue { label, name, .. } => {
                    terms.push(label.clone());
                    terms.push(name.clone());
                }
                libretune_core::ini::DialogComponent::Panel { name, .. } => {
                    // Recurse into the referenced sub-dialog
                    collect_dialog_terms(name, dialogs, visited, terms);
                }
                libretune_core::ini::DialogComponent::Table { name } => {
                    terms.push(name.clone());
                }
                libretune_core::ini::DialogComponent::LiveGraph { title, .. } => {
                    terms.push(title.clone());
                }
                libretune_core::ini::DialogComponent::Indicator {
                    label_off,
                    label_on,
                    ..
                } => {
                    terms.push(label_off.clone());
                    terms.push(label_on.clone());
                }
                libretune_core::ini::DialogComponent::CommandButton { label, .. } => {
                    terms.push(label.clone());
                }
            }
        }
    }

    // Index dialogs - collect field labels, panel titles, and nested panel content
    for dialog_name in def.dialogs.keys() {
        let mut terms: Vec<String> = Vec::new();
        let mut visited = std::collections::HashSet::new();
        collect_dialog_terms(dialog_name, &def.dialogs, &mut visited, &mut terms);

        if !terms.is_empty() {
            index.insert(dialog_name.clone(), terms);
        }
    }

    // Index tables - collect title, axis labels
    for (table_name, table) in &def.tables {
        let mut terms: Vec<String> = Vec::new();

        terms.push(table.title.clone());
        terms.push(table.x_bins.clone());

        if let Some(map_name) = &table.map_name {
            terms.push(map_name.clone());
        }
        if let Some(y_bins) = &table.y_bins {
            terms.push(y_bins.clone());
        }
        // Add the table's map constant name
        terms.push(table.map.clone());

        if !terms.is_empty() {
            index.insert(table_name.clone(), terms);
        }
    }

    // Index curves - collect title, axis labels
    for (curve_name, curve) in &def.curves {
        let mut terms: Vec<String> = Vec::new();

        terms.push(curve.title.clone());
        terms.push(curve.column_labels.0.clone()); // X label
        terms.push(curve.column_labels.1.clone()); // Y label

        // Add constant names
        terms.push(curve.x_bins.clone());
        terms.push(curve.y_bins.clone());

        if !terms.is_empty() {
            index.insert(curve_name.clone(), terms);
        }
    }

    Ok(index)
}
