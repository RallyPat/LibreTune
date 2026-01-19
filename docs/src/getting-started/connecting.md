# Connecting to Your ECU

LibreTune communicates with your ECU over a USB serial connection.

## Prerequisites

Before connecting:

1. Your ECU is powered on
2. USB cable is connected between ECU and computer
3. Appropriate USB drivers are installed (see [Installation](./installation.md))

## Opening the Connection Dialog

1. Click the **Connect** button in the toolbar, or
2. Go to **ECU → Connect**, or
3. Press `Ctrl+Shift+C`

## Selecting a Port

The connection dialog shows available serial ports:

- **Windows**: `COM3`, `COM4`, etc.
- **macOS**: `/dev/cu.usbserial-*`, `/dev/cu.usbmodem*`
- **Linux**: `/dev/ttyUSB0`, `/dev/ttyACM0`

If you don't see your ECU's port:
1. Check the USB cable connection
2. Verify the ECU is powered
3. Check USB drivers are installed
4. Click **Refresh** to rescan ports

## Connection Settings

| Setting | Description | Default |
|---------|-------------|---------|
| **Port** | Serial port for ECU | Auto-detected |
| **Baud Rate** | Communication speed | 115200 |
| **Timeout** | Connection timeout (ms) | 5000 |

### Auto-sync & Reconnect After Controller Commands

Some controller commands (for example, commands that apply base maps or change ECU configuration) modify settings directly on the ECU. These changes may not be visible to LibreTune until the app performs a fresh sync and, in some cases, reconnects the serial port.

You can enable the **Auto-sync & reconnect after controller commands** option in the Settings dialog (look for **Auto-sync & reconnect after controller commands**). When enabled, LibreTune will automatically perform a sync after a controller command completes and will reconnect if necessary so that newly applied ECU settings are reflected in the app.

> ⚠️ This option may cause the serial port to be temporarily re-opened. Enable it only if you expect to run controller commands that alter ECU configuration and you want those changes picked up automatically.


### Common Baud Rates

| ECU | Baud Rate |
|-----|-----------|
| Speeduino | 115200 |
| rusEFI | 115200 |
| epicEFI | 115200 |
| MegaSquirt MS2 | 115200 |
| MegaSquirt MS3 | 115200 |

## Connecting

1. Select the correct port
2. Verify baud rate matches your ECU
3. Click **Connect**

LibreTune will:
1. Open the serial connection
2. Query the ECU signature
3. Verify it matches your INI definition
4. Synchronize tune data

## Connection Status

The status bar shows your connection state:

| Status | Meaning |
|--------|---------|
| 🔴 Disconnected | No active connection |
| 🟡 Connecting | Establishing connection |
| 🟢 Connected | Successfully connected |
| ⚠️ Partial Sync | Connected but some pages failed to sync |

## Signature Mismatch

If the ECU signature doesn't match your INI file:

1. A dialog will appear with options:
   - **Search local repository** for matching INI files
   - **Search online** for INI files (Speeduino/rusEFI GitHub repos)
   - **Continue anyway** (advanced users only)

2. Select a matching INI file to update your project

## Troubleshooting

### "Port not found"
- Check USB cable connection
- Verify ECU power
- Install/reinstall USB drivers
- Try a different USB port

### "Connection timeout"
- Verify correct baud rate
- Check ECU is responsive (LEDs blinking)
- Try power cycling the ECU

### "Signature mismatch"
- Your INI file doesn't match the ECU firmware
- Download the correct INI for your firmware version
- Or update your ECU firmware to match the INI

### "Permission denied" (Linux)
- Add your user to the `dialout` group:
  ```bash
  sudo usermod -a -G dialout $USER
  ```
- Log out and back in

## Next Steps

Once connected, you can:
- View the [Dashboard](../features/dashboards.md) for real-time data
- Edit [Tables](../features/table-editing.md)
- Start [AutoTune](../features/autotune.md)
