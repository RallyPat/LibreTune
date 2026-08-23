//! Checks run before an AutoTune session starts, and what to tell the user.
//!
//! AutoTune fails quietly. It will start against a missing target table, a
//! filter that rejects every sample, or an authority limit of zero, and report
//! nothing wrong — the session simply produces recommendations that are absent,
//! wrong, or identical to what you started with. Every entry here is a failure
//! that has actually happened rather than one imagined for completeness:
//!
//! * a target table that never resolved, so every cell was tuned to a flat 14.7
//!   including wide-open throttle
//! * `min_clt` at its 160 default on a Celsius project — above boiling, so not
//!   one sample can pass
//! * a target table of the wrong shape, quietly flattening the high-load corner
//!
//! The point is to say so before the drive, not to explain it afterwards.

use super::{AutoTuneAuthorityLimits, AutoTuneFilters, AutoTuneSettings};
use serde::{Deserialize, Serialize};

/// How much a finding matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// AutoTune cannot produce a usable result. Starting is a wasted drive.
    Blocker,
    /// It will run, but the result is likely to be wrong or misleading.
    Warning,
    /// Worth knowing before starting.
    Info,
}

/// One thing worth telling the user before they start.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub severity: Severity,
    /// Stable identifier, so the UI can offer a specific fix.
    pub code: String,
    /// One line, in the user's terms.
    pub title: String,
    /// Why it matters, concretely.
    pub detail: String,
    /// What the value is now, where there is one.
    pub current: Option<String>,
    /// What it should be, where that can be worked out.
    pub suggested: Option<String>,
}

impl Finding {
    fn new(severity: Severity, code: &str, title: &str, detail: String) -> Self {
        Self {
            severity,
            code: code.to_string(),
            title: title.to_string(),
            detail,
            current: None,
            suggested: None,
        }
    }

    fn with_values(mut self, current: impl Into<String>, suggested: impl Into<String>) -> Self {
        self.current = Some(current.into());
        self.suggested = Some(suggested.into());
        self
    }
}

/// Everything the pure checks need to know about the session being started.
///
/// Deliberately plain data rather than the app's live state, so the rules can be
/// tested without a connection, a definition, or a running engine.
#[derive(Debug, Clone)]
pub struct PreflightInput<'a> {
    /// Was a per-cell AFR target table resolved, and under what name?
    pub target_table: Option<&'a str>,
    /// Its values, for a sanity check on units and range.
    pub target_values: &'a [Vec<f64>],
    /// VE table shape as (rows, cols); the target must match it.
    pub ve_shape: (usize, usize),
    /// VE table values, to catch an unread or empty table.
    pub ve_values: &'a [Vec<f64>],
    /// Is the INI in Celsius? Decides what a sane `min_clt` looks like.
    pub celsius: bool,
    /// Tables that could serve as an AFR target, for the fix prompt.
    pub candidate_target_tables: Vec<String>,
    /// Will accepted changes be written to the ECU, or only collected?
    pub will_write_to_ecu: bool,
}

/// Run every check. Sorted severity-first so the UI can render top-down.
pub fn check(
    input: &PreflightInput,
    settings: &AutoTuneSettings,
    filters: &AutoTuneFilters,
    authority: &AutoTuneAuthorityLimits,
) -> Vec<Finding> {
    let mut out = Vec::new();
    check_target_table(input, settings, &mut out);
    check_ve_table(input, &mut out);
    check_filters(input, filters, &mut out);
    check_authority(authority, &mut out);
    check_delay(settings, &mut out);
    check_write_mode(input, &mut out);
    out.sort_by_key(|f| match f.severity {
        Severity::Blocker => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
    });
    out
}

fn check_target_table(input: &PreflightInput, settings: &AutoTuneSettings, out: &mut Vec<Finding>) {
    let Some(name) = input.target_table else {
        out.push(
            Finding::new(
                Severity::Blocker,
                "target_table_missing",
                "No AFR target table",
                format!(
                    "Every cell will be tuned to a flat {:.1} AFR, including full load. \
                     If the engine is meant to run richer under load, this asks AutoTune \
                     to remove fuel exactly where it is most dangerous to do so.",
                    settings.target_afr
                ),
            )
            .with_values(
                format!("flat {:.1}", settings.target_afr),
                "pick a target table",
            ),
        );
        return;
    };

    let rows = input.target_values.len();
    let cols = input.target_values.first().map(|r| r.len()).unwrap_or(0);
    if (rows, cols) != input.ve_shape {
        out.push(
            Finding::new(
                Severity::Blocker,
                "target_table_shape",
                "AFR target table is the wrong shape",
                format!(
                    "'{name}' is {rows}x{cols} but the VE table is {}x{}. The target is \
                     read using the VE table's own cell indices, so the rows and columns \
                     past its end — the high-load, high-rpm corner — would silently fall \
                     back to the flat target.",
                    input.ve_shape.0, input.ve_shape.1
                ),
            )
            .with_values(
                format!("{rows}x{cols}"),
                format!("{}x{}", input.ve_shape.0, input.ve_shape.1),
            ),
        );
        return;
    }

    // Units check. AFR targets live near 12-15, lambda near 0.8-1.0. Anything
    // else means the table is not what the session believes it is.
    let vals: Vec<f64> = input
        .target_values
        .iter()
        .flatten()
        .copied()
        .filter(|v| *v > 0.0)
        .collect();
    if vals.is_empty() {
        out.push(Finding::new(
            Severity::Blocker,
            "target_table_empty",
            "AFR target table reads as all zeros",
            format!(
                "'{name}' resolved but holds no usable values — the page probably did \
                 not sync."
            ),
        ));
        return;
    }
    let lo = vals.iter().cloned().fold(f64::MAX, f64::min);
    let hi = vals.iter().cloned().fold(f64::MIN, f64::max);
    let looks_afr = lo >= 8.0 && hi <= 22.0;
    let looks_lambda = lo >= 0.5 && hi <= 1.6;
    if !looks_afr && !looks_lambda {
        out.push(
            Finding::new(
                Severity::Warning,
                "target_table_range",
                "AFR target values look wrong",
                format!(
                    "'{name}' spans {lo:.2} to {hi:.2}, which is neither an AFR table \
                     (8-22) nor a lambda one (0.5-1.6). Check the table's scaling."
                ),
            )
            .with_values(format!("{lo:.2}..{hi:.2}"), "8-22 AFR or 0.5-1.6 lambda"),
        );
    }
}

fn check_ve_table(input: &PreflightInput, out: &mut Vec<Finding>) {
    let vals: Vec<f64> = input.ve_values.iter().flatten().copied().collect();
    if vals.is_empty() {
        out.push(Finding::new(
            Severity::Blocker,
            "ve_table_unreadable",
            "VE table could not be read",
            "Without the current VE values there is nothing to correct from.".to_string(),
        ));
        return;
    }
    if vals.iter().all(|v| *v <= 0.0) {
        out.push(Finding::new(
            Severity::Blocker,
            "ve_table_zero",
            "VE table is all zeros",
            "The tune has not synced from the ECU, or the wrong table was selected. \
             Tuning from zero would propose zero."
                .to_string(),
        ));
        return;
    }
    let hi = vals.iter().cloned().fold(f64::MIN, f64::max);
    if hi > 255.0 {
        out.push(
            Finding::new(
                Severity::Warning,
                "ve_table_scale",
                "VE values are larger than the table can hold",
                format!(
                    "The largest value is {hi:.0}. A Speeduino VE byte tops out at 255, \
                     so the scaling is probably wrong."
                ),
            )
            .with_values(format!("max {hi:.0}"), "<= 255"),
        );
    }
}

fn check_filters(input: &PreflightInput, filters: &AutoTuneFilters, out: &mut Vec<Finding>) {
    // The one that silently rejects an entire session.
    let (sane_lo, sane_hi, unit, suggest) = if input.celsius {
        (40.0, 100.0, "C", 60.0)
    } else {
        (100.0, 210.0, "F", 140.0)
    };
    if filters.min_clt < sane_lo || filters.min_clt > sane_hi {
        let boils = input.celsius && filters.min_clt > 100.0;
        out.push(
            Finding::new(
                if boils {
                    Severity::Blocker
                } else {
                    Severity::Warning
                },
                "min_clt_units",
                "Minimum coolant temperature looks like the wrong units",
                format!(
                    "min_clt is {:.0} but this INI reports temperature in {unit}.{}",
                    filters.min_clt,
                    if boils {
                        " That is above boiling — no sample can ever pass, and the \
                         session will collect nothing."
                    } else {
                        " Warm-up samples would be accepted, which biases every cell rich."
                    }
                ),
            )
            .with_values(
                format!("{:.0} {unit}", filters.min_clt),
                format!("{suggest:.0} {unit}"),
            ),
        );
    }
    if filters.min_rpm >= filters.max_rpm {
        out.push(
            Finding::new(
                Severity::Blocker,
                "rpm_window_empty",
                "RPM filter window is empty",
                format!(
                    "min_rpm {:.0} is not below max_rpm {:.0}, so no sample can pass.",
                    filters.min_rpm, filters.max_rpm
                ),
            )
            .with_values(
                format!("{:.0}..{:.0}", filters.min_rpm, filters.max_rpm),
                "min below max",
            ),
        );
    }
    if filters.max_tps_rate <= 0.0 {
        out.push(
            Finding::new(
                Severity::Warning,
                "tps_rate_inert",
                "Transient filter is switched off",
                "max_tps_rate is zero or negative, so tip-ins and lift-offs are accepted \
                 as if they were steady state. Those samples carry accel enrichment and \
                 wall wetting, not the VE the cell actually needs."
                    .to_string(),
            )
            .with_values(format!("{:.1}", filters.max_tps_rate), "10 %/s"),
        );
    }
}

fn check_authority(a: &AutoTuneAuthorityLimits, out: &mut Vec<Finding>) {
    if a.max_cell_value_change <= 0.0 || a.max_cell_percentage_change <= 0.0 {
        out.push(
            Finding::new(
                Severity::Blocker,
                "authority_zero",
                "Authority limit is zero",
                "Every proposed change would be clamped to nothing, so the session \
                 cannot alter a single cell."
                    .to_string(),
            )
            .with_values(
                format!(
                    "{:.1} / {:.1}%",
                    a.max_cell_value_change, a.max_cell_percentage_change
                ),
                "10 / 20%",
            ),
        );
    }
    if a.min_cell_value > a.max_cell_value {
        out.push(
            Finding::new(
                Severity::Warning,
                "rails_reversed",
                "Cell value limits are the wrong way round",
                format!(
                    "min {:.0} is above max {:.0}. They will be ordered before use, but \
                     one of them is not what was intended.",
                    a.min_cell_value, a.max_cell_value
                ),
            )
            .with_values(
                format!("{:.0}..{:.0}", a.min_cell_value, a.max_cell_value),
                "min below max",
            ),
        );
    }
}

fn check_delay(settings: &AutoTuneSettings, out: &mut Vec<Finding>) {
    if settings.lambda_delay_ms <= 0.0 && !settings.lambda_delay_flow_scaled {
        out.push(
            Finding::new(
                Severity::Warning,
                "delay_default_curve",
                "Using the built-in RPM delay curve",
                "No measured transport delay is set, so AutoTune assumes 200 ms at idle \
                 falling to 50 ms at 6000 rpm. A real exhaust is usually far slower — \
                 measure it with the AFR Delay tool, or enable flow scaling. Too short a \
                 delay credits each reading to a cell the engine has already left."
                    .to_string(),
            )
            .with_values("200-50 ms (assumed)", "measure, or enable flow scaling"),
        );
    }
}

fn check_write_mode(input: &PreflightInput, out: &mut Vec<Finding>) {
    out.push(if input.will_write_to_ecu {
        Finding::new(
            Severity::Info,
            "writes_live",
            "Changes will be written to ECU RAM as they are accepted",
            "Nothing is burned to flash by the session itself, so a key cycle restores \
             the stored tune."
                .to_string(),
        )
    } else {
        Finding::new(
            Severity::Info,
            "collect_only",
            "Recommendations only — nothing will be written",
            "The session collects and proposes; the ECU is not touched until you send or \
             burn the result."
                .to_string(),
        )
    });
}

/// Whether anything found would stop the session producing a usable result.
pub fn has_blocker(findings: &[Finding]) -> bool {
    findings.iter().any(|f| f.severity == Severity::Blocker)
}
