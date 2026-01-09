//! INI Definition File Parser
//!
//! Parses standard ECU INI definition files that define ECU configurations.
//! These files describe:
//! - ECU signature and version info
//! - Constants (editable parameters)
//! - Output channels (real-time data)
//! - Table editor definitions
//! - Gauge configurations
//! - Menu structure

mod constants;
mod error;
pub mod expression;
mod gauges;
pub mod inc_tables;
mod output_channels;
mod parser;
mod tables;
mod types;

pub use constants::Constant;
pub use error::IniError;
pub use gauges::GaugeConfig;
pub use inc_tables::{IncTable, IncTableCache};
pub use output_channels::OutputChannel;
pub use tables::{CurveDefinition, TableDefinition};
pub use types::*;

use std::collections::HashMap;
use std::path::Path;

/// Complete ECU definition parsed from an INI file
#[derive(Debug, Clone)]
pub struct EcuDefinition {
    /// ECU signature string (e.g., "speeduino 202310")
    pub signature: String,

    /// Query command to retrieve signature
    pub query_command: String,

    /// Display version info
    pub version_info: String,

    /// INI spec version
    pub ini_spec_version: String,

    /// #define macros (name -> list of values)
    /// Used to expand $references in bits field options
    pub defines: HashMap<String, Vec<String>>,

    /// Endianness of ECU data
    pub endianness: Endianness,

    /// Page sizes for ECU memory
    pub page_sizes: Vec<u16>,

    /// Total number of pages
    pub n_pages: u8,

    /// Protocol settings for ECU communication
    pub protocol: ProtocolSettings,

    /// Editable constants/parameters
    pub constants: HashMap<String, Constant>,

    /// Real-time output channels
    pub output_channels: HashMap<String, OutputChannel>,

    /// Table editor definitions
    pub tables: HashMap<String, TableDefinition>,

    /// Lookup map from table map_name to table name
    /// This allows finding tables by either their name or map_name
    pub table_map_to_name: HashMap<String, String>,

    /// Curve editor definitions (2D curves)
    pub curves: HashMap<String, CurveDefinition>,

    /// Gauge configurations
    pub gauges: HashMap<String, GaugeConfig>,

    /// Setting groups for UI organization
    pub setting_groups: HashMap<String, SettingGroup>,

    /// Menu definitions
    pub menus: Vec<Menu>,

    /// Dialog/layout definitions
    pub dialogs: HashMap<String, DialogDefinition>,

    /// Help topic definitions
    pub help_topics: HashMap<String, HelpTopic>,

    /// Datalog output channel selections
    pub datalog_entries: Vec<DatalogEntry>,

    /// PC Variables (like tsCanId) used for variable substitution in commands
    /// Maps variable name -> byte value (e.g., "tsCanId" -> 0x00 for CAN ID 0)
    pub pc_variables: HashMap<String, u8>,

    /// Default values for constants (from [Defaults] section)
    /// Maps constant name -> default value
    pub default_values: HashMap<String, f64>,

    /// FrontPage configuration for default dashboard layout
    pub frontpage: Option<FrontPageConfig>,

    /// Indicator panels (groups of boolean indicators)
    pub indicator_panels: HashMap<String, IndicatorPanel>,

    /// Controller commands
    pub controller_commands: HashMap<String, ControllerCommand>,

    /// Logger definitions
    pub logger_definitions: HashMap<String, LoggerDefinition>,

    /// Port editor configurations
    pub port_editors: HashMap<String, PortEditorConfig>,

    /// Reference tables
    pub reference_tables: HashMap<String, ReferenceTable>,

    /// FTP browser configurations
    pub ftp_browsers: HashMap<String, FTPBrowserConfig>,

    /// Datalog views
    pub datalog_views: HashMap<String, DatalogView>,

    /// Key actions (keyboard shortcuts)
    pub key_actions: Vec<KeyAction>,

    /// VE Analysis configuration (from [VeAnalyze] section)
    pub ve_analyze: Option<VeAnalyzeConfig>,

    /// WUE Analysis configuration (from [WueAnalyze] section)
    pub wue_analyze: Option<WueAnalyzeConfig>,

    /// Gamma Enrichment configuration (from [GammaE] section)
    pub gamma_e: Option<GammaEConfig>,

    /// maintainConstantValue entries (from [ConstantsExtensions] section)
    /// These define expressions that auto-update constants
    pub maintain_constant_values: Vec<MaintainConstantValue>,

    /// Constants that require ECU power cycle after change
    pub requires_power_cycle: Vec<String>,
}

impl EcuDefinition {
    /// Parse an ECU definition from an INI file
    ///
    /// Handles various encodings (UTF-8, Windows-1252, Latin-1) by using
    /// lossy conversion for non-UTF-8 files.
    ///
    /// This method supports the `#include` directive, allowing INI files
    /// to include other INI files with relative path resolution.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, IniError> {
        parser::parse_ini_from_path(path.as_ref())
    }

    /// Parse an ECU definition from a string
    /// 
    /// Note: This method does not support `#include` directives since there
    /// is no file path context for resolving relative includes. Use `from_file`
    /// if you need `#include` support.
    pub fn from_str(content: &str) -> Result<Self, IniError> {
        parser::parse_ini(content)
    }

    /// Get a constant by name
    pub fn get_constant(&self, name: &str) -> Option<&Constant> {
        self.constants.get(name)
    }

    /// Get an output channel by name
    pub fn get_output_channel(&self, name: &str) -> Option<&OutputChannel> {
        self.output_channels.get(name)
    }

    /// Get a table definition by name
    pub fn get_table(&self, name: &str) -> Option<&TableDefinition> {
        self.tables.get(name)
    }

    /// Get a table definition by name or map_name
    /// Menus often reference tables by map_name (e.g., "veTable1Map"),
    /// but tables are indexed by name (e.g., "veTable1Tbl")
    pub fn get_table_by_name_or_map(&self, name_or_map: &str) -> Option<&TableDefinition> {
        // First try direct lookup by name
        if let Some(table) = self.tables.get(name_or_map) {
            return Some(table);
        }
        // Then try lookup by map_name
        if let Some(table_name) = self.table_map_to_name.get(name_or_map) {
            return self.tables.get(table_name);
        }
        None
    }

    /// Get the total ECU memory size across all pages
    pub fn total_memory_size(&self) -> usize {
        self.page_sizes.iter().map(|s| *s as usize).sum()
    }
}

impl Default for EcuDefinition {
    fn default() -> Self {
        Self {
            signature: String::new(),
            query_command: "Q".to_string(),
            version_info: String::new(),
            ini_spec_version: "3.64".to_string(),
            defines: HashMap::new(),
            endianness: Endianness::default(),
            page_sizes: Vec::new(),
            n_pages: 0,
            protocol: ProtocolSettings::default(),
            constants: HashMap::new(),
            output_channels: HashMap::new(),
            tables: HashMap::new(),
            table_map_to_name: HashMap::new(),
            curves: HashMap::new(),
            gauges: HashMap::new(),
            setting_groups: HashMap::new(),
            menus: Vec::new(),
            dialogs: HashMap::new(),
            datalog_entries: Vec::new(),
            help_topics: HashMap::new(),
            pc_variables: HashMap::new(),
            default_values: HashMap::new(),
            frontpage: None,
            indicator_panels: HashMap::new(),
            controller_commands: HashMap::new(),
            logger_definitions: HashMap::new(),
            port_editors: HashMap::new(),
            reference_tables: HashMap::new(),
            ftp_browsers: HashMap::new(),
            datalog_views: HashMap::new(),
            key_actions: Vec::new(),
            ve_analyze: None,
            wue_analyze: None,
            gamma_e: None,
            maintain_constant_values: Vec::new(),
            requires_power_cycle: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_definition() {
        let def = EcuDefinition::default();
        assert_eq!(def.query_command, "Q");
        assert!(def.constants.is_empty());
    }
}
