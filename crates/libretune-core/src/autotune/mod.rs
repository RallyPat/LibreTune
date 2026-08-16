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
pub mod health;
pub mod predictor;

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
        }
    }
}

/// Authority limits to restrict VE changes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoTuneAuthorityLimits {
    #[serde(alias = "max_change_per_cell")]
    pub max_cell_value_change: f64,
    #[serde(alias = "max_total_change")]
    pub max_cell_percentage_change: f64,
}

impl Default for AutoTuneAuthorityLimits {
    fn default() -> Self {
        Self {
            max_cell_value_change: 10.0,
            max_cell_percentage_change: 20.0,
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
            max_tps_rate: 10.0,         // 10%/sec threshold
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
        }
    }
}

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
            afr: 0.0,
            ve: 0.0,
            clt: 0.0,
            tps: 0.0,
            tps_rate: 0.0,
            accel_enrich_active: None,
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
            Some(&v) if v > 0.1 => v,
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
            .map(|&v| v.max(0.0) as u64)
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

    /// Find the data point from the buffer that best matches the lambda delay
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

        // Only use if within 50ms of target time
        if best_diff < 50 {
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
                    reason = Self::rejection_reason(&point, filters),
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
        current_recs.raw_required_cma = current_recs.raw_required_cma
            + (required_ve - current_recs.raw_required_cma) / current_recs.hit_count as f64;

        let clamped_ve = Self::apply_authority_limits(
            current_recs.beginning_value,
            current_recs.raw_required_cma,
            authority,
        );

        current_recs.recommended_value = clamped_ve;
        // Bug #16: store the actual Target AFR (not the measured AFR).
        current_recs.target_afr = target_afr;

        let hit_weight = 1.0;
        current_recs.hit_weighting += hit_weight;
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

        beginning_value + final_delta
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
    /// An unknown flag (None — e.g. an ECU that publishes no boolean AE
    /// channel) must not reject: treating unknown as active silently discarded
    /// 100% of samples on Speeduino, whose `accelEnrich` channel is a
    /// percentage (100 = no enrichment) and was misread as a boolean.
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
        point.tps_rate = 50.0;
        assert_eq!(
            AutoTuneState::rejection_reason(&point, &filters),
            "tps_rate above max_tps_rate"
        );
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
