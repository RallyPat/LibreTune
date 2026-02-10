//! Action Recording and Playback System
//!
//! This module provides functionality to record, store, and replay ECU tuning actions.
//! Actions are discrete operations like table edits, constant changes, or bulk operations.
//! They can be serialized to JSON for sharing and reuse as templates.

use crate::ini::EcuDefinition;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a single tuning action that can be recorded and replayed
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Action {
    /// Edit a single cell in a table
    TableEdit {
        table_name: String,
        x_index: u16,
        y_index: u16,
        new_value: f64,
        old_value: Option<f64>,
    },
    /// Modify a constant value
    ConstantChange {
        constant_name: String,
        new_value: f64,
        old_value: Option<f64>,
    },
    /// Bulk operation on table cells (scale, smooth, interpolate, etc.)
    BulkOperation {
        operation: String, // "scale", "smooth", "interpolate", "set_equal", "rebin"
        table_name: String,
        cells: Vec<(u16, u16)>,           // (x, y) coordinates
        parameters: HashMap<String, f64>, // operation-specific parameters
        old_values: Option<Vec<f64>>,     // for undo
    },
    /// Execute a Lua script for custom calculations
    ExecuteLuaScript { script: String, description: String },
    /// Pause/delay during playback (in milliseconds)
    Pause { duration_ms: u32 },
    /// Send a controller command (e.g. "burn", "reset")
    SendCommand { command: String },
}

/// A recorded or created sequence of actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionSet {
    /// Unique identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description of what this action set does
    pub description: String,
    /// Version of the action set
    pub version: String,
    /// Ordered sequence of actions
    pub actions: Vec<Action>,
    /// Metadata about creation and modification
    pub metadata: ActionMetadata,
}

/// Metadata for action sets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionMetadata {
    /// Author/creator name
    pub created_by: String,
    /// ISO 8601 timestamp
    pub created_at: String,
    /// Last modified timestamp
    pub modified_at: String,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Expected ECU type(s) (Speeduino, rusEFI, FOME, etc.)
    pub compatible_ecus: Vec<String>,
}

/// Recording context for capturing actions
#[derive(Debug)]
pub struct ActionRecorder {
    current_recording: Option<ActionSet>,
}

impl ActionRecorder {
    /// Create a new action recorder
    pub fn new() -> Self {
        Self {
            current_recording: None,
        }
    }
}

impl Default for ActionRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionRecorder {
    /// Start recording a new action set
    pub fn start_recording(
        &mut self,
        name: String,
        description: String,
        created_by: String,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        self.current_recording = Some(ActionSet {
            id: id.clone(),
            name,
            description,
            version: "1.0".to_string(),
            actions: Vec::new(),
            metadata: ActionMetadata {
                created_by,
                created_at: now.clone(),
                modified_at: now,
                tags: vec![],
                compatible_ecus: vec![],
            },
        });

        id
    }

    /// Stop recording and return the action set
    pub fn stop_recording(&mut self) -> Option<ActionSet> {
        self.current_recording.take()
    }

    /// Record an action
    pub fn record_action(&mut self, action: Action) -> Result<(), String> {
        match &mut self.current_recording {
            Some(set) => {
                set.actions.push(action);
                Ok(())
            }
            None => Err("No recording in progress".to_string()),
        }
    }

    /// Get the currently recording action set
    pub fn get_current(&self) -> Option<&ActionSet> {
        self.current_recording.as_ref()
    }

    /// Check if recording is active
    pub fn is_recording(&self) -> bool {
        self.current_recording.is_some()
    }

    /// Add a tag to the current recording
    pub fn add_tag(&mut self, tag: String) -> Result<(), String> {
        match &mut self.current_recording {
            Some(set) => {
                set.metadata.tags.push(tag);
                Ok(())
            }
            None => Err("No recording in progress".to_string()),
        }
    }

    /// Set compatible ECU types for the current recording
    pub fn set_compatible_ecus(&mut self, ecus: Vec<String>) -> Result<(), String> {
        match &mut self.current_recording {
            Some(set) => {
                set.metadata.compatible_ecus = ecus;
                Ok(())
            }
            None => Err("No recording in progress".to_string()),
        }
    }
}

/// Player for executing recorded action sets
pub struct ActionPlayer;

impl ActionPlayer {
    /// Execute an action set
    ///
    /// This is a simulated execution that validates actions.
    /// Actual execution requires integration with the ECU communication layer.
    pub fn validate_action_set(
        action_set: &ActionSet,
        def: Option<&EcuDefinition>,
    ) -> Result<Vec<String>, Vec<String>> {
        let mut warnings = Vec::new();
        let mut errors = Vec::new();

        for (idx, action) in action_set.actions.iter().enumerate() {
            match action {
                Action::TableEdit {
                    table_name,
                    x_index: _,
                    y_index: _,
                    new_value,
                    ..
                } => {
                    if table_name.is_empty() {
                        errors.push(format!("Action {}: Empty table name", idx));
                    }
                    if new_value.is_nan() || new_value.is_infinite() {
                        errors.push(format!("Action {}: Invalid value {}", idx, new_value));
                    }
                    if let Some(d) = def {
                        if !d.tables.contains_key(table_name)
                            && !d.table_map_to_name.contains_key(table_name)
                        {
                            errors.push(format!(
                                "Action {}: Table '{}' not found in ECU definition",
                                idx, table_name
                            ));
                        }
                    }
                }
                Action::ConstantChange {
                    constant_name,
                    new_value,
                    ..
                } => {
                    if constant_name.is_empty() {
                        errors.push(format!("Action {}: Empty constant name", idx));
                    }
                    if new_value.is_nan() || new_value.is_infinite() {
                        errors.push(format!("Action {}: Invalid value {}", idx, new_value));
                    }
                    if let Some(d) = def {
                        if !d.constants.contains_key(constant_name) {
                            errors.push(format!(
                                "Action {}: Constant '{}' not found in ECU definition",
                                idx, constant_name
                            ));
                        }
                    }
                }
                Action::BulkOperation {
                    operation,
                    table_name,
                    cells,
                    parameters,
                    ..
                } => {
                    if table_name.is_empty() {
                        errors.push(format!("Action {}: Empty table name", idx));
                    }
                    if cells.is_empty() {
                        warnings.push(format!(
                            "Action {}: No cells selected for {}",
                            idx, operation
                        ));
                    }
                    if let Some(d) = def {
                        if !d.tables.contains_key(table_name)
                            && !d.table_map_to_name.contains_key(table_name)
                        {
                            errors.push(format!(
                                "Action {}: Table '{}' not found in ECU definition",
                                idx, table_name
                            ));
                        }
                    }
                    match operation.as_str() {
                        "scale" => {
                            if !parameters.contains_key("factor") {
                                errors.push(format!(
                                    "Action {}: scale missing 'factor' parameter",
                                    idx
                                ));
                            }
                        }
                        "rebin" => {
                            if !parameters.contains_key("x_bins")
                                || !parameters.contains_key("y_bins")
                            {
                                errors
                                    .push(format!("Action {}: rebin missing bin parameters", idx));
                            }
                        }
                        _ => {} // Other operations
                    }
                }
                Action::ExecuteLuaScript { script, .. } => {
                    if script.is_empty() {
                        errors.push(format!("Action {}: Empty Lua script", idx));
                    }
                }
                Action::Pause { duration_ms } => {
                    if *duration_ms == 0 {
                        warnings.push(format!("Action {}: Zero-duration pause", idx));
                    }
                }
                Action::SendCommand { command } => {
                    if command.is_empty() {
                        errors.push(format!("Action {}: Empty command", idx));
                    } else if let Some(d) = def {
                        if !d.controller_commands.contains_key(command) {
                            errors.push(format!(
                                "Action {}: Command '{}' not supported by ECU definition",
                                idx, command
                            ));
                        }
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(warnings)
        } else {
            Err(errors)
        }
    }

    /// Get a summary of actions in a set
    pub fn summarize(action_set: &ActionSet) -> String {
        let mut summary = format!("Action Set: {}\n", action_set.name);
        summary.push_str(&format!("Actions: {}\n", action_set.actions.len()));

        let mut counts = HashMap::new();
        for action in &action_set.actions {
            let action_type = match action {
                Action::TableEdit { .. } => "TableEdit",
                Action::ConstantChange { .. } => "ConstantChange",
                Action::BulkOperation { .. } => "BulkOperation",
                Action::ExecuteLuaScript { .. } => "ExecuteLuaScript",
                Action::Pause { .. } => "Pause",
                Action::SendCommand { .. } => "SendCommand",
            };
            *counts.entry(action_type).or_insert(0) += 1;
        }

        for (action_type, count) in counts {
            summary.push_str(&format!("  - {}: {}\n", action_type, count));
        }

        summary
    }
}

/// Serialize an action set to JSON string
pub fn serialize_action_set(action_set: &ActionSet) -> Result<String, String> {
    serde_json::to_string_pretty(action_set)
        .map_err(|e| format!("Failed to serialize action set: {}", e))
}

/// Deserialize an action set from JSON string
pub fn deserialize_action_set(json: &str) -> Result<ActionSet, String> {
    serde_json::from_str(json).map_err(|e| format!("Failed to deserialize action set: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recorder_start_stop() {
        let mut recorder = ActionRecorder::new();
        assert!(!recorder.is_recording());

        let id = recorder.start_recording(
            "Test Set".to_string(),
            "Testing action recording".to_string(),
            "test_user".to_string(),
        );

        assert!(recorder.is_recording());
        assert!(!id.is_empty());

        let action_set = recorder.stop_recording();
        assert!(action_set.is_some());
        assert!(!recorder.is_recording());
    }

    #[test]
    fn test_record_action() {
        let mut recorder = ActionRecorder::new();
        recorder.start_recording(
            "Test".to_string(),
            "Test".to_string(),
            "test_user".to_string(),
        );

        let action = Action::ConstantChange {
            constant_name: "AFR_Target".to_string(),
            new_value: 14.5,
            old_value: Some(14.0),
        };

        assert!(recorder.record_action(action).is_ok());
        assert_eq!(recorder.get_current().unwrap().actions.len(), 1);
    }

    #[test]
    fn test_record_without_start_fails() {
        let mut recorder = ActionRecorder::new();
        let action = Action::Pause { duration_ms: 100 };

        assert!(recorder.record_action(action).is_err());
    }

    #[test]
    fn test_validate_empty_action_set() {
        let set = ActionSet {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "Test".to_string(),
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

        let result = ActionPlayer::validate_action_set(&set, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_invalid_value() {
        let set = ActionSet {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "Test".to_string(),
            version: "1.0".to_string(),
            actions: vec![Action::ConstantChange {
                constant_name: "AFR".to_string(),
                new_value: f64::NAN,
                old_value: None,
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
    fn test_serialize_deserialize() {
        let mut recorder = ActionRecorder::new();
        recorder.start_recording(
            "Test Set".to_string(),
            "Testing serialization".to_string(),
            "test_user".to_string(),
        );

        recorder
            .record_action(Action::ConstantChange {
                constant_name: "AFR".to_string(),
                new_value: 14.5,
                old_value: Some(14.0),
            })
            .unwrap();

        let original = recorder.stop_recording().unwrap();
        let json = serialize_action_set(&original).unwrap();
        let deserialized = deserialize_action_set(&json).unwrap();

        assert_eq!(original.name, deserialized.name);
        assert_eq!(original.actions.len(), deserialized.actions.len());
    }

    #[test]
    fn test_action_set_tags() {
        let mut recorder = ActionRecorder::new();
        recorder.start_recording(
            "Test".to_string(),
            "Test".to_string(),
            "test_user".to_string(),
        );

        assert!(recorder.add_tag("na_tuning".to_string()).is_ok());
        assert!(recorder.add_tag("fuel".to_string()).is_ok());

        let set = recorder.stop_recording().unwrap();
        assert_eq!(set.metadata.tags.len(), 2);
    }

    #[test]
    fn test_compatible_ecus() {
        let mut recorder = ActionRecorder::new();
        recorder.start_recording(
            "Test".to_string(),
            "Test".to_string(),
            "test_user".to_string(),
        );

        let ecus = vec!["Speeduino".to_string(), "rusEFI".to_string()];
        assert!(recorder.set_compatible_ecus(ecus).is_ok());

        let set = recorder.stop_recording().unwrap();
        assert_eq!(set.metadata.compatible_ecus.len(), 2);
    }

    #[test]
    fn test_action_summarize() {
        let mut set = ActionSet {
            id: "test".to_string(),
            name: "Test Actions".to_string(),
            description: "Test".to_string(),
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

        set.actions.push(Action::ConstantChange {
            constant_name: "AFR".to_string(),
            new_value: 14.5,
            old_value: None,
        });
        set.actions.push(Action::Pause { duration_ms: 100 });

        let summary = ActionPlayer::summarize(&set);
        assert!(summary.contains("Test Actions"));
        assert!(summary.contains("2"));
    }
}
