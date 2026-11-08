# Installation

LibreTune is available for Windows, macOS, and Linux.

## Download

Download the latest release for your operating system from the [GitHub Releases](https://github.com/RallyPat/LibreTune/releases) page.

| Platform | File |
|----------|------|
| Windows | `LibreTune_x.x.x_x64-setup.exe` |
| macOS (Apple Silicon) | `libretune-app_x.x.x_aarch64.dmg` |
| macOS (Intel) | `libretune-app_x.x.x_x64.dmg` |
| Linux (Debian/Ubuntu) | `libretune_x.x.x_amd64.deb` |
| Linux (AppImage) | `LibreTune_x.x.x_amd64.AppImage` |

macOS builds are **not** universal binaries — download the file matching your
Mac's processor. Apple Silicon Macs (M1 and newer) need the `aarch64` DMG;
the `x64` DMG only runs under Rosetta 2 and will fail outright if Rosetta is
not installed.

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

1. Download the `.dmg` file for your processor (see the table above)
2. Open the DMG and drag LibreTune to your Applications folder
3. On first launch, macOS will refuse to open the app because it is not
   notarized by Apple — see below

### First Launch on macOS (Gatekeeper)

LibreTune releases are ad-hoc signed but not signed with an Apple Developer ID
or notarized, so macOS quarantines them after download. Depending on your macOS
version you will see either *"LibreTune cannot be opened because it is from an
unidentified developer"* or *"LibreTune is damaged and can't be opened"*.

The app is not actually damaged. To allow it:

1. Try to open LibreTune once (double-click) and dismiss the warning
2. Open **System Settings → Privacy & Security**
3. Scroll to the Security section and click **Open Anyway** next to the
   LibreTune message
4. Confirm with **Open**

If the **Open Anyway** button does not appear, remove the quarantine flag from
Terminal instead:

```bash
xattr -dr com.apple.quarantine /Applications/LibreTune.app
```

Only run that command on downloads you trust.

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

## Updating

LibreTune checks for a newer release once after startup, without blocking
anything: if one exists you get a short notice, and **Help → About LibreTune**
has an **Install and restart** button (and a manual **Check for updates**).
Nothing is downloaded or installed until you press it.

Updates are cryptographically signed. The public key is built into the app,
and an update whose signature does not verify is refused before it is
installed. This is separate from macOS notarization and Windows publisher
signing, which the project does not have yet, so a first install may still
require **Open Anyway** (macOS) or **More info → Run anyway** (Windows).

Official builds, nightlies included, carry the release public key. A build
you made yourself without it can still *see* a newer release, but the download
is refused at the signature check and nothing is installed.

## Building from Source

For developers who want to build LibreTune from source, see the [Contributing Guide](../contributing.md).

## Next Steps

Once installed, proceed to [Creating Your First Project](./first-project.md) to set up your ECU.
