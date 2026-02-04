# AutoTune Algorithm

AutoTune is LibreTune's adaptive fuel table correction system. It analyzes real-world driving data (RPM, MAP, AFR) and generates recommendations to bring the actual AFR closer to the target AFR by adjusting VE table values.

## Overview

The AutoTune algorithm addresses three fundamental challenges in ECU tuning:

1. **Lambda Delay** - AFR sensors report the combustion result 50-500ms after the fuel was injected
2. **Transient Filtering** - Throttle changes cause enrichment that shouldn't be tuned out
3. **Data Quality** - Not all data points are equally valuable for tuning

## Core Algorithm

### 1. Data Point Collection

Each realtime data sample becomes a `VEDataPoint`:

```rust
pub struct VEDataPoint {
    pub rpm: f64,              // Engine speed (RPM)
    pub map: f64,              // Manifold pressure (kPa)
    pub afr: f64,              // Measured air-fuel ratio
    pub target_afr: f64,       // Target AFR from tune
    pub tps: f64,              // Throttle position (0-100%)
    pub tps_rate: f64,         // TPS change rate (%/sec)
    pub accel_enrich_active: Option<bool>,  // ECU acceleration enrichment flag
    pub timestamp_ms: u64,     // Timestamp for correlation
}
```

**Data Collection Rate**: 100ms (10 Hz) from realtime stream

### 2. Lambda Delay Compensation

The AFR sensor reading at time T corresponds to fuel injected at time T-Δ, where Δ is the lambda delay.

**Delay Calculation** (RPM-dependent):
```
delay_ms = 200 - (150 * (rpm - 800) / (6000 - 800))
```

| RPM | Delay |
|-----|-------|
| 800 (idle) | 200ms |
| 3400 (cruise) | 125ms |
| 6000 (redline) | 50ms |

**Implementation**:
```rust
fn get_lambda_delay_ms(rpm: f64) -> u64 {
    let rpm_clamped = rpm.clamp(800.0, 6000.0);
    let delay = 200.0 - (150.0 * (rpm_clamped - 800.0) / (6000.0 - 800.0));
    delay as u64
}
```

**Data Point Correlation**:
1. Current AFR reading is buffered
2. Historical data point is found: `timestamp_now - delay_ms`
3. Historical RPM/MAP determines which VE cell to update
4. Current AFR is used for correction calculation

### 3. Transient Filtering

Data points are rejected during throttle transients to avoid tuning out acceleration enrichment:

**Filter Conditions** (all must pass):
- `tps_rate < max_tps_rate` (default: 10%/sec)
- `accel_enrich_active == false` (if ECU provides this signal)
- RPM and MAP within table bounds

**TPS Rate Calculation**:
```rust
tps_rate = (tps_current - tps_previous) / time_delta_sec
```

**Why This Matters**: During rapid throttle opening, the ECU adds extra fuel (acceleration enrichment) to compensate for wall wetting. This is intentional and should not be "tuned out" by reducing VE values.

### 4. Cell Hit Weighting

Not all data points are equal. LibreTune uses weighted averaging based on:

**Weighting Factors**:
1. **RPM Stability** - More weight for stable RPM
   ```
   w_rpm = 1.0 / (1.0 + abs(rpm_current - rpm_previous))
   ```

2. **MAP Stability** - More weight for stable MAP
   ```
   w_map = 1.0 / (1.0 + abs(map_current - map_previous))
   ```

3. **AFR Error** - More weight for small errors (reduces oscillation)
   ```
   error_ratio = abs(afr - target_afr) / target_afr
   w_error = exp(-error_ratio * 2.0)
   ```

4. **Combined Weight**:
   ```
   weight = w_rpm * w_map * w_error
   ```

### 5. Recommendation Calculation

For each VE table cell (RPM, MAP):

**Accumulation**:
```rust
pub struct CellRecommendation {
    pub sum_corrections: f64,  // Σ(weight * correction)
    pub sum_weights: f64,      // Σ(weight)
    pub hit_count: u32,        // Number of data points
}
```

**AFR Correction Factor**:
```
correction = target_afr / measured_afr
```

- If AFR = 14.7 and Target = 13.5: correction = 0.918 (need more fuel, increase VE)
- If AFR = 12.8 and Target = 13.5: correction = 1.055 (too rich, decrease VE)

**Weighted Recommendation**:
```
recommendation = (sum_corrections / sum_weights)
```

**Apply to VE Table**:
```
new_ve = old_ve * recommendation
```

### 6. Authority Limits

Recommendations are clamped to prevent dangerous changes:

**Absolute Limit** (default: ±20%):
```
change = new_ve - old_ve
if abs(change) > max_absolute_change:
    new_ve = old_ve + sign(change) * max_absolute_change
```

**Percentage Limit** (default: ±20%):
```
ratio = new_ve / old_ve
if ratio > (1.0 + max_percent / 100.0):
    new_ve = old_ve * (1.0 + max_percent / 100.0)
elif ratio < (1.0 - max_percent / 100.0):
    new_ve = old_ve * (1.0 - max_percent / 100.0)
```

**Why Both Limits?**:
- Absolute prevents large swings in low VE areas (idle)
- Percentage prevents small absolute changes in high VE areas (WOT)

### 7. Cell Locking

Users can lock specific cells to prevent AutoTune from modifying them:

**Use Cases**:
- Cells with known-good values from dyno tuning
- Edge regions with insufficient data
- Cells requiring manual tuning (boost control zones)

**Implementation**: Locked cells are skipped during recommendation application.

## Data Structures

### AutoTuneState
```rust
pub struct AutoTuneState {
    pub running: bool,
    pub table_name: String,
    pub x_bins: Vec<f64>,          // RPM axis
    pub y_bins: Vec<f64>,          // MAP axis
    pub recommendations: HashMap<(usize, usize), CellRecommendation>,
    pub locked_cells: HashSet<(usize, usize)>,
    pub data_buffer: VecDeque<VEDataPoint>,
    pub buffer_max_age_ms: u64,    // Prune data older than this
    pub filters: AutoTuneFilters,
    pub authority: AutoTuneAuthority,
}
```

### AutoTuneFilters
```rust
pub struct AutoTuneFilters {
    pub min_rpm: f64,              // Minimum RPM (default: 800)
    pub max_rpm: f64,              // Maximum RPM (default: 6000)
    pub min_map: f64,              // Minimum MAP (default: 20 kPa)
    pub max_map: f64,              // Maximum MAP (default: 150 kPa)
    pub max_tps_rate: f64,         // Max TPS %/sec (default: 10)
    pub exclude_accel_enrich: bool, // Reject accel enrichment (default: true)
}
```

### AutoTuneAuthority
```rust
pub struct AutoTuneAuthority {
    pub max_absolute_change: f64,  // Max ± absolute change (default: 20.0)
    pub max_percent_change: f64,   // Max ± percentage change (default: 20.0)
}
```

## Performance Characteristics

- **Memory**: O(n*m + k) where n×m = table dimensions, k = buffer size
- **CPU per sample**: O(log k) for buffer insertion + O(1) for cell lookup
- **Buffer pruning**: O(k) every 1 second (removes data older than 500ms)

Typical memory usage: ~10 KB for 16×16 table with 100-point buffer

## Convergence Behavior

AutoTune typically converges to within 5% of optimal VE values after:

- **Light load (cruise)**: 5-10 minutes of driving
- **Medium load (acceleration)**: 10-20 minutes
- **Full load (WOT)**: Requires multiple WOT pulls (3-5 runs)

**Factors affecting convergence**:
1. Data coverage (drive through all RPM/MAP regions)
2. AFR sensor accuracy (narrowband vs wideband)
3. Authority limits (lower limits = slower but safer)
4. Engine stability (misfires cause AFR noise)

## Limitations

1. **Sensor Accuracy**: Garbage in, garbage out. Bad O2 sensor = bad tune
2. **Single-Fuel Assumption**: Algorithm assumes consistent fuel octane/quality
3. **Steady-State Bias**: Works best at stable RPM/MAP, less effective for transients
4. **No Knock Detection**: Cannot detect detonation (requires separate knock sensor analysis)
5. **VE-Only Tuning**: Does not adjust ignition timing, boost control, or other parameters

## Comparison to Other Algorithms

| Algorithm | Lambda Delay | Transient Filter | Hit Weighting | Authority Limits |
|-----------|--------------|------------------|---------------|------------------|
| LibreTune AutoTune | ✅ RPM-based | ✅ TPS rate + accel flag | ✅ Stability-weighted | ✅ Absolute + percentage |
| MegaLogViewer VE Analyze | ❌ Manual offset | ✅ TPS filter | ❌ Equal weight | ⚠️ Percentage only |
| TunerStudio AutoTune | ✅ Fixed delay | ✅ TPS + RPM filters | ✅ Hit count weighted | ✅ Configurable |
| MLV Pro | ✅ Configurable | ✅ Multi-condition | ✅ Advanced weighting | ✅ Multi-level |

## Source Code Reference

- Implementation: `crates/libretune-core/src/autotune.rs`
- Tauri integration: `crates/libretune-app/src-tauri/src/lib.rs` (feed_autotune_data)
- UI component: `crates/libretune-app/src/components/tuner-ui/AutoTune.tsx`
- Tests: `crates/libretune-core/tests/autotune_heatmap.rs`

## See Also

- [AutoTune Usage Guide](../features/autotune/usage-guide.md) - Step-by-step tuning workflow
- [Filters and Authority](../features/autotune/filters.md) - Configuring AutoTune settings
- [Understanding Recommendations](../features/autotune/recommendations.md) - Interpreting heatmaps
