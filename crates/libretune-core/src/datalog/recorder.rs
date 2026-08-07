//! Data logger / recorder
//!
//! Records real-time data from the ECU.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::LogEntry;

/// Hard ceiling on entries kept in memory. Once reached, the oldest entries
/// are discarded (and counted — see `discarded`).
///
/// The old ceiling was 10,000 samples. At the ~8 Hz the legacy read path
/// managed that was ~21 minutes, and an ordinary 28-minute drive silently
/// lost its first ~7 minutes (D7). Raising the realtime read rate (the
/// expected-length read exit) makes this far worse: at 50 Hz, 10,000 samples
/// is only ~3.3 minutes. This ceiling gives ~66 min at 50 Hz / ~5.5 h at
/// 10 Hz. At ~77 f64 channels/entry that is roughly 120 MB worst case, which
/// is acceptable for a desktop tool; streaming straight to disk removes the
/// ceiling entirely and is the longer-term fix.
const MAX_BUFFER_SIZE: usize = 200_000;

/// Data logger state
pub struct DataLogger {
    /// Channel names
    channels: Vec<String>,
    /// In-memory log buffer
    buffer: VecDeque<LogEntry>,
    /// Start time of logging
    start_time: Option<Instant>,
    /// Whether logging is active
    is_recording: bool,
    /// Target sample rate in Hz
    sample_rate: f64,
    /// Last sample time
    last_sample: Option<Instant>,
    /// Oldest entries discarded because the buffer hit `max_buffer_size`.
    /// Nonzero means the saved log is missing its earliest samples — surfaced
    /// so the truncation is never silent (the failure mode behind D7).
    discarded: u64,
    /// Hard ceiling on retained entries. Defaults to `MAX_BUFFER_SIZE`; a
    /// field (rather than the const directly) so tests can exercise the
    /// discard path without pushing hundreds of thousands of samples.
    max_buffer_size: usize,
    /// Continuous stream-to-disk writer (TunerStudio-style: the log is written
    /// to a file as it is recorded, so it is saved the whole time and survives
    /// a crash). `None` = in-memory only.
    stream: Option<BufWriter<File>>,
    /// Path of the file being streamed to, if any.
    stream_path: Option<PathBuf>,
    /// Rows written to the stream file (used to flush periodically).
    rows_written: u64,
}

impl DataLogger {
    /// Create a new data logger with the given channels
    pub fn new(channels: Vec<String>) -> Self {
        Self {
            channels,
            buffer: VecDeque::with_capacity(MAX_BUFFER_SIZE),
            start_time: None,
            is_recording: false,
            sample_rate: 10.0, // Default 10 Hz
            last_sample: None,
            discarded: 0,
            max_buffer_size: MAX_BUFFER_SIZE,
            stream: None,
            stream_path: None,
            rows_written: 0,
        }
    }

    /// Begin streaming the log to `path` as CSV (`Time` + channel columns),
    /// written as each sample is recorded. Overwrites any existing file.
    /// Returns the error if the file cannot be created.
    pub fn start_streaming<P: AsRef<Path>>(&mut self, path: P) -> std::io::Result<()> {
        let path = path.as_ref().to_path_buf();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let mut w = BufWriter::new(File::create(&path)?);
        write!(w, "Time")?;
        for c in &self.channels {
            write!(w, ",{c}")?;
        }
        writeln!(w)?;
        w.flush()?;
        self.stream = Some(w);
        self.stream_path = Some(path);
        self.rows_written = 0;
        Ok(())
    }

    /// Path of the file being streamed to, if streaming is active.
    pub fn stream_path(&self) -> Option<&Path> {
        self.stream_path.as_deref()
    }

    /// Override the buffer ceiling. Test-only: lets the discard/counter path
    /// be exercised without pushing `MAX_BUFFER_SIZE` samples through the
    /// real-time rate limiter.
    #[cfg(test)]
    fn set_max_buffer_size(&mut self, n: usize) {
        self.max_buffer_size = n;
    }

    /// Set the target sample rate in Hz
    pub fn set_sample_rate(&mut self, rate: f64) {
        self.sample_rate = rate.clamp(1.0, 200.0);
    }

    /// Get the sample rate
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }

    /// Start (or resume) recording.
    ///
    /// Recording appends to the existing buffer: the timeline continues from
    /// the last recorded entry, so stop/start cycles produce one continuous
    /// log with no gaps. Use [`clear`](Self::clear) to begin a fresh log.
    pub fn start(&mut self) {
        let now = Instant::now();
        let elapsed = self.duration();
        self.start_time = now.checked_sub(elapsed).or(Some(now));
        self.is_recording = true;
        self.last_sample = None;
    }

    /// Stop recording and finalize the continuous log file.
    ///
    /// The stream writer is flushed and dropped (closing the OS file handle), so
    /// the on-disk log is complete and nothing further can be appended to it.
    /// This is what ends logging cleanly on ECU disconnect: with the stream
    /// closed and recording off, the file can never grow with post-disconnect
    /// junk or be left half-open. A fresh `start_streaming` opens a new file.
    pub fn stop(&mut self) {
        self.is_recording = false;
        if let Some(mut w) = self.stream.take() {
            let _ = w.flush();
        }
        self.stream_path = None;
    }

    /// Check if recording is active
    pub fn is_recording(&self) -> bool {
        self.is_recording
    }

    /// Record a sample
    pub fn record(&mut self, values: Vec<f64>) {
        if !self.is_recording {
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

        // Stream this sample straight to disk (saved the whole time). Any error
        // here is a disk/file I/O failure (disk full, file gone) — NOT ECU data:
        // engine cut codes and the like are ordinary channel values and are
        // recorded in `values` above. Rather than fail silently, a write error
        // is logged once and the stream is dropped, so a broken file surfaces
        // instead of quietly stopping and never stalls the realtime path.
        if self.stream.is_some() {
            self.rows_written += 1;
            let should_flush = self.rows_written.is_multiple_of(25);
            let ts = timestamp.as_secs_f64();
            let w = self.stream.as_mut().unwrap();
            let res: std::io::Result<()> = (|| {
                write!(w, "{ts:.3}")?;
                for v in &values {
                    write!(w, ",{v}")?;
                }
                writeln!(w)?;
                if should_flush {
                    w.flush()?;
                }
                Ok(())
            })();
            if let Err(e) = res {
                tracing::warn!(
                    "Data log stream write failed ({e}); stopping the continuous \
                     file at {:?}. In-memory recording continues and can still be \
                     saved manually.",
                    self.stream_path
                );
                self.stream = None;
                self.stream_path = None;
            }
        }

        let entry = LogEntry::new(timestamp, values);

        // Manage buffer size. Discarding the oldest entry means the saved log
        // will be missing its start; count it (and warn the first time) so the
        // loss is visible rather than silent (D7).
        if self.buffer.len() >= self.max_buffer_size {
            self.buffer.pop_front();
            if self.discarded == 0 {
                tracing::warn!(
                    "Data log hit the {}-sample memory ceiling; oldest samples are now \
                     being discarded. Save more often, or lower the sample rate, to keep \
                     the whole session.",
                    self.max_buffer_size
                );
            }
            self.discarded += 1;
        }

        self.buffer.push_back(entry);
        self.last_sample = Some(now);
    }

    /// Number of oldest samples discarded because the buffer hit its memory
    /// ceiling. Nonzero means the log no longer covers the whole session.
    pub fn discarded_count(&self) -> u64 {
        self.discarded
    }

    /// Get the number of recorded entries
    pub fn entry_count(&self) -> usize {
        self.buffer.len()
    }

    /// Get all entries
    pub fn entries(&self) -> impl Iterator<Item = &LogEntry> {
        self.buffer.iter()
    }

    /// Get the channel names
    pub fn channels(&self) -> &[String] {
        &self.channels
    }

    /// Clear all recorded data
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.start_time = None;
        self.discarded = 0;
    }

    /// Get the duration of the log
    pub fn duration(&self) -> Duration {
        self.buffer.back().map(|e| e.timestamp).unwrap_or_default()
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
    fn test_restart_appends_with_continuous_timeline() {
        let mut logger = DataLogger::new(vec!["rpm".into()]);
        logger.set_sample_rate(200.0);

        logger.start();
        logger.record(vec![1000.0]);
        logger.stop();
        assert_eq!(logger.entry_count(), 1);
        let first_ts = logger.duration();

        // Restarting must keep the previous entries and continue the timeline
        logger.start();
        std::thread::sleep(Duration::from_millis(10));
        logger.record(vec![2000.0]);
        assert_eq!(logger.entry_count(), 2);
        assert!(logger.duration() >= first_ts);

        // Only clear() wipes the log
        logger.clear();
        assert_eq!(logger.entry_count(), 0);
        logger.start();
        logger.record(vec![3000.0]);
        assert_eq!(logger.entry_count(), 1);
    }

    #[test]
    fn discards_oldest_past_ceiling_and_counts_them() {
        let mut logger = DataLogger::new(vec!["rpm".into()]);
        logger.set_max_buffer_size(3);
        logger.set_sample_rate(200.0); // 5 ms min interval
        logger.start();

        // Push 5 samples spaced past the rate-limit interval so each is kept.
        for i in 0..5 {
            logger.record(vec![i as f64]);
            std::thread::sleep(Duration::from_millis(7));
        }

        // Buffer holds only the last 3; the 2 oldest were discarded and counted.
        assert_eq!(logger.entry_count(), 3);
        assert_eq!(logger.discarded_count(), 2);

        // clear() resets the discard counter for a fresh session.
        logger.clear();
        assert_eq!(logger.discarded_count(), 0);
    }

    #[test]
    fn streams_samples_to_disk_continuously() {
        // Each recorded sample must land in the file as it happens (saved the
        // whole time), with a Time + channel header.
        let dir = std::env::temp_dir().join(format!("lt_stream_{}", std::process::id()));
        let path = dir.join("session.csv");
        let mut logger = DataLogger::new(vec!["rpm".into(), "map".into()]);
        logger.set_sample_rate(200.0);
        logger.start_streaming(&path).expect("open stream file");
        assert_eq!(logger.stream_path(), Some(path.as_path()));
        logger.start();
        logger.record(vec![1000.0, 50.0]);
        std::thread::sleep(Duration::from_millis(7));
        logger.record(vec![2000.0, 60.0]);
        logger.stop(); // flushes

        let content = std::fs::read_to_string(&path).expect("read stream file");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines[0], "Time,rpm,map");
        assert!(lines.len() >= 3, "header + 2 rows, got {}", lines.len());
        assert!(lines[1].contains("1000") && lines[1].contains("50"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
