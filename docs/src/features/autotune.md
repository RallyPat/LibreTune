# AutoTune

AutoTune is LibreTune's adaptive VE-table correction system. It compares the
wideband AFR your engine is actually running against the AFR target you want,
then recommends changes to the VE table so the ECU delivers the right amount
of fuel.

## What AutoTune Does

1. **Collects** real-time engine data (RPM, load, AFR, coolant temp, TPS, etc.).
2. **Correlates** each AFR reading back to the exact VE cell that produced it,
   compensating for lambda-sensor transport delay.
3. **Calculates** a required VE correction for that cell.
4. **Averages** many samples so noise and transients cancel out.
5. **Displays** recommendations as a heat map you can review before applying.
6. **Applies** changes to the ECU when you click **Send**.

## When to Use AutoTune

AutoTune works best for steady-state correction of a VE table that is already
close enough to run the engine safely. It is not a substitute for:

- A safe base tune
- A working wideband O2 sensor
- Common-sense validation of mechanical issues (fuel pressure, injector scaling,
  sensor calibration, etc.)

## Key Concepts

### Target AFR

The air-fuel ratio you want the engine to run. AutoTune can use either:

- **Per-cell Target AFR table** — recommended. LibreTune auto-discovers the ECU's
  AFR target table and reads a target value for each VE cell. If a cell has no
  target, the session's fixed target AFR is used as a fallback.
- **Fixed Target AFR** — a single global value such as 14.7 for gasoline stoich.

### Lambda Delay

The AFR sensor does not see the result of combustion instantly. Exhaust gasses
take roughly 50–500 ms to travel from the cylinder to the sensor. AutoTune keeps
a short history buffer and attributes each AFR reading to the RPM/load cell the
engine was in when that exhaust charge was produced.

By default, **strict lambda matching** is enabled: if a historical match cannot
be found within a small time window, the sample is dropped rather than being
mis-attributed to the current cell. You can relax this behavior, but it reduces
accuracy during transients.

### Cumulative Moving Average (CMA)

AutoTune does not apply each sample directly. It maintains a running average of
the *raw* required VE for each cell. Authority limits are applied only to the
*displayed/applied* recommendation, so clamping cannot bias the underlying
average.

### Authority Limits

Safety clamps that prevent large, sudden changes. Two limits are enforced
together:

- **Max cell value change** — absolute VE units per update (default ±10).
- **Max cell percentage change** — percent of the original VE (default ±20%).

If a recommendation is clamped, the cell shows a warning icon.

## Documentation Sections

- [Usage Guide](./autotune/usage-guide.md) — step-by-step tuning workflow
- [Setting Up AutoTune](./autotune/setup.md) — configuration options
- [Understanding Recommendations](./autotune/recommendations.md) — how recommendations are calculated
- [Filters and Authority](./autotune/filters.md) — filter and limit reference
- [Health, Anomalies & Predictions](./autotune/health-anomaly-predictor.md) — analysis features
- [AutoTune Algorithm (Technical)](../../technical/autotune-algorithm.md) — implementation details

## Safety First

- Start with conservative authority limits.
- Verify wideband calibration and fuel-system health before trusting recommendations.
- Review the heat map for coverage and consistency before applying changes.
- Save the tune and create a restore point before sending large corrections.
