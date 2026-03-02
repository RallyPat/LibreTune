# Base Map Generator

The Base Map Generator creates a safe, driveable starting tune from your engine's specifications. It produces complete VE (Volumetric Efficiency), ignition timing, AFR target tables, plus all the enrichment curves your ECU needs to start and run.

> **Important**: A base map is a *starting point*, not a finished tune. It is intentionally conservative — biased toward safe, rich fueling and moderate timing. You should always follow up with real-world tuning (either manually or with [AutoTune](./autotune.md)) on a dyno or during careful road testing.

---

## When to Use the Base Map Generator

Use the base map generator when:
- You're setting up a **brand-new ECU** and have no existing tune file
- You're switching to a **new engine configuration** (e.g., different injectors, turbo conversion)
- You want a **safe baseline** to begin tuning from scratch
- You need a **quick sanity check** on what reasonable table values look like for your setup

Do **not** use it when:
- You already have a working tune that just needs refinement — use [AutoTune](./autotune.md) instead
- You're importing a tune from another tuner — use [Open Tune File](../getting-started/first-project.md#opening-a-tune-file) instead

---

## How to Access

There are two ways to open the Base Map Generator:

1. **Welcome View**: Click the **"Generate Base Map from Engine Specs"** link below the main action buttons
2. **Tools menu**: Go to **Tools → Generate Base Map...** (available any time, with or without a project open)

---

## Entering Engine Specifications

The generator dialog presents a form with your engine's key specifications. Fill in each field as accurately as possible — the generated tables are only as good as your inputs.

### Required Fields

#### Cylinders

Number of engine cylinders. Select from: 1, 2, 3, 4, 5, 6, 8, 10, or 12.

This directly affects:
- reqFuel calculation (fuel pulse width per cylinder)
- Injector divider for batch/sequential modes
- Ignition timing distribution

#### Displacement (cc)

Total engine displacement in cubic centimeters. For example:
- Honda B18B: 1834
- Toyota 2JZ-GTE: 2997
- Chevy LS3: 6162
- Mazda 13B Rotary: 1308

Valid range: 50 – 15,000 cc.

#### Injector Size (cc/min)

Fuel injector flow rate in cubic centimeters per minute, measured at the injector's rated fuel pressure (typically 43.5 psi / 3 bar for port injection).

Common values:
| Injector Size | Typical Use |
|---------------|-------------|
| 160–220 cc/min | Small NA engines |
| 350–440 cc/min | Medium NA engines (most 4-cyl) |
| 550–750 cc/min | Large NA engines, mild boost |
| 1000+ cc/min | High-boost turbo applications |

Valid range: 50 – 5,000 cc/min.

#### Fuel Type

| Fuel | Stoich AFR | Safe WOT AFR | Notes |
|------|-----------|-------------|-------|
| **Gasoline** | 14.7:1 | 12.5:1 | Most common, default choice |
| **E85** | 9.8:1 | 8.5:1 | Requires ~30% more fuel flow than gasoline |
| **E100** | 9.0:1 | 7.8:1 | Pure ethanol racing fuel |
| **Methanol** | 6.5:1 | 5.5:1 | Requires very large injectors |
| **LPG / Propane** | 15.7:1 | 13.5:1 | Gaseous fuel, leaner stoich |

The fuel type determines stoichiometric AFR, safe WOT targets, and enrichment scaling throughout all generated tables.

#### Aspiration

| Type | Description | Effect on Tables |
|------|-------------|-----------------|
| **Naturally Aspirated** | No forced induction | Load range 20–105 kPa. VE curves are smooth and moderate. |
| **Turbocharged** | Exhaust-driven compressor | Load range extends to boost target. VE rises into positive pressure zone. Ignition timing is pulled back under boost. |
| **Supercharged** | Belt/gear-driven compressor | Same boost handling as turbo. Load range extends to boost target. |

When Turbocharged or Supercharged is selected, an additional **Boost Target** field appears.

#### Stroke Type

| Type | Effect |
|------|--------|
| **4-Stroke** | Standard firing order, ignition timing uses standard advance curve |
| **2-Stroke** | Ignition timing is reduced by approximately 30% due to shorter combustion cycle |

#### Injection Mode

| Mode | Description | reqFuel Divider |
|------|-------------|----------------|
| **Sequential** | Each injector fires individually, once per engine cycle | Cylinder count |
| **Batch** | Pairs of injectors fire together | Cylinder count ÷ 2 |
| **Simultaneous** | All injectors fire at once | 1 |
| **Throttle Body** | Single-point injection at the throttle body | 1 |

The injection mode affects the **reqFuel** calculation. Sequential injection divides the fuel delivery across all cylinders, requiring a longer pulse per injection event. Simultaneous injects all fuel at once, requiring a shorter pulse.

#### Ignition Mode

| Mode | Description |
|------|-------------|
| **Wasted Spark** | Two cylinders share one coil. The spark fires on both the compression and exhaust strokes. Common on 4-cyl engines. |
| **Coil on Plug** | Each cylinder has its own ignition coil. Most precise timing control. |
| **Distributor** | Single coil with mechanical distribution. Found on classic engines. |

The ignition mode does not significantly affect generated timing values, but it's recorded in the base map for ECU configuration purposes.

#### Idle RPM

Target engine idle speed. Typically:
- 700–900 RPM for passenger cars
- 1000–1200 RPM for motorcycles and small engines

Valid range: 400 – 2,000 RPM. This sets the lowest RPM bin and the idle region in all tables.

#### Redline RPM

Maximum engine RPM. The highest RPM bin will be set near this value.

Valid range: 2,000 – 20,000 RPM.

### Optional Fields

#### Boost Target (kPa absolute)

Only shown when aspiration is Turbocharged or Supercharged. This is the **absolute** manifold pressure at full boost.

- Atmospheric pressure ≈ 101.3 kPa
- 200 kPa absolute = ~1 bar boost = ~14.5 psi gauge
- 250 kPa absolute = ~1.5 bar boost = ~21.6 psi gauge

The dialog shows a real-time conversion to bar and psi gauge pressure as you type.

Valid range: 120 – 400 kPa.

#### Target WOT AFR (optional)

Override the default safe WOT (wide-open throttle) AFR. Leave blank to use the fuel type's safe default.

Only adjust this if you know your engine's specific requirements. Running too lean at WOT can cause detonation and severe engine damage.

---

## Generating and Previewing

After filling in your engine specs, click **Generate Base Map**. The generator runs instantly and switches to a **preview view** with four tabs.

### Summary Bar

At the top, a summary bar shows key computed values:
- **reqFuel**: The calculated base fuel pulse width in milliseconds
- **Prime**: Cold-start priming pulse duration
- **RPM range**: The span of the generated RPM bins

### VE Table Tab

Shows the generated volumetric efficiency table as a grid:
- **Rows**: Load (kPa), highest at top
- **Columns**: RPM, lowest on the left
- **Values**: VE percentage (0–255)

For NA engines, VE values typically range from 20% at low load/low RPM to 85–95% at peak power. Turbocharged engines will show values rising above 100% in the positive-pressure zone.

RPM bins are **logarithmically spaced** — closer together at the bottom (where idle/cruise sensitivity matters) and wider apart near redline. Load bins span from ~20 kPa (deceleration/vacuum) up to 105 kPa (NA) or your boost target (forced induction).

### Ignition Table Tab

Shows the ignition advance table in degrees BTDC (Before Top Dead Center):
- Higher values = more advance
- Values decrease at high load (to prevent detonation)
- Boosted engines show significantly less advance under positive pressure
- 2-stroke engines use ~30% less advance than 4-stroke equivalents

Typical generated ranges:
- Idle: 10–15°
- Cruise: 25–35°
- WOT NA: 28–32°
- WOT Turbo (at boost): 12–20°

### AFR Target Tab

Shows the target air-fuel ratio across the operating range:
- **At low load**: Slightly lean of stoich (for fuel economy)
- **At cruise**: Near stoic (14.7:1 for gasoline)
- **At WOT**: Rich for safety (12.5:1 for gasoline, or your custom target)

The transition from stoich to rich is gradual, following the load axis. The generator automatically scales all values for your chosen fuel type's stoichiometric ratio.

### Enrichments Tab

Shows all the non-table settings the generator produces:

**Cranking Enrichment** — Extra fuel during cranking, indexed by coolant temperature:
- Very cold (−30°C): ~300% enrichment
- Freezing (0°C): ~200% enrichment
- Warm (80°C): ~115% enrichment

**Warmup Enrichment** — Extra fuel as the engine warms up:
- −30°C: ~145% of normal fuel
- 0°C: ~130%
- 40°C: ~115%
- 80°C+: 100% (no enrichment)

**Acceleration Enrichment** — Extra fuel on sudden throttle opening:
- TPS Threshold: Rate of throttle change that triggers enrichment (%/sec)
- Enrichment percentage: How much extra fuel to add
- Duration: How many engine cycles the enrichment lasts

**Idle Air Control** — Duty cycle for the idle control valve:
- Cold start percentage: IAC opening when engine is cold (~70%)
- Warm idle percentage: IAC opening once warmed up (~30%)
- Warm threshold: Temperature at which to transition (~70°C)

---

## Applying the Base Map

After reviewing the preview, you have three options:

### ← Edit Specs
Go back to the engine specification form to change values and re-generate.

### Cancel
Close the dialog without applying anything.

### Apply to Current Tune / Use as Starting Tune

The button label changes based on context:
- **Apply to Current Tune** — If you already have a project open, this writes the generated VE table values into your current tune's VE table
- **Use as Starting Tune** — If no project is open, you'll be guided to create a new project with these values as the starting point

---

## What Gets Generated

The complete base map includes:

| Component | Description |
|-----------|-------------|
| **RPM bins** | 16 logarithmically spaced RPM values from idle to redline |
| **Load bins** | 16 kPa values from vacuum to atmospheric (NA) or boost target |
| **VE table** | 16×16 volumetric efficiency values (%) |
| **Ignition table** | 16×16 ignition advance values (° BTDC) |
| **AFR target table** | 16×16 air-fuel ratio targets |
| **Cranking enrichment** | Temperature-indexed fuel enrichment curve |
| **Warmup enrichment** | Temperature-indexed warm-up correction curve |
| **Accel enrichment** | TPS rate threshold, percentage, duration, and taper |
| **IAC config** | Cold start, warm idle duty, and threshold temperature |
| **Prime pulse** | Cold-start priming pulse in milliseconds |
| **reqFuel** | Calculated base fuel pulse width |
| **Scalars** | Additional computed values (stoich AFR, divider, etc.) |

---

## Real-World Examples

### Example 1: 4-Cylinder NA Street Car (Honda B18)

| Setting | Value |
|---------|-------|
| Cylinders | 4 |
| Displacement | 1834 cc |
| Injector Size | 290 cc/min |
| Fuel Type | Gasoline |
| Aspiration | Naturally Aspirated |
| Stroke Type | 4-Stroke |
| Injection Mode | Sequential |
| Ignition Mode | Coil on Plug |
| Idle RPM | 800 |
| Redline | 7800 |

**Result**: reqFuel ≈ 11.7 ms, VE ranging 25–90%, ignition 12–34°, AFR 14.7 cruise / 12.5 WOT.

### Example 2: Turbocharged 4-Cylinder (EJ25 with GT35)

| Setting | Value |
|---------|-------|
| Cylinders | 4 |
| Displacement | 2457 cc |
| Injector Size | 1000 cc/min |
| Fuel Type | Gasoline |
| Aspiration | Turbocharged |
| Boost Target | 220 kPa (≈1.2 bar / 17 psi) |
| Stroke Type | 4-Stroke |
| Injection Mode | Sequential |
| Ignition Mode | Wasted Spark |
| Idle RPM | 800 |
| Redline | 6500 |

**Result**: reqFuel ≈ 9.0 ms, VE rising to 130% under boost, ignition pulled to 15° under boost, AFR 12.0 at WOT.

### Example 3: E85 Drift Car

| Setting | Value |
|---------|-------|
| Cylinders | 6 |
| Displacement | 2997 cc |
| Injector Size | 1200 cc/min |
| Fuel Type | E85 |
| Aspiration | Turbocharged |
| Boost Target | 250 kPa (≈1.5 bar / 21.5 psi) |
| Stroke Type | 4-Stroke |
| Injection Mode | Sequential |
| Ignition Mode | Coil on Plug |
| Idle RPM | 900 |
| Redline | 7000 |

**Result**: reqFuel ≈ 4.9 ms, stoich AFR 9.8:1, WOT target 8.5:1, ignition ~12° under full boost.

---

## How the Generator Works (Technical Details)

### reqFuel Calculation

The base fuel pulse width is calculated as:

```
reqFuel = (displacement_per_cylinder × stoichiometric_AFR × 10) / (injector_flow × divider)
```

Where:
- `displacement_per_cylinder` = total displacement ÷ cylinder count (cc)
- `stoichiometric_AFR` = fuel-type-specific ratio (e.g., 14.7 for gasoline)
- `injector_flow` = rated injector flow (cc/min)
- `divider` = injection mode factor (sequential = cylinder count, batch = cylinder count ÷ 2, simultaneous = 1)

The result is clamped to 0.1 – 25.5 ms (valid range for Speeduino U08 constant).

### RPM Bin Spacing

RPM bins are distributed using **logarithmic spacing**:
- Bins are closer together at low RPM (critical for idle stability and cruise)
- Bins spread out toward redline (where sensitivity is lower)
- First bin = idle RPM, last bin = redline RPM, 16 bins total

### VE Table Shape

The VE table follows a physics-informed curve:
- **Low load / low RPM**: Low VE (partial throttle, low airflow) — typically 20–35%
- **Low load / high RPM**: Slightly higher (pumping losses at speed) — typically 30–45%
- **High load / mid RPM**: Peak VE (volumetric resonance range) — typically 80–95%
- **High load / high RPM**: Slight decrease (flow restrictions) — typically 75–85%
- **Boost zone** (forced induction only): VE rises above 100%, proportional to boost pressure

### Ignition Curve Shape

Ignition advance follows the principle of MBT (Minimum spark advance for Best Torque):
- **Low load**: Higher advance (less mixture density, slower burn)
- **High load**: Lower advance (higher density, faster burn, knock risk)
- **Boost zone**: Significantly retarded (high knock risk under boost)
- **2-stroke**: Approximately 30% less advance overall

---

## Safety Notes

- The generated map runs **intentionally rich** — better to waste fuel than to damage your engine
- Ignition timing is **conservative** — leaving power on the table is safer than detonation
- Always verify your **sensor calibrations** (coolant temp, wideband O2, MAP) before relying on the tune
- Use [AutoTune](./autotune.md) to refine the VE table using real-world wideband feedback
- **Never tune WOT on the road** — use a dyno for high-load optimization
- Monitor for knock/detonation, especially on boosted or high-compression engines

---

## See Also

- [Getting Started — Generating a Base Map](../getting-started/first-project.md#generating-a-base-map)
- [AutoTune](./autotune.md) — Automatically refine VE tables using wideband feedback
- [Table Editing](./table-editing.md) — Manually adjust table values
- [Settings — ECU Definitions](../getting-started/settings.md#ecu-definitions) — Manage your INI file library
