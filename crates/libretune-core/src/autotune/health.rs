//! Tune Health Scoring
//!
//! Evaluates VE table quality across operating regions:
//! - Idle region (low RPM, low load)
//! - Cruise region (mid RPM, mid load)
//! - WOT region (high RPM, high load)
//! - Transition edges
//!
//! Each region is scored 0–100 based on:
//! - Data coverage (% of cells with AutoTune hits)
//! - VE gradient smoothness
//! - AFR target adherence (if recommendations exist)
//! - Monotonicity compliance

use serde::{Deserialize, Serialize};

/// Overall tune health report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuneHealthReport {
    /// Overall score 0–100
    pub overall_score: u32,
    /// Overall grade: A/B/C/D/F
    pub overall_grade: String,
    /// Per-region scores
    pub regions: Vec<RegionHealth>,
    /// Summary recommendations
    pub recommendations: Vec<String>,
    /// Total cells in the table
    pub total_cells: usize,
    /// Cells with AutoTune data
    pub data_coverage_cells: usize,
    /// Coverage percentage
    pub data_coverage_percent: f64,
}

/// Health score for a specific operating region
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionHealth {
    /// Region name (e.g. "Idle", "Cruise", "WOT")
    pub name: String,
    /// Region type
    pub region_type: RegionType,
    /// Overall region score 0–100
    pub score: u32,
    /// Coverage: % of cells with data
    pub coverage_score: u32,
    /// Smoothness: how smooth the VE gradient is
    pub smoothness_score: u32,
    /// Monotonicity: does VE increase with load
    pub monotonicity_score: u32,
    /// Number of cells in this region
    pub cell_count: usize,
    /// Row range (inclusive)
    pub row_range: (usize, usize),
    /// Column range (inclusive)
    pub col_range: (usize, usize),
}

/// Type of operating region
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RegionType {
    Idle,
    PartThrottle,
    Cruise,
    WOT,
    Transition,
}

/// Configuration for the health scorer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthConfig {
    /// RPM boundary between idle and cruise (default: 1500)
    pub idle_rpm_max: f64,
    /// RPM boundary between cruise and WOT (default: 4500)
    pub wot_rpm_min: f64,
    /// Load boundary between part-throttle and WOT (default: 80% of max)
    pub wot_load_fraction: f64,
    /// Load boundary between idle and part-throttle (default: 30% of max)
    pub idle_load_fraction: f64,
    /// Weight for coverage in final score
    pub coverage_weight: f64,
    /// Weight for smoothness in final score
    pub smoothness_weight: f64,
    /// Weight for monotonicity in final score
    pub monotonicity_weight: f64,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            idle_rpm_max: 1500.0,
            wot_rpm_min: 4500.0,
            wot_load_fraction: 0.8,
            idle_load_fraction: 0.3,
            coverage_weight: 0.4,
            smoothness_weight: 0.35,
            monotonicity_weight: 0.25,
        }
    }
}

/// Tune health scorer
pub struct HealthScorer {
    config: HealthConfig,
}

impl HealthScorer {
    pub fn new(config: HealthConfig) -> Self {
        Self { config }
    }

    /// Generate a complete health report for a VE table
    ///
    /// # Arguments
    /// * `table_values` - VE table values (row-major)
    /// * `hit_counts` - AutoTune hit counts per cell (same dimensions)
    /// * `x_bins` - RPM axis bins
    /// * `y_bins` - Load axis bins
    pub fn score_table(
        &self,
        table_values: &[Vec<f64>],
        hit_counts: &[Vec<u32>],
        x_bins: &[f64],
        y_bins: &[f64],
    ) -> TuneHealthReport {
        let rows = table_values.len();
        if rows == 0 || x_bins.is_empty() || y_bins.is_empty() {
            return TuneHealthReport {
                overall_score: 0,
                overall_grade: "F".to_string(),
                regions: Vec::new(),
                recommendations: vec!["No table data available".to_string()],
                total_cells: 0,
                data_coverage_cells: 0,
                data_coverage_percent: 0.0,
            };
        }
        let cols = table_values[0].len();

        // Determine region boundaries
        let regions_def = self.define_regions(x_bins, y_bins, rows, cols);

        // Score each region
        let mut regions: Vec<RegionHealth> = Vec::new();
        for (name, region_type, row_range, col_range) in &regions_def {
            let score = self.score_region(table_values, hit_counts, *row_range, *col_range);
            regions.push(RegionHealth {
                name: name.clone(),
                region_type: region_type.clone(),
                score: score.0,
                coverage_score: score.1,
                smoothness_score: score.2,
                monotonicity_score: score.3,
                cell_count: (row_range.1 - row_range.0 + 1) * (col_range.1 - col_range.0 + 1),
                row_range: *row_range,
                col_range: *col_range,
            });
        }

        // Overall coverage
        let total_cells = rows * cols;
        let data_coverage_cells: usize = hit_counts
            .iter()
            .flat_map(|row| row.iter())
            .filter(|&&h| h > 0)
            .count();
        let data_coverage_percent = if total_cells > 0 {
            data_coverage_cells as f64 / total_cells as f64 * 100.0
        } else {
            0.0
        };

        // Overall score: weighted average of region scores by cell count
        let total_region_cells: usize = regions.iter().map(|r| r.cell_count).sum();
        let overall_score = if total_region_cells > 0 {
            let weighted: f64 = regions
                .iter()
                .map(|r| r.score as f64 * r.cell_count as f64)
                .sum();
            (weighted / total_region_cells as f64) as u32
        } else {
            0
        };

        let overall_grade = Self::score_to_grade(overall_score);

        // Generate recommendations
        let mut recommendations = Vec::new();
        if data_coverage_percent < 50.0 {
            recommendations.push(format!(
                "Low data coverage ({:.0}%) — more driving needed to populate the VE table",
                data_coverage_percent
            ));
        }
        for region in &regions {
            if region.coverage_score < 30 {
                recommendations.push(format!(
                    "{} region has very low data coverage ({}) — focus on driving in this area",
                    region.name, region.coverage_score
                ));
            }
            if region.smoothness_score < 40 {
                recommendations.push(format!(
                    "{} region has rough VE transitions — consider smoothing",
                    region.name
                ));
            }
            if region.monotonicity_score < 50 {
                recommendations.push(format!(
                    "{} region has VE values that decrease with load — check for tuning errors",
                    region.name
                ));
            }
        }

        TuneHealthReport {
            overall_score,
            overall_grade,
            regions,
            recommendations,
            total_cells,
            data_coverage_cells,
            data_coverage_percent,
        }
    }

    /// Define operating regions based on axis bins
    fn define_regions(
        &self,
        x_bins: &[f64],
        y_bins: &[f64],
        rows: usize,
        cols: usize,
    ) -> Vec<(String, RegionType, (usize, usize), (usize, usize))> {
        let mut regions = Vec::new();

        // Find RPM boundary indices
        let idle_col_end = x_bins
            .iter()
            .position(|&x| x > self.config.idle_rpm_max)
            .unwrap_or(1)
            .max(1)
            - 1;
        let wot_col_start = x_bins
            .iter()
            .position(|&x| x >= self.config.wot_rpm_min)
            .unwrap_or(cols.saturating_sub(1));

        // Find load boundary indices
        let max_load = y_bins.last().copied().unwrap_or(100.0);
        let min_load = y_bins.first().copied().unwrap_or(0.0);
        let load_range = max_load - min_load;

        let idle_load_threshold = min_load + load_range * self.config.idle_load_fraction;
        let wot_load_threshold = min_load + load_range * self.config.wot_load_fraction;

        let idle_row_end = y_bins
            .iter()
            .position(|&y| y > idle_load_threshold)
            .unwrap_or(1)
            .max(1)
            - 1;
        let wot_row_start = y_bins
            .iter()
            .position(|&y| y >= wot_load_threshold)
            .unwrap_or(rows.saturating_sub(1));

        // Idle: low RPM, low load
        if idle_col_end > 0 && idle_row_end > 0 {
            regions.push((
                "Idle".to_string(),
                RegionType::Idle,
                (0, idle_row_end.min(rows - 1)),
                (0, idle_col_end.min(cols - 1)),
            ));
        }

        // Cruise: mid RPM, mid load
        let cruise_col_start = (idle_col_end + 1).min(cols - 1);
        let cruise_col_end = wot_col_start.saturating_sub(1).max(cruise_col_start);
        let cruise_row_start = (idle_row_end + 1).min(rows - 1);
        let cruise_row_end = wot_row_start.saturating_sub(1).max(cruise_row_start);

        if cruise_col_end > cruise_col_start && cruise_row_end > cruise_row_start {
            regions.push((
                "Cruise".to_string(),
                RegionType::Cruise,
                (cruise_row_start, cruise_row_end),
                (cruise_col_start, cruise_col_end),
            ));
        }

        // WOT: high RPM and/or high load
        if wot_row_start < rows && wot_col_start < cols {
            regions.push((
                "WOT".to_string(),
                RegionType::WOT,
                (wot_row_start, rows - 1),
                (wot_col_start, cols - 1),
            ));
        }

        // Part Throttle: everything not covered
        let pt_row_start = (idle_row_end + 1).min(rows - 1);
        let pt_row_end = wot_row_start.saturating_sub(1).max(pt_row_start);
        if pt_row_end > pt_row_start {
            regions.push((
                "Part Throttle".to_string(),
                RegionType::PartThrottle,
                (pt_row_start, pt_row_end),
                (0, cols - 1),
            ));
        }

        // Ensure at least one region exists
        if regions.is_empty() {
            regions.push((
                "Full Table".to_string(),
                RegionType::Cruise,
                (0, rows - 1),
                (0, cols - 1),
            ));
        }

        regions
    }

    /// Score a specific rectangular region of the table
    fn score_region(
        &self,
        table_values: &[Vec<f64>],
        hit_counts: &[Vec<u32>],
        row_range: (usize, usize),
        col_range: (usize, usize),
    ) -> (u32, u32, u32, u32) {
        let coverage = self.calc_coverage(hit_counts, row_range, col_range);
        let smoothness = self.calc_smoothness(table_values, row_range, col_range);
        let monotonicity = self.calc_monotonicity(table_values, row_range, col_range);

        let overall = (coverage as f64 * self.config.coverage_weight
            + smoothness as f64 * self.config.smoothness_weight
            + monotonicity as f64 * self.config.monotonicity_weight) as u32;

        (overall.min(100), coverage, smoothness, monotonicity)
    }

    /// Calculate coverage score for a region (0–100)
    fn calc_coverage(
        &self,
        hit_counts: &[Vec<u32>],
        row_range: (usize, usize),
        col_range: (usize, usize),
    ) -> u32 {
        let mut total = 0;
        let mut with_data = 0;

        for r in row_range.0..=row_range.1 {
            for c in col_range.0..=col_range.1 {
                total += 1;
                let hits = hit_counts
                    .get(r)
                    .and_then(|row| row.get(c))
                    .copied()
                    .unwrap_or(0);
                if hits > 0 {
                    with_data += 1;
                }
            }
        }

        if total == 0 {
            return 0;
        }

        ((with_data as f64 / total as f64) * 100.0) as u32
    }

    /// Calculate smoothness score for a region (0–100)
    /// Higher = smoother VE transitions
    fn calc_smoothness(
        &self,
        table_values: &[Vec<f64>],
        row_range: (usize, usize),
        col_range: (usize, usize),
    ) -> u32 {
        let mut gradients = Vec::new();

        for r in row_range.0..=row_range.1 {
            for c in col_range.0..=col_range.1 {
                let val = table_values
                    .get(r)
                    .and_then(|row| row.get(c))
                    .copied()
                    .unwrap_or(0.0);

                // Horizontal gradient
                if c + 1 <= col_range.1 {
                    let next = table_values
                        .get(r)
                        .and_then(|row| row.get(c + 1))
                        .copied()
                        .unwrap_or(val);
                    gradients.push((next - val).abs());
                }
                // Vertical gradient
                if r + 1 <= row_range.1 {
                    let next = table_values
                        .get(r + 1)
                        .and_then(|row| row.get(c))
                        .copied()
                        .unwrap_or(val);
                    gradients.push((next - val).abs());
                }
            }
        }

        if gradients.is_empty() {
            return 100;
        }

        let mean = gradients.iter().sum::<f64>() / gradients.len() as f64;
        let variance =
            gradients.iter().map(|g| (g - mean).powi(2)).sum::<f64>() / gradients.len() as f64;
        let std_dev = variance.sqrt();

        // CV (coefficient of variation) — lower is smoother
        let cv = if mean > 0.01 { std_dev / mean } else { 0.0 };

        // Score: CV of 0 = 100, CV of 2+ = 0
        ((1.0 - cv / 2.0).clamp(0.0, 1.0) * 100.0) as u32
    }

    /// Calculate monotonicity score (0–100)
    /// Checks if VE increases with load (row-wise)
    fn calc_monotonicity(
        &self,
        table_values: &[Vec<f64>],
        row_range: (usize, usize),
        col_range: (usize, usize),
    ) -> u32 {
        let mut total_pairs = 0;
        let mut monotonic_pairs = 0;

        // Check each column: VE should increase with row (increasing load)
        for c in col_range.0..=col_range.1 {
            for r in row_range.0..row_range.1 {
                let curr = table_values
                    .get(r)
                    .and_then(|row| row.get(c))
                    .copied()
                    .unwrap_or(0.0);
                let next = table_values
                    .get(r + 1)
                    .and_then(|row| row.get(c))
                    .copied()
                    .unwrap_or(0.0);

                total_pairs += 1;
                if next >= curr * 0.95 {
                    // Allow 5% tolerance
                    monotonic_pairs += 1;
                }
            }
        }

        if total_pairs == 0 {
            return 100;
        }

        ((monotonic_pairs as f64 / total_pairs as f64) * 100.0) as u32
    }

    /// Convert a numeric score to a letter grade
    fn score_to_grade(score: u32) -> String {
        match score {
            90..=100 => "A".to_string(),
            80..=89 => "B".to_string(),
            70..=79 => "C".to_string(),
            60..=69 => "D".to_string(),
            _ => "F".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_table(rows: usize, cols: usize, fill: f64) -> Vec<Vec<f64>> {
        vec![vec![fill; cols]; rows]
    }

    fn make_hits(rows: usize, cols: usize, fill: u32) -> Vec<Vec<u32>> {
        vec![vec![fill; cols]; rows]
    }

    #[test]
    fn test_perfect_table() {
        let scorer = HealthScorer::new(HealthConfig::default());

        // Well-tuned table: smooth, monotonic, full coverage
        let table = vec![
            vec![30.0, 32.0, 34.0, 36.0, 38.0, 40.0, 42.0, 44.0],
            vec![35.0, 37.0, 39.0, 41.0, 43.0, 45.0, 47.0, 49.0],
            vec![40.0, 42.0, 44.0, 46.0, 48.0, 50.0, 52.0, 54.0],
            vec![45.0, 47.0, 49.0, 51.0, 53.0, 55.0, 57.0, 59.0],
            vec![50.0, 52.0, 54.0, 56.0, 58.0, 60.0, 62.0, 64.0],
            vec![55.0, 57.0, 59.0, 61.0, 63.0, 65.0, 67.0, 69.0],
            vec![60.0, 62.0, 64.0, 66.0, 68.0, 70.0, 72.0, 74.0],
            vec![65.0, 67.0, 69.0, 71.0, 73.0, 75.0, 77.0, 79.0],
        ];
        let hits = make_hits(8, 8, 20); // Full coverage
        let x_bins = vec![
            800.0, 1200.0, 1800.0, 2500.0, 3500.0, 4500.0, 5500.0, 6500.0,
        ];
        let y_bins = vec![20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 100.0];

        let report = scorer.score_table(&table, &hits, &x_bins, &y_bins);
        assert!(
            report.overall_score >= 70,
            "Perfect table should score well, got {}",
            report.overall_score
        );
        assert_eq!(report.data_coverage_percent, 100.0);
    }

    #[test]
    fn test_empty_table() {
        let scorer = HealthScorer::new(HealthConfig::default());
        let report = scorer.score_table(&[], &[], &[], &[]);
        assert_eq!(report.overall_score, 0);
        assert_eq!(report.overall_grade, "F");
    }

    #[test]
    fn test_no_coverage() {
        let scorer = HealthScorer::new(HealthConfig::default());
        let table = make_table(4, 4, 50.0);
        let hits = make_hits(4, 4, 0);
        let x_bins = vec![1000.0, 2000.0, 3000.0, 4000.0];
        let y_bins = vec![20.0, 40.0, 60.0, 80.0];

        let report = scorer.score_table(&table, &hits, &x_bins, &y_bins);
        assert_eq!(report.data_coverage_cells, 0);
        assert_eq!(report.data_coverage_percent, 0.0);
        assert!(report
            .recommendations
            .iter()
            .any(|r| r.contains("coverage")));
    }

    #[test]
    fn test_grade_conversion() {
        assert_eq!(HealthScorer::score_to_grade(95), "A");
        assert_eq!(HealthScorer::score_to_grade(85), "B");
        assert_eq!(HealthScorer::score_to_grade(75), "C");
        assert_eq!(HealthScorer::score_to_grade(65), "D");
        assert_eq!(HealthScorer::score_to_grade(45), "F");
    }

    #[test]
    fn test_monotonicity_violation_lowers_score() {
        let scorer = HealthScorer::new(HealthConfig::default());

        // Table with monotonicity violations
        let table = vec![
            vec![50.0, 52.0, 54.0, 56.0],
            vec![60.0, 62.0, 64.0, 66.0],
            vec![30.0, 32.0, 34.0, 36.0], // Big drop in VE
            vec![70.0, 72.0, 74.0, 76.0],
        ];
        let hits = make_hits(4, 4, 10);
        let x_bins = vec![1000.0, 2000.0, 3000.0, 4000.0];
        let y_bins = vec![20.0, 40.0, 60.0, 80.0];

        let report = scorer.score_table(&table, &hits, &x_bins, &y_bins);

        // Should have lower score due to monotonicity issues
        let has_monotonicity_warning = report
            .recommendations
            .iter()
            .any(|r| r.contains("decrease") || r.contains("monoton"));
        // Score should reflect the monotonicity issue
        assert!(report.overall_score < 95);
    }
}
