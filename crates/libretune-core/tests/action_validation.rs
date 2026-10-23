//! Action-set validation tests.
//!
//! `ActionPlayer::validate_action_set` checks that every action in a recorded
//! set references objects that actually exist in the target ECU definition
//! (tables, constants, controller commands). These tests build a minimal
//! `EcuDefinition` fixture with exactly one of each, then exercise both the
//! happy path (all references resolve) and the failure path (every reference
//! is missing) so that each error message is asserted individually.

#[cfg(test)]
mod tests {
    #![allow(clippy::vec_init_then_push)]
    use libretune_core::action_scripting::{Action, ActionMetadata, ActionPlayer, ActionSet};
    use libretune_core::ini::DataType;
    use libretune_core::ini::{
        CommandPart, Constant, ControllerCommand, EcuDefinition, TableDefinition, TableType,
    };

    #[test]
    fn test_validate_action_set_valid() {
        let mut def = EcuDefinition::default();

        // Minimal definition with one table, one constant, one command —
        // exactly the surface the validator can resolve. Each is referenced
        // by an action below.
        let table = TableDefinition {
            name: "veTable1".to_string(),
            map_name: None,
            title: "VE Table".to_string(),
            table_type: TableType::ThreeD,
            map: "veMap".to_string(),
            x_bins: "rpmBins".to_string(),
            x_output_channel: None,
            y_bins: Some("loadBins".to_string()),
            y_output_channel: None,
            page: 1,
            x_size: 16,
            y_size: 16,
            up_label: None,
            down_label: None,
            grid_height: None,
            grid_orient: None,
            help: None,
            x_label: None,
            y_label: None,
            ..Default::default()
        };
        def.tables.insert("veTable1".to_string(), table);

        // A U16 constant (e.g. rev limit RPM) at page 1, offset 0. Set a
        // realistic max so the proposed value below passes the new bounds
        // check (the validator now enforces Constant.min/max, added for the
        // AI-assistant feature).
        let mut constant = Constant::new("RevLim", 1, 0, DataType::U16);
        constant.max = 10000.0;
        constant.units = "RPM".to_string();
        def.constants.insert("RevLim".to_string(), constant);

        // A controller command that burns the tune to flash (raw byte 'B').
        let cmd = ControllerCommand {
            name: "burn".to_string(),
            label: "Burn".to_string(),
            parts: vec![CommandPart::Raw("B".to_string())],
            enable_condition: None,
        };
        def.controller_commands.insert("burn".to_string(), cmd);

        let mut actions = Vec::new();
        actions.push(Action::TableEdit {
            table_name: "veTable1".to_string(),
            x_index: 0,
            y_index: 0,
            new_value: 10.0,
            old_value: None,
        });
        actions.push(Action::ConstantChange {
            constant_name: "RevLim".to_string(),
            new_value: 6500.0,
            old_value: None,
        });
        actions.push(Action::SendCommand {
            command: "burn".to_string(),
        });

        let metadata = ActionMetadata {
            created_by: "tester".to_string(),
            created_at: "".to_string(),
            modified_at: "".to_string(),
            tags: vec![],
            compatible_ecus: vec![],
        };

        let set = ActionSet {
            id: "test".to_string(),
            name: "Test Set".to_string(),
            description: "".to_string(),
            version: "1.0".to_string(),
            actions,
            metadata,
        };

        let result = ActionPlayer::validate_action_set(&set, Some(&def));
        assert!(result.is_ok(), "Validation failed: {:?}", result.err());
    }

    /// Every action references a name that does NOT exist in the (empty)
    /// default definition, so validation must fail and the error must call out
    /// each missing reference by name. The default `EcuDefinition` has empty
    /// tables/constants/commands maps, which is why this needs no fixture.
    #[test]
    fn test_validate_action_set_invalid() {
        let def = EcuDefinition::default();

        let mut actions = Vec::new();
        actions.push(Action::TableEdit {
            table_name: "MissingTable".to_string(),
            x_index: 0,
            y_index: 0,
            new_value: 10.0,
            old_value: None,
        });
        actions.push(Action::ConstantChange {
            constant_name: "MissingConst".to_string(),
            new_value: 1.0,
            old_value: None,
        });
        actions.push(Action::SendCommand {
            command: "MissingCmd".to_string(),
        });

        let metadata = ActionMetadata {
            created_by: "tester".to_string(),
            created_at: "".to_string(),
            modified_at: "".to_string(),
            tags: vec![],
            compatible_ecus: vec![],
        };

        let set = ActionSet {
            id: "test".to_string(),
            name: "Test Set".to_string(),
            description: "".to_string(),
            version: "1.0".to_string(),
            actions,
            metadata,
        };

        let result = ActionPlayer::validate_action_set(&set, Some(&def));
        assert!(result.is_err());
        let msg = format!("{:?}", result.unwrap_err());
        assert!(msg.contains("Table 'MissingTable' not found"));
        assert!(msg.contains("Constant 'MissingConst' not found"));
        assert!(msg.contains("Command 'MissingCmd' not supported"));
    }
}
