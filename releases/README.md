# LibreTune Releases

This directory contains built release artifacts for LibreTune.

## Structure

```
releases/
├── macos/        # macOS builds (.app bundles, .dmg installers)
├── windows/      # Windows builds (.exe installers, .msi packages)
└── linux/        # Linux builds (.AppImage, .deb packages)
```

## Building Releases

### Local Build (Current Platform)

To build for your current platform:

```bash
./scripts/build-release.sh
```

To clean build artifacts before building:

```bash
./scripts/build-release.sh --clean
```

### Automated Multi-Platform Builds

For building on all platforms (macOS, Windows, Linux), use GitHub Actions:

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

