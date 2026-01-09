//! Core INI file parser
//!
//! Handles the tokenization and section-by-section parsing of ECU INI definition files.

use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::{
    constants::{parse_constant_line, parse_pc_variable_line},
    gauges::parse_gauge_line,
    output_channels::parse_output_channel_line,
    tables::{CurveDefinition, TableDefinition},
    types::{
        AnalysisFilter, CommandButtonCloseAction, CommandPart, ControllerCommand, DatalogEntry,
        DatalogView, DialogComponent, DialogDefinition, FilterOperator, FTPBrowserConfig,
        FrontPageConfig, FrontPageIndicator, GammaEConfig, HelpTopic, IndicatorDefinition,
        IndicatorPanel, KeyAction, LoggerDefinition, MaintainConstantValue, Menu, MenuItem,
        PortEditorConfig, ReferenceTable, SettingGroup, SettingOption, VeAnalyzeConfig,
        WueAnalyzeConfig,
    },
    EcuDefinition, IniError,
};

/// Maximum depth for nested #include directives
const MAX_INCLUDE_DEPTH: usize = 16;

struct ParserState {
    current_page: u8,
    last_offset: u16,
    current_table: Option<String>,
    current_dialog: Option<String>,
    current_indicator_panel: Option<String>,
    current_curve: Option<String>,
    current_help: Option<String>,
}

/// Context for include resolution
struct IncludeContext {
    /// Base directory for resolving relative paths
    base_dir: Option<PathBuf>,
    /// Set of already-included files (canonical paths) for circular reference detection
    included_files: HashSet<PathBuf>,
    /// Current include depth
    depth: usize,
}

impl IncludeContext {
    fn new(base_path: Option<&Path>) -> Self {
        Self {
            base_dir: base_path.and_then(|p| p.parent().map(|d| d.to_path_buf())),
            included_files: HashSet::new(),
            depth: 0,
        }
    }

    /// Resolve an include path relative to the current file's directory
    fn resolve_include(&self, include_path: &str) -> Option<PathBuf> {
        let include_path = include_path.trim().trim_matches('"');
        let path = Path::new(include_path);
        
        if path.is_absolute() {
            Some(path.to_path_buf())
        } else if let Some(base) = &self.base_dir {
            Some(base.join(path))
        } else {
            // No base dir, try relative to current working directory
            Some(PathBuf::from(include_path))
        }
    }
}

/// Parse a complete INI file into an EcuDefinition
pub fn parse_ini(content: &str) -> Result<EcuDefinition, IniError> {
    parse_ini_internal(content, &mut IncludeContext::new(None))
}

/// Parse an INI file from a path, enabling #include directive support
pub fn parse_ini_from_path(path: &Path) -> Result<EcuDefinition, IniError> {
    let content = read_ini_file(path)?;
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    
    let mut ctx = IncludeContext::new(Some(path));
    ctx.included_files.insert(canonical);
    
    parse_ini_internal(&content, &mut ctx)
}

/// Read an INI file with encoding fallback (UTF-8 first, then lossy)
fn read_ini_file(path: &Path) -> Result<String, IniError> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::InvalidData {
                let bytes = std::fs::read(path)
                    .map_err(|e| IniError::IoError(e.to_string()))?;
                Ok(String::from_utf8_lossy(&bytes).into_owned())
            } else if e.kind() == std::io::ErrorKind::NotFound {
                Err(IniError::IncludeNotFound(path.display().to_string()))
            } else {
                Err(IniError::IoError(e.to_string()))
            }
        }
    }
}

/// Internal parsing function that handles #include directives
fn parse_ini_internal(content: &str, ctx: &mut IncludeContext) -> Result<EcuDefinition, IniError> {
    let mut definition = EcuDefinition::default();
    let mut current_section = String::new();
    let mut state = ParserState {
        current_page: 0,
        last_offset: 0,
        current_table: None,
        current_dialog: None,
        current_indicator_panel: None,
        current_curve: None,
        current_help: None,
    };

    // Preprocessor state for handling #set, #if, #else, #endif
    let mut defined_symbols: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut condition_stack: Vec<bool> = Vec::new();

    // Pre-compile regex for line continuations
    let continuation_re = Regex::new(r"\\\s*$").unwrap();

    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let mut line = lines[i].to_string();

        // Handle line continuations
        while continuation_re.is_match(&line) && i + 1 < lines.len() {
            line = continuation_re.replace(&line, "").to_string();
            i += 1;
            line.push_str(lines[i].trim());
        }

        let line = strip_comment(&line);
        let line = line.trim();

        if line.is_empty() {
            i += 1;
            continue;
        }

        // Handle preprocessor directives (always processed regardless of condition)
        if line.starts_with("#set ") {
            let symbol = line[5..].trim().to_string();
            eprintln!("[DEBUG] preprocessor: #set {}", symbol);
            defined_symbols.insert(symbol);
            i += 1;
            continue;
        }

        if line.starts_with("#unset ") {
            let symbol = line[7..].trim();
            eprintln!("[DEBUG] preprocessor: #unset {}", symbol);
            defined_symbols.remove(symbol);
            i += 1;
            continue;
        }

        if line.starts_with("#if ") {
            let symbol = line[4..].trim();
            let is_defined = defined_symbols.contains(symbol);
            eprintln!("[DEBUG] preprocessor: #if {} -> {}", symbol, is_defined);
            condition_stack.push(is_defined);
            i += 1;
            continue;
        }

        if line == "#else" {
            if let Some(last) = condition_stack.last_mut() {
                eprintln!(
                    "[DEBUG] preprocessor: #else (was {}, now {})",
                    *last, !*last
                );
                *last = !*last;
            }
            i += 1;
            continue;
        }

        if line == "#endif" {
            eprintln!("[DEBUG] preprocessor: #endif");
            condition_stack.pop();
            i += 1;
            continue;
        }

        // Handle #define directives (only if in active branch)
        if line.starts_with("#define") {
            if condition_stack.iter().all(|&c| c) {
                parse_define_directive(&mut definition, line);
            }
            i += 1;
            continue;
        }

        // Handle #include directives (only if in active branch)
        if line.starts_with("#include") {
            if condition_stack.iter().all(|&c| c) {
                let include_path = line[8..].trim().trim_matches('"');
                if let Some(resolved_path) = ctx.resolve_include(include_path) {
                    // Check depth limit
                    if ctx.depth >= MAX_INCLUDE_DEPTH {
                        return Err(IniError::IncludeDepthExceeded(MAX_INCLUDE_DEPTH));
                    }
                    
                    // Get canonical path for circular reference detection
                    let canonical = resolved_path.canonicalize()
                        .unwrap_or_else(|_| resolved_path.clone());
                    
                    // Check for circular includes
                    if ctx.included_files.contains(&canonical) {
                        return Err(IniError::CircularInclude(
                            resolved_path.display().to_string()
                        ));
                    }
                    
                    // Read and parse the included file
                    let included_content = read_ini_file(&resolved_path)?;
                    
                    // Save current base_dir and update for nested includes
                    let prev_base = ctx.base_dir.clone();
                    ctx.base_dir = resolved_path.parent().map(|p| p.to_path_buf());
                    ctx.included_files.insert(canonical.clone());
                    ctx.depth += 1;
                    
                    // Parse included content and merge into current definition
                    let included_def = parse_ini_internal(&included_content, ctx)?;
                    merge_definitions(&mut definition, included_def);
                    
                    // Restore context
                    ctx.base_dir = prev_base;
                    ctx.depth -= 1;
                    // Note: we keep canonical in included_files to prevent re-inclusion
                }
            }
            i += 1;
            continue;
        }

        // Skip other preprocessor directives
        if line.starts_with('#') {
            i += 1;
            continue;
        }

        // Skip content if we're in a false branch
        if !condition_stack.iter().all(|&c| c) {
            i += 1;
            continue;
        }

        // Check for section header
        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].trim().to_string();
            i += 1;
            continue;
        }

        // Parse key = value
        if let Some((key, value)) = parse_key_value(line) {
            match current_section.as_str() {
                "MegaTune" => parse_megatune(&mut definition, key, value),
                "TunerStudio" => parse_tunerstudio(&mut definition, key, value),
                "Constants" => parse_constants_entry(
                    &mut definition,
                    key,
                    value,
                    &mut state.current_page,
                    &mut state.last_offset,
                ),
                "OutputChannels" => parse_output_channel_entry(&mut definition, key, value),
                "BurstMode" => parse_burst_mode_entry(&mut definition, key, value),
                "GaugeConfigurations" => parse_gauge_entry(&mut definition, key, value),
                "SettingGroups" => parse_setting_group_entry(&mut definition, key, value),
                "PcVariables" => parse_pc_variable_entry(&mut definition, key, value),
                "Datalog" => parse_datalog_entry(&mut definition, key, value),
                "Defaults" => parse_defaults_entry(&mut definition, key, value),
                "Menu" => parse_menu_entry(&mut definition, key, value),
                "UserDefined" => parse_user_defined_entry(
                    &mut definition,
                    key,
                    value,
                    &mut state.current_dialog,
                    &mut state.current_indicator_panel,
                    &mut state.current_help,
                ),
                "SettingContextHelp" => parse_setting_context_help(&mut definition, key, value),
                "FrontPage" => parse_frontpage_entry(&mut definition, key, value),
                "ControllerCommands" => parse_controller_command_entry(&mut definition, key, value),
                "LoggerDefinition" => parse_logger_definition_entry(&mut definition, key, value),
                "PortEditor" => parse_port_editor_entry(&mut definition, key, value),
                "ReferenceTables" => parse_reference_table_entry(&mut definition, key, value),
                "FTPBrowser" => parse_ftp_browser_entry(&mut definition, key, value),
                "DatalogViews" => parse_datalog_view_entry(&mut definition, key, value),
                "KeyActions" => parse_key_action_entry(&mut definition, key, value),
                "VeAnalyze" => parse_ve_analyze_entry(&mut definition, key, value),
                "WueAnalyze" => parse_wue_analyze_entry(&mut definition, key, value),
                "GammaE" => parse_gamma_e_entry(&mut definition, key, value),
                "ConstantsExtensions" => {
                    parse_constants_extensions_entry(&mut definition, key, value)
                }
                _ => {
                    if current_section.contains("TableEditor") {
                        parse_table_editor_entry(
                            &mut definition,
                            key,
                            value,
                            &mut state.current_table,
                        );
                    } else if current_section.contains("CurveEditor") {
                        parse_curve_editor_entry(
                            &mut definition,
                            key,
                            value,
                            &mut state.current_curve,
                        );
                    }
                }
            }
        }

        i += 1;
    }

    // Post-process: Apply variable substitution to commands that may have been parsed
    // before [PcVariables] section was encountered (e.g., queryCommand in [MegaTune])
    post_process_variable_substitution(&mut definition);

    // Post-process menu items to fix types that couldn't be determined during initial parse
    // (e.g., help topics defined after menu section)
    post_process_menu_items(&mut definition);

    // Post-process tables to resolve x_size/y_size from referenced constants
    post_process_table_sizes(&mut definition);

    Ok(definition)
}

/// Merge an included definition into the main definition
/// Later definitions override earlier ones for matching keys
fn merge_definitions(target: &mut EcuDefinition, source: EcuDefinition) {
    // Merge defines (source overrides)
    target.defines.extend(source.defines);
    
    // Merge constants (source overrides)
    target.constants.extend(source.constants);
    
    // Merge output channels
    target.output_channels.extend(source.output_channels);
    
    // Merge tables
    target.tables.extend(source.tables);
    target.table_map_to_name.extend(source.table_map_to_name);
    
    // Merge curves
    target.curves.extend(source.curves);
    
    // Merge gauges
    target.gauges.extend(source.gauges);
    
    // Merge setting groups
    target.setting_groups.extend(source.setting_groups);
    
    // Merge dialogs
    target.dialogs.extend(source.dialogs);
    
    // Merge menus (append)
    target.menus.extend(source.menus);
    
    // Merge help topics
    target.help_topics.extend(source.help_topics);
    
    // Merge datalog entries (append)
    target.datalog_entries.extend(source.datalog_entries);
    
    // Merge PC variables
    target.pc_variables.extend(source.pc_variables);
    
    // Merge default values
    target.default_values.extend(source.default_values);
    
    // Merge indicator panels
    target.indicator_panels.extend(source.indicator_panels);
    
    // Merge controller commands
    target.controller_commands.extend(source.controller_commands);
    
    // Merge logger definitions
    target.logger_definitions.extend(source.logger_definitions);
    
    // Merge port editors
    target.port_editors.extend(source.port_editors);
    
    // Merge reference tables
    target.reference_tables.extend(source.reference_tables);
    
    // Merge FTP browsers
    target.ftp_browsers.extend(source.ftp_browsers);
    
    // Merge datalog views
    target.datalog_views.extend(source.datalog_views);
    
    // Merge key actions (append)
    target.key_actions.extend(source.key_actions);
    
    // Take frontpage from source if target doesn't have one
    if target.frontpage.is_none() && source.frontpage.is_some() {
        target.frontpage = source.frontpage;
    }
    
    // For scalar values, only take from source if they appear to be set
    // (non-default values override)
    if !source.signature.is_empty() {
        target.signature = source.signature;
    }
    if !source.query_command.is_empty() {
        target.query_command = source.query_command;
    }
    if !source.version_info.is_empty() {
        target.version_info = source.version_info;
    }
    if !source.page_sizes.is_empty() {
        target.page_sizes = source.page_sizes;
    }
    if source.n_pages > 0 {
        target.n_pages = source.n_pages;
    }
    
    // Merge analysis configs (source overrides target)
    if source.ve_analyze.is_some() {
        target.ve_analyze = source.ve_analyze;
    }
    if source.wue_analyze.is_some() {
        target.wue_analyze = source.wue_analyze;
    }
    if source.gamma_e.is_some() {
        target.gamma_e = source.gamma_e;
    }
    
    // Merge ConstantsExtensions (append)
    target.maintain_constant_values.extend(source.maintain_constant_values);
    target.requires_power_cycle.extend(source.requires_power_cycle);
}

/// Strip comments from a line (everything after ';')
/// Note: '#' is handled at the line level for preprocessor directives
fn strip_comment(line: &str) -> String {
    let mut result = String::new();
    let mut in_quotes = false;

    for ch in line.chars() {
        if ch == '"' {
            in_quotes = !in_quotes;
            result.push(ch);
        } else if ch == ';' && !in_quotes {
            break;
        } else {
            result.push(ch);
        }
    }

    result
}

/// Parse a key = value line
fn parse_key_value(line: &str) -> Option<(&str, &str)> {
    let parts: Vec<&str> = line.splitn(2, '=').collect();
    if parts.len() == 2 {
        Some((parts[0].trim(), parts[1].trim()))
    } else {
        None
    }
}

/// Split an INI line value by commas, respecting quotes and braces
/// This handles expressions like `{ bitStringValue(algorithmUnits , algorithm) }` correctly
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

/// Parse a #define directive
/// Format: #define name = value1, value2, value3
/// These are used to define option lists for bits fields, and can reference
/// other defines using $referenceName syntax.
fn parse_define_directive(def: &mut EcuDefinition, line: &str) {
    // Format: #define name = value1, value2, ...
    // Or:     #define = name = value1, value2, ...
    let content = line.strip_prefix("#define").unwrap_or(line).trim();

    // Handle both "name = values" and "= name = values" formats
    let content = content
        .strip_prefix('=')
        .map(|s| s.trim())
        .unwrap_or(content);

    if let Some((name, values_str)) = parse_key_value(content) {
        let values: Vec<String> = split_ini_line(values_str)
            .into_iter()
            .flat_map(|v| {
                let v = v.trim().trim_matches('"').to_string();
                // Resolve $references to other defines
                if v.starts_with('$') {
                    let ref_name = &v[1..];
                    def.defines
                        .get(ref_name)
                        .cloned()
                        .unwrap_or_else(|| vec![v])
                } else if !v.is_empty() {
                    vec![v]
                } else {
                    vec![]
                }
            })
            .collect();

        if !name.is_empty() && !values.is_empty() {
            def.defines.insert(name.to_string(), values);
        }
    }
}

/// Resolve bits field option values, expanding $references to defines
fn resolve_bits_options(
    defines: &std::collections::HashMap<String, Vec<String>>,
    options: Vec<String>,
) -> Vec<String> {
    options
        .into_iter()
        .flat_map(|v| {
            let v = v.trim().trim_matches('"').to_string();
            if v.starts_with('$') {
                let ref_name = &v[1..];
                defines.get(ref_name).cloned().unwrap_or_else(|| vec![v])
            } else if !v.is_empty() {
                vec![v]
            } else {
                vec![]
            }
        })
        .collect()
}

/// Parse [MegaTune] section entries
fn parse_megatune(def: &mut EcuDefinition, key: &str, value: &str) {
    match key.to_lowercase().as_str() {
        "signature" => {
            def.signature = value.trim_matches('"').to_string();
        }
        "querycommand" => {
            eprintln!("[DEBUG] parse_megatune: queryCommand = {:?}", value);
            def.query_command = value.trim_matches('"').to_string();
        }
        "versioninfo" => {
            def.version_info = value.trim_matches('"').to_string();
        }
        "delayafterportopen" => {
            def.protocol.delay_after_port_open = value.parse().unwrap_or(0);
            eprintln!(
                "[DEBUG] parse_megatune: delayAfterPortOpen = {}",
                def.protocol.delay_after_port_open
            );
        }
        "interwritedelay" => {
            def.protocol.inter_write_delay = value.parse().unwrap_or(0);
        }
        "pageactivationdelay" | "pageactivationdelayms" => {
            def.protocol.page_activation_delay = value.parse().unwrap_or(500);
        }
        _ => {}
    }
}

/// Parse [TunerStudio] section entries
fn parse_tunerstudio(def: &mut EcuDefinition, key: &str, value: &str) {
    eprintln!(
        "[DEBUG] parse_tunerstudio: key = {:?}, value = {:?}",
        key, value
    );
    match key.to_lowercase().as_str() {
        "inispecversion" => {
            def.ini_spec_version = value.trim_matches('"').to_string();
        }
        "pagesizes" | "pagesize" => {
            // Parse comma-separated page sizes
            let sizes: Vec<u32> = value
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            def.page_sizes = sizes.iter().map(|&s| s as u16).collect();
            def.protocol.page_sizes = sizes;
            def.n_pages = def.page_sizes.len() as u8;
        }
        "npages" | "numpages" => {
            def.n_pages = value.parse().unwrap_or(0);
        }
        "endianness" => {
            if value.to_lowercase().contains("little") {
                def.endianness = super::types::Endianness::Little;
            } else {
                def.endianness = super::types::Endianness::Big;
            }
        }
        "querycommand" => {
            def.query_command = value.trim_matches('"').to_string();
            def.protocol.query_command = def.query_command.clone();
        }
        "defaultipaddress" => {
            def.protocol.default_ip_address = Some(value.trim_matches('"').to_string());
        }
        "defaultipport" => {
            def.protocol.default_ip_port = value.parse().unwrap_or(29001);
        }
        "delayafterportopen" => {
            def.protocol.delay_after_port_open = value.parse().unwrap_or(0);
            eprintln!(
                "[DEBUG] parse_tunerstudio: delayAfterPortOpen = {}",
                def.protocol.delay_after_port_open
            );
        }
        "interwritedelay" => {
            def.protocol.inter_write_delay = value.parse().unwrap_or(0);
        }
        "pageactivationdelay" | "pageactivationdelayms" => {
            def.protocol.page_activation_delay = value.parse().unwrap_or(500);
        }
        "messageenvelopeformat" => {
            def.protocol.message_envelope_format = Some(value.trim_matches('"').to_string());
            eprintln!(
                "[DEBUG] parse_tunerstudio: messageEnvelopeFormat = {:?}",
                def.protocol.message_envelope_format
            );
        }
        _ => {}
    }
}

/// Parse [Constants] section entries
fn parse_constants_entry(
    def: &mut EcuDefinition,
    key: &str,
    value: &str,
    current_page: &mut u8,
    last_offset: &mut u16,
) {
    let key_lower = key.to_lowercase();

    // Check for page directive
    if key_lower == "page" {
        *current_page = value.parse().unwrap_or(0);
        *last_offset = 0; // Reset offset counter for new page
        return;
    }

    // Check for protocol settings that appear in [Constants] section
    match key_lower.as_str() {
        "messageenvelopeformat" => {
            def.protocol.message_envelope_format = Some(value.trim_matches('"').to_string());
            return;
        }
        "endianness" => {
            if value.to_lowercase().contains("little") {
                def.endianness = super::types::Endianness::Little;
            } else {
                def.endianness = super::types::Endianness::Big;
            }
            return;
        }
        "npages" => {
            def.n_pages = value.parse().unwrap_or(0);
            return;
        }
        "pagesize" => {
            let sizes: Vec<u32> = value
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            def.page_sizes = sizes.iter().map(|&s| s as u16).collect();
            def.protocol.page_sizes = sizes;
            def.n_pages = def.page_sizes.len() as u8;
            return;
        }
        "pageidentifier" => {
            // Parse page identifiers like "\x00\x00", "\x00\x01"
            // Also handles $tsCanId substitution
            def.protocol.page_identifiers = parse_page_identifiers(value, &def.pc_variables);
            return;
        }
        "pagereadcommand" => {
            def.protocol.page_read_commands = parse_command_list(value, &def.pc_variables);
            return;
        }
        "pagevaluewrite" | "pagechunkwrite" => {
            def.protocol.page_chunk_write_commands = parse_command_list(value, &def.pc_variables);
            return;
        }
        "burncommand" => {
            def.protocol.burn_commands = parse_command_list(value, &def.pc_variables);
            return;
        }
        "crc32checkcommand" => {
            def.protocol.crc32_check_commands = parse_command_list(value, &def.pc_variables);
            return;
        }
        "retrieveconfigerror" => {
            def.protocol.retrieve_config_error = Some(value.trim_matches('"').to_string());
            return;
        }
        "pageactivationdelay" => {
            def.protocol.page_activation_delay = value.parse().unwrap_or(500);
            return;
        }
        "blockingfactor" => {
            def.protocol.blocking_factor = value.parse().unwrap_or(256);
            return;
        }
        "interwritedelay" => {
            def.protocol.inter_write_delay = value.parse().unwrap_or(0);
            return;
        }
        "blockreadtimeout" => {
            def.protocol.block_read_timeout = value.parse().unwrap_or(1000);
            return;
        }
        "writeblocks" => {
            def.protocol.write_blocks =
                value.to_lowercase() == "on" || value == "1" || value.to_lowercase() == "true";
            return;
        }
        "enable2ndbytecanid" => {
            def.protocol.enable_can_id = value.to_lowercase() != "false" && value != "0";
            return;
        }
        _ => {}
    }

    // Parse constant definition, passing defines for bits options resolution
    if let Some(mut constant) = parse_constant_line(key, value, *current_page, *last_offset) {
        // Update last_offset for next constant (offset + size in bytes)
        let size = constant.data_type.size_bytes() as u16 * constant.shape.element_count() as u16;
        *last_offset = constant.offset + size;

        // Resolve $references in bit_options
        if !constant.bit_options.is_empty() {
            constant.bit_options = resolve_bits_options(&def.defines, constant.bit_options);
        }
        def.constants.insert(key.to_string(), constant);
    }
}

/// Parse a comma-separated list of command format strings
/// Applies variable substitution for $varName references
fn parse_command_list(
    value: &str,
    pc_variables: &std::collections::HashMap<String, u8>,
) -> Vec<String> {
    split_ini_line(value)
        .into_iter()
        .map(|s| {
            let s = s.trim().trim_matches('"').to_string();
            substitute_variables(&s, pc_variables)
        })
        .collect()
}

/// Parse page identifiers from INI format
/// e.g., "\x00\x00", "\x00\x01" -> [[0,0], [0,1]]
/// Also handles $variable substitution (e.g., $tsCanId -> 0x00)
fn parse_page_identifiers(
    value: &str,
    pc_variables: &std::collections::HashMap<String, u8>,
) -> Vec<Vec<u8>> {
    split_ini_line(value)
        .into_iter()
        .map(|s| {
            let s = s.trim().trim_matches('"');
            let substituted = substitute_variables(s, pc_variables);
            parse_escape_sequence(&substituted)
        })
        .collect()
}

/// Parse escape sequences like \x00 into bytes
fn parse_escape_sequence(s: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            match chars[i + 1] {
                'x' | 'X' if i + 3 < chars.len() => {
                    // Parse \xNN hex byte
                    let hex: String = chars[i + 2..i + 4].iter().collect();
                    if let Ok(b) = u8::from_str_radix(&hex, 16) {
                        bytes.push(b);
                    }
                    i += 4;
                }
                '0' => {
                    bytes.push(0);
                    i += 2;
                }
                'n' => {
                    bytes.push(b'\n');
                    i += 2;
                }
                'r' => {
                    bytes.push(b'\r');
                    i += 2;
                }
                '\\' => {
                    bytes.push(b'\\');
                    i += 2;
                }
                _ => {
                    bytes.push(chars[i] as u8);
                    i += 1;
                }
            }
        } else {
            bytes.push(chars[i] as u8);
            i += 1;
        }
    }

    bytes
}

/// Parse [OutputChannels] section entries
fn parse_output_channel_entry(def: &mut EcuDefinition, key: &str, value: &str) {
    let key_lower = key.to_lowercase();

    // Handle metadata entries
    if key_lower == "ochblocksize" {
        def.protocol.och_block_size = value.parse().unwrap_or(0);
        return;
    }
    if key_lower == "ochgetcommand" {
        def.protocol.och_get_command = Some(value.trim_matches('"').to_string());
        return;
    }

    if let Some(channel) = parse_output_channel_line(key, value) {
        def.output_channels.insert(key.to_string(), channel);
    }
}

/// Parse [BurstMode] section entries
fn parse_burst_mode_entry(def: &mut EcuDefinition, key: &str, value: &str) {
    let key_lower = key.to_lowercase();
    match key_lower.as_str() {
        "getcommand" => {
            def.protocol.burst_get_command = Some(value.trim_matches('"').to_string());
        }
        _ => {}
    }
}

/// Parse [GaugeConfigurations] section entries
fn parse_gauge_entry(def: &mut EcuDefinition, key: &str, value: &str) {
    if let Some(gauge) = parse_gauge_line(key, value) {
        def.gauges.insert(key.to_string(), gauge);
    }
}

/// Parse [SettingGroups] section entries
fn parse_setting_group_entry(def: &mut EcuDefinition, key: &str, value: &str) {
    if key.eq_ignore_ascii_case("settinggroup") {
        // Format: settingGroup = refName, "Display Name"
        let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
        if parts.len() >= 2 {
            let group = SettingGroup {
                name: parts[0].to_string(),
                label: parts[1].trim_matches('"').to_string(),
                options: Vec::new(),
            };
            def.setting_groups.insert(parts[0].to_string(), group);
        }
    } else if key.eq_ignore_ascii_case("settingoption") {
        // Format: settingOption = value, "Display Label"
        // These apply to the last setting group
        let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
        if parts.len() >= 2 {
            let option = SettingOption {
                value: parts[0].to_string(),
                label: parts[1].trim_matches('"').to_string(),
            };
            // Add to last group - for simplicity, we'll store in a temp structure
            // In a full implementation, we'd track the current group
            if let Some((_name, group)) = def.setting_groups.iter_mut().last() {
                group.options.push(option);
            }
        }
    }
}

/// Parse [PcVariables] section entries
/// These are PC-side variables like tsCanId, rpmwarn, etc.
/// They are stored locally, not on the ECU.
fn parse_pc_variable_entry(def: &mut EcuDefinition, key: &str, value: &str) {
    // Parse as a full constant for UI display
    if let Some(constant) = parse_pc_variable_line(key, value) {
        // Store as a constant so dialogs can look it up
        def.constants.insert(key.to_string(), constant);
    }

    // Also store byte value for command substitution (backward compatibility)
    let parts = split_ini_line(value);
    if parts.is_empty() {
        return;
    }

    let type_str = parts[0].trim().to_uppercase();
    if type_str == "BITS"
        || type_str == "SCALAR"
        || type_str == "U08"
        || type_str == "S08"
        || type_str == "U16"
        || type_str == "S16"
    {
        // Default to index 0
        def.pc_variables.insert(key.to_string(), 0);
    }
}

/// Parse [Defaults] section entries
/// Format: defaultValue = constantName, value
fn parse_defaults_entry(def: &mut EcuDefinition, key: &str, value: &str) {
    let key_lower = key.to_lowercase();

    if key_lower == "defaultvalue" {
        let parts = split_ini_line(value);
        if parts.len() >= 2 {
            let const_name = parts[0].trim().to_string();
            if let Ok(val) = parts[1].trim().parse::<f64>() {
                def.default_values.insert(const_name, val);
            }
        }
    }
}

/// Substitute $variableName or \$variableName references with byte values from pc_variables
/// In INI files, variables can appear as either $varName or \$varName (backslash-escaped)
/// Returns the string with variable references replaced by byte values
fn substitute_variables(
    input: &str,
    pc_variables: &std::collections::HashMap<String, u8>,
) -> String {
    let mut result = String::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Check for \$ (escaped variable reference - common in INI files)
        if chars[i] == '\\' && i + 1 < chars.len() && chars[i + 1] == '$' {
            // Start of escaped variable reference
            let mut var_name = String::new();
            i += 2; // Skip \$

            // Collect variable name (alphanumeric and underscore)
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                var_name.push(chars[i]);
                i += 1;
            }

            if let Some(&byte_val) = pc_variables.get(&var_name) {
                // Replace with the byte value as a character
                result.push(byte_val as char);
            } else {
                // Variable not found - keep the original \$varName
                result.push('\\');
                result.push('$');
                result.push_str(&var_name);
            }
        } else if chars[i] == '$' {
            // Start of unescaped variable reference
            let mut var_name = String::new();
            i += 1;

            // Collect variable name (alphanumeric and underscore)
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                var_name.push(chars[i]);
                i += 1;
            }

            if let Some(&byte_val) = pc_variables.get(&var_name) {
                // Replace with the byte value as a character
                result.push(byte_val as char);
            } else {
                // Variable not found - keep the original $varName
                result.push('$');
                result.push_str(&var_name);
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

/// Parse [Datalog] section entries
fn parse_datalog_entry(def: &mut EcuDefinition, _key: &str, value: &str) {
    // Format: entry = channel, "Label", "%.1f", 1
    let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
    if !parts.is_empty() {
        let entry = DatalogEntry {
            channel: parts[0].trim_matches('"').to_string(),
            label: parts
                .get(1)
                .map(|s| s.trim_matches('"').to_string())
                .unwrap_or_default(),
            format: parts
                .get(2)
                .map(|s| s.trim_matches('"').to_string())
                .unwrap_or_else(|| "%.2f".to_string()),
            enabled: parts.get(3).map(|s| *s != "0").unwrap_or(true),
        };
        def.datalog_entries.push(entry);
    }
}

/// Parse [FrontPage] section entries
/// Handles gauge1-gauge8 references and indicator definitions
fn parse_frontpage_entry(def: &mut EcuDefinition, key: &str, value: &str) {
    // Initialize frontpage if not already
    if def.frontpage.is_none() {
        def.frontpage = Some(FrontPageConfig::default());
    }

    let frontpage = def.frontpage.as_mut().unwrap();

    // Handle gauge1-gauge8 entries
    if key.to_lowercase().starts_with("gauge") {
        if let Some(num_str) = key
            .strip_prefix("gauge")
            .or_else(|| key.strip_prefix("Gauge"))
        {
            if let Ok(num) = num_str.parse::<usize>() {
                // Ensure vector is large enough
                while frontpage.gauges.len() < num {
                    frontpage.gauges.push(String::new());
                }
                if num > 0 && num <= frontpage.gauges.len() {
                    frontpage.gauges[num - 1] = value.trim().to_string();
                }
            }
        }
    }
    // Handle indicator entries
    // Format: indicator = { expression }, "off-label", "on-label", off-bg, off-fg, on-bg, on-fg
    else if key.eq_ignore_ascii_case("indicator") {
        if let Some(indicator) = parse_frontpage_indicator(value) {
            frontpage.indicators.push(indicator);
        }
    }
}

/// Parse a FrontPage indicator definition
/// Format: { expression }, "off-label", "on-label", off-bg, off-fg, on-bg, on-fg
/// Or:     { expression }, "off-label", { dynamic-on-label }, off-bg, off-fg, on-bg, on-fg
fn parse_frontpage_indicator(value: &str) -> Option<FrontPageIndicator> {
    let value = value.trim();

    // Extract the expression (between first { and matching })
    if !value.starts_with('{') {
        return None;
    }

    let mut brace_depth = 0;
    let mut expr_end = 0;
    for (i, ch) in value.char_indices() {
        match ch {
            '{' => brace_depth += 1,
            '}' => {
                brace_depth -= 1;
                if brace_depth == 0 {
                    expr_end = i;
                    break;
                }
            }
            _ => {}
        }
    }

    if expr_end == 0 {
        return None;
    }

    let expression = value[1..expr_end].trim().to_string();
    let remaining = value[expr_end + 1..].trim();

    // Skip the comma after expression
    let remaining = remaining.strip_prefix(',').unwrap_or(remaining).trim();

    // Parse the remaining parts: "off-label", "on-label" or { dynamic }, off-bg, off-fg, on-bg, on-fg
    // Use a custom split that respects braces and quotes
    let parts = split_frontpage_parts(remaining);

    if parts.len() < 6 {
        // Need at least: off-label, on-label, 4 colors
        return None;
    }

    Some(FrontPageIndicator {
        expression,
        label_off: parts[0].trim_matches('"').to_string(),
        label_on: parts[1].trim_matches('"').to_string(),
        bg_off: parts[2].trim().to_string(),
        fg_off: parts[3].trim().to_string(),
        bg_on: parts[4].trim().to_string(),
        fg_on: parts[5].trim().to_string(),
    })
}

/// Split FrontPage indicator parts, respecting braces and quotes
fn split_frontpage_parts(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut brace_depth = 0;

    for ch in s.chars() {
        match ch {
            '"' if brace_depth == 0 => {
                in_quotes = !in_quotes;
                current.push(ch);
            }
            '{' if !in_quotes => {
                brace_depth += 1;
                current.push(ch);
            }
            '}' if !in_quotes => {
                brace_depth -= 1;
                current.push(ch);
            }
            ',' if !in_quotes && brace_depth == 0 => {
                parts.push(current.trim().to_string());
                current = String::new();
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }

    parts
}

/// Parse [TableEditor] section entries
fn parse_table_editor_entry(
    def: &mut EcuDefinition,
    key: &str,
    value: &str,
    current_table: &mut Option<String>,
) {
    if key.eq_ignore_ascii_case("table") {
        // Format: table = tableName, mapName, "Title", page
        // Where mapName is what menus reference (e.g., veTable1Map)
        let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
        if parts.len() >= 3 {
            let mut table = TableDefinition::default();
            table.name = parts[0].to_string();
            // Store map_name (parts[1]) - this is what menus reference
            let map_name = parts[1].to_string();
            table.map_name = Some(map_name.clone());
            table.title = parts
                .get(2)
                .map(|s| s.trim_matches('"').to_string())
                .unwrap_or_default();
            table.page = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);

            // Build reverse lookup: map_name -> table name
            def.table_map_to_name.insert(map_name, table.name.clone());

            *current_table = Some(table.name.clone());
            def.tables.insert(table.name.clone(), table);
        }
    } else if let Some(table_name) = current_table {
        if let Some(table) = def.tables.get_mut(table_name) {
            match key.to_lowercase().as_str() {
                "xbins" => {
                    let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
                    if !parts.is_empty() {
                        table.x_bins = parts[0].to_string();
                        table.x_output_channel = parts.get(1).map(|s| s.to_string());
                    }
                }
                "ybins" => {
                    let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
                    if !parts.is_empty() {
                        table.y_bins = Some(parts[0].to_string());
                        table.y_output_channel = parts.get(1).map(|s| s.to_string());
                        table.table_type = super::tables::TableType::ThreeD;
                    }
                }
                "zbins" => {
                    table.map = value.trim().to_string();
                }
                "gridheight" => {
                    table.grid_height = value.parse().ok();
                }
                "topichelp" => {
                    table.help = Some(value.trim_matches('"').to_string());
                }
                "xylabels" => {
                    // Format: xyLabels = "X Label", "Y Label" or expressions like {bitStringValue(...)}
                    // Use split_ini_line to properly handle expressions with commas inside braces
                    let parts = split_ini_line(value);
                    if !parts.is_empty() {
                        table.x_label = Some(parts[0].trim_matches('"').to_string());
                    }
                    if parts.len() >= 2 {
                        table.y_label = Some(parts[1].trim_matches('"').to_string());
                    }
                }
                _ => {}
            }
        }
    }
}

/// Parse [CurveEditor] section entries
fn parse_curve_editor_entry(
    def: &mut EcuDefinition,
    key: &str,
    value: &str,
    current_curve: &mut Option<String>,
) {
    if key.eq_ignore_ascii_case("curve") {
        // Format: curve = curveName, "Title"
        let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
        if !parts.is_empty() {
            let mut curve = CurveDefinition::default();
            curve.name = parts[0].to_string();
            curve.title = parts
                .get(1)
                .map(|s| s.trim_matches('"').to_string())
                .unwrap_or_default();

            *current_curve = Some(curve.name.clone());
            def.curves.insert(curve.name.clone(), curve);
        }
    } else if let Some(curve_name) = current_curve {
        if let Some(curve) = def.curves.get_mut(curve_name) {
            match key.to_lowercase().as_str() {
                "columnlabel" => {
                    // Format: columnLabel = "X Label", "Y Label"
                    let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
                    if parts.len() >= 2 {
                        curve.column_labels = (
                            parts[0].trim_matches('"').to_string(),
                            parts[1].trim_matches('"').to_string(),
                        );
                    }
                }
                "xaxis" => {
                    // Format: xAxis = min, max, step
                    let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
                    if parts.len() >= 3 {
                        if let (Ok(min), Ok(max), Ok(step)) = (
                            parts[0].parse::<f32>(),
                            parts[1].parse::<f32>(),
                            parts[2].parse::<f32>(),
                        ) {
                            curve.x_axis = Some((min, max, step));
                        }
                    }
                }
                "yaxis" => {
                    // Format: yAxis = min, max, step
                    let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
                    if parts.len() >= 3 {
                        if let (Ok(min), Ok(max), Ok(step)) = (
                            parts[0].parse::<f32>(),
                            parts[1].parse::<f32>(),
                            parts[2].parse::<f32>(),
                        ) {
                            curve.y_axis = Some((min, max, step));
                        }
                    }
                }
                "xbins" => {
                    // Format: xBins = binVariable, displayVariable
                    let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
                    if !parts.is_empty() {
                        curve.x_bins = parts[0].to_string();
                        curve.x_output_channel = parts.get(1).map(|s| s.to_string());
                    }
                }
                "ybins" => {
                    // Format: yBins = valueVariable
                    curve.y_bins = value.trim().to_string();
                }
                "size" => {
                    // Format: size = count
                    curve.size = value.trim().parse().ok();
                }
                "topichelp" => {
                    curve.help = Some(value.trim_matches('"').to_string());
                }
                "gauge" => {
                    // Format: gauge = GaugeName
                    curve.gauge = Some(value.trim().to_string());
                }
                _ => {}
            }
        }
    }
}

/// Post-process: Apply variable substitution to command strings
/// This handles cases where commands (like queryCommand in [MegaTune]) are defined
/// before [PcVariables] section is parsed
fn post_process_variable_substitution(def: &mut EcuDefinition) {
    // Skip if no PC variables defined
    if def.pc_variables.is_empty() {
        return;
    }

    // Substitute in query_command
    def.query_command = substitute_variables(&def.query_command, &def.pc_variables);
    def.protocol.query_command =
        substitute_variables(&def.protocol.query_command, &def.pc_variables);

    // Substitute in version_info (can also contain variables)
    def.version_info = substitute_variables(&def.version_info, &def.pc_variables);

    // Re-process page identifiers and commands if they contain unresolved variables
    // (This handles cases where PcVariables was defined after Constants)
    // Note: The initial parse already applied substitution if PcVariables was parsed first,
    // but we re-apply to handle any remaining $varName references

    // Clone pc_variables to avoid borrow conflicts
    let pc_vars = def.pc_variables.clone();

    for cmd in &mut def.protocol.page_read_commands {
        if cmd.contains('$') {
            *cmd = substitute_variables(cmd, &pc_vars);
        }
    }
    for cmd in &mut def.protocol.page_chunk_write_commands {
        if cmd.contains('$') {
            *cmd = substitute_variables(cmd, &pc_vars);
        }
    }
    for cmd in &mut def.protocol.burn_commands {
        if cmd.contains('$') {
            *cmd = substitute_variables(cmd, &pc_vars);
        }
    }
    for cmd in &mut def.protocol.crc32_check_commands {
        if cmd.contains('$') {
            *cmd = substitute_variables(cmd, &pc_vars);
        }
    }

    // Re-process page identifiers
    // This is trickier because page_identifiers are Vec<Vec<u8>>, not strings
    // We need to check if any identifier bytes look like they might be unsubstituted
    // For simplicity, if pc_variables exist and we have commands with '$', we should
    // store the original command strings and re-parse. But since we've already parsed,
    // we'll trust the initial parse worked for [Constants] section (which comes after [PcVariables])

    // Handle och_get_command
    if let Some(ref cmd) = def.protocol.och_get_command {
        if cmd.contains('$') {
            def.protocol.och_get_command = Some(substitute_variables(cmd, &pc_vars));
        }
    }

    // Handle burst_get_command
    if let Some(ref cmd) = def.protocol.burst_get_command {
        if cmd.contains('$') {
            def.protocol.burst_get_command = Some(substitute_variables(cmd, &pc_vars));
        }
    }
}

/// Post-process menu items to fix types that couldn't be determined during initial parse
/// This handles cases where help topics are defined after the Menu section
fn post_process_menu_items(def: &mut EcuDefinition) {
    for menu in &mut def.menus {
        post_process_items(
            &mut menu.items,
            &def.help_topics,
            &def.tables,
            &def.table_map_to_name,
            &def.curves,
        );
    }
}

fn post_process_items(
    items: &mut Vec<MenuItem>,
    help_topics: &std::collections::HashMap<String, HelpTopic>,
    tables: &std::collections::HashMap<String, crate::ini::TableDefinition>,
    table_map_to_name: &std::collections::HashMap<String, String>,
    curves: &std::collections::HashMap<String, crate::ini::CurveDefinition>,
) {
    for item in items {
        match item {
            MenuItem::Dialog {
                label,
                target,
                visibility_condition,
                enabled_condition,
            } => {
                // Check if this should be a different type
                if help_topics.contains_key(target) {
                    *item = MenuItem::Help {
                        label: label.clone(),
                        target: target.clone(),
                        visibility_condition: visibility_condition.clone(),
                        enabled_condition: enabled_condition.clone(),
                    };
                } else if tables.contains_key(target)
                    || table_map_to_name.contains_key(target)
                    || curves.contains_key(target)
                {
                    // Target can be either the table name or the map name
                    *item = MenuItem::Table {
                        label: label.clone(),
                        target: target.clone(),
                        visibility_condition: visibility_condition.clone(),
                        enabled_condition: enabled_condition.clone(),
                    };
                }
            }
            MenuItem::SubMenu {
                items: sub_items, ..
            } => {
                post_process_items(sub_items, help_topics, tables, table_map_to_name, curves);
            }
            _ => {}
        }
    }
}

/// Post-process tables to resolve x_size and y_size from referenced constant shapes
///
/// TableEditor entries reference constants by name (e.g., xBins = veRpmBins, zBins = veTable)
/// but the table's x_size/y_size aren't set during initial parsing. This function looks up
/// each constant and extracts dimensions from its Shape.
fn post_process_table_sizes(def: &mut EcuDefinition) {
    // Collect table names first to avoid borrow issues
    let table_names: Vec<String> = def.tables.keys().cloned().collect();

    for table_name in table_names {
        // Get the constant names we need to look up
        let (x_bins_name, y_bins_name, map_name) = {
            let table = match def.tables.get(&table_name) {
                Some(t) => t,
                None => continue,
            };
            (
                table.x_bins.clone(),
                table.y_bins.clone(),
                table.map.clone(),
            )
        };

        // Look up x_size from x_bins constant
        let x_size = if let Some(x_const) = def.constants.get(&x_bins_name) {
            x_const.shape.x_size()
        } else {
            0
        };

        // Look up y_size from y_bins constant (if 3D table) or map constant
        let y_size = if let Some(ref y_bins_name) = y_bins_name {
            if let Some(y_const) = def.constants.get(y_bins_name) {
                y_const.shape.x_size() // y_bins is a 1D array, use its size
            } else {
                1
            }
        } else {
            1 // 2D table has y_size = 1
        };

        // Alternatively, infer from map constant shape if we have a 2D map
        let (final_x_size, final_y_size) = if let Some(map_const) = def.constants.get(&map_name) {
            match &map_const.shape {
                crate::ini::types::Shape::Array2D { rows, cols } => {
                    // Use map's actual dimensions if available
                    (*cols, *rows)
                }
                crate::ini::types::Shape::Array1D(size) => {
                    // 1D map - use x_size from bins, y_size = 1
                    if x_size > 0 {
                        (x_size, 1)
                    } else {
                        (*size, 1)
                    }
                }
                crate::ini::types::Shape::Scalar => (x_size, y_size),
            }
        } else {
            (x_size, y_size)
        };

        // Update the table - only update if we calculated a meaningful size
        // x_size defaults to 0, y_size defaults to 1 in TableDefinition::default()
        if let Some(table) = def.tables.get_mut(&table_name) {
            if table.x_size == 0 && final_x_size > 0 {
                table.x_size = final_x_size;
            }
            if table.y_size <= 1 && final_y_size > 1 {
                table.y_size = final_y_size;
            }
        }
    }
}

/// Parse [Menu] section entries
fn parse_menu_entry(def: &mut EcuDefinition, key: &str, value: &str) {
    let key = key.to_lowercase();
    match key.as_str() {
        "menu" => {
            let parts = split_ini_line(value);
            if !parts.is_empty() {
                let name = parts[0].trim_matches('"').to_string();
                let title = parts[0].trim_matches('"').to_string(); // Use same as name for key
                def.menus.push(Menu {
                    name,
                    title,
                    items: Vec::new(),
                });
            }
        }
        "submenu" | "groupchildmenu" => {
            let parts = split_ini_line(value);
            if parts.is_empty() {
                return;
            }

            // Handle multiple INI formats:
            // 1. Standard comma-separated: target, "Label", { condition }
            // 2. Space-separated: target    "Label", { condition }  (no comma between target and label)
            // 3. Dual-condition: target "Label", { visibility_cond }, { enable_cond }
            // According to EFI Analytics spec:
            //   - 1 expression = enable/disable condition
            //   - 2 expressions = visibility (first) and enable (second)
            let (target, label, visibility_condition, enabled_condition) = {
                let first_part = parts[0].trim();

                // Helper to extract condition from a part
                let extract_condition = |s: &str| -> Option<String> {
                    let trimmed = s.trim_matches(|c| c == '{' || c == '}').trim();
                    if trimmed.is_empty() || trimmed == "0" || trimmed == "1" {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                };

                // Check if first part contains both target and quoted label (space-separated format)
                if let Some(quote_pos) = first_part.find('"') {
                    // Format 2 or 3: target "Label" in parts[0]
                    let target = first_part[..quote_pos].trim().to_string();
                    // Extract the quoted label - find matching end quote
                    let label_start = quote_pos + 1;
                    let label = if let Some(end_quote) = first_part[label_start..].find('"') {
                        first_part[label_start..label_start + end_quote].to_string()
                    } else {
                        first_part[label_start..].trim_matches('"').to_string()
                    };

                    // Check for 2 conditions (visibility + enable)
                    let (vis_cond, en_cond) = if parts.len() >= 3 {
                        // Two conditions: first is visibility, second is enable
                        (extract_condition(&parts[1]), extract_condition(&parts[2]))
                    } else if parts.len() >= 2 {
                        // One condition: it's the enable condition
                        (None, extract_condition(&parts[1]))
                    } else {
                        (None, None)
                    };
                    (target, label, vis_cond, en_cond)
                } else if parts.len() >= 2 {
                    // Format 1: Standard comma-separated
                    let target = first_part.trim_matches('"').to_string();
                    let label = parts[1].trim_matches('"').to_string();

                    // Check for 2 conditions (visibility + enable)
                    let (vis_cond, en_cond) = if parts.len() >= 4 {
                        // Two conditions: first is visibility, second is enable
                        (extract_condition(&parts[2]), extract_condition(&parts[3]))
                    } else if parts.len() >= 3 {
                        // One condition: it's the enable condition
                        (None, extract_condition(&parts[2]))
                    } else {
                        (None, None)
                    };
                    (target, label, vis_cond, en_cond)
                } else {
                    // Only target, no label - use target as label
                    (first_part.to_string(), first_part.to_string(), None, None)
                }
            };

            // Skip invalid entries
            if target.is_empty() {
                return;
            }

            // Determine the correct MenuItem type based on target
            let item = if target == "std_separator" {
                // Separator - render as visual divider, not a clickable item
                MenuItem::Separator
            } else if target.starts_with("std_") {
                // Built-in standard targets like std_realtime, std_ms2gentherm, etc.
                MenuItem::Std {
                    label,
                    target,
                    visibility_condition,
                    enabled_condition,
                }
            } else if def.help_topics.contains_key(&target) {
                // Help topic reference
                MenuItem::Help {
                    label,
                    target,
                    visibility_condition,
                    enabled_condition,
                }
            } else if def.tables.contains_key(&target) || def.curves.contains_key(&target) {
                // Table or curve reference
                MenuItem::Table {
                    label,
                    target,
                    visibility_condition,
                    enabled_condition,
                }
            } else {
                // Default to dialog
                MenuItem::Dialog {
                    label,
                    target,
                    visibility_condition,
                    enabled_condition,
                }
            };

            if let Some(menu) = def.menus.last_mut() {
                if key == "groupchildmenu" {
                    if let Some(MenuItem::SubMenu { items, .. }) = menu.items.last_mut() {
                        items.push(item);
                        return;
                    }
                }
                menu.items.push(item);
            }
        }
        "groupmenu" => {
            let parts = split_ini_line(value);
            if !parts.is_empty() {
                let label = parts[0].trim_matches('"').to_string();

                // Extract conditions: 1 = enable, 2 = visibility + enable
                let extract_condition = |s: &str| -> Option<String> {
                    let trimmed = s.trim_matches(|c| c == '{' || c == '}').trim();
                    if trimmed.is_empty() || trimmed == "0" || trimmed == "1" {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                };

                let (visibility_condition, enabled_condition) = if parts.len() >= 3 {
                    // Two conditions: first is visibility, second is enable
                    (extract_condition(&parts[1]), extract_condition(&parts[2]))
                } else if parts.len() >= 2 {
                    // One condition: it's the enable condition
                    (None, extract_condition(&parts[1]))
                } else {
                    (None, None)
                };

                if let Some(menu) = def.menus.last_mut() {
                    menu.items.push(MenuItem::SubMenu {
                        label,
                        items: Vec::new(),
                        visibility_condition,
                        enabled_condition,
                    });
                }
            }
        }
        "std_separator" => {
            if let Some(menu) = def.menus.last_mut() {
                menu.items.push(MenuItem::Separator);
            }
        }
        _ => {}
    }
}

/// Parse [SettingContextHelp] section entries
/// Format: constantName = "Help text for this constant"
fn parse_setting_context_help(def: &mut EcuDefinition, key: &str, value: &str) {
    // The key is the constant name, value is the help/label text
    if let Some(constant) = def.constants.get_mut(key) {
        constant.label = Some(value.trim_matches('"').to_string());
    }
}

/// Parse [UserDefined] section entries
fn parse_user_defined_entry(
    def: &mut EcuDefinition,
    key: &str,
    value: &str,
    current_dialog: &mut Option<String>,
    current_indicator_panel: &mut Option<String>,
    current_help: &mut Option<String>,
) {
    let key = key.to_lowercase();
    match key.as_str() {
        "help" => {
            // Format: help = name, "Title"
            let parts = split_ini_line(value);
            if !parts.is_empty() {
                let name = parts[0].to_string();
                let title = parts
                    .get(1)
                    .map(|s| s.trim_matches('"').to_string())
                    .unwrap_or_else(|| name.clone());

                let help_topic = HelpTopic {
                    name: name.clone(),
                    title,
                    web_url: None,
                    text_lines: Vec::new(),
                };
                def.help_topics.insert(name.clone(), help_topic);
                *current_help = Some(name);
            }
        }
        "webhelp" => {
            // Format: webHelp = "https://example.com/help/topic"
            if let Some(name) = current_help {
                if let Some(help_topic) = def.help_topics.get_mut(name) {
                    help_topic.web_url = Some(value.trim().trim_matches('"').to_string());
                }
            }
        }
        "text" => {
            // Format: text = "Help text line"
            if let Some(name) = current_help {
                if let Some(help_topic) = def.help_topics.get_mut(name) {
                    help_topic
                        .text_lines
                        .push(value.trim().trim_matches('"').to_string());
                }
            }
        }
        "dialog" => {
            let parts = split_ini_line(value);
            if !parts.is_empty() {
                let name = parts[0].to_string();
                let title = parts
                    .get(1)
                    .map(|s| s.trim_matches('"').to_string())
                    .unwrap_or_else(|| name.clone());

                let dialog = DialogDefinition {
                    name: name.clone(),
                    title,
                    components: Vec::new(),
                };
                def.dialogs.insert(name.clone(), dialog);
                *current_dialog = Some(name);
            }
        }
        "indicatorpanel" => {
            // Format: indicatorPanel = name, columns [, {visibility_condition}]
            let parts = split_ini_line(value);
            if parts.len() >= 2 {
                let name = parts[0].to_string();
                let columns = parts[1].parse::<u8>().unwrap_or(2);

                // Check for visibility condition (last part in braces)
                let visibility_condition = parts
                    .iter()
                    .skip(2)
                    .find(|p| {
                        let trimmed = p.trim();
                        trimmed.starts_with('{') && trimmed.ends_with('}')
                    })
                    .map(|p| p.trim().trim_matches(|c| c == '{' || c == '}').to_string());

                let panel = IndicatorPanel {
                    name: name.clone(),
                    columns,
                    visibility_condition,
                    indicators: Vec::new(),
                };
                def.indicator_panels.insert(name.clone(), panel);
                *current_indicator_panel = Some(name);
            }
        }
        "panel" => {
            if let Some(name) = current_dialog {
                if let Some(dialog) = def.dialogs.get_mut(name) {
                    let parts = split_ini_line(value);
                    let panel_name = parts.first().unwrap_or(&String::new()).trim().to_string();
                    // Check for visibility condition in curly braces (last part)
                    let visibility_condition = parts
                        .iter()
                        .skip(1)
                        .find(|p| p.trim().starts_with('{') && p.trim().ends_with('}'))
                        .map(|p| p.trim().trim_matches(|c| c == '{' || c == '}').to_string());
                    dialog.components.push(DialogComponent::Panel {
                        name: panel_name,
                        visibility_condition,
                    });
                }
            }
        }
        "livegraph" => {
            if let Some(name) = current_dialog {
                if let Some(dialog) = def.dialogs.get_mut(name) {
                    let parts = split_ini_line(value);
                    if parts.len() >= 2 {
                        dialog.components.push(DialogComponent::LiveGraph {
                            name: parts[0].to_string(),
                            title: parts[1].trim_matches('"').to_string(),
                            position: parts.get(2).cloned().unwrap_or_else(|| "South".to_string()),
                            channels: Vec::new(),
                        });
                    }
                }
            }
        }
        "graphline" => {
            if let Some(name) = current_dialog {
                if let Some(dialog) = def.dialogs.get_mut(name) {
                    if let Some(DialogComponent::LiveGraph { channels, .. }) =
                        dialog.components.last_mut()
                    {
                        channels.push(value.trim().to_string());
                    }
                }
            }
        }
        "field" => {
            if let Some(name) = current_dialog {
                if let Some(dialog) = def.dialogs.get_mut(name) {
                    let parts = split_ini_line(value);
                    if parts.len() >= 2 {
                        let label = parts[0].trim_matches('"').to_string();
                        let field_name = parts[1].trim().to_string();
                        // Check for conditions in curly braces
                        // Format: field = "Label", name, {visibility}, {enable}
                        // Or: field = "Label", name, {condition} (single condition = enable)
                        let conditions: Vec<String> = parts
                            .iter()
                            .skip(2)
                            .filter_map(|p| {
                                let trimmed = p.trim();
                                if trimmed.starts_with('{') && trimmed.ends_with('}') {
                                    Some(trimmed.trim_matches(|c| c == '{' || c == '}').to_string())
                                } else {
                                    None
                                }
                            })
                            .collect();

                        let visibility_condition = if conditions.len() >= 2 {
                            Some(conditions[0].clone())
                        } else {
                            None
                        };

                        let enabled_condition = if conditions.len() >= 2 {
                            Some(conditions[1].clone())
                        } else if conditions.len() == 1 {
                            Some(conditions[0].clone())
                        } else {
                            None
                        };

                        dialog.components.push(DialogComponent::Field {
                            label,
                            name: field_name,
                            visibility_condition,
                            enabled_condition,
                        });
                    } else if !parts.is_empty() {
                        let text = parts[0].trim_matches('"').to_string();
                        dialog.components.push(DialogComponent::Label { text });
                    }
                }
            }
        }
        "table" => {
            if let Some(name) = current_dialog {
                if let Some(dialog) = def.dialogs.get_mut(name) {
                    dialog.components.push(DialogComponent::Table {
                        name: value.trim().to_string(),
                    });
                }
            }
        }
        "indicator" => {
            // Check if we're in an indicatorPanel or a dialog
            if let Some(panel_name) = current_indicator_panel {
                if let Some(panel) = def.indicator_panels.get_mut(panel_name) {
                    // Format: indicator = {expression}, "label_off", "label_on" [, color_off_fg, color_off_bg, color_on_fg, color_on_bg]
                    let parts = split_ini_line(value);
                    if parts.len() >= 3 {
                        let expression =
                            parts[0].trim_matches(|c| c == '{' || c == '}').to_string();
                        let label_off = parts[1].trim_matches('"').to_string();
                        let label_on = parts[2].trim_matches('"').to_string();

                        // Optional colors (parts 3-6)
                        let color_off_fg = parts.get(3).map(|s| s.trim().to_string());
                        let color_off_bg = parts.get(4).map(|s| s.trim().to_string());
                        let color_on_fg = parts.get(5).map(|s| s.trim().to_string());
                        let color_on_bg = parts.get(6).map(|s| s.trim().to_string());

                        panel.indicators.push(IndicatorDefinition {
                            expression,
                            label_off,
                            label_on,
                            color_off_fg,
                            color_off_bg,
                            color_on_fg,
                            color_on_bg,
                        });
                    }
                }
            } else if let Some(name) = current_dialog {
                if let Some(dialog) = def.dialogs.get_mut(name) {
                    let parts = split_ini_line(value);
                    if parts.len() >= 3 {
                        dialog.components.push(DialogComponent::Indicator {
                            expression: parts[0].trim_matches(|c| c == '{' || c == '}').to_string(),
                            label_off: parts[1].trim_matches('"').to_string(),
                            label_on: parts[2].trim_matches('"').to_string(),
                        });
                    }
                }
            }
        }
        "commandbutton" => {
            // Format: commandButton = "Label", command_name [, { condition }] [, clickOnClose|clickOnCloseIfEnabled|clickOnCloseIfDisabled]
            if let Some(name) = current_dialog {
                if let Some(dialog) = def.dialogs.get_mut(name) {
                    let parts = split_ini_line(value);
                    if parts.len() >= 2 {
                        let label = parts[0].trim_matches('"').to_string();
                        let command = parts[1].trim().to_string();
                        
                        // Parse optional condition and close behavior
                        let mut enabled_condition = None;
                        let mut on_close_behavior = None;
                        
                        for part in parts.iter().skip(2) {
                            let trimmed = part.trim();
                            if trimmed.starts_with('{') && trimmed.ends_with('}') {
                                enabled_condition = Some(trimmed.trim_matches(|c| c == '{' || c == '}').to_string());
                            } else {
                                match trimmed.to_lowercase().as_str() {
                                    "clickoncloseifenabled" => {
                                        on_close_behavior = Some(CommandButtonCloseAction::ClickOnCloseIfEnabled);
                                    }
                                    "clickoncloseifdisabled" => {
                                        on_close_behavior = Some(CommandButtonCloseAction::ClickOnCloseIfDisabled);
                                    }
                                    "clickonclose" => {
                                        on_close_behavior = Some(CommandButtonCloseAction::ClickOnClose);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        
                        dialog.components.push(DialogComponent::CommandButton {
                            label,
                            command,
                            enabled_condition,
                            on_close_behavior,
                        });
                    }
                }
            }
        }
        _ => {}
    }
}

/// Parse a [ControllerCommands] section entry
/// Format: cmd_name = "command_string" or cmd_name = cmd_ref1, "raw_bytes", cmd_ref2, ...
/// Command chaining: commands can reference other commands by name
fn parse_controller_command_entry(def: &mut EcuDefinition, key: &str, value: &str) {
    let parts = split_ini_line(value);
    if parts.is_empty() {
        return;
    }
    
    let name = key.to_string();
    let mut command_parts: Vec<CommandPart> = Vec::new();
    let mut enable_condition = None;
    
    for part in &parts {
        let trimmed = part.trim();
        
        // Check for enable condition (in braces)
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            enable_condition = Some(trimmed.trim_matches(|c| c == '{' || c == '}').to_string());
            continue;
        }
        
        // Check if it's a quoted string (raw bytes) or a command reference
        if trimmed.starts_with('"') && trimmed.ends_with('"') {
            // Raw command string with potential hex escapes
            let raw = trimmed.trim_matches('"').to_string();
            command_parts.push(CommandPart::Raw(raw));
        } else if !trimmed.is_empty() {
            // Command reference (another command name)
            command_parts.push(CommandPart::Reference(trimmed.to_string()));
        }
    }
    
    // If only one part and no parts parsed yet, it might be the old format "cmd_name = raw_string"
    if command_parts.is_empty() && !parts.is_empty() {
        command_parts.push(CommandPart::Raw(parts[0].trim_matches('"').to_string()));
    }
    
    // Use command name as label (no separate label in new format)
    let label = name.clone();
    
    def.controller_commands.insert(
        name.clone(),
        ControllerCommand {
            name,
            label,
            parts: command_parts,
            enable_condition,
        },
    );
}

/// Parse a [LoggerDefinition] section entry
/// Format: logger_name = "Label", sample_rate, channel1, channel2, ... [, {enable_condition}]
fn parse_logger_definition_entry(def: &mut EcuDefinition, key: &str, value: &str) {
    let parts = split_ini_line(value);
    if parts.len() >= 3 {
        let name = key.to_string();
        let label = parts[0].trim_matches('"').to_string();
        let sample_rate = parts[1].parse::<f64>().unwrap_or(100.0);

        // Channels are all parts between sample_rate and optional condition
        let mut channels = Vec::new();
        let mut enable_condition = None;

        for part in parts.iter().skip(2) {
            let trimmed = part.trim();
            if trimmed.starts_with('{') && trimmed.ends_with('}') {
                enable_condition = Some(trimmed.trim_matches(|c| c == '{' || c == '}').to_string());
                break;
            } else {
                channels.push(trimmed.to_string());
            }
        }

        def.logger_definitions.insert(
            name.clone(),
            LoggerDefinition {
                name,
                label,
                sample_rate,
                channels,
                enable_condition,
            },
        );
    }
}

/// Parse a [PortEditor] section entry
/// Format: port_name = "Label" [, {enable_condition}]
fn parse_port_editor_entry(def: &mut EcuDefinition, key: &str, value: &str) {
    let parts = split_ini_line(value);
    if !parts.is_empty() {
        let name = key.to_string();
        let label = parts[0].trim_matches('"').to_string();

        // Optional enable condition (last part in braces)
        let enable_condition = parts
            .iter()
            .skip(1)
            .find(|p| {
                let trimmed = p.trim();
                trimmed.starts_with('{') && trimmed.ends_with('}')
            })
            .map(|p| p.trim().trim_matches(|c| c == '{' || c == '}').to_string());

        def.port_editors.insert(
            name.clone(),
            PortEditorConfig {
                name,
                label,
                enable_condition,
            },
        );
    }
}

/// Parse a [ReferenceTables] section entry
/// Format: ref_name = "Label", table_name [, {enable_condition}]
fn parse_reference_table_entry(def: &mut EcuDefinition, key: &str, value: &str) {
    let parts = split_ini_line(value);
    if parts.len() >= 2 {
        let name = key.to_string();
        let label = parts[0].trim_matches('"').to_string();
        let table_name = parts[1].trim().to_string();

        // Optional enable condition (last part in braces)
        let enable_condition = parts
            .iter()
            .skip(2)
            .find(|p| {
                let trimmed = p.trim();
                trimmed.starts_with('{') && trimmed.ends_with('}')
            })
            .map(|p| p.trim().trim_matches(|c| c == '{' || c == '}').to_string());

        def.reference_tables.insert(
            name.clone(),
            ReferenceTable {
                name,
                label,
                table_name,
                enable_condition,
            },
        );
    }
}

/// Parse a [FTPBrowser] section entry
/// Format: browser_name = "Label", server, port [, {enable_condition}]
fn parse_ftp_browser_entry(def: &mut EcuDefinition, key: &str, value: &str) {
    let parts = split_ini_line(value);
    if parts.len() >= 3 {
        let name = key.to_string();
        let label = parts[0].trim_matches('"').to_string();
        let server = parts[1].trim().to_string();
        let port = parts[2].parse::<u16>().unwrap_or(21);

        // Optional enable condition (last part in braces)
        let enable_condition = parts
            .iter()
            .skip(3)
            .find(|p| {
                let trimmed = p.trim();
                trimmed.starts_with('{') && trimmed.ends_with('}')
            })
            .map(|p| p.trim().trim_matches(|c| c == '{' || c == '}').to_string());

        def.ftp_browsers.insert(
            name.clone(),
            FTPBrowserConfig {
                name,
                label,
                server,
                port,
                enable_condition,
            },
        );
    }
}

/// Parse a [DatalogViews] section entry
/// Format: view_name = "Label", channel1, channel2, ...
fn parse_datalog_view_entry(def: &mut EcuDefinition, key: &str, value: &str) {
    let parts = split_ini_line(value);
    if parts.len() >= 2 {
        let name = key.to_string();
        let label = parts[0].trim_matches('"').to_string();
        let channels: Vec<String> = parts.iter().skip(1).map(|s| s.trim().to_string()).collect();

        def.datalog_views.insert(
            name.clone(),
            DatalogView {
                name,
                label,
                channels,
            },
        );
    }
}

/// Parse a [KeyActions] section entry
/// Format: key = action, "Label" [, {enable_condition}]
/// Or: showPanel = key, panel_name
fn parse_key_action_entry(def: &mut EcuDefinition, key: &str, value: &str) {
    let parts = split_ini_line(value);
    if key.eq_ignore_ascii_case("showPanel") && parts.len() >= 2 {
        // Special case: showPanel = key, panel_name
        let key_combo = parts[0].trim().to_string();
        let action = format!("showPanel:{}", parts[1].trim());
        let label = format!("Show {}", parts[1].trim());

        def.key_actions.push(KeyAction {
            key: key_combo,
            action,
            label,
            enable_condition: None,
        });
    } else if parts.len() >= 2 {
        // Format: key = action, "Label" [, {enable_condition}]
        let key_combo = key.to_string();
        let action = parts[0].trim().to_string();
        let label = parts[1].trim_matches('"').to_string();

        // Optional enable condition (last part in braces)
        let enable_condition = parts
            .iter()
            .skip(2)
            .find(|p| {
                let trimmed = p.trim();
                trimmed.starts_with('{') && trimmed.ends_with('}')
            })
            .map(|p| p.trim().trim_matches(|c| c == '{' || c == '}').to_string());

        def.key_actions.push(KeyAction {
            key: key_combo,
            action,
            label,
            enable_condition,
        });
    }
}

/// Parse a VeAnalyze section entry
fn parse_ve_analyze_entry(def: &mut EcuDefinition, key: &str, value: &str) {
    // Initialize VeAnalyze config if not present
    if def.ve_analyze.is_none() {
        def.ve_analyze = Some(VeAnalyzeConfig::default());
    }
    let config = def.ve_analyze.as_mut().unwrap();

    let key_lower = key.to_lowercase();
    let parts = split_ini_line(value);

    match key_lower.as_str() {
        "veanalyzemap" => {
            // veAnalyzeMap = veTableTbl, lambdaTableTbl, lambdaValue, egoCorrectionForVeAnalyze, { 1 }
            if parts.len() >= 5 {
                config.ve_table_name = parts[0].trim().to_string();
                config.target_table_name = parts[1].trim().to_string();
                config.lambda_channel = parts[2].trim().to_string();
                config.ego_correction_channel = parts[3].trim().to_string();
                config.active_condition = parts[4].trim().to_string();
            }
        }
        "lambdatargettables" => {
            // lambdaTargetTables = lambdaTableTbl, afrTSCustom
            config.lambda_target_tables = parts.iter().map(|s| s.trim().to_string()).collect();
        }
        "filter" => {
            if let Some(filter) = parse_analysis_filter(value) {
                config.filters.push(filter);
            }
        }
        "option" => {
            config.options.push(value.trim().to_string());
        }
        _ => {}
    }
}

/// Parse a WueAnalyze section entry
fn parse_wue_analyze_entry(def: &mut EcuDefinition, key: &str, value: &str) {
    // Initialize WueAnalyze config if not present
    if def.wue_analyze.is_none() {
        def.wue_analyze = Some(WueAnalyzeConfig::default());
    }
    let config = def.wue_analyze.as_mut().unwrap();

    let key_lower = key.to_lowercase();
    let parts = split_ini_line(value);

    match key_lower.as_str() {
        "wueanalyzemap" => {
            // wueAnalyzeMap = wueCurveName, afrTempCompCurve, targetTableName, lambdaChannel, coolantChannel, wueChannel, egoCorrectionChannel
            if parts.len() >= 7 {
                config.wue_curve_name = parts[0].trim().to_string();
                config.afr_temp_comp_curve = parts[1].trim().to_string();
                config.target_table_name = parts[2].trim().to_string();
                config.lambda_channel = parts[3].trim().to_string();
                config.coolant_channel = parts[4].trim().to_string();
                config.wue_channel = parts[5].trim().to_string();
                config.ego_correction_channel = parts[6].trim().to_string();
            }
        }
        "lambdatargettables" => {
            config.lambda_target_tables = parts.iter().map(|s| s.trim().to_string()).collect();
        }
        "wuepercentoffset" => {
            config.wue_percent_offset = value.trim().parse().unwrap_or(0.0);
        }
        "filter" => {
            if let Some(filter) = parse_analysis_filter(value) {
                config.filters.push(filter);
            }
        }
        "option" => {
            config.options.push(value.trim().to_string());
        }
        _ => {}
    }
}

/// Parse a GammaE section entry
fn parse_gamma_e_entry(def: &mut EcuDefinition, key: &str, value: &str) {
    // Initialize GammaE config if not present
    if def.gamma_e.is_none() {
        def.gamma_e = Some(GammaEConfig::default());
    }
    let config = def.gamma_e.as_mut().unwrap();

    let key_lower = key.to_lowercase();
    let parts = split_ini_line(value);

    match key_lower.as_str() {
        "gammaemap" | "gammaetable" => {
            // gammaETable = gammaTableTbl, lambdaChannel, targetTableName
            if parts.len() >= 3 {
                config.gamma_table_name = parts[0].trim().to_string();
                config.lambda_channel = parts[1].trim().to_string();
                config.target_table_name = parts[2].trim().to_string();
            }
        }
        "filter" => {
            if let Some(filter) = parse_analysis_filter(value) {
                config.filters.push(filter);
            }
        }
        "option" => {
            config.options.push(value.trim().to_string());
        }
        _ => {}
    }
}

/// Parse an analysis filter line
/// Format: filter = name, "Display Name", channel, operator, defaultValue, userAdjustable
/// Or: filter = std_Custom (standard custom filter)
fn parse_analysis_filter(value: &str) -> Option<AnalysisFilter> {
    let parts = split_ini_line(value);
    
    // Handle standard filters like "std_Custom", "std_DeadLambda"
    if parts.len() == 1 {
        let name = parts[0].trim();
        if name.starts_with("std_") {
            return Some(AnalysisFilter {
                name: name.to_string(),
                display_name: name.to_string(),
                channel: String::new(),
                operator: FilterOperator::Equal,
                default_value: 0.0,
                user_adjustable: false,
            });
        }
        return None;
    }
    
    // Full filter format: name, "displayName", channel, operator, defaultValue, userAdjustable
    if parts.len() >= 5 {
        let name = parts[0].trim().to_string();
        let display_name = parts[1].trim().trim_matches('"').to_string();
        let channel = parts[2].trim().to_string();
        let operator = FilterOperator::from_str(parts[3].trim()).unwrap_or(FilterOperator::Equal);
        let default_value = parts[4].trim().parse().unwrap_or(0.0);
        let user_adjustable = if parts.len() > 5 {
            parts[5].trim().to_lowercase() == "true"
        } else {
            false
        };
        
        return Some(AnalysisFilter {
            name,
            display_name,
            channel,
            operator,
            default_value,
            user_adjustable,
        });
    }
    
    None
}

/// Parse [ConstantsExtensions] section entries
/// Handles:
/// - defaultValue = name, value1 value2 value3 ... (sets default array values for constants)
/// - maintainConstantValue = name, { expression } (auto-update expressions)
/// - requiresPowerCycle = name (mark constant as requiring ECU power cycle)
fn parse_constants_extensions_entry(def: &mut EcuDefinition, key: &str, value: &str) {
    let key_lower = key.to_lowercase();
    
    match key_lower.as_str() {
        "defaultvalue" => {
            // Format: defaultValue = constName, val1 val2 val3 ...
            // or: defaultValue = constName, val
            let parts = split_ini_line(value);
            if parts.len() >= 2 {
                let const_name = parts[0].trim().to_string();
                // The rest is space-separated values (for arrays) or a single value
                let values_str = parts[1..].join(",");
                // Store in default_values as f64 (for single values) or parse array
                // For now, parse the first value as f64 for simple scalar defaults
                let value_parts: Vec<&str> = values_str.split_whitespace().collect();
                if let Some(first) = value_parts.first() {
                    if let Ok(val) = first.trim().parse::<f64>() {
                        def.default_values.insert(const_name, val);
                    }
                }
            }
        }
        "maintainconstantvalue" => {
            // Format: maintainConstantValue = constName, { expression }
            // The expression auto-updates the constant value
            if let Some(brace_start) = value.find('{') {
                if let Some(brace_end) = value.rfind('}') {
                    let const_name = value[..brace_start].trim_end_matches(',').trim().to_string();
                    let expression = value[brace_start + 1..brace_end].trim().to_string();
                    
                    def.maintain_constant_values.push(MaintainConstantValue {
                        constant_name: const_name,
                        expression,
                    });
                }
            }
        }
        "requirespowercycle" => {
            // Format: requiresPowerCycle = constName
            let const_name = value.trim().to_string();
            if !const_name.is_empty() {
                def.requires_power_cycle.push(const_name);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_comment() {
        assert_eq!(strip_comment("value ; comment"), "value ");
        assert_eq!(
            strip_comment("\"value ; with semi\""),
            "\"value ; with semi\""
        );
        // Note: # is now handled at the line level for preprocessor directives,
        // so strip_comment doesn't remove # comment lines anymore
        assert_eq!(strip_comment("# comment line"), "# comment line");
    }

    #[test]
    fn test_parse_key_value() {
        assert_eq!(parse_key_value("key = value"), Some(("key", "value")));
        assert_eq!(parse_key_value("key=value"), Some(("key", "value")));
        assert_eq!(parse_key_value("no equals"), None);
    }

    #[test]
    fn test_parse_basic_ini() {
        let content = r#"
[MegaTune]
signature = "speeduino 202310"
queryCommand = "Q"
versionInfo = "Speeduino 2023.10"

[TunerStudio]
iniSpecVersion = 3.64
nPages = 2
pageSize = 288, 128

[Constants]
page = 1
reqFuel = scalar, U16, 0, "ms", 0.1, 0.0, 0, 25.5, 1

[OutputChannels]
rpm = U16, 0, "RPM", 1.0, 0.0
"#;

        let def = parse_ini(content).expect("Should parse successfully");

        assert_eq!(def.signature, "speeduino 202310");
        assert_eq!(def.query_command, "Q");
        assert_eq!(def.n_pages, 2);
        assert_eq!(def.page_sizes, vec![288, 128]);

        assert!(def.constants.contains_key("reqFuel"));
        assert!(def.output_channels.contains_key("rpm"));
    }

    #[test]
    fn test_parse_rusefi_ini() {
        let path = "/home/pat/.gemini/antigravity/scratch/libretune/definitions/rusEFI2025062101.2025.06.22.epicECU.1005735475.ini";
        if std::path::Path::new(path).exists() {
            let content = std::fs::read_to_string(path).expect("Should read file");
            let def = parse_ini(&content).expect("Should parse successfully");

            // Check for a specific table
            assert!(
                def.tables.contains_key("ignitionTableTbl")
                    || def.tables.contains_key("ignitionTable")
            );

            // Check for menus
            assert!(!def.menus.is_empty());
            assert!(def.menus.iter().any(|m| m.title.contains("Fuel")));

            // Check for dialogs
            assert!(!def.dialogs.is_empty());
            assert!(def.dialogs.contains_key("fuel_computerDialog"));
        }
    }

    #[test]
    fn test_substitute_variables() {
        use std::collections::HashMap;

        let mut vars = HashMap::new();
        vars.insert("tsCanId".to_string(), 0u8);

        // Test simple substitution (unescaped $)
        assert_eq!(substitute_variables("$tsCanId\\x04", &vars), "\x00\\x04");

        // Test escaped \$ substitution (common in INI files)
        assert_eq!(substitute_variables(r"\$tsCanId\x04", &vars), "\x00\\x04");

        // Test substitution at end of string
        assert_eq!(substitute_variables("prefix$tsCanId", &vars), "prefix\x00");
        assert_eq!(
            substitute_variables(r"prefix\$tsCanId", &vars),
            "prefix\x00"
        );

        // Test unknown variable preserved
        assert_eq!(substitute_variables("$unknownVar", &vars), "$unknownVar");
        assert_eq!(
            substitute_variables(r"\$unknownVar", &vars),
            r"\$unknownVar"
        );

        // Test no variables
        assert_eq!(
            substitute_variables("no variables here", &vars),
            "no variables here"
        );

        // Test with different CAN ID value
        vars.insert("tsCanId".to_string(), 5u8);
        assert_eq!(substitute_variables("$tsCanId\\x04", &vars), "\x05\\x04");
        assert_eq!(substitute_variables(r"\$tsCanId\x04", &vars), "\x05\\x04");
    }

    #[test]
    fn test_pc_variables_parsing() {
        let content = r#"
[PcVariables]
tsCanId = bits, U08, [0:3], "CAN ID 0", "CAN ID 1", "CAN ID 2"

[Constants]
pageIdentifier = "\$tsCanId\x04", "\$tsCanId\x05"
pageReadCommand = "r%2i%2o%2c", "r%2i%2o%2c"
"#;

        let def = parse_ini(content).expect("Should parse successfully");

        // Check that tsCanId was parsed with default value 0
        assert_eq!(def.pc_variables.get("tsCanId"), Some(&0u8));

        // Check that page identifiers have $tsCanId substituted with 0x00
        // "\$tsCanId\x04" -> "\x00\x04" -> bytes [0, 4]
        assert_eq!(def.protocol.page_identifiers.len(), 2);
        assert_eq!(def.protocol.page_identifiers[0], vec![0u8, 4u8]);
        assert_eq!(def.protocol.page_identifiers[1], vec![0u8, 5u8]);
    }

    #[test]
    fn test_query_command_substitution() {
        let content = r#"
[MegaTune]
queryCommand = "r\$tsCanId\x0f\x00\x00\x00\x14"

[PcVariables]
tsCanId = bits, U08, [0:3], "CAN ID 0", "CAN ID 1"
"#;

        let def = parse_ini(content).expect("Should parse successfully");

        // queryCommand should have $tsCanId replaced with 0x00
        // "r\$tsCanId\x0f..." -> "r\x00\x0f..."
        assert!(def.query_command.starts_with("r\x00"));
    }

    #[test]
    fn test_ms2extra_ini_parsing() {
        // Test with actual MS2Extra INI file if available
        let path =
            "/home/pat/codingprojects/libretune/TunerStudioMS/config/ecuDef/MS2Extracomms342hU.ini";
        if std::path::Path::new(path).exists() {
            // Use EcuDefinition::from_file which handles encoding
            let def =
                crate::ini::EcuDefinition::from_file(path).expect("Should parse successfully");

            // Check that tsCanId was parsed
            assert!(
                def.pc_variables.contains_key("tsCanId"),
                "tsCanId should be in pc_variables"
            );
            assert_eq!(
                def.pc_variables.get("tsCanId"),
                Some(&0u8),
                "tsCanId should default to 0"
            );

            // Check that page identifiers have been substituted
            // With tsCanId=0, pageIdentifier "\$tsCanId\x04" should become bytes [0, 4]
            assert!(
                !def.protocol.page_identifiers.is_empty(),
                "Should have page identifiers"
            );

            // First page identifier should start with 0x00 (the substituted tsCanId value)
            let first_id = &def.protocol.page_identifiers[0];
            assert!(
                !first_id.is_empty(),
                "First page identifier should not be empty"
            );
            assert_eq!(
                first_id[0], 0x00,
                "First byte should be substituted tsCanId value (0)"
            );

            // Check that pageReadCommand was parsed and substituted
            assert!(
                !def.protocol.page_read_commands.is_empty(),
                "Should have page read commands"
            );
            let first_cmd = &def.protocol.page_read_commands[0];
            // Should start with 'r' followed by 0x00 (substituted tsCanId)
            assert!(
                first_cmd.starts_with("r\x00"),
                "Page read command should have tsCanId substituted"
            );
        }
    }

    #[test]
    fn test_preprocessor_conditionals() {
        // Test that #set/#if/#else/#endif are handled correctly
        let content = r#"
#set CAN_COMMANDS

[MegaTune]
signature = "TestECU"
#if CAN_COMMANDS
   queryCommand = "r\x00\x0f\x00\x00\x00\x14"
#else
   queryCommand = "Q"
#endif
"#;

        let def = parse_ini(content).expect("Should parse successfully");

        // With CAN_COMMANDS set, should use the modern CRC command, not "Q"
        assert!(
            !def.query_command.starts_with("Q"),
            "Should use CAN command, not legacy Q"
        );
        assert!(
            def.query_command.starts_with("r"),
            "Should start with 'r' for read command"
        );
    }

    #[test]
    fn test_preprocessor_else_branch() {
        // Test that #else branch is used when symbol is not set
        let content = r#"
[MegaTune]
signature = "TestECU"
#if CAN_COMMANDS
   queryCommand = "r\x00\x0f\x00\x00\x00\x14"
#else
   queryCommand = "Q"
#endif
"#;

        let def = parse_ini(content).expect("Should parse successfully");

        // Without CAN_COMMANDS set, should use the legacy "Q" command
        assert_eq!(
            def.query_command, "Q",
            "Should use legacy Q command when symbol not set"
        );
    }

    #[test]
    fn test_preprocessor_unset() {
        // Test that #unset works
        let content = r#"
#set FEATURE
#unset FEATURE

[MegaTune]
signature = "TestECU"
#if FEATURE
   queryCommand = "enabled"
#else
   queryCommand = "disabled"
#endif
"#;

        let def = parse_ini(content).expect("Should parse successfully");

        // FEATURE was set then unset, so should use else branch
        assert_eq!(
            def.query_command, "disabled",
            "Should use else branch after #unset"
        );
    }

    #[test]
    fn test_table_size_resolution() {
        // Test that post_process_table_sizes resolves table dimensions from constants
        let content = r#"
[MegaTune]
signature = "TestECU"

[Constants]
page = 1
   veRpmBins = array, U08, 0, [16], "RPM", 100.0, 0.0
   veFuelBins = array, U08, 16, [16], "%", 1.0, 0.0
   veTable = array, U08, 32, [16x16], "%", 1.0, 0.0

[TableEditor]
table = veTable1, veTableMap, "VE Table 1"
  xBins = veRpmBins
  yBins = veFuelBins
  zBins = veTable
"#;

        let def = parse_ini(content).expect("Should parse successfully");

        // Check that the table was parsed
        assert!(
            def.tables.contains_key("veTable1"),
            "veTable1 should be parsed"
        );

        let table = def.tables.get("veTable1").unwrap();

        // Check that x_size and y_size were resolved from the constant shapes
        assert_eq!(table.x_size, 16, "x_size should be 16 from veRpmBins");
        assert_eq!(table.y_size, 16, "y_size should be 16 from veFuelBins");
    }

    #[test]
    fn test_table_size_from_2d_map() {
        // Test that x_size/y_size can be inferred from the 2D map constant
        let content = r#"
[MegaTune]
signature = "TestECU"

[Constants]
page = 1
   veTable = array, U08, 0, [8x12], "%", 1.0, 0.0

[TableEditor]
table = veTable1, veTableMap, "VE Table"
  zBins = veTable
"#;

        let def = parse_ini(content).expect("Should parse successfully");

        let table = def.tables.get("veTable1").unwrap();

        // When x_bins and y_bins are not specified, infer from the 2D map
        assert_eq!(table.x_size, 12, "x_size should be 12 (cols from 8x12)");
        assert_eq!(table.y_size, 8, "y_size should be 8 (rows from 8x12)");
    }

    #[test]
    fn test_frontpage_parsing() {
        let content = r#"
[MegaTune]
signature = "TestECU"

[FrontPage]
gauge1 = tachometer
gauge2 = throttleGauge
gauge3 = mapGauge
gauge4 = cltGauge
gauge5 = afrGauge
gauge6 = advanceGauge
gauge7 = batteryGauge
gauge8 = iatGauge

indicator = { running }, "Not Running", "Running", white, black, green, black
indicator = { sync }, "No Sync", "Full Sync", white, black, green, black
indicator = { (tps > tpsflood) && (rpm < crankRPM) }, "FLOOD OFF", "FLOOD CLEAR", white, black, red, black
"#;

        let def = parse_ini(content).expect("Should parse successfully");

        // Check that FrontPage was parsed
        assert!(def.frontpage.is_some(), "FrontPage should be parsed");
        let frontpage = def.frontpage.unwrap();

        // Check gauge references
        assert_eq!(frontpage.gauges.len(), 8, "Should have 8 gauges");
        assert_eq!(frontpage.gauges[0], "tachometer");
        assert_eq!(frontpage.gauges[1], "throttleGauge");
        assert_eq!(frontpage.gauges[7], "iatGauge");

        // Check indicators
        assert_eq!(frontpage.indicators.len(), 3, "Should have 3 indicators");

        let ind1 = &frontpage.indicators[0];
        assert_eq!(ind1.expression, "running");
        assert_eq!(ind1.label_off, "Not Running");
        assert_eq!(ind1.label_on, "Running");
        assert_eq!(ind1.bg_off, "white");
        assert_eq!(ind1.fg_off, "black");
        assert_eq!(ind1.bg_on, "green");
        assert_eq!(ind1.fg_on, "black");

        let ind3 = &frontpage.indicators[2];
        assert_eq!(ind3.expression, "(tps > tpsflood) && (rpm < crankRPM)");
        assert_eq!(ind3.label_off, "FLOOD OFF");
        assert_eq!(ind3.label_on, "FLOOD CLEAR");
        assert_eq!(ind3.bg_on, "red");
    }
}
