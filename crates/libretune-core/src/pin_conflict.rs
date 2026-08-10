//! Detect duplicate ECU pin assignments across INI constants.
//!
//! rusEFI-family firmwares reject overlapping pin uses with a Settings Error after
//! the fact. LibreTune checks the live tune (bit option labels) so users are warned
//! before burn and blocked from assigning a pin that is already in use.

use crate::ini::{Constant, DataType, EcuDefinition};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One physical pin claimed by multiple constants.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PinConflict {
    /// Resolved pin label from the INI bit list (e.g. `"PD13"`, `"Low Side Output  3"`).
    pub pin_label: String,
    /// Constant names currently assigned to this pin.
    pub constants: Vec<String>,
}

/// Result of scanning the tune for overlapping pin uses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PinConflictReport {
    /// Detected conflicts (empty when clear).
    pub conflicts: Vec<PinConflict>,
}

impl PinConflictReport {
    /// Whether any conflicts were found.
    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }

    /// Human-readable summary for dialogs / burn errors.
    pub fn summary(&self) -> String {
        if self.conflicts.is_empty() {
            return String::new();
        }
        let mut lines = vec![
            "Pin assignment conflict(s) detected — the same pin is used by multiple outputs:"
                .to_string(),
        ];
        for c in &self.conflicts {
            lines.push(format!(
                "• Pin '{}' used by: {}",
                c.pin_label,
                c.constants.join(", ")
            ));
        }
        lines.push(
            "Clear or reassign one of the functions before burning (or confirm to burn anyway)."
                .to_string(),
        );
        lines.join("\n")
    }
}

/// True when this constant looks like a pin selector (not a PinMode / boolean).
pub fn is_pin_assignment_constant(constant: &Constant) -> bool {
    if constant.data_type != DataType::Bits || constant.bit_options.len() < 3 {
        return false;
    }
    let name = constant.name.to_ascii_lowercase();
    if name.contains("pinmode") || name.ends_with("mode") || name.contains("invert") {
        return false;
    }
    let name_looks_like_pin = name.contains("pin");
    let has_none = constant
        .bit_options
        .iter()
        .any(|o| is_unassigned_pin_label(o));
    let has_pinish_option = constant.bit_options.iter().any(|o| looks_like_pin_label(o));
    (name_looks_like_pin && has_none)
        || (has_none && has_pinish_option && constant.bit_options.len() >= 8)
}

/// Labels that mean "no pin selected".
pub fn is_unassigned_pin_label(label: &str) -> bool {
    let t = label.trim();
    if t.is_empty() {
        return true;
    }
    matches!(
        t.to_ascii_lowercase().as_str(),
        "none" | "invalid" | "off" | "disabled" | "not used" | "unused" | "n/a" | "na"
    )
}

fn looks_like_pin_label(label: &str) -> bool {
    let t = label.trim();
    if t.len() < 2 {
        return false;
    }
    // STM32-style: PA0, PB12, PD13, PE15
    let bytes = t.as_bytes();
    if bytes[0].is_ascii_alphabetic()
        && bytes.get(1).is_some_and(|b| b.is_ascii_alphabetic())
        && t[2..].chars().all(|c| c.is_ascii_digit())
        && !t[2..].is_empty()
    {
        return true;
    }
    let lower = t.to_ascii_lowercase();
    lower.contains("output")
        || lower.contains("injector")
        || lower.contains("ignition")
        || lower.contains("digital input")
        || lower.contains("low side")
        || lower.contains("high side")
}

/// Resolve the pin label for a bits constant index.
pub fn pin_label_for_index(constant: &Constant, index: usize) -> Option<&str> {
    constant.bit_options.get(index).map(|s| s.as_str())
}

/// Scan pin-assignment constants for duplicate non-empty pin labels.
///
/// `value_index` returns the current bits option index for a constant name.
pub fn detect_pin_conflicts(
    def: &EcuDefinition,
    mut value_index: impl FnMut(&str, &Constant) -> Option<usize>,
) -> PinConflictReport {
    let mut by_pin: HashMap<String, Vec<String>> = HashMap::new();

    for (name, constant) in &def.constants {
        if !is_pin_assignment_constant(constant) {
            continue;
        }
        let Some(idx) = value_index(name, constant) else {
            continue;
        };
        let Some(label) = pin_label_for_index(constant, idx) else {
            continue;
        };
        if is_unassigned_pin_label(label) {
            continue;
        }
        by_pin
            .entry(label.to_string())
            .or_default()
            .push(name.clone());
    }

    let mut conflicts: Vec<PinConflict> = by_pin
        .into_iter()
        .filter(|(_, names)| names.len() > 1)
        .map(|(pin_label, mut constants)| {
            constants.sort();
            PinConflict {
                pin_label,
                constants,
            }
        })
        .collect();
    conflicts.sort_by(|a, b| a.pin_label.cmp(&b.pin_label));

    PinConflictReport { conflicts }
}

/// Would assigning `new_index` on `constant_name` collide with another pin use?
pub fn conflict_if_assigning(
    def: &EcuDefinition,
    constant_name: &str,
    new_index: usize,
    mut value_index: impl FnMut(&str, &Constant) -> Option<usize>,
) -> Option<PinConflict> {
    let constant = def.constants.get(constant_name)?;
    if !is_pin_assignment_constant(constant) {
        return None;
    }
    let label = pin_label_for_index(constant, new_index)?;
    if is_unassigned_pin_label(label) {
        return None;
    }

    let mut others = Vec::new();
    for (name, other) in &def.constants {
        if name == constant_name || !is_pin_assignment_constant(other) {
            continue;
        }
        let Some(idx) = value_index(name, other) else {
            continue;
        };
        let Some(other_label) = pin_label_for_index(other, idx) else {
            continue;
        };
        if other_label == label {
            others.push(name.clone());
        }
    }
    if others.is_empty() {
        return None;
    }
    others.sort();
    let mut constants = others;
    constants.insert(0, constant_name.to_string());
    Some(PinConflict {
        pin_label: label.to_string(),
        constants,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ini::{Constant, DataType, EcuDefinition, Shape};

    fn pin_const(name: &str, options: &[&str]) -> Constant {
        let mut c = Constant::new(name, 0, 0, DataType::Bits);
        c.shape = Shape::Scalar;
        c.bit_position = Some(0);
        c.bit_size = Some(8);
        c.bit_options = options.iter().map(|s| (*s).to_string()).collect();
        c
    }

    fn def_with(consts: Vec<Constant>) -> EcuDefinition {
        let mut def = EcuDefinition::default();
        for c in consts {
            def.constants.insert(c.name.clone(), c);
        }
        def
    }

    #[test]
    fn detects_duplicate_output_pins() {
        let opts = ["NONE", "INVALID", "PA0", "PD13", "PE1"];
        let def = def_with(vec![
            pin_const("fuelPumpPin", &opts),
            pin_const("gppwm1_pin", &opts),
            pin_const("fanPin", &opts),
        ]);
        let values = HashMap::from([
            ("fuelPumpPin".to_string(), 3usize), // PD13
            ("gppwm1_pin".to_string(), 3usize),  // PD13
            ("fanPin".to_string(), 2usize),      // PA0
        ]);
        let report = detect_pin_conflicts(&def, |name, _| values.get(name).copied());
        assert!(report.has_conflicts());
        assert_eq!(report.conflicts.len(), 1);
        assert_eq!(report.conflicts[0].pin_label, "PD13");
        assert_eq!(
            report.conflicts[0].constants,
            vec!["fuelPumpPin".to_string(), "gppwm1_pin".to_string()]
        );
    }

    #[test]
    fn ignores_none_and_pin_mode() {
        let opts = ["NONE", "INVALID", "PA0", "PD13"];
        let mut mode = pin_const("fuelPumpPinMode", &["default", "inverted", "open"]);
        mode.bit_options = vec![
            "default".into(),
            "default inverted".into(),
            "open collector".into(),
        ];
        let def = def_with(vec![
            pin_const("fuelPumpPin", &opts),
            pin_const("gppwm1_pin", &opts),
            mode,
        ]);
        let values = HashMap::from([
            ("fuelPumpPin".to_string(), 0usize),
            ("gppwm1_pin".to_string(), 0usize),
            ("fuelPumpPinMode".to_string(), 1usize),
        ]);
        let report = detect_pin_conflicts(&def, |name, _| values.get(name).copied());
        assert!(!report.has_conflicts());
    }

    #[test]
    fn blocks_reassign_onto_used_pin() {
        let opts = ["NONE", "PA0", "PD13"];
        let def = def_with(vec![
            pin_const("waterPumpPin", &opts),
            pin_const("gppwm1_pin", &opts),
        ]);
        let values = HashMap::from([("gppwm1_pin".to_string(), 2usize)]);
        let conflict =
            conflict_if_assigning(&def, "waterPumpPin", 2, |name, _| values.get(name).copied());
        assert!(conflict.is_some());
        let c = conflict.unwrap();
        assert_eq!(c.pin_label, "PD13");
        assert!(c.constants.contains(&"gppwm1_pin".to_string()));
    }

    #[test]
    fn allows_clearing_to_none() {
        let opts = ["NONE", "PA0", "PD13"];
        let def = def_with(vec![
            pin_const("waterPumpPin", &opts),
            pin_const("gppwm1_pin", &opts),
        ]);
        let values = HashMap::from([("gppwm1_pin".to_string(), 2usize)]);
        assert!(
            conflict_if_assigning(&def, "waterPumpPin", 0, |name, _| values.get(name).copied())
                .is_none()
        );
    }
}
