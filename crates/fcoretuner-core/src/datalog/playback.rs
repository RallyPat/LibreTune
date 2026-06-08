//! Log playback
//!
//! Plays back recorded log files for analysis.

use super::LogEntry;
use std::time::Duration;

/// Log file player for playback and analysis
pub struct LogPlayer {
    /// Log entries
    entries: Vec<LogEntry>,
    /// Channel names
    channels: Vec<String>,
    /// Current playback position
    position: usize,
}

impl LogPlayer {
    /// Create a new log player
    pub fn new(channels: Vec<String>, entries: Vec<LogEntry>) -> Self {
        Self {
            entries,
            channels,
            position: 0,
        }
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the channel names
    pub fn channels(&self) -> &[String] {
        &self.channels
    }

    /// Get the total duration
    pub fn duration(&self) -> Duration {
        self.entries.last().map(|e| e.timestamp).unwrap_or_default()
    }

    /// Get the current position
    pub fn position(&self) -> usize {
        self.position
    }

    /// Seek to a position
    pub fn seek(&mut self, position: usize) {
        self.position = position.min(self.entries.len().saturating_sub(1));
    }

    /// Seek to a time
    pub fn seek_to_time(&mut self, time: Duration) {
        self.position = self
            .entries
            .iter()
            .position(|e| e.timestamp >= time)
            .unwrap_or(self.entries.len().saturating_sub(1));
    }

    /// Get the current entry
    pub fn current(&self) -> Option<&LogEntry> {
        self.entries.get(self.position)
    }

    /// Advance to the next entry
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<&LogEntry> {
        if self.position < self.entries.len() - 1 {
            self.position += 1;
            self.current()
        } else {
            None
        }
    }

    /// Go to the previous entry
    pub fn previous(&mut self) -> Option<&LogEntry> {
        if self.position > 0 {
            self.position -= 1;
            self.current()
        } else {
            None
        }
    }

    /// Get all entries
    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }

    /// Get entries in a time range
    pub fn entries_in_range(
        &self,
        start: Duration,
        end: Duration,
    ) -> impl Iterator<Item = &LogEntry> {
        self.entries
            .iter()
            .filter(move |e| e.timestamp >= start && e.timestamp <= end)
    }

    /// Find the index of a channel by name
    pub fn channel_index(&self, name: &str) -> Option<usize> {
        self.channels.iter().position(|c| c == name)
    }

    /// Get values for a specific channel
    pub fn channel_values(&self, channel: &str) -> Vec<f64> {
        let idx = match self.channel_index(channel) {
            Some(i) => i,
            None => return Vec::new(),
        };

        self.entries
            .iter()
            .filter_map(|e| e.values.get(idx).copied())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_player() -> LogPlayer {
        let channels = vec!["rpm".into(), "map".into()];
        let entries = vec![
            LogEntry::new(Duration::from_secs(0), vec![1000.0, 100.0]),
            LogEntry::new(Duration::from_secs(1), vec![2000.0, 80.0]),
            LogEntry::new(Duration::from_secs(2), vec![3000.0, 60.0]),
        ];
        LogPlayer::new(channels, entries)
    }

    #[test]
    fn test_player_navigation() {
        let mut player = make_test_player();

        assert_eq!(player.position(), 0);
        assert_eq!(player.current().unwrap().values[0], 1000.0);

        player.next();
        assert_eq!(player.position(), 1);

        player.seek_to_time(Duration::from_millis(1500));
        assert_eq!(player.position(), 2);
    }

    #[test]
    fn test_channel_values() {
        let player = make_test_player();
        let rpm_values = player.channel_values("rpm");
        assert_eq!(rpm_values, vec![1000.0, 2000.0, 3000.0]);
    }
}
