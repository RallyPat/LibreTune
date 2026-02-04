use libretune_core::autotune::{AutoTuneFilters, AutoTuneState, VEDataPoint};

#[test]
fn heatmap_entries_accumulate_hits_and_compute_change() {
    let mut state = AutoTuneState::new();
    state.start();

    let table_x = vec![1000.0, 2000.0, 3000.0];
    let table_y = vec![10.0, 20.0, 30.0];

    let filters = AutoTuneFilters::default();

    // Simulate a few data points in same cell with different AFRs leading to change
    let pts = vec![
        VEDataPoint {
            rpm: 1000.0,
            map: 10.0,
            load: 10.0,
            afr: 14.7,
            ve: 50.0,
            clt: 170.0,
            ..Default::default()
        },
        VEDataPoint {
            rpm: 1000.0,
            map: 10.0,
            load: 10.0,
            afr: 13.0,
            ve: 50.0,
            clt: 170.0,
            ..Default::default()
        },
        VEDataPoint {
            rpm: 1000.0,
            map: 10.0,
            load: 10.0,
            afr: 12.0,
            ve: 50.0,
            clt: 170.0,
            ..Default::default()
        },
    ];

    for p in pts.into_iter() {
        state.add_data_point(
            p,
            &table_x,
            &table_y,
            &Default::default(),
            &filters,
            &Default::default(),
        );
    }

    let recs = state.get_recommendations();
    assert!(!recs.is_empty(), "Expect at least one recommendation");

    let r = &recs[0];
    assert_eq!(r.cell_x, 0);
    assert_eq!(r.cell_y, 0);
    assert!(r.hit_count >= 1);
    // Since AFR changed, recommended_value should differ from beginning
    assert!((r.recommended_value - r.beginning_value).abs() < 1e6);
}
