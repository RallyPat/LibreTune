//! AutoTune VE-cell recommendation tests.
//!
//! Drives `AutoTuneState` through a synthetic run (via the `feed` helper
//! below) and asserts the recommended VE values, hit counts, and the
//! cumulative-moving-average (CMA) behaviour converge correctly. Several
//! cases here are regression guards for specific past bugs — see the inline
//! `bug #…` references. Lambda-delay handling is exercised by advancing the
//! timestamp stream on each sample (delay ≈ 190 ms @ 1000 RPM).

use libretune_core::autotune::{
    AutoTuneFilters, AutoTuneReferenceTables, AutoTuneSettings, AutoTuneState, VEDataPoint,
};

/// Helper: feed a point with a continuous timestamp stream so each sample has a
/// matching delayed-buffer entry (lambda delay at 1000 RPM ≈ 190 ms).
fn feed(
    state: &mut AutoTuneState,
    rpm: f64,
    load: f64,
    afr: f64,
    ve: f64,
    ts_base: &mut u64,
    table_x: &[f64],
    table_y: &[f64],
) {
    // Two samples ~200ms apart so the later one finds the earlier one as its
    // delayed match (delay at 1000 RPM ≈ 190ms, within the 50ms window).
    let p1 = VEDataPoint {
        rpm,
        map: load,
        load,
        afr,
        ve,
        clt: 170.0,
        tps: 30.0,
        timestamp_ms: *ts_base,
        ..Default::default()
    };
    state.add_data_point(
        p1,
        table_x,
        table_y,
        // These tests exercise the correction FORMULA, so the confidence ramp
        // and the change threshold are switched off here. Left at their
        // defaults (20.0 / 1.0) a three-sample cell correctly
        // proposes almost nothing, which would make these read as failures
        // when the thing they check is unaffected. The ramp has its own tests.
        &AutoTuneSettings {
            base_weight: 0.0,
            min_change: 0.0,
            ..Default::default()
        },
        &AutoTuneFilters::default(),
        &Default::default(),
    );
    *ts_base += 200;
}

#[test]
fn heatmap_entries_accumulate_hits_and_compute_change() {
    let mut state = AutoTuneState::new();
    state.start();

    let table_x = vec![1000.0, 2000.0, 3000.0];
    let table_y = vec![10.0, 20.0, 30.0];
    let mut ts = 0u64;

    // Same cell, AFRs that are richer than the target (14.7) → VE should drop.
    feed(
        &mut state, 1000.0, 10.0, 14.7, 50.0, &mut ts, &table_x, &table_y,
    );
    feed(
        &mut state, 1000.0, 10.0, 13.0, 50.0, &mut ts, &table_x, &table_y,
    );
    feed(
        &mut state, 1000.0, 10.0, 12.0, 50.0, &mut ts, &table_x, &table_y,
    );

    let recs = state.get_recommendations();
    assert!(!recs.is_empty(), "Expect at least one recommendation");

    let r = &recs[0];
    assert_eq!(r.cell_x, 0);
    assert_eq!(r.cell_y, 0);
    assert!(r.hit_count >= 1);
    // target_afr must reflect the resolved target (14.7 from settings), NOT
    // the last measured AFR (bug #16).
    assert!(
        (r.target_afr - 14.7).abs() < 1e-6,
        "target_afr should be 14.7, got {}",
        r.target_afr
    );
    // Richer-than-target AFRs should drive VE down (bug #1 formula).
    assert!(
        r.recommended_value < r.beginning_value,
        "richer AFR should lower VE, got {} -> {}",
        r.beginning_value,
        r.recommended_value
    );
}

#[test]
fn target_afr_reference_table_is_used() {
    // Bug #14 / #16: a populated target_afr_table overrides settings.target_afr.
    let mut state = AutoTuneState::new();
    // 3x3 target AFR table: cell (0,0) targets 12.5
    state.set_reference_tables(AutoTuneReferenceTables {
        target_afr_table: vec![
            vec![12.5, 13.0, 13.5],
            vec![13.0, 13.5, 14.0],
            vec![14.0, 14.5, 15.0],
        ],
        ..Default::default()
    });
    state.start();

    let table_x = vec![1000.0, 2000.0, 3000.0];
    let table_y = vec![10.0, 20.0, 30.0];
    let mut ts = 0u64;

    // Measured AFR == target (12.5) → required VE == current VE (no change).
    feed(
        &mut state, 1000.0, 10.0, 12.5, 60.0, &mut ts, &table_x, &table_y,
    );
    feed(
        &mut state, 1000.0, 10.0, 12.5, 60.0, &mut ts, &table_x, &table_y,
    );

    let r = state.get_recommendations()[0].clone();
    assert!(
        (r.target_afr - 12.5).abs() < 1e-6,
        "per-cell target_afr should be 12.5, got {}",
        r.target_afr
    );
    assert!(
        (r.recommended_value - 60.0).abs() < 1e-6,
        "actual==target → no VE change, got {}",
        r.recommended_value
    );
}

#[test]
fn strict_lambda_match_drops_unmatched_samples() {
    // Bug #2: with strict matching (default), a sample whose delayed time has
    // no buffered match is dropped rather than attributed to the current cell.
    let mut state = AutoTuneState::new();
    state.set_strict_lambda_match(true);
    state.start();

    let table_x = vec![1000.0, 2000.0];
    let table_y = vec![10.0, 20.0];

    // Single isolated sample at t=10000 with no prior buffer history → no match.
    let p = VEDataPoint {
        rpm: 1000.0,
        load: 10.0,
        afr: 13.0,
        ve: 50.0,
        clt: 170.0,
        tps: 30.0,
        timestamp_ms: 10_000,
        ..Default::default()
    };
    state.add_data_point(
        p.clone(),
        &table_x,
        &table_y,
        &AutoTuneSettings::default(),
        &AutoTuneFilters::default(),
        &Default::default(),
    );

    assert!(
        state.get_recommendations().is_empty(),
        "strict mode should drop unmatched sample"
    );

    // Non-strict mode keeps the (inaccurate) fallback.
    state.set_strict_lambda_match(false);
    let p2 = VEDataPoint {
        timestamp_ms: 10_200,
        ..p.clone()
    };
    state.add_data_point(
        p2,
        &table_x,
        &table_y,
        &AutoTuneSettings::default(),
        &AutoTuneFilters::default(),
        &Default::default(),
    );
    assert!(
        !state.get_recommendations().is_empty(),
        "non-strict mode should keep the fallback sample"
    );
}

#[test]
fn cma_averages_required_ve() {
    // Bug #5: recommendations average across samples rather than reflecting
    // only the most recent one.
    let mut state = AutoTuneState::new();
    state.start();

    let table_x = vec![1000.0, 2000.0];
    let table_y = vec![10.0, 20.0];
    let mut ts = 0u64;

    // Beginning VE = 50. Feed a chain of samples 200 ms apart (delay at 1000
    // RPM ≈ 190 ms) so each later sample finds the previous one as its delayed
    // match. We alternate two lean AFRs so the CMA converges to their midpoint;
    // a pure last-value implementation would oscillate between the two.
    for i in 0..12 {
        let afr = if i % 2 == 0 { 15.0 } else { 16.0 };
        feed(
            &mut state, 1000.0, 10.0, afr, 50.0, &mut ts, &table_x, &table_y,
        );
    }

    let r = state.get_recommendations()[0].clone();
    assert!(
        r.hit_count >= 4,
        "expected multiple hits, got {}",
        r.hit_count
    );
    // Required VE per sample: 50*(afr/14.7). With equal counts of 15.0/16.0 the
    // true mean is 50*(15.5/14.7) ≈ 52.72. The CMA (seeded from the beginning
    // value 50) converges toward it; after ~10 hits it must be within 1.0.
    let mean = 50.0 * 15.5 / 14.7;
    assert!(
        (r.recommended_value - mean).abs() < 1.0,
        "CMA should converge toward ~{:.2}, got {}",
        mean,
        r.recommended_value
    );
    // A pure last-value impl would be near either v(15.0)≈51.02 or v(16.0)≈54.42;
    // confirm the recommended value sits between them, away from both extremes.
    let v15 = 50.0 * 15.0 / 14.7;
    let v16 = 50.0 * 16.0 / 14.7;
    assert!(
        r.recommended_value > v15 + 0.5 && r.recommended_value < v16 - 0.5,
        "should be an average (between {:.2} and {:.2}), got {}",
        v15,
        v16,
        r.recommended_value
    );
}

#[test]
fn y_axis_filter_rejects_out_of_bounds_load() {
    // Bug #15: min/max_y_axis bounds are now enforced.
    let state = AutoTuneState::new();
    let mut filters = AutoTuneFilters::default();
    filters.min_y_axis = Some("15.0".to_string());
    filters.max_y_axis = Some("25.0".to_string());

    let below = VEDataPoint {
        rpm: 1500.0,
        load: 10.0,
        clt: 170.0,
        ..Default::default()
    };
    let above = VEDataPoint {
        rpm: 1500.0,
        load: 30.0,
        clt: 170.0,
        ..Default::default()
    };
    let inside = VEDataPoint {
        rpm: 1500.0,
        load: 20.0,
        clt: 170.0,
        ..Default::default()
    };

    assert!(!state.passes_filters(&below, &filters));
    assert!(!state.passes_filters(&above, &filters));
    assert!(state.passes_filters(&inside, &filters));
}

/// The ramp is a real behaviour change, not a knob nobody feels: the same three
/// samples that move a cell with the ramp off must move it less with the ramp
/// at the conventional default.
#[test]
fn the_confidence_ramp_holds_a_sparse_cell_back() {
    let table_x = vec![1000.0, 2000.0];
    let table_y = vec![10.0, 20.0];

    let run = |base_weight: f64, min_change: f64| {
        let mut state = AutoTuneState::default();
        state.start();
        let mut ts = 0u64;
        for afr in [13.0_f64, 12.5, 12.0] {
            let p = VEDataPoint {
                rpm: 1000.0,
                map: 10.0,
                load: 10.0,
                afr,
                ve: 50.0,
                clt: 170.0,
                tps: 30.0,
                timestamp_ms: ts,
                ..Default::default()
            };
            state.add_data_point(
                p,
                &table_x,
                &table_y,
                &AutoTuneSettings {
                    base_weight,
                    min_change,
                    ..Default::default()
                },
                &AutoTuneFilters::default(),
                &Default::default(),
            );
            ts += 200;
        }
        let recs = state.get_recommendations();
        recs.first()
            .map(|r| r.recommended_value)
            .unwrap_or(f64::NAN)
    };

    let unramped = run(0.0, 0.0);
    let ramped = run(20.0, 0.0);
    assert!(
        unramped < 50.0,
        "richer than target must want less VE: {unramped}"
    );
    assert!(
        ramped > unramped,
        "three samples against a base weight of 20 should propose only part of          the change: ramped {ramped} vs unramped {unramped}"
    );
    assert!(ramped <= 50.0, "and never the wrong direction: {ramped}");
}
