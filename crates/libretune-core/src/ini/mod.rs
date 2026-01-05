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

mod parser;
mod constants;
mod output_channels;
mod tables;
mod gauges;
mod types;
mod error;
pub mod expression;

pub use error::IniError;
pub use types::*;
pub use constants::Constant;
pub use output_channels::OutputChannel;
pub use tables::{TableDefinition, CurveDefinition};
pub use gauges::GaugeConfig;

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
}

impl EcuDefinition {
    /// Parse an ECU definition from an INI file
    /// 
    /// Handles various encodings (UTF-8, Windows-1252, Latin-1) by using
    /// lossy conversion for non-UTF-8 files.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, IniError> {
        let content = Self::read_ini_file(path.as_ref())?;
        Self::from_str(&content)
    }
    
    /// Read an INI file, handling encoding issues
    fn read_ini_file(path: &Path) -> Result<String, IniError> {
        // Try UTF-8 first
        match std::fs::read_to_string(path) {
            Ok(content) => Ok(content),
            Err(e) => {
                // If it's a UTF-8 error, try reading as bytes and use lossy conversion
                if e.kind() == std::io::ErrorKind::InvalidData {
                    let bytes = std::fs::read(path)
                        .map_err(|e| IniError::IoError(e.to_string()))?;
                    
                    // Use lossy conversion - replaces invalid UTF-8 sequences
                    // This handles Windows-1252, Latin-1, and other single-byte encodings
                    Ok(String::from_utf8_lossy(&bytes).into_owned())
                } else {
                    Err(IniError::IoError(e.to_string()))
                }
            }
        }
    }
    
    /// Parse an ECU definition from a string
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
