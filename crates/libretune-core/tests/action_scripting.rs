//! Tests for action scripting system

#[cfg(test)]
mod tests {
    use libretune_core::action_scripting::{
        deserialize_action_set, serialize_action_set, Action, ActionMetadata, ActionPlayer,
        ActionRecorder, ActionSet,
    };
    use std::collections::HashMap;

    #[test]
    fn test_record_table_edit_action() {
        let mut recorder = ActionRecorder::new();
        recorder.start_recording(
            "Table Edit Test".to_string(),
            "Testing table edit recording".to_string(),
            "test_user".to_string(),
        );

        let action = Action::TableEdit {
            table_name: "veTable1".to_string(),
            x_index: 5,
            y_index: 3,
            new_value: 75.5,
            old_value: Some(74.2),
        };

        assert!(recorder.record_action(action).is_ok());

        let set = recorder.stop_recording().unwrap();
        assert_eq!(set.actions.len(), 1);
        assert_eq!(set.name, "Table Edit Test");
    }

    #[test]
    fn test_record_bulk_operation() {
        let mut recorder = ActionRecorder::new();
        recorder.start_recording(
            "Bulk Op Test".to_string(),
            "Testing bulk operations".to_string(),
            "test_user".to_string(),
        );

        let mut params = HashMap::new();
        params.insert("factor".to_string(), 1.05);

        let action = Action::BulkOperation {
            operation: "scale".to_string(),
            table_name: "veTable1".to_string(),
            cells: vec![(0, 0), (1, 1), (2, 2)],
            parameters: params,
            old_values: Some(vec![75.0, 76.0, 77.0]),
        };

        assert!(recorder.record_action(action).is_ok());

        let set = recorder.stop_recording().unwrap();
        assert_eq!(set.actions.len(), 1);
    }

    #[test]
    fn test_record_lua_script_action() {
        let mut recorder = ActionRecorder::new();
        recorder.start_recording(
            "Lua Script Test".to_string(),
            "Testing Lua execution".to_string(),
            "test_user".to_string(),
        );

        let action = Action::ExecuteLuaScript {
            script: r#"
                -- Calculate new AFR based on load
                local new_afr = 14.0 + (load * 0.5)
                return new_afr
            "#
            .to_string(),
            description: "Calculate AFR based on load".to_string(),
        };

        assert!(recorder.record_action(action).is_ok());

        let set = recorder.stop_recording().unwrap();
        assert_eq!(set.actions.len(), 1);
    }

    #[test]
    fn test_validate_scale_operation() {
        let mut params = HashMap::new();
        params.insert("factor".to_string(), 1.05);

        let set = ActionSet {
            id: "test".to_string(),
            name: "Scale Operation".to_string(),
            description: "Test scaling validation".to_string(),
            version: "1.0".to_string(),
            actions: vec![Action::BulkOperation {
                operation: "scale".to_string(),
                table_name: "veTable1".to_string(),
                cells: vec![(0, 0), (1, 1)],
                parameters: params,
                old_values: None,
            }],
            metadata: ActionMetadata {
                created_by: "test".to_string(),
                created_at: "2026-02-04T00:00:00Z".to_string(),
                modified_at: "2026-02-04T00:00:00Z".to_string(),
                tags: vec![],
                compatible_ecus: vec![],
            },
        };

        let result = ActionPlayer::validate_action_set(&set, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_rebin_missing_parameters() {
        let params = HashMap::new(); // Missing x_bins and y_bins

        let set = ActionSet {
            id: "test".to_string(),
            name: "Rebin Missing Params".to_string(),
            description: "Test rebin validation with missing params".to_string(),
            version: "1.0".to_string(),
            actions: vec![Action::BulkOperation {
                operation: "rebin".to_string(),
                table_name: "veTable1".to_string(),
                cells: vec![(0, 0)],
                parameters: params,
                old_values: None,
            }],
            metadata: ActionMetadata {
                created_by: "test".to_string(),
                created_at: "2026-02-04T00:00:00Z".to_string(),
                modified_at: "2026-02-04T00:00:00Z".to_string(),
                tags: vec![],
                compatible_ecus: vec![],
            },
        };

        let result = ActionPlayer::validate_action_set(&set, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_actions_in_sequence() {
        let mut recorder = ActionRecorder::new();
        recorder.start_recording(
            "Multi-Action Sequence".to_string(),
            "Testing sequential actions".to_string(),
            "test_user".to_string(),
        );

        // Lua script action
        recorder
            .record_action(Action::ExecuteLuaScript {
                script: "return 14.5".to_string(),
                description: "Calculate target AFR".to_string(),
            })
            .unwrap();

        // Constant change
        recorder
            .record_action(Action::ConstantChange {
                constant_name: "AFR_target".to_string(),
                new_value: 14.5,
                old_value: Some(14.0),
            })
            .unwrap();

        // Pause
        recorder
            .record_action(Action::Pause { duration_ms: 500 })
            .unwrap();

        // Table edit
        recorder
            .record_action(Action::TableEdit {
                table_name: "veTable1".to_string(),
                x_index: 0,
                y_index: 0,
                new_value: 75.0,
                old_value: Some(74.0),
            })
            .unwrap();

        let set = recorder.stop_recording().unwrap();
        assert_eq!(set.actions.len(), 4);
    }

    #[test]
    fn test_action_serialization_roundtrip() {
        let mut recorder = ActionRecorder::new();
        recorder.start_recording(
            "Serialization Test".to_string(),
            "Testing action set serialization".to_string(),
            "test_user".to_string(),
        );

        recorder
            .record_action(Action::ConstantChange {
                constant_name: "AFR_target".to_string(),
                new_value: 14.5,
                old_value: Some(14.0),
            })
            .unwrap();

        recorder.add_tag("fuel_tuning".to_string()).unwrap();
        recorder
            .set_compatible_ecus(vec!["Speeduino".to_string(), "rusEFI".to_string()])
            .unwrap();

        let original = recorder.stop_recording().unwrap();

        // Serialize
        let json = serialize_action_set(&original).expect("Serialization failed");
        assert!(!json.is_empty());

        // Deserialize
        let restored = deserialize_action_set(&json).expect("Deserialization failed");

        // Verify
        assert_eq!(restored.name, original.name);
        assert_eq!(restored.description, original.description);
        assert_eq!(restored.actions.len(), original.actions.len());
        assert_eq!(restored.metadata.tags.len(), original.metadata.tags.len());
        assert_eq!(
            restored.metadata.compatible_ecus.len(),
            original.metadata.compatible_ecus.len()
        );
    }

    #[test]
    fn test_action_summary() {
        let mut set = ActionSet {
            id: "test".to_string(),
            name: "Summary Test".to_string(),
            description: "Testing summary generation".to_string(),
            version: "1.0".to_string(),
            actions: vec![],
            metadata: ActionMetadata {
                created_by: "test".to_string(),
                created_at: "2026-02-04T00:00:00Z".to_string(),
                modified_at: "2026-02-04T00:00:00Z".to_string(),
                tags: vec![],
                compatible_ecus: vec![],
            },
        };

        // Add various action types
        set.actions.push(Action::ConstantChange {
            constant_name: "AFR".to_string(),
            new_value: 14.5,
            old_value: None,
        });
        set.actions.push(Action::Pause { duration_ms: 100 });
        set.actions.push(Action::Pause { duration_ms: 200 });

        let summary = ActionPlayer::summarize(&set);

        // Verify summary includes expected information
        assert!(summary.contains("Summary Test"));
        assert!(summary.contains("3")); // Total actions
        assert!(summary.contains("ConstantChange")); // Action type
        assert!(summary.contains("Pause")); // Action type
    }

    #[test]
    fn test_recording_without_start() {
        let mut recorder = ActionRecorder::new();

        let action = Action::Pause { duration_ms: 100 };

        let result = recorder.record_action(action);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "No recording in progress");
    }

    #[test]
    fn test_recorder_state_management() {
        let mut recorder = ActionRecorder::new();

        assert!(!recorder.is_recording());
        assert!(recorder.get_current().is_none());

        recorder.start_recording("Test".to_string(), "Test".to_string(), "user".to_string());

        assert!(recorder.is_recording());
        assert!(recorder.get_current().is_some());

        let stopped = recorder.stop_recording();

        assert!(!recorder.is_recording());
        assert!(recorder.get_current().is_none());
        assert!(stopped.is_some());
    }
}
