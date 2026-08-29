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

/// Read a saved log, choosing the reader from the file extension.
pub fn read_log<P: AsRef<Path>>(path: P) -> io::Result<(Vec<String>, Vec<LogEntry>)> {
    let path = path.as_ref();
    match LogFormat::from_extension(path) {
        Some(LogFormat::Csv) => read_csv(path),
        Some(LogFormat::Mlg) => super::mlg::read_mlg(path),
        None => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not a log format LibreTune reads", path.display()),
        )),
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

/// Read a CSV log back into `(channels, entries)`.
///
/// Accepts the files [`write_csv`] and the streaming recorder produce — a
/// `Time` header column followed by one column per channel — and is tolerant
/// of the usual real-world cruft: a UTF-8 BOM, trailing empty lines, and
/// short/malformed rows (skipped with a count rather than failing the whole
/// read). Channel names keep their file order, which is the recorder's
/// channel order.
///
/// Returns an error when the file has no header row or no parsable data
/// rows, so callers can distinguish "empty log" from "not a CSV log".
pub fn read_csv<P: AsRef<Path>>(path: P) -> io::Result<(Vec<String>, Vec<LogEntry>)> {
    let raw = std::fs::read_to_string(path)?;
    // Strip a UTF-8 BOM if present (some editors add one).
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);

    let mut lines = raw.lines();
    let header = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "log file has no header row"))?;
    let mut header_fields = header.split(',');
    // First column is the time axis: "Time" (seconds, the streaming format)
    // or "Time (ms)" (manual save_log format). Anything else is not a log.
    let time_header = header_fields.next().unwrap_or_default().trim();
    if !time_header.to_lowercase().starts_with("time") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unexpected first column {time_header:?}; expected 'Time'"),
        ));
    }
    let timestamps_in_ms = time_header.to_lowercase().contains("ms");
    let mut channels: Vec<String> = header_fields.map(|c| c.trim().to_string()).collect();
    // Drop trailing empty column names (a trailing comma in the header).
    while channels.last().is_some_and(|c| c.is_empty()) {
        channels.pop();
    }
    if channels.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "log header has no channel columns",
        ));
    }

    let mut entries = Vec::new();
    let mut skipped = 0usize;
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut fields = line.split(',');
        let Some(time_field) = fields.next() else {
            skipped += 1;
            continue;
        };
        let Ok(timestamp_secs) = time_field.trim().parse::<f64>() else {
            skipped += 1;
            continue;
        };
        let timestamp_secs = if timestamps_in_ms {
            timestamp_secs / 1000.0
        } else {
            timestamp_secs
        };
        let mut values = Vec::with_capacity(channels.len());
        let mut ok = true;
        for field in fields {
            match field.trim().parse::<f64>() {
                Ok(v) => values.push(v),
                Err(_) => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok || values.len() != channels.len() {
            skipped += 1;
            continue;
        }
        entries.push(LogEntry::new(
            std::time::Duration::from_secs_f64(timestamp_secs.max(0.0)),
            values,
        ));
    }

    if entries.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("log has no parsable data rows ({skipped} skipped)"),
        ));
    }
    Ok((channels, entries))
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn csv_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.csv");
        let channels = vec!["rpm".to_string(), "map".to_string()];
        let entries = vec![
            LogEntry::new(std::time::Duration::from_secs_f64(0.0), vec![800.0, 40.0]),
            LogEntry::new(std::time::Duration::from_secs_f64(0.5), vec![1200.0, 55.0]),
        ];
        write_csv(&path, &channels, &entries).unwrap();

        let (read_ch, read_entries) = read_csv(&path).unwrap();
        assert_eq!(read_ch, channels);
        assert_eq!(read_entries.len(), 2);
        assert_eq!(read_entries[0].values, vec![800.0, 40.0]);
        assert_eq!(read_entries[1].values[0], 1200.0);
    }

    #[test]
    fn csv_reader_skips_bad_rows_and_bom() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.csv");
        std::fs::write(
            &path,
            "\u{feff}Time,rpm,map\n0.0,900,50\n\nnot-a-number,1,2\n1.0,1000\n1.5,1100,60\n",
        )
        .unwrap();

        let (channels, entries) = read_csv(&path).unwrap();
        assert_eq!(channels, vec!["rpm", "map"]);
        // "not-a-number" row and the short "1.0,1000" row are skipped.
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].values, vec![1100.0, 60.0]);
    }

    #[test]
    fn csv_reader_rejects_headerless_or_empty_logs() {
        let dir = tempfile::tempdir().unwrap();
        let empty = dir.path().join("empty.csv");
        std::fs::write(&empty, "").unwrap();
        assert!(read_csv(&empty).is_err());

        let header_only = dir.path().join("header.csv");
        std::fs::write(&header_only, "Time,rpm\n").unwrap();
        assert!(read_csv(&header_only).is_err());
    }
    #[test]
    fn read_log_picks_the_reader_from_the_extension() {
        let dir = tempfile::tempdir().unwrap();
        let csv = dir.path().join("log.csv");
        std::fs::write(&csv, "Time,rpm\n0.0,900\n").unwrap();

        let (channels, entries) = read_log(&csv).unwrap();

        assert_eq!(channels, vec!["rpm"]);
        assert_eq!(entries[0].values, vec![900.0]);
    }

    #[test]
    fn read_log_rejects_an_extension_it_has_no_reader_for() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.msl");
        std::fs::write(&path, "Time\n0.0\n").unwrap();

        assert!(read_log(&path).is_err());
    }
    #[test]
    fn read_log_sends_mlg_files_to_the_mlg_reader() {
        let dir = tempfile::tempdir().unwrap();
        // CSV text under an .mlg name, long enough to reach the signature
        // check: only the MLG reader rejects it, so this proves the dispatch.
        let path = dir.path().join("log.mlg");
        std::fs::write(&path, "Time,rpm\n0.0,900\n1.0,1000\n2.0,1100\n").unwrap();

        let error = read_log(&path).unwrap_err();

        assert!(
            error.to_string().contains("signature"),
            "expected the MLG reader to reject it, got: {error}"
        );
    }
}
