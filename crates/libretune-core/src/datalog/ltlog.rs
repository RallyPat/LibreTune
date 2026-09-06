//! LibreTune native datalog (`.ltlog`).
//!
//! Compact, little-endian, crash-tolerant binary log. A torn write or a
//! corrupt block loses only that block: earlier CRC-valid blocks still read.
//!
//! ```text
//! header     "LTLg" u16 version u16 flags u32 schema_len
//!            schema JSON (UTF-8)  u32 crc32(schema)
//! block*     "LTBK" u32 raw_len u32 zstd_len u32 crc32(zstd)
//!            zstd payload:
//!              u16 n_samples  u64 t0_us
//!              per sample: u32 dt_us  f32[n_channels]
//! footer     "LTFT" u64 samples u32 blocks u64 duration_us u32 crc32
//!            (optional; a file without one is still valid)
//! ```

use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};

use super::LogEntry;

const MAGIC: &[u8; 4] = b"LTLg";
const BLOCK_SYNC: &[u8; 4] = b"LTBK";
const FOOTER_SYNC: &[u8; 4] = b"LTFT";
const VERSION: u16 = 1;
/// Flush a block once this many samples are pending.
const BLOCK_SAMPLES: usize = 128;
/// Also flush if a block has been open this long (crash window).
const BLOCK_WALL_FLUSH: Duration = Duration::from_secs(1);
const ZSTD_LEVEL: i32 = 3;

/// Channel metadata stored once in the file header.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LtlogChannel {
    pub name: String,
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub ini_type: String,
}

/// Self-describing schema written as JSON after the binary preamble.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LtlogSchema {
    pub created_utc: String,
    #[serde(default)]
    pub ini_signature: Option<String>,
    pub sample_rate_hz: f64,
    pub channels: Vec<LtlogChannel>,
}

impl LtlogSchema {
    pub fn channel_names(&self) -> Vec<String> {
        self.channels.iter().map(|c| c.name.clone()).collect()
    }
}

/// Streaming writer used by the live recorder.
pub struct LtlogWriter {
    file: BufWriter<File>,
    n_channels: usize,
    pending: Vec<LogEntry>,
    samples_written: u64,
    blocks_written: u32,
    last_flush: Instant,
    finished: bool,
    last_duration_us: u64,
}

impl LtlogWriter {
    /// Create (overwrite) `path` and write the header immediately.
    pub fn create<P: AsRef<Path>>(path: P, schema: &LtlogSchema) -> io::Result<Self> {
        let path = path.as_ref();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let mut file = BufWriter::new(File::create(path)?);
        write_header(&mut file, schema)?;
        file.flush()?;
        Ok(Self {
            file,
            n_channels: schema.channels.len(),
            pending: Vec::with_capacity(BLOCK_SAMPLES),
            samples_written: 0,
            blocks_written: 0,
            last_flush: Instant::now(),
            finished: false,
            last_duration_us: 0,
        })
    }

    /// Buffer a sample; may compress-and-append a block.
    pub fn push(&mut self, entry: &LogEntry) -> io::Result<()> {
        if self.finished {
            return Err(io::Error::other("ltlog writer already finished"));
        }
        if entry.values.len() != self.n_channels {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "ltlog sample has {} values, schema has {} channels",
                    entry.values.len(),
                    self.n_channels
                ),
            ));
        }
        self.last_duration_us = entry.timestamp.as_micros() as u64;
        self.pending.push(entry.clone());
        if self.pending.len() >= BLOCK_SAMPLES || self.last_flush.elapsed() >= BLOCK_WALL_FLUSH {
            self.flush_block()?;
        }
        Ok(())
    }

    fn flush_block(&mut self) -> io::Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let n = self.pending.len();
        write_block(&mut self.file, self.n_channels, &self.pending)?;
        self.samples_written += n as u64;
        self.blocks_written += 1;
        self.pending.clear();
        self.last_flush = Instant::now();
        self.file.flush()?;
        Ok(())
    }

    /// Write any pending samples so a concurrent reader sees them.
    pub fn flush_pending(&mut self) -> io::Result<()> {
        if self.finished {
            return Ok(());
        }
        self.flush_block()
    }

    /// Flush the last block and write the optional footer.
    pub fn finish(&mut self) -> io::Result<()> {
        if self.finished {
            return Ok(());
        }
        self.flush_block()?;
        write_footer(
            &mut self.file,
            self.samples_written,
            self.blocks_written,
            self.last_duration_us,
        )?;
        self.file.flush()?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for LtlogWriter {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.finish();
        }
    }
}

/// Write a complete log in one shot (manual save).
pub fn write_ltlog<P: AsRef<Path>>(
    path: P,
    schema: &LtlogSchema,
    entries: &[LogEntry],
) -> io::Result<()> {
    let mut w = LtlogWriter::create(path, schema)?;
    for e in entries {
        w.push(e)?;
    }
    w.finish()
}

/// Chart / IPC cap: the file stays on disk; the UI only ever sees this many
/// points (evenly strided), so a multi-hour log cannot fill RAM.
pub const UI_SAMPLE_CAP: usize = 4096;

/// Read only the header (channel names, rate, signature). No samples.
pub fn read_ltlog_schema<P: AsRef<Path>>(path: P) -> io::Result<LtlogSchema> {
    let mut r = BufReader::new(File::open(path)?);
    read_header(&mut r)
}

/// Visit every sample without accumulating the log. RAM is one compressed
/// block (~128 rows) plus whatever the callback keeps.
pub fn visit_ltlog<P, F>(path: P, mut visit: F) -> io::Result<LtlogSchema>
where
    P: AsRef<Path>,
    F: FnMut(&LogEntry) -> io::Result<()>,
{
    let mut r = BufReader::new(File::open(path)?);
    let schema = read_header(&mut r)?;
    let n_channels = schema.channels.len();
    loop {
        match read_next_block(&mut r, n_channels) {
            Ok(Some(block)) => {
                for e in &block {
                    visit(e)?;
                }
            }
            Ok(None) => break,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        }
    }
    Ok(schema)
}

/// Read a `.ltlog` into `(channel names, entries)`.
///
/// Loads the whole file. Prefer [`visit_ltlog`], [`downsample_ltlog`], or
/// [`extract_ltlog_columns`] for anything that might be large.
///
/// A truncated or CRC-failed last block is skipped. Earlier valid blocks are
/// kept. A header-only file (crash before the first block) returns the
/// channel list and no samples.
pub fn read_ltlog<P: AsRef<Path>>(path: P) -> io::Result<(Vec<String>, Vec<LogEntry>)> {
    let mut entries = Vec::new();
    let schema = visit_ltlog(path, |e| {
        entries.push(e.clone());
        Ok(())
    })?;
    Ok((schema.channel_names(), entries))
}

/// Evenly stride the file down to at most `max_points` samples.
///
/// Returns `(schema, preview, true_sample_count)`. RAM is O(max_points).
pub fn downsample_ltlog<P: AsRef<Path>>(
    path: P,
    max_points: usize,
) -> io::Result<(LtlogSchema, Vec<LogEntry>, u64)> {
    let path = path.as_ref();
    let max_points = max_points.max(1);
    let n = match footer_sample_count(path) {
        Some(n) => n,
        None => {
            let mut n = 0u64;
            visit_ltlog(path, |_| {
                n += 1;
                Ok(())
            })?;
            n
        }
    };
    if n == 0 {
        let schema = read_ltlog_schema(path)?;
        return Ok((schema, Vec::new(), 0));
    }
    if n as usize <= max_points {
        let mut entries = Vec::with_capacity(n as usize);
        let schema = visit_ltlog(path, |e| {
            entries.push(e.clone());
            Ok(())
        })?;
        return Ok((schema, entries, n));
    }
    let stride = n as f64 / max_points as f64;
    let mut out = Vec::with_capacity(max_points);
    let mut next = 0.0f64;
    let mut i = 0u64;
    let schema = visit_ltlog(path, |e| {
        if (i as f64) >= next && out.len() < max_points {
            out.push(e.clone());
            next += stride;
        }
        i += 1;
        Ok(())
    })?;
    Ok((schema, out, n))
}

/// Pull named columns in one pass. Missing names yield empty vectors.
///
/// Used by offline analysis so a 100-channel log never sits in RAM — only
/// the handful of channels the caller asked for.
pub fn extract_ltlog_columns<P: AsRef<Path>>(
    path: P,
    names: &[&str],
) -> io::Result<(LtlogSchema, Vec<f64>, Vec<Vec<f64>>)> {
    let schema = read_ltlog_schema(&path)?;
    let idx: Vec<Option<usize>> = names
        .iter()
        .map(|want| schema.channels.iter().position(|c| c.name == *want))
        .collect();
    let mut time_ms = Vec::new();
    let mut cols: Vec<Vec<f64>> = names.iter().map(|_| Vec::new()).collect();
    visit_ltlog(path, |e| {
        time_ms.push(e.timestamp.as_secs_f64() * 1000.0);
        for (col, src) in cols.iter_mut().zip(&idx) {
            if let Some(i) = *src {
                col.push(e.values.get(i).copied().unwrap_or(f64::NAN));
            }
        }
        Ok(())
    })?;
    Ok((schema, time_ms, cols))
}

fn footer_sample_count(path: &Path) -> Option<u64> {
    let mut f = File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    if len < 28 {
        return None;
    }
    f.seek(SeekFrom::End(-28)).ok()?;
    let mut buf = [0u8; 28];
    f.read_exact(&mut buf).ok()?;
    if &buf[0..4] != FOOTER_SYNC {
        return None;
    }
    let crc = u32::from_le_bytes(buf[24..28].try_into().ok()?);
    if crc32fast::hash(&buf[4..24]) != crc {
        return None;
    }
    Some(u64::from_le_bytes(buf[4..12].try_into().ok()?))
}

fn write_header<W: Write>(w: &mut W, schema: &LtlogSchema) -> io::Result<()> {
    let json = serde_json::to_vec(schema).map_err(io::Error::other)?;
    let crc = crc32fast::hash(&json);
    w.write_all(MAGIC)?;
    w.write_u16::<LittleEndian>(VERSION)?;
    w.write_u16::<LittleEndian>(0)?;
    w.write_u32::<LittleEndian>(json.len() as u32)?;
    w.write_all(&json)?;
    w.write_u32::<LittleEndian>(crc)?;
    Ok(())
}

fn read_header<R: Read>(r: &mut R) -> io::Result<LtlogSchema> {
    let mut magic = [0u8; 4];
    r.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not an .ltlog file (bad magic)",
        ));
    }
    let version = r.read_u16::<LittleEndian>()?;
    if version != VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported ltlog version {version}"),
        ));
    }
    let _flags = r.read_u16::<LittleEndian>()?;
    let schema_len = r.read_u32::<LittleEndian>()? as usize;
    if schema_len > 16 * 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "ltlog schema is implausibly large",
        ));
    }
    let mut json = vec![0u8; schema_len];
    r.read_exact(&mut json)?;
    let crc = r.read_u32::<LittleEndian>()?;
    if crc32fast::hash(&json) != crc {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "ltlog header CRC mismatch",
        ));
    }
    serde_json::from_slice(&json).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn write_block<W: Write>(w: &mut W, n_channels: usize, entries: &[LogEntry]) -> io::Result<()> {
    let raw = pack_samples(n_channels, entries)?;
    let compressed = zstd::encode_all(raw.as_slice(), ZSTD_LEVEL).map_err(io::Error::other)?;
    let crc = crc32fast::hash(&compressed);
    w.write_all(BLOCK_SYNC)?;
    w.write_u32::<LittleEndian>(raw.len() as u32)?;
    w.write_u32::<LittleEndian>(compressed.len() as u32)?;
    w.write_u32::<LittleEndian>(crc)?;
    w.write_all(&compressed)?;
    Ok(())
}

fn pack_samples(n_channels: usize, entries: &[LogEntry]) -> io::Result<Vec<u8>> {
    let n = u16::try_from(entries.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "ltlog block has too many samples",
        )
    })?;
    let t0_us = entries
        .first()
        .map(|e| e.timestamp.as_micros() as u64)
        .unwrap_or(0);
    let mut raw = Vec::with_capacity(10 + entries.len() * (4 + 4 * n_channels));
    raw.write_u16::<LittleEndian>(n)?;
    raw.write_u64::<LittleEndian>(t0_us)?;
    let mut prev = t0_us;
    for (i, e) in entries.iter().enumerate() {
        let us = e.timestamp.as_micros() as u64;
        let dt = if i == 0 {
            0u32
        } else {
            us.saturating_sub(prev).min(u32::MAX as u64) as u32
        };
        raw.write_u32::<LittleEndian>(dt)?;
        for v in &e.values {
            raw.write_f32::<LittleEndian>(*v as f32)?;
        }
        prev = prev.saturating_add(dt as u64);
    }
    Ok(raw)
}

fn unpack_samples(n_channels: usize, raw: &[u8]) -> io::Result<Vec<LogEntry>> {
    let mut cur = raw;
    let n = cur.read_u16::<LittleEndian>()? as usize;
    let t0_us = cur.read_u64::<LittleEndian>()?;
    let mut prev = t0_us;
    let mut out = Vec::with_capacity(n);
    let row = 4 + 4 * n_channels;
    for i in 0..n {
        if cur.len() < row {
            break;
        }
        let dt = cur.read_u32::<LittleEndian>()?;
        let us = if i == 0 {
            t0_us
        } else {
            prev.saturating_add(dt as u64)
        };
        prev = us;
        let mut values = Vec::with_capacity(n_channels);
        for _ in 0..n_channels {
            values.push(cur.read_f32::<LittleEndian>()? as f64);
        }
        out.push(LogEntry::new(Duration::from_micros(us), values));
    }
    Ok(out)
}

/// Read one sample block, skipping junk until the next `LTBK` (or EOF / footer).
///
/// On a bad CRC or decompress, seek one byte past the false sync and scan
/// again so a corrupt length field cannot swallow the next real block.
fn read_next_block<R: Read + Seek>(
    r: &mut R,
    n_channels: usize,
) -> io::Result<Option<Vec<LogEntry>>> {
    loop {
        match find_sync(r, BLOCK_SYNC, FOOTER_SYNC)? {
            SyncFound::Block => {}
            SyncFound::Footer | SyncFound::Eof => return Ok(None),
        }
        let after_sync = r.stream_position()?;
        let raw_len = match r.read_u32::<LittleEndian>() {
            Ok(v) => v as usize,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        };
        let zlen = match r.read_u32::<LittleEndian>() {
            Ok(v) => v as usize,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        };
        let crc = match r.read_u32::<LittleEndian>() {
            Ok(v) => v,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        };
        if zlen > 64 * 1024 * 1024 || raw_len > 128 * 1024 * 1024 {
            r.seek(SeekFrom::Start(after_sync.saturating_sub(3)))?;
            continue;
        }
        let mut compressed = vec![0u8; zlen];
        if r.read_exact(&mut compressed).is_err() {
            return Ok(None);
        }
        if crc32fast::hash(&compressed) != crc {
            continue;
        }
        let raw = match zstd::decode_all(compressed.as_slice()) {
            Ok(v) if v.len() == raw_len => v,
            _ => continue,
        };
        return Ok(Some(unpack_samples(n_channels, &raw)?));
    }
}

enum SyncFound {
    Block,
    Footer,
    Eof,
}

fn find_sync<R: Read>(r: &mut R, block: &[u8; 4], footer: &[u8; 4]) -> io::Result<SyncFound> {
    let mut window = [0u8; 4];
    match r.read(&mut window)? {
        0 => return Ok(SyncFound::Eof),
        1..=3 => return Ok(SyncFound::Eof),
        _ => {}
    }
    loop {
        if &window == block {
            return Ok(SyncFound::Block);
        }
        if &window == footer {
            return Ok(SyncFound::Footer);
        }
        window.copy_within(1.., 0);
        let mut b = [0u8; 1];
        match r.read(&mut b)? {
            0 => return Ok(SyncFound::Eof),
            _ => window[3] = b[0],
        }
    }
}

fn write_footer<W: Write>(
    w: &mut W,
    samples: u64,
    blocks: u32,
    duration_us: u64,
) -> io::Result<()> {
    let mut body = [0u8; 20];
    {
        let mut c = &mut body[..];
        c.write_u64::<LittleEndian>(samples)?;
        c.write_u32::<LittleEndian>(blocks)?;
        c.write_u64::<LittleEndian>(duration_us)?;
    }
    let crc = crc32fast::hash(&body);
    w.write_all(FOOTER_SYNC)?;
    w.write_all(&body)?;
    w.write_u32::<LittleEndian>(crc)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema(names: &[&str]) -> LtlogSchema {
        LtlogSchema {
            created_utc: "2026-01-01T00:00:00Z".into(),
            ini_signature: Some("speeduino 202501".into()),
            sample_rate_hz: 50.0,
            channels: names
                .iter()
                .map(|n| LtlogChannel {
                    name: (*n).into(),
                    unit: "rpm".into(),
                    ini_type: "float".into(),
                })
                .collect(),
        }
    }

    fn sample(t_ms: u64, values: Vec<f64>) -> LogEntry {
        LogEntry::new(Duration::from_millis(t_ms), values)
    }

    #[test]
    fn round_trips_samples_and_channel_names() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.ltlog");
        let sch = schema(&["rpm", "map"]);
        let entries = vec![
            sample(0, vec![800.0, 40.0]),
            sample(20, vec![1200.0, 55.5]),
            sample(40, vec![3000.0, 100.0]),
        ];
        write_ltlog(&path, &sch, &entries).unwrap();

        let (names, read) = read_ltlog(&path).unwrap();
        assert_eq!(names, vec!["rpm", "map"]);
        assert_eq!(read.len(), 3);
        assert!((read[1].values[0] - 1200.0).abs() < 0.01);
        assert_eq!(read[2].timestamp, Duration::from_millis(40));
    }

    #[test]
    fn header_only_file_returns_channels_and_no_samples() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.ltlog");
        write_ltlog(&path, &schema(&["rpm"]), &[]).unwrap();
        let (names, entries) = read_ltlog(&path).unwrap();
        assert_eq!(names, vec!["rpm"]);
        assert!(entries.is_empty());
    }

    #[test]
    fn truncated_tail_keeps_earlier_block() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("trunc.ltlog");
        let sch = schema(&["rpm"]);
        let mut entries = Vec::new();
        for i in 0..200 {
            entries.push(sample(i * 10, vec![i as f64]));
        }
        write_ltlog(&path, &sch, &entries).unwrap();

        let mut bytes = std::fs::read(&path).unwrap();
        bytes.truncate(bytes.len() - 40);
        std::fs::write(&path, &bytes).unwrap();

        let (_, read) = read_ltlog(&path).unwrap();
        assert!(read.len() >= BLOCK_SAMPLES, "got {} samples", read.len());
        assert!(read.len() < 200);
        assert_eq!(read[0].values[0], 0.0);
    }

    #[test]
    fn corrupt_later_block_keeps_earlier_samples() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corrupt.ltlog");
        let sch = schema(&["rpm"]);
        let mut entries = Vec::new();
        for i in 0..200 {
            entries.push(sample(i * 10, vec![i as f64]));
        }
        write_ltlog(&path, &sch, &entries).unwrap();

        let mut bytes = std::fs::read(&path).unwrap();
        let first = find_nth(&bytes, BLOCK_SYNC, 0).expect("first block");
        let second = find_nth(&bytes, BLOCK_SYNC, first + 4).expect("second block");
        // Flip a byte inside the second block's compressed payload.
        let flip = second + 16;
        bytes[flip] ^= 0xff;
        std::fs::write(&path, &bytes).unwrap();

        let (_, read) = read_ltlog(&path).unwrap();
        assert_eq!(read.len(), BLOCK_SAMPLES);
        assert_eq!(read[0].values[0], 0.0);
        assert_eq!(
            read[BLOCK_SAMPLES - 1].values[0],
            (BLOCK_SAMPLES - 1) as f64
        );
    }

    fn find_nth(hay: &[u8], needle: &[u8], start: usize) -> Option<usize> {
        hay[start..]
            .windows(needle.len())
            .position(|w| w == needle)
            .map(|i| start + i)
    }

    #[test]
    fn ltlog_is_much_smaller_than_csv_for_a_typical_session() {
        // 100 channels, 50 Hz, 10 s. Most ECU channels sit still or drift
        // slowly; only a handful move like RPM/MAP. That is what zstd eats.
        let n_ch = 100usize;
        let names: Vec<String> = (0..n_ch).map(|i| format!("ch{i:03}")).collect();
        let sch = LtlogSchema {
            created_utc: "2026-01-01T00:00:00Z".into(),
            ini_signature: None,
            sample_rate_hz: 50.0,
            channels: names
                .iter()
                .map(|n| LtlogChannel {
                    name: n.clone(),
                    unit: String::new(),
                    ini_type: String::new(),
                })
                .collect(),
        };
        let mut entries = Vec::new();
        for s in 0..500 {
            let t = s as f64 * 0.02;
            let rpm = 800.0 + t * 40.0;
            let map = 35.0 + (t * 0.7).sin() * 8.0;
            let values: Vec<f64> = (0..n_ch)
                .map(|c| match c {
                    0 => rpm,
                    1 => map,
                    2 => 14.7,
                    3 => 90.0 + t * 0.05,
                    _ if c < 20 => 0.0,
                    _ => 1.0,
                })
                .collect();
            entries.push(LogEntry::new(Duration::from_secs_f64(t), values));
        }
        let dir = tempfile::tempdir().unwrap();
        let lt = dir.path().join("a.ltlog");
        let csv = dir.path().join("a.csv");
        write_ltlog(&lt, &sch, &entries).unwrap();
        crate::datalog::format::write_csv(&csv, &names, &entries).unwrap();
        let lt_sz = std::fs::metadata(&lt).unwrap().len() as f64;
        let csv_sz = std::fs::metadata(&csv).unwrap().len() as f64;
        let ratio = csv_sz / lt_sz;
        eprintln!("size check: csv={csv_sz:.0} ltlog={lt_sz:.0} ratio={ratio:.1}x");
        assert!(
            ratio >= 8.0,
            "expected .ltlog to be at least 8× smaller than CSV, got {ratio:.1}x (csv={csv_sz}, ltlog={lt_sz})"
        );
    }

    #[test]
    fn downsample_keeps_bounds_not_every_sample() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.ltlog");
        let sch = schema(&["rpm"]);
        let entries: Vec<_> = (0..500).map(|i| sample(i * 10, vec![i as f64])).collect();
        write_ltlog(&path, &sch, &entries).unwrap();

        let mut visited = 0u64;
        visit_ltlog(&path, |_| {
            visited += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(visited, 500);

        let (_, preview, n) = downsample_ltlog(&path, 50).unwrap();
        assert_eq!(n, 500);
        assert!(preview.len() <= 50);
        assert!(preview.len() >= 40);
        assert_eq!(preview[0].values[0], 0.0);
        assert!(preview.last().unwrap().values[0] >= 450.0);
    }

    #[test]
    fn extract_columns_skips_unknown_names() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cols.ltlog");
        write_ltlog(
            &path,
            &schema(&["rpm", "map"]),
            &[sample(0, vec![800.0, 40.0]), sample(20, vec![1200.0, 55.0])],
        )
        .unwrap();
        let (_, time, cols) = extract_ltlog_columns(&path, &["map", "nope"]).unwrap();
        assert_eq!(time.len(), 2);
        assert_eq!(cols[0], vec![40.0, 55.0]);
        assert!(cols[1].is_empty());
    }
}
