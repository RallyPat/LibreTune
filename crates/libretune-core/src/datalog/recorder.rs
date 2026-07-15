//! Data logger / recorder
//!
//! Records real-time data from the ECU.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use super::LogEntry;

/// Maximum entries to keep in memory before flushing
const MAX_BUFFER_SIZE: usize = 10000;

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
        }
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

    /// Stop recording
    pub fn stop(&mut self) {
        self.is_recording = false;
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

        let entry = LogEntry::new(timestamp, values);

        // Manage buffer size
        if self.buffer.len() >= MAX_BUFFER_SIZE {
            self.buffer.pop_front();
        }

        self.buffer.push_back(entry);
        self.last_sample = Some(now);
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
}
