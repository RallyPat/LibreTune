# Nissan Micra K12 (QG18) Trigger Setup Support

## Issue Summary
Request to add support for the Nissan Micra K12 trigger pattern in LibreTune for use with Speeduino ECU.

## Background
The Nissan Micra K12 uses the **QG18DE** engine, which has a specific crankshaft and camshaft trigger pattern that needs to be recognized by the ECU for proper ignition and fuel timing.

### Engine Details
- **Engine**: QG18DE (1.8L 4-cylinder)
- **Crank Pattern**: 4 teeth per revolution on crank sensor
- **Cam Pattern**: 1 tooth per revolution on cam sensor
- **Reference**: [MaxxECU QG18 Documentation](https://www.maxxecu.com/webhelp/trigger_system-nissan_qg18.html)

## How Trigger Patterns Work

### Architecture Overview
```
ECU Firmware (Speeduino/rusEFI)
    ↓ (implements trigger decoding)
INI Definition File
    ↓ (defines available options)
Tuning Software (LibreTune/TunerStudio)
    ↓ (presents UI for selection)
User selects trigger pattern
    ↓ (sends configuration to ECU)
ECU uses pattern for timing
```

### Key Points
1. **Trigger pattern decoding happens in the ECU firmware**, not in the tuning software
2. **LibreTune reads the available patterns from the INI file**
3. **The INI file is generated from the ECU firmware source code**
4. **LibreTune does NOT decode or process trigger patterns** - it only displays what's available

## Current Status

### What's Already Available
The demo INI file (rusEFI/epicEFI) includes these Nissan trigger patterns:
- Nissan Primera (trigger index 24)
- Nissan VQ35 (trigger index 58)
- Nissan VQ30 (trigger index 60)
- Nissan QR25 (trigger index 61)
- Nissan MR18 Crank (trigger index 67)
- Nissan HR (trigger index 79)

### What's Missing
The **Nissan QG18** pattern is NOT currently in the list. This pattern needs to be:
1. Implemented in the Speeduino firmware
2. Added to the Speeduino INI file
3. Then it will automatically appear in LibreTune

## What Needs to Happen

### Step 1: Speeduino Firmware Support (REQUIRED FIRST)
The Speeduino firmware must add support for the QG18 trigger pattern. This involves:

1. **Research the QG18 trigger pattern**:
   - 4 teeth on crankshaft per revolution
   - 1 tooth on camshaft per revolution
   - Timing relationships between crank and cam
   - Reference: MaxxECU documentation and Nissan service manuals

2. **Implement trigger decoding in Speeduino**:
   - File: `speeduino/speeduino/decoders.ino`
   - Add a new trigger decoder function (e.g., `triggerSetup_QG18()`)
   - Implement tooth timing logic
   - Test with real engine or simulator

3. **Add to trigger type enum**:
   - File: `speeduino/speeduino/globals.h`
   - Add new enum value (e.g., `NISSAN_QG18 = XX`)

4. **Update the Speeduino INI template**:
   - File: `speeduino/reference/speeduino.ini`
   - Add "Nissan QG18" to the trigger_type bits definition

5. **Test thoroughly**:
   - Use trigger simulator
   - Test on real engine
   - Verify sync at various RPMs

**Resources**:
- Speeduino Forum: https://speeduino.com/forum/
- Speeduino GitHub: https://github.com/noisymime/speeduino
- Decoder Documentation: https://wiki.speeduino.com/en/decoders

### Step 2: Get Updated INI File
Once Speeduino firmware supports QG18:
1. Download the latest Speeduino INI file
2. It will include the new trigger pattern

### Step 3: Use in LibreTune (AUTOMATIC)
No LibreTune changes needed! Once you have the updated INI:
1. Import the new INI file into LibreTune
2. The QG18 option will appear in the trigger setup dropdown
3. Select it and configure your ECU

## For LibreTune Users

### If You Need QG18 Support Now
1. **Check Speeduino firmware**: See if your firmware version already supports QG18
2. **Update firmware if available**: Flash the latest Speeduino firmware
3. **Get matching INI**: Download the INI that matches your firmware version
4. **Import to LibreTune**: File → Import ECU Definition → Select new INI

### If Speeduino Doesn't Support QG18 Yet
1. **Request support from Speeduino project**:
   - Post on Speeduino forums: https://speeduino.com/forum/
   - Open GitHub issue: https://github.com/noisymime/speeduino/issues
   - Provide MaxxECU documentation link
   - Share scope captures if available

2. **Alternative approaches while waiting**:
   - Use "custom toothed wheel" option with manual configuration
   - Use a similar pattern (e.g., Honda K 4+1) if timing is close
   - Use a trigger converter/adapter

## Technical Details for Developers

### Where Trigger Patterns Are Defined

#### In Speeduino Firmware
```cpp
// speeduino/globals.h
enum TriggerType {
    CUSTOM = 0,
    FORD_ASPIRE = 1,
    // ... many others ...
    NISSAN_QR25 = 61,
    // QG18 would go here
    NISSAN_HR = 79,
    // ...
};

// speeduino/decoders.ino
void triggerSetup_NISSAN_QR25() {
    // Decoder implementation
    configPage4.triggerTeeth = 4;
    configPage4.trigPatternSec = 1;
    // ... timing logic ...
}
```

#### In INI File
```ini
; speeduino.ini
trigger_type = bits, U32, 564, [0:6], 
    "custom toothed wheel", 
    "Ford Aspire", 
    ; ... many others ...
    "Nissan QR25",
    ; "Nissan QG18" would be added here
    "Nissan HR",
    ; ...
```

#### In LibreTune
LibreTune automatically parses the INI and displays all options. No code changes needed.

```rust
// crates/libretune-core/src/ini/parser.rs
// Parses bits definitions and extracts all options
// Already handles any trigger pattern in the INI
```

### Why LibreTune Can't Add This Directly

LibreTune is a **tuning interface**, not an ECU firmware. It:
- ✅ Displays configuration options from INI
- ✅ Sends configuration values to ECU
- ✅ Shows real-time data
- ✅ Edits fuel/ignition tables
- ❌ Does NOT decode trigger patterns
- ❌ Does NOT implement engine timing logic
- ❌ Does NOT run on the ECU

The actual trigger pattern decoding must happen in the ECU firmware because:
1. It requires **real-time processing** (microsecond precision)
2. It runs on the **ECU's microcontroller**, not the laptop
3. It's **safety-critical** - errors could damage the engine
4. It's **hardware-specific** to the ECU platform

## Conclusion

**Summary**: Adding QG18 trigger support is a Speeduino firmware task, not a LibreTune task.

**What LibreTune Will Do**: Once Speeduino adds QG18 support and updates their INI file, LibreTune will automatically show the new trigger option without any code changes.

**What You Should Do**: 
1. Engage with the Speeduino community to request QG18 support
2. Provide documentation and scope traces if possible
3. Once available, update your Speeduino firmware
4. Import the new INI into LibreTune
5. Select "Nissan QG18" from the trigger dropdown

**Status**: This issue will remain open as a documentation reference, but the actual implementation must happen in the Speeduino project first.

## References

- MaxxECU QG18 Documentation: https://www.maxxecu.com/webhelp/trigger_system-nissan_qg18.html
- Speeduino Decoders Wiki: https://wiki.speeduino.com/en/decoders
- Speeduino Forums: https://speeduino.com/forum/
- Speeduino GitHub: https://github.com/noisymime/speeduino
- LibreTune INI Format Docs: [docs/reference/ini-format.md](../docs/reference/ini-format.md)
