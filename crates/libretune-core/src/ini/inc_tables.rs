//! .inc lookup table file parser
//!
//! Parses .inc files used for sensor linearization and ADC-to-value conversion.
//! Supports two formats:
//!
//! ## Format 1: Key-Value with TAB Separator (Interpolation)
//! ```text
//! ; Comment line
//! #Another comment style
//! 0.039    19.17
//! 0.085    17.24
//! 5.0      160
//! ```
//! Values between keys are linearly interpolated.
//!
//! ## Format 2: DB/DW Entries for ADC Lookup (No Interpolation)
//! ```text
//! ; ADC - Temp (dF)
//! DB    210T    ;   0 - sensor failure
//! DB    475T    ;   1 - 435.4
//! DW    123     ;  10 - 123
//! ```
//! One entry per ADC value (256 for 8-bit, 1024 for 10-bit). Direct index lookup, no interpolation.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Parsed .inc lookup table
#[derive(Debug, Clone)]
pub struct IncTable {
    /// Table name (filename without extension)
    pub name: String,

    /// The lookup data - depends on format
    data: IncTableData,
}

/// Internal representation of .inc data
#[derive(Debug, Clone)]
enum IncTableData {
    /// Format 1: Key-value pairs for interpolation
    /// Sorted by key for binary search
    KeyValue(Vec<(f64, f64)>),

    /// Format 2: Direct index lookup (ADC table)
    /// Index is the key, value is the result
    DirectIndex(Vec<f64>),
}

impl IncTable {
    /// Load an .inc file from disk
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read .inc file '{}': {}", path.display(), e))?;

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        Self::parse(&content, name)
    }

    /// Parse .inc file content
    pub fn parse(content: &str, name: String) -> Result<Self, String> {
        let mut key_value_pairs: Vec<(f64, f64)> = Vec::new();
        let mut direct_index_values: Vec<f64> = Vec::new();
        let mut is_db_dw_format = false;
        let mut current_index: usize = 0;

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines
            if line.is_empty() {
                continue;
            }

            // Skip comment lines
            if line.starts_with(';')
                || line.starts_with('#')
                || line.starts_with('\'')
                || line.starts_with(':')
            {
                continue;
            }

            // Remove inline comments (everything after ;)
            let line = if let Some(pos) = line.find(';') {
                line[..pos].trim()
            } else {
                line
            };

            if line.is_empty() {
                continue;
            }

            // Check for DB/DW format (Format 2)
            let upper = line.to_uppercase();
            if upper.starts_with("DB") || upper.starts_with("DW") {
                is_db_dw_format = true;

                // Parse DB/DW value
                // Format: "DB    210T" or "DW    123"
                let value_part = line[2..].trim();

                // Remove trailing 'T' if present (temperature marker in some formats)
                let value_str = value_part.trim_end_matches(['T', 't']);

                if let Ok(value) = value_str.trim().parse::<f64>() {
                    // Extend vector if needed and set value at current index
                    while direct_index_values.len() <= current_index {
                        direct_index_values.push(0.0);
                    }
                    direct_index_values[current_index] = value;
                    current_index += 1;
                }
                continue;
            }

            // Format 1: Key-value with TAB separator
            // Try TAB first, then multiple spaces as fallback
            let parts: Vec<&str> = if line.contains('\t') {
                line.split('\t').collect()
            } else {
                // Split on whitespace but only take first two non-empty parts
                line.split_whitespace().take(2).collect()
            };

            if parts.len() >= 2 {
                if let (Ok(key), Ok(value)) = (
                    parts[0].trim().parse::<f64>(),
                    parts[1].trim().parse::<f64>(),
                ) {
                    key_value_pairs.push((key, value));
                }
            }
        }

        let data = if is_db_dw_format {
            IncTableData::DirectIndex(direct_index_values)
        } else {
            // Sort key-value pairs by key for binary search during interpolation
            key_value_pairs
                .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            IncTableData::KeyValue(key_value_pairs)
        };

        Ok(Self { name, data })
    }

    /// Lookup a value in the table
    ///
    /// For Format 1 (key-value): Linearly interpolates between adjacent entries
    /// For Format 2 (DB/DW): Direct index lookup, no interpolation
    pub fn lookup(&self, key: f64) -> Option<f64> {
        match &self.data {
            IncTableData::KeyValue(pairs) => {
                if pairs.is_empty() {
                    return None;
                }

                // Find the two surrounding points for interpolation
                if key <= pairs[0].0 {
                    return Some(pairs[0].1);
                }
                if key >= pairs[pairs.len() - 1].0 {
                    return Some(pairs[pairs.len() - 1].1);
                }

                // Binary search for the insertion point
                let idx = pairs.partition_point(|(k, _)| *k < key);

                if idx == 0 {
                    return Some(pairs[0].1);
                }

                // Linear interpolation between pairs[idx-1] and pairs[idx]
                let (x0, y0) = pairs[idx - 1];
                let (x1, y1) = pairs[idx];

                if (x1 - x0).abs() < f64::EPSILON {
                    return Some(y0);
                }

                let t = (key - x0) / (x1 - x0);
                Some(y0 + t * (y1 - y0))
            }
            IncTableData::DirectIndex(values) => {
                // Round key to nearest integer index
                let idx = key.round() as usize;
                values.get(idx).copied()
            }
        }
    }

    /// Check if this is a DB/DW direct index table
    pub fn is_direct_index(&self) -> bool {
        matches!(&self.data, IncTableData::DirectIndex(_))
    }

    /// Get the number of entries in the table
    pub fn len(&self) -> usize {
        match &self.data {
            IncTableData::KeyValue(pairs) => pairs.len(),
            IncTableData::DirectIndex(values) => values.len(),
        }
    }

    /// Check if the table is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Cache for loaded .inc tables
#[derive(Debug, Default)]
pub struct IncTableCache {
    tables: HashMap<String, IncTable>,
    search_paths: Vec<std::path::PathBuf>,
}

impl IncTableCache {
    /// Create a new cache with search paths
    pub fn new(search_paths: Vec<std::path::PathBuf>) -> Self {
        Self {
            tables: HashMap::new(),
            search_paths,
        }
    }

    /// Add a search path
    pub fn add_search_path(&mut self, path: std::path::PathBuf) {
        if !self.search_paths.contains(&path) {
            self.search_paths.push(path);
        }
    }

    /// Get or load a table by filename
    pub fn get_or_load(&mut self, filename: &str) -> Option<&IncTable> {
        // Check if already loaded
        if self.tables.contains_key(filename) {
            return self.tables.get(filename);
        }

        // Search for the file
        for search_path in &self.search_paths {
            let full_path = search_path.join(filename);
            if full_path.exists() {
                if let Ok(table) = IncTable::load_from_file(&full_path) {
                    self.tables.insert(filename.to_string(), table);
                    return self.tables.get(filename);
                }
            }

            // Also try with .inc extension if not present
            if !filename.to_lowercase().ends_with(".inc") {
                let with_ext = format!("{}.inc", filename);
                let full_path = search_path.join(&with_ext);
                if full_path.exists() {
                    if let Ok(table) = IncTable::load_from_file(&full_path) {
                        self.tables.insert(filename.to_string(), table);
                        return self.tables.get(filename);
                    }
                }
            }
        }

        None
    }

    /// Clear the cache
    pub fn clear(&mut self) {
        self.tables.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_format1_key_value() {
        let content = r#"
; AFR lookup table
# Comment line
0.039	19.17
0.085	17.24
0.134	16.35
0.500	14.70
5.0	10.00
"#;
        let table = IncTable::parse(content, "test".to_string()).unwrap();
        assert!(!table.is_direct_index());
        assert_eq!(table.len(), 5);
    }

    #[test]
    fn test_parse_format2_db_dw() {
        let content = r#"
; ADC - Temp (dF)
DB    210T    ;   0 - sensor failure
DB    475T    ;   1 - 435.4
DB    409T    ;   2 - 369.4
DB    353T    ;   3 - 313.4
DB    303T    ;   4 - 263.4
"#;
        let table = IncTable::parse(content, "test".to_string()).unwrap();
        assert!(table.is_direct_index());
        assert_eq!(table.len(), 5);
    }

    #[test]
    fn test_lookup_interpolation() {
        let content = "0.0\t0.0\n1.0\t100.0\n2.0\t200.0";
        let table = IncTable::parse(content, "test".to_string()).unwrap();

        // Exact matches
        assert!((table.lookup(0.0).unwrap() - 0.0).abs() < 0.001);
        assert!((table.lookup(1.0).unwrap() - 100.0).abs() < 0.001);
        assert!((table.lookup(2.0).unwrap() - 200.0).abs() < 0.001);

        // Interpolated values
        assert!((table.lookup(0.5).unwrap() - 50.0).abs() < 0.001);
        assert!((table.lookup(1.5).unwrap() - 150.0).abs() < 0.001);

        // Out of range - clamp to endpoints
        assert!((table.lookup(-1.0).unwrap() - 0.0).abs() < 0.001);
        assert!((table.lookup(10.0).unwrap() - 200.0).abs() < 0.001);
    }

    #[test]
    fn test_lookup_direct_index() {
        let content = "DB 100\nDB 200\nDB 300\nDB 400\nDB 500";
        let table = IncTable::parse(content, "test".to_string()).unwrap();

        assert!(table.is_direct_index());
        assert_eq!(table.len(), 5);

        // Direct index lookup
        assert!((table.lookup(0.0).unwrap() - 100.0).abs() < 0.001);
        assert!((table.lookup(1.0).unwrap() - 200.0).abs() < 0.001);
        assert!((table.lookup(4.0).unwrap() - 500.0).abs() < 0.001);

        // Rounds to nearest index
        assert!((table.lookup(0.4).unwrap() - 100.0).abs() < 0.001);
        assert!((table.lookup(0.6).unwrap() - 200.0).abs() < 0.001);

        // Out of range returns None
        assert!(table.lookup(10.0).is_none());
    }

    #[test]
    fn test_inline_comments() {
        let content = "0.0\t10.0 ; this is a comment\n1.0\t20.0";
        let table = IncTable::parse(content, "test".to_string()).unwrap();
        assert_eq!(table.len(), 2);
        assert!((table.lookup(0.0).unwrap() - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_dw_format() {
        let content = "DW 1234\nDW 5678";
        let table = IncTable::parse(content, "test".to_string()).unwrap();
        assert!(table.is_direct_index());
        assert!((table.lookup(0.0).unwrap() - 1234.0).abs() < 0.001);
        assert!((table.lookup(1.0).unwrap() - 5678.0).abs() < 0.001);
    }
}
