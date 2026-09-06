//! Data logger / recorder
//!
//! Samples are written straight to a `.ltlog` file. RAM holds only a short
//! live-graph tail plus the writer's current compression block.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::ltlog::{LtlogChannel, LtlogSchema, LtlogWriter};
use super::LogEntry;

/// Samples kept in RAM for the live graph. The file is the log.
const LIVE_TAIL: usize = 2048;

/// Data logger state
pub struct DataLogger {
    /// Channel names
    channels: Vec<String>,
    /// Rolling window for the live graph only — not the session log.
    tail: VecDeque<LogEntry>,
    /// Start time of the current file
    start_time: Option<Instant>,
    /// Whether logging is active
    is_recording: bool,
    /// Target sample rate in Hz
    sample_rate: f64,
    /// Last sample time
    last_sample: Option<Instant>,
    /// Timestamp of the last accepted sample in this file
    last_timestamp: Duration,
    /// Continuous stream-to-disk writer. `None` = not streaming.
    stream: Option<LtlogWriter>,
    /// Path of the file currently being written.
    stream_path: Option<PathBuf>,
    /// Directory used to open timestamped files (for rotate-on-clear).
    stream_dir: Option<PathBuf>,
    /// Last finished file, so Save As / AI can still find it after stop.
    finished_path: Option<PathBuf>,
    /// Rows written to the current file (and accepted without a stream in tests).
    rows_written: u64,
    /// Rows dropped because their column count did not match `channels` — a
    /// torn/misaligned serial read. Nonzero means a few samples were skipped
    /// (never written with wrong columns), surfaced rather than silent.
    malformed: u64,
    /// ECU INI signature captured into the `.ltlog` header.
    ini_signature: Option<String>,
    /// Per-channel units (parallel to `channels`; empty strings if unknown).
    channel_units: Vec<String>,
    /// Per-channel INI type names (parallel to `channels`).
    channel_ini_types: Vec<String>,
}

impl DataLogger {
    /// Create a new data logger with the given channels
    pub fn new(channels: Vec<String>) -> Self {
        let n = channels.len();
        Self {
            channels,
            tail: VecDeque::with_capacity(LIVE_TAIL),
            start_time: None,
            is_recording: false,
            sample_rate: 10.0, // Default 10 Hz
            last_sample: None,
            last_timestamp: Duration::ZERO,
            stream: None,
            stream_path: None,
            stream_dir: None,
            finished_path: None,
            rows_written: 0,
            malformed: 0,
            ini_signature: None,
            channel_units: vec![String::new(); n],
            channel_ini_types: vec![String::new(); n],
        }
    }

    /// Begin streaming the log to `path` as `.ltlog`.
    /// Overwrites any existing file. Returns the error if the file cannot be created.
    pub fn start_streaming<P: AsRef<Path>>(&mut self, path: P) -> std::io::Result<()> {
        if let Some(mut prev) = self.stream.take() {
            let _ = prev.finish();
            if let Some(old) = self.stream_path.take() {
                self.finished_path = Some(old);
            }
        }
        let path = path.as_ref().to_path_buf();
        if let Some(dir) = path.parent() {
            self.stream_dir = Some(dir.to_path_buf());
        }
        let writer = LtlogWriter::create(&path, &self.capture_schema())?;
        self.stream = Some(writer);
        self.stream_path = Some(path);
        self.rows_written = 0;
        self.tail.clear();
        self.last_timestamp = Duration::ZERO;
        Ok(())
    }

    /// INI signature / channel units written into the `.ltlog` header.
    pub fn set_capture_meta(
        &mut self,
        signature: Option<String>,
        units: Vec<String>,
        ini_types: Vec<String>,
    ) {
        self.ini_signature = signature.filter(|s| !s.is_empty());
        if units.len() == self.channels.len() {
            self.channel_units = units;
        }
        if ini_types.len() == self.channels.len() {
            self.channel_ini_types = ini_types;
        }
    }

    /// Schema that a manual save or stream file should carry.
    pub fn capture_schema(&self) -> LtlogSchema {
        LtlogSchema {
            created_utc: chrono::Utc::now().to_rfc3339(),
            ini_signature: self.ini_signature.clone(),
            sample_rate_hz: self.sample_rate,
            channels: self
                .channels
                .iter()
                .enumerate()
                .map(|(i, name)| LtlogChannel {
                    name: name.clone(),
                    unit: self.channel_units.get(i).cloned().unwrap_or_default(),
                    ini_type: self.channel_ini_types.get(i).cloned().unwrap_or_default(),
                })
                .collect(),
        }
    }

    /// Path of the file being streamed to, if a writer is open.
    pub fn stream_path(&self) -> Option<&Path> {
        self.stream_path.as_deref()
    }

    /// Current open file, or the last file that was finished.
    pub fn log_path(&self) -> Option<&Path> {
        self.stream_path
            .as_deref()
            .or(self.finished_path.as_deref())
    }

    /// Flush pending compressed samples so a reader/copy sees them.
    pub fn flush_stream(&mut self) -> std::io::Result<()> {
        if let Some(w) = self.stream.as_mut() {
            w.flush_pending()?;
        }
        Ok(())
    }

    /// Set the target sample rate in Hz
    pub fn set_sample_rate(&mut self, rate: f64) {
        self.sample_rate = rate.clamp(1.0, 200.0);
    }

    /// Get the sample rate
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }

    /// Start recording into the current stream file (timeline from zero).
    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
        self.is_recording = true;
        self.last_sample = None;
        self.last_timestamp = Duration::ZERO;
        self.rows_written = 0;
        self.tail.clear();
    }

    /// Stop recording and finalize the log file.
    pub fn stop(&mut self) {
        self.is_recording = false;
        if let Some(mut w) = self.stream.take() {
            let _ = w.finish();
        }
        if let Some(path) = self.stream_path.take() {
            self.finished_path = Some(path);
        }
    }

    /// Check if recording is active
    pub fn is_recording(&self) -> bool {
        self.is_recording
    }

    /// Test helper: record ignoring the sample-rate limiter.
    #[cfg(test)]
    fn record_unthrottled(&mut self, values: Vec<f64>) {
        self.last_sample = None;
        self.record(values);
    }

    /// Record a sample
    pub fn record(&mut self, values: Vec<f64>) {
        if !self.is_recording {
            return;
        }

        // Guard the append point against a torn/misaligned read. A row with the
        // wrong column count would desync every column after it; dropping it
        // and counting is safer than persisting corrupt data. Empty `channels`
        // = no schema to check against, so accept (keeps existing tests).
        if !self.channels.is_empty() && values.len() != self.channels.len() {
            if self.malformed == 0 {
                tracing::warn!(
                    "Data log row had {} values but {} channels; dropping it (and any \
                     further mismatched rows). Likely a partial serial read.",
                    values.len(),
                    self.channels.len()
                );
            }
            self.malformed += 1;
            return;
        }

        let now = Instant::now();

        // Check sample rate
        let min_interval = Duration::from_secs_f64(1.0 / self.sample_rate);
        if let Some(last) = self.last_sample {
            if now.duration_since(last) < min_interval {
                return;
            }
        }

        let timestamp = self
            .start_time
            .map(|start| now.duration_since(start))
            .unwrap_or_default();

        let entry = LogEntry::new(timestamp, values);

        // Disk is the log. A write failure stops recording rather than
        // silently filling RAM.
        if let Some(w) = self.stream.as_mut() {
            if let Err(e) = w.push(&entry) {
                tracing::warn!(
                    "Data log stream write failed ({e}); stopping recording at {:?}.",
                    self.stream_path
                );
                self.stream = None;
                if let Some(path) = self.stream_path.take() {
                    self.finished_path = Some(path);
                }
                self.is_recording = false;
                return;
            }
        }

        self.rows_written += 1;
        self.last_timestamp = timestamp;
        if self.tail.len() >= LIVE_TAIL {
            self.tail.pop_front();
        }
        self.tail.push_back(entry);
        self.last_sample = Some(now);
    }

    /// Rows dropped for having the wrong column count (a torn serial read).
    pub fn malformed_count(&self) -> u64 {
        self.malformed
    }

    /// Samples written to the current file (session length, not RAM size).
    pub fn entry_count(&self) -> usize {
        self.rows_written as usize
    }

    /// Live-graph tail (not the full session).
    pub fn entries(&self) -> impl Iterator<Item = &LogEntry> {
        self.tail.iter()
    }

    /// Slice of the live tail using absolute session indices.
    pub fn live_window(&self, start_index: usize, count: usize) -> impl Iterator<Item = &LogEntry> {
        let total = self.rows_written as usize;
        let origin = total.saturating_sub(self.tail.len());
        let start = start_index.min(total);
        let end = start.saturating_add(count).min(total);
        let from = start.max(origin);
        let skip = from.saturating_sub(origin);
        let take = end.saturating_sub(from);
        self.tail.iter().skip(skip).take(take)
    }

    /// Get the channel names
    pub fn channels(&self) -> &[String] {
        &self.channels
    }

    /// Reset the live session. The file on disk is kept; if still recording,
    /// a new timestamped file is opened in the same directory.
    pub fn clear(&mut self) {
        self.tail.clear();
        self.rows_written = 0;
        self.last_timestamp = Duration::ZERO;
        self.malformed = 0;
        if self.is_recording {
            if let Some(dir) = self.stream_dir.clone() {
                let name = chrono::Local::now().format("%Y-%m-%d_%H.%M.%S").to_string();
                let path = dir.join(format!("{name}.ltlog"));
                if self.start_streaming(&path).is_ok() {
                    self.start_time = Some(Instant::now());
                    self.last_sample = None;
                    return;
                }
            }
            self.start_time = Some(Instant::now());
            self.last_sample = None;
        } else {
            self.start_time = None;
        }
    }

    /// Get the duration of the current file
    pub fn duration(&self) -> Duration {
        self.last_timestamp
    }
}

impl Default for DataLogger {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lt_rec_{}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            name
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("session.ltlog")
    }

    #[test]
    fn test_logger_basic() {
        let mut logger = DataLogger::new(vec!["rpm".into(), "map".into()]);

        assert!(!logger.is_recording());

        logger.start();
        assert!(logger.is_recording());

        logger.record(vec![1000.0, 100.0]);
        assert_eq!(logger.entry_count(), 1);

        logger.stop();
        assert!(!logger.is_recording());
    }

    #[test]
    fn test_malformed_row_is_dropped_not_stored() {
        let mut logger = DataLogger::new(vec!["rpm".into(), "map".into()]);
        logger.start();

        logger.record(vec![1000.0, 100.0]); // ok
        logger.record(vec![1000.0]); // short — torn read
        logger.record(vec![1.0, 2.0, 3.0]); // long — misaligned

        // Only the well-formed row is stored; the two mismatched rows are
        // dropped and counted (the sample-rate limiter is downstream of the
        // guard, so it never even sees them).
        assert_eq!(logger.entry_count(), 1);
        assert_eq!(logger.malformed_count(), 2);
    }

    #[test]
    fn start_begins_a_new_timeline() {
        let mut logger = DataLogger::new(vec!["rpm".into()]);
        logger.set_sample_rate(200.0);

        logger.start();
        logger.record(vec![1000.0]);
        logger.stop();
        assert_eq!(logger.entry_count(), 1);

        logger.start();
        std::thread::sleep(Duration::from_millis(10));
        logger.record(vec![2000.0]);
        assert_eq!(logger.entry_count(), 1);

        logger.clear();
        assert_eq!(logger.entry_count(), 0);
        logger.start();
        logger.record(vec![3000.0]);
        assert_eq!(logger.entry_count(), 1);
    }

    #[test]
    fn live_tail_stays_bounded_full_log_is_the_file() {
        let path = temp_path("tail");
        let mut logger = DataLogger::new(vec!["rpm".into()]);
        logger.set_sample_rate(200.0);
        logger.start_streaming(&path).expect("open stream file");
        logger.start();

        let n = LIVE_TAIL + 32;
        for i in 0..n {
            logger.record_unthrottled(vec![i as f64]);
        }
        logger.stop();

        assert_eq!(logger.entry_count(), n);
        assert_eq!(logger.entries().count(), LIVE_TAIL);

        let (_, entries) = crate::datalog::ltlog::read_ltlog(&path).expect("read stream file");
        assert_eq!(entries.len(), n);
        let _ = std::fs::remove_file(&path);
        if let Some(dir) = path.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn streams_samples_to_disk_continuously() {
        let path = temp_path("stream");
        let mut logger = DataLogger::new(vec!["rpm".into(), "map".into()]);
        logger.set_sample_rate(200.0);
        logger.start_streaming(&path).expect("open stream file");
        assert_eq!(logger.stream_path(), Some(path.as_path()));
        logger.start();
        logger.record(vec![1000.0, 50.0]);
        std::thread::sleep(Duration::from_millis(7));
        logger.record(vec![2000.0, 60.0]);
        logger.stop();

        let (channels, entries) =
            crate::datalog::ltlog::read_ltlog(&path).expect("read stream file");
        assert_eq!(channels, vec!["rpm", "map"]);
        assert!(
            entries.len() >= 2,
            "expected 2 samples, got {}",
            entries.len()
        );
        assert!((entries[0].values[0] - 1000.0).abs() < 0.01);
        assert!((entries[1].values[1] - 60.0).abs() < 0.01);
        let _ = std::fs::remove_file(&path);
        if let Some(dir) = path.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}
