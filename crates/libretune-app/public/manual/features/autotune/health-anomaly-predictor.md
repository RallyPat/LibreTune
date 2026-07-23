# Health, Anomalies & Predictions

AutoTune includes three analysis features that help you judge the quality of a
tune before applying corrections:

1. **Tune Health Score** — overall grade by operating region.
2. **Anomaly Detection** — flags suspicious cells.
3. **Predicted Cell Filling** — estimates values for cells with no data.

These features do **not** change VE values on their own; they are review tools
for you to decide what to tune next.

## Tune Health Score

The health scorer divides the VE table into operating regions and assigns each
region a coverage, smoothness, and monotonicity score. This lets you see at a
glance whether your tune is consistent and complete.

### Operating Regions

Regions are defined using RPM (table columns) and load (table rows). Boundaries
use `>=` comparisons, so a bin exactly on a boundary belongs to the higher
region.

| Region | Typical Definition | Purpose |
|--------|-------------------|---------|
| **Idle** | Low RPM, low load | Startup, traffic, closed-throttle behavior |
| **Cruise** | Mid RPM, mid load | Highway, steady-state efficiency |
| **Part Throttle** | Transition load/RPM bands | Daily driving between cruise and WOT |
| **WOT** | Load at or above 90 kPa | Full throttle power tuning |

The WOT region is detected by absolute MAP (`>= 90 kPa` by default), not by
throttle position. This makes it compatible with different load conventions.
Part Throttle regions are split into low and high bands and are kept
non-overlapping with Cruise so each cell belongs to exactly one region.

### Metrics

For every region, three metrics are reported:

- **Coverage** — percentage of cells in the region that have at least one hit.
  Low coverage means you haven't driven in that region enough.
- **Smoothness** — based on a discrete Laplacian of the VE surface. A planar
  ramp scores nearly 100; spiky or oscillating surfaces score low.
- **Monotonicity** — checks that VE generally increases with load. A floor
  value is applied so idle and very low-load cells don't skew the score.

The overall health score is a weighted combination of these metrics.

### Interpreting Health Scores

| Score | Meaning |
|-------|---------|
| 90–100 | Excellent. Table is smooth, monotonic, and well covered. |
| 70–89 | Good. A few rough cells or regions need attention. |
| 50–69 | Fair. Significant gaps or inconsistencies. Investigate before applying. |
| < 50 | Poor. Likely mechanical issues, insufficient data, or a bad base tune. |

## Anomaly Detection

Anomaly detection flags cells that look statistically or physically wrong. The
goal is to catch problems before you apply AutoTune recommendations across the
whole table.

### Detection Methods

#### 1. Statistical Outlier

A local plane is fit to the 3×3 neighborhood around a cell. The cell is flagged
if its residual is more than `outlier_sigma` standard deviations away from the
plane. This catches spikes and dips without false-flagging legitimate ramps.

#### 2. Monotonicity Violation

Flags cells where VE drops as load increases (above the configured load/RPM
floor). A healthy engine generally needs more fuel as load increases.

#### 3. Gradient Discontinuity

Flags cells whose local gradient differs sharply from the average gradient of
surrounding cells.

#### 4. Physically Unreasonable

Flags cells outside the configured `min_reasonable_ve` and `max_reasonable_ve`
range. The maximum supports VE values above 100% for forced-induction engines.

#### 5. Flat Region

Flags blocks of adjacent cells with values within `flat_tolerance` of each
other. Large flat blocks often indicate an untuned copy/paste area.

### Severity and Deduplication

Each detected issue has a severity. A single cell can trigger multiple
detectors, but only the highest-severity issue for that cell is reported. This
keeps the anomaly list actionable and avoids duplicate warnings.

### What to Do with Anomalies

1. **Check the sensor** — noisy AFR or MAP readings can create apparent outliers.
2. **Inspect the table** — a single spiky cell is usually worth smoothing
   manually.
3. **Verify the mechanical state** — fuel pressure, injector scaling, or a
   vacuum leak can create broad monotonicity issues.
4. **Lock bad cells** — if you are confident a cell should not be changed, lock
   it before applying AutoTune.

## Predicted Cell Filling

Predictions estimate VE values for cells that have not received enough AutoTune
hits to be trusted. They are shown as suggestions only and are not applied
unless you explicitly accept them.

### Prediction Methods (tried in order)

1. **Bilinear Interpolation** — four surrounding known cells in all four
   quadrants.
2. **Neighbor-Weighted Average** — distance-weighted average of nearby known
   cells. Distance is normalized by the physical RPM/load axis ranges, so a
   large gap reduces weight even when it is only one index away.
3. **One-Dimensional Interpolation** — when the target lies between two known
   points on the same row or column. These predictions are marked high
   confidence (≥ 0.70).
4. **Linear Extrapolation** — when the target lies outside the known range on a
   row or column. These predictions are lower confidence (≤ 0.40).
5. **Physics Model** — last-resort estimate using an RPM efficiency curve and
   configurable VE bounds. Used when no nearby known cells exist.

### Confidence Values

| Confidence | Meaning |
|------------|---------|
| ≥ 0.90 | High confidence — interpolated from close neighbors |
| 0.70–0.89 | Good confidence — 1D interpolation or close weighted average |
| 0.40–0.69 | Moderate confidence — extrapolation or sparse neighbors |
| < 0.40 | Low confidence — physics model or very sparse data |

### When to Use Predictions

Predictions are helpful for:

- Filling a few missing cells after a partial tuning session.
- Building a rough base map from a handful of known points.
- Sanity-checking whether the existing trend makes physical sense.

They should **not** be used for:

- Replacing real measurements in critical operating regions.
- Finalizing a tune without verification.
- Large-scale extrapolation across wide unknown regions.

## Configuration

Health, anomaly, and predictor options are controlled by
`AutoTuneAdvancedSettings`:

| Setting | Description | Default |
|---------|-------------|---------|
| `idle_rpm_max` | RPM boundary for Idle region | 1200 |
| `cruise_rpm_max` | RPM boundary for Cruise region | 3000 |
| `cruise_load_max` | Load boundary for Cruise region | 70 kPa |
| `wot_load_threshold` | Absolute MAP threshold for WOT region | 90 kPa |
| `monotonicity_load_floor` | Ignore load cells below this for monotonicity | 30 kPa |
| `monotonicity_rpm_floor` | Ignore RPM cells below this for monotonicity | 1000 |
| `outlier_sigma` | Standard-deviation threshold for statistical outliers | 2.5 |
| `min_reasonable_ve` | Minimum physically plausible VE | 20 |
| `max_reasonable_ve` | Maximum physically plausible VE (boost capable) | 150 |
| `flat_tolerance` | Maximum difference within a flagged flat block | 1.0 |
| `prediction_confidence_threshold` | Minimum confidence to show a prediction | 0.3 |

## Integration with AutoTune Workflow

A typical workflow:

1. Run AutoTune until coverage is reasonable.
2. Open the **Health** panel and check region scores.
3. Review **Anomalies** and investigate flagged cells.
4. Use **Predictions** cautiously to fill small gaps.
5. Apply only the corrections you are confident in.
6. Re-run health/anomaly checks after applying.

## See Also

- [AutoTune Overview](../autotune.md)
- [Usage Guide](./usage-guide.md)
- [Setting Up AutoTune](./setup.md)
- [Understanding Recommendations](./recommendations.md)
- [Filters and Authority](./filters.md)
- [AutoTune Algorithm (Technical)](../../../technical/autotune-algorithm.md)
