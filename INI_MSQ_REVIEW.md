# INI and MSQ Parsing Review Against EFI Analytics Spec

## Overview
This document reviews the current LibreTune implementation against the EFI Analytics ECU Definition files specification to identify gaps and ensure correctness.

## 1. INI File Parsing Review

### 1.1 [TunerStudio] Section ✅
**Spec Requirements:**
- `iniSpecVersion` - Version of INI spec used
- `pageSizes` - Comma-separated list of page sizes
- `nPages` - Number of pages
- `endianness` - "little" or "big"
- `queryCommand` - Command to query ECU signature
- `delayAfterPortOpen` - Delay in ms after opening port
- `interWriteDelay` - Delay between writes
- `pageActivationDelay` - Delay after burn command
- `messageEnvelopeFormat` - Protocol envelope format
- `defaultIpAddress` / `defaultIpPort` - Network defaults

**Current Implementation:**
- ✅ All attributes parsed correctly
- ✅ Handles both [MegaTune] and [TunerStudio] sections
- ✅ Protocol settings properly stored

**Issues Found:**
- ⚠️ `messageEnvelopeFormat` parsing has syntax error (line 372-375 in parser.rs) - missing opening brace

### 1.2 [Constants] Section ✅
**Spec Requirements:**
- Format: `constantName = dataType, size, offset, [shape], "label", scale, offset, min, max, [bit_options]`
- Supports `lastOffset` keyword for offset
- Supports expressions in scale/offset/min/max
- Supports bit fields with `[bit_position:bit_size]`
- Supports arrays with `[rows x cols]` or `[size]`

**Current Implementation:**
- ✅ Basic parsing works
- ✅ `lastOffset` keyword supported
- ✅ Bit fields supported
- ✅ Arrays supported
- ✅ Expressions in scale/offset supported

**Issues Found:**
- ⚠️ Expression evaluation might not handle all functions from spec (see section 22.3)
- ⚠️ Need to verify all data types are handled (U08, S08, U16, S16, U32, S32, F32, F64, String, Bits)

### 1.3 [PcVariables] Section ✅
**Spec Requirements:**
- Format: `name = dataType, size, [bit_position:bit_size], "label", scale, offset, min, max, [bit_options]`
- No offset field (calculated from previous)
- Used for runtime values, not stored in ECU

**Current Implementation:**
- ✅ Parsed correctly
- ✅ Stored in `def.constants` with `is_pc_variable` flag
- ✅ Values stored in `TuneCache.local_values` (not written to ECU)

**Issues Found:**
- None identified

### 1.4 [TableEditor] Section ✅
**Spec Requirements:**
- Format: `table = tableName, mapName, "Title", page`
- References constants for X bins, Y bins, Z data
- Supports `xyLabel` attribute (iniSpec 3.49+)

**Current Implementation:**
- ✅ Parsed correctly
- ✅ Map name lookup supported (`get_table_by_name_or_map`)
- ✅ Table dimensions resolved from referenced constants

**Issues Found:**
- ⚠️ `xyLabel` attribute not parsed (if needed)

### 1.5 [Menu] Section ✅ (Recently Fixed)
**Spec Requirements:**
- `menu = "MenuName"` - Top-level menu
- `subMenu = target, "Label", {enable_condition}` - 1 condition = enable
- `subMenu = target, "Label", {visibility_condition}, {enable_condition}` - 2 conditions
- `groupMenu = "Label", {visibility_condition}, {enable_condition}` - Group container
- `groupChildMenu = target, "Label", {conditions}` - Child of group

**Current Implementation:**
- ✅ Recently updated to support separate visibility/enable conditions
- ✅ Recursive filtering of SubMenu items
- ✅ Conditions evaluated at runtime

**Issues Found:**
- None identified

### 1.6 [UserDefined] / [UiDialogs] Section ✅
**Spec Requirements:**
- `dialog = name, "Title", {condition}`
- `field = "Label", constantName, {condition}`
- `panel = "Label", position` - North, Center, East, West, South
- `label = "Text"`
- `table = tableName` - Reference to table
- `indicatorPanel = name` - Reference to indicator panel

**Current Implementation:**
- ✅ Dialog parsing works
- ✅ Field conditions supported
- ✅ Panel structure supported
- ✅ Table references work

**Issues Found:**
- ⚠️ `indicatorPanel` not implemented
- ⚠️ Panel visibility conditions not fully implemented

### 1.7 [OutputChannels] Section ✅
**Spec Requirements:**
- Format: `name = dataType, offset, "label", scale, offset, min, max, units`
- Used for real-time data streaming

**Current Implementation:**
- ✅ Parsed correctly
- ✅ Used for real-time data

**Issues Found:**
- None identified

### 1.8 [GaugeConfigurations] Section ✅
**Spec Requirements:**
- Format: `name = channel, "title", "units", min, max, ...`
- Used for dashboard gauges

**Current Implementation:**
- ✅ Parsed correctly
- ✅ Used in dashboard system

**Issues Found:**
- None identified

### 1.9 Expression Evaluation ⚠️
**Spec Requirements (Section 22):**
- Operators: `+`, `-`, `*`, `/`, `%`, `==`, `!=`, `<`, `>`, `<=`, `>=`, `&&`, `||`, `!`
- Functions: `min()`, `max()`, `abs()`, `round()`, `floor()`, `ceil()`, `sqrt()`, `pow()`, `log()`, `exp()`, `sin()`, `cos()`, `tan()`, `asin()`, `acos()`, `atan()`, `atan2()`, `isNaN()`, `isAdvancedMathAvailable()`
- String functions: `bitStringValue()`, `stringValue()`, `$getProjectsDirPath()`, `$getWorkingDirPath()`

**Current Implementation:**
- ✅ Basic operators supported
- ✅ Some functions supported
- ⚠️ Need to verify all functions are implemented
- ⚠️ String functions not fully implemented

## 2. MSQ File Parsing Review

### 2.1 MSQ File Structure ✅
**Spec Requirements:**
- XML format with namespace `http://www.msefi.com/:msq`
- `<versionInfo signature="..."/>` - ECU signature
- `<bibliography>` - Metadata (author, writeDate, etc.)
- `<page number="N">` - Page containers
- `<constant name="...">value</constant>` - Constant values
- `<pcVariable name="...">value</pcVariable>` - PC variable values

**Current Implementation:**
- ✅ XML parsing works
- ✅ Signature extracted from `<versionInfo>`
- ✅ Constants and pcVariables extracted
- ✅ Value parsing handles Scalar, Array, String, Bool
- ✅ `<bibliography>` section parsed (author, writeDate)
- ✅ `<page number="N">` structure utilized (page tracked per constant)
- ✅ `<pageData page="N">hexcontent</pageData>` parsed from MSQ

**Issues Found:**
- None identified

### 2.2 Value Parsing ✅
**Spec Requirements:**
- Scalar values: numeric
- Arrays: space-separated or comma-separated numbers
- Strings: quoted or unquoted
- Booleans: "true"/"false"

**Current Implementation:**
- ✅ Handles all value types
- ✅ Quoted strings preserved
- ✅ Arrays parsed correctly
- ✅ Booleans handled

**Issues Found:**
- None identified

### 2.3 MSQ Saving ✅
**Spec Requirements:**
- Should preserve page structure
- Should include bibliography
- Should format arrays nicely

**Current Implementation:**
- ✅ Basic MSQ saving works
- ✅ Constants saved to appropriate pages (preserves original page structure)
- ✅ Bibliography saved
- ✅ Page data saved for raw binary pages

**Issues Found:**
- None identified

## 3. Protocol Communication Review

### 3.1 Command Format ✅
**Spec Requirements:**
- Commands defined in INI with format strings
- `%2i` = page identifier (2 bytes)
- `%2o` = offset (2 bytes)
- `%2c` = count/length (2 bytes)
- `%v` = value bytes
- `$tsCanId` substitution for CAN ID

**Current Implementation:**
- ✅ Command format parsing works
- ✅ Format string substitution implemented
- ✅ Page identifiers supported
- ✅ CAN ID substitution supported

**Issues Found:**
- None identified

### 3.2 Read/Write Operations ✅
**Spec Requirements:**
- Read: `R%2i%2o%2c` format
- Write: `C%2i%2o%2c%v` format (chunked)
- Blocking factor respected
- Inter-write delay respected
- Page activation delay after burn

**Current Implementation:**
- ✅ Read operations work
- ✅ Write operations work
- ✅ Blocking factor respected
- ✅ Delays implemented

**Issues Found:**
- None identified

### 3.3 Message Envelope Format ⚠️
**Spec Requirements:**
- `msEnvelope_1.0` - CRC framing
- Other formats possible

**Current Implementation:**
- ✅ CRC framing implemented
- ⚠️ Parser syntax error for `messageEnvelopeFormat` (line 372-375)

## 4. Critical Issues to Fix

### All Critical Issues Resolved ✅

The following items have been addressed:

1. ~~**Parser Syntax Error**~~ - ✅ Fixed: `messageEnvelopeFormat` parsing corrected
2. ~~**MSQ Page Structure**~~ - ✅ Fixed: Page numbers preserved from MSQ
3. ~~**MSQ Bibliography**~~ - ✅ Fixed: Bibliography metadata parsed and saved
4. ~~**Expression Functions**~~ - ✅ Implemented: All critical math functions from spec
5. ~~**String Functions**~~ - ✅ Implemented: `bitStringValue()`, `stringValue()` supported
6. ~~**Panel Visibility**~~ - ✅ Implemented: Panel-level visibility conditions work
7. ~~**Indicator Panels**~~ - ✅ Implemented: `indicatorPanel` references work in dialogs
8. ~~**Table xyLabel**~~ - ✅ Implemented: `xyLabel` attribute supported (iniSpec 3.49+)
9. ~~**Advanced Math Check**~~ - ✅ Implemented: Returns true (all math available)
10. ~~**MSQ Page Data**~~ - ✅ Implemented: Raw page data parsing from `<pageData>` elements

## 5. Recommendations

1. **Fix parser syntax error immediately** - This could cause protocol issues
2. **Improve MSQ page handling** - Track which page each constant belongs to
3. **Add comprehensive expression function tests** - Ensure all spec functions work
4. **Document expression limitations** - If some functions aren't implemented, document it
5. **Add MSQ validation** - Verify MSQ files match INI structure before loading

## 6. Testing Recommendations

1. Test with various INI files (Speeduino, rusEFI, MS) to ensure compatibility
2. Test MSQ round-trip (load → modify → save → load) to verify data integrity
3. Test expression evaluation with all operators and functions
4. Test menu visibility with various condition combinations
5. Test protocol communication with real ECUs or simulators

