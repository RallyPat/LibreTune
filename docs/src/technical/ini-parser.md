# INI Parser

The INI parser is responsible for reading ECU definition files and converting them into structured data that LibreTune can use for editing, communication, and validation.

## File Format Overview

INI files define:
- **Constants** - Tunable parameters (scalars, arrays, bits)
- **Tables** - 2D/3D tables with axis definitions
- **Menus** - Hierarchical navigation structure
- **Dialogs** - Settings screens with field layouts
- **Output Channels** - Realtime data structure
- **Gauges** - Dashboard gauge configurations

## Parser Architecture

### Single-Pass Streaming Parser

The parser reads the file once, building data structures incrementally:

```
File → Lines → Section Detection → Section Parser → Data Structures
```

**Benefits**:
- Low memory footprint (no AST)
- Fast parsing (no multiple passes)
- Immediate error reporting

**Complexity**: O(n) where n = file size

### State Machine

```rust
struct ParserState {
    current_section: Section,
    current_page: u8,
    last_offset: u16,           // For lastOffset keyword
    nested_depth: u32,          // For nested menus/dialogs
    line_number: usize,         // For error messages
}
```

## Parsing Sections

### [MegaTune] - Metadata

```ini
[MegaTune]
MTversion   = 2.25
queryCommand = "S"
signature   = "rusEFI 2025.08.19"
```

**Parsed Fields**:
- `queryCommand` - Command to retrieve ECU signature
- `signature` - Expected ECU firmware signature (string comparison)

**Signature Matching**:
```rust
pub enum SignatureMatchType {
    Exact,      // Byte-for-byte match
    Partial,    // ECU signature contains INI signature
    Mismatch,   // No match
}
```

### [Constants] - Tunable Parameters

#### Scalar Constants

```ini
page = 1
crankingRpm = scalar, U16, 0, "rpm", 1, 0, 0, 3000, 0
```

**Format**: `name = scalar, type, offset, units, scale, translate, min, max, digits`

**Parsed Structure**:
```rust
pub struct Constant {
    pub name: String,
    pub data_type: DataType,     // U08, S16, U32, F32, etc.
    pub page: u8,
    pub offset: u16,
    pub units: String,
    pub scale: f64,
    pub translate: f64,
    pub min: f64,
    pub max: f64,
    pub digits: u8,
}
```

**Type System**:
```rust
pub enum DataType {
    U08,   // Unsigned 8-bit  (0-255)
    S08,   // Signed 8-bit    (-128 to 127)
    U16,   // Unsigned 16-bit (0-65535)
    S16,   // Signed 16-bit   (-32768 to 32767)
    U32,   // Unsigned 32-bit
    S32,   // Signed 32-bit
    F32,   // 32-bit float
}
```

**Value Conversion** (raw bytes ↔ display value):
```
display_value = (raw_bytes * scale) + translate
raw_bytes = (display_value - translate) / scale
```

**Example**:
```ini
coolantTemp = scalar, U08, 10, "°C", 1.0, -40.0, -40, 150, 0
```
- Raw byte: 80
- Display: (80 × 1.0) + (-40) = 40°C

#### Array Constants (Tables)

```ini
veTable = array, U08, 100, [16x16], "VE", 0.5, 0, 0, 127.5, 1
```

**Format**: `name = array, type, offset, [rows x cols], units, scale, translate, min, max, digits`

**Special Keyword**: `lastOffset`
```ini
afrTable = array, U08, lastOffset, [16x16], "AFR", 0.1, 0, 7, 25.5, 1
```

`lastOffset` = offset of previous constant + size of previous constant

#### Bits Constants (Bitfields)

```ini
engineType = bits, U08, 5, [0:1], "Gasoline", "E85", "Diesel", "Methanol"
```

**Format**: `name = bits, type, offset, [start_bit:end_bit], "value0", "value1", ...`

**Bit Extraction**:
```rust
fn extract_bits(byte: u8, start: u8, end: u8) -> u8 {
    let mask = ((1 << (end - start + 1)) - 1) << start;
    (byte & mask) >> start
}
```

**Example**:
- Byte value: 0b00101101
- Bits [2:4]: (0b00101101 & 0b00011100) >> 2 = 0b011 = 3

### [Menu] - Navigation Structure

```ini
menu = "&Fuel"
    subMenu = veTable, "VE &Table"
    subMenu = afrTable, "AFR Ta&ble"
menu = "&Ignition"
    subMenu = ignTable, "Ignition &Table"
```

**Parsed Structure**:
```rust
pub enum MenuItem {
    Menu { label: String, children: Vec<MenuItem> },
    SubMenu { label: String, target: String },
    Separator,
    Std(String),  // std_realtime, std_ms2gentherm, etc.
}
```

**Special Targets**:
- `std_separator` - Horizontal line
- `std_realtime` - Opens dashboard
- Table names - Opens table editor
- Dialog names - Opens settings dialog

**Keyboard Shortcuts**: `&` prefix marks the hotkey character

### [TableEditor] - Table Definitions

```ini
table = veTableTbl, veTableMap, "VE Table", 1
    topicHelp = "veTableHelp"
    xBins = rpmBins, RPM
    yBins = mapBins, MAP
    zBins = veTable
    gridHeight = 2.0
    gridOrient = 250, 0, 340
    upDownLabel = "(RICHER)", "(LEANER)"
```

**Parsed Structure**:
```rust
pub struct TableDefinition {
    pub name: String,
    pub map_name: Option<String>,  // For menu lookup
    pub title: String,
    pub page: u8,
    pub x_axis: String,            // Constant name for X bins
    pub y_axis: String,            // Constant name for Y bins
    pub z_values: String,          // Constant name for Z values
    pub grid_height: Option<f32>,
    pub grid_orient: Option<(f32, f32, f32)>,
}
```

**Coordinate Systems**:
- **Table coordinates**: `(row, col)` where row=0 is bottom, col=0 is left
- **Array storage**: `z_values[row][col]`
- **3D visualization**: Y-up, X-right, Z-forward

### [OutputChannels] - Realtime Data

```ini
ochBlockSize = 75
ochGetCommand = "r\x00\x34\x00\x4b"

rpm = scalar, U16, 0, "rpm", 1, 0
afr = scalar, U16, 2, "AFR", 0.1, 0
coolant = scalar, S16, 4, "°C", 0.1, 0
```

**Parsed Structure**:
```rust
pub struct OutputChannel {
    pub name: String,
    pub data_type: DataType,
    pub offset: u16,         // Offset in data packet
    pub units: String,
    pub scale: f64,
    pub translate: f64,
}
```

**Protocol**:
1. Send `ochGetCommand` bytes to ECU
2. Read `ochBlockSize` bytes
3. Parse each channel from its offset using data type
4. Apply scale/translate to get display value

### [GaugeConfigurations] - Dashboard Gauges

```ini
gauge = rpmGauge
    displayMode = analog
    min = 0
    max = 8000
    loD = 500
    loW = 1000
    hiW = 6000
    hiD = 7000
    vd = 0
    ld = 1
```

**Parsed Structure**:
```rust
pub struct GaugeInfo {
    pub name: String,
    pub channel: String,
    pub title: String,
    pub units: String,
    pub min: f64,
    pub max: f64,
    pub low_danger: Option<f64>,
    pub low_warning: Option<f64>,
    pub high_warning: Option<f64>,
    pub high_danger: Option<f64>,
}
```

## Expression Evaluation

INI files support expressions in constant definitions:

**Syntax**:
```ini
value = { expression }
```

**Example**:
```ini
reqFuel = { 1000.0 / (injectorFlow * numCylinders / 60.0) }
```

**Supported Operators**:
- Arithmetic: `+`, `-`, `*`, `/`, `%`
- Comparison: `<`, `<=`, `>`, `>=`, `==`, `!=`
- Logical: `&&`, `||`, `!`
- Bitwise: `&`, `|`, `^`, `<<`, `>>`
- Functions: `sqrt()`, `pow()`, `abs()`, `min()`, `max()`

**Implementation**:
```rust
pub fn eval_expression(expr: &str, constants: &HashMap<String, f64>) -> Result<f64, String> {
    // Tokenize
    let tokens = tokenize(expr)?;
    // Parse to AST
    let ast = parse_tokens(tokens)?;
    // Evaluate with constant lookup
    evaluate_ast(&ast, constants)
}
```

**Complexity**: O(n) tokenization + O(n) parsing + O(n) evaluation

## Validation

### Structural Validation

**Checks**:
- All referenced constants exist
- Table axes reference valid array constants
- Menu targets reference valid tables/dialogs
- Gauge channels reference valid output channels
- Page numbers are consistent
- Offsets don't overlap within a page

**Example Error**:
```
Error: Table 'veTableTbl' references unknown X axis 'rpmBinsTypo' (line 245)
Hint: Did you mean 'rpmBins'?
```

### Semantic Validation

**Checks**:
- Data types are appropriate (e.g., RPM should be U16, not S08)
- Min/max ranges are sensible
- Scale factors don't cause overflow
- Array dimensions match axis definitions

**Example Warning**:
```
Warning: Constant 'coolantTemp' min=-40, max=150, but U08 with scale=1.0
can only represent -40 to 215. Consider using scale=0.5 for full range.
```

## Error Recovery

Parser attempts to continue after errors:

**Strategies**:
1. **Skip malformed lines** - Log error, continue to next line
2. **Use defaults** - If optional field missing, use sensible default
3. **Partial parsing** - Return partially parsed definition with warnings

**Example**:
```
Line 123: Invalid constant definition (missing units field)
Skipping constant 'brokenValue'
Parsing continued, 456 constants loaded (1 error, 0 warnings)
```

## Performance

### Parsing Speed

Typical INI files:
- **Small** (Speeduino): ~1000 lines, parsed in <5ms
- **Medium** (rusEFI): ~5000 lines, parsed in <20ms
- **Large** (FOME): ~10000 lines, parsed in <50ms

### Memory Usage

`EcuDefinition` structure size:
- ~50 KB for Speeduino
- ~200 KB for rusEFI
- ~500 KB for FOME (includes embedded documentation)

### Optimization Techniques

1. **String Interning**: Common strings (units, data types) stored once
2. **Lazy Evaluation**: Expressions evaluated on-demand, not during parse
3. **Incremental Hashing**: Structural hash computed as parsing progresses
4. **Zero-Copy Parsing**: Uses string slices where possible

## Source Code Reference

- Main parser: `crates/libretune-core/src/ini/parser.rs`
- Data types: `crates/libretune-core/src/ini/types.rs`
- Constants: `crates/libretune-core/src/ini/constants.rs`
- Expression evaluator: `crates/libretune-core/src/ini/expression.rs`
- Tests: `crates/libretune-core/tests/corpus.rs`

## See Also

- [INI File Format](../reference/ini-format.md) - User-facing INI documentation
- [Supported ECUs](../reference/supported-ecus.md) - ECU-specific INI patterns
- [Tune Migration](./version-control.md#tune-migration) - Handling INI version changes
