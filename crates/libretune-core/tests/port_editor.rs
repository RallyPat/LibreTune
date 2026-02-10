//! Tests for port editor module

#[cfg(test)]
mod tests {
    use libretune_core::port_editor::{DigitalOutputType, EcuPin, PortEditorConfig};

    fn create_mock_pins() -> Vec<EcuPin> {
        vec![
            EcuPin {
                pin_id: "P0.0".to_string(),
                pin_name: "Port 0, Pin 0".to_string(),
                is_available: true,
                description: "Injector 1".to_string(),
            },
            EcuPin {
                pin_id: "P0.1".to_string(),
                pin_name: "Port 0, Pin 1".to_string(),
                is_available: true,
                description: "Injector 2".to_string(),
            },
            EcuPin {
                pin_id: "P0.2".to_string(),
                pin_name: "Port 0, Pin 2".to_string(),
                is_available: true,
                description: "Ignition 1".to_string(),
            },
            EcuPin {
                pin_id: "P0.3".to_string(),
                pin_name: "Port 0, Pin 3".to_string(),
                is_available: true,
                description: "Ignition 2".to_string(),
            },
            EcuPin {
                pin_id: "P1.5".to_string(),
                pin_name: "Port 1, Pin 5".to_string(),
                is_available: true,
                description: "Fuel Pump".to_string(),
            },
        ]
    }

    #[test]
    fn test_create_port_config() {
        let pins = create_mock_pins();
        let config = PortEditorConfig::new(pins.clone());

        assert_eq!(config.available_pins.len(), 5);
        assert_eq!(config.assignments.len(), 0);
    }

    #[test]
    fn test_add_injector_assignment() {
        let pins = create_mock_pins();
        let mut config = PortEditorConfig::new(pins);

        let result = config.add_assignment(
            DigitalOutputType::InjectorOutput { number: 1 },
            "P0.0".to_string(),
        );

        assert!(result.is_ok());
        assert_eq!(config.assignments.len(), 1);
        assert!(config.assignments[0].enabled);
    }

    #[test]
    fn test_add_multiple_injectors() {
        let pins = create_mock_pins();
        let mut config = PortEditorConfig::new(pins);

        config
            .add_assignment(
                DigitalOutputType::InjectorOutput { number: 1 },
                "P0.0".to_string(),
            )
            .unwrap();
        config
            .add_assignment(
                DigitalOutputType::InjectorOutput { number: 2 },
                "P0.1".to_string(),
            )
            .unwrap();

        assert_eq!(config.assignments.len(), 2);
    }

    #[test]
    fn test_conflicting_assignments() {
        let pins = create_mock_pins();
        let mut config = PortEditorConfig::new(pins);

        config
            .add_assignment(
                DigitalOutputType::InjectorOutput { number: 1 },
                "P0.0".to_string(),
            )
            .unwrap();
        config
            .add_assignment(
                DigitalOutputType::IgnitionOutput { number: 1 },
                "P0.0".to_string(),
            )
            .unwrap();

        let report = config.detect_conflicts();

        assert!(report.has_conflicts);
        assert_eq!(report.conflicts.len(), 1);
        assert_eq!(report.conflicts[0].pin_id, "P0.0");
        assert_eq!(report.conflicts[0].conflicting_outputs.len(), 2);
    }

    #[test]
    fn test_conflict_detection_disabled() {
        let pins = create_mock_pins();
        let mut config = PortEditorConfig::new(pins);

        config
            .add_assignment(
                DigitalOutputType::InjectorOutput { number: 1 },
                "P0.0".to_string(),
            )
            .unwrap();

        // Disable the assignment - should not appear in conflict detection
        if let Some(assignment) = config.assignments.iter_mut().next() {
            assignment.enabled = false;
        }

        config
            .add_assignment(DigitalOutputType::FuelPumpOutput, "P0.0".to_string())
            .unwrap();

        let report = config.detect_conflicts();

        assert!(!report.has_conflicts);
    }

    #[test]
    fn test_invalid_pin() {
        let pins = create_mock_pins();
        let mut config = PortEditorConfig::new(pins);

        let result = config.add_assignment(
            DigitalOutputType::InjectorOutput { number: 1 },
            "P9.9".to_string(),
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[test]
    fn test_duplicate_output() {
        let pins = create_mock_pins();
        let mut config = PortEditorConfig::new(pins);

        config
            .add_assignment(
                DigitalOutputType::InjectorOutput { number: 1 },
                "P0.0".to_string(),
            )
            .unwrap();

        let result = config.add_assignment(
            DigitalOutputType::InjectorOutput { number: 1 },
            "P0.1".to_string(),
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already assigned"));
    }

    #[test]
    fn test_modify_assignment() {
        let pins = create_mock_pins();
        let mut config = PortEditorConfig::new(pins);

        config
            .add_assignment(
                DigitalOutputType::InjectorOutput { number: 1 },
                "P0.0".to_string(),
            )
            .unwrap();

        let result = config.modify_assignment(
            DigitalOutputType::InjectorOutput { number: 1 },
            "P0.1".to_string(),
        );

        assert!(result.is_ok());
        assert_eq!(config.assignments[0].pin_id, "P0.1");
    }

    #[test]
    fn test_modify_nonexistent_assignment() {
        let pins = create_mock_pins();
        let mut config = PortEditorConfig::new(pins);

        let result = config.modify_assignment(
            DigitalOutputType::InjectorOutput { number: 1 },
            "P0.0".to_string(),
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_remove_assignment() {
        let pins = create_mock_pins();
        let mut config = PortEditorConfig::new(pins);

        config
            .add_assignment(
                DigitalOutputType::InjectorOutput { number: 1 },
                "P0.0".to_string(),
            )
            .unwrap();
        assert_eq!(config.assignments.len(), 1);

        config.remove_assignment(&DigitalOutputType::InjectorOutput { number: 1 });

        assert_eq!(config.assignments.len(), 0);
    }

    #[test]
    fn test_get_available_pin_ids() {
        let pins = create_mock_pins();
        let config = PortEditorConfig::new(pins);

        let pin_ids = config.available_pin_ids();

        assert_eq!(pin_ids.len(), 5);
        assert!(pin_ids.contains(&"P0.0".to_string()));
        assert!(pin_ids.contains(&"P1.5".to_string()));
    }

    #[test]
    fn test_assignments_by_category() {
        let pins = create_mock_pins();
        let mut config = PortEditorConfig::new(pins);

        config
            .add_assignment(
                DigitalOutputType::InjectorOutput { number: 1 },
                "P0.0".to_string(),
            )
            .unwrap();
        config
            .add_assignment(
                DigitalOutputType::IgnitionOutput { number: 1 },
                "P0.2".to_string(),
            )
            .unwrap();
        config
            .add_assignment(DigitalOutputType::FuelPumpOutput, "P1.5".to_string())
            .unwrap();

        let grouped = config.assignments_by_category();

        assert!(grouped.contains_key("Fuel Control"));
        assert!(grouped.contains_key("Ignition Control"));
        assert_eq!(grouped["Fuel Control"].len(), 2); // Injector + FuelPump
        assert_eq!(grouped["Ignition Control"].len(), 1);
    }

    #[test]
    fn test_multiple_conflicts_same_pin() {
        let pins = create_mock_pins();
        let mut config = PortEditorConfig::new(pins);

        config
            .add_assignment(
                DigitalOutputType::InjectorOutput { number: 1 },
                "P0.0".to_string(),
            )
            .unwrap();
        config
            .add_assignment(
                DigitalOutputType::InjectorOutput { number: 2 },
                "P0.0".to_string(),
            )
            .unwrap();
        config
            .add_assignment(DigitalOutputType::FuelPumpOutput, "P0.0".to_string())
            .unwrap();

        let report = config.detect_conflicts();

        assert!(report.has_conflicts);
        assert_eq!(report.conflicts[0].conflicting_outputs.len(), 3);
    }

    #[test]
    fn test_all_output_types() {
        let pins = create_mock_pins();
        let mut config = PortEditorConfig::new(pins);

        let outputs = vec![
            (DigitalOutputType::InjectorOutput { number: 1 }, "P0.0"),
            (DigitalOutputType::IgnitionOutput { number: 1 }, "P0.2"),
            (DigitalOutputType::TachOutput, "P0.3"),
            (DigitalOutputType::FuelPumpOutput, "P1.5"),
        ];

        for (output, pin) in outputs {
            config.add_assignment(output, pin.to_string()).ok();
        }

        assert_eq!(config.assignments.len(), 4);

        let report = config.detect_conflicts();
        assert!(!report.has_conflicts);
    }
}
