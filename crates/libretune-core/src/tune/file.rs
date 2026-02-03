//! Tune file handling
//!
//! Supports .msq (XML) and JSON tune file formats.
//! MSQ is the standard TunerStudio-compatible format used by Speeduino, MS, rusEFI, etc.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

/// A tune file containing ECU configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuneFile {
    /// File format version
    pub version: String,

    /// ECU signature this tune is for
    pub signature: String,

    /// Author/creator name
    pub author: Option<String>,

    /// Description/notes
    pub description: Option<String>,

    /// Creation timestamp
    pub created: Option<String>,

    /// Last modified timestamp
    pub modified: Option<String>,

    /// Page data (page number -> raw bytes)
    pub pages: HashMap<u8, Vec<u8>>,

    /// Named constant overrides (for human-readable format)
    pub constants: HashMap<String, TuneValue>,

    /// Page number for each constant (constant name -> page number)
    /// This preserves the MSQ file structure where constants are organized by page
    pub constant_pages: HashMap<String, u8>,

    /// PC Variables (local-only, not stored on ECU)
    /// These are stored on page "-1" in TunerStudio format
    #[serde(default)]
    pub pc_variables: HashMap<String, TuneValue>,

    /// INI metadata for version tracking (added in v1.1)
    /// Contains hash and spec version of INI used when tune was saved
    #[serde(default)]
    pub ini_metadata: Option<IniMetadata>,

    /// Constant manifest for migration detection (added in v1.1)
    /// Lists all constants with their types/offsets at save time
    #[serde(default)]
    pub constant_manifest: Option<Vec<ConstantManifestEntry>>,
}

/// A value in a tune file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TuneValue {
    Scalar(f64),
    Array(Vec<f64>),
    String(String),
    Bool(bool),
}

/// INI metadata stored with tune files for version tracking
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IniMetadata {
    /// ECU signature from the INI file
    pub signature: String,

    /// INI filename (e.g., "speeduino.ini")
    pub name: String,

    /// SHA-256 hash of INI structural content (constants, pages, offsets)
    pub hash: String,

    /// INI spec version (from [MegaTune] section)
    pub spec_version: String,

    /// Timestamp when tune was saved with this INI
    pub saved_at: String,
}

/// A single constant's metadata for the manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstantManifestEntry {
    /// Constant name
    pub name: String,

    /// Data type string (e.g., "U08", "S16", "array[16x16]")
    pub data_type: String,

    /// ECU page number
    pub page: u8,

    /// Byte offset within page
    pub offset: u16,

    /// Scale factor
    pub scale: f64,

    /// Translation offset
    pub translate: f64,
}

impl TuneFile {
    /// Create a new empty tune file
    pub fn new(signature: impl Into<String>) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            version: "1.1".to_string(),
            signature: signature.into(),
            author: None,
            description: None,
            created: Some(now.clone()),
            modified: Some(now),
            pages: HashMap::new(),
            constants: HashMap::new(),
            constant_pages: HashMap::new(),
            pc_variables: HashMap::new(),
            ini_metadata: None,
            constant_manifest: None,
        }
    }

    /// Sanitize corrupted string values that may contain raw XML fragments
    /// This is a migration fix for tunes saved with an older buggy parser
    /// that didn't properly detect self-closing XML tags
    fn sanitize_corrupted_strings(&mut self) {
        let mut fixed_count = 0;

        // Check constants
        for (name, value) in self.constants.iter_mut() {
            if let TuneValue::String(s) = value {
                // Detect XML fragments that shouldn't be in string values
                if s.starts_with('<') && (s.contains("constant") || s.contains("pcVariable")) {
                    eprintln!(
                        "[MIGRATION] Fixing corrupted string value for '{}': was '{}'",
                        name, s
                    );
                    *value = TuneValue::String(String::new());
                    fixed_count += 1;
                }
            }
        }

        // Check pcVariables too
        for (name, value) in self.pc_variables.iter_mut() {
            if let TuneValue::String(s) = value {
                if s.starts_with('<') && (s.contains("constant") || s.contains("pcVariable")) {
                    eprintln!(
                        "[MIGRATION] Fixing corrupted pcVariable value for '{}': was '{}'",
                        name, s
                    );
                    *value = TuneValue::String(String::new());
                    fixed_count += 1;
                }
            }
        }

        if fixed_count > 0 {
            eprintln!(
                "[MIGRATION] Fixed {} corrupted string values from old parser bug",
                fixed_count
            );
        }
    }

    /// Load a tune file from disk
    ///
    /// MSQ files may use ISO-8859-1 (Latin-1) encoding, so we read as bytes
    /// and convert to UTF-8 to handle non-ASCII characters.
    pub fn load<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path = path.as_ref();

        // Read as bytes first to handle different encodings
        let bytes = fs::read(path)?;

        // Try UTF-8 first, fall back to ISO-8859-1 (Latin-1) conversion
        let content = match String::from_utf8(bytes.clone()) {
            Ok(s) => s,
            Err(_) => {
                // ISO-8859-1 is a direct byte-to-codepoint mapping
                bytes.iter().map(|&b| b as char).collect()
            }
        };

        // Detect format by extension and parse
        let mut tune = match path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .as_deref()
        {
            Some("msq") => Self::parse_msq(&content),
            Some("json") => serde_json::from_str(&content)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
            _ => {
                // Try JSON first, then MSQ
                serde_json::from_str(&content)
                    .or_else(|_| Self::parse_msq(&content))
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
            }
        }?;

        // Sanitize any corrupted string values (migration fix for older saved tunes)
        tune.sanitize_corrupted_strings();

        Ok(tune)
    }

    /// Parse MSQ XML format
    fn parse_msq(content: &str) -> io::Result<Self> {
        // MSQ format is XML with constants and pcVariables as elements
        // Example:
        // <?xml version="1.0" encoding="ISO-8859-1"?>
        // <msq xmlns="http://www.msefi.com/:msq">
        // <versionInfo signature="rusEFI master.2025.07.30.uaefi.3074276223"/>
        // <page number="0">
        //   <constant name="engineType">"MINIMAL_PINS"</constant>
        //   <pcVariable name="tsCanId">"0"</pcVariable>
        // </page>
        // </msq>

        let mut tune = TuneFile::default();

        // Extract signature from <versionInfo> tag (preferred) or <msq> attribute (fallback)
        if let Some(version_start) = content.find("<versionInfo") {
            if let Some(sig_start) = content[version_start..].find("signature=\"") {
                let sig_content = &content[version_start + sig_start + 11..];
                if let Some(sig_end) = sig_content.find('"') {
                    tune.signature = sig_content[..sig_end].to_string();
                }
            }
        } else if let Some(sig_start) = content.find("signature=\"") {
            // Fallback to <msq signature="..."> format
            let sig_content = &content[sig_start + 11..];
            if let Some(sig_end) = sig_content.find('"') {
                tune.signature = sig_content[..sig_end].to_string();
            }
        }

        // Extract bibliography metadata (author, writeDate, etc.)
        if let Some(bib_start) = content.find("<bibliography") {
            // Find author
            if let Some(auth_start) = content[bib_start..].find("author=\"") {
                let auth_content = &content[bib_start + auth_start + 8..];
                if let Some(auth_end) = auth_content.find('"') {
                    tune.author = Some(auth_content[..auth_end].to_string());
                }
            }
            // Find writeDate (last modified)
            if let Some(write_start) = content[bib_start..].find("writeDate=\"") {
                let write_content = &content[bib_start + write_start + 11..];
                if let Some(write_end) = write_content.find('"') {
                    tune.modified = Some(write_content[..write_end].to_string());
                }
            }
            // Find created date
            if let Some(created_start) = content[bib_start..].find("created=\"") {
                let created_content = &content[bib_start + created_start + 9..];
                if let Some(created_end) = created_content.find('"') {
                    tune.created = Some(created_content[..created_end].to_string());
                }
            }
        } else if let Some(ts_start) = content.find("timestamp=\"") {
            // Fallback to <msq timestamp="..."> format
            let ts_content = &content[ts_start + 11..];
            if let Some(ts_end) = ts_content.find('"') {
                tune.modified = Some(ts_content[..ts_end].to_string());
            }
        }

        // Parse iniMetadata section (LibreTune 1.1+ format for version tracking)
        if let Some(ini_start) = content.find("<iniMetadata") {
            let ini_remaining = &content[ini_start..];
            let mut metadata = IniMetadata::default();

            // Extract each attribute
            if let Some(attr_start) = ini_remaining.find("signature=\"") {
                let attr_content = &ini_remaining[attr_start + 11..];
                if let Some(attr_end) = attr_content.find('"') {
                    metadata.signature = attr_content[..attr_end].to_string();
                }
            }
            if let Some(attr_start) = ini_remaining.find("name=\"") {
                let attr_content = &ini_remaining[attr_start + 6..];
                if let Some(attr_end) = attr_content.find('"') {
                    metadata.name = attr_content[..attr_end].to_string();
                }
            }
            if let Some(attr_start) = ini_remaining.find("hash=\"") {
                let attr_content = &ini_remaining[attr_start + 6..];
                if let Some(attr_end) = attr_content.find('"') {
                    metadata.hash = attr_content[..attr_end].to_string();
                }
            }
            if let Some(attr_start) = ini_remaining.find("specVersion=\"") {
                let attr_content = &ini_remaining[attr_start + 13..];
                if let Some(attr_end) = attr_content.find('"') {
                    metadata.spec_version = attr_content[..attr_end].to_string();
                }
            }
            if let Some(attr_start) = ini_remaining.find("savedAt=\"") {
                let attr_content = &ini_remaining[attr_start + 9..];
                if let Some(attr_end) = attr_content.find('"') {
                    metadata.saved_at = attr_content[..attr_end].to_string();
                }
            }

            tune.ini_metadata = Some(metadata);
        }

        // Parse constantManifest section (LibreTune 1.1+ format for migration detection)
        if let Some(manifest_start) = content.find("<constantManifest>") {
            if let Some(manifest_end) = content[manifest_start..].find("</constantManifest>") {
                let manifest_content = &content[manifest_start..manifest_start + manifest_end];
                let mut manifest = Vec::new();

                // Parse each <entry> element
                let mut entry_pos = 0;
                while let Some(entry_start) = manifest_content[entry_pos..].find("<entry ") {
                    let abs_entry_start = entry_pos + entry_start;
                    let entry_remaining = &manifest_content[abs_entry_start..];

                    let mut entry = ConstantManifestEntry {
                        name: String::new(),
                        data_type: String::new(),
                        page: 0,
                        offset: 0,
                        scale: 1.0,
                        translate: 0.0,
                    };

                    // Extract attributes
                    if let Some(attr_start) = entry_remaining.find("name=\"") {
                        let attr_content = &entry_remaining[attr_start + 6..];
                        if let Some(attr_end) = attr_content.find('"') {
                            entry.name = attr_content[..attr_end].to_string();
                        }
                    }
                    if let Some(attr_start) = entry_remaining.find("type=\"") {
                        let attr_content = &entry_remaining[attr_start + 6..];
                        if let Some(attr_end) = attr_content.find('"') {
                            entry.data_type = attr_content[..attr_end].to_string();
                        }
                    }
                    if let Some(attr_start) = entry_remaining.find("page=\"") {
                        let attr_content = &entry_remaining[attr_start + 6..];
                        if let Some(attr_end) = attr_content.find('"') {
                            entry.page = attr_content[..attr_end].parse().unwrap_or(0);
                        }
                    }
                    if let Some(attr_start) = entry_remaining.find("offset=\"") {
                        let attr_content = &entry_remaining[attr_start + 8..];
                        if let Some(attr_end) = attr_content.find('"') {
                            entry.offset = attr_content[..attr_end].parse().unwrap_or(0);
                        }
                    }
                    if let Some(attr_start) = entry_remaining.find("scale=\"") {
                        let attr_content = &entry_remaining[attr_start + 7..];
                        if let Some(attr_end) = attr_content.find('"') {
                            entry.scale = attr_content[..attr_end].parse().unwrap_or(1.0);
                        }
                    }
                    if let Some(attr_start) = entry_remaining.find("translate=\"") {
                        let attr_content = &entry_remaining[attr_start + 11..];
                        if let Some(attr_end) = attr_content.find('"') {
                            entry.translate = attr_content[..attr_end].parse().unwrap_or(0.0);
                        }
                    }

                    if !entry.name.is_empty() {
                        manifest.push(entry);
                    }

                    entry_pos = abs_entry_start + 1;
                }

                if !manifest.is_empty() {
                    tune.constant_manifest = Some(manifest);
                }
            }
        }

        // Helper function to parse a value string
        fn parse_value(value_str: &str) -> TuneValue {
            // Remove surrounding quotes if present
            let trimmed = value_str.trim();
            let was_quoted =
                trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2;
            let unquoted = if was_quoted {
                &trimmed[1..trimmed.len() - 1]
            } else {
                trimmed
            };

            // If it was quoted, it's definitely a string (unless it's a boolean or number)
            // But we still need to handle arrays that might be quoted
            if was_quoted {
                // Quoted strings: check for boolean first, then try to parse as number
                // If it fails, it's a string
                if unquoted == "true" || unquoted == "false" {
                    return TuneValue::Bool(unquoted == "true");
                }
                if let Ok(val) = unquoted.parse::<f64>() {
                    // Quoted number - could be a string representation of a number
                    // But for bits constants, we want to preserve it as a string if it's not a pure number
                    // Check if it's a pure number (no letters, just digits and maybe decimal point)
                    if unquoted
                        .chars()
                        .all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c == '+')
                    {
                        return TuneValue::Scalar(val);
                    }
                    // Otherwise treat as string (e.g., "1" in a bits context might be a label)
                }
                // Quoted string that's not a boolean or pure number - keep as string
                return TuneValue::String(unquoted.to_string());
            }

            // Unquoted values: check if it's an array (brackets or multiple space-separated numbers)
            if unquoted.starts_with('[') && unquoted.ends_with(']') {
                // Explicit array notation
                let clean = unquoted.trim_start_matches('[').trim_end_matches(']');
                let values: Vec<f64> = clean
                    .split(|c: char| c.is_whitespace() || c == ',')
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.parse::<f64>().ok())
                    .collect();
                if values.len() > 1 {
                    TuneValue::Array(values)
                } else if values.len() == 1 {
                    TuneValue::Scalar(values[0])
                } else {
                    TuneValue::String(unquoted.to_string())
                }
            } else if unquoted.contains(' ') || unquoted.contains('\n') {
                // Has spaces/newlines - try to parse as array of numbers
                // But only if all parts are numeric
                let parts: Vec<&str> = unquoted
                    .split(|c: char| c.is_whitespace() || c == ',')
                    .filter(|s| !s.is_empty())
                    .collect();
                let values: Vec<f64> = parts.iter().filter_map(|s| s.parse::<f64>().ok()).collect();
                // Only treat as array if ALL parts were numeric and we got multiple values
                if values.len() > 1 && values.len() == parts.len() {
                    TuneValue::Array(values)
                } else if values.len() == 1 && parts.len() == 1 {
                    // Single numeric value
                    TuneValue::Scalar(values[0])
                } else {
                    // Contains non-numeric parts - treat as string
                    TuneValue::String(unquoted.to_string())
                }
            } else if let Ok(val) = unquoted.parse::<f64>() {
                TuneValue::Scalar(val)
            } else if unquoted == "true" || unquoted == "false" {
                TuneValue::Bool(unquoted == "true")
            } else {
                TuneValue::String(unquoted.to_string())
            }
        }

        // Parse pageData elements FIRST (raw binary page data as base)
        // Constants parsed later will override individual values in the pages
        // Format: <pageData page="N">hexcontent</pageData>
        let mut page_data_pos = 0;
        while page_data_pos < content.len() {
            if let Some(pd_start) = content[page_data_pos..].find("<pageData") {
                let abs_pd_start = page_data_pos + pd_start;
                let pd_remaining = &content[abs_pd_start..];

                // Extract page="N" attribute
                if let Some(page_attr_start) = pd_remaining.find("page=\"") {
                    let page_num_content = &pd_remaining[page_attr_start + 6..];
                    if let Some(page_num_end) = page_num_content.find('"') {
                        if let Ok(page_num) = page_num_content[..page_num_end].parse::<u8>() {
                            // Find the > after attributes
                            if let Some(tag_end) = pd_remaining.find('>') {
                                let after_tag = &pd_remaining[tag_end + 1..];
                                // Find closing </pageData>
                                if let Some(close_tag) = after_tag.find("</pageData>") {
                                    let hex_content = after_tag[..close_tag].trim();
                                    // Decode hex to bytes (2 chars per byte, lowercase)
                                    let mut bytes = Vec::with_capacity(hex_content.len() / 2);
                                    let mut chars =
                                        hex_content.chars().filter(|c| !c.is_whitespace());
                                    while let Some(high) = chars.next() {
                                        let low = match chars.next() {
                                            Some(c) => c,
                                            None => break,
                                        };
                                        if let (Some(h), Some(l)) =
                                            (high.to_digit(16), low.to_digit(16))
                                        {
                                            bytes.push(((h << 4) | l) as u8);
                                        }
                                    }
                                    if !bytes.is_empty() {
                                        tune.pages.insert(page_num, bytes);
                                    }
                                    page_data_pos = abs_pd_start + tag_end + 1 + close_tag + 11;
                                    continue;
                                }
                            }
                        }
                    }
                }
                page_data_pos = abs_pd_start + 1;
            } else {
                break;
            }
        }

        // Parse page structure and extract constants/pcVariables with their page numbers
        // MSQ format: <page number="0"> ... <constant name="...">...</constant> ... </page>
        let mut current_page: u8 = 0; // Default to page 0 if no page tags found
        let mut search_pos = 0;
        let mut constants_found = 0;
        let mut pcvars_found = 0;

        // First pass: find all <page number="N"> tags and track page boundaries
        // Then extract constants/pcVariables within each page
        while search_pos < content.len() {
            // Look for <page number="N"> tag
            if let Some(page_start) = content[search_pos..].find("<page") {
                let abs_page_start = search_pos + page_start;
                let page_remaining = &content[abs_page_start..];

                // Extract page number
                if let Some(num_start) = page_remaining.find("number=\"") {
                    let num_content = &page_remaining[num_start + 8..];
                    if let Some(num_end) = num_content.find('"') {
                        if let Ok(page_num) = num_content[..num_end].parse::<u8>() {
                            current_page = page_num;
                        }
                    }
                }

                // Find the closing </page> tag for this page
                if let Some(page_end) = page_remaining.find("</page>") {
                    let page_content = &page_remaining[..page_end];

                    // Extract constants from this page
                    let mut const_pos = 0;
                    while let Some(const_start) = page_content[const_pos..].find("<constant") {
                        let abs_const_start = const_pos + const_start;
                        let const_remaining = &page_content[abs_const_start..];

                        if let Some(name_attr_start) = const_remaining.find("name=\"") {
                            let name_start = name_attr_start + 6;
                            if let Some(name_end) = const_remaining[name_start..].find('"') {
                                let name =
                                    const_remaining[name_start..name_start + name_end].to_string();

                                if let Some(tag_end) = const_remaining.find('>') {
                                    // Check for self-closing tag: <constant name="..."/> or <constant name="..." />
                                    // Look at the content before '>' to see if it contains '/'
                                    let tag_content = &const_remaining[..tag_end];
                                    if tag_content.trim_end().ends_with('/') {
                                        // Self-closing tag - value is empty
                                        let value = TuneValue::String(String::new());
                                        tune.constants.insert(name.clone(), value);
                                        tune.constant_pages.insert(name, current_page);
                                        constants_found += 1;
                                        const_pos = abs_const_start + tag_end + 1;
                                        continue;
                                    }
                                    let value_start = tag_end + 1;
                                    if let Some(close_tag) =
                                        const_remaining[value_start..].find("</constant>")
                                    {
                                        let value_str = const_remaining
                                            [value_start..value_start + close_tag]
                                            .trim();
                                        let value = parse_value(value_str);
                                        tune.constants.insert(name.clone(), value);
                                        tune.constant_pages.insert(name, current_page);
                                        constants_found += 1;
                                        const_pos = abs_const_start + value_start + close_tag + 11;
                                        continue;
                                    }
                                }
                            }
                        }
                        const_pos = abs_const_start + 1;
                    }

                    // Extract pcVariables from this page
                    let mut pc_pos = 0;
                    while let Some(pc_start) = page_content[pc_pos..].find("<pcVariable") {
                        let abs_pc_start = pc_pos + pc_start;
                        let pc_remaining = &page_content[abs_pc_start..];

                        if let Some(name_attr_start) = pc_remaining.find("name=\"") {
                            let name_start = name_attr_start + 6;
                            if let Some(name_end) = pc_remaining[name_start..].find('"') {
                                let name =
                                    pc_remaining[name_start..name_start + name_end].to_string();

                                if let Some(tag_end) = pc_remaining.find('>') {
                                    // Check for self-closing tag: <pcVariable name="..."/> or <pcVariable name="..." />
                                    let tag_content = &pc_remaining[..tag_end];
                                    if tag_content.trim_end().ends_with('/') {
                                        // Self-closing tag - value is empty
                                        let value = TuneValue::String(String::new());
                                        tune.pc_variables.insert(name, value);
                                        pcvars_found += 1;
                                        pc_pos = abs_pc_start + tag_end + 1;
                                        continue;
                                    }
                                    let value_start = tag_end + 1;
                                    if let Some(close_tag) =
                                        pc_remaining[value_start..].find("</pcVariable>")
                                    {
                                        let value_str = pc_remaining
                                            [value_start..value_start + close_tag]
                                            .trim();
                                        let value = parse_value(value_str);
                                        // Store in pc_variables, not constants
                                        tune.pc_variables.insert(name, value);
                                        pcvars_found += 1;
                                        pc_pos = abs_pc_start + value_start + close_tag + 13;
                                        continue;
                                    }
                                }
                            }
                        }
                        pc_pos = abs_pc_start + 1;
                    }

                    search_pos = abs_page_start + page_end + 7; // Move past </page>
                    continue;
                }
            }

            // If no page tags found, fall back to old method (for compatibility)
            // But only if we haven't found any page-structured content
            if constants_found == 0 && pcvars_found == 0 {
                // Extract constants without page structure (legacy format)
                if let Some(const_start) = content[search_pos..].find("<constant") {
                    let abs_start = search_pos + const_start;
                    let remaining = &content[abs_start..];

                    if let Some(name_attr_start) = remaining.find("name=\"") {
                        let name_start = name_attr_start + 6;
                        if let Some(name_end) = remaining[name_start..].find('"') {
                            let name = remaining[name_start..name_start + name_end].to_string();

                            if let Some(tag_end) = remaining.find('>') {
                                // Check for self-closing tag: <constant name="..."/> or <constant name="..." />
                                let tag_content = &remaining[..tag_end];
                                if tag_content.trim_end().ends_with('/') {
                                    // Self-closing tag - value is empty
                                    let value = TuneValue::String(String::new());
                                    tune.constants.insert(name.clone(), value);
                                    tune.constant_pages.insert(name, current_page);
                                    constants_found += 1;
                                    search_pos = abs_start + tag_end + 1;
                                    continue;
                                }
                                let value_start = tag_end + 1;
                                if let Some(close_tag) =
                                    remaining[value_start..].find("</constant>")
                                {
                                    let value_str =
                                        remaining[value_start..value_start + close_tag].trim();
                                    let value = parse_value(value_str);
                                    tune.constants.insert(name.clone(), value);
                                    tune.constant_pages.insert(name, current_page);
                                    constants_found += 1;
                                    search_pos = abs_start + value_start + close_tag + 11;
                                    continue;
                                }
                            }
                        }
                    }
                    search_pos = abs_start + 1;
                } else if let Some(pc_start) = content[search_pos..].find("<pcVariable") {
                    let abs_start = search_pos + pc_start;
                    let remaining = &content[abs_start..];

                    if let Some(name_attr_start) = remaining.find("name=\"") {
                        let name_start = name_attr_start + 6;
                        if let Some(name_end) = remaining[name_start..].find('"') {
                            let name = remaining[name_start..name_start + name_end].to_string();

                            if let Some(tag_end) = remaining.find('>') {
                                // Check for self-closing tag: <pcVariable name="..."/> or <pcVariable name="..." />
                                let tag_content = &remaining[..tag_end];
                                if tag_content.trim_end().ends_with('/') {
                                    // Self-closing tag - value is empty
                                    let value = TuneValue::String(String::new());
                                    tune.pc_variables.insert(name, value);
                                    pcvars_found += 1;
                                    search_pos = abs_start + tag_end + 1;
                                    continue;
                                }
                                let value_start = tag_end + 1;
                                if let Some(close_tag) =
                                    remaining[value_start..].find("</pcVariable>")
                                {
                                    let value_str =
                                        remaining[value_start..value_start + close_tag].trim();
                                    let value = parse_value(value_str);
                                    // Store in pc_variables, not constants
                                    tune.pc_variables.insert(name, value);
                                    pcvars_found += 1;
                                    search_pos = abs_start + value_start + close_tag + 13;
                                    continue;
                                }
                            }
                        }
                    }
                    search_pos = abs_start + 1;
                } else {
                    break; // No more constants or pcVariables
                }
            } else {
                break; // Already processed page-structured content
            }
        }

        eprintln!(
            "[DEBUG] parse_msq: Extracted {} constants and {} pcVariables from MSQ file",
            constants_found, pcvars_found
        );

        // If we found no constants and no signature, this isn't a valid MSQ
        if tune.constants.is_empty() && tune.signature.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Not a valid MSQ file",
            ));
        }

        Ok(tune)
    }

    /// Save the tune file to disk
    pub fn save<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let path = path.as_ref();

        match path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .as_deref()
        {
            Some("msq") => self.save_msq(path),
            Some("json") | None => {
                let content = serde_json::to_string_pretty(self)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                fs::write(path, content)
            }
            Some(ext) => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("Unknown format: {}", ext),
            )),
        }
    }

    /// Save as MSQ XML format
    fn save_msq<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let mut xml = String::new();
        xml.push_str("<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>\n");

        // Add metadata comment
        if let Some(ref desc) = self.description {
            xml.push_str(&format!("<!-- {} -->\n", desc));
        }

        // Start msq tag with signature
        xml.push_str("<msq xmlns=\"http://www.msefi.com/:msq\">\n");

        // Add versionInfo with signature
        xml.push_str(&format!(
            "  <versionInfo signature=\"{}\"/>\n",
            self.signature
        ));

        // Add bibliography if we have metadata
        if self.author.is_some() || self.created.is_some() || self.modified.is_some() {
            xml.push_str("  <bibliography");
            if let Some(ref author) = self.author {
                xml.push_str(&format!(" author=\"{}\"", author));
            }
            if let Some(ref created) = self.created {
                xml.push_str(&format!(" created=\"{}\"", created));
            }
            if let Some(ref modified) = self.modified {
                xml.push_str(&format!(" writeDate=\"{}\"", modified));
            }
            xml.push_str("/>\n");
        }

        // Add INI metadata for version tracking (LibreTune 1.1+)
        if let Some(ref metadata) = self.ini_metadata {
            xml.push_str(&format!(
                "  <iniMetadata signature=\"{}\" name=\"{}\" hash=\"{}\" specVersion=\"{}\" savedAt=\"{}\"/>\n",
                metadata.signature,
                metadata.name,
                metadata.hash,
                metadata.spec_version,
                metadata.saved_at
            ));
        }

        // Add constant manifest for migration detection (LibreTune 1.1+)
        if let Some(ref manifest) = self.constant_manifest {
            xml.push_str("  <constantManifest>\n");
            for entry in manifest {
                xml.push_str(&format!(
                    "    <entry name=\"{}\" type=\"{}\" page=\"{}\" offset=\"{}\" scale=\"{}\" translate=\"{}\"/>\n",
                    entry.name,
                    entry.data_type,
                    entry.page,
                    entry.offset,
                    entry.scale,
                    entry.translate
                ));
            }
            xml.push_str("  </constantManifest>\n");
        }

        // Group constants by page number (preserve page structure)
        let mut constants_by_page: HashMap<u8, Vec<String>> = HashMap::new();
        for name in self.constants.keys() {
            let page = self.constant_pages.get(name).copied().unwrap_or(0);
            constants_by_page
                .entry(page)
                .or_default()
                .push(name.clone());
        }

        // Sort page numbers and constants within each page
        let mut page_numbers: Vec<u8> = constants_by_page.keys().copied().collect();
        page_numbers.sort();

        for page_num in page_numbers {
            xml.push_str(&format!("  <page number=\"{}\">\n", page_num));

            let mut const_names = constants_by_page.remove(&page_num).unwrap_or_default();
            const_names.sort();

            for name in const_names {
                if let Some(value) = self.constants.get(&name) {
                    let value_str = match value {
                        TuneValue::Scalar(v) => {
                            // Use high precision format to preserve F64 values accurately
                            // Format with enough precision to round-trip any f64 exactly
                            let formatted = format!("{:.17}", v);
                            // Trim unnecessary trailing zeros for cleaner output
                            let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
                            if trimmed.is_empty() {
                                "0".to_string()
                            } else {
                                trimmed.to_string()
                            }
                        }
                        TuneValue::Array(arr) => {
                            // Format as space-separated for arrays with high precision
                            arr.iter()
                                .map(|v| {
                                    let formatted = format!("{:.17}", v);
                                    let trimmed =
                                        formatted.trim_end_matches('0').trim_end_matches('.');
                                    if trimmed.is_empty() {
                                        "0".to_string()
                                    } else {
                                        trimmed.to_string()
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join(" ")
                        }
                        TuneValue::String(s) => format!("\"{}\"", s),
                        TuneValue::Bool(b) => if *b { "true" } else { "false" }.to_string(),
                    };
                    xml.push_str(&format!(
                        "    <constant name=\"{}\">{}</constant>\n",
                        name, value_str
                    ));
                }
            }

            xml.push_str("  </page>\n");
        }

        // Add page data if present (raw binary page data)
        for (page_num, data) in &self.pages {
            // Encode as hex for binary page data
            let hex: String = data.iter().map(|b| format!("{:02x}", b)).collect();
            xml.push_str(&format!(
                "  <pageData page=\"{}\">{}</pageData>\n",
                page_num, hex
            ));
        }

        // Add pcVariables on page -1 (TunerStudio convention)
        if !self.pc_variables.is_empty() {
            xml.push_str("  <page number=\"-1\">\n");

            let mut pc_var_names: Vec<&String> = self.pc_variables.keys().collect();
            pc_var_names.sort();

            for name in pc_var_names {
                if let Some(value) = self.pc_variables.get(name) {
                    let value_str = match value {
                        TuneValue::Scalar(v) => {
                            let formatted = format!("{:.17}", v);
                            let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
                            if trimmed.is_empty() {
                                "0".to_string()
                            } else {
                                trimmed.to_string()
                            }
                        }
                        TuneValue::Array(arr) => arr
                            .iter()
                            .map(|v| {
                                let formatted = format!("{:.17}", v);
                                let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
                                if trimmed.is_empty() {
                                    "0".to_string()
                                } else {
                                    trimmed.to_string()
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(" "),
                        TuneValue::String(s) => format!("\"{}\"", s),
                        TuneValue::Bool(b) => if *b { "true" } else { "false" }.to_string(),
                    };
                    xml.push_str(&format!(
                        "    <pcVariable name=\"{}\">{}</pcVariable>\n",
                        name, value_str
                    ));
                }
            }

            xml.push_str("  </page>\n");
        }

        xml.push_str("</msq>\n");

        fs::write(path, xml)
    }

    /// Update the modified timestamp
    pub fn touch(&mut self) {
        self.modified = Some(Utc::now().to_rfc3339());
    }

    /// Set a page's raw data
    pub fn set_page(&mut self, page: u8, data: Vec<u8>) {
        self.pages.insert(page, data);
    }

    /// Get a page's raw data
    pub fn get_page(&self, page: u8) -> Option<&[u8]> {
        self.pages.get(&page).map(|v| v.as_slice())
    }

    /// Set a constant value
    pub fn set_constant(&mut self, name: impl Into<String>, value: TuneValue) {
        self.constants.insert(name.into(), value);
    }

    /// Set a constant value with its page number (preserves MSQ page structure)
    pub fn set_constant_with_page(&mut self, name: impl Into<String>, value: TuneValue, page: u8) {
        let name = name.into();
        self.constants.insert(name.clone(), value);
        self.constant_pages.insert(name, page);
    }

    /// Get a constant value
    pub fn get_constant(&self, name: &str) -> Option<&TuneValue> {
        self.constants.get(name)
    }

    /// Set a PC variable value
    pub fn set_pc_variable(&mut self, name: impl Into<String>, value: TuneValue) {
        self.pc_variables.insert(name.into(), value);
    }

    /// Get a PC variable value
    pub fn get_pc_variable(&self, name: &str) -> Option<&TuneValue> {
        self.pc_variables.get(name)
    }

    /// Get a value (constant or PC variable)
    pub fn get_value(&self, name: &str) -> Option<&TuneValue> {
        self.constants
            .get(name)
            .or_else(|| self.pc_variables.get(name))
    }

    /// Save only PC variables to a separate file (pcVariableValues.msq format)
    pub fn save_pc_variables<P: AsRef<Path>>(&self, path: P, signature: &str) -> io::Result<()> {
        let mut xml = String::new();
        xml.push_str("<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>\n");
        xml.push_str("<msq xmlns=\"http://www.msefi.com/:msq\">\n");
        xml.push_str(&format!(
            "<bibliography author=\"LibreTune\" writeDate=\"{}\"/>\n",
            Utc::now().format("%a %b %d %H:%M:%S %Y")
        ));
        xml.push_str(&format!(
            "<versionInfo fileFormat=\"4.0\" signature=\"{}\"/>\n",
            signature
        ));

        if !self.pc_variables.is_empty() {
            xml.push_str("<page number=\"-1\">\n");

            let mut names: Vec<&String> = self.pc_variables.keys().collect();
            names.sort();

            for name in names {
                if let Some(value) = self.pc_variables.get(name) {
                    let value_str = format_tune_value(value);
                    xml.push_str(&format!(
                        "<constant name=\"{}\">{}</constant>\n",
                        name, value_str
                    ));
                }
            }

            xml.push_str("</page>\n");
        }

        xml.push_str("</msq>\n");
        fs::write(path, xml)
    }

    /// Load PC variables from a separate file (pcVariableValues.msq format)
    pub fn load_pc_variables<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let loaded = Self::load(path)?;
        // The file uses <constant> tags on page -1, but we parse them as constants
        // We need to merge them into pc_variables
        for (name, value) in loaded.constants {
            self.pc_variables.insert(name, value);
        }
        // Also merge any pc_variables that were parsed
        for (name, value) in loaded.pc_variables {
            self.pc_variables.insert(name, value);
        }
        Ok(())
    }
}

/// Format a TuneValue for XML output with high precision
fn format_tune_value(value: &TuneValue) -> String {
    match value {
        TuneValue::Scalar(v) => {
            let formatted = format!("{:.17}", v);
            let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
            if trimmed.is_empty() {
                "0".to_string()
            } else {
                trimmed.to_string()
            }
        }
        TuneValue::Array(arr) => arr
            .iter()
            .map(|v| {
                let formatted = format!("{:.17}", v);
                let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
                if trimmed.is_empty() {
                    "0".to_string()
                } else {
                    trimmed.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
        TuneValue::String(s) => format!("\"{}\"", s),
        TuneValue::Bool(b) => if *b { "true" } else { "false" }.to_string(),
    }
}

impl Default for TuneFile {
    fn default() -> Self {
        Self {
            version: "1.1".to_string(),
            signature: String::new(),
            author: None,
            description: None,
            created: None,
            modified: None,
            pages: HashMap::new(),
            constants: HashMap::new(),
            constant_pages: HashMap::new(),
            pc_variables: HashMap::new(),
            ini_metadata: None,
            constant_manifest: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tune_file_creation() {
        let mut tune = TuneFile::new("speeduino 202310");
        tune.set_constant("reqFuel", TuneValue::Scalar(10.5));

        assert_eq!(tune.signature, "speeduino 202310");
        assert!(tune.get_constant("reqFuel").is_some());
    }

    #[test]
    fn test_parse_msq_self_closing_constant() {
        // Test that self-closing tags like <constant name="vinNumber"/> are handled correctly
        // Previously this would grab content from the next constant, causing VIN to show XML
        let msq_content = r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE msq PUBLIC "-//rusEfi//DTD msq//EN" "msq.dtd">
<msq version="1.0">
  <bibliography author="LibreTune Test" tuneComment="Test tune" writeDate="2026-01-07"/>
  <versionInfo firmwareInfo="rusEFI test"/>
  <page number="0">
    <constant name="vinNumber"/>
    <constant name="engineType">25</constant>
    <constant name="tuneHidingKey"/>
    <constant name="someValue">42.5</constant>
  </page>
</msq>"#;

        // Write to temp file
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("test_self_closing.msq");
        std::fs::write(&temp_path, msq_content).expect("Failed to write temp file");

        // Parse the MSQ
        let result = TuneFile::load(&temp_path);
        assert!(result.is_ok(), "Failed to parse MSQ: {:?}", result.err());

        let tune = result.unwrap();

        // vinNumber should be empty string, NOT "25</constant>..." or other garbage
        let vin = tune.get_constant("vinNumber");
        assert!(vin.is_some(), "vinNumber constant not found");
        match vin.unwrap() {
            TuneValue::String(s) => {
                assert!(
                    s.is_empty(),
                    "vinNumber should be empty for self-closing tag, got: '{}'",
                    s
                );
            }
            other => panic!(
                "vinNumber should be String type for self-closing tag, got: {:?}",
                other
            ),
        }

        // engineType should be 25
        let engine_type = tune.get_constant("engineType");
        assert!(engine_type.is_some(), "engineType constant not found");
        match engine_type.unwrap() {
            TuneValue::Scalar(v) => assert_eq!(*v, 25.0),
            other => panic!("engineType should be Scalar, got: {:?}", other),
        }

        // someValue should be 42.5
        let some_value = tune.get_constant("someValue");
        assert!(some_value.is_some(), "someValue constant not found");
        match some_value.unwrap() {
            TuneValue::Scalar(v) => assert_eq!(*v, 42.5),
            other => panic!("someValue should be Scalar, got: {:?}", other),
        }

        // Cleanup
        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn test_parse_msq_page_data() {
        // Test that <pageData page="N">hexcontent</pageData> is parsed correctly
        // pageData provides raw binary page data which constants can override
        let msq_content = r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE msq PUBLIC "-//rusEfi//DTD msq//EN" "msq.dtd">
<msq version="1.0">
  <bibliography author="LibreTune Test" tuneComment="Test tune" writeDate="2026-01-11"/>
  <versionInfo firmwareInfo="rusEFI test"/>
  <pageData page="0">0102030405060708</pageData>
  <pageData page="1">aabbccdd</pageData>
  <page number="0">
    <constant name="testValue">100</constant>
  </page>
</msq>"#;

        // Write to temp file
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("test_page_data.msq");
        std::fs::write(&temp_path, msq_content).expect("Failed to write temp file");

        // Parse the MSQ
        let result = TuneFile::load(&temp_path);
        assert!(result.is_ok(), "Failed to parse MSQ: {:?}", result.err());

        let tune = result.unwrap();

        // Check page 0 data was parsed
        assert!(tune.pages.contains_key(&0), "Page 0 should be present");
        let page0 = tune.pages.get(&0).unwrap();
        assert_eq!(page0.len(), 8, "Page 0 should have 8 bytes");
        assert_eq!(page0, &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);

        // Check page 1 data was parsed (lowercase hex)
        assert!(tune.pages.contains_key(&1), "Page 1 should be present");
        let page1 = tune.pages.get(&1).unwrap();
        assert_eq!(page1.len(), 4, "Page 1 should have 4 bytes");
        assert_eq!(page1, &[0xaa, 0xbb, 0xcc, 0xdd]);

        // Constant should still be parsed
        let test_value = tune.get_constant("testValue");
        assert!(test_value.is_some(), "testValue constant not found");
        match test_value.unwrap() {
            TuneValue::Scalar(v) => assert_eq!(*v, 100.0),
            other => panic!("testValue should be Scalar, got: {:?}", other),
        }

        // Cleanup
        let _ = std::fs::remove_file(&temp_path);
    }
}
