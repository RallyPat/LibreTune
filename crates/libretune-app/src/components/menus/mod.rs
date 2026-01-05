//! Menu System Components
//!
//! Implements menu system with:
//! - Top-level menubar (MenuBar)
//! - Menu groups with collapsible sections
//! - Individual menu items with keyboard shortcuts
//! - MenuManager to parse INI-defined menus

use crate::types::{BinOp, UnaryOp, Expr};

/// Standard menu separators
pub const MENU_SEPARATOR: &str = "std_separator";

/// Menu item type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MenuItemType {
    /// Opens a dialog or performs action
    #[serde(rename = "action")]
    Action,
    /// Links to another menu or submenu
    SubMenu,
    /// Visual separator line
    Separator,
}

/// Single menu item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuItem {
    /// Internal identifier
    pub id: String,
    /// Display label (use '&' for mnemonic)
    pub label: String,
    /// Keyboard shortcut (e.g., "Ctrl+S")
    #[serde(skip_serializing_if = "None")]
    pub shortcut: Option<String>,
    /// Target: dialog name, table name, action ID
    pub target: Option<String>,
    /// Visibility condition expression
    pub condition: Option<String>,
    /// Icon identifier (lucide or component name)
    pub icon: Option<String>,
    /// Children for submenus
    #[serde(skip_serializing_if = "None")]
    pub children: Option<Vec<MenuItem>>,
}

/// Menu group section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuGroup {
    pub name: String,
    pub title: String,
    pub items: Vec<MenuItem>,
    #[serde(default)]
    pub default_expanded: bool,
}

/// Menu item types
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "actionType")]
pub enum MenuActionType {
    /// Opens a built-in dialog
    #[serde(rename = "dialog")]
    Dialog { name: String },
    /// User-defined action
    #[serde(rename = "action")]
    UserAction { id: String, action: Box<dyn Fn() + SendSync> },
    /// Opens a submenu
    SubMenu,
}

impl MenuActionType {
    pub fn is_action(&self) -> bool {
        matches!(self, MenuActionType::Dialog(_) | MenuActionType::UserAction(_))
    }
}
