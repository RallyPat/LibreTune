//! Tune file handling
//!
//! Supports .msq (XML) and JSON tune file formats.
//! MSQ is the standard TunerStudio-compatible format used by Speeduino, MS, rusEFI, etc.

use std::collections::HashMap;
use std::path::Path;
use std::io;
use std::fs;
use serde::{Deserialize, Serialize};
use chrono::Utc;

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
}

/// A value in a tune file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TuneValue {
    Scalar(f64),
    Array(Vec<f64>),
    String(String),
    Bool(bool),
}

impl TuneFile {
    /// Create a new empty tune file
    pub fn new(signature: impl Into<String>) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            version: "1.0".to_string(),
            signature: signature.into(),
            author: None,
            description: None,
            created: Some(now.clone()),
            modified: Some(now),
            pages: HashMap::new(),
            constants: HashMap::new(),
        }
    }
    
    /// Load a tune file from disk
    pub fn load<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)?;
        
        // Detect format by extension
        match path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase()).as_deref() {
            Some("msq") => Self::parse_msq(&content),
            Some("json") => {
                serde_json::from_str(&content)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
            }
            _ => {
                // Try JSON first, then MSQ
                serde_json::from_str(&content)
                    .or_else(|_| Self::parse_msq(&content))
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
            }
        }
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
        
        // Extract timestamp from <bibliography> writeDate or <msq> timestamp attribute
        if let Some(bib_start) = content.find("<bibliography") {
            if let Some(write_start) = content[bib_start..].find("writeDate=\"") {
                let write_content = &content[bib_start + write_start + 11..];
                if let Some(write_end) = write_content.find('"') {
                    tune.modified = Some(write_content[..write_end].to_string());
                }
            }
        } else if let Some(ts_start) = content.find("timestamp=\"") {
            let ts_content = &content[ts_start + 11..];
            if let Some(ts_end) = ts_content.find('"') {
                tune.modified = Some(ts_content[..ts_end].to_string());
            }
        }
        
        // Helper function to parse a value string
        fn parse_value(value_str: &str) -> TuneValue {
            // Remove surrounding quotes if present
            let trimmed = value_str.trim();
            let unquoted = if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
                &trimmed[1..trimmed.len()-1]
            } else {
                trimmed
            };
            
            // Check if it's an array (has spaces/newlines or brackets)
            if unquoted.starts_with('[') || unquoted.contains(' ') || unquoted.contains('\n') {
                // Array - parse space/newline separated values
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
            } else if let Ok(val) = unquoted.parse::<f64>() {
                TuneValue::Scalar(val)
            } else if unquoted == "true" || unquoted == "false" {
                TuneValue::Bool(unquoted == "true")
            } else {
                TuneValue::String(unquoted.to_string())
            }
        }
        
        // Extract constants - handle both inline and multi-line values
        // Note: attributes can be in any order, e.g., <constant cols="16" name="veTable" rows="16">
        let mut search_pos = 0;
        let mut constants_found = 0;
        while let Some(const_start) = content[search_pos..].find("<constant") {
            let abs_start = search_pos + const_start;
            let remaining = &content[abs_start..];
            
            // Find the name attribute (can be anywhere in the tag)
            if let Some(name_attr_start) = remaining.find("name=\"") {
                let name_start = name_attr_start + 6; // length of name="
                if let Some(name_end) = remaining[name_start..].find('"') {
                    let name = remaining[name_start..name_start + name_end].to_string();
                    
                    // Find the closing > of the opening tag
                    if let Some(tag_end) = remaining.find('>') {
                        let value_start = tag_end + 1;
                        if let Some(close_tag) = remaining[value_start..].find("</constant>") {
                            let value_str = remaining[value_start..value_start + close_tag].trim();
                            let value = parse_value(value_str);
                            let value_type_str = match &value {
                                TuneValue::Scalar(v) => format!("Scalar({})", v),
                                TuneValue::Array(arr) => format!("Array[{}]", arr.len()),
                                TuneValue::String(s) => format!("String(\"{}\")", if s.len() > 20 { &s[..20] } else { s }),
                                TuneValue::Bool(b) => format!("Bool({})", b),
                            };
                            tune.constants.insert(name.clone(), value);
                            constants_found += 1;
                            
                            // Debug first few constants and VE table specifically
                            if constants_found <= 5 || name == "veTable" || name == "veRpmBins" || name == "veLoadBins" {
                                eprintln!("[DEBUG] parse_msq: Found constant '{}' - type: {}, value_len: {} chars", 
                                    name, value_type_str, value_str.len());
                            }
                            
                            search_pos = abs_start + value_start + close_tag + 11;
                            continue;
                        } else {
                            eprintln!("[DEBUG] parse_msq: Found constant '{}' but no closing </constant> tag", name);
                        }
                    } else {
                        eprintln!("[DEBUG] parse_msq: Found constant '{}' but no closing > tag", name);
                    }
                }
            } else {
                // Found <constant but no name attribute
                let tag_end = remaining.find('>').unwrap_or(remaining.len().min(100));
                eprintln!("[DEBUG] parse_msq: Found <constant tag but no name attribute: {}", &remaining[..tag_end]);
            }
            search_pos = abs_start + 1;
        }
        eprintln!("[DEBUG] parse_msq: Extracted {} constants from MSQ file", constants_found);
        
        // Extract pcVariables (PC variables) - same format as constants
        // Note: attributes can be in any order
        search_pos = 0;
        let mut pcvars_found = 0;
        while let Some(pc_start) = content[search_pos..].find("<pcVariable") {
            let abs_start = search_pos + pc_start;
            let remaining = &content[abs_start..];
            
            // Find the name attribute (can be anywhere in the tag)
            if let Some(name_attr_start) = remaining.find("name=\"") {
                let name_start = name_attr_start + 6; // length of name="
                if let Some(name_end) = remaining[name_start..].find('"') {
                    let name = remaining[name_start..name_start + name_end].to_string();
                    
                    // Find the closing > of the opening tag
                    if let Some(tag_end) = remaining.find('>') {
                        let value_start = tag_end + 1;
                        if let Some(close_tag) = remaining[value_start..].find("</pcVariable>") {
                            let value_str = remaining[value_start..value_start + close_tag].trim();
                            let value = parse_value(value_str);
                            tune.constants.insert(name, value);
                            pcvars_found += 1;
                            search_pos = abs_start + value_start + close_tag + 13;
                            continue;
                        }
                    }
                }
            }
            search_pos = abs_start + 1;
        }
        eprintln!("[DEBUG] parse_msq: Extracted {} pcVariables from MSQ file", pcvars_found);
        
        // If we found no constants and no signature, this isn't a valid MSQ
        if tune.constants.is_empty() && tune.signature.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Not a valid MSQ file"));
        }
        
        Ok(tune)
    }
    
    /// Save the tune file to disk
    pub fn save<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let path = path.as_ref();
        
        match path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase()).as_deref() {
            Some("msq") => self.save_msq(path),
            Some("json") | None => {
                let content = serde_json::to_string_pretty(self)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                fs::write(path, content)
            }
            Some(ext) => {
                Err(io::Error::new(io::ErrorKind::Unsupported, format!("Unknown format: {}", ext)))
            }
        }
    }
    
    /// Save as MSQ XML format
    fn save_msq<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let mut xml = String::new();
        xml.push_str("<?xml version=\"1.0\"?>\n");
        
        // Add metadata comment
        if let Some(ref desc) = self.description {
            xml.push_str(&format!("<!-- {} -->\n", desc));
        }
        
        let timestamp = self.modified.as_deref().unwrap_or("");
        xml.push_str(&format!("<msq signature=\"{}\" timestamp=\"{}\">\n", 
            self.signature, timestamp));
        
        // Group constants by their typical page structure
        xml.push_str("  <page number=\"0\">\n");
        
        // Sort constants for consistent output
        let mut const_names: Vec<_> = self.constants.keys().collect();
        const_names.sort();
        
        for name in const_names {
            if let Some(value) = self.constants.get(name) {
                let value_str = match value {
                    TuneValue::Scalar(v) => format!("{}", v),
                    TuneValue::Array(arr) => {
                        // Format as multi-line for large arrays
                        if arr.len() > 16 {
                            let mut lines = vec![String::new()];
                            for (i, v) in arr.iter().enumerate() {
                                if i > 0 && i % 16 == 0 {
                                    lines.push(String::new());
                                }
                                if !lines.last().unwrap().is_empty() {
                                    lines.last_mut().unwrap().push(' ');
                                }
                                lines.last_mut().unwrap().push_str(&format!("{}", v));
                            }
                            format!("\n        {}\n      ", lines.join("\n        "))
                        } else {
                            arr.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" ")
                        }
                    }
                    TuneValue::String(s) => s.clone(),
                    TuneValue::Bool(b) => if *b { "true" } else { "false" }.to_string(),
                };
                xml.push_str(&format!("    <constant name=\"{}\">{}</constant>\n", name, value_str));
            }
        }
        
        // Add page data if present
        for (page_num, data) in &self.pages {
            // Encode as hex for binary page data
            let hex: String = data.iter().map(|b| format!("{:02x}", b)).collect();
            xml.push_str(&format!("    <pageData page=\"{}\">{}</pageData>\n", page_num, hex));
        }
        
        xml.push_str("  </page>\n");
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
    
    /// Get a constant value
    pub fn get_constant(&self, name: &str) -> Option<&TuneValue> {
        self.constants.get(name)
    }
}

impl Default for TuneFile {
    fn default() -> Self {
        Self::new("")
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
}
