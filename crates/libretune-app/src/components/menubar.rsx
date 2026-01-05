//! MenuBar Component
//!
//! Top-level application menubar following TunerStudio menu structure:
//! File | Edit | View | Tools | Communications | Help

use crate::menus::{MenuItem, MenuGroup};

/// Menubar component
pub struct MenuBar {
    pub menu_items: Vec<MenuBarItem>,
}

/// Menubar item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MenuBarItem {
    /// File menu items
    #[serde(rename = "file")]
    File(FileItem),
    
    /// Edit menu items
    #[serde(rename = "edit")]
    Edit(EditItem),
    
    /// View menu items
    #[serde(rename = "view")]
    View(ViewItem),
    
    /// Tools menu items
    #[serde(rename = "tools")]
    Tools(ToolsItem),
    
    /// Communications menu items
    #[serde(rename = "comms")]
    Comms(CommsItem),
    
    /// Help menu items
    #[serde(rename = "help")]
    Help(HelpItem),
}

/// File menu items
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "file")]
pub enum FileItem {
    #[serde(rename = "new_project")]
    NewProject,
    #[serde(rename = "open_project")]
    OpenProject,
    #[serde(rename = "save")]
    Save,
    #[serde(rename = "save_as")]
    SaveAs,
    #[serde(rename = "load_dialog_tune")]
    LoadDialogTune,
    #[serde(rename = "burn_to_ecu")]
    BurnToEcu,
    #[serde(rename = "exit")]
    Exit,
}

/// Edit menu items
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "edit")]
pub enum EditItem {
    #[serde(rename = "undo")]
    Undo,
    #[serde(rename = "redo")]
    Redo,
    #[serde(rename = "copy")]
    Copy,
    #[serde(rename = "paste")]
    Paste,
    #[serde(rename = "cut")]
    Cut,
    #[serde(rename = "select_all")]
    SelectAll,
}

/// View menu items
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "view")]
pub enum ViewItem {
    #[serde(rename = "realtime_display")]
    RealtimeDisplay,
    #[serde(rename = "table_2d")]
    Table2D,
    #[serde(rename = "table_3d")]
    Table3D,
    #[serde(rename = "fullscreen")]
    Fullscreen,
    #[serde(rename = "tabbed_dashboards")]
    TabbedDashboards,
}

/// Tools menu items
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tools")]
pub enum ToolsItem {
    #[serde(rename = "ve_analyze_live")]
    VEAnalyzeLive,
    #[serde(rename = "wue_analyze_live")]
    WUEAnalyzeLive,
    #[serde(rename = "tuning_aids")]
    TuningAids,
    #[serde(rename = "output_test_mode")]
    OutputTestMode,
}

/// Communications menu items
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "comms")]
pub enum CommsItem {
    #[serde(rename = "connect")]
    Connect,
    #[serde(rename = "disconnect")]
    Disconnect,
    #[serde(rename = "port_selection")]
    PortSelection,
    #[serde(rename = "baud_rate")]
    BaudRate,
}

/// Help menu items
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "help")]
pub enum HelpItem {
    #[serde(rename = "firmware_help")]
    FirmwareHelp,
    #[serde(rename = "enter_registration")]
    EnterRegistration,
    #[serde(rename = "manuals")]
    Manuals,
}
