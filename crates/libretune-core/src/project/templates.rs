//! Project Templates
//!
//! Built-in templates for common ECU configurations to simplify project creation.
//! Templates include pre-configured INI references, connection settings, and baseline tunes.

use serde::{Deserialize, Serialize};

/// A project template with pre-configured settings for a specific ECU type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTemplate {
    /// Unique identifier for the template
    pub id: String,
    /// Display name for the template
    pub name: String,
    /// Description of what this template is for
    pub description: String,
    /// ECU type/brand (e.g., "Speeduino", "rusEFI", "epicEFI")
    pub ecu_type: String,
    /// ECU signature to match INI files
    pub ini_signature: String,
    /// Suggested INI filename pattern (for finding in repository)
    pub ini_pattern: String,
    /// Default connection settings
    pub connection: TemplateConnection,
    /// Default dashboard preset name
    pub dashboard_preset: String,
    /// Icon identifier for UI display
    pub icon: String,
    /// Baseline constant values (name -> value as string)
    pub baseline_constants: Vec<(String, String)>,
}

/// Connection settings for a template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateConnection {
    /// Default baud rate
    pub baud_rate: u32,
    /// Default timeout in milliseconds
    pub timeout_ms: u64,
    /// Protocol type hint
    pub protocol: String,
}

/// Manager for project templates
pub struct TemplateManager;

impl TemplateManager {
    /// Get all built-in project templates
    pub fn list_templates() -> Vec<ProjectTemplate> {
        vec![
            Self::speeduino_4cyl_template(),
            Self::rusefi_proteus_template(),
            Self::epicefi_template(),
        ]
    }

    /// Get a specific template by ID
    pub fn get_template(id: &str) -> Option<ProjectTemplate> {
        Self::list_templates().into_iter().find(|t| t.id == id)
    }

    /// Speeduino 4-cylinder naturally aspirated template
    fn speeduino_4cyl_template() -> ProjectTemplate {
        ProjectTemplate {
            id: "speeduino-4cyl-na".to_string(),
            name: "Speeduino 4-Cylinder NA".to_string(),
            description: "Standard 4-cylinder naturally aspirated engine with Speeduino ECU. \
                          Includes safe baseline fuel and ignition maps."
                .to_string(),
            ecu_type: "Speeduino".to_string(),
            ini_signature: "speeduino 202310".to_string(),
            ini_pattern: "speeduino*.ini".to_string(),
            connection: TemplateConnection {
                baud_rate: 115200,
                timeout_ms: 1000,
                protocol: "speeduino".to_string(),
            },
            dashboard_preset: "Basic".to_string(),
            icon: "speeduino".to_string(),
            baseline_constants: vec![
                ("nCylinders".to_string(), "4".to_string()),
                ("engineType".to_string(), "0".to_string()), // Even fire
                ("twoStroke".to_string(), "0".to_string()),  // 4-stroke
                ("nInjectors".to_string(), "4".to_string()),
                ("injType".to_string(), "1".to_string()), // Saturated
                ("algorithm".to_string(), "0".to_string()), // Speed density
                ("strokes".to_string(), "1".to_string()), // 4-stroke
            ],
        }
    }

    /// rusEFI Proteus F4 template
    fn rusefi_proteus_template() -> ProjectTemplate {
        ProjectTemplate {
            id: "rusefi-proteus-f4".to_string(),
            name: "rusEFI Proteus F4".to_string(),
            description: "rusEFI Proteus board with STM32F4 processor. \
                          Versatile ECU for various engine configurations."
                .to_string(),
            ecu_type: "rusEFI".to_string(),
            ini_signature: "rusEFI master".to_string(),
            ini_pattern: "rusEFI*proteus*.ini".to_string(),
            connection: TemplateConnection {
                baud_rate: 115200,
                timeout_ms: 1000,
                protocol: "rusefi".to_string(),
            },
            dashboard_preset: "Tuning".to_string(),
            icon: "rusefi".to_string(),
            baseline_constants: vec![
                ("cylindersCount".to_string(), "4".to_string()),
                ("firingOrder".to_string(), "0".to_string()), // 1-3-4-2
                ("injectionMode".to_string(), "1".to_string()), // Sequential
                ("ignitionMode".to_string(), "2".to_string()), // Wasted spark
            ],
        }
    }

    /// epicEFI template
    fn epicefi_template() -> ProjectTemplate {
        ProjectTemplate {
            id: "epicefi-standard".to_string(),
            name: "epicEFI".to_string(),
            description: "epicEFI ECU based on rusEFI. \
                          Compatible with rusEFI tune format and features."
                .to_string(),
            ecu_type: "epicEFI".to_string(),
            ini_signature: "rusEFI epicECU".to_string(),
            ini_pattern: "rusEFI*epicECU*.ini".to_string(),
            connection: TemplateConnection {
                baud_rate: 115200,
                timeout_ms: 1000,
                protocol: "rusefi".to_string(),
            },
            dashboard_preset: "Tuning".to_string(),
            icon: "epicefi".to_string(),
            baseline_constants: vec![
                ("cylindersCount".to_string(), "4".to_string()),
                ("firingOrder".to_string(), "0".to_string()),
                ("injectionMode".to_string(), "1".to_string()),
                ("ignitionMode".to_string(), "2".to_string()),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_templates() {
        let templates = TemplateManager::list_templates();
        assert_eq!(templates.len(), 3);

        let ids: Vec<&str> = templates.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"speeduino-4cyl-na"));
        assert!(ids.contains(&"rusefi-proteus-f4"));
        assert!(ids.contains(&"epicefi-standard"));
    }

    #[test]
    fn test_get_template() {
        let template = TemplateManager::get_template("speeduino-4cyl-na");
        assert!(template.is_some());

        let t = template.unwrap();
        assert_eq!(t.name, "Speeduino 4-Cylinder NA");
        assert_eq!(t.ecu_type, "Speeduino");
        assert_eq!(t.connection.baud_rate, 115200);
    }

    #[test]
    fn test_template_not_found() {
        let template = TemplateManager::get_template("nonexistent");
        assert!(template.is_none());
    }
}
