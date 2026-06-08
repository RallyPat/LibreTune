//! Output channels section parser
//!
//! Parses the [OutputChannels] section which defines real-time data from the ECU.

use super::types::DataType;
use serde::{Deserialize, Serialize};

/// An output channel (real-time data) definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputChannel {
    /// Channel name/identifier
    pub name: String,

    /// Human-readable label
    pub label: Option<String>,

    /// Data type
    pub data_type: DataType,

    /// Byte offset in the output data block
    pub offset: u16,

    /// For bits type: bit position
    pub bit_position: Option<u8>,

    /// Unit of measurement
    pub units: String,

    /// Scale factor
    pub scale: f64,

    /// Translation offset
    pub translate: f64,

    /// Expression for computed channels
    pub expression: Option<String>,
}

impl OutputChannel {
    /// Create a new output channel
    pub fn new(name: impl Into<String>, data_type: DataType, offset: u16) -> Self {
        Self {
            name: name.into(),
            label: None,
            data_type,
            offset,
            bit_position: None,
            units: String::new(),
            scale: 1.0,
            translate: 0.0,
            expression: None,
        }
    }

    /// Convert a raw value to display value
    pub fn raw_to_display(&self, raw: f64) -> f64 {
        raw * self.scale + self.translate
    }

    /// Check if this is a computed channel (has expression)
    pub fn is_computed(&self) -> bool {
        self.expression.is_some()
    }

    /// Size in bytes for this channel (0 for computed)
    pub fn size_bytes(&self) -> usize {
        if self.is_computed() || self.data_type == DataType::Bits {
            0
        } else {
            self.data_type.size_bytes()
        }
    }

    /// Parse value from raw bytes
    pub fn parse(&self, data: &[u8], endian: super::Endianness) -> Option<f64> {
        if self.is_computed() {
            // Expression evaluation not yet implemented
            return None;
        }

        let raw = self
            .data_type
            .read_from_bytes(data, self.offset as usize, endian)?;

        if self.data_type == DataType::Bits {
            if let Some(pos) = self.bit_position {
                // Prevent shift overflow - if bit position >= 8, treat as invalid
                if pos < 8 {
                    let bit_val = (raw as u8 >> pos) & 1;
                    return Some(self.raw_to_display(bit_val as f64));
                } else {
                    // For larger bit positions, use u64 shift
                    let bit_val = ((raw as u64) >> (pos as u64)) & 1;
                    return Some(self.raw_to_display(bit_val as f64));
                }
            }
        }

        Some(self.raw_to_display(raw))
    }

    /// Parse value from raw bytes, with context for computed channel expressions
    ///
    /// For non-computed channels, behaves like `parse()`.
    /// For computed channels, evaluates the expression using the provided context.
    pub fn parse_with_context(
        &self,
        data: &[u8],
        endian: super::Endianness,
        context: &std::collections::HashMap<String, f64>,
    ) -> Option<f64> {
        if self.is_computed() {
            // Evaluate expression using context
            if let Some(ref expr_str) = self.expression {
                let mut parser = super::expression::Parser::new(expr_str);
                match parser.parse() {
                    Ok(expr) => match super::expression::evaluate_simple(&expr, context) {
                        Ok(value) => return Some(value.as_f64()),
                        Err(_) => return None,
                    },
                    Err(_) => return None,
                }
            }
            return None;
        }

        // For non-computed channels, use the standard parse method
        self.parse(data, endian)
    }
}

impl Default for OutputChannel {
    fn default() -> Self {
        Self {
            name: String::new(),
            label: None,
            data_type: DataType::U08,
            offset: 0,
            bit_position: None,
            units: String::new(),
            scale: 1.0,
            translate: 0.0,
            expression: None,
        }
    }
}

/// Parse an output channel definition line
///
/// Format: name = type, offset, units, scale, translate
/// Format (Modern): name = scalar, type, offset, units, scale, translate
/// Format (Bits): name = bits, type, offset, [start:end]
/// Or: name = { expression }, units
pub fn parse_output_channel_line(name: &str, value: &str) -> Option<OutputChannel> {
    let value = value.trim();

    // Check for computed channel (expression in braces)
    if value.starts_with('{') {
        // Computed channel
        if let Some(end_brace) = value.find('}') {
            let expression = value[1..end_brace].trim().to_string();
            let rest = value[end_brace + 1..].trim().trim_start_matches(',').trim();
            let units = rest
                .split(',')
                .next()
                .unwrap_or("")
                .trim()
                .trim_matches('"');

            return Some(OutputChannel {
                name: name.to_string(),
                label: None,
                data_type: DataType::F32, // Computed channels are float
                offset: 0,
                bit_position: None,
                units: units.to_string(),
                scale: 1.0,
                translate: 0.0,
                expression: Some(expression),
            });
        }
        return None;
    }

    // Regular channel
    let mut parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();

    if parts.len() < 2 {
        return None;
    }

    // Handle "scalar" or "bits" prefixes (common in Speeduino/newer specs)
    let first = parts[0].to_lowercase();
    let is_bits_prefix = first == "bits";
    let is_scalar_prefix = first == "scalar";

    if is_bits_prefix || is_scalar_prefix {
        parts.remove(0);
    }

    if parts.len() < 2 {
        return None;
    }

    let data_type_str = parts[0];
    let data_type = DataType::from_ini_str(data_type_str)?;
    let offset: u16 = parts[1].parse().ok()?;

    let mut channel = OutputChannel::new(name, data_type, offset);

    if is_bits_prefix {
        channel.data_type = DataType::Bits;
        if parts.len() > 2 {
            let p2 = parts[2];
            if p2.starts_with('[') {
                // Bit range format [start:end]
                let range = p2.trim_matches(|c| c == '[' || c == ']');
                let bit_parts: Vec<&str> = range.split(':').collect();
                if let Some(bit_str) = bit_parts.first() {
                    channel.bit_position = bit_str.parse().ok();
                }
            } else {
                channel.units = p2.trim_matches('"').to_string();
            }
        }
    } else {
        // Parse optional fields for scalar
        // Format: type, offset, units, scale, translate
        if parts.len() > 2 {
            channel.units = parts[2].trim_matches('"').to_string();
        }
        if parts.len() > 3 {
            channel.scale = parts[3].parse().unwrap_or(1.0);
        }
        if parts.len() > 4 {
            channel.translate = parts[4].parse().unwrap_or(0.0);
        }
    }

    Some(channel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_channel() {
        let ch = parse_output_channel_line("rpm", "U16, 0, \"RPM\", 1.0, 0.0");
        assert!(ch.is_some());
        let ch = ch.unwrap();
        assert_eq!(ch.name, "rpm");
        assert_eq!(ch.data_type, DataType::U16);
        assert_eq!(ch.offset, 0);
        assert_eq!(ch.units, "RPM");
    }

    #[test]
    fn test_parse_computed_channel() {
        let ch = parse_output_channel_line("afr", "{ ego1 / 10.0 }, \"AFR\"");
        assert!(ch.is_some());
        let ch = ch.unwrap();
        assert!(ch.is_computed());
        assert_eq!(ch.expression.unwrap(), "ego1 / 10.0");
        assert_eq!(ch.units, "AFR");
    }
}
