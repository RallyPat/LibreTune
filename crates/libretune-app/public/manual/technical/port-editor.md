# Port Editor

The **Port Editor** allows you to configure ECU pin assignments and digital output mappings. This document covers the Port Editor UI, common configurations, and troubleshooting.

## Overview

The Port Editor lets you:

- **Assign Functions to Pins**: Map injector, ignition, and auxiliary outputs to ECU pins
- **Organize by Category**: Injectors, Ignition, Auxiliary Outputs, and inputs in separate groups
- **Visual Pin Diagram**: See hardware pinout organized by voltage levels and type
- **Validate Assignments**: Automatic warnings for unassigned required functions
- **Detect Conflicts**: Alerts when multiple functions are assigned to the same pin

## Opening Port Editor

The port editor is accessed through the **Setup** menu → **Programmable Ports**. 

**Note**: The exact menu path depends on your ECU's INI definition. Most ECUs place it under Setup, but some may have it under Tools or Hardware. The `std_port_edit` feature is built-in and will work if your INI file has a `subMenu = std_port_edit, "Label"` entry in the `[Menu]` section.

## User Interface

### Left Panel: Function Assignments

Shows all available functions grouped by type:

- **Injectors**: INJ1-INJ8+ (required = at least INJ1)
- **Ignition**: IGN1-IGN8+ (required = at least IGN1)
- **Auxiliary Outputs**: AUX1-AUX16 (fuel pump, fan, etc.)
- **Inputs**: TPS (throttle), MAP (boost), CLT (coolant temp), IAT (intake), O2 (oxygen), Battery voltage, VR sensors (crank/cam)

Each function shows:
- **Icon**: Color-coded dot indicating function type
- **Name**: Function label (e.g., "Injector 1")
- **Required Badge**: Red badge for functions that must be assigned
- **Dropdown**: Select which physical pin this function uses

### Right Panel: Hardware Pinout

Visual representation of available pins:

- **High-Side Outputs**: HSO1-HSO8 (12V injector drivers)
- **Low-Side Outputs**: LSO1-LSO8 (5V ignition drivers)
- **Analog Inputs**: ADC0-ADC7 (sensor inputs)
- **Digital & VR Inputs**: DIN1-DIN8, VR1±, VR2± (crank/cam sensors)
- **CAN Bus**: CANH, CANL (if supported)

Pin colors indicate:
- **Solid Border**: Already assigned to a function
- **Dashed Border**: Available for assignment
- **Color**: Matches the function type (green=injector, orange=ignition, etc.)

### Search & Warnings

**Search Filter**: Type to find functions quickly (e.g., "fuel" finds "Fuel pump")

**Warning Banner** (if present):
- Shows unassigned required functions in red
- Displays number of conflicts (if any)
- Maximum 3 warnings shown; click to expand

## Common Configurations

### 4-Cylinder Engine (Speeduino)

```
Injectors:
  INJ1 → HSO1
  INJ2 → HSO2
  INJ3 → HSO3
  INJ4 → HSO4

Ignition:
  IGN1 → LSO1
  IGN2 → LSO2
  IGN3 → LSO3
  IGN4 → LSO4

Inputs:
  TPS → ADC0
  MAP → ADC1
  CLT → ADC2
  IAT → ADC3
  O2 → ADC4
  Battery → ADC5
  Crank → VR1+/VR1-
  Cam → VR2+/VR2- (optional)

Auxiliary:
  Fuel Pump → LSO7
  Fan → LSO8
```

### 6-Cylinder Engine (rusEFI)

Similar to 4-cylinder, but with:
- INJ1-INJ6, IGN1-IGN6 assignments
- Same input structure
- May use different pin names depending on board

### Turbocharged Engine

Same as naturally aspirated, but may include:
- **Boost Control**: AUX1 for wastegate solenoid
- **Anti-Lag**: AUX2 for fuel cut solenoid
- Same injector/ignition structure

## Best Practices

1. **Assign all required functions** before saving
   - Required functions have a red "Required" badge
   - System warns if any are unassigned

2. **Use high-side outputs for injectors**
   - HSO (12V) is designed for injector drivers
   - Provides adequate current for fuel injectors

3. **Use low-side outputs for coils**
   - LSO (5V) drives ignition coil primary circuits
   - Matches typical coil driver specifications

4. **Separate input types by ADC channel**
   - Each analog input should use a unique ADC channel
   - Shared channels cause crosstalk and interference

5. **Verify pinout with ECU schematic**
   - Different boards have different physical pin layouts
   - Always reference your ECU's documentation

6. **Test with a multimeter** (power cycle required):
   - After saving configuration, power the ECU
   - Test each output pin for correct voltage/ground
   - Verify inputs read correct sensor values

## Troubleshooting

### "Function X is already assigned" error

**Cause**: Two functions were trying to use the same pin

**Solution**: 
1. Select a different pin for one of the functions
2. Check the visual pin diagram for available pins
3. Resolve all conflicts before saving

### "Required function X is not assigned" warning

**Cause**: A function marked as "Required" was left unassigned

**Solution**:
1. Click the warning to see which functions are missing
2. Assign each required function to an available pin
3. Save will be blocked until all required functions are assigned

### Port editor doesn't open

**Cause**: 
- Menu item "Programmable Ports" is not in your INI's `[Menu]` section
- INI version doesn't include the port editor feature

**Solution**:
- Check that your INI has: `subMenu = std_port_edit, "Programmable Ports"` in the `[Menu]` section
- If not present, you may need to upgrade your ECU firmware or INI definition
- Not all ECUs support port configuration
- Check your ECU documentation for supported features

### Changes not persisting

**Cause**: Did not click **Save Configuration** button

**Solution**:
- Always click **Save Configuration** to persist changes
- Unsaved changes indicator shows "Unsaved changes" in red
- Closing without saving will discard your assignments

## Per-ECU Notes

### Speeduino

- Megasquirt-compatible pinout on most boards
- Carefully verify your specific board layout (v0.3, v0.4, v3.57, etc.)
- Most common: STM32F407 with standard pin arrangement

### rusEFI (STM32 boards)

- Pin names vary by board (Proteus F4, Frankenso, etc.)
- Always reference the specific board's pinout diagram
- Some boards have limited pin count (check your variant)

### EpicEFI

- Similar to rusEFI pinout
- Verify board variant (256C, etc.)
- May support dual-bank fuel injection on some configurations

## See Also

- [ECU Protocol](./ecu-protocol.md) - Pin communication details
- [Troubleshooting](../reference/troubleshooting.md) - General ECU issues
- Your ECU's documentation for specific pinout diagrams
