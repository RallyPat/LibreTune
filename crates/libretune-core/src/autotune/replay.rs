//! Replay a recorded drive through AutoTune, offline.
//!
//! The engine behind the Log Analyze view, and the thing that makes a tune
//! reviewable without another drive: the same log can be run through several
//! configurations and the results compared, which is impossible live because no
//! two drives present the same samples.
//!
//! It drives the real [`AutoTuneState`], so what comes out is what a live
//! session would have produced from the same data. A separate implementation
//! would only answer questions about itself.
//!
//! # Held-out validation
//!
//! [`ReplayConfig::validate`] scores a proposal against samples it never saw,
//! by k-fold rotation over time blocks. Without it the only measure of a
//! configuration is how large a change it makes, which rewards the configs that
//! chase noise hardest. See [`validate`] for why the scoring is restricted to
//! steady-state samples.

use super::{
    AutoTuneAuthorityLimits, AutoTuneFilters, AutoTuneReferenceTables, AutoTuneSettings,
    AutoTuneState, VEDataPoint, STEADY_LOAD_TOLERANCE, STEADY_RPM_TOLERANCE,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A recorded drive, one array per channel.
///
/// Channel-major rather than a row per sample because that is how the log
/// arrives from the parser and how it is drawn afterwards, and it keeps a
/// 9-channel, 10,000-row log to nine flat arrays rather than 10,000 maps.
///
/// `fuel_cut` and `accel_enrich` may be empty, meaning the log has no such
/// channel — which is different from "the flag was false", and is passed to
/// AutoTune as `None` so its own AFR-rail check takes over.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LogChannels {
    pub time_ms: Vec<f64>,
    pub rpm: Vec<f64>,
    pub load: Vec<f64>,
    pub afr: Vec<f64>,
    /// The ECU's VE at the time. Only used when no VE table is supplied; the
    /// correction itself anchors on the cell.
    #[serde(default)]
    pub ve: Vec<f64>,
    #[serde(default)]
    pub clt: Vec<f64>,
    #[serde(default)]
    pub tps: Vec<f64>,
    #[serde(default)]
    pub tps_rate: Vec<f64>,
    #[serde(default)]
    pub fuel_cut: Vec<f64>,
    #[serde(default)]
    pub accel_enrich: Vec<f64>,
}

impl LogChannels {
    /// Samples present in every required channel.
    ///
    /// The minimum, not the maximum: a log whose columns disagree in length is
    /// truncated to what is actually complete rather than read past the end of
    /// the short one.
    pub fn len(&self) -> usize {
        [
            self.time_ms.len(),
            self.rpm.len(),
            self.load.len(),
            self.afr.len(),
        ]
        .into_iter()
        .min()
        .unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn at(v: &[f64], i: usize, default: f64) -> f64 {
        v.get(i).copied().unwrap_or(default)
    }

    fn flag(v: &[f64], i: usize) -> Option<bool> {
        v.get(i).map(|f| *f > 0.5)
    }

    /// Build the data point for sample `i`.
    pub fn point(&self, i: usize) -> VEDataPoint {
        VEDataPoint {
            rpm: self.rpm[i],
            map: self.load[i],
            load: self.load[i],
            afr: self.afr[i],
            ve: Self::at(&self.ve, i, 0.0),
            // A log with no coolant channel must not be read as stone cold and
            // rejected wholesale; 90 C is a warm engine.
            clt: Self::at(&self.clt, i, 90.0),
            tps: Self::at(&self.tps, i, 0.0),
            tps_rate: Self::at(&self.tps_rate, i, 0.0),
            fuel_cut_active: Self::flag(&self.fuel_cut, i),
            accel_enrich_active: Self::flag(&self.accel_enrich, i),
            timestamp_ms: self.time_ms[i].max(0.0) as u64,
            ..Default::default()
        }
    }
}

/// How to run the replay.
#[derive(Debug, Clone, Deserialize)]
pub struct ReplayConfig {
    pub settings: AutoTuneSettings,
    pub filters: AutoTuneFilters,
    pub authority: AutoTuneAuthorityLimits,
    /// Drop samples with no delayed-buffer match rather than crediting them to
    /// the current (wrong) cell.
    #[serde(default = "default_true")]
    pub strict_lambda_match: bool,
    /// Also score the proposal against samples it never saw.
    #[serde(default = "default_true")]
    pub validate: bool,
}

fn default_true() -> bool {
    true
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            settings: AutoTuneSettings::default(),
            filters: AutoTuneFilters::default(),
            authority: AutoTuneAuthorityLimits::default(),
            strict_lambda_match: true,
            validate: true,
        }
    }
}

/// What one cell of the table came out as.
#[derive(Debug, Clone, Serialize)]
pub struct CellResult {
    pub x: usize,
    pub y: usize,
    pub rpm: f64,
    pub load: f64,
    pub current_ve: f64,
    pub proposed_ve: f64,
    pub delta: f64,
    pub hits: u32,
    /// Accumulated hit weight. Below `base_weight` the change was scaled down
    /// in proportion, so this is what says how much the cell is trusted.
    pub weight: f64,
    /// `weight / base_weight`, capped at 1. How much of the change it wanted
    /// the cell was actually allowed to ask for.
    pub confidence: f64,
    pub target_afr: f64,
    /// Mean measured AFR of the accepted samples that landed here.
    pub mean_afr: f64,
}

/// Why each sample did or did not count.
///
/// One entry per row of the log, so the view can shade a timeline by it. This
/// is the thing a general log viewer cannot show: it needs the tuning filters
/// to know the answer.
#[derive(Debug, Clone, Serialize)]
pub struct SampleVerdict {
    /// `None` when the sample was accepted.
    pub rejected_because: Option<&'static str>,
    /// Cell the sample was attributed to, once accepted.
    pub cell: Option<(usize, usize)>,
}

/// Held-out score. See [`validate`].
#[derive(Debug, Clone, Serialize)]
pub struct ValidationScore {
    /// Percent reduction in mean absolute AFR error on samples not trained on.
    /// Negative means the proposal would have made the mixture worse.
    pub gain_pct: f64,
    /// Percent of held-out samples the proposal moves further from target.
    pub worsened_pct: f64,
    pub scored: usize,
    pub folds: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplayReport {
    pub cells: Vec<CellResult>,
    pub total_samples: u64,
    /// Rejections by reason, largest first.
    pub rejections: Vec<(String, u64)>,
    pub verdicts: Vec<SampleVerdict>,
    pub validation: Option<ValidationScore>,
    /// Accepted-sample count per cell, `[row][col]`, for the coverage map.
    pub coverage: Vec<Vec<u32>>,
}

/// Run `log` through AutoTune and report what it would have proposed.
pub fn replay(
    log: &LogChannels,
    x_bins: &[f64],
    y_bins: &[f64],
    tables: &AutoTuneReferenceTables,
    config: &ReplayConfig,
) -> ReplayReport {
    let (state, verdicts, coverage) = run(log, x_bins, y_bins, tables, config, |_| true);

    // Mean measured AFR of the samples that actually counted, per cell. Taken
    // from the verdicts so it covers exactly the samples AutoTune used — a mean
    // over every sample in the cell would include the ones it refused, and read
    // as disagreeing with its own recommendation.
    let mut afr_sum: HashMap<(usize, usize), (f64, u32)> = HashMap::new();
    for (i, v) in verdicts.iter().enumerate() {
        if let Some(cell) = v.cell {
            let e = afr_sum.entry(cell).or_insert((0.0, 0));
            e.0 += log.afr[i];
            e.1 += 1;
        }
    }
    let mean_afr: HashMap<(usize, usize), f64> = afr_sum
        .into_iter()
        .map(|(k, (sum, n))| (k, sum / f64::from(n.max(1))))
        .collect();

    let mut cells: Vec<CellResult> = state
        .get_recommendations()
        .into_iter()
        .map(|r| {
            // `hit_count`, not the coverage map: coverage bins a sample by the
            // conditions it was read at, while the recommendation was built
            // from samples attributed through the transport delay. The count
            // shown beside a number must be the count that produced it.
            CellResult {
                x: r.cell_x,
                y: r.cell_y,
                rpm: x_bins.get(r.cell_x).copied().unwrap_or(0.0),
                load: y_bins.get(r.cell_y).copied().unwrap_or(0.0),
                current_ve: r.beginning_value,
                proposed_ve: r.recommended_value,
                delta: r.recommended_value - r.beginning_value,
                hits: r.hit_count,
                weight: r.hit_weighting,
                confidence: if config.settings.base_weight > 0.0 {
                    (r.hit_weighting / config.settings.base_weight).min(1.0)
                } else {
                    1.0
                },
                target_afr: r.target_afr,
                mean_afr: mean_afr.get(&(r.cell_x, r.cell_y)).copied().unwrap_or(0.0),
            }
        })
        .collect();
    cells.sort_by_key(|c| (c.y, c.x));

    let mut rejections: Vec<(String, u64)> = state
        .rejection_counts()
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    rejections.sort_by_key(|r| std::cmp::Reverse(r.1));

    ReplayReport {
        validation: config
            .validate
            .then(|| validate(log, x_bins, y_bins, tables, config))
            .flatten(),
        cells,
        total_samples: state.total_samples(),
        rejections,
        verdicts,
        coverage,
    }
}

/// Feed the samples `include` selects into a fresh state.
fn run(
    log: &LogChannels,
    x_bins: &[f64],
    y_bins: &[f64],
    tables: &AutoTuneReferenceTables,
    config: &ReplayConfig,
    include: impl Fn(usize) -> bool,
) -> (AutoTuneState, Vec<SampleVerdict>, Vec<Vec<u32>>) {
    let mut state = AutoTuneState::new();
    state.set_reference_tables(tables.clone());
    state.set_strict_lambda_match(config.strict_lambda_match);
    state.start();

    let mut coverage = vec![vec![0u32; x_bins.len()]; y_bins.len()];
    let mut verdicts = Vec::with_capacity(log.len());

    for i in 0..log.len() {
        if !include(i) {
            verdicts.push(SampleVerdict {
                rejected_because: Some("not in this fold"),
                cell: None,
            });
            continue;
        }
        let point = log.point(i);
        // Ask before adding: `classify` reads the same buffer `add_data_point`
        // is about to push onto, and a sample is not evidence of its own
        // steadiness.
        let verdict = state.classify(&point, &config.filters);
        let cell = verdict
            .is_ok()
            .then(|| (nearest(x_bins, point.rpm), nearest(y_bins, point.load)));
        if let Some((cx, cy)) = cell {
            if let Some(row) = coverage.get_mut(cy) {
                if let Some(c) = row.get_mut(cx) {
                    *c += 1;
                }
            }
        }
        verdicts.push(SampleVerdict {
            rejected_because: verdict.err(),
            cell,
        });

        state.add_data_point(
            point,
            x_bins,
            y_bins,
            &config.settings,
            &config.filters,
            &config.authority,
        );
    }

    (state, verdicts, coverage)
}

/// Time block used to split training from testing, in ms.
///
/// Blocks rather than individual samples: consecutive samples at typical log
/// rates are the same engine state, and splitting those leaks the answer into
/// the test set.
pub const BLOCK_MS: u64 = 10_000;
/// Folds rotated over. Each trains on the other four fifths.
pub const FOLDS: u64 = 5;

/// Score a proposal against samples it never trained on.
///
/// For each held-out sample, ask what the mixture would have been had the
/// proposal been in the table: fuel scales with VE and AFR inversely with fuel,
/// so `afr_after = afr * current / proposed`. Then compare the distance to
/// target before and after. On the training data that would be circular; on
/// held-out samples it is not, because a configuration that chased noise cannot
/// have fitted these.
///
/// Only steady-state samples are scored. Over a window longer than any
/// plausible transport delay the engine was in one cell throughout, so the
/// reading belongs to that cell whichever delay model you believe. Both cruder
/// choices are biased: attributing at the instant of the reading rewards
/// configurations that also ignore delay, and attributing through a delay model
/// rewards the one that shares it. Trying each in turn reversed the ranking of
/// delay compensation between them, which is how this restriction was arrived
/// at.
///
/// Totals are pooled across folds before any ratio is taken, so a fold with 20
/// samples does not weigh the same as one with 800.
pub fn validate(
    log: &LogChannels,
    x_bins: &[f64],
    y_bins: &[f64],
    tables: &AutoTuneReferenceTables,
    config: &ReplayConfig,
) -> Option<ValidationScore> {
    let n = log.len();
    if n == 0 {
        return None;
    }
    let block = |i: usize| (log.time_ms[i].max(0.0) as u64) / BLOCK_MS;
    let steady_flags = steady_mask(log);

    let (mut before, mut after, mut scored, mut worse) = (0.0, 0.0, 0usize, 0usize);

    for fold in 0..FOLDS {
        let (state, _, _) = run(log, x_bins, y_bins, tables, config, |i| {
            block(i) % FOLDS != fold
        });
        let by_cell: HashMap<(usize, usize), (f64, f64)> = state
            .get_recommendations()
            .into_iter()
            .map(|r| {
                (
                    (r.cell_x, r.cell_y),
                    (r.beginning_value, r.recommended_value),
                )
            })
            .collect();

        for (i, &is_steady) in steady_flags.iter().enumerate() {
            if block(i) % FOLDS != fold || !is_steady {
                continue;
            }
            let (rpm, load, afr) = (log.rpm[i], log.load[i], log.afr[i]);
            let (x, y) = (nearest(x_bins, rpm), nearest(y_bins, load));
            let Some(&(current, proposed)) = by_cell.get(&(x, y)) else {
                continue;
            };
            if current <= 0.0 || proposed <= 0.0 {
                continue;
            }
            let target = tables
                .target_afr_table
                .get(y)
                .and_then(|r| r.get(x))
                .copied()
                .filter(|v| *v > 0.1)
                .map_or(config.settings.target_afr, super::normalise_to_afr);

            let e0 = (afr - target).abs();
            let e1 = (afr * current / proposed - target).abs();
            before += e0;
            after += e1;
            scored += 1;
            if e1 > e0 + 1e-9 {
                worse += 1;
            }
        }
    }

    (scored > 0 && before > 0.0).then(|| ValidationScore {
        gain_pct: 100.0 * (before - after) / before,
        worsened_pct: 100.0 * worse as f64 / scored as f64,
        scored,
        folds: FOLDS,
    })
}

/// Comfortably longer than any transport delay in normal use, so a steady
/// window covers the whole exhaust path.
pub const VALIDATION_STEADY_MS: f64 = 800.0;

/// Hard cap on how many samples the backward steadiness walk may inspect.
///
/// The walk is otherwise bounded only by [`VALIDATION_STEADY_MS`] of log time,
/// which a broken or constant time column never satisfies - the walk then runs
/// to index 0 for every sample and the pass becomes O(n²). The rpm/load
/// tolerances usually cut it short, but a long idle or steady-state hold keeps
/// those satisfied too.
///
/// 512 samples is far beyond any real 800 ms window (8 samples at 10 Hz, 80 at
/// 100 Hz), so this changes no verdict on a sane log. On an insane one the walk
/// simply stops without setting `reached_back`, which already means "not
/// evidence of steadiness".
const MAX_STEADY_LOOKBACK: usize = 512;

/// Which samples had rpm and load unchanged for the whole window before them.
///
/// Independent of [`AutoTuneFilters::min_steady_ms`] on purpose: the score must
/// mean the same thing whether or not the configuration being judged filters on
/// steadiness itself.
fn steady_mask(log: &LogChannels) -> Vec<bool> {
    let n = log.len();
    let mut out = vec![false; n];
    for (i, flag) in out.iter_mut().enumerate() {
        let (rpm, load, afr) = (log.rpm[i], log.load[i], log.afr[i]);
        if rpm < 1.0 || !(super::AFR_RAIL_LOW..=super::AFR_RAIL_HIGH).contains(&afr) {
            continue;
        }
        let start = log.time_ms[i] - VALIDATION_STEADY_MS;
        let mut reached_back = false;
        let mut steady = true;
        for j in (i.saturating_sub(MAX_STEADY_LOOKBACK)..=i).rev() {
            if log.time_ms[j] < start {
                reached_back = true;
                break;
            }
            if (log.rpm[j] - rpm).abs() > STEADY_RPM_TOLERANCE
                || (log.load[j] - load).abs() > STEADY_LOAD_TOLERANCE
                || LogChannels::flag(&log.fuel_cut, j) == Some(true)
            {
                steady = false;
                break;
            }
        }
        // Not reaching back far enough is not evidence of steadiness: it is the
        // start of the log, or the far side of a gap.
        *flag = steady && reached_back;
    }
    out
}

fn nearest(bins: &[f64], v: f64) -> usize {
    bins.iter()
        .enumerate()
        .min_by(|a, b| {
            (a.1 - v)
                .abs()
                .partial_cmp(&(b.1 - v).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map_or(0, |(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    const X: [f64; 3] = [2000.0, 3000.0, 4000.0];
    const Y: [f64; 3] = [30.0, 50.0, 70.0];

    /// A drive that sits at one operating point, reading lean against target.
    fn steady_lean_log(n: usize) -> LogChannels {
        LogChannels {
            time_ms: (0..n).map(|i| i as f64 * 100.0).collect(),
            rpm: vec![3000.0; n],
            load: vec![50.0; n],
            afr: vec![14.7; n],
            ve: vec![45.0; n],
            clt: vec![90.0; n],
            tps: vec![20.0; n],
            tps_rate: vec![0.0; n],
            fuel_cut: vec![0.0; n],
            accel_enrich: vec![0.0; n],
        }
    }

    fn tables() -> AutoTuneReferenceTables {
        AutoTuneReferenceTables {
            ve_table: vec![vec![50.0; 3]; 3],
            target_afr_table: vec![vec![14.0; 3]; 3],
            lambda_delay_table: Vec::new(),
        }
    }

    fn config() -> ReplayConfig {
        ReplayConfig {
            filters: AutoTuneFilters {
                min_clt: 71.0,
                ..Default::default()
            },
            strict_lambda_match: false,
            ..Default::default()
        }
    }

    #[test]
    fn a_lean_cell_is_proposed_richer_and_anchored_to_the_table() {
        let report = replay(&steady_lean_log(200), &X, &Y, &tables(), &config());
        let cell = report
            .cells
            .iter()
            .find(|c| (c.x, c.y) == (1, 1))
            .expect("the cell that was driven");
        assert_eq!(
            cell.current_ve, 50.0,
            "anchored on the table, not veCurr 45"
        );
        assert!(cell.delta > 0.0, "lean must add VE, got {}", cell.delta);
        assert!(cell.hits > 0);
    }

    /// The verdict list is what lets the view explain a quiet session.
    #[test]
    fn every_sample_gets_a_verdict_and_rejections_are_named() {
        let mut log = steady_lean_log(50);
        log.clt = vec![20.0; 50]; // stone cold: below min_clt
        let report = replay(&log, &X, &Y, &tables(), &config());

        assert_eq!(report.verdicts.len(), log.len(), "one verdict per sample");
        assert_eq!(report.total_samples, 0, "a cold log teaches nothing");
        assert!(
            report
                .rejections
                .iter()
                .any(|(r, n)| r == "clt below min_clt" && *n > 0),
            "the failing filter must be named, got {:?}",
            report.rejections
        );
    }

    /// Coverage counts only accepted samples, so it shows where the tune
    /// actually has evidence rather than merely where the car went.
    #[test]
    fn coverage_counts_the_cell_that_was_driven() {
        let report = replay(&steady_lean_log(120), &X, &Y, &tables(), &config());
        assert!(report.coverage[1][1] > 0, "the driven cell");
        assert_eq!(report.coverage[0][0], 0, "a cell never visited");
    }

    /// A log too short to fill a fold must say it cannot score, not report zero
    /// as though the proposal were neutral.
    #[test]
    fn validation_is_absent_when_there_is_nothing_to_score() {
        let report = replay(&steady_lean_log(3), &X, &Y, &tables(), &config());
        assert!(report.validation.is_none());
    }

    /// Ragged channels are truncated to what is complete rather than read past
    /// the end of the shortest.
    #[test]
    fn a_short_channel_bounds_the_log() {
        let mut log = steady_lean_log(100);
        log.afr.truncate(40);
        assert_eq!(log.len(), 40);
        let report = replay(&log, &X, &Y, &tables(), &config());
        assert_eq!(report.verdicts.len(), 40);
    }
}

/// Regression for the 2026-09-05 audit finding that `steady_mask`'s backward
/// walk is bounded only by log time.
#[cfg(test)]
mod audit_regression_tests {
    use super::*;

    /// A log whose time channel does not advance never satisfies
    /// `time_ms[j] < start`, so the walk runs to index 0 for every sample and
    /// the whole pass becomes O(n²). The rpm/load tolerances usually cut it
    /// short, but a long steady hold (or an idle) keeps them satisfied too.
    /// Capping the walk by index as well as by time bounds the cost without
    /// changing the verdict: not reaching back far enough is still not
    /// evidence of steadiness.
    #[test]
    fn a_stalled_time_channel_does_not_take_quadratic_time() {
        let n = 20_000;
        let log = LogChannels {
            time_ms: vec![0.0; n],
            rpm: vec![3000.0; n],
            load: vec![50.0; n],
            afr: vec![14.7; n],
            ve: vec![50.0; n],
            clt: vec![90.0; n],
            tps: vec![20.0; n],
            tps_rate: vec![0.0; n],
            fuel_cut: vec![0.0; n],
            accel_enrich: vec![0.0; n],
        };

        let began = std::time::Instant::now();
        let mask = steady_mask(&log);
        let elapsed = began.elapsed();

        assert_eq!(mask.len(), n);
        assert!(
            mask.iter().all(|s| !s),
            "a log that never reaches back 800ms cannot claim steadiness"
        );
        assert!(
            elapsed < std::time::Duration::from_secs(3),
            "the backward walk must be bounded, took {elapsed:?} for {n} samples"
        );
    }

    /// The index cap must sit far above any realistic sample rate, so a normal
    /// log's verdicts are untouched: 800ms holds 8 samples at 10Hz and 80 at
    /// 100Hz, against a cap in the hundreds.
    #[test]
    fn a_normal_log_still_reports_its_steady_stretch() {
        let n = 200;
        let log = LogChannels {
            time_ms: (0..n).map(|i| i as f64 * 100.0).collect(),
            rpm: vec![3000.0; n],
            load: vec![50.0; n],
            afr: vec![14.7; n],
            ve: vec![50.0; n],
            clt: vec![90.0; n],
            tps: vec![20.0; n],
            tps_rate: vec![0.0; n],
            fuel_cut: vec![0.0; n],
            accel_enrich: vec![0.0; n],
        };

        let mask = steady_mask(&log);
        // The first 800ms cannot reach back; everything after it is steady.
        assert!(!mask[0], "the start of the log has no history");
        assert!(mask[100], "a long steady hold must be marked steady");
        assert!(
            mask.iter().filter(|s| **s).count() > n / 2,
            "most of a wholly steady log must be marked steady"
        );
    }
}
