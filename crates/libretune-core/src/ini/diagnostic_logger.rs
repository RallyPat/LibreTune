//! Diagnostic loggers (tooth, composite) as the INI declares them.
//!
//! `[LoggerDefinition]` describes these as blocks, not single lines:
//!
//! ```text
//! loggerDef = tooth, "Tooth Logger", tooth
//!    startCommand    = "H"
//!    stopCommand     = "h"
//!    dataReadCommand = "T\$tsCanId\x01\xFC\x00\x01\xFC"
//!    dataReadTimeout = 5000
//!    continuousRead  = true
//!    dataReadyCondition = { toothLog1Ready == 1 }
//!    recordDef   = 0, 0, 4
//!    recordField = toothTime, "ToothTime", 0, 32, 1.0, "uS"
//! ```
//!
//! None of it was read. The app sent invented command bytes instead - `H` to
//! *start* logging, then treated the (empty) reply as data, and for the
//! composite logger sent `O`, which is not a read command at all but the START
//! command of a different logger, `compositeLogger2`. It then parsed the result
//! against a fabricated "2-byte count then 2-byte tooth number" layout that no
//! part of the INI describes.
//!
//! The `continuousRead = true` flag is the other half. 127 is the size of the
//! ECU's buffer, not the length of a log: the firmware refills it and raises
//! `toothLog1Ready` again, so a caller that reads once gets about half a second
//! and concludes that is the limit.

use serde::{Deserialize, Serialize};

/// One field within a logger record, as `recordField` declares it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggerRecordField {
    pub name: String,
    pub label: String,
    /// Bit offset within the record.
    pub start_bit: u32,
    pub bit_count: u32,
    pub scale: f64,
    pub units: String,
}

/// A diagnostic logger the INI declares, with the commands to drive it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiagnosticLogger {
    /// Identifier, e.g. `tooth` or `compositeLogger`.
    pub name: String,
    /// Human label, e.g. "Tooth Logger".
    pub label: String,
    /// Declared kind, e.g. `tooth` or `composite`.
    pub kind: String,
    /// Raw command strings, still carrying `\xNN` escapes and `$var` names -
    /// decoding needs tune values, which the parser does not have.
    pub start_command: String,
    pub stop_command: String,
    pub data_read_command: String,
    pub data_read_timeout_ms: u64,
    /// Whether the ECU refills its buffer for repeated reads. When true, a
    /// single read is a sample, not the whole log.
    pub continuous_read: bool,
    /// Expression that must hold before a read will return data.
    pub data_ready_condition: Option<String>,
    /// `recordDef = header, footer, record` - all in bytes.
    pub header_len: usize,
    pub footer_len: usize,
    pub record_len: usize,
    pub fields: Vec<LoggerRecordField>,
}

impl DiagnosticLogger {
    /// Decode one record into its declared fields.
    ///
    /// Bit offsets are taken from the start of the record and read
    /// little-endian, matching how Speeduino packs `refTime` and the composite
    /// flag bits into the same 5 bytes.
    pub fn decode(&self, record: &[u8]) -> Vec<(String, f64)> {
        self.fields
            .iter()
            .filter_map(|f| {
                let raw = read_bits(record, f.start_bit, f.bit_count)?;
                Some((f.name.clone(), raw as f64 * f.scale))
            })
            .collect()
    }

    /// How many whole records a payload holds.
    pub fn record_count(&self, payload_len: usize) -> usize {
        if self.record_len == 0 {
            return 0;
        }
        payload_len
            .saturating_sub(self.header_len)
            .saturating_sub(self.footer_len)
            / self.record_len
    }
}

/// Read `bit_count` bits starting at `start_bit`, little-endian within the record.
fn read_bits(record: &[u8], start_bit: u32, bit_count: u32) -> Option<u64> {
    if bit_count == 0 || bit_count > 64 {
        return None;
    }
    let end_bit = start_bit as u64 + bit_count as u64;
    if end_bit > (record.len() as u64) * 8 {
        return None;
    }
    let mut out: u64 = 0;
    for i in 0..bit_count as u64 {
        let bit = start_bit as u64 + i;
        let byte = record[(bit / 8) as usize];
        if byte >> (bit % 8) & 1 == 1 {
            out |= 1 << i;
        }
    }
    Some(out)
}

/// Parse the `[LoggerDefinition]` section into diagnostic loggers.
///
/// Takes the section's raw lines because these are blocks: a `loggerDef` line
/// opens one and the indented keys that follow belong to it until the next
/// `loggerDef`. The line-at-a-time dispatch the rest of the parser uses cannot
/// express that.
pub fn parse_logger_definitions(lines: &[&str]) -> Vec<DiagnosticLogger> {
    let mut out: Vec<DiagnosticLogger> = Vec::new();
    for raw in lines {
        let line = strip_comment(raw);
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();

        if key.eq_ignore_ascii_case("loggerDef") {
            // loggerDef = name, "Label", kind
            let parts: Vec<&str> = value.split(',').map(str::trim).collect();
            out.push(DiagnosticLogger {
                name: parts.first().unwrap_or(&"").to_string(),
                label: parts.get(1).unwrap_or(&"").trim_matches('"').to_string(),
                kind: parts.get(2).unwrap_or(&"").to_string(),
                data_read_timeout_ms: 5000,
                ..Default::default()
            });
            continue;
        }

        let Some(cur) = out.last_mut() else { continue };
        let unquoted = value.trim_matches('"');
        match key.to_ascii_lowercase().as_str() {
            "startcommand" => cur.start_command = unquoted.to_string(),
            "stopcommand" => cur.stop_command = unquoted.to_string(),
            "datareadcommand" => cur.data_read_command = unquoted.to_string(),
            "datareadtimeout" => {
                if let Ok(v) = unquoted.parse::<u64>() {
                    cur.data_read_timeout_ms = v;
                }
            }
            "continuousread" => cur.continuous_read = unquoted.eq_ignore_ascii_case("true"),
            "datareadycondition" => {
                cur.data_ready_condition = Some(
                    unquoted
                        .trim_matches(|c| c == '{' || c == '}')
                        .trim()
                        .to_string(),
                );
            }
            "recorddef" => {
                let n: Vec<usize> = value
                    .split(',')
                    .filter_map(|p| p.trim().parse::<usize>().ok())
                    .collect();
                if n.len() >= 3 {
                    cur.header_len = n[0];
                    cur.footer_len = n[1];
                    cur.record_len = n[2];
                }
            }
            "recordfield" => {
                let p: Vec<&str> = value.split(',').map(str::trim).collect();
                if p.len() >= 5 {
                    cur.fields.push(LoggerRecordField {
                        name: p[0].to_string(),
                        label: p[1].trim_matches('"').to_string(),
                        start_bit: p[2].parse().unwrap_or(0),
                        bit_count: p[3].parse().unwrap_or(0),
                        scale: p[4].parse().unwrap_or(1.0),
                        units: p.get(5).unwrap_or(&"").trim_matches('"').to_string(),
                    });
                }
            }
            _ => {}
        }
    }
    out
}

/// Strip a trailing `;` comment, which INI lines use freely.
fn strip_comment(line: &str) -> &str {
    match line.find(';') {
        Some(i) => &line[..i],
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real block from a Speeduino INI, verbatim.
    const TOOTH: &str = r#"
    loggerDef = tooth, "Tooth Logger", tooth
       ;dataReadCommand = "r\\x00\\xf4\\x00\\x00\\x04\\x00" ; standard TS command format
       startCommand = "H"
       stopCommand = "h"
       dataReadCommand = "T\$tsCanId\x01\xFC\x00\x01\xFC" ; shared with composite
       dataReadTimeout = 5000 ; time in ms
       continuousRead = true
       dataReadyCondition = { toothLog1Ready == 1 }
       dataLength =  508
       recordDef =   0,   0,   4
       recordField = toothTime,         "ToothTime",     0,          32,       1.0,    "uS"

    loggerDef = compositeLogger, "Composite Logger", composite
        startCommand = "J"
        stopCommand = "j"
        dataReadCommand = "T\$tsCanId\x00\x00\x00\x02\x7B"
        dataReadTimeout = 5000
        dataReadyCondition = { toothLog1Ready == 1 }
        continuousRead = true
        dataLength = 127
        recordDef =   0,   0,   5
        recordField = priLevel,          "PriLevel",     0,          1,          1.0,    "Flag"
        recordField = secLevel,          "SecLevel",     1,          1,          1.0,    "Flag"
        recordField = sync,              "Sync",         4,          1,          1.0,    "Flag"
        recordField = refTime,           "RefTime",      8,          32,         0.001,  "ms"
"#;

    fn parse() -> Vec<DiagnosticLogger> {
        parse_logger_definitions(&TOOTH.lines().collect::<Vec<_>>())
    }

    #[test]
    fn both_loggers_are_found_with_their_own_commands() {
        let l = parse();
        assert_eq!(l.len(), 2, "a new loggerDef starts a new block");
        assert_eq!(l[0].name, "tooth");
        assert_eq!(l[0].start_command, "H");
        assert_eq!(l[0].stop_command, "h");
        assert_eq!(l[1].name, "compositeLogger");
        assert_eq!(l[1].start_command, "J");
        assert_eq!(l[1].stop_command, "j");
    }

    /// The bug this exists to prevent: the app sent `H` and read the reply as
    /// data, and sent `O` for the composite - which is `compositeLogger2`'s
    /// START command, not a read at all.
    #[test]
    fn the_read_command_is_not_the_start_command() {
        let l = parse();
        for lg in &l {
            assert!(
                lg.data_read_command.starts_with('T'),
                "{} reads with the T-form command, not {:?}",
                lg.name,
                lg.start_command
            );
            assert_ne!(lg.data_read_command, lg.start_command);
        }
    }

    /// 127 is a buffer size. Missing this is why a read returned half a second
    /// and looked like a hardware limit.
    #[test]
    fn continuous_read_is_picked_up() {
        for lg in parse() {
            assert!(
                lg.continuous_read,
                "{} declares continuousRead = true",
                lg.name
            );
        }
    }

    #[test]
    fn a_comment_does_not_become_a_command() {
        let l = parse();
        // The commented-out `dataReadCommand = "r\\x00..."` precedes the real
        // one; taking it would send an entirely different request.
        assert!(!l[0].data_read_command.starts_with('r'));
    }

    #[test]
    fn record_geometry_comes_from_the_ini() {
        let l = parse();
        assert_eq!(
            (l[0].header_len, l[0].footer_len, l[0].record_len),
            (0, 0, 4)
        );
        assert_eq!(l[1].record_len, 5, "composite records are 5 bytes, not 1");
        assert_eq!(l[0].record_count(508), 127);
        assert_eq!(l[1].record_count(635), 127);
    }

    #[test]
    fn a_tooth_record_decodes_as_one_32_bit_microsecond_value() {
        let l = parse();
        // 0x00012345 = 74565 us, little-endian
        let got = l[0].decode(&[0x45, 0x23, 0x01, 0x00]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, "toothTime");
        assert!((got[0].1 - 74565.0).abs() < 1e-6, "got {}", got[0].1);
    }

    /// The composite record packs flags and a 32-bit time into five bytes; the
    /// old code read one byte per entry and invented the rest.
    #[test]
    fn a_composite_record_splits_flags_from_the_timestamp() {
        let l = parse();
        // byte0 bits: pri=1, sec=0, sync=1  -> 0b0001_0001 = 0x11
        // bytes 1..5: refTime = 1000 -> 1.000 ms after the 0.001 scale
        let got: std::collections::HashMap<_, _> = l[1]
            .decode(&[0x11, 0xE8, 0x03, 0x00, 0x00])
            .into_iter()
            .collect();
        assert_eq!(got["priLevel"], 1.0);
        assert_eq!(got["secLevel"], 0.0);
        assert_eq!(got["sync"], 1.0);
        assert!(
            (got["refTime"] - 1.0).abs() < 1e-9,
            "got {}",
            got["refTime"]
        );
    }

    #[test]
    fn a_short_record_yields_nothing_rather_than_garbage() {
        let l = parse();
        assert!(
            l[0].decode(&[0x01, 0x02]).is_empty(),
            "2 bytes cannot hold 32 bits"
        );
    }
}
