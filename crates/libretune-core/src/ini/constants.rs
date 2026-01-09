//! Constants section parser
//!
//! Parses the [Constants] section which defines editable ECU parameters.

use super::parser::split_ini_line;
use super::types::{DataType, Endianness, Shape};
use serde::{Deserialize, Serialize};

/// A constant/parameter definition from the INI file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constant {
    /// Constant name/identifier
    pub name: String,

    /// Human-readable label
    pub label: Option<String>,

    /// ECU page number (0-indexed)
    pub page: u8,

    /// Byte offset within page
    pub offset: u16,

    /// Data type
    pub data_type: DataType,

    /// Per-field endianness override (from BU08, BS16, etc. types)
    /// If None, use global ECU endianness
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endianness_override: Option<Endianness>,

    /// Shape (scalar, 1D, 2D)
    pub shape: Shape,

    /// For bits type: bit position(s)
    pub bit_position: Option<u8>,

    /// For bits type: bit size
    pub bit_size: Option<u8>,

    /// For bits type: display offset (e.g., +1 means raw 0 displays as 1)
    /// Used in notations like [4:7+1] where the +1 is added to displayed value
    pub display_offset: i8,

    /// Unit of measurement
    pub units: String,

    /// Scale factor (multiply raw by this)
    pub scale: f64,

    /// Translation offset (add this after scaling)
    pub translate: f64,

    /// Minimum allowed value (display units)
    pub min: f64,

    /// Maximum allowed value (display units)
    pub max: f64,

    /// Number of decimal digits for display
    pub digits: u8,

    /// Tooltip/help text
    pub help: Option<String>,

    /// Condition expression for visibility
    pub visibility_condition: Option<String>,

    /// For bits type: option labels (e.g., ["Off", "On"])
    pub bit_options: Vec<String>,

    /// Whether this is a PC variable (stored locally, not on ECU)
    pub is_pc_variable: bool,
}

impl Constant {
    /// Create a new scalar constant with defaults
    pub fn new(name: impl Into<String>, page: u8, offset: u16, data_type: DataType) -> Self {
        Self {
            name: name.into(),
            label: None,
            page,
            offset,
            data_type,
            endianness_override: None,
            shape: Shape::Scalar,
            bit_position: None,
            bit_size: None,
            display_offset: 0,
            units: String::new(),
            scale: 1.0,
            translate: 0.0,
            min: 0.0,
            max: 255.0,
            digits: 0,
            help: None,
            visibility_condition: None,
            bit_options: Vec::new(),
            is_pc_variable: false,
        }
    }

    /// Total size in bytes for this constant
    pub fn size_bytes(&self) -> usize {
        if self.data_type == DataType::Bits {
            // Bits don't take extra space, they're packed
            0
        } else {
            self.data_type.size_bytes() * self.shape.element_count()
        }
    }

    /// Convert a raw value to display value
    pub fn raw_to_display(&self, raw: f64) -> f64 {
        raw * self.scale + self.translate
    }

    /// Convert a display value to raw value
    pub fn display_to_raw(&self, display: f64) -> f64 {
        (display - self.translate) / self.scale
    }

    /// Check if a display value is within allowed range
    pub fn is_in_range(&self, display_value: f64) -> bool {
        display_value >= self.min && display_value <= self.max
    }
}

impl Default for Constant {
    fn default() -> Self {
        Self {
            name: String::new(),
            label: None,
            page: 0,
            offset: 0,
            data_type: DataType::U08,
            endianness_override: None,
            shape: Shape::Scalar,
            bit_position: None,
            bit_size: None,
            display_offset: 0,
            units: String::new(),
            scale: 1.0,
            translate: 0.0,
            min: 0.0,
            max: 255.0,
            digits: 0,
            help: None,
            visibility_condition: None,
            bit_options: Vec::new(),
            is_pc_variable: false,
        }
    }
}

/// Parse a constant definition line from INI
///
/// Format: name = class, type, offset, shape, units, scale, translate, min, max, digits
/// Note: Uses split_ini_line to properly handle expressions with commas inside braces,
/// such as: { bitStringValue(algorithmUnits , algorithm) }
/// The last_offset parameter supports the "lastOffset" keyword which means "use running offset counter"
/// Supports per-field big-endian types (BU08, BS16, etc.) that override global endianness.
pub fn parse_constant_line(
    name: &str,
    value: &str,
    page: u8,
    last_offset: u16,
) -> Option<Constant> {
    let parts_vec = split_ini_line(value);
    let parts: Vec<&str> = parts_vec.iter().map(|s| s.as_str()).collect();

    if parts.len() < 3 {
        return None;
    }

    // parts[0] = class (scalar, array, bits)
    // parts[1] = type (U08, S16, BU16, etc. - B* types force big-endian)
    // parts[2] = offset (can be numeric or "lastOffset" keyword)

    let class = parts[0].to_lowercase();
    let (data_type, endianness_override) = DataType::from_ini_str_with_endianness(parts[1])?;

    // Handle "lastOffset" keyword - use the running offset counter
    let offset: u16 = if parts[2].trim().to_lowercase() == "lastoffset" {
        last_offset
    } else {
        parts[2].parse().ok()?
    };

    let mut constant = Constant::new(name, page, offset, data_type);
    constant.endianness_override = endianness_override;

    // Parse shape based on class and remaining parts
    if class == "bits" {
        constant.data_type = DataType::Bits;
        // Format: bits, U08, offset, [bit_position:bit_size+display_offset], "Option1", "Option2", ...
        // Examples: [0:3], [4:7+1] (the +1 means display raw 0 as 1)
        if parts.len() > 3 {
            let bit_spec = parts[3].trim_matches(|c| c == '[' || c == ']');
            let bit_parts: Vec<&str> = bit_spec.split(':').collect();
            if !bit_parts.is_empty() {
                constant.bit_position = bit_parts[0].parse().ok();
            }
            if bit_parts.len() > 1 {
                // Check for +N or -N display offset suffix (e.g., "7+1" or "3-1")
                let size_part = bit_parts[1];
                if let Some(plus_pos) = size_part.find('+') {
                    // Parse "7+1" -> bit_size=7, display_offset=+1
                    constant.bit_size = size_part[..plus_pos].parse().ok();
                    constant.display_offset = size_part[plus_pos + 1..].parse().unwrap_or(0);
                } else if let Some(minus_pos) = size_part.rfind('-') {
                    // Parse "7-1" -> bit_size=7, display_offset=-1
                    // Use rfind to avoid negative bit indices like [-1:3]
                    if minus_pos > 0 {
                        constant.bit_size = size_part[..minus_pos].parse().ok();
                        constant.display_offset = -(size_part[minus_pos + 1..].parse::<i8>().unwrap_or(0));
                    } else {
                        constant.bit_size = size_part.parse().ok();
                    }
                } else {
                    constant.bit_size = size_part.parse().ok();
                }
            }
        }
        // Collect bit options (everything after the bit spec)
        // These are the labels for each possible value (e.g., "Off", "On")
        for part in parts.iter().skip(4) {
            let opt = part.trim().trim_matches('"').to_string();
            if !opt.is_empty() && !opt.starts_with('{') {
                // Skip empty options and visibility conditions
                constant.bit_options.push(opt);
            }
        }
        return Some(constant);
    } else if class == "array" && parts.len() > 3 {
        constant.shape = Shape::from_ini_str(parts[3]);
    }

    // Parse units (index 4 for bits/array, 3 for scalar)
    let units_idx = if class == "bits" || class == "array" {
        4
    } else {
        3
    };
    if parts.len() > units_idx {
        constant.units = parts[units_idx].trim_matches('"').to_string();
    }

    // Parse scale, translate, min, max, digits
    let scale_idx = units_idx + 1;
    if parts.len() > scale_idx {
        constant.scale = parts[scale_idx].parse().unwrap_or(1.0);
    }
    if parts.len() > scale_idx + 1 {
        constant.translate = parts[scale_idx + 1].parse().unwrap_or(0.0);
    }
    if parts.len() > scale_idx + 2 {
        constant.min = parts[scale_idx + 2].parse().unwrap_or(0.0);
    }
    if parts.len() > scale_idx + 3 {
        constant.max = parts[scale_idx + 3].parse().unwrap_or(255.0);
    }
    if parts.len() > scale_idx + 4 {
        constant.digits = parts[scale_idx + 4].parse().unwrap_or(0);
    }

    Some(constant)
}

/// Parse a PcVariable constant line (no offset field)
/// Format: name = class, type, units, scale, translate, min, max, digits
/// or: name = bits, U08, [bit_spec], "Option1", "Option2", ...
/// PcVariables are stored locally (not on ECU), so they use page 255 and offset 0
pub fn parse_pc_variable_line(name: &str, value: &str) -> Option<Constant> {
    let parts_vec = split_ini_line(value);
    let parts: Vec<&str> = parts_vec.iter().map(|s| s.as_str()).collect();

    if parts.len() < 2 {
        return None;
    }

    // parts[0] = class (scalar, array, bits)
    // parts[1] = type (U08, S16, etc)
    // NO offset for PcVariables

    let class = parts[0].to_lowercase();
    let data_type = DataType::from_ini_str(parts[1])?;

    // Use page 255 to indicate PC variable (not stored on ECU)
    let mut constant = Constant::new(name, 255, 0, data_type);
    constant.is_pc_variable = true;

    // Parse based on class
    if class == "bits" {
        constant.data_type = DataType::Bits;
        // Format: bits, U08, [bit_position:bit_size+display_offset], "Option1", "Option2", ...
        // Examples: [0:3], [4:7+1] (the +1 means display raw 0 as 1)
        if parts.len() > 2 {
            let bit_spec = parts[2].trim_matches(|c| c == '[' || c == ']');
            let bit_parts: Vec<&str> = bit_spec.split(':').collect();
            if !bit_parts.is_empty() {
                constant.bit_position = bit_parts[0].parse().ok();
            }
            if bit_parts.len() > 1 {
                // Check for +N or -N display offset suffix (e.g., "7+1" or "3-1")
                let size_part = bit_parts[1];
                if let Some(plus_pos) = size_part.find('+') {
                    constant.bit_size = size_part[..plus_pos].parse().ok();
                    constant.display_offset = size_part[plus_pos + 1..].parse().unwrap_or(0);
                } else if let Some(minus_pos) = size_part.rfind('-') {
                    if minus_pos > 0 {
                        constant.bit_size = size_part[..minus_pos].parse().ok();
                        constant.display_offset = -(size_part[minus_pos + 1..].parse::<i8>().unwrap_or(0));
                    } else {
                        constant.bit_size = size_part.parse().ok();
                    }
                } else {
                    constant.bit_size = size_part.parse().ok();
                }
            }
        }
        // Collect bit options
        for part in parts.iter().skip(3) {
            let opt = part.trim().trim_matches('"').to_string();
            if !opt.is_empty() && !opt.starts_with('{') {
                constant.bit_options.push(opt);
            }
        }
        return Some(constant);
    } else if class == "array" && parts.len() > 2 {
        // Format: array, type, [shape], units, scale, ...
        constant.shape = Shape::from_ini_str(parts[2]);
        // Parse units starting at index 3
        if parts.len() > 3 {
            constant.units = parts[3].trim_matches('"').to_string();
        }
        if parts.len() > 4 {
            constant.scale = parts[4].parse().unwrap_or(1.0);
        }
        if parts.len() > 5 {
            constant.translate = parts[5].parse().unwrap_or(0.0);
        }
        if parts.len() > 6 {
            constant.min = parts[6].parse().unwrap_or(0.0);
        }
        if parts.len() > 7 {
            constant.max = parts[7].parse().unwrap_or(255.0);
        }
        if parts.len() > 8 {
            constant.digits = parts[8].parse().unwrap_or(0);
        }
        return Some(constant);
    }

    // Scalar format: scalar, type, units, scale, translate, min, max, digits
    if parts.len() > 2 {
        constant.units = parts[2].trim_matches('"').to_string();
    }
    if parts.len() > 3 {
        constant.scale = parts[3].parse().unwrap_or(1.0);
    }
    if parts.len() > 4 {
        constant.translate = parts[4].parse().unwrap_or(0.0);
    }
    if parts.len() > 5 {
        constant.min = parts[5].parse().unwrap_or(0.0);
    }
    if parts.len() > 6 {
        constant.max = parts[6].parse().unwrap_or(255.0);
    }
    if parts.len() > 7 {
        constant.digits = parts[7].parse().unwrap_or(0);
    }

    Some(constant)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_new() {
        let c = Constant::new("test", 0, 100, DataType::U16);
        assert_eq!(c.name, "test");
        assert_eq!(c.offset, 100);
        assert_eq!(c.data_type, DataType::U16);
    }

    #[test]
    fn test_raw_display_conversion() {
        let mut c = Constant::new("afr", 0, 0, DataType::U08);
        c.scale = 0.1;
        c.translate = 0.0;

        assert!((c.raw_to_display(147.0) - 14.7).abs() < 0.01);
        assert!((c.display_to_raw(14.7) - 147.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_constant_line_scalar() {
        let c = parse_constant_line(
            "reqFuel",
            "scalar, U16, 0, \"ms\", 0.1, 0.0, 0, 25.5, 1",
            0,
            0,
        );
        assert!(c.is_some());
        let c = c.unwrap();
        assert_eq!(c.name, "reqFuel");
        assert_eq!(c.data_type, DataType::U16);
        assert_eq!(c.offset, 0);
        assert!((c.scale - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_parse_constant_line_lastoffset() {
        // Test the lastOffset keyword - should use the provided last_offset value
        let c = parse_constant_line(
            "afrTable",
            "array, U08, lastOffset, [16x16], \"AFR\", 0.1, 0.0, 7, 25.5, 1",
            0,
            1234,
        );
        assert!(c.is_some());
        let c = c.unwrap();
        assert_eq!(c.name, "afrTable");
        assert_eq!(c.data_type, DataType::U08);
        assert_eq!(c.offset, 1234); // Should use the last_offset value
        assert_eq!(c.shape, Shape::Array2D { rows: 16, cols: 16 });
    }

    #[test]
    fn test_parse_pc_variable_line_scalar() {
        // Test PC variable scalar parsing (no offset)
        let c = parse_pc_variable_line("rpmwarn", "scalar, U16, \"rpm\", 1, 0, 0, 30000, 0");
        assert!(c.is_some());
        let c = c.unwrap();
        assert_eq!(c.name, "rpmwarn");
        assert_eq!(c.data_type, DataType::U16);
        assert_eq!(c.page, 255); // PC variable marker
        assert_eq!(c.offset, 0);
        assert!(c.is_pc_variable);
        assert_eq!(c.units, "rpm");
        assert!((c.max - 30000.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_pc_variable_line_bits() {
        // Test PC variable bits parsing
        let c = parse_pc_variable_line(
            "tsCanId",
            "bits, U08, [0:3], \"CAN ID 0\", \"CAN ID 1\", \"CAN ID 2\"",
        );
        assert!(c.is_some());
        let c = c.unwrap();
        assert_eq!(c.name, "tsCanId");
        assert_eq!(c.data_type, DataType::Bits);
        assert!(c.is_pc_variable);
        assert_eq!(c.bit_position, Some(0));
        assert_eq!(c.bit_size, Some(3));
        assert_eq!(c.bit_options.len(), 3);
        assert_eq!(c.bit_options[0], "CAN ID 0");
        assert_eq!(c.display_offset, 0); // No offset
    }

    #[test]
    fn test_parse_bits_with_display_offset_positive() {
        // Test [4:7+1] notation - display offset of +1
        let c = parse_constant_line(
            "nCylinders",
            "bits, U08, 182, [4:7+1]",
            0,
            0,
        );
        assert!(c.is_some());
        let c = c.unwrap();
        assert_eq!(c.name, "nCylinders");
        assert_eq!(c.data_type, DataType::Bits);
        assert_eq!(c.bit_position, Some(4));
        assert_eq!(c.bit_size, Some(7));
        assert_eq!(c.display_offset, 1); // +1 display offset
    }

    #[test]
    fn test_parse_bits_with_display_offset_negative() {
        // Test [0:3-1] notation - display offset of -1
        let c = parse_constant_line(
            "someField",
            "bits, U08, 100, [0:3-1], \"Val 0\", \"Val 1\"",
            0,
            0,
        );
        assert!(c.is_some());
        let c = c.unwrap();
        assert_eq!(c.name, "someField");
        assert_eq!(c.bit_position, Some(0));
        assert_eq!(c.bit_size, Some(3));
        assert_eq!(c.display_offset, -1); // -1 display offset
        assert_eq!(c.bit_options.len(), 2);
    }

    #[test]
    fn test_parse_bits_without_display_offset() {
        // Test [0:7] notation - no display offset
        let c = parse_constant_line(
            "normalBits",
            "bits, U08, 50, [0:7], \"Off\", \"On\"",
            0,
            0,
        );
        assert!(c.is_some());
        let c = c.unwrap();
        assert_eq!(c.bit_position, Some(0));
        assert_eq!(c.bit_size, Some(7));
        assert_eq!(c.display_offset, 0); // No offset
    }

    #[test]
    fn test_parse_bits_with_invalid_options() {
        // Test that INVALID options are still collected (filtering happens in frontend)
        let c = parse_constant_line(
            "strategy",
            "bits, U08, 10, [2:3], \"Basic\", \"INVALID\", \"INVALID\", \"Advanced\"",
            0,
            0,
        );
        assert!(c.is_some());
        let c = c.unwrap();
        assert_eq!(c.bit_options.len(), 4);
        assert_eq!(c.bit_options[0], "Basic");
        assert_eq!(c.bit_options[1], "INVALID");
        assert_eq!(c.bit_options[2], "INVALID");
        assert_eq!(c.bit_options[3], "Advanced");
    }
}
