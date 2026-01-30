# LibreTune Dashboard System - Technical Overview

## Current Architecture

### Backend (Rust)

LibreTune has **two separate dashboard systems**:

1. **Simple Dashboard System** (`crates/libretune-core/src/dashboard.rs`)
   - Basic gauge types: AnalogDial, DigitalReadout, BarGauge, SweepGauge, LEDIndicator, WarningLight
   - Simple serialization to JSON
   - Used for basic LibreTune-native dashboards
   - Converts to/from TS format via `convert_layout_to_dashfile` / `convert_dashfile_to_layout`

2. **TunerStudio-Compatible System** (`crates/libretune-core/src/dash/`)
   - **Full TS .dash/.gauge format support**
   - 18 gauge painter types (all TS gauge types supported)
   - 2 indicator painter types
   - XML parsing and writing (`parser.rs`, `writer.rs`)
   - Type definitions matching TS schema exactly (`types.rs`)
   - **NEW**: Comprehensive validation system (`validation.rs`)

### Frontend (React/TypeScript)

- **TsDashboard Component** (`TsDashboard.tsx`)
  - Renders TunerStudio-compatible dashboards
  - Real-time data subscription (per-channel for efficiency)
  - Gauge sweep animation on load
  - Context menu for gauge configuration
  - Designer mode for layout editing

- **Dashboard Designer** (`DashboardDesigner.tsx`)
  - Drag-to-reposition gauges
  - Resize handles (8-point)
  - Snap-to-grid
  - Multi-select, copy/paste, undo/redo

- **Gauge Renderer** (`TsGauge.tsx`)
  - Canvas-based rendering
  - Supports all 18 TS gauge painter types
  - Embedded font/image loading
  - High-quality visual effects

## What Was Broken

Based on code analysis and the problem statement, the issues were:

### 1. **No Validation or Error Reporting**
- Dashboards could reference non-existent output channels
- Invalid gauge configurations (min > max) went undetected
- Missing embedded images failed silently
- No way to diagnose why a dashboard wouldn't load

### 2. **Generic Error Messages**
- "Failed to load dashboard" with no details
- "Unknown error" during import
- No indication of which gauges had problems
- No channel existence checking

### 3. **Silent Failures**
- Gauges with missing channels showed minimum values
- Unsupported painters fell back to BasicReadout
- Embedded images failed without warning
- No validation before save/import

### 4. **No INI Integration**
- Dashboards weren't validated against ECU definition
- Channel references not checked
- No way to know if a dashboard would work with your ECU

## Improvements Made

### 1. **Comprehensive Validation System**

Created `crates/libretune-core/src/dash/validation.rs`:

```rust
pub fn validate_dashboard(
    dash: &DashFile, 
    ecu_def: Option<&EcuDefinition>
) -> ValidationReport
```

**Validation Checks:**
- ✓ Unknown output channels (compares against INI definition)
- ✓ Invalid min/max ranges
- ✓ Missing embedded images
- ✓ Empty dashboards
- ✓ Overlapping gauges (warning)
- ✓ Tiny gauges (warning)
- ✓ Out-of-bounds gauges (warning)
- ✓ Performance warnings for large dashboards

**ValidationReport Structure:**
```rust
pub struct ValidationReport {
    pub errors: Vec<ValidationError>,      // Critical issues
    pub warnings: Vec<ValidationWarning>,  // Non-critical issues
    pub stats: DashboardStats,             // Dashboard statistics
}
```

### 2. **Detailed Error Types**

```rust
pub enum ValidationError {
    UnknownOutputChannel { gauge_id: String, channel: String },
    InvalidRange { gauge_id: String, min: f64, max: f64 },
    MissingEmbeddedImage { gauge_id: String, image_name: String },
    UnknownIndicatorChannel { indicator_id: String, channel: String },
    EmptyDashboard,
}
```

Each error provides:
- **What went wrong** (unknown channel, invalid range, etc.)
- **Which gauge** has the problem (gauge_id)
- **Specific details** (channel name, min/max values, image name)

### 3. **Dashboard Statistics**

```rust
pub struct DashboardStats {
    pub gauge_count: usize,
    pub indicator_count: usize,
    pub unique_channels: usize,
    pub embedded_image_count: usize,
    pub has_embedded_fonts: bool,
}
```

Helps users understand:
- Complexity of the dashboard
- How many data channels are needed
- Resource usage (images, fonts)

### 4. **Tauri Command Integration**

Added `validate_dashboard` command that:
- Loads ECU definition for the project (if available)
- Validates dashboard against actual INI output channels
- Returns detailed ValidationReport
- Logs validation results for debugging

### 5. **Comprehensive Test Suite**

**Parser Tests** (`tests/dash_parsing.rs`):
- ✓ Parse basic dashboard with multiple gauge types
- ✓ Parse gauge colors (ARGB format)
- ✓ Parse different painter types
- ✓ Handle invalid XML
- ✓ Parse empty dashboards
- ✓ Parse minimal gauges
- ✓ Parse .gauge files

**Validation Tests** (in `validation.rs`):
- ✓ Detect empty dashboards
- ✓ Detect invalid ranges

## TunerStudio .dash File Format

### XML Structure

```xml
<?xml version="1.0" encoding="UTF-8"?>
<dashboard>
  <gaugeCluster>
    <dashComp type="Gauge">
      <Title>Engine Speed</Title>
      <OutputChannel>rpm</OutputChannel>
      <GaugePainter>Tachometer</GaugePainter>
      <Min>0</Min>
      <Max>8000</Max>
      <Units>rpm</Units>
      <ValueDigits>0</ValueDigits>
      <BackColor>-16777216</BackColor>  <!-- ARGB as signed 32-bit int -->
      <FontColor>-1</FontColor>
      <NeedleColor>-65536</NeedleColor>
      <RelativeX>0.0125</RelativeX>      <!-- 0.0-1.0 range -->
      <RelativeY>0.02</RelativeY>
      <RelativeWidth>0.375</RelativeWidth>
      <RelativeHeight>0.625</RelativeHeight>
    </dashComp>
    
    <dashComp type="Indicator">
      <OnText>Battery OK</OnText>
      <OffText>Low Batt</OffText>
      <OutputChannel>battery</OutputChannel>
      <Painter>Bulb Indicator</Painter>
      <OnBackgroundColor>-16711936</OnBackgroundColor>
      <OffBackgroundColor>-8355712</OffBackgroundColor>
    </dashComp>
  </gaugeCluster>
</dashboard>
```

### Key Points

1. **Element Names**: PascalCase (e.g., `<Title>`, `<OutputChannel>`, `<GaugePainter>`)
2. **Gauge Types**: Use `<GaugePainter>` element with TS names like "Tachometer", "Vertical Bar Gauge"
3. **Indicator Types**: Use `<Painter>` element with names like "Bulb Indicator", "Basic Rectangle Indicator"
4. **Colors**: ARGB format as signed 32-bit integers (e.g., -16777216 = black, -1 = white, -65536 = red)
5. **Positioning**: Relative coordinates 0.0-1.0 (`RelativeX`, `RelativeY`, `RelativeWidth`, `RelativeHeight`)

### Supported Gauge Painter Types (18 total)

- `Tachometer`
- `Round Analog Gauge` / `Round Gauge`
- `Round Dashed Gauge`
- `Fuel Meter`
- `Analog Gauge`
- `Basic Analog Gauge`
- `Circle Analog Gauge`
- `Asymetric Sweep Gauge` (note: TS spelling)
- `Basic Readout`
- `Horizontal Bar Gauge`
- `Vertical Bar Gauge`
- `Horizontal Line Gauge`
- `Horizontal Dashed Bar Gauge`
- `Vertical Dashed Bar Gauge`
- `Analog Bar Gauge`
- `Analog Moving Bar Gauge`
- `Histogram`
- `Line Graph`

### Supported Indicator Painter Types (2 total)

- `Bulb Indicator`
- `Basic Rectangle Indicator`

## Usage Examples

### Validating a Dashboard

```typescript
// Frontend (React)
const handleValidate = async () => {
  const report = await invoke('validate_dashboard', {
    dashFile: currentDash,
    projectName: 'my_project',  // Optional: enables INI validation
  });
  
  if (report.errors.length > 0) {
    console.error('Dashboard has errors:', report.errors);
    // Show error dialog with details
  }
  
  if (report.warnings.length > 0) {
    console.warn('Dashboard has warnings:', report.warnings);
    // Show warnings in UI
  }
  
  console.log('Dashboard stats:', report.stats);
};
```

### Creating a Valid Dashboard

```rust
// Backend (Rust)
let mut dash = DashFile::default();

// Add a gauge
let mut rpm_gauge = GaugeConfig::default();
rpm_gauge.id = "gauge_rpm".to_string();
rpm_gauge.title = "Engine Speed".to_string();
rpm_gauge.output_channel = "rpm".to_string();  // Must match INI
rpm_gauge.min = 0.0;
rpm_gauge.max = 8000.0;
rpm_gauge.gauge_painter = GaugePainter::Tachometer;

dash.gauge_cluster.components.push(
    DashComponent::Gauge(Box::new(rpm_gauge))
);

// Validate before saving
let report = validate_dashboard(&dash, Some(&ecu_definition));
if report.is_valid() {
    save_dash_file(&dash, &path)?;
}
```

## Roadmap

### Phase 3: Frontend Integration (Next Steps)

- [ ] Update `ImportDashboardDialog` to call `validate_dashboard`
- [ ] Show validation errors with gauge IDs and channel names
- [ ] Add "View Issues" button that highlights problematic gauges
- [ ] Create `ValidationReportDialog` component
- [ ] Add "Ignore Warning" checkbox for non-critical issues
- [ ] Show validation status icon next to dashboard name

### Phase 4: Enhanced Features

- [ ] Auto-fix common issues (e.g., swap min/max if inverted)
- [ ] Suggest alternative channels if exact match not found
- [ ] Highlight gauges with issues in designer mode
- [ ] Export validation report as text/JSON
- [ ] Add "Test Dashboard" mode with dummy data

### Phase 5: Native LibreTune Format (Future)

- [ ] Design enhanced `.ltdash` format with new features:
  - Advanced expressions for dynamic min/max
  - Animation support (smooth transitions, effects)
  - Theme system (dark/light modes, color schemes)
  - Layout templates and presets
  - Conditional visibility (show gauge only when condition met)
  - Chart gauge types (trend lines, scatter plots)
- [ ] Implement parser and writer for new format
- [ ] Create migration tool from TS .dash to .ltdash
- [ ] Backward compatibility: always export to .dash for TS users

## Testing

### Running Tests

```bash
# Parser tests
cargo test -p libretune-core --test dash_parsing

# Validation tests
cargo test -p libretune-core --lib validation

# All dashboard tests
cargo test -p libretune-core dash
```

### Test Coverage

- ✅ Parser: 7 tests passing
- ✅ Validation: 2 tests passing
- ✅ Example fixture: `tests/fixtures/dashboards/basic.dash`

## Contributing

When adding new gauge types or features:

1. Update `types.rs` with new types
2. Update `parser.rs` to parse new elements
3. Update `writer.rs` to write new elements
4. Update `validation.rs` with new validation rules
5. Add test cases in `dash_parsing.rs`
6. Update this documentation

## References

- TunerStudio documentation: [tunerstudio.com](https://www.tunerstudio.com/)
- LibreTune INI parsing: `crates/libretune-core/src/ini/`
- Dashboard examples: `crates/libretune-core/tests/fixtures/dashboards/`
