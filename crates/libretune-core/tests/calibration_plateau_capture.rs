//! The plateau auto-calibrator against a real key-on capture.
//!
//! Fixture: `fixtures/calibration/spartan2_keyon_2026-08-14.csv` — the AFR,
//! RPM and time columns of an actual NA6 datalog
//! (`2026-08-14_22.44.51.csv`), trimmed to the first 25 s and re-based to
//! t=0. The 14Point7 Spartan 2 runs its startup sequence across this window
//! and the calibration it implies was solved by hand on the car: 9.69 AFR at
//! 0 V, 19.80 at 5 V.
//!
//! This is the test that stops the detector regressing into something that
//! only works on synthetic square waves. The real trace is messier than the
//! spec in three ways that all bit during development:
//!
//! * the ECU's `ADCFILTER_O2` smoothing ramps into each level rather than
//!   stepping, so the plateau's leading edge is unusable;
//! * the trace opens with the channel pinned at the bottom of the loaded
//!   calibration (10.0 AFR) for 3.5 s, which is flat, long enough, and not a
//!   plateau;
//! * the engine is *running* through both plateaus (~850 rpm) — see the
//!   module docs on why detection must not gate on that.

use libretune_core::calibration::plateau::{
    auto_calibrate_from_plateaus, AfrSample, PlateauDetectorConfig,
};
use libretune_core::calibration::ADC_COUNTS;

/// The calibration loaded in the ECU when this log was captured: the INI's
/// stock `14Point7` preset, `10.0001 + adcValue * 0.0097752`.
fn stock_14point7_curve() -> Vec<f64> {
    (0..ADC_COUNTS)
        .map(|adc| 10.0001 + adc as f64 * 0.0097752)
        .collect()
}

struct Capture {
    samples: Vec<AfrSample>,
    max_rpm: f64,
}

fn load_capture() -> Capture {
    let csv = include_str!("fixtures/calibration/spartan2_keyon_2026-08-14.csv");
    let mut samples = Vec::new();
    let mut max_rpm: f64 = 0.0;
    for line in csv.lines().skip(1).filter(|l| !l.trim().is_empty()) {
        let cols: Vec<&str> = line.split(',').collect();
        assert_eq!(cols.len(), 3, "unexpected fixture row: {line}");
        samples.push(AfrSample {
            time_s: cols[0].parse().expect("time_s"),
            afr: cols[1].parse().expect("afr"),
        });
        max_rpm = max_rpm.max(cols[2].parse::<f64>().expect("rpm"));
    }
    Capture { samples, max_rpm }
}

#[test]
fn solves_the_hand_validated_calibration_from_the_real_capture() {
    let capture = load_capture();
    let result = auto_calibrate_from_plateaus(
        &capture.samples,
        &stock_14point7_curve(),
        &PlateauDetectorConfig::default(),
    )
    .expect("the startup plateaus should be found in the reference capture");

    // What the ECU reported during the two windows.
    assert!(
        (result.observed_afr.0 - 13.6).abs() < 0.15,
        "first plateau settled at {} AFR, expected ~13.6",
        result.observed_afr.0
    );
    assert!(
        (result.observed_afr.1 - 16.9).abs() < 0.15,
        "second plateau settled at {} AFR, expected ~16.9",
        result.observed_afr.1
    );

    // The calibration those readings imply, versus the one solved by hand.
    let at_0v = result.calibration.afr_at_volts(0.0);
    let at_5v = result.calibration.afr_at_volts(5.0);
    assert!(
        (at_0v - 9.69).abs() < 0.2,
        "0 V point solved as {at_0v}, hand-validated value is 9.69"
    );
    assert!(
        (at_5v - 19.80).abs() < 0.2,
        "5 V point solved as {at_5v}, hand-validated value is 19.80"
    );

    // The fault really is a ground offset, so both plateaus must imply the
    // same one. If this ever fails the fit is picking up something else and
    // the result should not be offered as a one-click apply.
    assert!(
        result.offset_consistency_volts < 0.05,
        "plateaus disagreed on the implied offset by {} V",
        result.offset_consistency_volts
    );
    assert!(
        (result.ground_offset_volts - 0.125).abs() < 0.06,
        "implied ground offset {} V, expected ~+0.125 V",
        result.ground_offset_volts
    );
}

#[test]
fn the_reference_capture_has_the_engine_running() {
    // Guards the design decision, not the code: if this fixture were ever
    // replaced with an engine-off capture, the "detection must not gate on
    // RPM" reasoning would silently stop being exercised.
    let capture = load_capture();
    assert!(
        capture.max_rpm > 500.0,
        "fixture should contain a running engine, saw max {} rpm",
        capture.max_rpm
    );
}

#[test]
fn detected_windows_land_where_the_sequence_actually_is() {
    let capture = load_capture();
    let result = auto_calibrate_from_plateaus(
        &capture.samples,
        &stock_14point7_curve(),
        &PlateauDetectorConfig::default(),
    )
    .unwrap();

    let (first, second) = result.windows;
    // The opening 3.5 s of rail-pinned 10.0 AFR must not have been taken as
    // the first plateau.
    assert!(
        first.0 > 4.0,
        "first plateau starts at {} s — that is the rail-pinned opening, \
         not the sequence",
        first.0
    );
    assert!(
        second.0 > first.1 - 0.5,
        "plateaus should be consecutive, got {first:?} then {second:?}"
    );
    // Both windows sit inside the ~10 s startup sequence.
    assert!(
        second.1 < 16.0,
        "second plateau ends at {} s, past the startup sequence",
        second.1
    );
}
