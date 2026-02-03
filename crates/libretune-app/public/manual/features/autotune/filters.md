# Filters and Authority

Control data quality and limit correction magnitude.

## Why Filter?

Not all AFR readings represent steady-state engine behavior:
- **Acceleration**: Fuel enrichment skews readings
- **Deceleration**: Fuel cut skews readings lean
- **Cold engine**: Different fuel requirements
- **Sensor lag**: O2 sensor response time

Filters ensure only valid data affects recommendations.

## Filter Types

### RPM Filter
**Purpose**: Focus on relevant operating range

| Setting | Description | Default |
|---------|-------------|---------|
| Min RPM | Ignore below this | 800 |
| Max RPM | Ignore above this | 6500 |

**When to adjust**:
- Raise min if idle is unstable
- Lower max if you don't rev that high

### Coolant Temperature Filter
**Purpose**: Ignore cold engine data

| Setting | Description | Default |
|---------|-------------|---------|
| Min CLT | Ignore below this | 160°F |

**When to adjust**:
- Lower for cold climate testing
- Raise if engine needs longer warmup

### Throttle Position Filters
**Purpose**: Ignore transient conditions

| Setting | Description | Default |
|---------|-------------|---------|
| Min TPS | Ignore closed throttle | 1% |
| Max TPS Rate | Ignore rapid changes | 10%/sec |

**When to adjust**:
- Raise min TPS if decel noise is an issue
- Lower max rate if you want more data (less filtering)

### Accel Enrichment Filter
**Purpose**: Ignore acceleration enrichment periods

| Setting | Description | Default |
|---------|-------------|---------|
| Exclude AE | Ignore when AE active | Yes |

Most ECUs flag when acceleration enrichment is active. This filter excludes that data.

## Authority Limits

### Why Limit Authority?

Large corrections can be dangerous:
- **Too rich**: Washes cylinders, carbon buildup
- **Too lean**: Detonation, melted pistons
- **Wrong direction**: Compounding errors

Authority limits prevent runaway corrections.

### Per-Update Limits

| Setting | Description | Default |
|---------|-------------|---------|
| Max Increase | Most fuel to add per update | 10% |
| Max Decrease | Most fuel to remove per update | 10% |

Each time changes are applied, cells can only change by this much.

### Absolute Limits

| Setting | Description | Default |
|---------|-------------|---------|
| Absolute Max | Total change from original | 25% |

Even with multiple updates, a cell can't deviate more than this from the original value.

### Progressive Strategy

Start conservative and increase as tune improves:

| Stage | Per-Update | Absolute | When |
|-------|------------|----------|------|
| Initial | 5% | 15% | Base tune rough |
| Refining | 10% | 25% | Getting close |
| Fine-tuning | 15% | 30% | Nearly dialed |

## Filter Indicators

### Status Bar
Shows current filter status:
- ✅ **Accepting**: Data passes filters
- ❌ **Filtered**: Data rejected (reason shown)

### Cell Warnings
- ⚠️ **Authority limit hit**: Recommendation larger than allowed
- 🔒 **Cell locked**: Manually excluded from updates

## Troubleshooting Filters

### "All data filtered"
1. Check CLT - engine may be cold
2. Check RPM range - may be too narrow
3. Check TPS rate - may be too strict

### "Not enough hits"
1. Loosen filters to accept more data
2. Drive more in target conditions
3. Wait longer in each cell

### "Erratic recommendations"
1. Tighten TPS rate filter
2. Enable accel enrichment exclusion
3. Check for sensor issues
