//! Output channels section parser
//!
//! Parses the [OutputChannels] section which defines real-time data from the ECU.

use super::types::DataType;
use serde::{Deserialize, Serialize};

fn default_bit_storage() -> DataType {
    DataType::U08
}

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
    /// How many bits the field spans, from [`OutputChannel::bit_position`].
    ///
    /// `[5:7]` is three bits, not one. Speeduino packs `nSquirts` there and a
    /// four-cylinder reports 2, which is `010` - so reading only the start bit
    /// returns 0, and the INI's `pulseLimit = { cycleTime / nSquirts }` then
    /// divides by zero and takes injector duty cycle with it.
    pub bit_count: Option<u8>,

    /// For bits type: the declared backing storage type (`bits, U32, ...`).
    ///
    /// `data_type` collapses to [`DataType::Bits`] for bit channels, which
    /// loses the word width the bit position is measured against. Bit 11 of
    /// a `U32` sits in a different byte under each endianness, so the width
    /// has to survive parsing or every position above 7 decodes as zero.
    #[serde(default = "default_bit_storage")]
    pub bit_storage: DataType,

    /// Unit of measurement
    pub units: String,

    /// Scale factor
    pub scale: f64,

    /// Translation offset
    pub translate: f64,

    /// Expression for computed channels
    pub expression: Option<String>,

    /// Cached parsed expression AST (for performance - avoids reparsing every update)
    #[serde(skip)]
    pub cached_expr: Option<super::expression::Expr>,
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
            bit_count: None,
            bit_storage: DataType::U08,
            units: String::new(),
            scale: 1.0,
            translate: 0.0,
            expression: None,
            cached_expr: None,
        }
    }

    /// Cache the parsed expression AST for computed channels.
    /// Call this after parsing INI to avoid reparsing on every realtime update.
    pub fn cache_expression(&mut self) {
        if let Some(ref expr_str) = self.expression {
            let mut parser = super::expression::Parser::new(expr_str);
            if let Ok(expr) = parser.parse() {
                self.cached_expr = Some(expr);
            }
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

    /// Size in bytes this channel reads out of the block (0 for computed).
    ///
    /// A bits channel reads its whole backing word, not a byte: `bits, U32,
    /// 1, [8:8]` needs bytes 1..5 present. Reporting 0 let a bounds check
    /// wave through a channel whose decode then returns `None`.
    pub fn size_bytes(&self) -> usize {
        if self.is_computed() {
            0
        } else if self.data_type == DataType::Bits {
            self.bit_storage.size_bytes()
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

        if self.data_type == DataType::Bits {
            if let Some(pos) = self.bit_position {
                // Read the *backing word*, not one byte: a `bits, U32` field
                // can name any of 32 positions, and which byte holds bit 11
                // depends on the block's endianness.
                let raw = self
                    .bit_storage
                    .read_from_bytes(data, self.offset as usize, endian)?;
                let count = self.bit_count.unwrap_or(1).clamp(1, 64);
                let mask: u64 = if count >= 64 {
                    u64::MAX
                } else {
                    (1u64 << count) - 1
                };
                // A position past the backing word reads 0 rather than
                // panicking on an overflowing shift.
                let bit_val = (raw as u64).checked_shr(u32::from(pos)).unwrap_or(0) & mask;
                return Some(self.raw_to_display(bit_val as f64));
            }
        }

        let raw = self
            .data_type
            .read_from_bytes(data, self.offset as usize, endian)?;
        Some(self.raw_to_display(raw))
    }

    /// Parse value from raw bytes, with context for computed channel expressions
    ///
    /// For non-computed channels, behaves like `parse()`.
    /// For computed channels, evaluates the expression using the provided context.
    /// Uses cached AST if available for performance.
    pub fn parse_with_context(
        &self,
        data: &[u8],
        endian: super::Endianness,
        context: &std::collections::HashMap<String, f64>,
    ) -> Option<f64> {
        self.parse_with_contexts(data, endian, context, None)
    }

    /// Like [`parse_with_context`], but supplies a [`super::expression::StringContext`]
    /// so functions such as `timeNow()` / `table()` can resolve.
    pub fn parse_with_contexts(
        &self,
        data: &[u8],
        endian: super::Endianness,
        context: &std::collections::HashMap<String, f64>,
        string_context: Option<&super::expression::StringContext>,
    ) -> Option<f64> {
        if self.is_computed() {
            // Use cached expression if available (much faster)
            if let Some(ref expr) = self.cached_expr {
                match super::expression::evaluate(expr, context, string_context) {
                    Ok(value) => return Some(value.as_f64()),
                    Err(_) => return None,
                }
            }
            // Fallback: parse expression on the fly (slower)
            if let Some(ref expr_str) = self.expression {
                let mut parser = super::expression::Parser::new(expr_str);
                match parser.parse() {
                    Ok(expr) => match super::expression::evaluate(&expr, context, string_context) {
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
            bit_count: None,
            bit_storage: DataType::U08,
            units: String::new(),
            scale: 1.0,
            translate: 0.0,
            expression: None,
            cached_expr: None,
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
                bit_count: None,
                bit_storage: DataType::U08,
                units: units.to_string(),
                scale: 1.0,
                translate: 0.0,
                expression: Some(expression),
                cached_expr: None, // Will be cached after parsing
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
        channel.bit_storage = data_type;
        channel.data_type = DataType::Bits;
        if parts.len() > 2 {
            let p2 = parts[2];
            if p2.starts_with('[') {
                // Bit range format [start:end]
                let range = p2.trim_matches(|c| c == '[' || c == ']');
                let bit_parts: Vec<&str> = range.split(':').collect();
                if let Some(bit_str) = bit_parts.first() {
                    channel.bit_position = bit_str.trim().parse().ok();
                }
                // `[start:end]` is inclusive both ends, the same arithmetic
                // parse_bit_range_spec already does for [Constants].
                channel.bit_count = match (channel.bit_position, bit_parts.get(1)) {
                    (Some(start), Some(end)) => end
                        .trim()
                        .parse::<u8>()
                        .ok()
                        .filter(|e| *e >= start)
                        .map(|e| e - start + 1),
                    // A bare `[n]` is a single bit.
                    (Some(_), None) => Some(1),
                    _ => None,
                };
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
    fn a_bits_channel_keeps_the_width_its_bit_position_is_measured_against() {
        // `data_type` collapses to `Bits`, which used to throw away the `U32`.
        // Bit 11 then decoded off byte 0 alone and always read as zero — the
        // demo INI puts `crank` exactly there.
        let ch =
            parse_output_channel_line("crank", "bits, U32, 852, [11:11]").expect("the line parses");
        assert_eq!(ch.data_type, DataType::Bits);
        assert_eq!(ch.bit_storage, DataType::U32);
        assert_eq!(ch.bit_position, Some(11));
        assert_eq!(ch.bit_count, Some(1));

        let mut block = vec![0u8; 4];
        block[1] = 0b0000_1000; // bit 11 of a little-endian U32
        let mut positioned = ch.clone();
        positioned.offset = 0;
        assert_eq!(
            positioned.parse(&block, super::super::Endianness::Little),
            Some(1.0),
            "bit 11 must be read out of the second byte"
        );
    }

    #[test]
    fn a_bits_channel_reports_the_width_it_actually_reads() {
        // Reporting 0 let a bounds check wave through a channel whose decode
        // then returns None: `bits, U32, 1, [8:8]` in a 2-byte block needs
        // bytes 1..5, not byte 1.
        let ch = parse_output_channel_line("flag", "bits, U32, 1, [8:8]").expect("parses");
        assert_eq!(ch.size_bytes(), 4);
        assert_eq!(
            ch.parse(&[0u8; 2], super::super::Endianness::Little),
            None,
            "a block that short cannot hold the backing word"
        );

        // Single-byte bit channels are unchanged.
        let byte_ch = parse_output_channel_line("run", "bits, U08, 0, [1:1]").expect("parses");
        assert_eq!(byte_ch.size_bytes(), 1);
        assert_eq!(
            byte_ch.parse(&[0b0000_0010], super::super::Endianness::Little),
            Some(1.0)
        );
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

#[cfg(test)]
mod multibit_channel_tests {
    use super::*;
    use crate::ini::Endianness;

    fn parse_line(line: &str) -> OutputChannel {
        let (name, rest) = line.split_once('=').expect("name = spec");
        parse_output_channel_line(name.trim(), rest).expect("parses")
    }

    /// Speeduino packs nSquirts into three bits. A four-cylinder reports 2,
    /// which is `010` - so reading only the start bit returns 0, and the INI's
    /// `pulseLimit = { cycleTime / nSquirts }` divides by zero. Injector duty
    /// cycle then reads a permanent 0, which was confirmed against a live ECU:
    /// byte 84 held 0b01000000 while LibreTune reported 0.0.
    #[test]
    fn a_multi_bit_field_reads_all_of_its_bits() {
        let c = parse_line("nSquirts = bits, U08, 84, [5:7]");
        assert_eq!(c.bit_position, Some(5));
        assert_eq!(c.bit_count, Some(3));

        let mut block = vec![0u8; 130];
        block[84] = 0b0100_0000; // bits [5:7] = 010 = 2
        assert_eq!(c.parse(&block, Endianness::Little), Some(2.0));

        block[84] = 0b1110_0000; // = 7, the widest the field holds
        assert_eq!(c.parse(&block, Endianness::Little), Some(7.0));
    }

    /// A six-bit error code must not collapse to one bit, or the Datalog
    /// "Error ID" column can never name a fault.
    #[test]
    fn adjacent_fields_do_not_bleed_into_each_other() {
        let err_num = parse_line("errorNum = bits, U08, 75, [0:1]");
        let current = parse_line("currentError = bits, U08, 75, [2:7]");
        assert_eq!(current.bit_count, Some(6));

        let mut block = vec![0u8; 130];
        // errorNum = 3, currentError = 42 -> 42<<2 | 3
        block[75] = (42u8 << 2) | 3;
        assert_eq!(err_num.parse(&block, Endianness::Little), Some(3.0));
        assert_eq!(current.parse(&block, Endianness::Little), Some(42.0));
    }

    /// Single-bit flags keep working - most channels are these.
    #[test]
    fn a_single_bit_flag_is_unchanged() {
        let c = parse_line("hasSync = bits, U08, 31, [7:7]");
        assert_eq!(c.bit_count, Some(1));
        let mut block = vec![0u8; 130];
        block[31] = 0b1000_0000;
        assert_eq!(c.parse(&block, Endianness::Little), Some(1.0));
        block[31] = 0;
        assert_eq!(c.parse(&block, Endianness::Little), Some(0.0));
    }
}
