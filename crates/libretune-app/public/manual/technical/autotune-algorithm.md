# AutoTune Algorithm

AutoTune is LibreTune's adaptive fuel-table correction system. It analyzes
real-world driving data and generates recommendations to bring the actual AFR
closer to the target AFR by adjusting VE table values.

## Overview

The algorithm addresses four fundamental challenges in ECU tuning:

1. **Correct reference** — each cell may have its own Target AFR and lambda delay.
2. **Lambda delay** — AFR sensors report combustion results 50–500 ms after fuel injection.
3. **Transient filtering** — throttle changes cause enrichment that should not be tuned out.
4. **Data quality** — not all data points are equally valuable.

## Core Pipeline

### 1. Data Point Collection

Each realtime data sample becomes a `VEDataPoint`:

```rust
pub struct VEDataPoint {
    pub rpm: f64,
    pub map: f64,
    pub maf: f64,
    pub load: f64,             // Generic load (MAP or MAF, depending on table)
    pub afr: f64,              // Measured air-fuel ratio
    pub ve: f64,               // Current VE estimate from ECU
    pub clt: f64,              // Coolant temperature
    pub tps: f64,              // Throttle position (%)
    pub tps_rate: f64,         // TPS change rate (%/sec)
    pub accel_enrich_active: Option<bool>,
    pub timestamp_ms: u64,
}
```

The frontend feeds samples as fast as the realtime stream provides them
(typically ~100 ms).

### 2. Reference Tables

Before starting, AutoTune can be configured with `AutoTuneReferenceTables`:

```rust
pub struct AutoTuneReferenceTables {
    pub lambda_delay_table: Vec<Vec<f64>>, // per-cell delay in ms
    pub target_afr_table: Vec<Vec<f64>>,   // per-cell Target AFR
}
```

- The **Target AFR table** is usually auto-discovered from the ECU definition.
  If a cell has a positive value, it is used; otherwise the session's fixed
  `target_afr` is used as fallback.
- The **lambda delay table** overrides the RPM-based transport-delay curve on
  a per-cell basis when present.

### 3. Lambda Delay Compensation

The AFR reading at time *T* corresponds to fuel injected at time *T − Δ*, where
Δ is the lambda delay.

**Default RPM-based delay curve**:
```
delay_ms = 200 - (150 × (rpm - 800) / (6000 - 800))
```

| RPM | Delay |
|-----|-------|
| 800 (idle) | 200 ms |
| 3400 (cruise) | 125 ms |
| 6000 (redline) | 50 ms |

**Correlation**:
1. The current sample is appended to a 500 ms rolling buffer.
2. A historical point is searched for `timestamp ≈ now - delay_ms` (within 50 ms).
3. In **strict lambda match** mode (default), the sample is dropped if no close
   historical match is found. This prevents attributing the AFR reading to the
   wrong VE cell during transients.
4. The historical RPM/load determine which VE cell receives the correction.

### 4. Filtering

A sample passes only if it satisfies every active filter:

- `min_rpm ≤ rpm ≤ max_rpm`
- `min_y_axis ≤ load ≤ max_y_axis` (when configured)
- `clt ≥ min_clt`
- `tps_rate ≤ max_tps_rate`
- `accel_enrich_active != true` (when `exclude_accel_enrich` is set)
- `custom_filter` expression evaluates to true (when configured)

The `custom_filter` is an `evalexpr` expression with variables: `rpm`, `map`,
`maf`, `load`, `afr`, `ve`, `clt`, `tps`, `tps_rate`, `accel_enrich`.

### 5. Target AFR Resolution

For the matched cell `(cell_x, cell_y)`:

```rust
target_afr = reference_tables.target_afr_table[cell_y][cell_x]
               .or_else(settings.target_afr)
```

### 6. Required VE Calculation

```rust
required_ve = current_ve * (actual_afr / target_afr)
```

This formula directly follows from the definition of AFR: if the engine is
running lean, VE must increase to add fuel; if rich, VE must decrease.

### 7. Cumulative Moving Average (CMA)

Each cell maintains `raw_required_cma`. On every accepted sample:

```rust
hit_count += 1
raw_required_cma += (required_ve - raw_required_cma) / hit_count
```

Authority limits are applied to the CMA to produce the displayed/applied value:

```rust
recommended_value = apply_authority_limits(
    beginning_value,
    raw_required_cma,
    authority,
)
```

Because clamping is applied *after* the average, a clamped outlier cannot bias
the long-term average.

### 8. Authority Limits

Two limits are enforced together:

```rust
delta = raw_required_cma - beginning_value
clamped_delta = clamp(delta,
                      -max_cell_value_change,
                      +max_cell_value_change)
max_pct_delta = beginning_value * max_cell_percentage_change / 100.0
final_delta = clamp(clamped_delta, -max_pct_delta, +max_pct_delta)
final_ve = beginning_value + final_delta
```

Defaults:
- `max_cell_value_change` = ±10.0 VE units
- `max_cell_percentage_change` = ±20%

### 9. Hit Percentage

```rust
hit_percentage = (cell_hit_count / total_accepted_samples) * 100.0
```

`total_accepted_samples` counts every sample that passed filters, making the
percentage a realistic measure of dwell time per cell.

## Analysis Subsystems

### Predictive Cell Filling (`autotune/predictor.rs`)

For cells with too few hits to trust, the predictor estimates a VE value from
neighboring known cells. Methods (tried in order):

1. **Bilinear Interpolation** — four surrounding known cells in all quadrants.
2. **Neighbor-Weighted Average** — distance-weighted average of nearby known
   cells. Distance is normalized by the physical RPM/load axis ranges, so a
   large physical gap reduces weight even if it is only one index away.
3. **One-Dimensional Interpolation** — when the target lies between two known
   points on the same row or column (high confidence, ≥ 0.70).
4. **Linear Extrapolation** — when the target lies outside the known range on a
   row or column (lower confidence, ≤ 0.40).
5. **Physics Model** — last-resort estimate using an RPM efficiency curve and
   configurable VE clamps (supports forced-induction VE > 100%).

### Tune Health Scoring (`autotune/health.rs`)

Divides the table into operating regions and scores each:

- **Idle** — low RPM, low load
- **Cruise** — mid RPM, mid load
- **WOT** — high load (absolute MAP ≥ 90 kPa by default)
- **Part Throttle** — transition load bands not covered by the above

Region boundaries use `>=` comparisons so bins exactly on a threshold belong to
the higher region. Part Throttle regions are split to avoid overlapping the
Cruise rectangle.

Per-region scores:
- **Coverage** — percent of cells with hits
- **Smoothness** — discrete Laplacian curvature (planar ramps score ~100)
- **Monotonicity** — VE generally increasing with load, with configurable load
  and RPM floors to ignore idle/decel areas

### Anomaly Detection (`autotune/anomaly.rs`)

Flags suspect cells:

- **Statistical Outlier** — cell residual from a local plane fit exceeds
  `outlier_sigma` standard deviations. This avoids false positives on smooth
  ramped surfaces that previously triggered scalar-mean comparison.
- **Monotonicity Violation** — VE drops sharply with increasing load (above the
  configured floor).
- **Gradient Discontinuity** — sharp jump vs. the average local gradient.
- **Physically Unreasonable** — VE outside `[min_reasonable_ve, max_reasonable_ve]`.
- **Flat Region** — adjacent cells with values within `flat_tolerance` of each
  other, suggesting an untuned copy/paste block.

Anomalies are sorted by severity and deduplicated per cell `(row, col)`, keeping
the single highest-severity issue for each cell.

## Data Structures

### AutoTuneState
```rust
pub struct AutoTuneState {
    pub is_running: bool,
    pub locked_cells: Vec<(usize, usize)>,
    pub recommendations: HashMap<(usize, usize), AutoTuneRecommendation>,
    data_buffer: VecDeque<VEDataPoint>,
    buffer_max_age_ms: u64,
    reference_tables: AutoTuneReferenceTables,
    strict_lambda_match: bool,
    total_samples: u64,
}
```

### AutoTuneFilters
```rust
pub struct AutoTuneFilters {
    pub min_rpm: f64,                 // default: 1000
    pub max_rpm: f64,                 // default: 7000
    pub min_y_axis: Option<String>,   // optional load lower bound
    pub max_y_axis: Option<String>,   // optional load upper bound
    pub min_clt: f64,                 // default: 160
    pub custom_filter: Option<String>,// optional evalexpr expression
    pub max_tps_rate: f64,            // default: 10 %/sec
    pub exclude_accel_enrich: bool,   // default: true
}
```

### AutoTuneAuthorityLimits
```rust
pub struct AutoTuneAuthorityLimits {
    pub max_cell_value_change: f64,       // default: 10.0
    pub max_cell_percentage_change: f64,  // default: 20.0
}
```

### AutoTuneRecommendation
```rust
pub struct AutoTuneRecommendation {
    pub cell_x: usize,
    pub cell_y: usize,
    pub beginning_value: f64,
    pub recommended_value: f64,
    pub hit_count: u64,
    pub hit_weighting: f64,
    pub target_afr: f64,
    pub hit_percentage: f64,
    pub raw_required_cma: f64,
}
```

## Performance Characteristics

- **Memory**: O(n×m + k) where n×m = table dimensions, k = buffer size
- **CPU per sample**: O(log k) for buffer insertion + O(1) for cell lookup
- **Health/Anomaly/Predictor**: O(n×m) when explicitly invoked, not per-sample

## Limitations

1. **Sensor Accuracy**: Garbage in, garbage out. A bad O2 sensor produces bad recommendations.
2. **Single-Fuel Assumption**: Assumes consistent fuel octane/quality during a session.
3. **Steady-State Bias**: Most accurate at stable RPM/load; transients are filtered.
4. **No Knock Detection**: Does not adjust ignition timing or detect detonation.
5. **VE-Only Tuning**: Does not directly tune ignition, boost, or other tables.

## Source Code Reference

- Core: `crates/libretune-core/src/autotune/mod.rs`
- Predictor: `crates/libretune-core/src/autotune/predictor.rs`
- Health: `crates/libretune-core/src/autotune/health.rs`
- Anomaly: `crates/libretune-core/src/autotune/anomaly.rs`
- Tauri commands: `crates/libretune-app/src-tauri/src/commands/autotune_*.rs`
- UI: `crates/libretune-app/src/components/tuner-ui/AutoTune.tsx`
- Tests: `crates/libretune-core/tests/autotune_heatmap.rs`

## See Also

- [AutoTune Overview](../features/autotune.md)
- [Usage Guide](../features/autotune/usage-guide.md)
- [Setting Up AutoTune](../features/autotune/setup.md)
- [Understanding Recommendations](../features/autotune/recommendations.md)
- [Filters and Authority](../features/autotune/filters.md)
- [Health, Anomalies & Predictions](../features/autotune/health-anomaly-predictor.md)
