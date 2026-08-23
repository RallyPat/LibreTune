//! Automated AFR transport-delay measurement.
//!
//! Measuring how long the wideband takes to see a fuelling change needs a
//! clean, precisely-timed step at a known operating point. Doing that by hand
//! means typing a value, watching a gauge, and typing it back — awkward while
//! driving, and impossible to time accurately enough for a delay of a few
//! hundred milliseconds.
//!
//! This runs the step automatically: enrich by a set percentage, hold, restore,
//! settle, repeat. The operator holds the engine at the operating point; the
//! app only touches fuelling.
//!
//! # Safety
//!
//! This writes to a running engine, so the design is deliberately narrow:
//!
//! - **Enrichment only.** The step percentage is clamped positive. A rich
//!   excursion is harmless; a lean one at load destroys pistons. There is no
//!   parameter that can make the mixture leaner.
//! - **Bounded magnitude.** Capped at [`MAX_STEP_PERCENT`], well inside the
//!   range where an engine simply runs rich.
//! - **RAM only, never burned.** The step is written to the ECU's live memory,
//!   so it is not persisted. Cycling the key restores the stored tune even if
//!   this process dies mid-step.
//! - **One byte.** The lever is the warmup curve's warm-plateau slot
//!   (`wueRates[9]`, normally 100%): a single-byte write steps fuelling
//!   engine-wide, and a single byte restores it. See [`WUE_CONSTANT`].
//! - **Restore on every path.** The original value is written back after each
//!   step, on abort, and on error. The restore is attempted even when the run
//!   is failing, and a failure to restore is escalated loudly.
//! - **Abortable.** Abort takes effect within one tick (~50 ms), not at the
//!   next step boundary: an abort during the hold cuts the enrichment short
//!   and restores immediately, and an abort during the settle skips the
//!   remaining wait.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use libretune_core::autotune::delay_measure::{
    detect_delay, AfrSample, DelayRejection, DelayTable,
};

use crate::state::AppState;

/// Largest enrichment the test will apply. Chosen so the mixture stays in the
/// range where an engine runs rich but healthy; the useful signal is around
/// 8-10% and anything beyond this is measuring nothing new.
const MAX_STEP_PERCENT: f64 = 20.0;

/// Smallest enrichment worth applying. Below this the AFR change disappears
/// into normal cycle-to-cycle scatter and no delay can be extracted.
const MIN_STEP_PERCENT: f64 = 3.0;

/// Most steps a single run will perform.
///
/// Each step writes and restores one byte, so the cost of more steps is time,
/// not risk — and the statistics need them: individual measurements scatter by
/// hundreds of milliseconds, so a trustworthy delay figure comes from many
/// repeats rather than a few. The cap only stops a mistyped value running for
/// hours. The previous ceiling of 20 was both low and silent: asking for 100
/// ran 20 while the dialog still quoted the time for 100.
const MAX_REPEATS: u32 = 200;

/// Throttle movement above which a step is not measuring transport.
///
/// A moving throttle changes the mixture for its own reasons and drags accel
/// enrichment along with it. On a real 59-minute drive this and the fuel-cut
/// test together disqualified 44 of 141 steps; the remaining 97 gave a
/// coherent flow-scaled curve where the unfiltered set gave 52-2979 ms.
const TPS_DOT_LIMIT: f64 = 10.0;

/// Bounds on how long a step is held, in milliseconds.
const MIN_HOLD_MS: u64 = 500;
const MAX_HOLD_MS: u64 = 5_000;

/// The fuel lever: the LAST slot of the warmup-enrichment curve.
///
/// On a warm engine the firmware's `correctionWUE()` returns this slot's raw
/// value every fuel calculation — cache-free, multiplied straight into pulse
/// width engine-wide (corrections.cpp @ 202501). The INI mandates the slot be
/// 100 (= 100%, no enrichment) on a healthy tune, so stepping it to 108 is an
/// instant +8% everywhere with a ONE-BYTE write, and restoring is one byte
/// back. Earlier levers failed: `reqFuel` is in requiresPowerCycle (live
/// writes ignored by a running engine — measured), and whole-VE-table
/// stepping needs 256 writes each way with the edge smeared across ~2.5 s of
/// serial. As a bonus the ECU broadcasts the multiplier it is actually
/// applying (`warmupEnrich` output channel), giving a ground-truth anchor for
/// the moment the step took effect.
const WUE_CONSTANT: &str = "wueRates";
/// Index of the warm-plateau slot within the WUE curve.
const WUE_LAST_SLOT: usize = 9;
/// Realtime channel reporting the WUE multiplier the ECU is applying now.
const WUE_CHANNEL: &str = "warmupEnrich";

/// One sample of a step's trace, as the UI plots it. `t_ms` is relative to the
/// step anchor, so traces from different steps overlay directly.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TracePoint {
    pub t_ms: i64,
    pub afr: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pw: Option<f64>,
}

/// Progress event emitted to the frontend during a delay-test run (as
/// `afr_delay_test:progress`). `phase` is a coarse stage label —
/// "starting", "enriching", "settling", then "complete" or "aborted".
/// `applied_value` and `baseline_value` are the applied and restore
/// WUE-slot values (percent) so the UI can show exactly what is written
/// and confirm the restore.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DelayTestProgress {
    pub phase: String,
    pub step: u32,
    pub total_steps: u32,
    pub applied_value: f64,
    pub baseline_value: f64,
    pub message: String,
    /// Measured transport delay for the step just completed, when the AFR
    /// trace supported one. Absent on phases other than "settling".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measured_delay_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measured_rpm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measured_load: Option<f64>,
    /// Why no delay was measured for this step (operator-facing label).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection: Option<String>,
    /// The step's AFR/pulse-width trace, times relative to the anchor, so the
    /// UI can overlay it on the ones before it. Present on "settling" only.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub trace: Vec<TracePoint>,
    /// True when the ECU was cutting fuel, or the throttle moving faster than
    /// TPS_DOT_LIMIT, anywhere in the window - the trace is drawn but excluded
    /// from the statistics.
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub unusable: bool,
}

impl DelayTestProgress {
    fn plain(
        phase: &str,
        step: u32,
        total_steps: u32,
        applied_value: f64,
        baseline_value: f64,
        message: String,
    ) -> Self {
        Self {
            phase: phase.into(),
            step,
            total_steps,
            applied_value,
            baseline_value,
            message,
            measured_delay_ms: None,
            measured_rpm: None,
            measured_load: None,
            rejection: None,
            trace: Vec::new(),
            unusable: false,
        }
    }
}

fn emit(app: &AppHandle, p: DelayTestProgress) {
    let _ = app.emit("afr_delay_test:progress", p);
}

/// Set while a run owns the WUE slot, so a second run cannot start.
///
/// The baseline is read from the tune cache, which the running test itself
/// updates when it applies the enriched value. A second invocation starting
/// mid-step therefore reads the ENRICHED value as its baseline, and every
/// "restore" it performs writes that value back — leaving the engine
/// permanently rich in RAM while reporting "WUE slot restored". The abort flag
/// and the sample buffer are process-wide statics too, so concurrent runs
/// would also abort each other and interleave their measurements.
///
/// The dialog disables its own button, but the command is reachable from a
/// second window, a retry after a perceived hang, or a script.
static RUNNING: AtomicBool = AtomicBool::new(false);

/// Clears [`RUNNING`] however the run ends, including on an early `?` return.
struct RunGuard;

impl Drop for RunGuard {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::SeqCst);
    }
}

/// Shared abort flag so [`abort_afr_delay_test`] can stop a run in progress.
static ABORT: std::sync::OnceLock<Arc<AtomicBool>> = std::sync::OnceLock::new();

fn abort_flag() -> Arc<AtomicBool> {
    ABORT
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

/// Request that a running test stop. The current step is restored before the
/// run ends.
#[tauri::command]
pub async fn abort_afr_delay_test() -> Result<(), String> {
    abort_flag().store(true, Ordering::SeqCst);
    Ok(())
}

/// Sleep for `total_ms`, waking every ~50 ms to check the abort flag, so an
/// abort takes effect almost immediately instead of after the full hold or
/// settle (up to 15 s combined — long enough that the Abort button appears
/// dead, over an engine being held rich). Returns true if an abort was
/// requested during (or before) the wait.
///
/// Used for the settle window when there is no AFR channel to sample: with
/// channels present the settle is sampled instead, and `sample_window` runs
/// the same per-tick abort check.
async fn sleep_abortable(total_ms: u64) -> bool {
    const TICK_MS: u64 = 50;
    let mut remaining = total_ms;
    while remaining > 0 {
        if abort_flag().load(Ordering::SeqCst) {
            return true;
        }
        let tick = remaining.min(TICK_MS);
        tokio::time::sleep(Duration::from_millis(tick)).await;
        remaining -= tick;
    }
    abort_flag().load(Ordering::SeqCst)
}

/// Delay measurements accumulated across runs: (rpm, load, delay_ms).
/// Session-scoped by design — a delay map belongs to one engine/exhaust
/// combination; clearing is explicit via [`clear_afr_delay_samples`].
static DELAY_SAMPLES: std::sync::OnceLock<StdMutex<Vec<(f64, f64, f64)>>> =
    std::sync::OnceLock::new();

/// A copy of every measurement taken this session, for callers that want to
/// model the delay rather than read a binned table.
pub(crate) fn delay_samples_snapshot() -> Vec<(f64, f64, f64)> {
    delay_samples()
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default()
}

fn delay_samples() -> &'static StdMutex<Vec<(f64, f64, f64)>> {
    DELAY_SAMPLES.get_or_init(|| StdMutex::new(Vec::new()))
}

/// Realtime channels the sampler needs, resolved once per run from a live
/// snapshot's keys (INI channel naming varies across dialects).
struct SampleChannels {
    afr: String,
    rpm: Option<String>,
    load: Option<String>,
    /// The ECU-reported WUE multiplier — used to gate on the warm plateau and
    /// to anchor t0 at the moment the ECU actually applied the step.
    wue: Option<String>,
    /// Injector pulse width: the fuel the engine actually received, as opposed
    /// to the multiplier we asked for. Overlaying it against AFR is how the
    /// delay is read by eye, and it is the honest t0 - the commanded step and
    /// the delivered step are not the same instant.
    pw: Option<String>,
    /// Deceleration fuel cut and throttle rate. A step measured while the ECU
    /// is cutting fuel, or while the throttle is moving, is not measuring
    /// transport; both are recorded so the trace can be marked unusable.
    dfco: Option<String>,
    tps_dot: Option<String>,
}

/// What was true at each sampled instant besides AFR: the fuel actually
/// delivered, and whether the moment was valid to measure at all.
struct SampleContext {
    pw: Option<f64>,
    dfco: bool,
    tps_dot: f64,
}

/// One window's samples: AFR for the delay extraction, and the matching
/// context for the plot and the validity test. Kept together so the two can
/// never drift out of step - they are pushed as a pair.
#[derive(Default)]
struct StepSamples {
    afr: Vec<AfrSample>,
    ctx: Vec<SampleContext>,
}

fn resolve_sample_channels(
    snapshot: &std::collections::HashMap<String, f64>,
) -> Option<SampleChannels> {
    let find_exact = |want: &str| {
        snapshot
            .keys()
            .find(|k| k.eq_ignore_ascii_case(want))
            .cloned()
    };
    let afr = find_exact("afr")
        .or_else(|| {
            snapshot
                .keys()
                .find(|k| {
                    let l = k.to_ascii_lowercase();
                    l.contains("afr") && !l.contains("target") && !l.contains("protect")
                })
                .cloned()
        })
        .or_else(|| find_exact("lambda"))
        .or_else(|| find_exact("o2"))?;
    Some(SampleChannels {
        pw: find_exact("pulseWidth").or_else(|| find_exact("pw")),
        dfco: find_exact("DFCOOn").or_else(|| find_exact("dfco")),
        tps_dot: find_exact("TPSdot").or_else(|| find_exact("tpsDot")),
        afr,
        rpm: find_exact("rpm"),
        // MAP in kPa preferred as the load axis; TPS is the fallback.
        load: find_exact("map").or_else(|| find_exact("tps")),
        wue: find_exact(WUE_CHANNEL),
    })
}

/// Read or write the single byte of `wueRates[WUE_LAST_SLOT]`, with the same
/// cache/tune bookkeeping as the table-cell writer. One count=1 frame on the
/// wire — the write form proven to reach a running engine.
async fn wue_slot(state: &AppState, write: Option<u8>) -> Result<u8, String> {
    let (page, offset) = {
        let def_guard = state.definition.lock().await;
        let def = def_guard.as_ref().ok_or("Definition not loaded")?;
        let c = def
            .constants
            .get(WUE_CONSTANT)
            .ok_or_else(|| format!("{WUE_CONSTANT} not found in this INI"))?;
        // Never compute an offset past the curve the INI actually declares.
        // Speeduino ships `wueRates = array, U08, 4, [10]`, so slot 9 is the
        // last valid one — but a fork or an older firmware with a shorter
        // curve would otherwise have this write a live byte into whichever
        // constant follows it in the page, on a running engine.
        let count = c.shape.element_count();
        if count <= WUE_LAST_SLOT {
            return Err(format!(
                "{WUE_CONSTANT} declares {count} element(s); the delay test needs at least {}. \
                 This firmware's warm-up curve is too short for the test to drive it safely.",
                WUE_LAST_SLOT + 1
            ));
        }
        let elem = c.data_type.size_bytes().max(1);
        (c.page, c.offset + (WUE_LAST_SLOT * elem) as u16)
    };

    let mut conn_guard = state.connection.lock().await;
    let mut cache_guard = state.tune_cache.lock().await;

    let current = cache_guard
        .as_ref()
        .and_then(|c| c.read_bytes(page, offset, 1).map(|b| b[0]))
        .ok_or("WUE curve not in the tune cache — sync the ECU first")?;

    let Some(value) = write else {
        return Ok(current);
    };

    if let Some(cache) = cache_guard.as_mut() {
        cache.write_bytes(page, offset, &[value]);
    }
    let mut tune_guard = state.current_tune.lock().await;
    if let Some(tune) = tune_guard.as_mut() {
        if let Some(page_data) = tune.pages.get_mut(&page) {
            if (offset as usize) < page_data.len() {
                page_data[offset as usize] = value;
            }
        }
    }
    if let Some(conn) = conn_guard.as_mut() {
        let params = libretune_core::protocol::commands::WriteMemoryParams {
            can_id: 0,
            page,
            offset,
            data: vec![value],
        };
        conn.write_memory(params)
            .map_err(|e| format!("ECU write failed: {e}"))?;
    } else {
        return Err("Not connected to the ECU".to_string());
    }
    Ok(current)
}

/// Sample AFR (and remember the latest rpm/load) for `total_ms`, checking the
/// abort flag between samples exactly like [`sleep_abortable`]. Each sample is
/// a live single-shot read that serializes with the realtime stream on the
/// connection lock, giving ~15-20 Hz effective — adequate for transport delays
/// of 100-500 ms. Read failures skip the sample rather than aborting the run
/// (a short trace downgrades to a rejection, not an error).
///
/// Returns true if an abort was requested during the window.
async fn sample_window(
    state: &tauri::State<'_, AppState>,
    epoch: Instant,
    channels: Option<&SampleChannels>,
    total_ms: u64,
    out: &mut StepSamples,
    last_point: &mut (Option<f64>, Option<f64>),
    // When set: (threshold, slot for first crossing time). The ECU's reported
    // WUE multiplier crossing the threshold marks the instant the step was
    // actually applied — a ground-truth t0 immune to serial/write latency.
    mut wue_edge: Option<(f64, &mut Option<u64>)>,
) -> bool {
    const TICK_MS: u64 = 30;
    let end = epoch.elapsed().as_millis() as u64 + total_ms;
    loop {
        if abort_flag().load(Ordering::SeqCst) {
            return true;
        }
        let now = epoch.elapsed().as_millis() as u64;
        if now >= end {
            return abort_flag().load(Ordering::SeqCst);
        }
        if let Some(ch) = channels {
            if let Ok(snap) = crate::commands::realtime_get::get_realtime_data(state.clone()).await
            {
                let t_ms = epoch.elapsed().as_millis() as u64;
                if let Some(afr) = snap.get(&ch.afr) {
                    out.afr.push(AfrSample { t_ms, afr: *afr });
                    out.ctx.push(SampleContext {
                        pw: ch.pw.as_ref().and_then(|k| snap.get(k)).copied(),
                        dfco: ch
                            .dfco
                            .as_ref()
                            .and_then(|k| snap.get(k))
                            .is_some_and(|v| *v > 0.5),
                        tps_dot: ch
                            .tps_dot
                            .as_ref()
                            .and_then(|k| snap.get(k))
                            .copied()
                            .unwrap_or(0.0),
                    });
                }
                if let Some(rpm_key) = &ch.rpm {
                    if let Some(v) = snap.get(rpm_key) {
                        last_point.0 = Some(*v);
                    }
                }
                if let Some(load_key) = &ch.load {
                    if let Some(v) = snap.get(load_key) {
                        last_point.1 = Some(*v);
                    }
                }
                if let Some((threshold, slot)) = wue_edge.as_mut() {
                    if slot.is_none() {
                        if let Some(wue_key) = &ch.wue {
                            if let Some(v) = snap.get(wue_key) {
                                if *v >= *threshold {
                                    **slot = Some(t_ms);
                                }
                            }
                        }
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(TICK_MS)).await;
    }
}

/// Run an automated series of enrichment steps.
///
/// `step_percent` is clamped to [`MIN_STEP_PERCENT`]..=[`MAX_STEP_PERCENT`] and
/// forced positive. `hold_ms` is how long the enrichment is applied;
/// `settle_ms` is the pause afterwards for the mixture to return to baseline
/// before the next step.
///
/// Each step now also measures the AFR transport delay: a short pre-roll
/// establishes the baseline, the hold window is sampled live, and the edge is
/// extracted by [`detect_delay`]. Successful measurements accumulate into the
/// session's rpm×load delay table ([`get_afr_delay_table`]).
#[tauri::command]
pub async fn run_afr_delay_test(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    step_percent: f64,
    hold_ms: u64,
    settle_ms: u64,
    repeats: u32,
) -> Result<String, String> {
    // Refuse to start a second run: see RUNNING for why a concurrent one can
    // leave the engine rich. Claim the slot before anything else touches the
    // ECU, and release it on every exit path via the guard.
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err(
            "An AFR delay test is already running. Stop it before starting another.".to_string(),
        );
    }
    let _run_guard = RunGuard;

    // Enrichment only: take the magnitude, then clamp. A negative input cannot
    // survive this, so no combination of arguments leans the engine out.
    let step_percent = step_percent.abs().clamp(MIN_STEP_PERCENT, MAX_STEP_PERCENT);
    let hold_ms = hold_ms.clamp(MIN_HOLD_MS, MAX_HOLD_MS);
    let settle_ms = settle_ms.clamp(MIN_HOLD_MS, MAX_HOLD_MS * 2);
    // `repeats == 0` means run until the operator stops it. Delay resolution
    // comes from the number of events in each rpm/load bin, not from the sample
    // rate - bootstrapping a real drive put a 34-event bin at +/-20 ms and a
    // 10-event bin at +/-80 ms - so a long run that keeps stepping while the
    // car is driven normally fills the map far better than a fixed batch.
    let continuous = repeats == 0;
    let requested_repeats = repeats;
    let repeats = if continuous {
        u32::MAX
    } else {
        repeats.clamp(1, MAX_REPEATS)
    };
    // What the UI is told the total is; 0 reads as "unbounded" there.
    let reported_total = if continuous { 0 } else { repeats };

    // Baseline = the tune's warm-plateau WUE value (the INI mandates 100).
    // Reading it up front also fails the run early if the ECU is unreachable,
    // instead of discovering that after a step has already been applied.
    let baseline = wue_slot(&state, None)
        .await
        .map_err(|e| format!("Could not read {WUE_CONSTANT}[{WUE_LAST_SLOT}] ({e})."))?;

    if baseline == 0 {
        return Err(format!(
            "{WUE_CONSTANT}[{WUE_LAST_SLOT}] reads 0, which is not a usable baseline"
        ));
    }

    let enriched_u8 = ((baseline as f64) * (1.0 + step_percent / 100.0)).round() as u16;
    let enriched_u8 = enriched_u8.min(255) as u8;
    if enriched_u8 <= baseline {
        return Err(format!(
            "A {step_percent:.1}% step on {baseline} does not change the value at this \
             resolution. Use a larger step."
        ));
    }
    let baseline = baseline as f64;
    let enriched = enriched_u8 as f64;

    abort_flag().store(false, Ordering::SeqCst);

    // One clock for the whole run: sample timestamps and step anchors must be
    // comparable for the delay extraction.
    let epoch = Instant::now();

    // Resolve the AFR/rpm/load channel names once from a live snapshot. When
    // unavailable (offline, or no AFR channel in this INI) the run proceeds as
    // a plain step test and every step reports a rejection instead of a delay.
    let channels = crate::commands::realtime_get::get_realtime_data(state.clone())
        .await
        .ok()
        .as_ref()
        .and_then(resolve_sample_channels);

    // Warm-plateau gate: the lever maps 1:1 onto fuelling only when the ECU
    // is already applying the last WUE slot. The ECU reports what it applies
    // via the warmupEnrich channel — a higher reading means the engine is
    // still on the warmup ramp and any measurement would be scaled and mushy.
    if let Some(ch) = channels.as_ref() {
        if let Some(wue_key) = &ch.wue {
            if let Ok(snap) = crate::commands::realtime_get::get_realtime_data(state.clone()).await
            {
                if let Some(applied) = snap.get(wue_key) {
                    if (applied - baseline).abs() > 2.0 {
                        return Err(format!(
                            "Engine is still in warmup: the ECU is applying {applied:.0}% \
                             warmup enrichment vs the warm-plateau value {baseline:.0}. \
                             Warm it up, then run the test."
                        ));
                    }
                }
            }
        }
    }

    // If the request was trimmed, say so here rather than quietly running
    // fewer steps than the operator asked for and than the UI estimated.
    let clamp_note = if !continuous && requested_repeats > repeats {
        format!(" (asked for {requested_repeats}, capped at {repeats})")
    } else {
        String::new()
    };
    let run_len = if continuous {
        "running until stopped".to_string()
    } else {
        format!("{repeats} steps{clamp_note}")
    };

    emit(
        &app,
        DelayTestProgress::plain(
            "starting",
            0,
            repeats,
            enriched,
            baseline,
            format!(
                "WUE step {baseline:.0}% -> {enriched:.0}% ({step_percent:.1}% richer), \
                 {run_len}, {hold_ms} ms hold. \
                 One byte, RAM only, never burned."
            ),
        ),
    );

    // Anything that leaves this function must first put `baseline` back. The
    // helper is used on the happy path, on abort, and on error.
    async fn restore(state: &tauri::State<'_, AppState>, baseline: f64) -> Result<(), String> {
        wue_slot(state, Some(baseline as u8)).await.map(|_| ())
    }

    /// Baseline window sampled immediately before each enrichment write.
    /// Sized for real-world sampling: live single-shot reads contend with the
    /// realtime stream for the serial link and land at ~5-9 Hz on hardware
    /// (not the ~20 Hz seen on the bench), so 600 ms could gather fewer than
    /// MIN_BASELINE_SAMPLES and reject every step. 1.5 s gives 7+ samples at
    /// the worst observed rate.
    const PRE_ROLL_MS: u64 = 1_500;

    let mut completed = 0u32;
    for step in 1..=repeats {
        if abort_flag().load(Ordering::SeqCst) {
            break;
        }

        emit(
            &app,
            DelayTestProgress::plain(
                "enriching",
                step,
                reported_total,
                enriched,
                baseline,
                format!("step {step}/{repeats}: hold steady"),
            ),
        );

        // Baseline trace for this step's delay extraction (abort-aware, like
        // every other wait in the run).
        let mut pre = StepSamples::default();
        let mut point = (None, None);
        if sample_window(
            &state,
            epoch,
            channels.as_ref(),
            PRE_ROLL_MS,
            &mut pre,
            &mut point,
            None,
        )
        .await
        {
            break;
        }

        if let Err(e) = wue_slot(&state, Some(enriched as u8)).await {
            // The write failed, so the ECU may or may not have taken it.
            // Restore regardless and stop.
            let restore_err = restore(&state, baseline).await.err();
            return Err(match restore_err {
                None => format!("Step {step} failed to apply ({e}). Baseline restored."),
                Some(r) => format!(
                    "Step {step} failed to apply ({e}) AND restoring the WUE slot to \
                     {baseline} also failed ({r}). CYCLE THE KEY to reload the stored tune."
                ),
            });
        }

        // Fallback anchor: the instant the write finished. Preferred anchor:
        // the ECU's own warmupEnrich channel crossing toward the stepped
        // value, captured during the hold sampling — the moment the ECU
        // actually started applying the step, immune to serial and
        // scheduling latency.
        let write_anchor_ms = epoch.elapsed().as_millis() as u64;
        let mut ecu_anchor_ms: Option<u64> = None;
        let wue_threshold = (baseline + enriched) / 2.0;

        // Abort-aware hold, sampling AFR throughout: an abort mid-hold falls
        // straight through to the restore below, so the enrichment is cut
        // short rather than held for the remainder of `hold_ms`.
        let mut post = StepSamples::default();
        let aborted_mid_hold = sample_window(
            &state,
            epoch,
            channels.as_ref(),
            hold_ms,
            &mut post,
            &mut point,
            Some((wue_threshold, &mut ecu_anchor_ms)),
        )
        .await;
        let anchor_ms = ecu_anchor_ms.unwrap_or(write_anchor_ms);

        if let Err(e) = restore(&state, baseline).await {
            return Err(format!(
                "Applied step {step} but could not restore the WUE slot to {baseline} ({e}). \
                 The engine is running RICH. CYCLE THE KEY to reload the stored tune."
            ));
        }

        if aborted_mid_hold {
            // Restored, but the step's hold was cut short — don't count it and
            // don't settle; the summary below reports the abort.
            break;
        }

        completed = step;

        // Extract this step's transport delay from the sampled traces.
        let measurement = if channels.is_some() {
            Some(detect_delay(anchor_ms, &pre.afr, &post.afr))
        } else {
            None
        };

        // The step's trace, for the UI to overlay on the ones before it.
        // Times are relative to the anchor so steps taken at different moments
        // line up, and pulse width rides along because the delay is read from
        // where AFR moves relative to where the FUEL moved - the commanded step
        // and the delivered step are not the same instant.
        let trace: Vec<TracePoint> = pre
            .afr
            .iter()
            .chain(post.afr.iter())
            .zip(pre.ctx.iter().chain(post.ctx.iter()))
            .map(|(s, c)| TracePoint {
                t_ms: s.t_ms as i64 - anchor_ms as i64,
                afr: s.afr,
                pw: c.pw,
            })
            .collect();
        // A step taken while fuel was cut, or while the throttle was moving,
        // is not measuring transport. Mark it so the UI can draw it greyed and
        // leave it out of the statistics rather than silently discarding it.
        let unusable = pre
            .ctx
            .iter()
            .chain(post.ctx.iter())
            .any(|c| c.dfco || c.tps_dot.abs() > TPS_DOT_LIMIT);

        let mut settling = DelayTestProgress::plain(
            "settling",
            step,
            reported_total,
            baseline,
            baseline,
            format!("step {step}/{repeats} done, settling"),
        );
        settling.trace = trace;
        settling.unusable = unusable;
        match measurement {
            Some(Ok(m)) => {
                let (rpm, load) = point;
                if let (Some(rpm), Some(load)) = (rpm, load) {
                    delay_samples()
                        .lock()
                        .map(|mut v| v.push((rpm, load, m.delay_ms)))
                        .ok();
                }
                settling.measured_delay_ms = Some(m.delay_ms);
                settling.measured_rpm = rpm;
                settling.measured_load = load;
                settling.message = format!(
                    "step {step}/{repeats}: delay {:.0} ms (AFR moved {:.2}), settling",
                    m.delay_ms, m.excursion
                );
            }
            Some(Err(rej)) => {
                settling.rejection = Some(rej.label().to_string());
                // "No response" and "still moving" both usually mean the hold
                // was short relative to the transport delay at this operating
                // point — the response simply had not arrived (or not finished
                // arriving) before enrichment was removed. Sampling continues
                // through the settle window below, so the next step's report
                // can say so with a number instead of leaving the operator to
                // guess which knob is wrong.
                let hint = matches!(
                    rej,
                    DelayRejection::NoResponse | DelayRejection::ResponseNotSettled
                )
                .then(|| format!(" — try a hold longer than {} ms", hold_ms))
                .unwrap_or_default();
                settling.message = format!(
                    "step {step}/{repeats}: no measurement ({}{hint}), settling",
                    rej.label()
                );
            }
            None => {
                settling.rejection = Some("no AFR channel / offline".to_string());
            }
        }
        emit(&app, settling);

        // Keep sampling through the settle window instead of sleeping through
        // it. The mixture returning to baseline is the second half of the same
        // event, and on a step whose delay approached the hold length the
        // response is still arriving here — sampling is what lets the next
        // report distinguish "the sensor saw nothing" from "the hold ended
        // before the exhaust gas got here". The samples are appended to the
        // step's trace so the UI overlay shows the full pulse.
        if step < repeats && channels.is_none() {
            // Nothing to sample without an AFR channel, so just wait — still
            // abortable within a tick.
            if sleep_abortable(settle_ms).await {
                break;
            }
        } else if step < repeats {
            let mut tail = StepSamples::default();
            let aborted_in_settle = sample_window(
                &state,
                epoch,
                channels.as_ref(),
                settle_ms,
                &mut tail,
                &mut point,
                None,
            )
            .await;
            if !tail.afr.is_empty() {
                let mut recovery = DelayTestProgress::plain(
                    "settling",
                    step,
                    reported_total,
                    baseline,
                    baseline,
                    format!("step {step}/{repeats} settled"),
                );
                recovery.trace = tail
                    .afr
                    .iter()
                    .zip(tail.ctx.iter())
                    .map(|(s, c)| TracePoint {
                        t_ms: s.t_ms as i64 - anchor_ms as i64,
                        afr: s.afr,
                        pw: c.pw,
                    })
                    .collect();
                emit(&app, recovery);
            }
            if aborted_in_settle {
                break;
            }
        }
    }

    // Belt and braces: restore once more on the way out, in case the loop was
    // broken by an abort between the enrich and the restore.
    restore(&state, baseline).await.map_err(|e| {
        format!(
            "Test finished but the final restore of the WUE slot to {baseline} failed ({e}). \
             CYCLE THE KEY to reload the stored tune."
        )
    })?;

    let aborted = abort_flag().load(Ordering::SeqCst);
    let summary = format!(
        "{} after {completed}/{repeats} steps. WUE slot restored to {baseline:.0}%. \
         Nothing was burned.",
        if aborted { "Aborted" } else { "Completed" }
    );

    emit(
        &app,
        DelayTestProgress::plain(
            if aborted { "aborted" } else { "complete" },
            completed,
            repeats,
            baseline,
            baseline,
            summary.clone(),
        ),
    );

    Ok(summary)
}

/// The session's accumulated rpm×load delay table, aggregated from every
/// successful step measurement since the last clear.
#[tauri::command]
pub async fn get_afr_delay_table() -> Result<DelayTable, String> {
    let samples = delay_samples()
        .lock()
        .map_err(|_| "delay sample store poisoned".to_string())?;
    let mut table = DelayTable::new();
    for (rpm, load, delay_ms) in samples.iter() {
        table.add(*rpm, *load, *delay_ms);
    }
    Ok(table)
}

/// Discard all accumulated delay measurements (e.g. after exhaust or sensor
/// changes that invalidate the map).
#[tauri::command]
pub async fn clear_afr_delay_samples() -> Result<(), String> {
    delay_samples()
        .lock()
        .map_err(|_| "delay sample store poisoned".to_string())?
        .clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The clamp is the safety boundary: no input may produce a lean step or an
    /// unbounded one. Mirrors the clamping in `run_afr_delay_test`.
    fn clamp_step(p: f64) -> f64 {
        p.abs().clamp(MIN_STEP_PERCENT, MAX_STEP_PERCENT)
    }

    #[test]
    fn negative_input_cannot_produce_a_lean_step() {
        // A negative percentage would lean the engine — the one outcome that
        // damages hardware. `abs` must make it an enrichment.
        assert_eq!(clamp_step(-8.0), 8.0);
        assert_eq!(clamp_step(-100.0), MAX_STEP_PERCENT);
        assert!(clamp_step(-0.001) >= MIN_STEP_PERCENT);
    }

    #[test]
    fn step_magnitude_is_bounded() {
        assert_eq!(clamp_step(1000.0), MAX_STEP_PERCENT);
        assert_eq!(clamp_step(0.5), MIN_STEP_PERCENT);
        assert_eq!(clamp_step(8.0), 8.0);
    }

    #[test]
    fn hold_and_settle_are_bounded() {
        assert_eq!(0u64.clamp(MIN_HOLD_MS, MAX_HOLD_MS), MIN_HOLD_MS);
        assert_eq!(u64::MAX.clamp(MIN_HOLD_MS, MAX_HOLD_MS), MAX_HOLD_MS);
    }

    /// Abort must interrupt a wait within a tick or two, not after the full
    /// duration — this is the "Abort button appears dead for 15 s over a rich
    /// engine" regression. Single test (not two) because the abort flag is a
    /// process-wide static shared with any parallel test.
    #[tokio::test]
    async fn abort_interrupts_a_long_wait_quickly() {
        // Phase 1: no abort — the full (short) wait elapses, returns false.
        abort_flag().store(false, Ordering::SeqCst);
        assert!(!sleep_abortable(120).await);

        // Phase 2: abort pre-set — a 5 s wait must return true immediately.
        abort_flag().store(true, Ordering::SeqCst);
        let started = std::time::Instant::now();
        assert!(sleep_abortable(5_000).await);
        assert!(
            started.elapsed() < Duration::from_millis(1_000),
            "abort took {:?} to interrupt the wait",
            started.elapsed()
        );

        abort_flag().store(false, Ordering::SeqCst);
    }

    /// The enriched value must round to the constant's 0.1 resolution and be
    /// strictly richer, otherwise the step is invisible to the ECU.
    #[test]
    fn enriched_value_rounds_to_resolution_and_is_richer() {
        let baseline: f64 = 12.6;
        let enriched = (baseline * 1.08 * 10.0).round() / 10.0;
        assert!(enriched > baseline, "must be richer");
        assert!((enriched - 13.6).abs() < 1e-9, "got {enriched}");
        // reqFuel is stored at 0.1 ms resolution, so a value that does not land
        // on that grid would be silently truncated by the ECU write.
        assert!(
            ((enriched * 10.0) - (enriched * 10.0).round()).abs() < 1e-9,
            "must land on the 0.1 ms grid"
        );
    }
}
