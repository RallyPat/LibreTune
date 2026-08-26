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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
    /// Bit offsets count from the record's least significant bit, the record
    /// being a big-endian integer - see [`read_bits`].
    pub fn decode(&self, record: &[u8]) -> Vec<(String, f64)> {
        self.fields
            .iter()
            .filter_map(|f| {
                let raw = read_bits(record, f.start_bit, f.bit_count)?;
                Some((f.name.clone(), raw as f64 * f.scale))
            })
            .collect()
    }

    /// Is this record entirely zeros?
    ///
    /// Speeduino hands back a zero-filled buffer when nothing has been logged -
    /// a stopped engine produces a full 127 records of nothing. Counting those
    /// as data reports a successful capture of five thousand teeth from an
    /// engine that never turned, which is worse than reporting none.
    ///
    /// Every field, not just the first: a composite record's flags are
    /// legitimately zero while its timestamp is not.
    pub fn is_empty_record(&self, record: &[u8]) -> bool {
        self.decode(record).iter().all(|(_, v)| *v == 0.0)
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
/// Read `bit_count` bits starting at `start_bit` out of one log record.
///
/// The record is a big-endian integer and bit offsets count from its least
/// significant bit - the last byte on the wire. Both loggers this INI declares
/// fall out of that one rule, which is how it was arrived at: captures from a
/// Speeduino 202501 at a known 1427 rpm, 36 teeth, so 1168 us per tooth.
///
/// Tooth records are four bytes, `toothTime` at bit 0 for 32 bits:
///   `[00 00 04 94]` -> 0x494 = 1172 us. Read the other way round it is
///   2,483,290,112 us, or forty-one minutes between two teeth.
///
/// Composite records are five, `priLevel`..`cycle` at bits 0-5 and `refTime` at
/// bit 8 for 32 bits:
///   `[00 01 eb e8 10]` -> flags 0x10 from the last byte, refTime 0x0001ebe8 =
///   125,928 us. The flag bits genuinely are at the low end and the timestamp
///   above them, which is only true when the record is read big-endian; the
///   firmware writes that timestamp with `serialWrite(uint32_t)`, which calls
///   `reverse_bytes` before writing, i.e. big-endian on the wire.
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
        // Bit 0 is the low bit of the LAST byte, so index back from the end.
        let byte = record[record.len() - 1 - (bit / 8) as usize];
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

    /// Bytes captured from a Speeduino 202501 turning a 36-1 wheel at a known
    /// 1427 rpm, which is 1168 us per tooth. The fixture is real wire data
    /// rather than a constructed value: the previous one assumed the record was
    /// little-endian and, being synthetic, agreed with itself.
    #[test]
    fn a_tooth_record_decodes_as_one_32_bit_microsecond_value() {
        let l = parse();
        for (bytes, want) in [
            ([0x00, 0x00, 0x04, 0x94], 1172.0),
            ([0x00, 0x00, 0x04, 0x90], 1168.0),
            // The gap. A 36-1 wheel reports one double-length tooth per
            // revolution, and 2336 is exactly twice 1168 - which is what makes
            // this fixture self-checking.
            ([0x00, 0x00, 0x09, 0x20], 2336.0),
        ] {
            let got = l[0].decode(&bytes);
            assert_eq!(got.len(), 1);
            assert_eq!(got[0].0, "toothTime");
            assert!(
                (got[0].1 - want).abs() < 1e-6,
                "{bytes:02x?} decoded to {} us, expected {want}",
                got[0].1
            );
        }
    }

    /// The composite record packs flags and a 32-bit time into five bytes; the
    /// old code read one byte per entry and invented the rest.
    #[test]
    fn a_composite_record_splits_flags_from_the_timestamp() {
        let l = parse();
        // Captured from the same run as the tooth fixture above: a 32-bit
        // big-endian timestamp in bytes 0..4 and the flags in the last byte,
        // which is the low end of the record.
        //   0x0001ed28 = 126248 us -> 126.248 ms at the declared 0.001 scale
        //   0x19 = 0b0001_1001 -> priLevel, trigger, sync
        let got: std::collections::HashMap<_, _> = l[1]
            .decode(&[0x00, 0x01, 0xED, 0x28, 0x19])
            .into_iter()
            .collect();
        assert_eq!(got["priLevel"], 1.0);
        assert_eq!(got["secLevel"], 0.0);
        assert_eq!(got["sync"], 1.0);
        assert!(
            (got["refTime"] - 126.248).abs() < 1e-6,
            "got {}",
            got["refTime"]
        );

        // The next edge, half a tooth later: primary has gone low and the
        // timestamp has advanced by ~584 us, half of 1168.
        let next: std::collections::HashMap<_, _> = l[1]
            .decode(&[0x00, 0x01, 0xEE, 0xF4, 0x10])
            .into_iter()
            .collect();
        assert_eq!(next["priLevel"], 0.0);
        assert_eq!(next["sync"], 1.0);
        assert!(
            (next["refTime"] - 126.708).abs() < 1e-6,
            "got {}",
            next["refTime"]
        );
    }

    /// A stopped engine returns a full buffer of zeros. Counting those as data
    /// reported a successful capture of thousands of teeth from an engine that
    /// never turned - verified on a car with the ignition off.
    #[test]
    fn a_zero_filled_record_is_not_data() {
        let l = parse();
        assert!(
            l[0].is_empty_record(&[0, 0, 0, 0]),
            "no tooth takes zero time"
        );
        assert!(!l[0].is_empty_record(&[0x45, 0x23, 0x01, 0x00]));
        // Composite flags are legitimately zero while the timestamp is not, so
        // the test has to be every field rather than the first.
        assert!(l[1].is_empty_record(&[0, 0, 0, 0, 0]));
        assert!(
            !l[1].is_empty_record(&[0x00, 0xE8, 0x03, 0x00, 0x00]),
            "all flags clear but a real timestamp is still a record"
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
