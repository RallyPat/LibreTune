# INI File Format

Understanding ECU definition files.

## Overview

INI files define:
- ECU memory layout
- Configuration constants
- Tables and curves
- Output channels
- Gauge configurations
- Menu structure

## File Structure

```ini
[MegaTune]
signature = "speeduino 202310"
queryCommand = "Q"
...

[Constants]
; Memory layout and defaults
...

[OutputChannels]
; Real-time data definitions
...

[TableEditor]
; Table definitions
...

[GaugeConfigurations]
; Gauge setup
...

[Menu]
; Menu structure
...
```

## Key Sections

### [MegaTune]
ECU identification and commands:
```ini
signature = "speeduino 202310"
queryCommand = "Q"
versionInfo = "S"
```

### [Constants]
Memory-mapped values:
```ini
page = 1
blockingFactor = 128
reqFuel = scalar, U08, 0, "ms", 0.1, 0, 0, 25.5
```

### [OutputChannels]
Real-time data:
```ini
rpm = scalar, U16, 0, "RPM", 1, 0
map = scalar, U16, 2, "kPa", 0.1, 0
```

### [TableEditor]
Table definitions:
```ini
table = veTable1Tbl, veTable1Map, "VE Table 1", 1
  xBins = rpmBins, rpm
  yBins = mapBins, map
  zBins = veTable1
```

## Data Types

| Type | Size | Range |
|------|------|-------|
| U08 | 1 byte | 0-255 |
| S08 | 1 byte | -128 to 127 |
| U16 | 2 bytes | 0-65535 |
| S16 | 2 bytes | -32768 to 32767 |
| U32 | 4 bytes | 0-4294967295 |
| F32 | 4 bytes | Floating point |

## Expressions

Constants can use expressions:
```ini
value = scalar, U08, 0, "", 1, 0, 0, { nCylinders * 2 }
```

Supported operators: `+`, `-`, `*`, `/`, `?:`, `&&`, `||`

## Menu Definitions

Define application menus:
```ini
menu = "Fuel"
  subMenu = veTable1Tbl, "VE Table 1"
  subMenu = reqFuelCalc, "Required Fuel"
  subMenu = injCharacteristics, "Injector Characteristics"
```

## Dialog Definitions

Settings screens are built in the `[UserDefined]` section from named dialogs
referenced by `subMenu` (or embedded via `panel =`):

```ini
[UserDefined]
dialog = moreIgnitionOptions, "Ignition Options"
    field = "Use Prediction", use_prediction
    checkbox = "Use Prediction", use_prediction, { !use_prediction }
    checkbox = "Don't use Prediction", use_prediction, flipValue, { use_prediction }
```

Common directives: `field` (labeled constant editor), `checkbox` (bound
toggle), `panel` (embed another dialog), `commandButton`, `settingSelector`,
`gauge`, `indicator`, `runtimeValue`, `table`.

**Checkbox directives** render as real checkboxes bound to the constant:
checked ⇄ non-zero by default; the optional `flipValue` token inverts the
sense (checked ⇄ 0). Brace conditions follow the same convention as fields —
a single `{ expression }` enables/disables the control (greyed out, not
hidden), two braces give visibility then enable. Two checkboxes on the same
constant with opposite conditions act as a radio pair, exactly as in
TunerStudio.

## Gauge Definitions

Configure gauges:
```ini
[GaugeConfigurations]
rpmGauge = rpm, "Engine Speed", "RPM", 0, 8000, 300, 600, 6500, 7500
```

Format: channel, title, units, min, max, lowWarn, lowDanger, highWarn, highDanger

## Modifying INI Files

**Warning**: Modifying INI files can cause problems:
- Incorrect offsets corrupt data
- Wrong types cause misreads
- Always backup original

For custom ECUs:
1. Start from similar existing INI
2. Modify gradually
3. Test each change
4. Document modifications

## Resources

- Speeduino INI documentation
- rusEFI INI reference
- MegaSquirt INI format guide
