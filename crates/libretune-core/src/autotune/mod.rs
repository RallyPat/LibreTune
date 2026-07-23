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
}

impl Default for AutoTuneSettings {
    fn default() -> Self {
        Self {
            target_afr: 14.7,
            algorithm: "simple".to_string(),
            update_rate_ms: 100,
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
    fn resolve_target_afr(
        &self,
        cell_x: usize,
        cell_y: usize,
        fallback: f64,
    ) -> f64 {
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

        // Always add to buffer for lambda delay correlation
        self.data_buffer.push_back(point.clone());
        self.prune_data_buffer(point.timestamp_ms);

        if !self.passes_filters(&point, filters) {
            return;
        }

        // Count every sample that passed filters; used as the denominator for
        // per-cell hit_percentage (#16).
        self.total_samples += 1;

        // Resolve the lambda delay. Prefer the per-cell value from the
        // reference table (bug #14) at the *current* conditions, falling back
        // to the RPM-based transport-delay curve.
        let cur_x_idx = self.find_bin_index(point.rpm, table_x_bins);
        let cur_y_idx = self.find_bin_index(point.load, table_y_bins);
        let delay_ms = match (cur_x_idx, cur_y_idx) {
            (Some(cx), Some(cy)) => self
                .resolve_lambda_delay_ms(cx, cy)
                .unwrap_or_else(|| self.get_lambda_delay_ms(point.rpm)),
            _ => self.get_lambda_delay_ms(point.rpm),
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

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]
    use super::*;

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
}
