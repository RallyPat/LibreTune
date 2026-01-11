# LibreTune Releases

This directory contains built release artifacts for LibreTune.

## Structure

```
releases/
├── linux/                              # Linux builds
│   ├── libretune-app_x.x.x_amd64.AppImage
│   └── libretune-app_x.x.x_amd64.deb
├── macos/                              # macOS builds
│   ├── libretune-app.app/              # Universal binary (Intel + Apple Silicon)
│   └── libretune-app_x.x.x.dmg
├── windows/                            # Windows builds
│   ├── libretune-app_x.x.x_x64-setup.exe
│   └── libretune-app_x.x.x_x64.msi
└── BUILD_INFO.txt                      # Build metadata
```

## Building Releases

### Local Build (Current Platform)

Build for your current platform using the build script:

```bash
# From project root
./scripts/build-release.sh

# Or from libretune-app directory with npm
cd crates/libretune-app
npm run release
```

To clean the releases directory before building:

```bash
./scripts/build-release.sh --clean
# or
npm run release:clean
```

### Platform-Specific Behavior

| Platform | Targets Built | Notes |
|----------|--------------|-------|
| **Linux** | AppImage, .deb | Native x86_64 build |
| **macOS** | .app (universal), .dmg | Builds both x86_64 and ARM64, merges with `lipo` |
| **Windows** | .exe (NSIS), .msi | Native x64 build |

### Prerequisites

**Linux (Ubuntu/Debian):**
```bash
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

**macOS:**
```bash
xcode-select --install
```

**Windows:**
- Visual Studio Build Tools with "Desktop development with C++" workload
- Or full Visual Studio 2019/2022 with C++ components

### Automated Multi-Platform Builds (CI)

For building on all platforms simultaneously, use GitHub Actions:

1. Push a tag (e.g., `v0.1.0`):
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

2. Or manually trigger the workflow:
   - Go to GitHub Actions → "Build Release" workflow
   - Click "Run workflow"
   - Enter version tag (e.g., `v0.1.0`)
   - Artifacts will be available for download after builds complete

## Artifact Types

### macOS
- **.app bundle**: Application bundle (can be run directly or copied to Applications)
- **.dmg**: Disk image installer (standard macOS distribution format)

### Windows
- **.exe**: NSIS installer (executable installer)
- **.msi**: Windows Installer package

### Linux
- **.AppImage**: Portable application (runs on most Linux distributions)
- **.deb**: Debian package (for Debian/Ubuntu-based distributions)

## Downloading Build Artifacts

### From GitHub Actions (Automated Builds)

After the workflow completes:

1. Go to: https://github.com/brngates98/LibreTune/actions
2. Click on the "Build Release" workflow run
3. Scroll down to "Artifacts" section
4. Download:
   - `all-platforms-release` - Contains all platforms in one zip
   - Or individual platform artifacts: `macos-release`, `windows-release`, `linux-release`

### Local Builds

Build artifacts are stored in:
- `releases/macos/` - macOS builds
- `releases/windows/` - Windows builds  
- `releases/linux/` - Linux builds

## Notes

- Release artifacts are **not** committed to git (see `.gitignore`)
- Each platform directory contains a `BUILD_INFO.txt` with build metadata
- For distribution, you may want to:
  - Code sign the applications
  - Create checksums (SHA256) for verification
  - Upload to a release hosting service (GitHub Releases, etc.)

## Current Build Status

The latest build was triggered by tag: **v0.1.0**

Check build status: https://github.com/brngates98/LibreTune/actions

