# ECU Console

The ECU Console provides a direct text-based command interface for communicating with rusEFI, FOME, and epicEFI engine control units.

## Overview

The console allows you to send text commands directly to your ECU and see the responses in real time. This is useful for diagnostics, configuration, firmware debugging, and accessing features not exposed through the standard tuning UI.

> **Note**: The ECU Console is only available for rusEFI-based ECUs (rusEFI, FOME, epicEFI). Speeduino and MegaSquirt ECUs do not support a text console interface.

## Opening the Console

1. Connect to a rusEFI, FOME, or epicEFI ECU
2. Go to **Tools → ECU Console**
3. A new tab opens with the console interface

The menu item is only enabled when:
- A project is open
- The ECU is connected
- The ECU type supports console commands (rusEFI/FOME/epicEFI)

## Using the Console

### Sending Commands

1. Type a command in the input field at the bottom
2. Press **Enter** to send
3. The command and response appear in the output log

### Output Colors

The console uses color coding to distinguish different message types:

| Color | Prefix | Meaning |
|-------|--------|---------|
| **Cyan** | `>` | Command you sent |
| **Gray** | `<-` | Response from ECU |
| **Red** | `✗` | Error message |
| **Orange** | `…` | Waiting for response |

### Command History

Navigate through previously-sent commands:
- **Arrow Up**: Previous command
- **Arrow Down**: Next command

### Clearing the Console

Click the **Clear** button in the console header to clear the output history.

## FOME Fast Comms

If your ECU is running **FOME** firmware, LibreTune can use an optimized communication protocol for console commands. This is enabled by default and provides faster command/response times.

- The toggle appears automatically when a FOME ECU is detected
- If the fast path fails for any reason, LibreTune falls back to the standard protocol transparently
- You can disable it in the console header if you experience issues

This setting can also be toggled in **Settings → FOME Fast Comms**.

## Connection States

| State | Behavior |
|-------|----------|
| **Connected** | Commands can be sent and responses received |
| **Disconnected** | Input is disabled; a "Not connected" message is shown |
| **Loading** | A blinking cursor animation indicates the ECU is processing your command |

## Common Use Cases

- **Firmware version**: Query the ECU's firmware build info
- **Sensor diagnostics**: Read raw sensor values for debugging
- **Configuration commands**: Set parameters not available in the GUI
- **Debugging**: Monitor ECU internal state during development

## See Also

- [Connecting to Your ECU](../getting-started/connecting.md) — How to establish a connection
- [Supported ECUs](../reference/supported-ecus.md) — Which ECUs support the console
- [Troubleshooting](../reference/troubleshooting.md) — Resolving connection issues
