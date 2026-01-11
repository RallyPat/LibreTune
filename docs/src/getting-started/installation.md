# Installation

LibreTune is available for Windows, macOS, and Linux.

## Download

Download the latest release for your operating system from the [GitHub Releases](https://github.com/RallyPat/LibreTune/releases) page.

| Platform | File |
|----------|------|
| Windows | `LibreTune_x.x.x_x64-setup.exe` |
| macOS | `LibreTune_x.x.x_universal.dmg` |
| Linux (Debian/Ubuntu) | `libretune_x.x.x_amd64.deb` |
| Linux (AppImage) | `LibreTune_x.x.x_amd64.AppImage` |

## Windows Installation

1. Download the `.exe` installer
2. Run the installer and follow the prompts
3. LibreTune will be added to your Start Menu

### USB Driver Setup (Windows)

For Speeduino and most Arduino-based ECUs, you may need to install USB drivers:

1. Download [CH340 drivers](https://sparks.gogo.co.nz/ch340.html) for Arduino clones
2. Or [FTDI drivers](https://ftdichip.com/drivers/vcp-drivers/) for genuine Arduino boards
3. Connect your ECU and verify it appears in Device Manager as a COM port

## macOS Installation

1. Download the `.dmg` file
2. Open the DMG and drag LibreTune to your Applications folder
3. On first launch, you may need to allow the app in **System Preferences → Security & Privacy**

### USB Permissions (macOS)

macOS should automatically recognize most USB serial adapters. If you have issues:

1. Check **System Preferences → Security & Privacy → Privacy → Files and Folders**
2. Ensure LibreTune has access to removable volumes

## Linux Installation

### Debian/Ubuntu (.deb)

```bash
sudo dpkg -i libretune_x.x.x_amd64.deb
sudo apt-get install -f  # Install dependencies if needed
```

### AppImage

```bash
chmod +x LibreTune_x.x.x_amd64.AppImage
./LibreTune_x.x.x_amd64.AppImage
```

### USB Permissions (Linux)

To access serial ports without root, add your user to the `dialout` group:

```bash
sudo usermod -a -G dialout $USER
```

Log out and back in for the change to take effect.

## Building from Source

For developers who want to build LibreTune from source, see the [Contributing Guide](../contributing.md).

## Next Steps

Once installed, proceed to [Creating Your First Project](./first-project.md) to set up your ECU.
