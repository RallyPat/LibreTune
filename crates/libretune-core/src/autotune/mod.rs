//! AutoTune Module
//!
//! Implements automatic VE table tuning based on real-time AFR data.
//! Features:
//! - Auto-tuning with recommendations based on AFR data
//! - Authority limits to restrict changes
//! - Data filtering (RPM ranges, coolant temp, custom expressions)
//! - Cell locking functionality
//! - Reference tables (Lambda Delay, AFR Target)
//!
//! AI Analysis submodules:
//! - Predictive cell filling for zero-hit VE table cells
//! - Anomaly detection for identifying suspect data and tune problems
//! - Tune health scoring with per-region quality assessment

pub mod accel_enrich;
pub mod anomaly;
pub mod declared_filters;
pub mod delay_measure;
pub mod health;
pub mod predictor;
pub mod preflight;

use evalexpr::{eval_with_context, ContextWithMutableVariables, HashMapContext, Value};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single cell recommendation in the VE table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTuneRecommendation {
    pub cell_x: usize,
    pub cell_y: usize,
    pub beginning_value: f64,
    pub recommended_value: f64,
    pub hit_count: u32,
    pub hit_weighting: f64,
    pub target_afr: f64,
    pub hit_percentage: f64,
    /// Cumulative moving average of the raw (un-clamped) required VE.
    /// Not serialized to the frontend; used internally so authority clamping
    /// does not pollute the running average (bug #5).
    #[serde(skip)]
    pub raw_required_cma: f64,
}

/// AutoTune settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoTuneSettings {
    pub target_afr: f64,
    pub algorithm: String,
    pub update_rate_ms: u32,
    /// Fixed lambda/AFR transport delay in ms — the lag from a fuelling change
    /// to the wideband seeing it. `0` (default) means "auto": use the per-cell
    /// reference table if present, otherwise the RPM-based curve. A measured
    /// value belongs here because the RPM curve tops out at ~200 ms, far short
    /// of a real exhaust's dead time (this NA6 measures ~990 ms). The
    /// correlation buffer is sized to cover whatever delay is in use.
    ///
    /// When `lambda_delay_flow_scaled` is set, this is instead the delay at the
    /// low-flow (idle/cruise) anchor, and per-cell delays are scaled down from
    /// it toward `lambda_delay_floor_ms` as exhaust flow rises.
    pub lambda_delay_ms: f64,
    /// Build a per-cell delay table scaled by exhaust flow instead of using a
    /// single fixed delay. Transport delay ≈ exhaust-plumbing volume / flow,
    /// and flow ∝ rpm·load·VE, so the delay is long at idle/cruise and short at
    /// high load. `lambda_delay_ms` anchors the low-flow end; the table is
    /// generated at session start from the VE table and populates the same
    /// per-cell `lambda_delay_table` an INI could otherwise supply.
    pub lambda_delay_flow_scaled: bool,
    /// High-flow asymptote (ms) for the flow-scaled table — roughly the
    /// sensor's own response floor, approached as flow rises. Only used when
    /// `lambda_delay_flow_scaled` is set.
    pub lambda_delay_floor_ms: f64,
    /// How much a sample counts toward the cell it lands in.
    pub hit_weighting: HitWeighting,
    /// Accumulated weight at which a cell's recommendation carries full
    /// authority. Below it the proposed change is scaled down in proportion.
    ///
    /// Commonly called `baseWeight`; 20.0 is the usual value
    /// (`VeAnalyzePanel_<table>_baseWeight=20.0` in `project.properties`).
    /// Without it, a cell that has seen one sample proposes as confidently as
    /// one that has seen fifty - which is how a table ends up chasing single
    /// readings. Set to 0 to disable the ramp.
    pub base_weight: f64,
    /// Smallest change worth making, in table units. Commonly called
    /// `minChangeThreshold`, usually 1.0. Stops a cell twitching by a
    /// fraction of a VE point every pass.
    pub min_change: f64,
}

/// How much an accepted sample counts toward its cell's average.
///
/// Samples are attributed to the *nearest* bin, winner-takes-all. That is a
/// reasonable place to put a sample and a poor description of what the ECU did
/// with it: the ECU interpolates between cells, so a sample taken halfway
/// between two rpm bins reflects both of them roughly equally. Counting it as
/// a full vote for one drags that cell toward conditions it never ran at, and
/// the effect is worst exactly where bins are widest — the top of the rpm axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HitWeighting {
    /// Every accepted sample counts fully. What AutoTune has always done, and
    /// the default so existing sessions do not change under anyone.
    #[default]
    Uniform,
    /// Weight by how close the sample sat to the cell's centre, falling to
    /// zero at the neighbouring bin. A sample dead-centre counts 1.0; one
    /// halfway to the next bin counts 0.5.
    ///
    /// This is the honest reading of a sample: it says how much of the
    /// evidence really belongs to this cell rather than the one next door.
    /// Established tools do something of this kind - a `weightThreshold` of 0.1
    /// is only meaningful if samples carry fractional weight.
    CellProximity,
    /// Proximity, but squared: a sample halfway to the neighbour counts 0.25
    /// rather than 0.5.
    ///
    /// For a table whose cells genuinely differ - a peaky VE curve, a sharp
    /// torque step - linear proximity still lets a neighbour's conditions bleed
    /// in noticeably. Squaring concentrates each cell's answer on the samples
    /// actually taken near it, at the cost of needing more of them.
    CellProximitySquared,
    /// Nearest cell only, but a sample more than halfway toward its neighbour
    /// is dropped rather than counted.
    ///
    /// A `weightThreshold` in spirit: refuse the ambiguous samples
    /// instead of weighting them. Cleanest per-cell answer, and the slowest to
    /// fill a map - worth it when a table is being characterised rather than
    /// trimmed.
    CellCentreOnly,
}

impl HitWeighting {
    /// Weight for a sample at (`rpm`, `load`) landing in cell (`x`, `y`).
    ///
    /// Distance is measured against the gap to the *adjacent* bin on the side
    /// the sample fell, not a fixed width, because the axes are not evenly
    /// spaced — a Speeduino rpm axis is dense at idle and coarse at the top.
    /// Using one width would over-weight the wide bins, which is the error
    /// this setting exists to remove.
    pub fn weight(
        self,
        rpm: f64,
        load: f64,
        x: usize,
        y: usize,
        x_bins: &[f64],
        y_bins: &[f64],
    ) -> f64 {
        match self {
            Self::Uniform => 1.0,
            Self::CellProximity => axis_weight(rpm, x, x_bins) * axis_weight(load, y, y_bins),
            Self::CellProximitySquared => {
                let w = axis_weight(rpm, x, x_bins) * axis_weight(load, y, y_bins);
                w * w
            }
            Self::CellCentreOnly => {
                // Both axes must be within half a bin, or the sample belongs as
                // much to a neighbour as to here and is not worth guessing over.
                let wx = axis_weight(rpm, x, x_bins);
                let wy = axis_weight(load, y, y_bins);
                if wx >= 0.5 && wy >= 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }
}

/// Fraction of a sample that belongs to `idx` on one axis: 1.0 at the bin
/// centre, falling linearly to 0.0 at the neighbouring bin.
fn axis_weight(value: f64, idx: usize, bins: &[f64]) -> f64 {
    let Some(&centre) = bins.get(idx) else {
        return 1.0;
    };
    let d = value - centre;
    if d.abs() < f64::EPSILON {
        return 1.0;
    }
    // Gap to the neighbour on the side the sample actually fell.
    let neighbour = if d > 0.0 {
        bins.get(idx + 1).copied()
    } else {
        idx.checked_sub(1).and_then(|i| bins.get(i).copied())
    };
    // At an edge bin there is no neighbour to share with, so the sample is
    // wholly this cell's - clamping instead would silently discard the ends of
    // the map, which are the cells with the least data to spare.
    let Some(n) = neighbour else {
        return 1.0;
    };
    let span = (n - centre).abs();
    if span < f64::EPSILON {
        return 1.0;
    }
    (1.0 - d.abs() / span).clamp(0.0, 1.0)
}

impl Default for AutoTuneSettings {
    fn default() -> Self {
        Self {
            target_afr: 14.7,
            algorithm: "simple".to_string(),
            update_rate_ms: 100,
            lambda_delay_ms: 0.0,
            lambda_delay_flow_scaled: false,
            lambda_delay_floor_ms: 120.0,
            hit_weighting: HitWeighting::default(),
            // The values other popular tuning software uses, so a tuner arriving from
            // one of those sees familiar behaviour rather than rediscovering them.
            base_weight: 20.0,
            min_change: 1.0,
        }
    }
}

/// Authority limits to restrict VE changes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoTuneAuthorityLimits {
    /// Largest single-update change, in **absolute table units** (not percent).
    #[serde(alias = "max_change_per_cell")]
    pub max_cell_value_change: f64,
    /// Largest single-update change as a percentage of the cell's value at the
    /// start of this session.
    #[serde(alias = "max_total_change")]
    pub max_cell_percentage_change: f64,
    /// Absolute floor for any cell value.
    ///
    /// Both clamps above are relative to `beginning_value`, which is re-anchored
    /// to the live table at the first hit of every session (see
    /// `add_data_point`). That makes them per-session allowances: three sessions
    /// of "+20% max" compound to +73%, with nothing to stop the fourth. These
    /// two are the only limits expressed against the table itself rather than
    /// against wherever the last session happened to finish, so they are what
    /// actually bounds a runaway.
    ///
    /// The UI has always sent them. Nothing received them: the struct had no
    /// such fields and `#[serde(default)]` drops unknown keys without a word,
    /// so a tuner who set "min 0 / max 200" got no clamp at all.
    #[serde(alias = "min_value")]
    pub min_cell_value: f64,
    /// Absolute ceiling for any cell value. See [`Self::min_cell_value`].
    #[serde(alias = "max_value")]
    pub max_cell_value: f64,
}

impl AutoTuneAuthorityLimits {
    /// Clamp a proposed cell value to the absolute rails.
    ///
    /// Shared rather than inlined because the authority policy has two
    /// implementations — [`AutoTuneState::apply_authority_limits`] for a live
    /// VE Analyze session and `agent::safety::clamp_table_edit` for agent-driven
    /// edits. Both enforce the same two relative limits; adding the rails to
    /// only one would leave the other able to write past them.
    ///
    /// A reversed pair (min above max) is ordered rather than trusted, because
    /// `f64::clamp` panics when `lo > hi` and this runs per accepted sample.
    pub fn clamp_to_rails(&self, value: f64) -> f64 {
        let lo = self.min_cell_value.min(self.max_cell_value);
        let hi = self.min_cell_value.max(self.max_cell_value);
        value.clamp(lo, hi)
    }
}

impl Default for AutoTuneAuthorityLimits {
    fn default() -> Self {
        Self {
            max_cell_value_change: 10.0,
            max_cell_percentage_change: 20.0,
            // The full range a Speeduino VE byte can represent. A default rail
            // should catch a runaway without ever trimming a legitimate tune —
            // a big turbo VE can pass 200, so defaulting there would silently
            // cap real tuning. Operators who want a tighter rail set one.
            min_cell_value: 0.0,
            max_cell_value: 255.0,
        }
    }
}

/// Data filters for VE Analyze
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoTuneFilters {
    pub min_rpm: f64,
    pub max_rpm: f64,
    pub min_y_axis: Option<String>,
    pub max_y_axis: Option<String>,
    pub min_clt: f64,
    pub custom_filter: Option<String>,
    // Transient filtering
    pub max_tps_rate: f64, // Max TPS change rate (%/sec) before filtering
    pub exclude_accel_enrich: bool, // Exclude data when accel enrichment active
}

impl Default for AutoTuneFilters {
    fn default() -> Self {
        Self {
            min_rpm: 1000.0,
            max_rpm: 7000.0,
            min_y_axis: None,
            max_y_axis: None,
            min_clt: 160.0,
            custom_filter: None,
            // 50 %/s: brisk-but-deliberate throttle use. The old 10 %/s
            // default rejected nearly everything on individual-throttle-body
            // (Alpha-N) engines, whose throttles snap far faster than a
            // single-plenum setup — AutoTune looked dead (issue #132). Genuine
            // accel transients are still caught by `exclude_accel_enrich`.
            max_tps_rate: 50.0,
            exclude_accel_enrich: true, // Exclude accel enrichment by default
        }
    }
}

/// Reference tables used by VE Analyze
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutoTuneReferenceTables {
    /// Per-cell lambda (exhaust transport) delay in ms, indexed `[row][col]`
    /// matching the VE table layout (y=load rows, x=rpm cols). When present,
    /// overrides the simple RPM-based delay curve.
    pub lambda_delay_table: Vec<Vec<f64>>,
    /// Per-cell Target AFR, indexed `[row][col]` matching the VE table.
    /// Used to compute the required VE correction (#1) and populate
    /// `target_afr` on recommendations (#16).
    pub target_afr_table: Vec<Vec<f64>>,
}

/// VE Analyze runtime state
#[derive(Debug)]
pub struct AutoTuneState {
    pub is_running: bool,
    pub locked_cells: Vec<(usize, usize)>,
    pub recommendations: HashMap<(usize, usize), AutoTuneRecommendation>,
    // Lambda delay buffer - stores recent data points for delayed correlation
    data_buffer: std::collections::VecDeque<VEDataPoint>,
    buffer_max_age_ms: u64, // How long to keep data points (default 500ms)
    // Reference tables for the current tuning session. Resolved per-cell
    // Target AFR / lambda delay. Empty by default → callers fall back to
    // settings.target_afr and the RPM-based delay curve.
    reference_tables: AutoTuneReferenceTables,
    // When true, samples with no delayed-buffer match are dropped instead of
    // being attributed to the current (wrong) cell. See bug #2.
    strict_lambda_match: bool,
    // Total number of samples that passed filters (denominator for
    // hit_percentage). See bug #16.
    total_samples: u64,
    // Per-reason counts of samples rejected by the filters. Surface these in
    // the UI (issue #132): a session that accepts nothing looks exactly like
    // a broken one, and the counts say which filter is eating the data
    // (a warm-up CLT below min_clt and a tip-in TPS rate above max_tps_rate
    // are the usual culprits).
    rejected_by_reason: HashMap<&'static str, u64>,
}

impl Default for AutoTuneState {
    fn default() -> Self {
        Self {
            is_running: false,
            locked_cells: Vec::new(),
            recommendations: HashMap::new(),
            data_buffer: std::collections::VecDeque::new(),
            buffer_max_age_ms: 500, // Keep 500ms of data for lambda delay correlation
            reference_tables: AutoTuneReferenceTables::default(),
            strict_lambda_match: true, // Safe default: drop unmatched samples
            total_samples: 0,
            rejected_by_reason: HashMap::new(),
        }
    }
}

/// Normalise a mixture reading to AFR, accepting either AFR or lambda.
///
/// Lambda runs about 0.7-1.2 and AFR about 10-20, so the two ranges do not
/// overlap and a threshold of 2.0 separates them safely.
///
/// This exists because the measured and target sides used to disagree. The
/// realtime path normalised what the sensor reported, while `resolve_target_afr`
/// took the target table's value as-is — so a lambda target table (declared by
/// the INI on any `#if LAMBDA` project) supplied 0.88 to be divided into a
/// measured 13.0. That is a correction factor of 14.8 on every cell of every
/// pass, pinned to whatever the authority ceiling allows and re-anchored higher
/// next session. Both sides call this now so they cannot drift apart again.
pub fn normalise_to_afr(value: f64) -> f64 {
    if value < LAMBDA_AFR_THRESHOLD {
        value * STOICH_AFR
    } else {
        value
    }
}

/// Below this a mixture reading is lambda, above it AFR.
pub const LAMBDA_AFR_THRESHOLD: f64 = 2.0;

/// Stoichiometric AFR for petrol, the lambda -> AFR scale factor.
pub const STOICH_AFR: f64 = 14.7;

/// Wideband readings outside this range are the sensor at a stop rather than
/// a mixture: no running engine sustains them, and a railed reading carries no
/// information about the VE table. Deliberately wider than any tuning target
/// so a genuinely rich or lean cell is still analysed.
pub const AFR_RAIL_LOW: f64 = 10.0;
pub const AFR_RAIL_HIGH: f64 = 19.5;

/// Data point from ECU for VE analysis
#[derive(Debug, Clone)]
pub struct VEDataPoint {
    pub rpm: f64,
    pub map: f64,
    pub maf: f64,
    pub load: f64,
    pub afr: f64,
    pub ve: f64,
    pub clt: f64,
    // Transient detection fields
    pub tps: f64,                          // Current TPS value (%)
    pub tps_rate: f64,                     // TPS change rate (%/sec)
    pub accel_enrich_active: Option<bool>, // ECU accel enrichment flag (if available)
    /// ECU overrun fuel-cut flag (DFCO), when the INI exposes one.
    ///
    /// During a cut the injectors are off, so the wideband reads full lean and
    /// the mixture says nothing about the VE table. `None` means the channel
    /// was not found; the AFR-rail check below still catches most of it.
    pub fuel_cut_active: Option<bool>,
    // Lambda delay correlation
    pub timestamp_ms: u64, // Timestamp for delay correlation
}

impl Default for VEDataPoint {
    fn default() -> Self {
        Self {
            rpm: 0.0,
            map: 0.0,
            maf: 0.0,
            load: 0.0,
            // Stoich, not 0.0: a default point should be a VALID one. Zero
            // AFR is physically impossible and now trips the sensor-rail
            // check, which would make every default-constructed point fail
            // filtering for a reason that has nothing to do with the test.
            afr: 14.7,
            ve: 0.0,
            clt: 0.0,
            tps: 0.0,
            tps_rate: 0.0,
            accel_enrich_active: None,
            fuel_cut_active: None,
            timestamp_ms: 0,
        }
    }
}

impl AutoTuneState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(&mut self) {
        self.is_running = true;
        self.recommendations.clear();
        self.data_buffer.clear();
        self.total_samples = 0;
        self.rejected_by_reason.clear();
    }

    pub fn stop(&mut self) {
        self.is_running = false;
    }

    /// Configure the reference tables (Target AFR / lambda delay) for this
    /// tuning session. Should be called before `start()` or before the first
    /// data point. Empty tables are valid and cause fallback behavior.
    pub fn set_reference_tables(&mut self, tables: AutoTuneReferenceTables) {
        self.reference_tables = tables;
    }

    /// Configure strict lambda-delay matching (bug #2). When true (default),
    /// samples with no buffered historical match are dropped instead of being
    /// attributed to the current cell.
    pub fn set_strict_lambda_match(&mut self, strict: bool) {
        self.strict_lambda_match = strict;
    }

    /// Resolve the Target AFR for a given cell. Uses the per-cell value from
    /// `reference_tables.target_afr_table` when available; otherwise falls back
    /// to `settings.target_afr`. Used for the VE correction formula (#1) and
    /// to populate recommendation `target_afr` (#16).
    ///
    /// Note: recommendations use (cell_x, cell_y) = (col, row), while the
    /// reference table is laid out `[row][col]`, so we index as `[y][x]`.
    fn resolve_target_afr(&self, cell_x: usize, cell_y: usize, fallback: f64) -> f64 {
        match self
            .reference_tables
            .target_afr_table
            .get(cell_y)
            .and_then(|row| row.get(cell_x))
        {
            // Normalised, because the INI may legitimately declare a lambda
            // table as the target (`#if LAMBDA`) while the measured value
            // arriving here has already been converted to AFR.
            Some(&v) if v > 0.1 => normalise_to_afr(v),
            _ => fallback,
        }
    }

    /// Look up the per-cell lambda delay (ms) from the reference table when
    /// present; falls back to `None` so callers use the RPM-based curve.
    fn resolve_lambda_delay_ms(&self, cell_x: usize, cell_y: usize) -> Option<u64> {
        self.reference_tables
            .lambda_delay_table
            .get(cell_y)
            .and_then(|row| row.get(cell_x))
            // A zero cell means "no value here", not "no delay" - the same
            // reading its AFR sibling `resolve_target_afr` already takes. It
            // matters more here: a delay of 0 fails the `delay_ms > 0` check
            // downstream, `historical_point` comes back None, and under strict
            // matching the sample is dropped. That cell then accumulates
            // nothing for the whole session, silently. Returning None instead
            // sends it to the RPM curve, which is at least a number.
            .filter(|&&v| v > 0.1)
            .map(|&v| v as u64)
    }

    pub fn is_cell_locked(&self, x: usize, y: usize) -> bool {
        self.locked_cells.contains(&(x, y))
    }

    pub fn lock_cells(&mut self, cells: Vec<(usize, usize)>) {
        self.locked_cells.extend(cells);
    }

    pub fn unlock_cells(&mut self, cells: Vec<(usize, usize)>) {
        for cell in cells {
            if let Some(pos) = self.locked_cells.iter().position(|c| c == &cell) {
                self.locked_cells.remove(pos);
            }
        }
    }

    /// Calculate lambda sensor delay based on RPM
    /// Higher RPM = faster exhaust flow = less delay
    /// Returns delay in milliseconds
    fn get_lambda_delay_ms(&self, rpm: f64) -> u64 {
        // Default delay curve:
        // - At idle (800 RPM): ~200ms delay
        // - At redline (6000 RPM): ~50ms delay
        // Linear interpolation between these points
        const IDLE_RPM: f64 = 800.0;
        const REDLINE_RPM: f64 = 6000.0;
        const IDLE_DELAY_MS: f64 = 200.0;
        const REDLINE_DELAY_MS: f64 = 50.0;

        let clamped_rpm = rpm.clamp(IDLE_RPM, REDLINE_RPM);
        let rpm_ratio = (clamped_rpm - IDLE_RPM) / (REDLINE_RPM - IDLE_RPM);
        let delay = IDLE_DELAY_MS - (rpm_ratio * (IDLE_DELAY_MS - REDLINE_DELAY_MS));

        delay as u64
    }

    /// Lambda delay to use when no per-cell reference value applies: the
    /// session's configured fixed delay if set (> 0), otherwise the RPM-based
    /// curve. The RPM curve tops out at ~200 ms, so a car whose real transport
    /// delay is longer (this NA6: ~990 ms) must set a fixed value here.
    fn configured_or_curve_delay_ms(&self, settings: &AutoTuneSettings, rpm: f64) -> u64 {
        if settings.lambda_delay_ms > 0.0 {
            settings.lambda_delay_ms as u64
        } else {
            self.get_lambda_delay_ms(rpm)
        }
    }

    /// How much history the correlation buffer must hold: enough to still
    /// contain a sample `delay` ago, plus margin so `find_delayed_data_point`
    /// can bracket the target time. Never shrinks below the original 500 ms.
    /// When nothing is configured (auto/RPM curve), stays at 500 ms exactly so
    /// behaviour is unchanged.
    fn required_buffer_ms(&self, settings: &AutoTuneSettings) -> u64 {
        const DEFAULT_BUFFER_MS: u64 = 500;
        const MARGIN_MS: u64 = 500;
        let table_max = self
            .reference_tables
            .lambda_delay_table
            .iter()
            .flatten()
            .cloned()
            .fold(0.0_f64, f64::max);
        let configured = settings.lambda_delay_ms.max(0.0).max(table_max);
        if configured <= 0.0 {
            DEFAULT_BUFFER_MS
        } else {
            (configured as u64 + MARGIN_MS).max(DEFAULT_BUFFER_MS)
        }
    }
    /// Prune old entries from the data buffer
    fn prune_data_buffer(&mut self, current_timestamp_ms: u64) {
        let cutoff = current_timestamp_ms.saturating_sub(self.buffer_max_age_ms);
        while let Some(front) = self.data_buffer.front() {
            if front.timestamp_ms < cutoff {
                self.data_buffer.pop_front();
            } else {
                break;
            }
        }
    }

    /// Mean spacing between buffered samples, in milliseconds.
    ///
    /// The stream's real cadence, not the configured one: single-shot reads
    /// contend with the realtime poll for the connection lock and land at
    /// 5-9 Hz on hardware, against a nominal 20 Hz.
    fn mean_sample_gap_ms(&self) -> Option<u64> {
        let n = self.data_buffer.len();
        if n < 2 {
            return None;
        }
        let first = self.data_buffer.front()?.timestamp_ms;
        let last = self.data_buffer.back()?.timestamp_ms;
        last.checked_sub(first).map(|span| span / (n as u64 - 1))
    }

    /// Find the data point from the buffer that best matches the lambda delay.
    ///
    /// The match tolerance follows the stream's actual sample spacing instead
    /// of a fixed 50 ms. With samples every T ms the nearest one to any target
    /// inside the buffer is at most T/2 away, so a fixed 50 ms window rejects
    /// good matches as soon as T exceeds ~100 ms — and on this hardware T is
    /// 111-200 ms, so roughly a third to a half of all samples were dropped
    /// with nothing to show for it but a throttled debug line. That is the
    /// "AutoTune runs but recommendations never accumulate" symptom.
    ///
    /// Scaling to the cadence keeps the check meaningful: a genuine gap in the
    /// buffer (a dropout, or a delay reaching past the buffered history) is
    /// still far larger than a sample interval and still rejected.
    fn find_delayed_data_point(
        &self,
        current_timestamp_ms: u64,
        delay_ms: u64,
    ) -> Option<VEDataPoint> {
        let target_time = current_timestamp_ms.saturating_sub(delay_ms);

        // Find the closest data point to the target time
        let mut best_match: Option<&VEDataPoint> = None;
        let mut best_diff = u64::MAX;

        for point in self.data_buffer.iter() {
            let diff = point.timestamp_ms.abs_diff(target_time);

            if diff < best_diff {
                best_diff = diff;
                best_match = Some(point);
            }
        }

        // Half a sample interval is the best any evenly-spaced stream can do;
        // allow a little over that for jitter, and never tighten below the
        // historical 50 ms for fast streams.
        let tolerance = self
            .mean_sample_gap_ms()
            .map(|gap| (gap * 6 / 10).max(50))
            .unwrap_or(50);
        if best_diff <= tolerance {
            best_match.cloned()
        } else {
            None
        }
    }

    /// Number of samples that passed the filters this session — the
    /// denominator behind each cell's hit percentage. Exposed for diagnostics
    /// (e.g. logging how much data a session actually accepted).
    pub fn total_samples(&self) -> u64 {
        self.total_samples
    }

    /// Per-reason counts of filter-rejected samples this session, most
    /// frequent first. Empty when nothing has been rejected. Lets the UI say
    /// *why* a session is not accumulating data instead of looking dead
    /// (issue #132).
    pub fn rejection_counts(&self) -> Vec<(&'static str, u64)> {
        let mut counts: Vec<(&'static str, u64)> = self
            .rejected_by_reason
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect();
        counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        counts
    }

    /// Which filter rejected a sample, for the diagnostic log. Mirrors the
    /// order of the checks in `passes_filters`.
    fn rejection_reason(point: &VEDataPoint, filters: &AutoTuneFilters) -> &'static str {
        if point.rpm < filters.min_rpm || point.rpm > filters.max_rpm {
            return "rpm out of range";
        }
        if point.clt < filters.min_clt {
            return "clt below min_clt";
        }
        let bound = |s: &Option<String>| s.as_deref().and_then(|v| v.trim().parse::<f64>().ok());
        if bound(&filters.min_y_axis).is_some_and(|b| point.load < b) {
            return "load below min_y_axis";
        }
        if bound(&filters.max_y_axis).is_some_and(|b| point.load > b) {
            return "load above max_y_axis";
        }
        if point.fuel_cut_active == Some(true) {
            return "overrun fuel cut";
        }
        if !(AFR_RAIL_LOW..=AFR_RAIL_HIGH).contains(&point.afr) {
            return "afr at sensor rail";
        }
        if point.tps_rate.abs() > filters.max_tps_rate {
            return "tps_rate above max_tps_rate";
        }
        if filters.exclude_accel_enrich && point.accel_enrich_active == Some(true) {
            return "accel enrichment active";
        }
        "custom_filter"
    }

    pub fn add_data_point(
        &mut self,
        point: VEDataPoint,
        table_x_bins: &[f64],
        table_y_bins: &[f64],
        settings: &AutoTuneSettings,
        filters: &AutoTuneFilters,
        authority: &AutoTuneAuthorityLimits,
    ) {
        if !self.is_running {
            return;
        }

        // Size the correlation buffer to cover the delay actually in use, so a
        // delayed AFR sample that old is still present when we look for it.
        // A configured fixed delay or per-cell reference value can far exceed
        // the RPM-curve's ~200 ms max (this NA6 measures ~990 ms); the old
        // fixed 500 ms buffer silently pruned those away, so strict mode
        // dropped every such sample. Must run before pruning below.
        self.buffer_max_age_ms = self.required_buffer_ms(settings);

        // Always add to buffer for lambda delay correlation
        self.data_buffer.push_back(point.clone());
        self.prune_data_buffer(point.timestamp_ms);

        if !self.passes_filters(&point, filters) {
            // Count per reason for the UI's rejection indicator (issue #132):
            // a session that accepts nothing looks exactly like a broken one.
            let reason = Self::rejection_reason(&point, filters);
            *self.rejected_by_reason.entry(reason).or_insert(0) += 1;
            // Throttled so a full drive doesn't flood the log, but enough to
            // show *why* AutoTune "does nothing": compare these against the
            // active filter thresholds (a warm-up CLT below min_clt and a
            // tip-in TPS rate above max_tps_rate are the usual culprits).
            static REJECTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = REJECTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if n < 5 || n.is_multiple_of(100) {
                // Name the specific filter. Previously the line printed only
                // rpm/clt/tps_rate, so a rejection by load bounds or accel
                // enrichment showed every printed value passing while samples
                // still vanished — an hour of misdiagnosis on real hardware.
                tracing::debug!(
                    reason = reason,
                    rpm = point.rpm,
                    clt = point.clt,
                    load = point.load,
                    tps_rate = point.tps_rate,
                    accel_enrich_active = ?point.accel_enrich_active,
                    min_rpm = filters.min_rpm,
                    max_rpm = filters.max_rpm,
                    min_clt = filters.min_clt,
                    min_y_axis = ?filters.min_y_axis,
                    max_y_axis = ?filters.max_y_axis,
                    max_tps_rate = filters.max_tps_rate,
                    exclude_accel_enrich = filters.exclude_accel_enrich,
                    rejected_so_far = n + 1,
                    "AutoTune: sample rejected by filters"
                );
            }
            return;
        }

        // Count every sample that passed filters; used as the denominator for
        // per-cell hit_percentage (#16).
        self.total_samples += 1;

        // Resolve the lambda delay. Prefer the per-cell value from the
        // reference table (bug #14) at the *current* conditions; otherwise use
        // the session's configured fixed delay, or the RPM-based curve.
        let cur_x_idx = self.find_bin_index(point.rpm, table_x_bins);
        let cur_y_idx = self.find_bin_index(point.load, table_y_bins);
        let delay_ms = match (cur_x_idx, cur_y_idx) {
            (Some(cx), Some(cy)) => self
                .resolve_lambda_delay_ms(cx, cy)
                .unwrap_or_else(|| self.configured_or_curve_delay_ms(settings, point.rpm)),
            _ => self.configured_or_curve_delay_ms(settings, point.rpm),
        };

        // Find the data point from when the current AFR reading was actually
        // generated. The current AFR corresponds to conditions from delay_ms
        // ago.
        let historical_point = if delay_ms > 0 && point.timestamp_ms > delay_ms {
            self.find_delayed_data_point(point.timestamp_ms, delay_ms)
        } else {
            None
        };

        // Bug #2: when no delayed match is available, the current (delayed)
        // AFR must NOT be attributed to the current cell — that injects the
        // reading into the wrong load cell during transients. In strict mode
        // (default) we drop the sample entirely.
        let historical_point = match historical_point {
            Some(hp) => hp,
            None => {
                if self.strict_lambda_match {
                    // Strict mode (the default) silently dropped these before,
                    // a common reason accepted-sample counts stay near zero on
                    // an otherwise-valid session. Throttled to stay readable.
                    static NO_DELAY: std::sync::atomic::AtomicU64 =
                        std::sync::atomic::AtomicU64::new(0);
                    let n = NO_DELAY.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if n < 5 || n.is_multiple_of(100) {
                        tracing::debug!(
                            timestamp_ms = point.timestamp_ms,
                            delay_ms,
                            dropped_so_far = n + 1,
                            "AutoTune: sample dropped (strict lambda-delay: no delayed \
                             buffer match at target delay)"
                        );
                    }
                    return;
                }
                tracing::warn!(
                    "AutoTune: no delayed buffer match for sample at {}ms (delay={}ms) — \
                     using inaccurate current-cell fallback",
                    point.timestamp_ms,
                    delay_ms
                );
                // Fall back to current conditions (less accurate).
                point.clone()
            }
        };

        // Attribute the (delayed) AFR reading to the cell the engine was
        // actually in when that exhaust charge was produced.
        let cell_rpm = historical_point.rpm;
        let cell_load = historical_point.load;
        let cell_ve = historical_point.ve;

        let x_idx = self.find_bin_index(cell_rpm, table_x_bins);
        let y_idx = self.find_bin_index(cell_load, table_y_bins);

        if x_idx.is_none() || y_idx.is_none() {
            return;
        }

        let cell_x_idx = x_idx.unwrap();
        let cell_y_idx = y_idx.unwrap();

        if self.is_cell_locked(cell_x_idx, cell_y_idx) {
            return;
        }

        // Resolve the Target AFR for this cell from the reference table,
        // falling back to the global setting (#14, #16).
        let target_afr = self.resolve_target_afr(cell_x_idx, cell_y_idx, settings.target_afr);

        // Required VE correction against the *target* AFR for this cell (#1).
        let required_ve = self.calculate_required_ve(cell_ve, point.afr, target_afr);

        let current_recs = self
            .recommendations
            .entry((cell_x_idx, cell_y_idx))
            .or_insert_with(|| AutoTuneRecommendation {
                cell_x: cell_x_idx,
                cell_y: cell_y_idx,
                beginning_value: cell_ve,
                recommended_value: cell_ve,
                hit_count: 0,
                hit_weighting: 0.0,
                target_afr,
                hit_percentage: 0.0,
                raw_required_cma: cell_ve,
            });

        current_recs.hit_count += 1;

        // Bug #5: maintain a cumulative moving average of the RAW required VE
        // in a dedicated field, so authority clamping does not bias the
        // running average. The clamped result is what gets displayed/applied.
        //
        // Weighted, so a sample that only half-belongs to this cell only half
        // moves it. `hit_weighting` is the running total of weight and is what
        // the incremental mean divides by; under `Uniform` every weight is 1.0
        // and this reduces exactly to the old count-based average.
        let hit_weight = settings.hit_weighting.weight(
            cell_rpm,
            cell_load,
            cell_x_idx,
            cell_y_idx,
            table_x_bins,
            table_y_bins,
        );
        current_recs.hit_weighting += hit_weight;
        let w_total = current_recs.hit_weighting.max(f64::MIN_POSITIVE);
        current_recs.raw_required_cma = current_recs.raw_required_cma
            + (required_ve - current_recs.raw_required_cma) * (hit_weight / w_total);

        // Confidence ramp: a cell that has barely been visited proposes
        // proportionally less of the change it thinks it wants. Without this a
        // single sample carries the same authority as fifty, and the sparse
        // cells - which are the high-load, high-rpm ones the engine passes
        // through briefly - are exactly where a wrong answer costs most.
        let confidence = if settings.base_weight > 0.0 {
            (current_recs.hit_weighting / settings.base_weight).min(1.0)
        } else {
            1.0
        };
        let ramped = current_recs.beginning_value
            + (current_recs.raw_required_cma - current_recs.beginning_value) * confidence;

        let clamped_ve =
            Self::apply_authority_limits(current_recs.beginning_value, ramped, authority);

        // Below the change threshold, leave the cell alone rather than
        // proposing a fraction of a VE point that will only be rounded away.
        let clamped_ve = if (clamped_ve - current_recs.beginning_value).abs() < settings.min_change
        {
            current_recs.beginning_value
        } else {
            clamped_ve
        };

        current_recs.recommended_value = clamped_ve;
        // Bug #16: store the actual Target AFR (not the measured AFR).
        current_recs.target_afr = target_afr;

        // Bug #16: realistic hit percentage based on total filtered samples.
        current_recs.hit_percentage = if self.total_samples > 0 {
            (current_recs.hit_count as f64 / self.total_samples as f64) * 100.0
        } else {
            0.0
        };

        // Periodic proof-of-life: shows AutoTune is accepting samples and how
        // the current cell's recommendation is evolving. Capturing these ends
        // the `current_recs` borrow so `self` can be read for the summary.
        let (rec_begin, rec_value, rec_hits) = (
            current_recs.beginning_value,
            current_recs.recommended_value,
            current_recs.hit_count,
        );
        if self.total_samples.is_multiple_of(50) {
            tracing::debug!(
                total_accepted = self.total_samples,
                cells_touched = self.recommendations.len(),
                cell_x = cell_x_idx,
                cell_y = cell_y_idx,
                begin_ve = rec_begin,
                recommended_ve = rec_value,
                cell_hits = rec_hits,
                "AutoTune: accepted sample"
            );
        }
    }

    /// Apply authority limits to clamp the recommended VE change
    fn apply_authority_limits(
        beginning_value: f64,
        recommended_value: f64,
        authority: &AutoTuneAuthorityLimits,
    ) -> f64 {
        let delta = recommended_value - beginning_value;

        // Clamp by absolute value change
        let clamped_delta = delta.clamp(
            -authority.max_cell_value_change,
            authority.max_cell_value_change,
        );

        // Clamp by percentage change
        let max_pct_delta = beginning_value * (authority.max_cell_percentage_change / 100.0);
        let final_delta = clamped_delta.clamp(-max_pct_delta, max_pct_delta);

        // Absolute rails last, so they bound the result no matter what the two
        // relative clamps allowed. Both of those are measured from
        // `beginning_value`, which each session re-reads from the live table —
        // so on their own they permit unlimited drift one session at a time.
        authority.clamp_to_rails(beginning_value + final_delta)
    }

    fn find_bin_index(&self, value: f64, bins: &[f64]) -> Option<usize> {
        if bins.is_empty() {
            return None;
        }

        if let Some((i, _)) = bins
            .iter()
            .enumerate()
            .find(|&(_, bin)| (bin - value).abs() < 0.1)
        {
            return Some(i);
        }

        bins.iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let da = (*a - value).abs();
                let db = (*b - value).abs();
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
    }

    fn evaluate_custom_filter(&self, expr: &str, point: &VEDataPoint) -> Result<bool, String> {
        let mut ctx = HashMapContext::new();

        let set_value = |ctx: &mut HashMapContext, name: &str, value: Value| {
            ctx.set_value(name.to_string(), value)
                .map_err(|e| format!("Failed to set {name}: {e}"))
        };

        set_value(&mut ctx, "rpm", point.rpm.into())?;
        set_value(&mut ctx, "map", point.map.into())?;
        set_value(&mut ctx, "maf", point.maf.into())?;
        set_value(&mut ctx, "load", point.load.into())?;
        set_value(&mut ctx, "afr", point.afr.into())?;
        set_value(&mut ctx, "ve", point.ve.into())?;
        set_value(&mut ctx, "clt", point.clt.into())?;
        set_value(&mut ctx, "tps", point.tps.into())?;
        set_value(&mut ctx, "tps_rate", point.tps_rate.into())?;

        let accel_enrich = point.accel_enrich_active.unwrap_or(false);
        set_value(&mut ctx, "accel_enrich", accel_enrich.into())?;
        set_value(&mut ctx, "accel_enrich_active", accel_enrich.into())?;

        match eval_with_context(expr, &ctx) {
            Ok(Value::Boolean(val)) => Ok(val),
            Ok(Value::Int(val)) => Ok(val != 0),
            Ok(Value::Float(val)) => Ok(val != 0.0),
            Ok(other) => Err(format!(
                "Custom filter must return boolean or number, got {other:?}"
            )),
            Err(e) => Err(format!("Custom filter eval error: {e}")),
        }
    }

    pub fn passes_filters(&self, point: &VEDataPoint, filters: &AutoTuneFilters) -> bool {
        // Basic RPM and CLT filters
        if point.rpm < filters.min_rpm || point.rpm > filters.max_rpm {
            return false;
        }
        if point.clt < filters.min_clt {
            return false;
        }

        // Bug #15: enforce Y-axis (load) bounds. min_y_axis / max_y_axis are
        // stored as Option<String>; parse a leading numeric value (e.g. "40").
        // More complex expressions should go through custom_filter instead.
        if let Some(bound) = filters
            .min_y_axis
            .as_deref()
            .and_then(|s| s.trim().parse::<f64>().ok())
        {
            if point.load < bound {
                return false;
            }
        }
        if let Some(bound) = filters
            .max_y_axis
            .as_deref()
            .and_then(|s| s.trim().parse::<f64>().ok())
        {
            if point.load > bound {
                return false;
            }
        }

        // Overrun fuel cut: injectors off, so the wideband reads full lean and
        // the reading carries no VE information. Left in, these samples land in
        // exactly the low-load cells the car passes through on every lift, and
        // each one asks for the maximum enrichment the authority limit allows —
        // a required-VE of ve * (21/14.7) is +43%, clamped to +20%, over and
        // over. Established VE analysers exclude overrun for the same reason.
        if point.fuel_cut_active == Some(true) {
            return false;
        }

        // A railed wideband is not a measurement. Below ~10 or above ~19.5 AFR
        // no engine is running a real mixture: it is the sensor at a stop, an
        // unpowered heater, or a cut still washing out. Catches the fuel-cut
        // case too when the ECU exposes no DFCO channel.
        if !(AFR_RAIL_LOW..=AFR_RAIL_HIGH).contains(&point.afr) {
            return false;
        }

        // Transient filtering: reject if TPS is changing too fast
        if point.tps_rate.abs() > filters.max_tps_rate {
            return false;
        }

        // Transient filtering: reject if accel enrichment is active (if flag available)
        if filters.exclude_accel_enrich {
            if let Some(true) = point.accel_enrich_active {
                return false;
            }
        }

        if let Some(ref expr) = filters.custom_filter {
            let trimmed = expr.trim();
            if !trimmed.is_empty() {
                match self.evaluate_custom_filter(trimmed, point) {
                    Ok(true) => {}
                    Ok(false) => return false,
                    Err(e) => {
                        tracing::warn!("AutoTune custom filter rejected data: {e}");
                        return false;
                    }
                }
            }
        }

        true
    }

    fn calculate_required_ve(&self, current_ve: f64, actual_afr: f64, target_afr: f64) -> f64 {
        // Bug #1: compute the required VE from the measured AFR relative to the
        // cell's Target AFR, NOT against a hardcoded stoichiometric ratio.
        //
        //   Required VE = Current VE × (Actual AFR / Target AFR)
        //
        // If the measured AFR is leaner than target (Actual > Target) the
        // cylinder got too much air for the fuel delivered, so VE must rise;
        // if richer (Actual < Target) VE must fall.
        if actual_afr < 0.1 || target_afr < 0.1 {
            return current_ve;
        }

        current_ve * (actual_afr / target_afr)
    }

    pub fn get_recommendations(&self) -> Vec<AutoTuneRecommendation> {
        self.recommendations.values().cloned().collect()
    }
}

/// The VE table a session is proposing: current values with every accepted
/// recommendation applied, and everything else left alone.
///
/// Kept separate from sending or burning so the result can be written to a file
/// and applied later. A session's work currently lives only in memory, so
/// closing the app throws away a whole drive's worth of collection.
///
/// Cells outside the table are skipped rather than panicking: a recommendation
/// set can outlive the table it was computed against - a project reload, a
/// different definition - and losing an entire export to one stale index would
/// be a poor trade.
pub fn proposed_ve_table(
    current: &[Vec<f64>],
    recommendations: &[AutoTuneRecommendation],
) -> Vec<Vec<f64>> {
    let mut out = current.to_vec();
    for r in recommendations {
        if let Some(cell) = out.get_mut(r.cell_y).and_then(|row| row.get_mut(r.cell_x)) {
            *cell = r.recommended_value;
        }
    }
    out
}

/// How many cells a proposal would actually change, and the largest delta.
///
/// The count matters because a recommendation equal to the current value is not
/// a change - `min_change` and the confidence ramp both produce those
/// deliberately - so a UI counting recommendations rather than changes
/// overstates what a session achieved.
pub fn proposal_summary(
    current: &[Vec<f64>],
    recommendations: &[AutoTuneRecommendation],
) -> (usize, f64) {
    let mut changed = 0usize;
    let mut largest = 0.0f64;
    for r in recommendations {
        let Some(&now) = current.get(r.cell_y).and_then(|row| row.get(r.cell_x)) else {
            continue;
        };
        let d = r.recommended_value - now;
        if d.abs() > f64::EPSILON {
            changed += 1;
            if d.abs() > largest.abs() {
                largest = d;
            }
        }
    }
    (changed, largest)
}

/// Build a per-cell lambda-delay table scaled by exhaust flow.
///
/// Transport delay ≈ exhaust-plumbing volume ÷ exhaust volumetric flow, and
/// flow ∝ rpm·load·VE (the air actually burned, speed-density). So each cell's
/// delay = `floor + K/flow`, anchored so a nominal warm idle / light-cruise
/// point takes `idle_delay_ms` (where the cruise logs actually constrain it)
/// and high-flow cells fall toward `floor_ms` (the sensor's own response).
///
/// `ve_table` is indexed `[load_row][rpm_col]` matching `y_bins`×`x_bins`; the
/// returned table matches, ready for [`AutoTuneState::set_reference_tables`].
/// Idle is treated as the modelled maximum (cells below the anchor flow clamp
/// to `idle_delay_ms`).
pub fn compute_flow_scaled_delay_table(
    ve_table: &[Vec<f64>],
    x_bins: &[f64],
    y_bins: &[f64],
    idle_delay_ms: f64,
    floor_ms: f64,
) -> Vec<Vec<f64>> {
    // Warm-idle / light-cruise anchor: where cruise logs give the delay a real
    // measurement. The value only sets which cell equals idle_delay_ms; the
    // shape is 1/flow either way.
    const ANCHOR_RPM: f64 = 800.0;
    const ANCHOR_LOAD: f64 = 40.0;

    let flow = |rpm: f64, load: f64, ve: f64| rpm.max(1.0) * load.max(1.0) * ve.max(1.0);
    let nearest = |v: f64, b: &[f64]| {
        b.iter()
            .enumerate()
            .min_by(|(_, a), (_, c)| {
                (**a - v)
                    .abs()
                    .partial_cmp(&(**c - v).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(0)
    };

    let ax = nearest(ANCHOR_RPM, x_bins);
    let ay = nearest(ANCHOR_LOAD, y_bins);
    let anchor_ve = ve_table
        .get(ay)
        .and_then(|r| r.get(ax))
        .copied()
        .unwrap_or(50.0);
    let anchor_flow = flow(
        x_bins.get(ax).copied().unwrap_or(ANCHOR_RPM),
        y_bins.get(ay).copied().unwrap_or(ANCHOR_LOAD),
        anchor_ve,
    );
    let idle = idle_delay_ms.max(floor_ms);
    let k = (idle - floor_ms) * anchor_flow;

    y_bins
        .iter()
        .enumerate()
        .map(|(j, &load)| {
            x_bins
                .iter()
                .enumerate()
                .map(|(i, &rpm)| {
                    let ve = ve_table
                        .get(j)
                        .and_then(|r| r.get(i))
                        .copied()
                        .unwrap_or(anchor_ve);
                    (floor_ms + k / flow(rpm, load, ve)).clamp(floor_ms, idle)
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]
    use super::*;

    /// The accel-enrich filter must only reject when the flag is known-true.
    /// During overrun the injectors are off: the wideband reads full lean
    /// while rpm, load, CLT and a steady closed throttle all pass. Left in,
    /// each such sample asks for the authority-limit maximum enrichment in
    /// exactly the low-load cells the car crosses on every lift.
    #[test]
    fn fuel_cut_samples_are_rejected() {
        let state = AutoTuneState::default();
        let filters = AutoTuneFilters {
            min_rpm: 0.0,
            min_clt: -100.0,
            max_tps_rate: 1000.0,
            ..Default::default()
        };
        let mut point = VEDataPoint::default();
        point.rpm = 2500.0;
        point.clt = 80.0;
        point.afr = 14.7;

        point.fuel_cut_active = Some(true);
        assert!(!state.passes_filters(&point, &filters), "a cut must reject");
        assert_eq!(
            AutoTuneState::rejection_reason(&point, &filters),
            "overrun fuel cut"
        );

        point.fuel_cut_active = Some(false);
        assert!(state.passes_filters(&point, &filters), "not cutting passes");

        point.fuel_cut_active = None;
        assert!(
            state.passes_filters(&point, &filters),
            "unknown must pass — many INIs expose no DFCO channel"
        );
    }

    /// A railed sensor is not a mixture. This also covers the fuel-cut case on
    /// ECUs that expose no DFCO channel at all.
    #[test]
    fn railed_wideband_readings_are_rejected() {
        let state = AutoTuneState::default();
        let filters = AutoTuneFilters {
            min_rpm: 0.0,
            min_clt: -100.0,
            max_tps_rate: 1000.0,
            ..Default::default()
        };
        let mut point = VEDataPoint::default();
        point.rpm = 2500.0;
        point.clt = 80.0;

        for railed in [9.9, 10.0 - 0.01, 19.6, 22.0, 0.0] {
            point.afr = railed;
            assert!(
                !state.passes_filters(&point, &filters),
                "{railed} AFR is a rail, not a measurement"
            );
        }
        for real in [11.5, 12.7, 14.7, 16.0, 19.4] {
            point.afr = real;
            assert!(
                state.passes_filters(&point, &filters),
                "{real} AFR is a usable mixture"
            );
        }
    }

    /// The match window must follow the stream's real cadence. At the 111-200
    /// ms spacing single-shot reads actually achieve, a fixed 50 ms window
    /// rejects samples that are as close as an evenly-spaced stream can ever
    /// get — half an interval — which silently discarded a third to a half of
    /// every session.
    #[test]
    fn delayed_match_tolerates_the_streams_real_sample_spacing() {
        let mut state = AutoTuneState::default();
        state.buffer_max_age_ms = 5_000;
        // 150 ms spacing: the nearest sample to any target is up to 75 ms away,
        // which the old fixed 50 ms window rejected outright.
        for i in 0..20u64 {
            let mut p = VEDataPoint::default();
            p.timestamp_ms = i * 150;
            p.rpm = 2000.0;
            state.data_buffer.push_back(p);
        }
        assert_eq!(state.mean_sample_gap_ms(), Some(150));

        // Worst case for evenly-spaced sampling: a target exactly BETWEEN two
        // samples, 75 ms from each. No stream at this cadence can ever do
        // better, yet the old fixed 50 ms window rejected it. Delay 975 ms
        // from the newest sample (t=2850) targets t=1875, midway between the
        // samples at 1800 and 1950.
        assert!(
            state.find_delayed_data_point(2850, 975).is_some(),
            "a target midway between samples is the best this cadence can do              and must match"
        );

        // A genuine hole is still rejected. Rebuild the buffer with a dropout
        // in the middle and aim at it: the nearest sample is then many
        // intervals away, not half of one.
        state.data_buffer.clear();
        for t in (0..900u64).step_by(150).chain((2400..2900u64).step_by(150)) {
            let mut p = VEDataPoint::default();
            p.timestamp_ms = t;
            p.rpm = 2000.0;
            state.data_buffer.push_back(p);
        }
        // Target t=1650, sitting inside the 750-2400 ms dropout.
        assert!(
            state.find_delayed_data_point(2850, 1200).is_none(),
            "a target inside a dropout must not match"
        );
    }

    /// The accel-enrich filter must only reject when the flag is known-true.
    /// An unknown flag (None — e.g. an ECU that publishes no boolean AE
    /// channel) must not reject: treating unknown as active silently discarded
    /// 100% of samples on Speeduino, whose `accelEnrich` channel is a
    /// percentage (100 = no enrichment) and was misread as a boolean.
    /// (The `#[test]` for this function was previously stranded on the wrong
    /// doc comment above `fuel_cut_samples_are_rejected`, so this test never
    /// ran.)
    #[test]
    fn accel_filter_rejects_only_known_active() {
        let state = AutoTuneState::default();
        let filters = AutoTuneFilters {
            min_rpm: 0.0,
            min_clt: -100.0,
            max_tps_rate: 1000.0,
            exclude_accel_enrich: true,
            ..Default::default()
        };
        let mut point = VEDataPoint::default();
        point.rpm = 2000.0;
        point.clt = 80.0;

        point.accel_enrich_active = Some(true);
        assert!(
            !state.passes_filters(&point, &filters),
            "known-active must reject"
        );
        assert_eq!(
            AutoTuneState::rejection_reason(&point, &filters),
            "accel enrichment active"
        );

        point.accel_enrich_active = Some(false);
        assert!(
            state.passes_filters(&point, &filters),
            "known-inactive must pass"
        );

        point.accel_enrich_active = None;
        assert!(state.passes_filters(&point, &filters), "unknown must pass");
    }

    /// The rejection log's reason must name the filter that actually fired —
    /// rpm/clt looking fine while samples vanish cost an hour on real hardware.
    #[test]
    fn rejection_reason_names_the_failing_filter() {
        let filters = AutoTuneFilters::default(); // min_rpm 1000, min_clt 160
        let mut point = VEDataPoint::default();

        point.rpm = 500.0;
        assert_eq!(
            AutoTuneState::rejection_reason(&point, &filters),
            "rpm out of range"
        );

        point.rpm = 2000.0;
        point.clt = 100.0;
        assert_eq!(
            AutoTuneState::rejection_reason(&point, &filters),
            "clt below min_clt"
        );

        point.clt = 180.0;
        point.tps_rate = 80.0;
        assert_eq!(
            AutoTuneState::rejection_reason(&point, &filters),
            "tps_rate above max_tps_rate"
        );
    }

    /// The default transient threshold must stay ITB-friendly: the old
    /// 10 %/s rejected nearly every sample on individual-throttle-body
    /// (Alpha-N) engines, whose throttles snap far faster than a
    /// single-plenum setup — AutoTune looked dead (issue #132). A point at
    /// 50 %/s (deliberate-but-brisk throttle use) must pass by default.
    #[test]
    fn default_tps_rate_is_itb_friendly() {
        assert_eq!(AutoTuneFilters::default().max_tps_rate, 50.0);

        let mut point = VEDataPoint::default();
        point.rpm = 2000.0;
        point.clt = 180.0;
        point.tps_rate = 50.0;
        assert!(
            AutoTuneState::default().passes_filters(&point, &AutoTuneFilters::default()),
            "50 %/s throttle movement must not be rejected by default"
        );
    }

    /// Rejected samples must be counted per reason so the UI can say *why* a
    /// session is not accumulating data instead of looking dead (issue #132).
    #[test]
    fn rejected_samples_are_counted_per_reason() {
        let mut state = AutoTuneState::default();
        state.start();

        let filters = AutoTuneFilters {
            min_rpm: 1000.0,
            min_clt: 160.0,
            max_tps_rate: 50.0,
            ..Default::default()
        };

        let mut cold = VEDataPoint::default();
        cold.rpm = 2000.0;
        cold.clt = 100.0; // below min_clt

        let mut fast_throttle = VEDataPoint::default();
        fast_throttle.rpm = 2000.0;
        fast_throttle.clt = 180.0;
        fast_throttle.tps_rate = 80.0; // above max_tps_rate

        let mut good = VEDataPoint::default();
        good.rpm = 2000.0;
        good.clt = 180.0;
        good.afr = 14.7;
        good.ve = 50.0;

        // Bins for a 1-cell table; attribution is not what's under test.
        let bins_x = [1000.0];
        let bins_y = [50.0];
        let settings = AutoTuneSettings::default();
        let authority = AutoTuneAuthorityLimits::default();

        state.add_data_point(
            cold.clone(),
            &bins_x,
            &bins_y,
            &settings,
            &filters,
            &authority,
        );
        state.add_data_point(cold, &bins_x, &bins_y, &settings, &filters, &authority);
        state.add_data_point(
            fast_throttle,
            &bins_x,
            &bins_y,
            &settings,
            &filters,
            &authority,
        );
        state.add_data_point(good, &bins_x, &bins_y, &settings, &filters, &authority);

        let counts = state.rejection_counts();
        assert_eq!(
            counts[0],
            ("clt below min_clt", 2),
            "sorted most frequent first"
        );
        assert_eq!(counts[1], ("tps_rate above max_tps_rate", 1));
        assert_eq!(state.total_samples(), 1, "accepted sample is counted too");

        // Restarting the session clears the tallies.
        state.start();
        assert!(state.rejection_counts().is_empty());
        assert_eq!(state.total_samples(), 0);
    }

    /// Captures `tracing` event messages into a shared Vec so a test can assert
    /// that a specific diagnostic actually fired (the whole point of D9: these
    /// drop paths used to be silent).
    #[derive(Clone, Default)]
    struct LogCapture(std::sync::Arc<std::sync::Mutex<Vec<String>>>);
    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for LogCapture {
        fn on_event(&self, e: &tracing::Event<'_>, _c: tracing_subscriber::layer::Context<'_, S>) {
            struct V(std::sync::Arc<std::sync::Mutex<Vec<String>>>);
            impl tracing::field::Visit for V {
                fn record_debug(&mut self, f: &tracing::field::Field, v: &dyn std::fmt::Debug) {
                    if f.name() == "message" {
                        self.0.lock().unwrap().push(format!("{v:?}"));
                    }
                }
            }
            e.record(&mut V(self.0.clone()));
        }
    }

    #[test]
    fn filter_rejected_sample_is_logged_not_silent() {
        use tracing_subscriber::layer::SubscriberExt;
        let cap = LogCapture::default();
        let logs = cap.0.clone();
        let sub = tracing_subscriber::registry().with(cap);
        tracing::subscriber::with_default(sub, || {
            let mut st = AutoTuneState::new();
            st.start();
            let s = AutoTuneSettings::default();
            let f = AutoTuneFilters::default(); // min_clt = 160
            let a = AutoTuneAuthorityLimits::default();
            // clt=20 is far below min_clt: the sample must be rejected AND logged
            // (before D9 this path returned silently). The reject log is
            // throttled by a process-global counter, so feed a full throttle
            // window (100+) to guarantee at least one line regardless of what
            // other tests already put on that counter -- otherwise this flakes
            // depending on test order.
            let p = VEDataPoint {
                rpm: 2000.0,
                load: 50.0,
                afr: 14.0,
                ve: 50.0,
                clt: 20.0,
                tps: 5.0,
                tps_rate: 0.0,
                timestamp_ms: 1000,
                ..Default::default()
            };
            for _ in 0..101 {
                st.add_data_point(p.clone(), &[1000.0, 2000.0], &[40.0, 80.0], &s, &f, &a);
            }
        });
        assert!(
            logs.lock()
                .unwrap()
                .iter()
                .any(|m| m.contains("rejected by filters")),
            "a filtered-out sample must emit a diagnostic, not drop silently"
        );
    }

    #[test]
    fn auto_delay_leaves_buffer_at_default_500ms() {
        // lambda_delay_ms = 0 (auto) must keep the historical 500 ms window,
        // so existing behaviour is unchanged when nothing is configured.
        let state = AutoTuneState::new();
        let settings = AutoTuneSettings::default();
        assert_eq!(settings.lambda_delay_ms, 0.0);
        assert_eq!(state.required_buffer_ms(&settings), 500);
    }

    #[test]
    fn configured_delay_extends_buffer_beyond_500ms() {
        // A configured 900 ms delay must size the buffer past the old fixed
        // 500 ms cap, and the fixed value (not the RPM curve) must be used.
        let mut state = AutoTuneState::new();
        state.start();
        let mut settings = AutoTuneSettings::default();
        settings.lambda_delay_ms = 900.0;
        let filters = AutoTuneFilters::default();
        let authority = AutoTuneAuthorityLimits::default();
        let x = vec![1000.0, 2000.0];
        let y = vec![50.0, 100.0];

        assert_eq!(state.required_buffer_ms(&settings), 1400); // 900 + 500 margin
        assert_eq!(state.configured_or_curve_delay_ms(&settings, 1500.0), 900);

        // Feed samples spanning 0..1000 ms; the oldest (t=0) must survive,
        // whereas the old 500 ms buffer would have pruned everything before 500.
        for ts in (0..=1000).step_by(100) {
            let p = VEDataPoint {
                rpm: 1500.0,
                load: 75.0,
                afr: 14.0,
                ve: 50.0,
                clt: 80.0,
                tps: 10.0,
                tps_rate: 0.0,
                timestamp_ms: ts,
                ..Default::default()
            };
            state.add_data_point(p, &x, &y, &settings, &filters, &authority);
        }
        assert_eq!(
            state.data_buffer.front().map(|p| p.timestamp_ms),
            Some(0),
            "buffer must retain samples ~900 ms old for a 900 ms configured delay"
        );
        assert!(state.buffer_max_age_ms >= 900);
    }

    #[test]
    fn flow_scaled_delay_table_shortens_with_flow() {
        // 3x3 grid, VE flat at 50. Delay must fall as rpm·load (flow) rises,
        // hit idle_delay at the low-flow anchor, and never breach the bounds.
        let x = vec![800.0, 3000.0, 6000.0]; // rpm
        let y = vec![40.0, 70.0, 100.0]; // load
        let ve = vec![vec![50.0; 3]; 3];
        let idle = 1050.0;
        let floor = 120.0;
        let t = compute_flow_scaled_delay_table(&ve, &x, &y, idle, floor);

        // Low-flow corner (anchor rpm=800, load=40) is the longest.
        assert!(
            (t[0][0] - idle).abs() < 1.0,
            "anchor cell should equal idle delay"
        );
        // High-flow corner (6000 rpm, 100 load) is the shortest, near the floor.
        assert!(t[2][2] < t[0][0], "high flow must be shorter than idle");
        assert!(t[2][2] >= floor - 0.001 && t[2][2] < 400.0);
        // Monotonic along rising rpm at fixed load, and rising load at fixed rpm.
        assert!(t[0][0] > t[0][1] && t[0][1] > t[0][2]);
        assert!(t[0][0] > t[1][0] && t[1][0] > t[2][0]);
        // Bounds hold everywhere.
        for row in &t {
            for &d in row {
                assert!((floor - 0.001..=idle + 0.001).contains(&d));
            }
        }
    }

    #[test]
    fn custom_filter_allows_matching_point() {
        let state = AutoTuneState::default();
        let mut filters = AutoTuneFilters::default();
        filters.custom_filter = Some("rpm > 2000 && tps < 50 && clt > 70".to_string());
        filters.min_clt = 70.0;

        let point = VEDataPoint {
            rpm: 2500.0,
            tps: 25.0,
            clt: 85.0,
            ..VEDataPoint::default()
        };

        assert!(state.passes_filters(&point, &filters));
    }

    #[test]
    fn custom_filter_rejects_non_matching_point() {
        let state = AutoTuneState::default();
        let mut filters = AutoTuneFilters::default();
        filters.custom_filter = Some("rpm > 3000 && afr < 13.5".to_string());

        let point = VEDataPoint {
            rpm: 2500.0,
            afr: 14.7,
            ..VEDataPoint::default()
        };

        assert!(!state.passes_filters(&point, &filters));
    }

    #[test]
    fn custom_filter_invalid_expression_rejects_point() {
        let state = AutoTuneState::default();
        let mut filters = AutoTuneFilters::default();
        filters.custom_filter = Some("rpm >".to_string());

        let point = VEDataPoint {
            rpm: 2500.0,
            ..VEDataPoint::default()
        };

        assert!(!state.passes_filters(&point, &filters));
    }

    /// Regression test for issue #132: on an Alpha-N / ITB tune the VE table's
    /// load (Y) axis is throttle position, not manifold pressure. The caller
    /// (realtime_stream) must therefore set `VEDataPoint.load = tps` so samples
    /// are attributed to the correct cell. This test fixes the contract: a
    /// point with `load = 75` against TPS bins `[0, 25, 50, 75, 100]` lands in
    /// Y cell 3, regardless of `map`/`maf` (which are irrelevant for Alpha-N).
    #[test]
    fn tps_load_axis_attributes_to_correct_cell() {
        let mut state = AutoTuneState::new();
        // Disable strict lambda-delay matching: the point of this test is the
        // load-axis attribution, not exhaust-transport correlation. With strict
        // off, the current cell is used as the fallback when no delayed match
        // exists.
        state.set_strict_lambda_match(false);
        state.start();

        let settings = AutoTuneSettings::default(); // lambda_delay_ms = 0 (auto curve)
        let mut filters = AutoTuneFilters::default();
        filters.min_clt = 0.0; // accept the sample regardless of warm-up state
        let authority = AutoTuneAuthorityLimits::default();

        // X = rpm bins, Y = throttle-position bins (0–100 %), as an Alpha-N
        // VE table would define them.
        let x_bins = vec![1000.0, 2000.0, 3000.0, 4000.0];
        let y_bins = vec![0.0, 25.0, 50.0, 75.0, 100.0];

        // Engine at 3000 rpm, 75 % throttle. MAP is meaningless for Alpha-N
        // (set to 0 to prove it isn't used); `load` carries the TPS value.
        let point = VEDataPoint {
            rpm: 3000.0,
            map: 0.0,   // intentionally zero — Alpha-N ignores MAP
            maf: 0.0,   // no MAF either
            load: 75.0, // <- the throttle value the caller put here
            afr: 14.7,
            ve: 80.0,
            clt: 85.0,
            tps: 75.0,
            tps_rate: 0.0,
            accel_enrich_active: Some(false),
            timestamp_ms: 1000,
            ..Default::default()
        };

        state.add_data_point(point, &x_bins, &y_bins, &settings, &filters, &authority);

        let recs = state.get_recommendations();
        assert_eq!(recs.len(), 1, "exactly one cell should have been hit");

        let r = &recs[0];
        // rpm 3000 -> X bin index 2 ; throttle 75 % -> Y bin index 3.
        assert_eq!(r.cell_x, 2, "rpm 3000 should map to X bin 2");
        assert_eq!(r.cell_y, 3, "75%% throttle should map to Y bin 3");
        assert!(r.hit_count >= 1);
    }
}

#[cfg(test)]
mod authority_rail_tests {
    use super::*;

    fn limits(max_val: f64, max_pct: f64, lo: f64, hi: f64) -> AutoTuneAuthorityLimits {
        AutoTuneAuthorityLimits {
            max_cell_value_change: max_val,
            max_cell_percentage_change: max_pct,
            min_cell_value: lo,
            max_cell_value: hi,
        }
    }

    /// The UI has always sent `min_value`/`max_value`. Before these fields
    /// existed, serde dropped them and the rails did nothing at all.
    #[test]
    fn the_ui_payload_field_names_actually_bind() {
        let json = r#"{
            "max_change_per_cell": 15.0,
            "max_total_change": 30.0,
            "min_value": 40.0,
            "max_value": 120.0
        }"#;
        let a: AutoTuneAuthorityLimits = serde_json::from_str(json).expect("UI payload parses");
        assert_eq!(a.max_cell_value_change, 15.0);
        assert_eq!(a.max_cell_percentage_change, 30.0);
        assert_eq!(a.min_cell_value, 40.0, "min_value must reach the backend");
        assert_eq!(a.max_cell_value, 120.0, "max_value must reach the backend");
    }

    #[test]
    fn the_ceiling_bounds_a_single_update() {
        // Relative clamps would allow 100 -> 120; the rail stops it at 110.
        let a = limits(50.0, 50.0, 0.0, 110.0);
        assert_eq!(
            AutoTuneState::apply_authority_limits(100.0, 120.0, &a),
            110.0
        );
    }

    #[test]
    fn the_floor_bounds_a_single_update() {
        let a = limits(50.0, 50.0, 80.0, 255.0);
        assert_eq!(AutoTuneState::apply_authority_limits(100.0, 60.0, &a), 80.0);
    }

    /// The real failure the rails exist for: each session re-anchors
    /// `beginning_value` to the live table, so a percentage allowance renews
    /// itself indefinitely. Without an absolute rail this compounds forever.
    #[test]
    fn repeated_sessions_cannot_compound_past_the_rail() {
        let unrailed = limits(1000.0, 20.0, 0.0, f64::INFINITY);
        let mut ve = 100.0;
        for _ in 0..10 {
            // Each session asks for far more than allowed and is clamped to
            // +20% of wherever the last one finished.
            ve = AutoTuneState::apply_authority_limits(ve, ve * 10.0, &unrailed);
        }
        assert!(
            ve > 600.0,
            "without a rail, ten sessions of +20% compound unbounded; got {ve}"
        );

        let railed = limits(1000.0, 20.0, 0.0, 130.0);
        let mut ve = 100.0;
        for _ in 0..10 {
            ve = AutoTuneState::apply_authority_limits(ve, ve * 10.0, &railed);
        }
        assert_eq!(ve, 130.0, "the rail must stop the compounding");
    }

    #[test]
    fn a_reversed_min_max_pair_does_not_panic() {
        // f64::clamp panics when lo > hi. A misconfigured pair should be inert,
        // not fatal, in a loop that runs per accepted sample on a live engine.
        let a = limits(50.0, 50.0, 200.0, 50.0);
        let out = AutoTuneState::apply_authority_limits(100.0, 120.0, &a);
        assert!(out.is_finite());
        assert!((50.0..=200.0).contains(&out));
    }

    #[test]
    fn the_default_rail_never_trims_a_legitimate_tune() {
        // A big-turbo VE can pass 200; a default that clipped there would
        // silently cap real tuning rather than catch a runaway.
        let a = AutoTuneAuthorityLimits::default();
        assert_eq!(a.min_cell_value, 0.0);
        assert!(
            a.max_cell_value >= 255.0,
            "default ceiling must cover the full byte range, got {}",
            a.max_cell_value
        );
    }
}

#[cfg(test)]
mod hit_weighting_tests {
    use super::*;

    // A Speeduino rpm axis: dense at idle, coarse at the top. The uneven
    // spacing is the whole reason weight is measured against the neighbouring
    // bin rather than a fixed width.
    const RPM: [f64; 5] = [800.0, 1200.0, 2000.0, 4000.0, 6500.0];
    const LOAD: [f64; 4] = [30.0, 50.0, 70.0, 100.0];

    #[test]
    fn uniform_counts_every_sample_fully() {
        let w = HitWeighting::Uniform;
        assert_eq!(w.weight(1234.0, 41.0, 1, 0, &RPM, &LOAD), 1.0);
        assert_eq!(w.weight(6499.0, 99.0, 4, 3, &RPM, &LOAD), 1.0);
    }

    #[test]
    fn a_sample_on_the_bin_centre_counts_fully() {
        let w = HitWeighting::CellProximity;
        assert!((w.weight(2000.0, 50.0, 2, 1, &RPM, &LOAD) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_sample_halfway_to_the_neighbour_counts_half() {
        let w = HitWeighting::CellProximity;
        // 3000 rpm is halfway from bin 2 (2000) to bin 3 (4000); load on centre.
        let got = w.weight(3000.0, 50.0, 2, 1, &RPM, &LOAD);
        assert!((got - 0.5).abs() < 1e-9, "expected 0.5, got {got}");
    }

    /// The point of measuring against the neighbour rather than a fixed width:
    /// the same rpm offset means very different things at the two ends.
    #[test]
    fn the_same_offset_weighs_differently_on_a_wide_bin() {
        let w = HitWeighting::CellProximity;
        // +200 rpm off a narrow bin (800->1200, span 400) is half a bin.
        let narrow = w.weight(1000.0, 50.0, 0, 1, &RPM, &LOAD);
        // +200 rpm off a wide bin (4000->6500, span 2500) is a twelfth.
        let wide = w.weight(4200.0, 50.0, 3, 1, &RPM, &LOAD);
        assert!((narrow - 0.5).abs() < 1e-9, "narrow: {narrow}");
        assert!(wide > 0.9, "wide bin should barely discount: {wide}");
        assert!(wide > narrow);
    }

    #[test]
    fn an_edge_bin_keeps_the_whole_sample() {
        let w = HitWeighting::CellProximity;
        // Below the first rpm bin there is no neighbour to share with. Docking
        // it would quietly starve the ends of the map, which have least data.
        assert_eq!(w.weight(600.0, 50.0, 0, 1, &RPM, &LOAD), 1.0);
        assert_eq!(w.weight(7000.0, 50.0, 4, 1, &RPM, &LOAD), 1.0);
    }

    #[test]
    fn both_axes_multiply() {
        let w = HitWeighting::CellProximity;
        // halfway on rpm (0.5) and halfway on load (0.5) -> 0.25
        let got = w.weight(3000.0, 60.0, 2, 1, &RPM, &LOAD);
        assert!((got - 0.25).abs() < 1e-9, "expected 0.25, got {got}");
    }

    /// The weight must reach the AVERAGE, not just accumulate as a counter.
    /// A weighted incremental mean has to reproduce the plain mean when every
    /// weight is 1.0, or switching to Uniform would silently change results.
    #[test]
    fn the_weighted_mean_reduces_to_the_plain_mean_under_uniform() {
        let samples: [f64; 4] = [90.0, 100.0, 110.0, 95.0];
        let (mut cma, mut wtot) = (samples[0], 0.0);
        cma = samples[0];
        for (i, x) in samples.iter().enumerate() {
            let w = 1.0;
            wtot += w;
            if i == 0 {
                cma = *x;
                continue;
            }
            cma += (x - cma) * (w / wtot);
        }
        // Plain incremental mean over the same series, seeded the same way.
        let (mut plain, mut n) = (samples[0], 1.0);
        for x in samples.iter().skip(1) {
            n += 1.0;
            plain += (x - plain) / n;
        }
        assert!(
            (cma - plain).abs() < 1e-9,
            "weighted {cma} vs plain {plain}"
        );
    }

    #[test]
    fn a_half_weight_sample_moves_the_average_half_as_far() {
        // Start at 100 with one full-weight sample, then add 200.
        let full = {
            let (mut cma, mut w) = (100.0_f64, 1.0_f64);
            w += 1.0;
            cma += (200.0 - cma) * (1.0 / w);
            cma
        };
        let half = {
            let (mut cma, mut w) = (100.0_f64, 1.0_f64);
            w += 0.5;
            cma += (200.0 - cma) * (0.5 / w);
            cma
        };
        assert!((full - 150.0).abs() < 1e-9, "full: {full}");
        assert!(half < full, "a half-weight sample must move it less");
        assert!((half - 133.3333).abs() < 0.01, "half: {half}");
    }
}

#[cfg(test)]
mod confidence_ramp_tests {
    use super::*;

    fn settings(base: f64, min_change: f64) -> AutoTuneSettings {
        AutoTuneSettings {
            base_weight: base,
            min_change,
            ..Default::default()
        }
    }

    /// The ramp is the piece established tools have and a plain average does not: a
    /// cell that has seen one sample must not propose as confidently as one
    /// that has seen fifty.
    #[test]
    fn a_sparse_cell_proposes_only_part_of_the_change() {
        let s = settings(20.0, 0.0);
        // Wants 100 -> 120, but only 5 of the 20 weight needed: a quarter.
        let confidence = (5.0_f64 / s.base_weight).min(1.0);
        let ramped = 100.0 + (120.0 - 100.0) * confidence;
        assert!((ramped - 105.0).abs() < 1e-9, "got {ramped}");
    }

    #[test]
    fn a_well_sampled_cell_proposes_all_of_it() {
        let s = settings(20.0, 0.0);
        let confidence = (25.0_f64 / s.base_weight).min(1.0);
        assert_eq!(
            confidence, 1.0,
            "past base_weight the ramp must not exceed 1"
        );
        let ramped = 100.0 + (120.0 - 100.0) * confidence;
        assert!((ramped - 120.0).abs() < 1e-9);
    }

    #[test]
    fn a_zero_base_weight_disables_the_ramp() {
        let s = settings(0.0, 0.0);
        let confidence = if s.base_weight > 0.0 {
            (1.0_f64 / s.base_weight).min(1.0)
        } else {
            1.0
        };
        assert_eq!(confidence, 1.0, "0 must mean 'off', not 'divide by zero'");
    }

    #[test]
    fn a_change_below_the_threshold_is_not_proposed() {
        let s = settings(0.0, 1.0);
        let begin = 100.0_f64;
        for proposed in [100.4_f64, 99.7, 100.0] {
            let out = if (proposed - begin).abs() < s.min_change {
                begin
            } else {
                proposed
            };
            assert_eq!(
                out, begin,
                "{proposed} is within the threshold, leave the cell alone"
            );
        }
        let big = 102.0_f64;
        let out = if (big - begin).abs() < s.min_change {
            begin
        } else {
            big
        };
        assert_eq!(out, big, "a real change must still pass");
    }

    #[test]
    fn the_shipped_defaults_match_convention() {
        let d = AutoTuneSettings::default();
        assert_eq!(d.base_weight, 20.0, "conventional baseWeight is 20.0");
        assert_eq!(d.min_change, 1.0, "conventional minChangeThreshold is 1.0");
    }
}

#[cfg(test)]
mod proposal_export_tests {
    use super::*;

    fn rec(x: usize, y: usize, begin: f64, value: f64) -> AutoTuneRecommendation {
        AutoTuneRecommendation {
            cell_x: x,
            cell_y: y,
            beginning_value: begin,
            recommended_value: value,
            hit_count: 10,
            hit_weighting: 10.0,
            target_afr: 12.8,
            hit_percentage: 5.0,
            raw_required_cma: value,
        }
    }

    fn table() -> Vec<Vec<f64>> {
        vec![vec![40.0, 50.0], vec![90.0, 96.0]]
    }

    #[test]
    fn only_recommended_cells_move() {
        let t = table();
        let out = proposed_ve_table(&t, &[rec(1, 1, 96.0, 102.0)]);
        assert_eq!(out[1][1], 102.0, "the recommended cell takes its new value");
        assert_eq!(out[0][0], 40.0, "everything else is untouched");
        assert_eq!(out[1][0], 90.0);
        assert_eq!(out[0][1], 50.0);
    }

    #[test]
    fn a_stale_index_is_skipped_not_fatal() {
        // A recommendation set can outlive the table it was computed against.
        let t = table();
        let out = proposed_ve_table(&t, &[rec(9, 9, 1.0, 2.0), rec(0, 0, 40.0, 44.0)]);
        assert_eq!(out[0][0], 44.0, "the valid one still applies");
        assert_eq!(out.len(), 2, "the table keeps its shape");
    }

    #[test]
    fn the_summary_counts_changes_not_recommendations() {
        let t = table();
        // One real change, one recommendation that equals what is already there
        // (which min_change and the confidence ramp both produce on purpose).
        let recs = [rec(0, 0, 40.0, 40.0), rec(1, 1, 96.0, 102.0)];
        let (changed, largest) = proposal_summary(&t, &recs);
        assert_eq!(changed, 1, "a no-op recommendation is not a change");
        assert!((largest - 6.0).abs() < 1e-9, "largest delta: {largest}");
    }

    #[test]
    fn the_largest_delta_keeps_its_sign() {
        let t = table();
        let recs = [rec(0, 0, 40.0, 38.0), rec(1, 1, 96.0, 97.0)];
        let (_, largest) = proposal_summary(&t, &recs);
        assert!(
            largest < 0.0,
            "a fuel cut must not be reported as a gain: {largest}"
        );
        assert!((largest + 2.0).abs() < 1e-9);
    }

    #[test]
    fn an_empty_proposal_changes_nothing() {
        let t = table();
        assert_eq!(proposed_ve_table(&t, &[]), t);
        assert_eq!(proposal_summary(&t, &[]), (0, 0.0));
    }
}

#[cfg(test)]
mod weighting_approach_tests {
    use super::*;

    const RPM: [f64; 5] = [800.0, 1200.0, 2000.0, 4000.0, 6500.0];
    const LOAD: [f64; 4] = [30.0, 50.0, 70.0, 100.0];

    /// The four approaches must actually differ, or offering a choice is a lie.
    #[test]
    fn the_approaches_rank_as_described() {
        // 3000 rpm is halfway from bin 2 (2000) to bin 3 (4000); load on centre.
        let at = |w: HitWeighting| w.weight(3000.0, 50.0, 2, 1, &RPM, &LOAD);
        assert_eq!(at(HitWeighting::Uniform), 1.0);
        assert!((at(HitWeighting::CellProximity) - 0.5).abs() < 1e-9);
        assert!((at(HitWeighting::CellProximitySquared) - 0.25).abs() < 1e-9);
        // exactly halfway still counts under centre-only; past it does not
        assert_eq!(at(HitWeighting::CellCentreOnly), 1.0);
    }

    #[test]
    fn centre_only_drops_a_sample_past_the_halfway_point() {
        let w = HitWeighting::CellCentreOnly;
        // 3400 of the way from 2000 to 4000 is 70% - too far to claim.
        assert_eq!(w.weight(3400.0, 50.0, 2, 1, &RPM, &LOAD), 0.0);
        assert_eq!(w.weight(2200.0, 50.0, 2, 1, &RPM, &LOAD), 1.0);
    }

    #[test]
    fn squared_is_always_at_or_below_linear() {
        let lin = HitWeighting::CellProximity;
        let sq = HitWeighting::CellProximitySquared;
        for rpm in [2100.0, 2500.0, 3000.0, 3600.0, 3900.0] {
            let a = lin.weight(rpm, 50.0, 2, 1, &RPM, &LOAD);
            let b = sq.weight(rpm, 50.0, 2, 1, &RPM, &LOAD);
            assert!(b <= a + 1e-12, "at {rpm}: squared {b} exceeded linear {a}");
        }
    }

    #[test]
    fn every_approach_counts_a_dead_centre_sample_fully() {
        for w in [
            HitWeighting::Uniform,
            HitWeighting::CellProximity,
            HitWeighting::CellProximitySquared,
            HitWeighting::CellCentreOnly,
        ] {
            assert!(
                (w.weight(2000.0, 50.0, 2, 1, &RPM, &LOAD) - 1.0).abs() < 1e-9,
                "{w:?} discounted a sample taken exactly at the cell centre"
            );
        }
    }
}
