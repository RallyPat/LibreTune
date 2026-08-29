//! MegaLogViewer binary logs (`.mlg`, format version 2).
//!
//! The format TunerStudio writes and MegaLogViewer reads. Only reading is
//! implemented: LibreTune records to CSV, and the reason to understand `.mlg`
//! is to open logs captured by TunerStudio.
//!
//! Layout, all values big-endian:
//!
//! ```text
//! header      24 bytes  "MLVLG\0", version u16, capture time u32,
//!                       info offset u32, data offset u32,
//!                       record length u16, field count u16
//! field       89 bytes  type u8, name [34], units [10], display style u8,
//!                       scale f32, transform f32, digits u8, category [34]
//! info                  the tune the log was captured with, as MSQ text
//! record                kind u8, counter u8, clock u16 (10 µs ticks), then
//!                       either a payload plus its checksum u8 (kind 0), or
//!                       50 bytes of marker text (kind 1)
//! ```

use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Duration;

use super::LogEntry;

const MAGIC: &[u8; 6] = b"MLVLG\0";
const VERSION: u16 = 2;
const HEADER_LEN: usize = 24;
const DESCRIPTOR_LEN: usize = 89;
/// kind + counter + 16-bit clock, ahead of every record payload.
const RECORD_PREFIX_LEN: usize = 4;
const KIND_DATA: u8 = 0;
const KIND_MARKER: u8 = 1;
/// Marker text is a fixed-width field, so a marker record is always 54 bytes.
const MARKER_TEXT_LEN: usize = 50;
/// The record clock counts 10 µs ticks and wraps every 65536 of them.
const CLOCK_TICK_MICROS: u64 = 10;
const CLOCK_WRAP: u64 = 1 << 16;

/// A scalar as stored in a record payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldType {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    I64,
    F32,
}

impl FieldType {
    fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            0 => Self::U8,
            1 => Self::I8,
            2 => Self::U16,
            3 => Self::I16,
            4 => Self::U32,
            5 => Self::I32,
            6 => Self::I64,
            7 => Self::F32,
            _ => return None,
        })
    }

    fn width(self) -> usize {
        match self {
            Self::U8 | Self::I8 => 1,
            Self::U16 | Self::I16 => 2,
            Self::U32 | Self::I32 | Self::F32 => 4,
            Self::I64 => 8,
        }
    }

    /// Decode exactly `self.width()` bytes into the stored number.
    fn read(self, bytes: &[u8]) -> f64 {
        fn array<const N: usize>(bytes: &[u8]) -> [u8; N] {
            bytes[..N].try_into().expect("caller slices to width")
        }
        match self {
            Self::U8 => bytes[0] as f64,
            Self::I8 => bytes[0] as i8 as f64,
            Self::U16 => u16::from_be_bytes(array(bytes)) as f64,
            Self::I16 => i16::from_be_bytes(array(bytes)) as f64,
            Self::U32 => u32::from_be_bytes(array(bytes)) as f64,
            Self::I32 => i32::from_be_bytes(array(bytes)) as f64,
            Self::I64 => i64::from_be_bytes(array(bytes)) as f64,
            Self::F32 => f32::from_be_bytes(array(bytes)) as f64,
        }
    }
}

struct MlgField {
    name: String,
    field_type: FieldType,
    offset: usize,
    scale: f32,
    transform: f32,
}

impl MlgField {
    fn value(&self, payload: &[u8]) -> f64 {
        let raw = self
            .field_type
            .read(&payload[self.offset..self.offset + self.field_type.width()]);
        (raw + self.transform as f64) * self.scale as f64
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// A fixed-width, NUL-padded string. TunerStudio writes UTF-8 into these.
fn read_name(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

struct Header {
    data_start: u64,
    record_len: usize,
    field_count: usize,
}

fn read_header(reader: &mut impl Read) -> io::Result<Header> {
    let mut bytes = [0u8; HEADER_LEN];
    reader
        .read_exact(&mut bytes)
        .map_err(|_| invalid("file is too short to be an MLG log"))?;
    if &bytes[..6] != MAGIC {
        return Err(invalid("not an MLG log (bad file signature)"));
    }
    let version = u16::from_be_bytes([bytes[6], bytes[7]]);
    if version != VERSION {
        return Err(invalid(format!(
            "MLG format version {version} is not supported, expected {VERSION}"
        )));
    }
    Ok(Header {
        data_start: u32::from_be_bytes(bytes[16..20].try_into().expect("fixed slice")) as u64,
        record_len: u16::from_be_bytes([bytes[20], bytes[21]]) as usize,
        field_count: u16::from_be_bytes([bytes[22], bytes[23]]) as usize,
    })
}

fn read_fields(reader: &mut impl Read, header: &Header) -> io::Result<Vec<MlgField>> {
    let mut bytes = vec![0u8; header.field_count * DESCRIPTOR_LEN];
    reader
        .read_exact(&mut bytes)
        .map_err(|_| invalid("log ends inside its field descriptors"))?;

    let mut fields = Vec::with_capacity(header.field_count);
    let mut offset = 0usize;
    for (index, descriptor) in bytes.chunks_exact(DESCRIPTOR_LEN).enumerate() {
        let field_type = FieldType::from_code(descriptor[0]).ok_or_else(|| {
            invalid(format!(
                "field {index} has unknown storage type {}",
                descriptor[0]
            ))
        })?;
        let scale = f32::from_be_bytes(descriptor[46..50].try_into().expect("fixed slice"));
        let transform = f32::from_be_bytes(descriptor[50..54].try_into().expect("fixed slice"));
        if !scale.is_finite() || !transform.is_finite() {
            return Err(invalid(format!("field {index} has a non-finite scale")));
        }
        fields.push(MlgField {
            name: read_name(&descriptor[1..35]),
            field_type,
            offset,
            scale,
            transform,
        });
        offset += field_type.width();
    }
    if offset != header.record_len {
        return Err(invalid(format!(
            "header declares {}-byte records but the fields need {offset}",
            header.record_len
        )));
    }
    Ok(fields)
}

/// Read an `.mlg` log into the same `(channels, entries)` shape as
/// [`super::format::read_csv`].
///
/// Marker records — the notes TunerStudio writes when logging goes offline and
/// back online — advance the clock but carry no channel values, so they are
/// skipped.
///
/// Tolerates the damage real logs carry: a record cut short by a disconnect
/// ends the read, and a record whose checksum disagrees with its payload is
/// dropped rather than failing the whole file.
///
/// The whole log is held in memory, exactly like `read_csv`, and `.mlg`
/// captures are large: a few hundred channels over half an hour decode to
/// gigabytes of `f64`. Reading only the channels a caller asked for is the way
/// past that ceiling when it starts to bite.
pub fn read_mlg<P: AsRef<Path>>(path: P) -> io::Result<(Vec<String>, Vec<LogEntry>)> {
    let mut reader = BufReader::new(File::open(path)?);
    let header = read_header(&mut reader)?;
    let fields = read_fields(&mut reader, &header)?;

    let descriptors_end = (HEADER_LEN + header.field_count * DESCRIPTOR_LEN) as u64;
    if header.data_start < descriptors_end {
        return Err(invalid("log data would start inside its own header"));
    }
    reader.seek(SeekFrom::Start(header.data_start))?;

    let channels = fields.iter().map(|f| f.name.clone()).collect();
    let mut entries = Vec::new();
    let mut prefix = [0u8; RECORD_PREFIX_LEN];
    let mut data = vec![0u8; header.record_len + 1]; // payload then checksum
    let mut marker = [0u8; MARKER_TEXT_LEN];
    let mut clock = Clock::default();

    loop {
        if ends_log(reader.read_exact(&mut prefix))? {
            break;
        }
        // Advanced for every record, including ones dropped below, so that
        // damage does not shift the timestamps of what follows.
        let ticks = clock.advance(u16::from_be_bytes([prefix[2], prefix[3]]));
        match prefix[0] {
            KIND_DATA => {
                if ends_log(reader.read_exact(&mut data))? {
                    break;
                }
                let (payload, checksum) = data.split_at(header.record_len);
                if payload.iter().fold(0u8, |sum, b| sum.wrapping_add(*b)) != checksum[0] {
                    continue;
                }
                entries.push(LogEntry::new(
                    Duration::from_micros(ticks * CLOCK_TICK_MICROS),
                    fields.iter().map(|f| f.value(payload)).collect(),
                ));
            }
            KIND_MARKER => {
                if ends_log(reader.read_exact(&mut marker))? {
                    break;
                }
            }
            // Nothing else has a known length, so there is no way to resync.
            _ => break,
        }
    }
    if entries.is_empty() {
        return Err(invalid("MLG log contains no readable records"));
    }
    Ok((channels, entries))
}

/// Whether a read ran off the end of the log — the shape a capture cut short
/// by a disconnect or a full card takes. Any other failure is a real error.
fn ends_log(result: io::Result<()>) -> io::Result<bool> {
    match result {
        Ok(()) => Ok(false),
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(true),
        Err(e) => Err(e),
    }
}

/// Turns the 16-bit record clock back into a tick count from the first record.
#[derive(Default)]
struct Clock {
    start: Option<u64>,
    previous: u16,
    wraps: u64,
}

impl Clock {
    fn advance(&mut self, raw: u16) -> u64 {
        if self.start.is_some() && raw < self.previous {
            self.wraps += 1;
        }
        self.previous = raw;
        let absolute = self.wraps * CLOCK_WRAP + raw as u64;
        absolute - *self.start.get_or_insert(absolute)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    const MAGIC: &[u8; 6] = b"MLVLG\0";
    const HEADER_LEN: usize = 24;
    const DESCRIPTOR_LEN: usize = 89;

    struct TestField {
        code: u8,
        width: usize,
        name: &'static str,
        units: &'static str,
        scale: f32,
        transform: f32,
    }

    const TIME: TestField = TestField {
        code: 7,
        width: 4,
        name: "Time",
        units: "s",
        scale: 1.0,
        transform: 0.0,
    };
    const CLT: TestField = TestField {
        code: 2,
        width: 2,
        name: "CLT",
        units: "deg C",
        scale: 0.01,
        transform: 0.0,
    };
    const STFT: TestField = TestField {
        code: 7,
        width: 4,
        name: "STFT: Bank 1",
        units: "%",
        scale: 100.0,
        transform: -1.0,
    };

    /// A marker record: kind 1, counter, 16-bit clock, 50 bytes of text.
    /// TunerStudio writes one whenever logging goes offline or back online.
    fn marker(counter: u8, timestamp_10us: u16, text: &str) -> Vec<u8> {
        let mut out = vec![1u8, counter];
        out.extend_from_slice(&timestamp_10us.to_be_bytes());
        out.extend_from_slice(text.as_bytes());
        out.extend(std::iter::repeat_n(0u8, 50 - text.len()));
        out
    }

    /// One data record: kind 0, counter, 16-bit timestamp, payload, checksum.
    fn record(counter: u8, timestamp_10us: u16, payload: &[u8]) -> Vec<u8> {
        let mut out = vec![0u8, counter];
        out.extend_from_slice(&timestamp_10us.to_be_bytes());
        out.extend_from_slice(payload);
        out.push(payload.iter().fold(0u8, |sum, b| sum.wrapping_add(*b)));
        out
    }

    fn build_log(version: u16, fields: &[&TestField], records: &[Vec<u8>]) -> Vec<u8> {
        let record_len: usize = fields.iter().map(|f| f.width).sum();
        let info_start = HEADER_LEN + fields.len() * DESCRIPTOR_LEN;

        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&version.to_be_bytes());
        out.extend_from_slice(&1_784_378_352u32.to_be_bytes()); // capture time
        out.extend_from_slice(&(info_start as u32).to_be_bytes());
        out.extend_from_slice(&(info_start as u32).to_be_bytes()); // no info block
        out.extend_from_slice(&(record_len as u16).to_be_bytes());
        out.extend_from_slice(&(fields.len() as u16).to_be_bytes());
        assert_eq!(out.len(), HEADER_LEN);

        for field in fields {
            let start = out.len();
            out.push(field.code);
            let mut fixed = |text: &str, len: usize| {
                let bytes = text.as_bytes();
                out.extend_from_slice(bytes);
                out.extend(std::iter::repeat_n(0u8, len - bytes.len()));
            };
            fixed(field.name, 34);
            fixed(field.units, 10);
            out.push(0); // display style
            out.extend_from_slice(&field.scale.to_be_bytes());
            out.extend_from_slice(&field.transform.to_be_bytes());
            out.push(3); // digits
            out.extend(std::iter::repeat_n(0u8, 34)); // category
            assert_eq!(out.len() - start, DESCRIPTOR_LEN);
        }

        for r in records {
            out.extend_from_slice(r);
        }
        out
    }

    fn write_temp(bytes: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.mlg");
        std::fs::write(&path, bytes).unwrap();
        (dir, path)
    }

    fn payload(time: f32, clt: u16, stft: f32) -> Vec<u8> {
        let mut p = time.to_be_bytes().to_vec();
        p.extend_from_slice(&clt.to_be_bytes());
        p.extend_from_slice(&stft.to_be_bytes());
        p
    }

    #[test]
    fn reads_channel_names_and_physically_scaled_values() {
        let bytes = build_log(
            2,
            &[&TIME, &CLT, &STFT],
            &[
                record(0, 1_000, &payload(0.0, 8741, 0.935_228_5)),
                record(1, 2_000, &payload(0.01, 8747, 1.0)),
            ],
        );
        let (_dir, path) = write_temp(&bytes);

        let (channels, entries) = read_mlg(&path).unwrap();

        assert_eq!(channels, vec!["Time", "CLT", "STFT: Bank 1"]);
        assert_eq!(entries.len(), 2);
        // physical = (raw + transform) * scale
        assert!((entries[0].values[1] - 87.41).abs() < 1e-4, "CLT scaling");
        assert!(
            (entries[0].values[2] + 6.477).abs() < 1e-3,
            "trim transform"
        );
        // an untrimmed bank reads raw 1.0, which is 0 % of correction
        assert!(entries[1].values[2].abs() < 1e-4, "neutral trim is 0 %");
    }

    #[test]
    fn timestamps_start_at_zero_and_step_by_the_record_clock() {
        let bytes = build_log(
            2,
            &[&TIME, &CLT, &STFT],
            &[
                record(0, 1_000, &payload(0.0, 8741, 1.0)),
                record(1, 2_000, &payload(0.01, 8741, 1.0)),
            ],
        );
        let (_dir, path) = write_temp(&bytes);

        let (_, entries) = read_mlg(&path).unwrap();

        // The record clock counts 10 µs ticks: 1000 ticks apart is 10 ms.
        assert_eq!(entries[0].timestamp, std::time::Duration::ZERO);
        assert_eq!(
            entries[1].timestamp,
            std::time::Duration::from_micros(10_000)
        );
    }

    #[test]
    fn rejects_a_file_that_is_not_an_mlg_log() {
        let (_dir, path) = write_temp(b"Time,rpm\n0.0,900\n");
        assert!(read_mlg(&path).is_err());
    }

    #[test]
    fn rejects_a_format_version_it_cannot_decode() {
        let bytes = build_log(1, &[&TIME], &[record(0, 0, &0.0f32.to_be_bytes())]);
        let (_dir, path) = write_temp(&bytes);
        assert!(read_mlg(&path).is_err());
    }

    #[test]
    fn keeps_the_records_before_a_truncated_tail() {
        // Logs cut short by a disconnect or a full SD card end mid-record.
        let mut bytes = build_log(
            2,
            &[&TIME, &CLT, &STFT],
            &[
                record(0, 1_000, &payload(0.0, 8741, 1.0)),
                record(1, 2_000, &payload(0.01, 8747, 1.0)),
            ],
        );
        bytes.truncate(bytes.len() - 5);

        let (_dir, path) = write_temp(&bytes);
        let (_, entries) = read_mlg(&path).unwrap();

        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn skips_a_record_whose_checksum_does_not_match() {
        let mut corrupt = record(0, 1_000, &payload(0.0, 8741, 1.0));
        let last = corrupt.len() - 1;
        corrupt[last] ^= 0xff;

        let bytes = build_log(
            2,
            &[&TIME, &CLT, &STFT],
            &[corrupt, record(1, 2_000, &payload(0.01, 8747, 1.0))],
        );
        let (_dir, path) = write_temp(&bytes);

        let (_, entries) = read_mlg(&path).unwrap();

        assert_eq!(entries.len(), 1);
        assert!((entries[0].values[1] - 87.47).abs() < 1e-4);
    }
    #[test]
    fn keeps_the_records_that_follow_a_marker() {
        // Markers are shorter than data records, so a reader that mistakes one
        // for a data record loses everything recorded after it.
        let bytes = build_log(
            2,
            &[&TIME, &CLT, &STFT],
            &[
                record(0, 1_000, &payload(0.0, 8741, 1.0)),
                marker(1, 1_500, "MARK 000 - Going Offline"),
                record(2, 2_000, &payload(0.01, 8747, 1.0)),
            ],
        );
        let (_dir, path) = write_temp(&bytes);

        let (_, entries) = read_mlg(&path).unwrap();

        assert_eq!(entries.len(), 2);
        assert!((entries[1].values[1] - 87.47).abs() < 1e-4);
    }

    #[test]
    fn rejects_a_log_that_decodes_to_no_records() {
        let bytes = build_log(2, &[&TIME, &CLT, &STFT], &[]);
        let (_dir, path) = write_temp(&bytes);

        assert!(read_mlg(&path).is_err());
    }

    /// Checks the reader against a log TunerStudio actually wrote. Repo
    /// fixtures are synthetic because real logs run to hundreds of megabytes,
    /// so point this at one by hand:
    ///
    /// ```text
    /// LIBRETUNE_MLG_FIXTURE=/path/to/log.mlg \
    ///   cargo test -p libretune-core reads_a_real_tunerstudio_log -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "needs a real .mlg log, see LIBRETUNE_MLG_FIXTURE"]
    fn reads_a_real_tunerstudio_log() {
        let Ok(fixture) = std::env::var("LIBRETUNE_MLG_FIXTURE") else {
            panic!("set LIBRETUNE_MLG_FIXTURE to a .mlg log");
        };

        let (channels, entries) = read_mlg(&fixture).unwrap();

        assert!(!channels.is_empty(), "log declares no channels");
        assert!(!entries.is_empty(), "log decoded to no records");
        for entry in &entries {
            assert_eq!(entry.values.len(), channels.len());
        }
        let rpm = channels
            .iter()
            .position(|c| c == "RPM")
            .expect("a rusEFI log has an RPM channel");
        let peak = entries
            .iter()
            .map(|e| e.values[rpm])
            .fold(f64::MIN, f64::max);
        assert!(
            (0.0..=20_000.0).contains(&peak),
            "peak RPM {peak} is not a physical engine speed"
        );
        println!(
            "{} channels, {} records, {:?} long, peak RPM {peak}",
            channels.len(),
            entries.len(),
            entries.last().unwrap().timestamp,
        );
    }
}
