//! Port Editor Module
//!
//! Handles ECU digital output pin configuration and conflict detection.
//! Supports configuration of injectors, ignition outputs, tach, fuel pump, and idle valve.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents an ECU digital output port/pin
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DigitalOutputType {
    /// Fuel injector output
    InjectorOutput { number: u8 }, // 1-8
    /// Ignition coil output
    IgnitionOutput { number: u8 }, // 1-8
    /// Tachometer output
    TachOutput,
    /// Fuel pump relay output
    FuelPumpOutput,
    /// Idle air control valve output
    IdleValveOutput,
    /// VVT (Variable Valve Timing) control output
    VvtOutput,
    /// Other generic digital output
    Other(String),
}

impl DigitalOutputType {
    /// Get human-readable name
    pub fn name(&self) -> String {
        match self {
            Self::InjectorOutput { number } => format!("Injector {}", number),
            Self::IgnitionOutput { number } => format!("Ignition Output {}", number),
            Self::TachOutput => "Tachometer".to_string(),
            Self::FuelPumpOutput => "Fuel Pump".to_string(),
            Self::IdleValveOutput => "Idle Valve".to_string(),
            Self::VvtOutput => "VVT Control".to_string(),
            Self::Other(name) => name.clone(),
        }
    }

    /// Get category for grouping
    pub fn category(&self) -> &str {
        match self {
            Self::InjectorOutput { .. } => "Fuel Control",
            Self::IgnitionOutput { .. } => "Ignition Control",
            Self::TachOutput => "Diagnostics",
            Self::FuelPumpOutput => "Fuel Control",
            Self::IdleValveOutput => "Fuel Control",
            Self::VvtOutput => "Variable Timing",
            Self::Other(_) => "Other",
        }
    }
}

/// Represents a physical ECU pin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcuPin {
    /// Pin number or identifier
    pub pin_id: String,
    /// Human-readable pin name
    pub pin_name: String,
    /// Available for assignment
    pub is_available: bool,
    /// Description of pin capability
    pub description: String,
}

/// Port configuration entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortAssignment {
    /// The output type being assigned
    pub output: DigitalOutputType,
    /// The physical pin it's assigned to
    pub pin_id: String,
    /// Whether this assignment is active
    pub enabled: bool,
}

/// Port editor configuration for an ECU
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortEditorConfig {
    /// Available physical pins on the ECU
    pub available_pins: Vec<EcuPin>,
    /// Current port assignments
    pub assignments: Vec<PortAssignment>,
}

/// Conflict detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictReport {
    /// List of detected conflicts
    pub conflicts: Vec<PortConflict>,
    /// Whether any conflicts exist
    pub has_conflicts: bool,
}

/// A single port conflict
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortConflict {
    /// Pin where conflict occurs
    pub pin_id: String,
    /// Outputs assigned to the same pin
    pub conflicting_outputs: Vec<String>,
    /// Description of the conflict
    pub description: String,
}

impl PortEditorConfig {
    /// Create a new port editor config
    pub fn new(available_pins: Vec<EcuPin>) -> Self {
        Self {
            available_pins,
            assignments: Vec::new(),
        }
    }

    /// Add a port assignment
    pub fn add_assignment(&mut self, output: DigitalOutputType, pin_id: String) -> Result<(), String> {
        // Validate pin exists
        if !self.available_pins.iter().any(|p| p.pin_id == pin_id) {
            return Err(format!("Pin {} does not exist", pin_id));
        }

        // Check for duplicate output assignments
        if self
            .assignments
            .iter()
            .any(|a| a.output == output && a.enabled)
        {
            return Err(format!("{} is already assigned", output.name()));
        }

        self.assignments.push(PortAssignment {
            output,
            pin_id,
            enabled: true,
        });

        Ok(())
    }

    /// Remove an assignment
    pub fn remove_assignment(&mut self, output: &DigitalOutputType) {
        self.assignments.retain(|a| &a.output != output);
    }

    /// Modify an existing assignment
    pub fn modify_assignment(&mut self, output: DigitalOutputType, new_pin_id: String) -> Result<(), String> {
        // Validate pin exists
        if !self.available_pins.iter().any(|p| p.pin_id == new_pin_id) {
            return Err(format!("Pin {} does not exist", new_pin_id));
        }

        // Find and update the assignment
        if let Some(assignment) = self.assignments.iter_mut().find(|a| a.output == output) {
            assignment.pin_id = new_pin_id;
            Ok(())
        } else {
            Err(format!("Assignment for {} not found", output.name()))
        }
    }

    /// Detect port conflicts
    pub fn detect_conflicts(&self) -> ConflictReport {
        let mut conflicts = Vec::new();
        let mut pin_usage: HashMap<String, Vec<String>> = HashMap::new();

        // Count which outputs are assigned to each pin
        for assignment in &self.assignments {
            if assignment.enabled {
                pin_usage
                    .entry(assignment.pin_id.clone())
                    .or_insert_with(Vec::new)
                    .push(assignment.output.name());
            }
        }

        // Find conflicts (multiple outputs on same pin)
        for (pin_id, outputs) in pin_usage {
            if outputs.len() > 1 {
                conflicts.push(PortConflict {
                    pin_id: pin_id.clone(),
                    conflicting_outputs: outputs.clone(),
                    description: format!(
                        "Multiple outputs assigned to pin {}: {}",
                        pin_id,
                        outputs.join(", ")
                    ),
                });
            }
        }

        ConflictReport {
            has_conflicts: !conflicts.is_empty(),
            conflicts,
        }
    }

    /// Get all available pin IDs
    pub fn available_pin_ids(&self) -> Vec<String> {
        self.available_pins.iter().map(|p| p.pin_id.clone()).collect()
    }

    /// Get assignments grouped by category
    pub fn assignments_by_category(&self) -> HashMap<String, Vec<PortAssignment>> {
        let mut grouped: HashMap<String, Vec<PortAssignment>> = HashMap::new();

        for assignment in &self.assignments {
            let category = assignment.output.category().to_string();
            grouped.entry(category).or_insert_with(Vec::new).push(assignment.clone());
        }

        grouped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> PortEditorConfig {
        PortEditorConfig::new(vec![
            EcuPin {
                pin_id: "A1".to_string(),
                pin_name: "Port A, Pin 1".to_string(),
                is_available: true,
                description: "General purpose digital output".to_string(),
            },
            EcuPin {
                pin_id: "A2".to_string(),
                pin_name: "Port A, Pin 2".to_string(),
                is_available: true,
                description: "General purpose digital output".to_string(),
            },
            EcuPin {
                pin_id: "B1".to_string(),
                pin_name: "Port B, Pin 1".to_string(),
                is_available: true,
                description: "General purpose digital output".to_string(),
            },
        ])
    }

    #[test]
    fn test_add_assignment() {
        let mut config = create_test_config();

        let result = config.add_assignment(DigitalOutputType::InjectorOutput { number: 1 }, "A1".to_string());
        assert!(result.is_ok());
        assert_eq!(config.assignments.len(), 1);
    }

    #[test]
    fn test_add_assignment_invalid_pin() {
        let mut config = create_test_config();

        let result = config.add_assignment(DigitalOutputType::InjectorOutput { number: 1 }, "Z9".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_assignment() {
        let mut config = create_test_config();

        config.add_assignment(DigitalOutputType::InjectorOutput { number: 1 }, "A1".to_string()).unwrap();

        let result = config.add_assignment(DigitalOutputType::InjectorOutput { number: 1 }, "A2".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_detect_conflicts() {
        let mut config = create_test_config();

        config.add_assignment(DigitalOutputType::InjectorOutput { number: 1 }, "A1".to_string()).unwrap();
        config.add_assignment(DigitalOutputType::FuelPumpOutput, "A1".to_string()).unwrap();

        let report = config.detect_conflicts();
        assert!(report.has_conflicts);
        assert_eq!(report.conflicts.len(), 1);
        assert_eq!(report.conflicts[0].conflicting_outputs.len(), 2);
    }

    #[test]
    fn test_no_conflicts() {
        let mut config = create_test_config();

        config.add_assignment(DigitalOutputType::InjectorOutput { number: 1 }, "A1".to_string()).unwrap();
        config.add_assignment(DigitalOutputType::InjectorOutput { number: 2 }, "A2".to_string()).unwrap();
        config.add_assignment(DigitalOutputType::FuelPumpOutput, "B1".to_string()).unwrap();

        let report = config.detect_conflicts();
        assert!(!report.has_conflicts);
        assert_eq!(report.conflicts.len(), 0);
    }

    #[test]
    fn test_remove_assignment() {
        let mut config = create_test_config();

        config.add_assignment(DigitalOutputType::InjectorOutput { number: 1 }, "A1".to_string()).unwrap();
        assert_eq!(config.assignments.len(), 1);

        config.remove_assignment(&DigitalOutputType::InjectorOutput { number: 1 });
        assert_eq!(config.assignments.len(), 0);
    }

    #[test]
    fn test_modify_assignment() {
        let mut config = create_test_config();

        config.add_assignment(DigitalOutputType::InjectorOutput { number: 1 }, "A1".to_string()).unwrap();

        let result = config.modify_assignment(DigitalOutputType::InjectorOutput { number: 1 }, "A2".to_string());
        assert!(result.is_ok());
        assert_eq!(config.assignments[0].pin_id, "A2");
    }

    #[test]
    fn test_modify_nonexistent() {
        let mut config = create_test_config();

        let result = config.modify_assignment(DigitalOutputType::InjectorOutput { number: 1 }, "A1".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_group_by_category() {
        let mut config = create_test_config();

        config.add_assignment(DigitalOutputType::InjectorOutput { number: 1 }, "A1".to_string()).unwrap();
        config.add_assignment(DigitalOutputType::IgnitionOutput { number: 1 }, "A2".to_string()).unwrap();
        config.add_assignment(DigitalOutputType::FuelPumpOutput, "B1".to_string()).unwrap();

        let grouped = config.assignments_by_category();
        assert!(grouped.contains_key("Fuel Control"));
        assert!(grouped.contains_key("Ignition Control"));
        assert_eq!(grouped["Fuel Control"].len(), 2); // Injector + FuelPump
    }

    #[test]
    fn test_output_type_names() {
        assert_eq!(DigitalOutputType::InjectorOutput { number: 1 }.name(), "Injector 1");
        assert_eq!(DigitalOutputType::IgnitionOutput { number: 2 }.name(), "Ignition Output 2");
        assert_eq!(DigitalOutputType::FuelPumpOutput.name(), "Fuel Pump");
        assert_eq!(DigitalOutputType::TachOutput.name(), "Tachometer");
    }
}
