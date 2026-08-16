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
pub mod encoding;
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
pub use gauges::{parse_gauge_line, GaugeConfig};
pub use inc_tables::{IncTable, IncTableCache};
pub use output_channels::OutputChannel;
/// Seed preprocessor symbols (e.g. `CELSIUS`) applied to every parse — see
/// [`parser::set_default_symbols`].
pub use parser::set_default_symbols;
pub use tables::{CurveDefinition, TableDefinition, TableRole, TableType};
pub use types::*;

use std::collections::HashMap;
use std::path::Path;

/// Split an INI line value by commas, respecting quotes and braces.
/// Handles expressions like `{ bitStringValue(algorithmUnits , algorithm) }`.
pub fn split_ini_line(value: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut in_braces = 0;

    for ch in value.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                current.push(ch);
            }
            '{' if !in_quotes => {
                in_braces += 1;
                current.push(ch);
            }
            '}' if !in_quotes => {
                in_braces -= 1;
                current.push(ch);
            }
            ',' if !in_quotes && in_braces == 0 => {
                parts.push(current.trim().to_string());
                current = String::new();
            }
            _ => {
                current.push(ch);
            }
        }
    }
    parts.push(current.trim().to_string());
    parts
}

/// Complete ECU definition parsed from an INI file
#[derive(Debug, Clone)]
pub struct EcuDefinition {
    /// ECU type detected from signature
    pub ecu_type: EcuType,

    /// ECU signature string (e.g., "speeduino 202310")
    pub signature: String,

    /// Optional `signaturePrefix` declared by the INI (msEnvelope_1.0 spec §3.4).
    /// If present, an ECU signature whose leading bytes match this prefix is
    /// considered compatible even when the trailing build/version differs.
    pub signature_prefix: Option<String>,

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

    /// Lookup map from curve map_name to curve name (if curves have map names)
    /// Similar to table_map_to_name for consistent lookup patterns
    pub curve_map_to_name: HashMap<String, String>,

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

    /// Default values for constants (from [Defaults] / [ConstantsExtensions])
    /// Maps constant name -> default value
    pub default_values: HashMap<String, f64>,

    /// Cell budget for dynamically sized 2D arrays (`maximumElements` in ConstantsExtensions).
    pub maximum_elements: HashMap<String, usize>,

    /// FrontPage configuration for default dashboard layout
    pub frontpage: Option<FrontPageConfig>,

    /// Indicator panels (groups of boolean indicators)
    pub indicator_panels: HashMap<String, IndicatorPanel>,

    /// Live numeric readout panels (TunerStudio readoutPanel)
    pub readout_panels: HashMap<String, ReadoutPanel>,

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
    #[allow(clippy::should_implement_trait)]
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

    /// Populate [`TableDefinition::role`] for every table in this definition,
    /// using the INI's `[VeAnalyze]` and `[WueAnalyze]` configuration as the
    /// source of truth.
    ///
    /// This is the recommended way to attach machine-readable roles so that
    /// automation (e.g. the AI assistant) can tell a VE table from an ignition
    /// table without guessing from names. Should be called once after the INI
    /// is fully parsed (both `[TableEditor]` and the analyze sections). Safe
    /// to call more than once. Tables not referenced by any analyze config
    /// are left as [`TableRole::Other`] (their default), with a name-based
    /// heuristic for ignition tables as a fallback.
    pub fn infer_table_roles(&mut self) {
        let mut ve_names: Vec<String> = Vec::new();
        let mut afr_target_names: Vec<String> = Vec::new();
        let mut wue_names: Vec<String> = Vec::new();

        if let Some(cfg) = &self.ve_analyze {
            ve_names.push(cfg.ve_table_name.clone());
            afr_target_names.push(cfg.target_table_name.clone());
            afr_target_names.extend(cfg.lambda_target_tables.iter().cloned());
        }
        if let Some(cfg) = &self.wue_analyze {
            wue_names.push(cfg.wue_curve_name.clone());
            afr_target_names.push(cfg.target_table_name.clone());
            afr_target_names.extend(cfg.lambda_target_tables.iter().cloned());
        }

        // Assign roles, taking care not to overwrite a VE role with a weaker
        // AFR-target role if a name appears in both lists (shouldn't happen
        // in practice, but be defensive).
        for table in self.tables.values_mut() {
            let canonical_name = &table.name;
            let map_name = table.map_name.as_deref();

            // A table matches a candidate name if either its canonical name
            // or its map_name equals the candidate.
            let matches_any = |names: &[String]| {
                names
                    .iter()
                    .any(|n| n == canonical_name || map_name == Some(n.as_str()))
            };

            if matches_any(&ve_names) {
                table.role = TableRole::Ve;
            } else if matches_any(&afr_target_names) {
                table.role = TableRole::AfrTarget;
            } else if matches_any(&wue_names) {
                table.role = TableRole::WarmupEnrichment;
            } else {
                // Name-based fallback for ignition tables, since most INIs do
                // not have an ignition analyze section. Conservative: only
                // match obvious names so we don't mislabel.
                let lname = canonical_name.to_lowercase();
                if lname.contains("ign") || lname.contains("spark") || lname.contains("advance") {
                    table.role = TableRole::Ignition;
                } else {
                    table.role = TableRole::Other;
                }
            }
        }
    }

    /// Synthesize a [`DialogDefinition`] for a built-in TunerStudio `std_*`
    /// panel name that the INI references via `panel = std_injection` (or
    /// similar) but does not itself define as a `dialog = ...`.
    ///
    /// These are standard panels that TunerStudio ships natively. LibreTune
    /// does not carry TunerStudio's implementation, so we rebuild them from
    /// the constants actually present in the loaded INI. Only constants that
    /// exist in `[Constants]` are emitted, so the same panel adapts across
    /// Speeduino / MegaSquirt / MS2 / MS3 despite small naming differences.
    ///
    /// Returns `None` for names we do not synthesize, so the caller can fall
    /// back to a friendly placeholder (or an error).
    pub fn std_panel_definition(&self, name: &str) -> Option<DialogDefinition> {
        // (constant name, human label). Ordered to match the layout a user
        // expects in the corresponding TunerStudio standard panel.
        let (title, candidates): (&str, &[(&str, &str)]) = match name {
            "std_injection" => (
                "Injection Setup",
                &[
                    ("reqFuel", "Required Fuel"),
                    ("nCylinders", "Number of Cylinders"),
                    ("injType", "Injector Type"),
                    ("divider", "Injector Divider"),
                    ("alternate", "Injection Timing"),
                    ("nInjectors", "Number of Injectors"),
                    ("injOpen", "Injector Open Time"),
                ],
            ),
            // MS3-style real-time-clock panel, referenced by Speeduino as
            // `panel = std_ms3Rtc {rtc_mode}`. The native TunerStudio panel
            // also offers a "set clock to PC time" action; we surface the
            // editable calibration constant (rtc_trim) instead. rtc_mode is
            // intentionally omitted — it is the panel's own enable gate and
            // is already rendered as a field in the enclosing dialog.
            "std_ms3Rtc" => ("Real Time Clock", &[("rtc_trim", "RTC Trim (ppm)")]),
            _ => return None,
        };

        // Build a Field component for every candidate that exists as a
        // constant in this definition. Skipping missing ones keeps the panel
        // correct per-ECU without erroring on optional fields.
        let mut components: Vec<DialogComponent> = Vec::new();
        for (const_name, label) in candidates {
            if self.constants.contains_key(*const_name) {
                components.push(DialogComponent::Field {
                    label: (*label).to_string(),
                    name: (*const_name).to_string(),
                    visibility_condition: None,
                    enabled_condition: None,
                });
            }
        }

        // If none of the expected constants are present this almost certainly
        // is not the panel we think it is — bail out so the caller shows the
        // generic placeholder instead of an empty dialog.
        if components.is_empty() {
            return None;
        }

        Some(DialogDefinition {
            name: name.to_string(),
            title: title.to_string(),
            components,
        })
    }

    /// Get a curve definition by name or map_name
    /// Similar to get_table_by_name_or_map for consistent lookup patterns
    pub fn get_curve_by_name_or_map(&self, name_or_map: &str) -> Option<&CurveDefinition> {
        // First try direct lookup by name
        if let Some(curve) = self.curves.get(name_or_map) {
            return Some(curve);
        }
        // Then try lookup by map_name
        if let Some(curve_name) = self.curve_map_to_name.get(name_or_map) {
            return self.curves.get(curve_name);
        }
        None
    }

    /// Get the total ECU memory size across all pages
    pub fn total_memory_size(&self) -> usize {
        self.page_sizes.iter().map(|s| *s as usize).sum()
    }

    /// Compute a structural hash of the INI definition
    ///
    /// This hash is based on constant names, types, pages, offsets, and scales.
    /// It changes when the INI structure changes, but not for cosmetic changes
    /// like label updates or help text.
    pub fn compute_structural_hash(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::collections::BTreeMap;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        // Hash signature
        self.signature.hash(&mut hasher);

        // Hash page configuration
        self.n_pages.hash(&mut hasher);
        for size in &self.page_sizes {
            size.hash(&mut hasher);
        }

        // Sort constants by name for deterministic ordering
        let sorted_constants: BTreeMap<_, _> = self
            .constants
            .iter()
            .filter(|(_, c)| !c.is_pc_variable)
            .collect();

        for (name, constant) in sorted_constants {
            // Hash structural properties only
            name.hash(&mut hasher);
            format!("{:?}", constant.data_type).hash(&mut hasher);
            constant.page.hash(&mut hasher);
            constant.offset.hash(&mut hasher);
            // Convert floats to bits for hashing
            constant.scale.to_bits().hash(&mut hasher);
            constant.translate.to_bits().hash(&mut hasher);
        }

        format!("{:016x}", hasher.finish())
    }

    /// Generate a constant manifest for saving with tune files
    pub fn generate_constant_manifest(&self) -> Vec<crate::tune::ConstantManifestEntry> {
        let mut manifest = Vec::new();

        for (name, constant) in &self.constants {
            // Skip PC variables
            if constant.is_pc_variable {
                continue;
            }

            manifest.push(crate::tune::ConstantManifestEntry {
                name: name.clone(),
                data_type: format!("{:?}", constant.data_type),
                page: constant.page,
                offset: constant.offset,
                scale: constant.scale,
                translate: constant.translate,
            });
        }

        // Sort by name for consistent ordering
        manifest.sort_by(|a, b| a.name.cmp(&b.name));

        manifest
    }

    /// Generate INI metadata for saving with tune files
    pub fn generate_ini_metadata(&self, ini_filename: &str) -> crate::tune::IniMetadata {
        crate::tune::IniMetadata {
            signature: self.signature.clone(),
            name: ini_filename.to_string(),
            hash: self.compute_structural_hash(),
            spec_version: self.ini_spec_version.clone(),
            saved_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Derive INI-driven feature capabilities for UI gating.
    pub fn capabilities(&self) -> IniCapabilities {
        IniCapabilities {
            has_constants: !self.constants.is_empty(),
            has_output_channels: !self.output_channels.is_empty(),
            has_tables: !self.tables.is_empty(),
            has_curves: !self.curves.is_empty(),
            has_gauges: !self.gauges.is_empty(),
            has_frontpage: self.frontpage.is_some(),
            has_dialogs: !self.dialogs.is_empty(),
            has_help_topics: !self.help_topics.is_empty(),
            has_setting_groups: !self.setting_groups.is_empty(),
            has_pc_variables: !self.pc_variables.is_empty(),
            has_default_values: !self.default_values.is_empty(),
            has_datalog_entries: !self.datalog_entries.is_empty(),
            has_datalog_views: !self.datalog_views.is_empty(),
            has_logger_definitions: !self.logger_definitions.is_empty(),
            has_controller_commands: !self.controller_commands.is_empty(),
            has_port_editors: !self.port_editors.is_empty(),
            has_reference_tables: !self.reference_tables.is_empty(),
            has_key_actions: !self.key_actions.is_empty(),
            has_ve_analyze: self.ve_analyze.is_some(),
            has_wue_analyze: self.wue_analyze.is_some(),
            has_gamma_e: self.gamma_e.is_some(),
            supports_console: self.ecu_type.supports_console()
                && !self.controller_commands.is_empty(),
            dfu_command_name: self
                .controller_commands
                .keys()
                .find(|k| k.eq_ignore_ascii_case("cmd_dfu"))
                .cloned(),
            openblt_command_name: self
                .controller_commands
                .keys()
                .find(|k| k.eq_ignore_ascii_case("cmd_openblt"))
                .cloned(),
            lua_script_constant: self
                .constants
                .iter()
                .find(|(name, c)| {
                    name.eq_ignore_ascii_case("luaScript") && c.data_type == DataType::String
                })
                .map(|(name, _)| name.clone()),
        }
    }
}

impl Default for EcuDefinition {
    fn default() -> Self {
        Self {
            ecu_type: EcuType::Unknown,
            signature: String::new(),
            signature_prefix: None,
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
            curve_map_to_name: HashMap::new(),
            gauges: HashMap::new(),
            setting_groups: HashMap::new(),
            menus: Vec::new(),
            dialogs: HashMap::new(),
            datalog_entries: Vec::new(),
            help_topics: HashMap::new(),
            pc_variables: HashMap::new(),
            default_values: HashMap::new(),
            maximum_elements: HashMap::new(),
            frontpage: None,
            indicator_panels: HashMap::new(),
            readout_panels: HashMap::new(),
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

    /// Helper: build a minimal `Constant` with just a name (enough for the
    /// std-panel synthesizer, which only checks for key presence).
    fn scalar_const(name: &str) -> Constant {
        Constant::new(name, 0, 0, DataType::U08)
    }

    #[test]
    fn std_panel_injection_synthesizes_from_constants() {
        // Mimic a Speeduino-like [Constants] block: a subset of the candidate
        // names exists, the rest (nInjectors) is absent.
        let mut def = EcuDefinition::default();
        for n in [
            "reqFuel",
            "divider",
            "alternate",
            "injOpen",
            "nCylinders",
            "injType",
        ] {
            def.constants.insert(n.to_string(), scalar_const(n));
        }

        let d = def
            .std_panel_definition("std_injection")
            .expect("std_injection should synthesize");

        assert_eq!(d.name, "std_injection");
        // nInjectors was not in constants, so it must be dropped. Every other
        // candidate should be present in declaration order.
        let field_names: Vec<String> = d
            .components
            .iter()
            .filter_map(|c| match c {
                DialogComponent::Field { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(
            field_names,
            vec![
                "reqFuel".to_string(),
                "nCylinders".to_string(),
                "injType".to_string(),
                "divider".to_string(),
                "alternate".to_string(),
                "injOpen".to_string(),
            ]
        );
    }

    #[test]
    fn std_panel_injection_none_when_no_candidates_present() {
        // Constants exist, but none of the injection candidates — bail out so
        // the caller shows the generic placeholder instead of an empty dialog.
        let mut def = EcuDefinition::default();
        def.constants
            .insert("unrelatedConst".to_string(), scalar_const("unrelatedConst"));
        assert!(def.std_panel_definition("std_injection").is_none());
    }

    #[test]
    fn std_panel_unknown_name_returns_none() {
        // Only std_injection / std_ms3Rtc are synthesized today; other std_*
        // names (calibration tables, wizards) fall through to the generic
        // placeholder path.
        let mut def = EcuDefinition::default();
        def.constants
            .insert("reqFuel".to_string(), scalar_const("reqFuel"));
        assert!(def.std_panel_definition("std_ms2gentherm").is_none());
    }

    #[test]
    fn std_panel_ms3rtc_synthesizes_trim() {
        // Speeduino references `panel = std_ms3Rtc {rtc_mode}` inside its
        // rtc_settings dialog. The panel surfaces the rtc_trim calibration
        // constant (rtc_mode is the enclosing dialog's own enable field).
        let mut def = EcuDefinition::default();
        def.constants
            .insert("rtc_trim".to_string(), scalar_const("rtc_trim"));

        let d = def
            .std_panel_definition("std_ms3Rtc")
            .expect("std_ms3Rtc should synthesize when rtc_trim exists");

        assert_eq!(d.name, "std_ms3Rtc");
        assert_eq!(d.title, "Real Time Clock");
        let field_names: Vec<String> = d
            .components
            .iter()
            .filter_map(|c| match c {
                DialogComponent::Field { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(field_names, vec!["rtc_trim".to_string()]);
    }

    #[test]
    fn std_panel_ms3rtc_none_without_trim() {
        // MS3 itself does not define rtc_trim and does not reference the panel
        // via `panel = std_ms3Rtc`; if somehow requested it should bail out.
        let mut def = EcuDefinition::default();
        def.constants
            .insert("rtc_mode".to_string(), scalar_const("rtc_mode"));
        assert!(def.std_panel_definition("std_ms3Rtc").is_none());
    }
}
