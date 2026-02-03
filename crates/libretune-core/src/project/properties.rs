//! Java Properties File Parser
//!
//! Parses TS `.properties` files used for project configuration.
//! Format: `key=value` or `key:value` with support for:
//! - Backslash continuation lines
//! - Unicode escapes (\uXXXX)
//! - Comment lines (# or !)
//! - Escaped special characters

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

/// A parsed Java properties file
#[derive(Debug, Clone, Default)]
pub struct Properties {
    /// Key-value pairs in order of appearance
    entries: Vec<(String, String)>,
    /// Fast lookup by key
    map: HashMap<String, String>,
    /// Header comments (lines before first entry)
    header_comments: Vec<String>,
}

impl Properties {
    /// Create an empty Properties collection
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse properties from a string
    pub fn parse(content: &str) -> Self {
        let mut props = Properties::new();
        let mut current_key: Option<String> = None;
        let mut current_value = String::new();
        let mut in_header = true;

        for line in content.lines() {
            // Handle continuation from previous line
            if let Some(ref key) = current_key {
                let trimmed = line.trim_start();

                // Check if this line also continues
                let continues = trimmed.ends_with('\\') && !trimmed.ends_with("\\\\");

                // Strip trailing backslash before unescaping if continuation
                let value_part = if continues {
                    &trimmed[..trimmed.len() - 1]
                } else {
                    trimmed
                };
                current_value.push_str(&unescape_value(value_part));

                if continues {
                    continue;
                }

                // Complete the entry
                props.set(key.clone(), current_value.clone());
                current_key = None;
                current_value.clear();
                continue;
            }

            let trimmed = line.trim();

            // Skip empty lines
            if trimmed.is_empty() {
                continue;
            }

            // Handle comments
            if trimmed.starts_with('#') || trimmed.starts_with('!') {
                if in_header {
                    props.header_comments.push(trimmed.to_string());
                }
                continue;
            }

            in_header = false;

            // Find the separator (= or :)
            let (key, value) = parse_key_value(trimmed);

            // Handle continuation lines
            if value.ends_with('\\') && !value.ends_with("\\\\") {
                current_key = Some(key);
                current_value = value[..value.len() - 1].to_string();
                continue;
            }

            props.set(key, unescape_value(&value));
        }

        // Handle any remaining continuation
        if let Some(key) = current_key {
            props.set(key, current_value);
        }

        props
    }

    /// Load properties from a file
    ///
    /// Handles both UTF-8 and ISO-8859-1 (Latin-1) encoded files.
    pub fn load<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let bytes = fs::read(path)?;
        let content = match String::from_utf8(bytes.clone()) {
            Ok(s) => s,
            Err(_) => bytes.iter().map(|&b| b as char).collect(),
        };
        Ok(Self::parse(&content))
    }

    /// Save properties to a file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let content = self.to_string();
        fs::write(path, content)
    }

    /// Get a value by key
    pub fn get(&self, key: &str) -> Option<&String> {
        self.map.get(key)
    }

    /// Get a value or default
    pub fn get_or(&self, key: &str, default: &str) -> String {
        self.map
            .get(key)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }

    /// Get as i32
    pub fn get_i32(&self, key: &str) -> Option<i32> {
        self.map.get(key).and_then(|v| v.parse().ok())
    }

    /// Get as f64
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.map.get(key).and_then(|v| v.parse().ok())
    }

    /// Get as bool (true, yes, 1 are true)
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.map.get(key).map(|v| {
            let lower = v.to_lowercase();
            lower == "true" || lower == "yes" || lower == "1"
        })
    }

    /// Set a value
    pub fn set(&mut self, key: String, value: String) {
        if self.map.contains_key(&key) {
            // Update existing entry
            for (k, v) in &mut self.entries {
                if k == &key {
                    *v = value.clone();
                    break;
                }
            }
        } else {
            // Add new entry
            self.entries.push((key.clone(), value.clone()));
        }
        self.map.insert(key, value);
    }

    /// Remove a key
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.entries.retain(|(k, _)| k != key);
        self.map.remove(key)
    }

    /// Get all keys
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.iter().map(|(k, _)| k)
    }

    /// Get all entries in order
    pub fn entries(&self) -> impl Iterator<Item = (&String, &String)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }

    /// Check if a key exists
    pub fn contains(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    /// Get number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl fmt::Display for Properties {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Write header comments
        for comment in &self.header_comments {
            writeln!(f, "{}", comment)?;
        }

        // Write entries
        for (key, value) in &self.entries {
            let escaped_key = escape_key(key);
            let escaped_value = escape_value(value);
            writeln!(f, "{}={}", escaped_key, escaped_value)?;
        }

        Ok(())
    }
}

/// Parse a key=value or key:value line
fn parse_key_value(line: &str) -> (String, String) {
    // Find the first unescaped = or :
    let mut key_end = None;
    let chars = line.chars();
    let mut i = 0;
    let mut prev_backslash = false;

    for c in chars {
        if !prev_backslash && (c == '=' || c == ':') {
            key_end = Some(i);
            break;
        }
        prev_backslash = c == '\\' && !prev_backslash;
        i += c.len_utf8();
    }

    match key_end {
        Some(pos) => {
            let key = unescape_key(line[..pos].trim_end());
            let value_start = pos + 1;
            let value = if value_start < line.len() {
                line[value_start..].trim_start().to_string()
            } else {
                String::new()
            };
            (key, value)
        }
        None => {
            // No separator found, treat whole line as key with empty value
            (unescape_key(line.trim()), String::new())
        }
    }
}

/// Unescape a property key
fn unescape_key(s: &str) -> String {
    unescape_value(s)
}

/// Unescape a property value
fn unescape_value(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('t') => result.push('\t'),
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('f') => result.push('\x0C'),
                Some('\\') => result.push('\\'),
                Some('=') => result.push('='),
                Some(':') => result.push(':'),
                Some(' ') => result.push(' '),
                Some('u') => {
                    // Unicode escape \uXXXX
                    let mut hex = String::new();
                    for _ in 0..4 {
                        if let Some(h) = chars.next() {
                            hex.push(h);
                        }
                    }
                    if let Ok(code) = u32::from_str_radix(&hex, 16) {
                        if let Some(ch) = char::from_u32(code) {
                            result.push(ch);
                        }
                    }
                }
                Some(other) => {
                    // Unknown escape, keep as-is
                    result.push(other);
                }
                None => {}
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Escape a property key for writing
fn escape_key(s: &str) -> String {
    let mut result = String::with_capacity(s.len());

    for c in s.chars() {
        match c {
            ' ' => result.push_str("\\ "),
            '=' => result.push_str("\\="),
            ':' => result.push_str("\\:"),
            '\\' => result.push_str("\\\\"),
            '\t' => result.push_str("\\t"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            _ => result.push(c),
        }
    }

    result
}

/// Escape a property value for writing
fn escape_value(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut is_first = true;

    for c in s.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            '\t' => result.push_str("\\t"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            ' ' if is_first => result.push_str("\\ "),
            _ => result.push(c),
        }
        is_first = false;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_parsing() {
        let content = r#"
# Comment line
key1=value1
key2=value2
"#;
        let props = Properties::parse(content);
        assert_eq!(props.get("key1"), Some(&"value1".to_string()));
        assert_eq!(props.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_colon_separator() {
        let props = Properties::parse("key:value");
        assert_eq!(props.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_escaped_spaces() {
        let content = r"key\ with\ spaces=value with spaces";
        let props = Properties::parse(content);
        assert_eq!(
            props.get("key with spaces"),
            Some(&"value with spaces".to_string())
        );
    }

    #[test]
    fn test_continuation_lines() {
        let content = r"key=value1\
value2\
value3";
        let props = Properties::parse(content);
        assert_eq!(props.get("key"), Some(&"value1value2value3".to_string()));
    }

    #[test]
    fn test_unicode_escape() {
        let content = r"key=\u0048\u0065\u006C\u006C\u006F";
        let props = Properties::parse(content);
        assert_eq!(props.get("key"), Some(&"Hello".to_string()));
    }

    #[test]
    fn test_special_chars() {
        let content = r"key=line1\nline2\ttabbed";
        let props = Properties::parse(content);
        assert_eq!(props.get("key"), Some(&"line1\nline2\ttabbed".to_string()));
    }

    #[test]
    fn test_ts_project_properties() {
        // Real TS format
        let content = r#"#Project Attributes.
#TunerStudio by EFI Analytics, Inc
#Last Saved on: Wed Sep 14 13:44:14 EDT 2022
commPort=COM8
baudRate=115200
projectName=MS3-Example_Project
ecuConfigFile=mainController.ini
useCommonDashboardDir=false
recordsPerSec=100
"#;
        let props = Properties::parse(content);
        assert_eq!(props.get("commPort"), Some(&"COM8".to_string()));
        assert_eq!(props.get_i32("baudRate"), Some(115200));
        assert_eq!(
            props.get("projectName"),
            Some(&"MS3-Example_Project".to_string())
        );
        assert_eq!(
            props.get("ecuConfigFile"),
            Some(&"mainController.ini".to_string())
        );
        assert_eq!(props.get_bool("useCommonDashboardDir"), Some(false));
        assert_eq!(props.get_i32("recordsPerSec"), Some(100));
    }

    #[test]
    fn test_vehicle_properties() {
        let content = r#"#Vehicles Attributes.
transmissionType=Manual
weight=3200
firstGearRatio=3.42
finalDriveRatio=3.73
"#;
        let props = Properties::parse(content);
        assert_eq!(props.get("transmissionType"), Some(&"Manual".to_string()));
        assert_eq!(props.get_i32("weight"), Some(3200));
        assert_eq!(props.get_f64("firstGearRatio"), Some(3.42));
        assert_eq!(props.get_f64("finalDriveRatio"), Some(3.73));
    }

    #[test]
    fn test_roundtrip() {
        let mut props = Properties::new();
        props.set("key1".to_string(), "value1".to_string());
        props.set("key2".to_string(), "value2".to_string());

        let serialized = props.to_string();
        let parsed = Properties::parse(&serialized);

        assert_eq!(parsed.get("key1"), props.get("key1"));
        assert_eq!(parsed.get("key2"), props.get("key2"));
    }

    #[test]
    fn test_escaped_equals_in_key() {
        // TS uses escaped spaces in keys
        let content = r"limitsettingsGauge\ and\ Settings\ Limits_X=2563";
        let props = Properties::parse(content);
        assert_eq!(
            props.get("limitsettingsGauge and Settings Limits_X"),
            Some(&"2563".to_string())
        );
    }
}
