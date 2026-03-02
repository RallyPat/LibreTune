//! Dyno data import and analysis
//!
//! Parses dynamometer run data from CSV files and computes power/torque curves.
//! Supports overlay on VE table views for visual correlation.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

/// A single dyno data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynoDataPoint {
    /// Engine RPM at this sample
    pub rpm: f64,
    /// Horsepower (if available)
    pub hp: Option<f64>,
    /// Torque in ft-lbs (if available)
    pub torque: Option<f64>,
    /// AFR at this point (if available)
    pub afr: Option<f64>,
    /// Boost pressure in PSI (if available, negative for vacuum)
    pub boost: Option<f64>,
    /// Time in seconds from start of run (if available)
    pub time: Option<f64>,
}

/// A complete dyno run with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynoRun {
    /// Name/label for this run (e.g., "Baseline", "After VE tune")
    pub name: String,
    /// Data points sorted by RPM
    pub data: Vec<DynoDataPoint>,
    /// Peak HP value and RPM
    pub peak_hp: Option<(f64, f64)>,
    /// Peak torque value and RPM
    pub peak_torque: Option<(f64, f64)>,
    /// Color for chart rendering (hex string)
    pub color: String,
    /// Source file path
    pub source_file: Option<String>,
}

/// Column mapping for CSV import
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynoColumnMap {
    /// Column index for RPM (required)
    pub rpm_col: usize,
    /// Column index for horsepower
    pub hp_col: Option<usize>,
    /// Column index for torque
    pub torque_col: Option<usize>,
    /// Column index for AFR
    pub afr_col: Option<usize>,
    /// Column index for boost
    pub boost_col: Option<usize>,
    /// Column index for time
    pub time_col: Option<usize>,
}

/// Result of comparing two dyno runs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynoComparison {
    /// Run A (baseline)
    pub run_a: DynoRun,
    /// Run B (comparison)
    pub run_b: DynoRun,
    /// HP difference at common RPM points
    pub hp_diff: Vec<(f64, f64)>,
    /// Torque difference at common RPM points
    pub torque_diff: Vec<(f64, f64)>,
    /// Overall HP gain/loss
    pub total_hp_change: Option<f64>,
    /// Overall torque gain/loss
    pub total_torque_change: Option<f64>,
}

/// VE table overlay data — maps dyno measurements to table cells
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynoTableOverlay {
    /// Table cell annotations from dyno data
    /// Each entry is (row, col, hp_at_cell, torque_at_cell)
    pub cell_data: Vec<DynoCellOverlay>,
}

/// Dyno data mapped to a single table cell
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynoCellOverlay {
    /// Row index in VE table
    pub row: usize,
    /// Column index in VE table
    pub col: usize,
    /// HP at this operating point
    pub hp: Option<f64>,
    /// Torque at this operating point
    pub torque: Option<f64>,
    /// AFR at this operating point
    pub afr: Option<f64>,
    /// Number of dyno samples hitting this cell
    pub sample_count: usize,
}

impl DynoRun {
    /// Create a new empty dyno run
    pub fn new(name: impl Into<String>, color: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
            peak_hp: None,
            peak_torque: None,
            color: color.into(),
            source_file: None,
        }
    }

    /// Parse a dyno CSV file with auto-detection of columns
    pub fn from_csv<P: AsRef<Path>>(path: P, name: impl Into<String>) -> io::Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())?;
        let name = name.into();

        // Try to auto-detect column mapping from headers
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Empty CSV file"));
        }

        // Parse header line
        let headers: Vec<String> = parse_csv_line(lines[0])
            .iter()
            .map(|s| s.to_lowercase().trim().to_string())
            .collect();

        let col_map = auto_detect_columns(&headers)?;

        // Parse data lines
        let mut run = DynoRun::new(name, "#4fc3f7");
        run.source_file = Some(path.as_ref().to_string_lossy().to_string());

        for line in &lines[1..] {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let fields = parse_csv_line(line);
            if fields.len() <= col_map.rpm_col {
                continue;
            }

            let rpm: f64 = match fields[col_map.rpm_col].trim().parse() {
                Ok(v) => v,
                Err(_) => continue,
            };

            // Skip unreasonable RPM values
            if rpm < 100.0 || rpm > 20000.0 {
                continue;
            }

            let hp = col_map
                .hp_col
                .and_then(|c| fields.get(c))
                .and_then(|s| s.trim().parse().ok());

            let torque = col_map
                .torque_col
                .and_then(|c| fields.get(c))
                .and_then(|s| s.trim().parse().ok());

            let afr = col_map
                .afr_col
                .and_then(|c| fields.get(c))
                .and_then(|s| s.trim().parse().ok());

            let boost = col_map
                .boost_col
                .and_then(|c| fields.get(c))
                .and_then(|s| s.trim().parse().ok());

            let time = col_map
                .time_col
                .and_then(|c| fields.get(c))
                .and_then(|s| s.trim().parse().ok());

            run.data.push(DynoDataPoint {
                rpm,
                hp,
                torque,
                afr,
                boost,
                time,
            });
        }

        // Sort by RPM
        run.data.sort_by(|a, b| {
            a.rpm
                .partial_cmp(&b.rpm)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Compute peaks
        run.compute_peaks();

        Ok(run)
    }

    /// Parse from pre-mapped CSV with explicit column mapping
    pub fn from_csv_with_map<P: AsRef<Path>>(
        path: P,
        name: impl Into<String>,
        col_map: &DynoColumnMap,
    ) -> io::Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())?;
        let lines: Vec<&str> = content.lines().collect();

        let mut run = DynoRun::new(name, "#4fc3f7");
        run.source_file = Some(path.as_ref().to_string_lossy().to_string());

        // Skip header line
        for line in lines.iter().skip(1) {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let fields = parse_csv_line(line);
            if fields.len() <= col_map.rpm_col {
                continue;
            }

            let rpm: f64 = match fields[col_map.rpm_col].trim().parse() {
                Ok(v) if v >= 100.0 && v <= 20000.0 => v,
                _ => continue,
            };

            run.data.push(DynoDataPoint {
                rpm,
                hp: col_map
                    .hp_col
                    .and_then(|c| fields.get(c))
                    .and_then(|s| s.trim().parse().ok()),
                torque: col_map
                    .torque_col
                    .and_then(|c| fields.get(c))
                    .and_then(|s| s.trim().parse().ok()),
                afr: col_map
                    .afr_col
                    .and_then(|c| fields.get(c))
                    .and_then(|s| s.trim().parse().ok()),
                boost: col_map
                    .boost_col
                    .and_then(|c| fields.get(c))
                    .and_then(|s| s.trim().parse().ok()),
                time: col_map
                    .time_col
                    .and_then(|c| fields.get(c))
                    .and_then(|s| s.trim().parse().ok()),
            });
        }

        run.data.sort_by(|a, b| {
            a.rpm
                .partial_cmp(&b.rpm)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        run.compute_peaks();

        Ok(run)
    }

    /// Compute peak HP and torque from data
    fn compute_peaks(&mut self) {
        self.peak_hp = self
            .data
            .iter()
            .filter_map(|d| d.hp.map(|hp| (hp, d.rpm)))
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        self.peak_torque = self
            .data
            .iter()
            .filter_map(|d| d.torque.map(|tq| (tq, d.rpm)))
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    }

    /// Map dyno data to VE table cells for overlay
    /// x_bins = RPM bins, y_bins = load/MAP bins
    pub fn map_to_table(
        &self,
        x_bins: &[f64],
        y_bins: &[f64],
        boost_to_load: Option<f64>, // conversion factor from boost PSI to load axis units
    ) -> DynoTableOverlay {
        let rows = y_bins.len();
        let cols = x_bins.len();

        // Accumulators for each cell
        let mut hp_sums: Vec<Vec<f64>> = vec![vec![]; rows * cols];
        let mut tq_sums: Vec<Vec<f64>> = vec![vec![]; rows * cols];
        let mut afr_sums: Vec<Vec<f64>> = vec![vec![]; rows * cols];

        for point in &self.data {
            // Find closest RPM column
            let col = find_nearest_bin(x_bins, point.rpm);
            if col >= cols {
                continue;
            }

            // Find row from boost/MAP if available
            let row = if let (Some(boost), Some(factor)) = (point.boost, boost_to_load) {
                let load = boost * factor;
                find_nearest_bin(y_bins, load)
            } else {
                // Default to middle of load range (WOT assumption for dyno)
                rows.saturating_sub(1)
            };
            if row >= rows {
                continue;
            }

            let idx = row * cols + col;
            if let Some(hp) = point.hp {
                hp_sums[idx].push(hp);
            }
            if let Some(tq) = point.torque {
                tq_sums[idx].push(tq);
            }
            if let Some(afr) = point.afr {
                afr_sums[idx].push(afr);
            }
        }

        let mut cell_data = Vec::new();
        for row in 0..rows {
            for col in 0..cols {
                let idx = row * cols + col;
                let count = hp_sums[idx]
                    .len()
                    .max(tq_sums[idx].len())
                    .max(afr_sums[idx].len());
                if count > 0 {
                    cell_data.push(DynoCellOverlay {
                        row,
                        col,
                        hp: if hp_sums[idx].is_empty() {
                            None
                        } else {
                            Some(hp_sums[idx].iter().sum::<f64>() / hp_sums[idx].len() as f64)
                        },
                        torque: if tq_sums[idx].is_empty() {
                            None
                        } else {
                            Some(tq_sums[idx].iter().sum::<f64>() / tq_sums[idx].len() as f64)
                        },
                        afr: if afr_sums[idx].is_empty() {
                            None
                        } else {
                            Some(afr_sums[idx].iter().sum::<f64>() / afr_sums[idx].len() as f64)
                        },
                        sample_count: count,
                    });
                }
            }
        }

        DynoTableOverlay { cell_data }
    }

    /// Get peak-to-peak RPM range
    pub fn rpm_range(&self) -> Option<(f64, f64)> {
        if self.data.is_empty() {
            return None;
        }
        Some((
            self.data.first().unwrap().rpm,
            self.data.last().unwrap().rpm,
        ))
    }

    /// Get data count
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl DynoComparison {
    /// Compare two dyno runs
    pub fn compare(run_a: DynoRun, run_b: DynoRun) -> Self {
        let mut hp_diff = Vec::new();
        let mut torque_diff = Vec::new();

        // Interpolate run_b data at run_a's RPM points
        for point_a in &run_a.data {
            if let Some(hp_a) = point_a.hp {
                if let Some(hp_b) = interpolate_at_rpm(&run_b.data, point_a.rpm, |p| p.hp) {
                    hp_diff.push((point_a.rpm, hp_b - hp_a));
                }
            }
            if let Some(tq_a) = point_a.torque {
                if let Some(tq_b) = interpolate_at_rpm(&run_b.data, point_a.rpm, |p| p.torque) {
                    torque_diff.push((point_a.rpm, tq_b - tq_a));
                }
            }
        }

        let total_hp_change = if hp_diff.is_empty() {
            None
        } else {
            Some(hp_diff.iter().map(|(_, d)| d).sum::<f64>() / hp_diff.len() as f64)
        };

        let total_torque_change = if torque_diff.is_empty() {
            None
        } else {
            Some(torque_diff.iter().map(|(_, d)| d).sum::<f64>() / torque_diff.len() as f64)
        };

        Self {
            run_a,
            run_b,
            hp_diff,
            torque_diff,
            total_hp_change,
            total_torque_change,
        }
    }
}

// ===== Helpers =====

/// Parse a CSV line handling quoted fields
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in line.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' | '\t' if !in_quotes => {
                fields.push(current.clone());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    fields.push(current);
    fields
}

/// Auto-detect CSV columns from header names
fn auto_detect_columns(headers: &[String]) -> io::Result<DynoColumnMap> {
    let mut rpm_col = None;
    let mut hp_col = None;
    let mut torque_col = None;
    let mut afr_col = None;
    let mut boost_col = None;
    let mut time_col = None;

    for (i, header) in headers.iter().enumerate() {
        let h = header.to_lowercase();
        if rpm_col.is_none() && (h.contains("rpm") || h == "speed" || h == "engine speed") {
            rpm_col = Some(i);
        } else if hp_col.is_none()
            && (h.contains("hp")
                || h.contains("horsepower")
                || h.contains("power")
                || h.contains("whp")
                || h.contains("bhp"))
        {
            hp_col = Some(i);
        } else if torque_col.is_none()
            && (h.contains("torque")
                || h.contains("tq")
                || h.contains("ft-lb")
                || h.contains("ftlb")
                || h.contains("nm"))
        {
            torque_col = Some(i);
        } else if afr_col.is_none()
            && (h.contains("afr")
                || h.contains("lambda")
                || h.contains("air fuel")
                || h.contains("a/f"))
        {
            afr_col = Some(i);
        } else if boost_col.is_none()
            && (h.contains("boost") || h.contains("manifold") || h.contains("map"))
        {
            boost_col = Some(i);
        } else if time_col.is_none()
            && (h.contains("time") || h.contains("elapsed") || h.contains("sec"))
        {
            time_col = Some(i);
        }
    }

    let rpm_col = rpm_col.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Could not find RPM column in CSV headers",
        )
    })?;

    Ok(DynoColumnMap {
        rpm_col,
        hp_col,
        torque_col,
        afr_col,
        boost_col,
        time_col,
    })
}

/// Find the nearest bin index for a given value
fn find_nearest_bin(bins: &[f64], value: f64) -> usize {
    if bins.is_empty() {
        return 0;
    }
    let mut best = 0;
    let mut best_dist = (bins[0] - value).abs();
    for (i, &bin) in bins.iter().enumerate().skip(1) {
        let dist = (bin - value).abs();
        if dist < best_dist {
            best_dist = dist;
            best = i;
        }
    }
    best
}

/// Linearly interpolate a value at a given RPM from sorted data points
fn interpolate_at_rpm<F>(data: &[DynoDataPoint], rpm: f64, getter: F) -> Option<f64>
where
    F: Fn(&DynoDataPoint) -> Option<f64>,
{
    if data.is_empty() {
        return None;
    }

    // Find surrounding points
    let mut lower = None;
    let mut upper = None;

    for point in data {
        if let Some(val) = getter(point) {
            if point.rpm <= rpm {
                lower = Some((point.rpm, val));
            }
            if point.rpm >= rpm && upper.is_none() {
                upper = Some((point.rpm, val));
            }
        }
    }

    match (lower, upper) {
        (Some((r1, v1)), Some((r2, v2))) => {
            if (r2 - r1).abs() < 1e-9 {
                Some(v1)
            } else {
                let t = (rpm - r1) / (r2 - r1);
                Some(v1 + t * (v2 - v1))
            }
        }
        (Some((_, v)), None) | (None, Some((_, v))) => Some(v),
        (None, None) => None,
    }
}

/// Detect CSV headers from file content without parsing all data
pub fn detect_csv_headers<P: AsRef<Path>>(path: P) -> io::Result<Vec<String>> {
    let content = std::fs::read_to_string(path)?;
    let first_line = content
        .lines()
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Empty file"))?;
    Ok(parse_csv_line(first_line))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_detect_columns() {
        let headers = vec![
            "time".to_string(),
            "rpm".to_string(),
            "hp".to_string(),
            "torque".to_string(),
            "afr".to_string(),
        ];
        let map = auto_detect_columns(&headers).unwrap();
        assert_eq!(map.rpm_col, 1);
        assert_eq!(map.hp_col, Some(2));
        assert_eq!(map.torque_col, Some(3));
        assert_eq!(map.afr_col, Some(4));
        assert_eq!(map.time_col, Some(0));
    }

    #[test]
    fn test_auto_detect_missing_rpm() {
        let headers = vec!["hp".to_string(), "torque".to_string()];
        assert!(auto_detect_columns(&headers).is_err());
    }

    #[test]
    fn test_find_nearest_bin() {
        let bins = vec![500.0, 1000.0, 1500.0, 2000.0];
        assert_eq!(find_nearest_bin(&bins, 800.0), 1); // closer to 1000
        assert_eq!(find_nearest_bin(&bins, 500.0), 0);
        assert_eq!(find_nearest_bin(&bins, 1999.0), 3);
        assert_eq!(find_nearest_bin(&bins, 1250.0), 1); // equidistant → first match (1000)
    }

    #[test]
    fn test_interpolate_at_rpm() {
        let data = vec![
            DynoDataPoint {
                rpm: 1000.0,
                hp: Some(50.0),
                torque: None,
                afr: None,
                boost: None,
                time: None,
            },
            DynoDataPoint {
                rpm: 2000.0,
                hp: Some(100.0),
                torque: None,
                afr: None,
                boost: None,
                time: None,
            },
            DynoDataPoint {
                rpm: 3000.0,
                hp: Some(150.0),
                torque: None,
                afr: None,
                boost: None,
                time: None,
            },
        ];

        let hp = interpolate_at_rpm(&data, 1500.0, |p| p.hp).unwrap();
        assert!((hp - 75.0).abs() < 1e-9);

        let hp = interpolate_at_rpm(&data, 2500.0, |p| p.hp).unwrap();
        assert!((hp - 125.0).abs() < 1e-9);
    }

    #[test]
    fn test_dyno_run_peaks() {
        let mut run = DynoRun::new("Test Run", "#ff0000");
        run.data = vec![
            DynoDataPoint {
                rpm: 3000.0,
                hp: Some(100.0),
                torque: Some(175.0),
                afr: None,
                boost: None,
                time: None,
            },
            DynoDataPoint {
                rpm: 4000.0,
                hp: Some(150.0),
                torque: Some(197.0),
                afr: None,
                boost: None,
                time: None,
            },
            DynoDataPoint {
                rpm: 5000.0,
                hp: Some(200.0),
                torque: Some(210.0),
                afr: None,
                boost: None,
                time: None,
            },
            DynoDataPoint {
                rpm: 6000.0,
                hp: Some(220.0),
                torque: Some(192.0),
                afr: None,
                boost: None,
                time: None,
            },
        ];
        run.compute_peaks();

        assert_eq!(run.peak_hp, Some((220.0, 6000.0)));
        assert_eq!(run.peak_torque, Some((210.0, 5000.0)));
    }

    #[test]
    fn test_dyno_comparison() {
        let mut run_a = DynoRun::new("Baseline", "#ff0000");
        run_a.data = vec![
            DynoDataPoint {
                rpm: 3000.0,
                hp: Some(100.0),
                torque: Some(175.0),
                afr: None,
                boost: None,
                time: None,
            },
            DynoDataPoint {
                rpm: 5000.0,
                hp: Some(200.0),
                torque: Some(210.0),
                afr: None,
                boost: None,
                time: None,
            },
        ];

        let mut run_b = DynoRun::new("After Tune", "#00ff00");
        run_b.data = vec![
            DynoDataPoint {
                rpm: 3000.0,
                hp: Some(110.0),
                torque: Some(185.0),
                afr: None,
                boost: None,
                time: None,
            },
            DynoDataPoint {
                rpm: 5000.0,
                hp: Some(215.0),
                torque: Some(225.0),
                afr: None,
                boost: None,
                time: None,
            },
        ];

        let cmp = DynoComparison::compare(run_a, run_b);
        assert_eq!(cmp.hp_diff.len(), 2);
        // HP should be +10 at 3000, +15 at 5000
        assert!((cmp.hp_diff[0].1 - 10.0).abs() < 1e-6);
        assert!((cmp.hp_diff[1].1 - 15.0).abs() < 1e-6);
    }

    #[test]
    fn test_map_to_table() {
        let mut run = DynoRun::new("Test", "#ff0000");
        run.data = vec![
            DynoDataPoint {
                rpm: 2000.0,
                hp: Some(50.0),
                torque: Some(131.0),
                afr: Some(14.7),
                boost: None,
                time: None,
            },
            DynoDataPoint {
                rpm: 3000.0,
                hp: Some(100.0),
                torque: Some(175.0),
                afr: Some(13.5),
                boost: None,
                time: None,
            },
            DynoDataPoint {
                rpm: 4000.0,
                hp: Some(150.0),
                torque: Some(197.0),
                afr: Some(12.8),
                boost: None,
                time: None,
            },
        ];

        let x_bins = vec![1000.0, 2000.0, 3000.0, 4000.0, 5000.0];
        let y_bins = vec![20.0, 40.0, 60.0, 80.0, 100.0];

        let overlay = run.map_to_table(&x_bins, &y_bins, None);

        // All samples should map to last row (WOT assumption) when no boost data
        assert!(!overlay.cell_data.is_empty());
        for cell in &overlay.cell_data {
            assert_eq!(cell.row, 4); // Last row (WOT)
        }
    }

    #[test]
    fn test_parse_csv_line() {
        let line = "1000,50.5,\"quoted,value\",14.7";
        let fields = parse_csv_line(line);
        assert_eq!(fields.len(), 4);
        assert_eq!(fields[0], "1000");
        assert_eq!(fields[2], "quoted,value");
    }
}
