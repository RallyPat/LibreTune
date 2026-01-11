//! JSON-RPC protocol types for JVM ↔ LibreTune communication

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// JSON-RPC request from LibreTune to JVM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl RpcRequest {
    pub fn new(id: u64, method: &str, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        }
    }
}

/// JSON-RPC response from JVM to LibreTune
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

/// JSON-RPC error object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// JSON-RPC notification (no id, no response expected)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl RpcNotification {
    pub fn new(method: &str, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        }
    }
}

// ============================================================================
// Swing Component Tree Types
// ============================================================================

/// Serialized Swing component tree node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwingComponent {
    /// Unique ID for this component instance
    pub id: String,
    /// Component type (JPanel, JButton, JTextField, etc.)
    pub component_type: String,
    /// Position and size
    pub bounds: ComponentBounds,
    /// Layout manager type if this is a container
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<LayoutInfo>,
    /// Layout constraint for this component within parent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout_constraint: Option<serde_json::Value>,
    /// Component-specific properties
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,
    /// Child components (if container)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<SwingComponent>,
}

/// Component bounds (x, y, width, height)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComponentBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Layout manager information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LayoutInfo {
    #[serde(rename = "BorderLayout")]
    Border { hgap: i32, vgap: i32 },
    #[serde(rename = "GridBagLayout")]
    GridBag,
    #[serde(rename = "FlowLayout")]
    Flow {
        alignment: i32,
        hgap: i32,
        vgap: i32,
    },
    #[serde(rename = "BoxLayout")]
    Box {
        axis: i32, // 0 = X_AXIS, 1 = Y_AXIS
    },
    #[serde(rename = "GridLayout")]
    Grid {
        rows: i32,
        cols: i32,
        hgap: i32,
        vgap: i32,
    },
    #[serde(rename = "CardLayout")]
    Card { hgap: i32, vgap: i32 },
    #[serde(rename = "null")]
    None,
}

/// GridBagConstraints serialized form
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GridBagConstraint {
    pub gridx: i32,
    pub gridy: i32,
    pub gridwidth: i32,
    pub gridheight: i32,
    pub weightx: f64,
    pub weighty: f64,
    pub anchor: i32,
    pub fill: i32,
    pub insets_top: i32,
    pub insets_left: i32,
    pub insets_bottom: i32,
    pub insets_right: i32,
    pub ipadx: i32,
    pub ipady: i32,
}

/// BorderLayout constraint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum BorderConstraint {
    North,
    South,
    East,
    West,
    Center,
}

// ============================================================================
// Incremental Update Types
// ============================================================================

/// Incremental UI diff event
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum UiDiff {
    /// Component added to tree
    Add {
        parent_id: String,
        index: usize,
        component: SwingComponent,
    },
    /// Component removed from tree
    Remove { component_id: String },
    /// Component property changed
    Update {
        component_id: String,
        property: String,
        value: serde_json::Value,
    },
    /// Full tree replacement (initial load or major change)
    Replace { root: SwingComponent },
}

// ============================================================================
// Event Types
// ============================================================================

/// Event from React to JVM (user interaction)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PluginEvent {
    /// Button click or action
    Action { component_id: String },
    /// Text field value changed
    TextChange { component_id: String, text: String },
    /// Checkbox/radio state changed
    StateChange {
        component_id: String,
        selected: bool,
    },
    /// ComboBox selection changed
    ItemSelect {
        component_id: String,
        selected_index: i32,
        selected_item: String,
    },
    /// Slider value changed
    SliderChange { component_id: String, value: i32 },
    /// Table cell edited
    TableEdit {
        component_id: String,
        row: i32,
        column: i32,
        value: String,
    },
    /// Table row selected
    TableSelect {
        component_id: String,
        selected_rows: Vec<i32>,
    },
}

// ============================================================================
// Controller Access Proxy Types
// ============================================================================

/// Output channel data from ECU
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputChannelData {
    pub name: String,
    pub value: f64,
    pub units: String,
    pub min_value: f64,
    pub max_value: f64,
}

/// Controller parameter data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerParameterData {
    pub name: String,
    pub param_class: String, // "scalar", "bits", "array"
    pub units: String,
    pub min: f64,
    pub max: f64,
    pub decimal_places: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scalar_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub array_values: Option<Vec<Vec<f64>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub string_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<[i32; 2]>,
}

/// Plugin metadata extracted from JAR manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub version: String,
    pub plugin_type: String, // "TAB_PANEL", "DIALOG_WIDGET", "PERSISTENT_DIALOG_PANEL"
    pub jar_path: String,
    pub help_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_request_serialization() {
        let req = RpcRequest::new(
            1,
            "loadPlugin",
            Some(serde_json::json!({"path": "/test.jar"})),
        );
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"loadPlugin\""));
    }

    #[test]
    fn test_swing_component_serialization() {
        let comp = SwingComponent {
            id: "btn1".to_string(),
            component_type: "JButton".to_string(),
            bounds: ComponentBounds {
                x: 10,
                y: 20,
                width: 100,
                height: 30,
            },
            layout: None,
            layout_constraint: Some(serde_json::json!("CENTER")),
            properties: [("text".to_string(), serde_json::json!("Click Me"))]
                .into_iter()
                .collect(),
            children: vec![],
        };
        let json = serde_json::to_string_pretty(&comp).unwrap();
        assert!(json.contains("\"componentType\": \"JButton\""));
        assert!(json.contains("\"text\": \"Click Me\""));
    }

    #[test]
    fn test_ui_diff_serialization() {
        let diff = UiDiff::Update {
            component_id: "txt1".to_string(),
            property: "text".to_string(),
            value: serde_json::json!("New Value"),
        };
        let json = serde_json::to_string(&diff).unwrap();
        assert!(json.contains("\"action\":\"update\""));
    }
}
