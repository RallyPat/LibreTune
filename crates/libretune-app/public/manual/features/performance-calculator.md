# Performance Calculator

The Performance Calculator estimates your vehicle's horsepower, torque, and acceleration performance based on engine parameters and tune data.

## Overview

The Performance Calculator helps you:
- Estimate wheel horsepower and torque
- Predict acceleration times (0-60 mph, quarter mile)
- Visualize power curves across RPM range
- Compare performance before and after tune changes
- Understand how tuning affects overall performance

## Opening the Calculator

1. Go to **Tuning → Performance Calculator**
2. Configure your vehicle specs (see below)
3. Enter target AFR and boost pressure (if turbo)
4. View calculated results

## Vehicle Specifications

### Basic Settings

**Weight**: Total vehicle weight in pounds or kilograms
- Include fuel, driver, and cargo
- Heavier = slower acceleration

**Tire Diameter**: Outside diameter of your tires in inches
- Affects RPM-to-speed conversion
- Check sidewall: e.g., "P225/50R16" means ~25" diameter

**Gear Ratios**: Final drive and transmission ratios
- Example: 3.73:1 final drive, 1st gear 2.97
- Affects torque multiplication at wheels

**Drag Coefficient**: Aerodynamic drag factor (0.25-0.40 for cars)
- Stock sedan: ~0.30
- Sports car: ~0.27
- High-drag pickup: ~0.35
- **Note**: This is a simplified estimate

### Engine Settings

**Type**: Naturally aspirated, turbocharged, supercharged
- Affects VE curve and boost/altitude behavior

**Displacement**: Engine size in cubic inches or liters
- Used for rough horsepower estimates

**Target AFR**: Desired air-fuel ratio
- 14.7 for gasoline
- 12.5-13.0 for turbocharged
- 11.0-12.0 for high-boost/race

**Boost Pressure** (turbo/supercharger):
- In PSI or bar
- 0 = naturally aspirated
- Each PSI adds ~5-7% power

## Understanding Results

### Power Curve

The power curve shows estimated horsepower across the RPM range:
- **X-axis**: Engine RPM (800 to redline)
- **Y-axis**: Wheel horsepower
- **Area under curve**: Total work done per cycle
- **Peak location**: Where your engine makes most power

### Torque Curve

Torque shows rotational force at the wheels:
- **Peak torque**: Maximum force available
- **Location**: Usually lower RPM than power peak
- **Plateau**: Sustained torque range for consistent acceleration

### Acceleration Times

Based on physics simulation:
- **0-60 mph**: Time to accelerate from standstill to 60 mph
- **Quarter mile**: Time to complete 1/4 mile (402 meters)
- **Top speed**: Estimated maximum speed (limited by drag/power)

## Factors That Affect Results

### Tuning Impact

| Change | Effect | Power Impact |
|--------|--------|--------------|
| Richer AFR (12.0 vs 14.7) | More fuel, better ignition | +10-15% |
| Higher boost | More air/fuel mixture | +5-7% per PSI |
| Optimized VE table | Better volumetric efficiency | +3-8% |
| Advanced timing | Better combustion | +5-10% |
| Cold air intake | Denser intake charge | +2-3% |

### Vehicle Impact

| Change | Effect | Acceleration |
|--------|--------|--------------|
| Lighter weight | Faster 0-60 | -100 lbs = ~0.1 sec faster |
| Better tires | Grip for acceleration | Good tires = ~0.2 sec faster |
| Lower drag | Higher top speed | Every 0.01 Cd = +5 mph top speed |
| Taller gearing | Higher top speed | Sacrifices 0-60 |

## Workflow

### Step 1: Enter Vehicle Specs
```
Weight:         3200 lbs
Tire diameter:  25.5 inches
Final drive:    3.73:1
Drag coeff:     0.32
```

### Step 2: Enter Engine Settings
```
Type:           Turbocharged
Displacement:   2.0L
Target AFR:     12.5
Boost:          10 PSI
```

### Step 3: View Results
- Power curve displays
- Torque curve displays
- Acceleration times calculated
- Top speed estimated

### Step 4: Compare Tunes
- Adjust AFR/boost
- Change VE table settings
- Compare power curves side-by-side
- See performance delta

## Tips for Accurate Results

### Good Data
- Use actual dyno data if available
- Measure tire diameter directly
- Weigh vehicle with fuel and driver
- Use realistic drag coefficient

### Known Limitations
- Does NOT account for:
  - Transmission losses (typically 10-15%)
  - Turbo spool-up delay
  - Engine braking
  - Traction control losses
  - Gear shift times
- Results are **estimates only**
- Always verify on dyno or track
- Environmental conditions (temp, altitude) affect results

## Real-World Examples

### Example 1: Stock NA Gasoline Engine

```
Weight:    3000 lbs
Engine:    2.4L NA
AFR:       14.7
Result:    ~180 hp, 0-60 in ~8.2 sec
```

### Example 2: Turbocharged with Tune

```
Weight:    3200 lbs
Engine:    2.0L turbo
Boost:     12 PSI
AFR:       12.0
Result:    ~320 hp, 0-60 in ~5.1 sec
```

### Example 3: Effect of Tune Changes

```
Before:    AFR 13.0, 10 PSI  = 280 hp
After:     AFR 12.0, 12 PSI  = 350 hp
Gain:      +70 hp, 0-60 0.6 sec faster
```

## Comparing Before/After Tunes

1. Note baseline performance results
2. Load new tune file
3. Adjust engine settings if changed
4. Compare new power curves
5. Calculate performance delta

## Exporting Results

Save performance data:
- Power and torque curves
- Acceleration time estimates
- Excel/CSV for further analysis
- Screenshots for documentation

## Limitations & Disclaimers

- **Estimates only**: Results are calculated, not measured
- **No dyno required**: But dyno verification is recommended
- **Environmental**: Hot/cold/altitude changes actual results
- **Not for tuning**: Use for understanding, not calibration
- **Your mileage varies**: Individual vehicles behave differently

## See Also

- [AutoTune Usage Guide](./autotune/usage-guide.md) - How AutoTune optimizes power
- [Settings](../getting-started/first-project.md) - Engine configuration
- [Troubleshooting](../reference/troubleshooting.md) - Common calculator issues

