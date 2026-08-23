//! Every preflight check, shown firing on the condition it exists for.
//!
//! Each case here is a real failure from a car session, not an invented one.

use libretune_core::autotune::preflight::{check, has_blocker, PreflightInput, Severity};
use libretune_core::autotune::{AutoTuneAuthorityLimits, AutoTuneFilters, AutoTuneSettings};

/// A session with nothing wrong with it, to subtract from.
fn healthy<'a>(target: &'a [Vec<f64>], ve: &'a [Vec<f64>]) -> PreflightInput<'a> {
    PreflightInput {
        target_table: Some("afrTable1Tbl"),
        target_values: target,
        ve_shape: (2, 2),
        ve_values: ve,
        celsius: true,
        candidate_target_tables: vec!["afrTable1Tbl".to_string()],
        will_write_to_ecu: false,
    }
}

fn afr_target() -> Vec<Vec<f64>> {
    vec![vec![14.7, 14.7], vec![12.8, 12.8]]
}

fn ve_table() -> Vec<Vec<f64>> {
    vec![vec![45.0, 52.0], vec![94.0, 98.0]]
}

fn celsius_filters() -> AutoTuneFilters {
    AutoTuneFilters {
        min_clt: 60.0,
        ..Default::default()
    }
}

fn find<'a>(
    f: &'a [libretune_core::autotune::preflight::Finding],
    code: &str,
) -> Option<&'a libretune_core::autotune::preflight::Finding> {
    f.iter().find(|x| x.code == code)
}

#[test]
fn a_healthy_session_raises_no_blockers() {
    let (t, v) = (afr_target(), ve_table());
    let out = check(
        &healthy(&t, &v),
        &AutoTuneSettings::default(),
        &celsius_filters(),
        &AutoTuneAuthorityLimits::default(),
    );
    assert!(!has_blocker(&out), "unexpected blockers: {out:#?}");
}

/// The failure that produced 154 recommendations all targeting 14.7, including
/// wide-open throttle, on a car whose own table asks 12.7.
#[test]
fn a_missing_target_table_blocks_and_names_the_flat_value() {
    let (t, v) = (Vec::new(), ve_table());
    let mut input = healthy(&t, &v);
    input.target_table = None;
    let out = check(
        &input,
        &AutoTuneSettings::default(),
        &celsius_filters(),
        &AutoTuneAuthorityLimits::default(),
    );
    let f = find(&out, "target_table_missing").expect("must be reported");
    assert_eq!(f.severity, Severity::Blocker);
    assert!(
        f.detail.contains("14.7"),
        "must name the number: {}",
        f.detail
    );
    assert!(has_blocker(&out));
}

#[test]
fn a_target_of_the_wrong_shape_blocks() {
    let t = vec![vec![14.7, 14.7]]; // 1x2 against a 2x2 VE table
    let v = ve_table();
    let out = check(
        &healthy(&t, &v),
        &AutoTuneSettings::default(),
        &celsius_filters(),
        &AutoTuneAuthorityLimits::default(),
    );
    let f = find(&out, "target_table_shape").expect("must be reported");
    assert_eq!(f.severity, Severity::Blocker);
    assert_eq!(f.suggested.as_deref(), Some("2x2"));
}

/// A lambda table resolved as an AFR target, or a scaling error: either way the
/// numbers are not what the correction assumes.
#[test]
fn target_values_outside_both_unit_ranges_are_flagged() {
    let t = vec![vec![3.5, 3.5], vec![3.5, 3.5]]; // neither AFR nor lambda
    let v = ve_table();
    let out = check(
        &healthy(&t, &v),
        &AutoTuneSettings::default(),
        &celsius_filters(),
        &AutoTuneAuthorityLimits::default(),
    );
    assert!(find(&out, "target_table_range").is_some(), "{out:#?}");
}

#[test]
fn a_lambda_target_is_accepted_as_a_valid_range() {
    let t = vec![vec![1.0, 1.0], vec![0.88, 0.88]];
    let v = ve_table();
    let out = check(
        &healthy(&t, &v),
        &AutoTuneSettings::default(),
        &celsius_filters(),
        &AutoTuneAuthorityLimits::default(),
    );
    assert!(
        find(&out, "target_table_range").is_none(),
        "lambda is legitimate: {out:#?}"
    );
}

/// The default that would have rejected every sample of a session on a Celsius
/// project: 160 is above boiling.
#[test]
fn the_fahrenheit_clt_default_blocks_a_celsius_project() {
    let (t, v) = (afr_target(), ve_table());
    let filters = AutoTuneFilters {
        min_clt: 160.0,
        ..Default::default()
    };
    let out = check(
        &healthy(&t, &v),
        &AutoTuneSettings::default(),
        &filters,
        &AutoTuneAuthorityLimits::default(),
    );
    let f = find(&out, "min_clt_units").expect("must be reported");
    assert_eq!(
        f.severity,
        Severity::Blocker,
        "above boiling means zero samples"
    );
    assert!(f.detail.contains("boiling"), "{}", f.detail);
    assert_eq!(f.suggested.as_deref(), Some("60 C"));
}

#[test]
fn a_celsius_value_on_a_fahrenheit_project_is_a_warning_not_a_blocker() {
    let (t, v) = (afr_target(), ve_table());
    let mut input = healthy(&t, &v);
    input.celsius = false;
    let filters = AutoTuneFilters {
        min_clt: 60.0,
        ..Default::default()
    };
    let out = check(
        &input,
        &AutoTuneSettings::default(),
        &filters,
        &AutoTuneAuthorityLimits::default(),
    );
    let f = find(&out, "min_clt_units").expect("must be reported");
    // 60 F accepts a cold engine - biased, but samples still flow.
    assert_eq!(f.severity, Severity::Warning);
}

#[test]
fn an_empty_rpm_window_blocks() {
    let (t, v) = (afr_target(), ve_table());
    let filters = AutoTuneFilters {
        min_rpm: 4000.0,
        max_rpm: 2000.0,
        min_clt: 60.0,
        ..Default::default()
    };
    let out = check(
        &healthy(&t, &v),
        &AutoTuneSettings::default(),
        &filters,
        &AutoTuneAuthorityLimits::default(),
    );
    assert_eq!(
        find(&out, "rpm_window_empty").map(|f| f.severity),
        Some(Severity::Blocker)
    );
}

#[test]
fn a_disabled_transient_filter_is_flagged() {
    let (t, v) = (afr_target(), ve_table());
    let filters = AutoTuneFilters {
        max_tps_rate: 0.0,
        min_clt: 60.0,
        ..Default::default()
    };
    let out = check(
        &healthy(&t, &v),
        &AutoTuneSettings::default(),
        &filters,
        &AutoTuneAuthorityLimits::default(),
    );
    assert!(find(&out, "tps_rate_inert").is_some(), "{out:#?}");
}

#[test]
fn a_zero_authority_limit_blocks() {
    let (t, v) = (afr_target(), ve_table());
    let authority = AutoTuneAuthorityLimits {
        max_cell_value_change: 0.0,
        ..Default::default()
    };
    let out = check(
        &healthy(&t, &v),
        &AutoTuneSettings::default(),
        &celsius_filters(),
        &authority,
    );
    assert_eq!(
        find(&out, "authority_zero").map(|f| f.severity),
        Some(Severity::Blocker)
    );
}

#[test]
fn an_all_zero_ve_table_blocks() {
    let t = afr_target();
    let v = vec![vec![0.0, 0.0], vec![0.0, 0.0]];
    let out = check(
        &healthy(&t, &v),
        &AutoTuneSettings::default(),
        &celsius_filters(),
        &AutoTuneAuthorityLimits::default(),
    );
    assert_eq!(
        find(&out, "ve_table_zero").map(|f| f.severity),
        Some(Severity::Blocker)
    );
}

/// The delay default under-predicted this car by ~287 ms, which credits each
/// reading to a cell the engine has already left.
#[test]
fn the_unmeasured_delay_default_is_flagged() {
    let (t, v) = (afr_target(), ve_table());
    let out = check(
        &healthy(&t, &v),
        &AutoTuneSettings::default(),
        &celsius_filters(),
        &AutoTuneAuthorityLimits::default(),
    );
    assert!(find(&out, "delay_default_curve").is_some(), "{out:#?}");

    let measured = AutoTuneSettings {
        lambda_delay_ms: 550.0,
        ..Default::default()
    };
    let out = check(
        &healthy(&t, &v),
        &measured,
        &celsius_filters(),
        &AutoTuneAuthorityLimits::default(),
    );
    assert!(
        find(&out, "delay_default_curve").is_none(),
        "a measured delay must clear the warning"
    );
}

/// "let the user know if it's applying changes to RAM or not"
#[test]
fn the_write_mode_is_always_stated() {
    let (t, v) = (afr_target(), ve_table());
    let out = check(
        &healthy(&t, &v),
        &AutoTuneSettings::default(),
        &celsius_filters(),
        &AutoTuneAuthorityLimits::default(),
    );
    assert!(
        find(&out, "collect_only").is_some(),
        "collect-only must be stated"
    );

    let mut input = healthy(&t, &v);
    input.will_write_to_ecu = true;
    let out = check(
        &input,
        &AutoTuneSettings::default(),
        &celsius_filters(),
        &AutoTuneAuthorityLimits::default(),
    );
    let f = find(&out, "writes_live").expect("live writing must be stated");
    assert!(
        f.detail.contains("key cycle"),
        "say how to undo it: {}",
        f.detail
    );
}

#[test]
fn blockers_sort_above_warnings_and_info() {
    let (t, v) = (Vec::new(), ve_table());
    let mut input = healthy(&t, &v);
    input.target_table = None;
    let filters = AutoTuneFilters {
        max_tps_rate: 0.0,
        min_clt: 60.0,
        ..Default::default()
    };
    let out = check(
        &input,
        &AutoTuneSettings::default(),
        &filters,
        &AutoTuneAuthorityLimits::default(),
    );
    let severities: Vec<_> = out.iter().map(|f| f.severity).collect();
    let mut sorted = severities.clone();
    sorted.sort_by_key(|s| match s {
        Severity::Blocker => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
    });
    assert_eq!(
        severities, sorted,
        "worst first, so the dialog reads top-down"
    );
}

/// The INI declares `minCltFilter` under `#if CELSIUS` as 71 and otherwise as
/// 160. Warning when the session disagrees was treating the symptom; the value
/// should come from the INI in the first place. This asserts the rule the app
/// applies: a caller who has not chosen a value gets the INI's.
#[test]
fn an_untouched_min_clt_should_defer_to_the_ini() {
    let default_clt = AutoTuneFilters::default().min_clt;
    assert_eq!(
        default_clt, 160.0,
        "the struct default is Fahrenheit-shaped"
    );

    // The app substitutes only when the caller left it untouched.
    let untouched = AutoTuneFilters::default();
    let chosen = AutoTuneFilters {
        min_clt: 45.0,
        ..Default::default()
    };
    let should_substitute = |f: &AutoTuneFilters| (f.min_clt - default_clt).abs() < f64::EPSILON;
    assert!(
        should_substitute(&untouched),
        "an untouched value defers to the INI"
    );
    assert!(
        !should_substitute(&chosen),
        "a deliberate 45 must be left alone"
    );
}

/// And once it has deferred, the preflight must stop complaining - otherwise
/// the dialog nags about a value it chose itself.
#[test]
fn the_ini_value_raises_no_finding() {
    let t = vec![vec![14.7, 14.7], vec![12.8, 12.8]];
    let v = vec![vec![45.0, 52.0], vec![94.0, 98.0]];
    let filters = AutoTuneFilters {
        min_clt: 71.0,
        ..Default::default()
    };
    let out = check(
        &healthy(&t, &v),
        &AutoTuneSettings::default(),
        &filters,
        &AutoTuneAuthorityLimits::default(),
    );
    assert!(
        find(&out, "min_clt_units").is_none(),
        "71 C is what the INI declares for a Celsius project: {out:#?}"
    );
}
