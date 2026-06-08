//! Log file formats
//!
//! Supports reading/writing log files in various formats.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use super::LogEntry;

/// Supported log file formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// Comma-separated values
    Csv,
    /// MegaLogViewer format (.mlg)
    Mlg,
}

impl LogFormat {
    /// Detect format from file extension
    pub fn from_extension(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()?.to_lowercase().as_str() {
            "csv" => Some(LogFormat::Csv),
            "mlg" => Some(LogFormat::Mlg),
            _ => None,
        }
    }

    /// Get the file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            LogFormat::Csv => "csv",
            LogFormat::Mlg => "mlg",
        }
    }
}

/// Write log entries to a CSV file
#[allow(dead_code)]
pub fn write_csv<P: AsRef<Path>>(
    path: P,
    channels: &[String],
    entries: &[LogEntry],
) -> io::Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    // Write header
    write!(writer, "Time")?;
    for channel in channels {
        write!(writer, ",{}", channel)?;
    }
    writeln!(writer)?;

    // Write data rows
    for entry in entries {
        write!(writer, "{:.3}", entry.timestamp.as_secs_f64())?;
        for value in &entry.values {
            write!(writer, ",{:.4}", value)?;
        }
        writeln!(writer)?;
    }

    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_format_detection() {
        assert_eq!(
            LogFormat::from_extension(Path::new("log.csv")),
            Some(LogFormat::Csv)
        );
        assert_eq!(
            LogFormat::from_extension(Path::new("log.mlg")),
            Some(LogFormat::Mlg)
        );
        assert_eq!(LogFormat::from_extension(Path::new("log.txt")), None);
    }
}
