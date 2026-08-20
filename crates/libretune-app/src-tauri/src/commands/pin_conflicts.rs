//! Pin assignment conflict checks (pre-burn / before pin constant writes).

use crate::commands::constant_values::read_constant_from_cache_or_tune;
use crate::commands::string_context::numeric_context_from_tune;
use crate::AppState;
use libretune_core::ini::expression::{evaluate, Parser};
use libretune_core::ini::{DialogComponent, EcuDefinition};
use libretune_core::pin_conflict::{
    conflict_if_assigning, detect_pin_conflicts, PinConflictReport,
};
use libretune_core::tune::TuneFile;
use std::collections::HashMap;

/// Constants the INI itself says are switched off, and whose pin therefore
/// means nothing.
///
/// A pin selector holds a value whether or not its feature is enabled, and the
/// unused ones sit on defaults that collide by construction: on a stock
/// Speeduino tune the sixteen `Auxin*` selectors share two analog pins, and
/// `knock_pin` sits on the same pin as `ignBypassPin` while knock detection is
/// off. Reporting those as conflicts made the pre-burn check cry wolf on every
/// burn of an unmodified tune — which teaches people to click through the one
/// warning that might be real.
///
/// The INI already knows: each pin field carries the condition under which it
/// applies (`field = "Knock Pin", knock_pin, { knock_mode }`). A constant whose
/// condition evaluates false is not claiming its pin.
///
/// An unparseable or unevaluatable condition keeps the constant in the scan:
/// a missed conflict is worse than a spurious one.
fn disabled_pin_constants(def: &EcuDefinition, tune: Option<&TuneFile>) -> HashMap<String, bool> {
    let context = numeric_context_from_tune(tune);
    let mut out = HashMap::new();

    for dialog in def.dialogs.values() {
        for component in &dialog.components {
            let DialogComponent::Field {
                name,
                visibility_condition,
                enabled_condition,
                ..
            } = component
            else {
                continue;
            };
            let Some(cond) = enabled_condition.as_ref().or(visibility_condition.as_ref()) else {
                continue;
            };
            let enabled = Parser::new(cond)
                .parse()
                .ok()
                .and_then(|expr| evaluate(&expr, &context, None).ok())
                .map(|v| v.as_bool())
                .unwrap_or(true);
            // A constant can appear in more than one dialog; enabled anywhere
            // means it is live.
            *out.entry(name.clone()).or_insert(false) |= enabled;
        }
    }

    out
}

/// Scan the loaded tune for pins claimed by more than one constant.
pub(crate) async fn scan_pin_conflicts(state: &AppState) -> Result<PinConflictReport, String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let cache_guard = state.tune_cache.lock().await;
    let tune_guard = state.current_tune.lock().await;
    let endianness = def.endianness;
    let live = disabled_pin_constants(def, tune_guard.as_ref());

    Ok(detect_pin_conflicts(def, |name, constant| {
        // `None` drops the constant from the scan entirely.
        if live.get(name) == Some(&false) {
            return None;
        }
        let v = read_constant_from_cache_or_tune(
            name,
            constant,
            endianness,
            tune_guard.as_ref(),
            cache_guard.as_ref(),
        );
        Some(v as usize)
    }))
}

/// Scan the loaded tune for pins claimed by more than one constant.
#[tauri::command]
pub async fn check_pin_conflicts(
    state: tauri::State<'_, AppState>,
) -> Result<PinConflictReport, String> {
    scan_pin_conflicts(&state).await
}

/// Shared helper: reject assigning `name` to bits index `value` when that pin is taken.
pub(crate) async fn deny_if_pin_conflict(
    state: &AppState,
    name: &str,
    value: f64,
) -> Result<(), String> {
    let def_guard = state.definition.lock().await;
    let def = def_guard.as_ref().ok_or("Definition not loaded")?;
    let cache_guard = state.tune_cache.lock().await;
    let tune_guard = state.current_tune.lock().await;
    let endianness = def.endianness;
    let new_index = value as usize;
    let live = disabled_pin_constants(def, tune_guard.as_ref());

    if let Some(conflict) = conflict_if_assigning(def, name, new_index, |other_name, constant| {
        if other_name == name {
            return None;
        }
        // A switched-off feature does not hold its pin against a new
        // assignment, for the same reason it is not a conflict at burn time.
        if live.get(other_name) == Some(&false) {
            return None;
        }
        let v = read_constant_from_cache_or_tune(
            other_name,
            constant,
            endianness,
            tune_guard.as_ref(),
            cache_guard.as_ref(),
        );
        Some(v as usize)
    }) {
        return Err(format!(
            "Pin '{}' is already used by {}. Clear that assignment first.",
            conflict.pin_label,
            conflict
                .constants
                .iter()
                .filter(|c| c.as_str() != name)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use libretune_core::ini::{Constant, DataType, DialogDefinition, Shape};

    fn pin_const(name: &str) -> Constant {
        let mut c = Constant::new(name, 0, 0, DataType::Bits);
        c.shape = Shape::Scalar;
        c.bit_position = Some(0);
        c.bit_size = Some(8);
        c.bit_options = ["Board Default", "INVALID", "3", "4"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        c
    }

    fn field(name: &str, cond: Option<&str>) -> DialogComponent {
        DialogComponent::Field {
            label: name.to_string(),
            name: name.to_string(),
            visibility_condition: None,
            enabled_condition: cond.map(|c| c.to_string()),
        }
    }

    /// `knock_pin` and `ignBypassPin` both sit on pin 3 in a stock Speeduino
    /// tune, and the INI gates the knock field on `{ knock_mode }`. With knock
    /// off that is not a conflict — reporting it made every burn of an
    /// unmodified tune raise a warning, which is how a real one gets clicked
    /// through.
    #[test]
    fn a_switched_off_feature_does_not_claim_its_pin() {
        let mut def = EcuDefinition::default();
        for n in ["knock_pin", "ignBypassPin"] {
            def.constants.insert(n.to_string(), pin_const(n));
        }
        def.dialogs.insert(
            "ignitionSettings".to_string(),
            DialogDefinition {
                name: "ignitionSettings".to_string(),
                title: "Ignition".to_string(),
                components: vec![
                    field("knock_pin", Some("knock_mode")),
                    field("ignBypassPin", None),
                ],
                layout_hint: None,
            },
        );

        // knock_mode absent from the tune => 0 => the field is off.
        let live = disabled_pin_constants(&def, None);
        assert_eq!(live.get("knock_pin"), Some(&false));

        let report = detect_pin_conflicts(&def, |name, _| {
            if live.get(name) == Some(&false) {
                return None;
            }
            Some(2usize) // both would select pin "3"
        });
        assert!(
            !report.has_conflicts(),
            "gated-off pin must not conflict: {}",
            report.summary()
        );
    }

    /// The gate must not swallow real conflicts: an ungated pin field still
    /// collides, and a condition that cannot be evaluated keeps its constant
    /// in the scan rather than silently dropping it.
    #[test]
    fn live_and_unparseable_fields_still_conflict() {
        let mut def = EcuDefinition::default();
        for n in ["fuelPumpPin", "fanPin"] {
            def.constants.insert(n.to_string(), pin_const(n));
        }
        def.dialogs.insert(
            "outputs".to_string(),
            DialogDefinition {
                name: "outputs".to_string(),
                title: "Outputs".to_string(),
                components: vec![
                    field("fuelPumpPin", Some("!@#$ not an expression")),
                    field("fanPin", None),
                ],
                layout_hint: None,
            },
        );

        let live = disabled_pin_constants(&def, None);
        assert_ne!(
            live.get("fuelPumpPin"),
            Some(&false),
            "an unevaluatable condition must not disable the check"
        );

        let report = detect_pin_conflicts(&def, |name, _| {
            if live.get(name) == Some(&false) {
                return None;
            }
            Some(2usize)
        });
        assert!(report.has_conflicts(), "a real conflict must still fire");
    }
}
