//! AFR transport-delay extraction from an enrichment step.
//!
//! The delay test applies a fuel step at a known instant; the wideband sees it
//! only after the exhaust transport delay plus the sensor's own lag. This
//! module turns the sampled AFR trace around one step into a single measured
//! delay, or declines when the trace can't support one.
//!
//! Pure functions, no I/O — the command layer feeds it samples and an anchor
//! timestamp. Everything here is unit-tested against synthetic traces because
//! the bench simulator's AFR does not respond to fuelling; first live
//! validation happens on a real engine.

use serde::Serialize;

/// One realtime sample: milliseconds (monotonic, same clock as the step
/// anchor) and the AFR reading.
#[derive(Debug, Clone, Copy)]
pub struct AfrSample {
    pub t_ms: u64,
    pub afr: f64,
}

/// A successful delay extraction for one enrichment step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DelayMeasurement {
    /// Milliseconds from the step anchor to half the settled excursion.
    ///
    /// A step response is the cumulative distribution of transit times, so its
    /// half-height is the *median* transit — the transport delay. The leading
    /// edge is only the fastest path through the manifold and sits in the
    /// noise: on 80 real steps the fixed-threshold leading edge gave a median
    /// of 268 ms and scattered 8-2117 ms, while half-excursion on the same
    /// steps gave 435 ms with an IQR of +/-30 ms.
    pub delay_ms: f64,
    /// Baseline minus the SETTLED AFR — the size of the step the engine
    /// actually delivered, not the depth at the trigger.
    ///
    /// The trigger depth is ~the threshold by construction and carries no
    /// information; the settled excursion is what a commanded-vs-delivered
    /// comparison needs (a fuel step of x% should move AFR by a predictable
    /// amount, and the shortfall is the injector/flow story).
    pub excursion: f64,
    /// Pre-step baseline the response was measured against.
    pub baseline_afr: f64,
    /// Milliseconds to the first sustained crossing of the noise threshold.
    ///
    /// Kept for comparison with historical numbers, which were all measured
    /// this way. Biased short; do not use it as the delay.
    pub leading_edge_ms: f64,
}

/// Why a step produced no measurement — surfaced to the UI so a silent
/// no-result is distinguishable from a broken run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelayRejection {
    /// Fewer pre-step samples than needed to establish a baseline.
    InsufficientBaseline,
    /// Pre-step AFR was moving too much to anchor a baseline (operator not
    /// steady, or sensor noise beyond usable).
    UnstableBaseline,
    /// No sustained excursion past the threshold inside the window (step too
    /// small for the noise, sensor dead, or — on the bench — a simulator
    /// whose AFR ignores fuelling).
    NoResponse,
    /// The trigger was crossed at or before the step instant, so the AFR was
    /// already moving when the step landed — the previous step had not settled,
    /// or the mixture was drifting for reasons of its own. An effect cannot
    /// precede its cause, so there is no transport delay to report here.
    ResponsePrecedesStep,
    /// AFR responded but was still moving when the hold ended, so there is no
    /// settled level to take half of. Almost always the hold is short relative
    /// to the transport delay at this operating point — the fix is a longer
    /// hold, not a different threshold.
    ResponseNotSettled,
}

impl DelayRejection {
    /// Short operator-facing label.
    pub fn label(&self) -> &'static str {
        match self {
            DelayRejection::InsufficientBaseline => "too few baseline samples",
            DelayRejection::UnstableBaseline => "baseline unstable — hold steadier",
            DelayRejection::NoResponse => "no AFR response detected",
            DelayRejection::ResponsePrecedesStep => "AFR already moving at the step",
            DelayRejection::ResponseNotSettled => "still moving at end of hold — use a longer hold",
        }
    }
}

/// Minimum samples required in the pre-step window.
const MIN_BASELINE_SAMPLES: usize = 4;
/// Baseline scatter (median absolute deviation) above which no edge can be
/// trusted, in AFR points.
const MAX_BASELINE_MAD: f64 = 0.35;
/// The excursion must exceed max(K_MAD * MAD, MIN_EXCURSION_AFR).
const K_MAD: f64 = 3.0;
const MIN_EXCURSION_AFR: f64 = 0.15;
/// An edge only counts when the crossing is sustained for this many samples,
/// so a single noise spike cannot fake a response.
const SUSTAIN_SAMPLES: usize = 2;
/// Tail of the hold window used to estimate the settled AFR, as a fraction of
/// the window's duration.
const PLATEAU_TAIL_FRAC: f64 = 0.30;
/// The plateau must be flatter than this (MAD, AFR points) to count as
/// settled. Looser than the baseline limit: a warm plateau under enrichment is
/// noisier than an idle baseline, but a trace still in transit is far noisier
/// than either.
const MAX_PLATEAU_MAD: f64 = 0.30;

/// First sustained crossing below `level`, interpolated across the crossing.
///
/// Returns (crossing_ms, sample_at_crossing). Reporting the sample time alone
/// biases every measurement late by up to a full interval — 31 ms average at
/// the ~16 Hz these logs actually run at, a large share of a high-flow delay.
fn first_crossing(post: &[AfrSample], level: f64) -> Option<(f64, AfrSample)> {
    let mut run = 0usize;
    let mut edge: Option<AfrSample> = None;
    let mut before_edge: Option<AfrSample> = None;
    let mut prev: Option<AfrSample> = None;
    for s in post {
        if s.afr < level {
            run += 1;
            if run == 1 {
                edge = Some(*s);
                before_edge = prev;
            }
            if run >= SUSTAIN_SAMPLES {
                let e = edge.expect("run >= 1 implies edge set");
                let crossing_ms = match before_edge {
                    Some(b) if b.afr > e.afr && b.t_ms < e.t_ms => {
                        let span = (e.t_ms - b.t_ms) as f64;
                        let frac = ((b.afr - level) / (b.afr - e.afr)).clamp(0.0, 1.0);
                        b.t_ms as f64 + span * frac
                    }
                    // No usable prior sample (the crossing is the first sample
                    // after the anchor): fall back to the sample's own time.
                    _ => e.t_ms as f64,
                };
                return Some((crossing_ms, e));
            }
        } else {
            run = 0;
            edge = None;
            before_edge = None;
        }
        prev = Some(*s);
    }
    None
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    }
}

/// Median absolute deviation — robust scatter estimate for a noisy trace.
fn mad(values: &[f64], med: f64) -> f64 {
    let mut devs: Vec<f64> = values.iter().map(|v| (v - med).abs()).collect();
    median(&mut devs)
}

/// Extract the transport delay for one enrichment step.
///
/// `anchor_ms` is the instant the enriched value finished writing to the ECU.
/// `pre` are samples strictly before the anchor (baseline window); `post` are
/// samples at or after it, in time order. Enrichment drives AFR *down*, so the
/// edge is the first sustained crossing below `baseline - threshold`.
pub fn detect_delay(
    anchor_ms: u64,
    pre: &[AfrSample],
    post: &[AfrSample],
) -> Result<DelayMeasurement, DelayRejection> {
    if pre.len() < MIN_BASELINE_SAMPLES {
        return Err(DelayRejection::InsufficientBaseline);
    }

    let mut base_vals: Vec<f64> = pre.iter().map(|s| s.afr).collect();
    let baseline = median(&mut base_vals);
    let scatter = mad(&base_vals, baseline);
    if scatter > MAX_BASELINE_MAD {
        return Err(DelayRejection::UnstableBaseline);
    }

    let threshold = (K_MAD * scatter).max(MIN_EXCURSION_AFR);

    // 1. Did anything happen at all? The noise-threshold crossing answers that
    //    and gives the historical leading-edge figure for comparison.
    let (leading_ms, _) =
        first_crossing(post, baseline - threshold).ok_or(DelayRejection::NoResponse)?;

    // A crossing at or before the anchor is not a fast delay, it is a
    // measurement of something that started earlier. Clamping it to zero used
    // to record it as a valid 0 ms sample: on a 52-minute drive three such
    // samples landed in two rpm/load bins and pulled both means to exactly 0.
    //
    // Zero is worse than a missing value, because this figure feeds AutoTune's
    // historical-point lookup — a 0 ms delay makes it test the CURRENT sample
    // for fuel cut, which is exactly the test that misses the tail of a cut
    // still in the exhaust.
    if leading_ms <= anchor_ms as f64 {
        return Err(DelayRejection::ResponsePrecedesStep);
    }

    // 2. Where did it settle? The tail of the hold window is the plateau, and
    //    it is only a plateau if it has stopped moving.
    let (first_t, last_t) = (
        post.first().map(|s| s.t_ms).unwrap_or(0) as f64,
        post.last().map(|s| s.t_ms).unwrap_or(0) as f64,
    );
    let tail_start = last_t - (last_t - first_t) * PLATEAU_TAIL_FRAC;
    let mut tail: Vec<f64> = post
        .iter()
        .filter(|s| s.t_ms as f64 >= tail_start)
        .map(|s| s.afr)
        .collect();
    if tail.len() < MIN_BASELINE_SAMPLES {
        return Err(DelayRejection::ResponseNotSettled);
    }
    let plateau = median(&mut tail);
    if mad(&tail, plateau) > MAX_PLATEAU_MAD {
        return Err(DelayRejection::ResponseNotSettled);
    }

    let excursion = baseline - plateau;
    // The plateau has to be a real step, not the threshold scraped by noise.
    if excursion < threshold {
        return Err(DelayRejection::NoResponse);
    }

    // 3. The delay is where the response reaches half its settled size. A step
    //    response is the CDF of transit times, so the half-height is the
    //    median transit; the leading edge is only the fastest path.
    let (half_ms, _) = first_crossing(post, baseline - excursion / 2.0)
        .ok_or(DelayRejection::ResponseNotSettled)?;
    if half_ms <= anchor_ms as f64 {
        return Err(DelayRejection::ResponsePrecedesStep);
    }

    Ok(DelayMeasurement {
        delay_ms: half_ms - anchor_ms as f64,
        excursion,
        baseline_afr: baseline,
        leading_edge_ms: leading_ms - anchor_ms as f64,
    })
}

/// Fixed coarse grid for aggregating measurements by operating point.
/// Bin edges chosen for a small NA engine; a cell is (load_bin, rpm_bin).
pub const RPM_EDGES: [f64; 5] = [1200.0, 2000.0, 3000.0, 4500.0, 6500.0];
pub const LOAD_EDGES: [f64; 4] = [40.0, 60.0, 80.0, 100.0];

/// One aggregated cell of the delay table.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct DelayCell {
    pub n: u32,
    pub mean_ms: f64,
    /// Simple spread indicator: max - min of contributing delays.
    pub range_ms: f64,
    #[serde(skip)]
    min_ms: f64,
    #[serde(skip)]
    max_ms: f64,
}

/// rpm × load grid of aggregated delay measurements.
#[derive(Debug, Clone, Serialize)]
pub struct DelayTable {
    /// Upper edges of the rpm bins; the last bin is open-ended.
    pub rpm_edges: Vec<f64>,
    /// Upper edges of the load bins; the last bin is open-ended.
    pub load_edges: Vec<f64>,
    /// `cells[load_bin][rpm_bin]`
    pub cells: Vec<Vec<DelayCell>>,
}

impl DelayTable {
    pub fn new() -> Self {
        let rows = LOAD_EDGES.len() + 1;
        let cols = RPM_EDGES.len() + 1;
        Self {
            rpm_edges: RPM_EDGES.to_vec(),
            load_edges: LOAD_EDGES.to_vec(),
            cells: vec![vec![DelayCell::default(); cols]; rows],
        }
    }

    fn bin(edges: &[f64], v: f64) -> usize {
        edges.iter().position(|e| v < *e).unwrap_or(edges.len())
    }

    /// Bin indices for an operating point — exposed for tests and callers
    /// that need to address a cell directly.
    pub fn cell_index(&self, rpm: f64, load: f64) -> (usize, usize) {
        (
            Self::bin(&self.load_edges, load),
            Self::bin(&self.rpm_edges, rpm),
        )
    }

    /// Fold one measurement taken at (rpm, load) into its cell.
    pub fn add(&mut self, rpm: f64, load: f64, delay_ms: f64) {
        let (l, r) = self.cell_index(rpm, load);
        let c = &mut self.cells[l][r];
        if c.n == 0 {
            c.min_ms = delay_ms;
            c.max_ms = delay_ms;
        } else {
            c.min_ms = c.min_ms.min(delay_ms);
            c.max_ms = c.max_ms.max(delay_ms);
        }
        c.mean_ms = (c.mean_ms * c.n as f64 + delay_ms) / (c.n as f64 + 1.0);
        c.n += 1;
        c.range_ms = c.max_ms - c.min_ms;
    }
}

impl Default for DelayTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trace(points: &[(u64, f64)]) -> Vec<AfrSample> {
        points
            .iter()
            .map(|&(t_ms, afr)| AfrSample { t_ms, afr })
            .collect()
    }

    /// The crossing rarely lands exactly on a sample. Reporting the sample's own
    /// time biases every measurement late by up to a full interval — 63 ms at
    /// the ~16 Hz these logs run at, which is a big share of a high-flow delay.
    /// Interpolating across the crossing must land between the two samples and
    /// close to where the AFR genuinely passed the trigger.
    #[test]
    fn crossing_is_interpolated_between_samples() {
        let pre = trace(&[(0, 14.70), (60, 14.70), (120, 14.70), (180, 14.70)]);
        // Settles to 13.70, so the half level is 14.20 — crossed somewhere
        // inside the 240-300 gap, nearer 300 the deeper the later sample.
        let post = trace(&[
            (240, 14.70),
            (300, 14.10),
            (360, 13.85),
            (420, 13.72),
            (480, 13.70),
            (540, 13.70),
            (600, 13.70),
            (660, 13.70),
            (720, 13.70),
            (780, 13.70),
            (840, 13.70),
            (900, 13.70),
        ]);

        let m = detect_delay(240, &pre, &post).expect("should measure");
        assert!(
            m.delay_ms > 0.0 && m.delay_ms < 60.0,
            "interpolated crossing {:.0} ms must fall inside the 240-300 ms gap",
            m.delay_ms
        );
        // Reporting the raw sample time would have given exactly 60 ms.
        assert!(
            m.delay_ms < 59.0,
            "expected sub-sample resolution, got the sample time itself ({:.0} ms)",
            m.delay_ms
        );
    }

    /// An AFR already past the trigger at the step instant is a REJECTION,
    /// not a zero-millisecond delay.
    ///
    /// This test previously asserted `delay_ms == 0.0`, on the reasoning that
    /// the sample's own time was "the honest answer". It is not: exhaust gas
    /// cannot reach the sensor in zero time, so a crossing at the anchor means
    /// the mixture was already moving before the step could have any effect.
    /// Recording it as a measurement put exact zeros into the delay table - on
    /// a real 52-minute drive, three of them, pulling two rpm/load bins to a
    /// mean of 0 ms.
    #[test]
    fn crossing_at_or_before_the_anchor_is_rejected() {
        let pre = trace(&[(0, 14.70), (60, 14.70), (120, 14.70), (180, 14.70)]);
        let post = trace(&[(240, 14.20), (300, 14.10), (360, 14.05)]);
        assert_eq!(
            detect_delay(240, &pre, &post),
            Err(DelayRejection::ResponsePrecedesStep),
            "a response at the step instant has no transport delay to report"
        );
    }

    /// Clean step: steady 14.7 baseline. AFR is still 14.68 at 320 ms and 14.2
    /// at 380 ms, so it passes the 14.55 trigger at ~336 ms — 136 ms after the
    /// anchor. Reporting the sample time would say 180 ms, 44 ms late.
    #[test]
    fn clean_step_yields_the_transport_delay() {
        let pre = trace(&[
            (0, 14.7),
            (40, 14.68),
            (80, 14.72),
            (120, 14.7),
            (160, 14.69),
        ]);
        // Baseline 14.70 settling to 13.70: a 1.00 AFR step, so the delay is
        // where the trace passes 14.20 — not where it first leaves the noise.
        let post = trace(&[
            (200, 14.70),
            (240, 14.70),
            (280, 14.70),
            (320, 14.60),
            (360, 14.40),
            (400, 14.15),
            (440, 13.95),
            (480, 13.80),
            (520, 13.72),
            (560, 13.70),
            (600, 13.70),
            (640, 13.70),
            (680, 13.70),
        ]);
        let m = detect_delay(200, &pre, &post).expect("clean step must measure");
        assert!(
            (m.delay_ms - 192.0).abs() < 1.0,
            "half-excursion crossing, got {}",
            m.delay_ms
        );
        assert!(
            (m.excursion - 1.00).abs() < 0.05,
            "excursion must be the SETTLED step, got {}",
            m.excursion
        );
        assert!((m.baseline_afr - 14.7).abs() < 0.05);
    }

    /// A single noise spike below threshold must not register as the edge.
    #[test]
    fn single_spike_is_not_an_edge() {
        let pre = trace(&[(0, 14.7), (40, 14.7), (80, 14.7), (120, 14.7)]);
        let post = trace(&[
            (200, 14.7),
            (240, 13.9), // lone spike
            (280, 14.7),
            (320, 14.71),
            (360, 14.69),
        ]);
        assert_eq!(
            detect_delay(200, &pre, &post),
            Err(DelayRejection::NoResponse)
        );
    }

    /// A sustained crossing right after a lone spike anchors on the real run,
    /// not the spike: the crossing is interpolated between 280 ms (14.7) and
    /// 320 ms (14.0), giving ~89 ms — comfortably after the 40 ms spike.
    #[test]
    fn edge_anchors_at_the_sustained_run() {
        let pre = trace(&[(0, 14.7), (40, 14.7), (80, 14.7), (120, 14.7)]);
        // A one-sample spike must not start the response; the real edge does.
        let post = trace(&[
            (200, 14.7),
            (240, 13.9), // spike, run resets after
            (280, 14.7),
            (320, 14.3),
            (360, 14.0),
            (400, 13.85),
            (440, 13.75),
            (480, 13.70),
            (520, 13.70),
            (560, 13.70),
            (600, 13.70),
        ]);
        let m = detect_delay(200, &pre, &post).expect("must measure");
        // Had the lone spike at 240 ms started the run, the interpolated
        // crossing would land ~7 ms after the anchor. The real edge begins at
        // 320 ms, so both figures must be far past that.
        assert!(
            m.leading_edge_ms > 50.0,
            "the leading edge must ignore the spike, got {}",
            m.leading_edge_ms
        );
        assert!(
            m.delay_ms >= m.leading_edge_ms,
            "half-excursion {} cannot precede the leading edge {}",
            m.delay_ms,
            m.leading_edge_ms
        );
    }

    #[test]
    fn wandering_baseline_is_rejected() {
        let pre = trace(&[(0, 13.2), (40, 15.4), (80, 12.9), (120, 15.8), (160, 13.5)]);
        let post = trace(&[(200, 12.0), (240, 12.0), (280, 12.0)]);
        assert_eq!(
            detect_delay(200, &pre, &post),
            Err(DelayRejection::UnstableBaseline)
        );
    }

    #[test]
    fn too_few_baseline_samples_rejected() {
        let pre = trace(&[(0, 14.7), (40, 14.7)]);
        let post = trace(&[(200, 13.0), (240, 13.0)]);
        assert_eq!(
            detect_delay(200, &pre, &post),
            Err(DelayRejection::InsufficientBaseline)
        );
    }

    /// The dead simulator case: AFR never reacts. Must decline, not invent.
    #[test]
    fn flat_trace_declines() {
        let pre = trace(&[(0, 14.7), (40, 14.7), (80, 14.7), (120, 14.7)]);
        let post: Vec<AfrSample> = (0..40)
            .map(|i| AfrSample {
                t_ms: 200 + i * 40,
                afr: 14.7 + if i % 2 == 0 { 0.02 } else { -0.02 },
            })
            .collect();
        assert_eq!(
            detect_delay(200, &pre, &post),
            Err(DelayRejection::NoResponse)
        );
    }

    #[test]
    fn table_bins_and_aggregates() {
        let mut t = DelayTable::new();
        t.add(900.0, 35.0, 400.0); // idle cell
        t.add(950.0, 38.0, 360.0); // same cell
        t.add(3200.0, 85.0, 140.0); // mid-rpm, high-load
        t.add(7000.0, 105.0, 90.0); // open-ended top bins

        let idle = &t.cells[0][0];
        assert_eq!(idle.n, 2);
        assert!((idle.mean_ms - 380.0).abs() < 1e-9);
        assert!((idle.range_ms - 40.0).abs() < 1e-9);

        let (l, r) = t.cell_index(3200.0, 85.0);
        assert_eq!(t.cells[l][r].n, 1);

        let top = &t.cells[LOAD_EDGES.len()][RPM_EDGES.len()];
        assert_eq!(top.n, 1);
        assert!((top.mean_ms - 90.0).abs() < 1e-9);
    }
    /// The whole point of the change: a step response is the CDF of transit
    /// times, so its half-height is the median transit. The leading edge is
    /// the fastest path and always arrives earlier — on real data it read
    /// 268 ms median against 435 ms for half-excursion, and scattered
    /// 8-2117 ms against an IQR of +/-30 ms.
    #[test]
    fn half_excursion_is_later_than_the_leading_edge() {
        let pre = trace(&[(0, 14.70), (40, 14.70), (80, 14.70), (120, 14.70)]);
        let post = trace(&[
            (200, 14.70),
            (240, 14.60),
            (280, 14.45),
            (320, 14.25),
            (360, 14.05),
            (400, 13.90),
            (440, 13.78),
            (480, 13.72),
            (520, 13.70),
            (560, 13.70),
            (600, 13.70),
            (640, 13.70),
        ]);
        let m = detect_delay(200, &pre, &post).expect("must measure");
        assert!(
            m.delay_ms > m.leading_edge_ms,
            "half-excursion {:.0} must be later than leading edge {:.0}",
            m.delay_ms,
            m.leading_edge_ms
        );
    }

    /// A hold shorter than the transport delay leaves the trace still falling
    /// when enrichment ends. There is no settled level to halve, so the honest
    /// answer is a rejection naming the cause — not a number biased by however
    /// far the response happened to get.
    #[test]
    fn still_moving_at_end_of_hold_is_rejected() {
        let pre = trace(&[(0, 14.7), (40, 14.7), (80, 14.7), (120, 14.7)]);
        let post = trace(&[
            (200, 14.70),
            (240, 14.60),
            (280, 14.35),
            (320, 14.05),
            (360, 13.75),
            (400, 13.45),
            (440, 13.15),
        ]);
        assert_eq!(
            detect_delay(200, &pre, &post),
            Err(DelayRejection::ResponseNotSettled)
        );
    }
}

/// A transport-delay model fitted to real measurements.
///
/// `delay = floor_ms + k / (rpm * load)` — exhaust transport time is plumbing
/// volume divided by flow, and flow rises with both engine speed and load, so
/// the delay is long at idle and short under power. The alternative on offer is
/// a fixed rpm ramp (200 ms at idle to 50 ms at redline) which on one measured
/// NA6 under-predicted every cell by an average of 287 ms.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowDelayFit {
    /// High-flow asymptote: roughly the sensor's own response time.
    pub floor_ms: f64,
    /// Numerator of the flow term. Not user-facing; `anchor_ms` is.
    pub k: f64,
    /// Delay the model predicts at the anchor point AutoTune uses
    /// (800 rpm, 40 kPa) — this is what `lambda_delay_ms` should be set to.
    pub anchor_ms: f64,
    /// Weighted RMS residual, in ms. Compare against the spread of the
    /// measurements themselves before trusting it.
    pub rms_ms: f64,
    /// How many measurements the fit used.
    pub samples: usize,
}

/// Anchor AutoTune's flow-scaled table is built around.
const ANCHOR_RPM: f64 = 800.0;
const ANCHOR_LOAD: f64 = 40.0;

/// Fit [`FlowDelayFit`] to `(rpm, load, delay_ms)` measurements.
///
/// `floor` is scanned rather than solved because the model is only linear in
/// `k` once `floor` is fixed; the range covers a plausible sensor response
/// (40-250 ms) and the grid is far finer than the measurement spread, so a
/// closed-form solve would add precision the data does not contain.
///
/// Returns `None` below four samples — three points will fit anything, and a
/// confident-looking delay drawn from noise is worse than no recommendation.
pub fn fit_flow_delay(samples: &[(f64, f64, f64)]) -> Option<FlowDelayFit> {
    let usable: Vec<(f64, f64, f64)> = samples
        .iter()
        .copied()
        .filter(|(rpm, load, ms)| *rpm > 0.0 && *load > 0.0 && *ms > 0.0)
        .collect();
    if usable.len() < 4 {
        return None;
    }

    let inv_flow = |rpm: f64, load: f64| 1.0 / (rpm * load);
    let mut best: Option<(f64, f64, f64)> = None; // (ss, floor, k)

    let mut floor = 40.0_f64;
    while floor <= 250.0 {
        // Least squares for k with this floor: k = sum(x*y) / sum(x*x),
        // x = 1/flow, y = measured - floor.
        let (mut num, mut den) = (0.0_f64, 0.0_f64);
        for (rpm, load, ms) in &usable {
            let x = inv_flow(*rpm, *load);
            num += x * (ms - floor);
            den += x * x;
        }
        if den > 0.0 {
            let k = num / den;
            let ss: f64 = usable
                .iter()
                .map(|(rpm, load, ms)| {
                    let pred = floor + k * inv_flow(*rpm, *load);
                    (pred - ms).powi(2)
                })
                .sum();
            if best.is_none_or(|(bss, _, _)| ss < bss) {
                best = Some((ss, floor, k));
            }
        }
        floor += 5.0;
    }

    let (ss, floor_ms, k) = best?;
    Some(FlowDelayFit {
        floor_ms,
        k,
        anchor_ms: floor_ms + k * inv_flow(ANCHOR_RPM, ANCHOR_LOAD),
        rms_ms: (ss / usable.len() as f64).sqrt(),
        samples: usable.len(),
    })
}

#[cfg(test)]
mod flow_fit_tests {
    use super::*;

    /// Data generated from a known model must recover that model.
    #[test]
    fn it_recovers_a_known_model() {
        let (floor, k) = (150.0, 30_000_000.0);
        let samples: Vec<(f64, f64, f64)> = [
            (900.0, 35.0),
            (1500.0, 40.0),
            (2500.0, 50.0),
            (3500.0, 70.0),
            (5000.0, 90.0),
            (6000.0, 96.0),
        ]
        .iter()
        .map(|(rpm, load)| (*rpm, *load, floor + k / (rpm * load)))
        .collect();

        let fit = fit_flow_delay(&samples).expect("enough samples");
        assert!(
            (fit.floor_ms - floor).abs() <= 5.0,
            "floor: {}",
            fit.floor_ms
        );
        assert!(
            fit.rms_ms < 5.0,
            "clean data should fit tightly: {}",
            fit.rms_ms
        );
        assert_eq!(fit.samples, 6);
    }

    /// The shape that matters: delay must fall as flow rises. A model that got
    /// this backwards would attribute high-load readings to the wrong cells.
    #[test]
    fn the_fitted_model_falls_with_flow() {
        let samples = vec![
            (900.0, 35.0, 1100.0),
            (1500.0, 40.0, 700.0),
            (2500.0, 50.0, 450.0),
            (5000.0, 90.0, 260.0),
        ];
        let fit = fit_flow_delay(&samples).expect("fits");
        let at = |rpm: f64, load: f64| fit.floor_ms + fit.k / (rpm * load);
        assert!(
            at(900.0, 35.0) > at(5000.0, 90.0),
            "delay must fall with flow"
        );
        assert!(fit.k > 0.0, "a negative k would invert the model");
    }

    #[test]
    fn too_few_samples_yields_no_recommendation() {
        // Three points fit anything; a confident number from noise is worse
        // than admitting there is none.
        assert!(fit_flow_delay(&[(1000.0, 40.0, 500.0)]).is_none());
        assert!(fit_flow_delay(&[(1000.0, 40.0, 500.0), (2000.0, 50.0, 400.0)]).is_none());
        assert!(fit_flow_delay(&[
            (1000.0, 40.0, 500.0),
            (2000.0, 50.0, 400.0),
            (3000.0, 60.0, 300.0)
        ])
        .is_none());
    }

    #[test]
    fn rubbish_samples_are_dropped_before_fitting() {
        let samples = vec![
            (0.0, 40.0, 500.0),   // engine stopped
            (1000.0, 0.0, 500.0), // no load reading
            (1000.0, 40.0, 0.0),  // no delay measured
            (900.0, 35.0, 1100.0),
            (1500.0, 40.0, 700.0),
            (2500.0, 50.0, 450.0),
            (5000.0, 90.0, 260.0),
        ];
        let fit = fit_flow_delay(&samples).expect("fits on the four good ones");
        assert_eq!(fit.samples, 4);
    }

    /// The anchor is what `lambda_delay_ms` gets set to, so it must be the
    /// model's value at the anchor point rather than any measured sample.
    #[test]
    fn the_anchor_is_the_model_at_800_by_40() {
        let samples = vec![
            (900.0, 35.0, 1100.0),
            (1500.0, 40.0, 700.0),
            (2500.0, 50.0, 450.0),
            (5000.0, 90.0, 260.0),
        ];
        let fit = fit_flow_delay(&samples).expect("fits");
        let expected = fit.floor_ms + fit.k / (800.0 * 40.0);
        assert!((fit.anchor_ms - expected).abs() < 1e-6);
        assert!(fit.anchor_ms > fit.floor_ms, "the anchor is the slow end");
    }
}
