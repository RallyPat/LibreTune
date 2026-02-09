
#[cfg(test)]
mod tests {
    use libretune_core::action_scripting::{Action, ActionSet, ActionPlayer, ActionMetadata};
    use libretune_core::ini::{EcuDefinition, TableDefinition, Constant, TableType, ControllerCommand, CommandPart};
    use libretune_core::ini::DataType;

    #[test]
    fn test_validate_action_set_valid() {
        let mut def = EcuDefinition::default();
        
        // Setup Table
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
            up_color: None,
            down_color: None,
            grid_height: None,
            grid_orient: None,
            help: None,
            x_label: None,
            y_label: None,
        };
        def.tables.insert("veTable1".to_string(), table);
        
        // Setup Constant
        let constant = Constant::new("RevLim", 1, 0, DataType::U16);
        def.constants.insert("RevLim".to_string(), constant);
    
        // Setup Command
        let cmd = ControllerCommand {
            name: "burn".to_string(),
            label: "Burn".to_string(),
            parts: vec![CommandPart::Raw("B".to_string())],
            enable_condition: None
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
    
    #[test]
    fn test_validate_action_set_invalid() {
        let mut def = EcuDefinition::default();
        
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
