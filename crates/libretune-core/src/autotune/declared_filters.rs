//! Evaluating the filters an INI declares, as they are meant to be applied.
//!
//! `[VeAnalyze]` lists them as `name, label, channel, operator, value, , enabled`:
//!
//! ```text
//! filter = minCltFilter,  "Minimum CLT", coolant,    <, 71, , true
//! filter = accelFilter,   "Accel Flag",  engine,     &, 16, , false
//! filter = aseFilter,     "ASE Flag",    engine,     &, 4,  , false
//! filter = overrunFilter, "Overrun",     pulseWidth, =, 0,  , false
//! ```
//!
//! Two things about this are easy to get wrong, and both change the result
//! silently rather than loudly.
//!
//! **Each line states a REJECT condition, not an accept one.** `coolant < 71`
//! discards samples below 71; it does not require them to be below it. Read the
//! other way, a warm-up filter becomes a warm-up-only filter and every cell is
//! tuned on cold data.
//!
//! **`&` is a bitmask test, not equality.** `engine & 16` rejects a sample when
//! bit 4 of the engine status byte is set — that is Speeduino's accel-enrichment
//! flag. Treated as `==`, it matches only the exact value 16, so a sample with
//! accel *and* warm-up set (engine = 24) sails through the filter that exists to
//! catch it.
//!
//! LibreTune previously applied none of these, using hardcoded thresholds
//! instead — which is how a Celsius project ran with a coolant limit of 160,
//! above boiling, rejecting every sample of a session without a word.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// How a declared filter compares its channel against its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredOp {
    /// Reject when the channel is below the value.
    LessThan,
    /// Reject when the channel is above the value.
    GreaterThan,
    /// Reject when the channel equals the value.
    Equal,
    /// Reject when the channel is not equal to the value.
    NotEqual,
    /// Reject when any bit of the value is set in the channel.
    BitwiseAnd,
}

/// One filter, as declared by the INI and possibly adjusted by the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeclaredFilterSpec {
    pub name: String,
    pub channel: String,
    pub operator: DeclaredOp,
    pub value: f64,
    /// Filters the INI ships disabled stay off until the user turns them on.
    pub enabled: bool,
}

impl DeclaredFilterSpec {
    /// Does this filter reject `sample`?
    ///
    /// A missing channel does NOT reject. A filter that cannot see its input is
    /// not evidence the sample is bad, and rejecting on absence would silently
    /// discard an entire session on any INI whose channel names differ.
    pub fn rejects(&self, sample: &HashMap<String, f64>) -> bool {
        if !self.enabled {
            return false;
        }
        let Some(&v) = sample.get(&self.channel) else {
            return false;
        };
        match self.operator {
            DeclaredOp::LessThan => v < self.value,
            DeclaredOp::GreaterThan => v > self.value,
            // Channels arrive as f64 even when they are integers, so compare
            // with a tolerance rather than `==`, which would miss 0.0000001.
            DeclaredOp::Equal => (v - self.value).abs() < 1e-6,
            DeclaredOp::NotEqual => (v - self.value).abs() >= 1e-6,
            // Bitmask: both sides are integer flags carried in a float.
            DeclaredOp::BitwiseAnd => {
                let bits = v.round() as i64;
                let mask = self.value.round() as i64;
                mask != 0 && (bits & mask) != 0
            }
        }
    }
}

/// The first filter that rejects `sample`, or `None` if it passes them all.
///
/// Returns which one so a caller can report *why* a session collected nothing —
/// "0 samples accepted" on its own sends people looking at the wiring.
pub fn first_rejecting<'a>(
    filters: &'a [DeclaredFilterSpec],
    sample: &HashMap<String, f64>,
) -> Option<&'a DeclaredFilterSpec> {
    filters.iter().find(|f| f.rejects(sample))
}

/// Count rejections per filter across many samples.
///
/// The useful diagnostic when a session under-collects: it names the filter
/// doing the discarding instead of leaving the tuner to guess.
pub fn rejection_tally(
    filters: &[DeclaredFilterSpec],
    samples: &[HashMap<String, f64>],
) -> Vec<(String, usize)> {
    let mut tally: Vec<(String, usize)> = filters.iter().map(|f| (f.name.clone(), 0)).collect();
    for s in samples {
        if let Some(f) = first_rejecting(filters, s) {
            if let Some(e) = tally.iter_mut().find(|(n, _)| *n == f.name) {
                e.1 += 1;
            }
        }
    }
    tally
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn spec(name: &str, channel: &str, op: DeclaredOp, value: f64) -> DeclaredFilterSpec {
        DeclaredFilterSpec {
            name: name.into(),
            channel: channel.into(),
            operator: op,
            value,
            enabled: true,
        }
    }

    /// The car's own INI, under `#if CELSIUS`: coolant < 71 discards warm-up.
    #[test]
    fn min_clt_rejects_below_the_threshold_not_above() {
        let f = spec("minCltFilter", "coolant", DeclaredOp::LessThan, 71.0);
        assert!(
            f.rejects(&sample(&[("coolant", 40.0)])),
            "cold must be rejected"
        );
        assert!(
            !f.rejects(&sample(&[("coolant", 85.0)])),
            "warm must be kept"
        );
        assert!(
            !f.rejects(&sample(&[("coolant", 71.0)])),
            "exactly at the limit is kept"
        );
    }

    /// The bit that makes `&` different from `=`. Speeduino's engine byte sets
    /// bit 4 (16) for accel enrichment and bit 3 (8) for warm-up; a sample with
    /// both reads 24, which `==16` would let straight through.
    #[test]
    fn a_bitmask_filter_catches_the_flag_among_others() {
        let f = spec("accelFilter", "engine", DeclaredOp::BitwiseAnd, 16.0);
        assert!(f.rejects(&sample(&[("engine", 16.0)])), "accel alone");
        assert!(
            f.rejects(&sample(&[("engine", 24.0)])),
            "accel + warm-up: this is the case equality would miss"
        );
        assert!(f.rejects(&sample(&[("engine", 17.0)])), "accel + running");
        assert!(
            !f.rejects(&sample(&[("engine", 9.0)])),
            "warm-up + running, no accel"
        );
        assert!(!f.rejects(&sample(&[("engine", 1.0)])), "running only");
    }

    #[test]
    fn the_ase_flag_uses_a_different_bit() {
        let ase = spec("aseFilter", "engine", DeclaredOp::BitwiseAnd, 4.0);
        assert!(ase.rejects(&sample(&[("engine", 5.0)])), "ASE + running");
        assert!(
            !ase.rejects(&sample(&[("engine", 17.0)])),
            "accel + running is not ASE"
        );
    }

    /// `pulseWidth = 0` is how overrun is spotted: injectors off.
    #[test]
    fn overrun_is_an_equality_test_on_pulse_width() {
        let f = spec("overrunFilter", "pulseWidth", DeclaredOp::Equal, 0.0);
        assert!(f.rejects(&sample(&[("pulseWidth", 0.0)])));
        assert!(!f.rejects(&sample(&[("pulseWidth", 3.2)])));
    }

    /// A filter that cannot see its channel must not discard the session.
    #[test]
    fn a_missing_channel_does_not_reject() {
        let f = spec("minCltFilter", "coolant", DeclaredOp::LessThan, 71.0);
        assert!(
            !f.rejects(&sample(&[("rpm", 3000.0)])),
            "no coolant channel is not evidence the sample is bad"
        );
    }

    #[test]
    fn a_disabled_filter_never_rejects() {
        let mut f = spec("accelFilter", "engine", DeclaredOp::BitwiseAnd, 16.0);
        f.enabled = false;
        assert!(!f.rejects(&sample(&[("engine", 16.0)])));
    }

    #[test]
    fn a_zero_mask_rejects_nothing() {
        // Otherwise `x & 0` would be treated as "always matches" by a naive
        // implementation that only checked for inequality.
        let f = spec("bogus", "engine", DeclaredOp::BitwiseAnd, 0.0);
        assert!(!f.rejects(&sample(&[("engine", 255.0)])));
    }

    #[test]
    fn the_first_rejecting_filter_is_named() {
        let filters = vec![
            spec("minCltFilter", "coolant", DeclaredOp::LessThan, 71.0),
            spec("accelFilter", "engine", DeclaredOp::BitwiseAnd, 16.0),
        ];
        let cold_and_accel = sample(&[("coolant", 30.0), ("engine", 16.0)]);
        assert_eq!(
            first_rejecting(&filters, &cold_and_accel).map(|f| f.name.as_str()),
            Some("minCltFilter"),
            "reports the first, so the tuner fixes the outermost cause first"
        );
        let warm_ok = sample(&[("coolant", 85.0), ("engine", 1.0)]);
        assert!(first_rejecting(&filters, &warm_ok).is_none());
    }

    /// The diagnostic that answers "why did this session collect nothing?".
    #[test]
    fn the_tally_names_which_filter_did_the_discarding() {
        let filters = vec![
            spec("minCltFilter", "coolant", DeclaredOp::LessThan, 71.0),
            spec("accelFilter", "engine", DeclaredOp::BitwiseAnd, 16.0),
        ];
        let samples = vec![
            sample(&[("coolant", 30.0), ("engine", 1.0)]),
            sample(&[("coolant", 40.0), ("engine", 1.0)]),
            sample(&[("coolant", 85.0), ("engine", 16.0)]),
            sample(&[("coolant", 85.0), ("engine", 1.0)]),
        ];
        let tally = rejection_tally(&filters, &samples);
        assert_eq!(
            tally,
            vec![("minCltFilter".into(), 2), ("accelFilter".into(), 1)]
        );
    }
}
