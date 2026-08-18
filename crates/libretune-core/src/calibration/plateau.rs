//! Sequencer-assisted wideband auto-calibration.
//!
//! A 14Point7 Spartan 2 runs a fixed self-test at power-on: it drives its
//! analogue output to 1.666 V (= 13.328 AFR on the sensor's own nominal
//! 0–5 V ↔ 10–20 AFR scale) for about five seconds, then 3.333 V
//! (= 16.666 AFR) for another five, before switching to live readings. Those
//! two levels are generated inside the sensor and owe nothing to what is in
//! the exhaust, so they are a free two-point reference: whatever the ECU
//! *reads* during those windows, minus what the sensor was *sending*, is the
//! error in the wiring and the ADC path — most of it ground offset.
//!
//! TunerStudio cannot do this at all; it only lets you type a calibration in.
//! Here we watch the live AFR channel through a key-on, recover the voltage
//! the ECU actually saw (by inverting the calibration currently loaded), and
//! solve the line that would have made those two windows read correctly.
//!
//! ## Why the engine may be running
//!
//! The firmware requires a stopped engine for the calibration *write* (it
//! flushes serial and blocks), and this module's callers enforce that. But
//! detection deliberately does not gate on RPM: the sequence starts at
//! power-on and frequently overlaps cranking and the first seconds of
//! idle — the reference capture this module is tested against has the engine
//! running at ~850 rpm through both plateaus. Since the plateau voltages are
//! sensor-generated, that overlap is harmless, and rejecting it would throw
//! away the common case.

use super::{CalibrationError, LinearWideband, ADC_COUNTS, ADC_REFERENCE_VOLTS};

/// Nominal Spartan 2 startup levels: `(volts, afr)` for each plateau.
pub const SPARTAN2_PLATEAU_1: (f64, f64) = (1.666, 13.328);
/// Second Spartan 2 startup level.
pub const SPARTAN2_PLATEAU_2: (f64, f64) = (3.333, 16.666);

/// One sample of the live AFR channel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AfrSample {
    /// Seconds since the start of the log.
    pub time_s: f64,
    /// AFR as the ECU reported it, through whatever calibration is loaded.
    pub afr: f64,
}

/// Tunables for the plateau detector.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlateauDetectorConfig {
    /// Shortest run accepted as a plateau. The nominal window is 5 s, but the
    /// leading edge is eaten by `ADCFILTER_O2` smoothing and by however late
    /// logging started, so this is generously short.
    pub min_duration_s: f64,
    /// Longest run accepted. A level that holds far longer than the sequence
    /// is steady-state running, not a plateau.
    pub max_duration_s: f64,
    /// Widest AFR spread tolerated inside one run.
    pub tolerance_afr: f64,
    /// Trailing fraction of each run averaged to get its settled value —
    /// the head of the window is still ramping through the input filter.
    pub settle_fraction: f64,
    /// Reject a run whose recovered voltage is within this margin of either
    /// supply rail; a channel pinned at the end of its scale is a
    /// disconnected or saturated input, not a plateau.
    pub rail_margin_volts: f64,
    /// The two plateaus must be at least this far apart in recovered volts.
    pub min_separation_volts: f64,
}

impl Default for PlateauDetectorConfig {
    fn default() -> Self {
        Self {
            min_duration_s: 2.5,
            max_duration_s: 9.0,
            tolerance_afr: 0.25,
            settle_fraction: 0.5,
            rail_margin_volts: 0.2,
            min_separation_volts: 0.5,
        }
    }
}

/// A calibration solved from a detected plateau pair.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlateauCalibration {
    /// The corrected transfer function to write.
    pub calibration: LinearWideband,
    /// Per plateau: the settled AFR the ECU reported.
    pub observed_afr: (f64, f64),
    /// Per plateau: the input voltage that reading implies, recovered
    /// through the calibration that was loaded at capture time.
    pub observed_volts: (f64, f64),
    /// Per plateau: `(start, end)` seconds of the detected window.
    pub windows: ((f64, f64), (f64, f64)),
    /// Mean of (recovered volts − sensor's nominal volts) across the two
    /// plateaus: the constant input offset this calibration cancels. A
    /// positive value means the ECU sees the sensor as higher than it is,
    /// which is the signature of a ground offset between the two.
    pub ground_offset_volts: f64,
    /// How far apart the two plateaus' implied offsets are. A pure offset
    /// gives near-zero here; a large value means the error is not a simple
    /// offset and the fit should be treated with suspicion.
    pub offset_consistency_volts: f64,
}

impl PlateauCalibration {
    /// One-line description for logs and the tune's stored metadata.
    pub fn describe(&self) -> String {
        format!(
            "{} — auto-calibrated from sensor startup plateaus \
             (read {:.2}/{:.2} AFR where the sensor sent {:.3}/{:.3} AFR; \
             ground offset {:+.3} V)",
            self.calibration.describe(),
            self.observed_afr.0,
            self.observed_afr.1,
            SPARTAN2_PLATEAU_1.1,
            SPARTAN2_PLATEAU_2.1,
            self.ground_offset_volts,
        )
    }
}

/// A candidate flat run in the AFR trace.
#[derive(Debug, Clone, Copy)]
struct Run {
    start_s: f64,
    end_s: f64,
    settled_afr: f64,
    volts: f64,
}

/// Recover the input voltage that produced an observed AFR, by inverting the
/// calibration curve that was loaded when the sample was taken.
///
/// `curve` is AFR per ADC count (index 0..1023). Inversion is a scan for the
/// bracketing pair plus linear interpolation, so it works for the non-linear
/// presets too, not just straight lines. Returns `None` if the reading falls
/// outside the curve's range — which is what a rail-pinned channel does.
pub fn invert_curve(curve: &[f64], afr: f64) -> Option<f64> {
    if curve.len() < 2 {
        return None;
    }
    for i in 0..curve.len() - 1 {
        let (a, b) = (curve[i], curve[i + 1]);
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        if afr >= lo && afr <= hi {
            let span = b - a;
            let frac = if span.abs() < 1e-12 {
                0.0
            } else {
                (afr - a) / span
            };
            let adc = i as f64 + frac;
            return Some(adc * ADC_REFERENCE_VOLTS / (ADC_COUNTS as f64 - 1.0));
        }
    }
    None
}

/// Find flat runs in the trace and recover each one's input voltage.
fn find_runs(samples: &[AfrSample], curve: &[f64], cfg: &PlateauDetectorConfig) -> Vec<Run> {
    let mut runs = Vec::new();
    let mut start = 0usize;

    let close_run = |start: usize, end_exclusive: usize, runs: &mut Vec<Run>| {
        if end_exclusive <= start {
            return;
        }
        let group = &samples[start..end_exclusive];
        let (first, last) = (group[0], group[group.len() - 1]);
        let duration = last.time_s - first.time_s;
        if duration < cfg.min_duration_s || duration > cfg.max_duration_s {
            return;
        }
        // Average only the settled tail: the input filter is still catching
        // up across the leading edge of each step.
        let cutoff = last.time_s - duration * cfg.settle_fraction;
        let tail: Vec<f64> = group
            .iter()
            .filter(|s| s.time_s >= cutoff)
            .map(|s| s.afr)
            .collect();
        if tail.is_empty() {
            return;
        }
        let settled = tail.iter().sum::<f64>() / tail.len() as f64;

        let Some(volts) = invert_curve(curve, settled) else {
            return;
        };
        if volts < cfg.rail_margin_volts || volts > ADC_REFERENCE_VOLTS - cfg.rail_margin_volts {
            return;
        }
        runs.push(Run {
            start_s: first.time_s,
            end_s: last.time_s,
            settled_afr: settled,
            volts,
        });
    };

    for i in 1..samples.len() {
        if (samples[i].afr - samples[start].afr).abs() > cfg.tolerance_afr {
            close_run(start, i, &mut runs);
            start = i;
        }
    }
    close_run(start, samples.len(), &mut runs);
    runs
}

/// Detect the sensor's startup plateaus in a captured AFR trace and solve the
/// calibration that would have made them read correctly.
///
/// * `samples` — the live AFR channel, in time order.
/// * `current_curve` — the 1024-entry AFR-per-ADC-count curve that was loaded
///   in the ECU when the trace was captured. This is what makes the result
///   absolute rather than relative: the observed AFR is meaningless on its
///   own, but run backwards through the loaded curve it gives the voltage the
///   ADC actually saw.
///
/// The two plateaus must be *consecutive* detected runs, rising, and far
/// enough apart in voltage — which is what distinguishes the real sequence
/// from a coincidentally steady idle.
pub fn auto_calibrate_from_plateaus(
    samples: &[AfrSample],
    current_curve: &[f64],
    cfg: &PlateauDetectorConfig,
) -> Result<PlateauCalibration, CalibrationError> {
    if samples.len() < 2 {
        return Err(CalibrationError::DegenerateInputs(
            "need a captured AFR trace to auto-calibrate from".to_string(),
        ));
    }

    let runs = find_runs(samples, current_curve, cfg);
    let pair = runs
        .windows(2)
        .filter(|w| w[1].volts - w[0].volts >= cfg.min_separation_volts)
        .min_by(|a, b| {
            // Of the rising pairs, prefer the one whose voltage ratio is
            // closest to the sensor's nominal 3.333/1.666 = 2.0.
            let expected = SPARTAN2_PLATEAU_2.0 / SPARTAN2_PLATEAU_1.0;
            let score = |w: &[Run]| (w[1].volts / w[0].volts - expected).abs();
            score(a)
                .partial_cmp(&score(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .ok_or_else(|| {
            CalibrationError::DegenerateInputs(format!(
                "no pair of rising sensor startup plateaus found in {:.1} s of \
                 AFR data ({} steady run(s) detected). Capture from key-on, \
                 with logging already running.",
                samples[samples.len() - 1].time_s - samples[0].time_s,
                runs.len()
            ))
        })?;

    let (p1, p2) = (pair[0], pair[1]);
    let calibration = LinearWideband::new(
        (p1.volts, SPARTAN2_PLATEAU_1.1),
        (p2.volts, SPARTAN2_PLATEAU_2.1),
    )?;

    let offset1 = p1.volts - SPARTAN2_PLATEAU_1.0;
    let offset2 = p2.volts - SPARTAN2_PLATEAU_2.0;

    Ok(PlateauCalibration {
        calibration,
        observed_afr: (p1.settled_afr, p2.settled_afr),
        observed_volts: (p1.volts, p2.volts),
        windows: ((p1.start_s, p1.end_s), (p2.start_s, p2.end_s)),
        ground_offset_volts: (offset1 + offset2) / 2.0,
        offset_consistency_volts: (offset1 - offset2).abs(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The calibration that was loaded in the NA6 when the reference capture
    /// was taken: the INI's stock 14Point7 preset.
    fn stock_14point7_curve() -> Vec<f64> {
        (0..ADC_COUNTS)
            .map(|adc| 10.0001 + adc as f64 * 0.0097752)
            .collect()
    }

    fn trace(steps: &[(f64, f64, f64)]) -> Vec<AfrSample> {
        // (start_s, end_s, afr) at 10 Hz, the car's actual log rate.
        let mut out = Vec::new();
        for (start, end, afr) in steps {
            let mut t = *start;
            while t < *end {
                out.push(AfrSample {
                    time_s: t,
                    afr: *afr,
                });
                t += 0.1;
            }
        }
        out
    }

    #[test]
    fn invert_curve_round_trips_the_stock_preset() {
        let curve = stock_14point7_curve();
        // 14Point7 spans ~10.0 to ~20.0 across 0-5 V, so 15.0 sits mid-scale.
        let v = invert_curve(&curve, 15.0).unwrap();
        assert!(v > 2.4 && v < 2.6, "15.0 AFR recovered as {v} V");
        // Out of range readings must not silently clamp to a rail.
        assert!(invert_curve(&curve, 5.0).is_none());
        assert!(invert_curve(&curve, 30.0).is_none());
    }

    #[test]
    fn solves_the_hand_validated_na6_calibration() {
        // Synthetic version of the real key-on: the two plateaus read 13.60
        // and 16.90 through the stock preset. Hand-solved on the car, this
        // gives 9.69 AFR at 0 V and 19.80 at 5 V.
        let samples = trace(&[
            (0.0, 4.0, 13.60),
            (4.0, 9.0, 16.90),
            (9.0, 14.0, 14.90), // live reading once the sequence ends
        ]);
        let result =
            auto_calibrate_from_plateaus(&samples, &stock_14point7_curve(), &Default::default())
                .unwrap();

        let at_0v = result.calibration.afr_at_volts(0.0);
        let at_5v = result.calibration.afr_at_volts(5.0);
        assert!(
            (at_0v - 9.69).abs() < 0.05,
            "0 V calibration point was {at_0v}, expected ~9.69"
        );
        assert!(
            (at_5v - 19.80).abs() < 0.05,
            "5 V calibration point was {at_5v}, expected ~19.80"
        );
        // The error really is a near-constant offset, so the two plateaus
        // must agree on it.
        assert!(
            result.offset_consistency_volts < 0.02,
            "plateaus disagreed on the offset by {} V",
            result.offset_consistency_volts
        );
        assert!(
            result.ground_offset_volts > 0.05 && result.ground_offset_volts < 0.25,
            "implied ground offset {} V is outside the plausible range",
            result.ground_offset_volts
        );
    }

    #[test]
    fn a_channel_pinned_at_the_rail_is_not_a_plateau() {
        // A disconnected wideband sits at the bottom of the scale forever.
        // Two such "plateaus" must not be mistaken for the startup sequence.
        let samples = trace(&[(0.0, 5.0, 10.0), (5.0, 10.0, 10.0)]);
        let err = auto_calibrate_from_plateaus(&samples, &stock_14point7_curve(), &Default::default())
            .unwrap_err();
        assert!(matches!(err, CalibrationError::DegenerateInputs(_)));
    }

    #[test]
    fn a_steady_idle_alone_is_not_a_plateau_pair() {
        let samples = trace(&[(0.0, 20.0, 14.7)]);
        assert!(
            auto_calibrate_from_plateaus(&samples, &stock_14point7_curve(), &Default::default())
                .is_err()
        );
    }

    #[test]
    fn falling_pairs_are_rejected() {
        // The sequence rises. A high-then-low pair is the tail of a sequence
        // plus a live reading, and fitting it would invert the curve.
        let samples = trace(&[(0.0, 4.0, 16.90), (4.0, 9.0, 13.60)]);
        assert!(
            auto_calibrate_from_plateaus(&samples, &stock_14point7_curve(), &Default::default())
                .is_err()
        );
    }

    #[test]
    fn an_already_correct_sensor_yields_a_near_identity_offset() {
        // If the wiring were perfect the ECU would read exactly what the
        // sensor sends, and the solved offset should be ~0.
        let curve = stock_14point7_curve();
        let afr1 = curve[(SPARTAN2_PLATEAU_1.0 / 5.0 * 1023.0).round() as usize];
        let afr2 = curve[(SPARTAN2_PLATEAU_2.0 / 5.0 * 1023.0).round() as usize];
        let samples = trace(&[(0.0, 4.0, afr1), (4.0, 9.0, afr2)]);
        let result =
            auto_calibrate_from_plateaus(&samples, &curve, &Default::default()).unwrap();
        assert!(
            result.ground_offset_volts.abs() < 0.01,
            "offset {} V should be ~0 for a perfect install",
            result.ground_offset_volts
        );
    }
}
