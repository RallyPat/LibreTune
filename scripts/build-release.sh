#!/usr/bin/env bash
#
# LibreTune Release Build Script
#
# Builds native release binaries for the current platform and outputs to releases/
# 
# Usage:
#   ./scripts/build-release.sh           # Build for current platform
#   ./scripts/build-release.sh --clean   # Clean releases/ first, then build
#   ./scripts/build-release.sh --help    # Show usage
#
# Outputs:
#   releases/linux/    - AppImage, .deb
#   releases/macos/    - Universal .app, .dmg
#   releases/windows/  - .exe (NSIS), .msi
#   releases/BUILD_INFO.txt
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Get script and project directories
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
APP_DIR="$PROJECT_ROOT/crates/libretune-app"
TAURI_DIR="$APP_DIR/src-tauri"
RELEASES_DIR="$PROJECT_ROOT/releases"

# Parse version from tauri.conf.json
VERSION=$(grep -o '"version": "[^"]*"' "$TAURI_DIR/tauri.conf.json" | head -1 | cut -d'"' -f4)

echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}  LibreTune Release Builder v${VERSION}${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

# Show help
if [[ "$1" == "--help" || "$1" == "-h" ]]; then
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --clean    Clean releases/ directory before building"
    echo "  --help     Show this help message"
    echo ""
    echo "Builds native release binaries for the current platform."
    echo "Output is placed in releases/<platform>/"
    echo ""
    exit 0
fi

# Detect host OS
detect_os() {
    case "$(uname -s)" in
        Linux*)
            # Check for WSL
            if grep -qi microsoft /proc/version 2>/dev/null; then
                echo "wsl"
            else
                echo "linux"
            fi
            ;;
        Darwin*)
            echo "macos"
            ;;
        CYGWIN*|MINGW*|MSYS*)
            echo "windows"
            ;;
        *)
            echo "unknown"
            ;;
    esac
}

HOST_OS=$(detect_os)
echo -e "${GREEN}▶ Detected platform:${NC} $HOST_OS"

if [[ "$HOST_OS" == "unknown" ]]; then
    echo -e "${RED}Error: Unknown operating system. Cannot build.${NC}"
    exit 1
fi

# Clean if requested
if [[ "$1" == "--clean" ]]; then
    echo -e "${YELLOW}▶ Cleaning releases directory...${NC}"
    rm -rf "$RELEASES_DIR/linux" "$RELEASES_DIR/macos" "$RELEASES_DIR/windows"
    rm -f "$RELEASES_DIR/BUILD_INFO.txt"
    echo -e "${GREEN}  ✓ Cleaned${NC}"
fi

# Create output directories
mkdir -p "$RELEASES_DIR/linux"
mkdir -p "$RELEASES_DIR/macos"
mkdir -p "$RELEASES_DIR/windows"

# Change to app directory
cd "$APP_DIR"

# Ensure dependencies are installed
echo -e "${GREEN}▶ Installing npm dependencies...${NC}"
npm install --silent

# Build frontend
echo -e "${GREEN}▶ Building frontend...${NC}"
npm run build

# Build function for Tauri
build_tauri() {
    local target="$1"
    local label="$2"
    
    echo -e "${GREEN}▶ Building Tauri app${label:+ for $label}...${NC}"
    
    # Temporarily disable exit-on-error for build (bundling may partially fail)
    set +e
    if [[ -n "$target" ]]; then
        npx tauri build --target "$target"
        local result=$?
    else
        npx tauri build
        local result=$?
    fi
    set -e
    
    return $result
}

# Copy artifacts to releases folder
copy_artifacts() {
    local platform="$1"
    local target_dir="$2"
    local bundle_dir="$PROJECT_ROOT/target/${target_dir}/release/bundle"
    
    echo -e "${GREEN}▶ Copying $platform artifacts...${NC}"
    
    case "$platform" in
        linux)
            # AppImage
            if ls "$bundle_dir/appimage/"*.AppImage 1>/dev/null 2>&1; then
                cp "$bundle_dir/appimage/"*.AppImage "$RELEASES_DIR/linux/"
                echo -e "${GREEN}  ✓ AppImage${NC}"
            fi
            # Deb
            if ls "$bundle_dir/deb/"*.deb 1>/dev/null 2>&1; then
                cp "$bundle_dir/deb/"*.deb "$RELEASES_DIR/linux/"
                echo -e "${GREEN}  ✓ .deb${NC}"
            fi
            # RPM (optional)
            if ls "$bundle_dir/rpm/"*.rpm 1>/dev/null 2>&1; then
                cp "$bundle_dir/rpm/"*.rpm "$RELEASES_DIR/linux/"
                echo -e "${GREEN}  ✓ .rpm${NC}"
            fi
            ;;
        macos)
            # .app bundle
            if [[ -d "$bundle_dir/macos/"*.app ]]; then
                cp -R "$bundle_dir/macos/"*.app "$RELEASES_DIR/macos/"
                echo -e "${GREEN}  ✓ .app bundle${NC}"
            fi
            # DMG
            if ls "$bundle_dir/dmg/"*.dmg 1>/dev/null 2>&1; then
                cp "$bundle_dir/dmg/"*.dmg "$RELEASES_DIR/macos/"
                echo -e "${GREEN}  ✓ .dmg${NC}"
            fi
            ;;
        windows)
            # NSIS installer
            if ls "$bundle_dir/nsis/"*.exe 1>/dev/null 2>&1; then
                cp "$bundle_dir/nsis/"*.exe "$RELEASES_DIR/windows/"
                echo -e "${GREEN}  ✓ .exe (NSIS)${NC}"
            fi
            # MSI
            if ls "$bundle_dir/msi/"*.msi 1>/dev/null 2>&1; then
                cp "$bundle_dir/msi/"*.msi "$RELEASES_DIR/windows/"
                echo -e "${GREEN}  ✓ .msi${NC}"
            fi
            ;;
    esac
}

# Generate BUILD_INFO.txt
generate_build_info() {
    local info_file="$RELEASES_DIR/BUILD_INFO.txt"
    
    echo "LibreTune Build Information" > "$info_file"
    echo "============================" >> "$info_file"
    echo "" >> "$info_file"
    echo "Version:     $VERSION" >> "$info_file"
    echo "Build Date:  $(date -u '+%Y-%m-%d %H:%M:%S UTC')" >> "$info_file"
    echo "Platform:    $HOST_OS" >> "$info_file"
    
    # Git info if available
    if command -v git &>/dev/null && git rev-parse --git-dir &>/dev/null 2>&1; then
        echo "Git Commit:  $(git rev-parse --short HEAD 2>/dev/null || echo 'N/A')" >> "$info_file"
        echo "Git Branch:  $(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo 'N/A')" >> "$info_file"
        if ! git diff-index --quiet HEAD -- 2>/dev/null; then
            echo "Git Status:  Modified (uncommitted changes)" >> "$info_file"
        else
            echo "Git Status:  Clean" >> "$info_file"
        fi
    fi
    
    echo "" >> "$info_file"
    echo "Rust Version: $(rustc --version 2>/dev/null || echo 'N/A')" >> "$info_file"
    echo "Node Version: $(node --version 2>/dev/null || echo 'N/A')" >> "$info_file"
    echo "Tauri CLI:    $(npx tauri --version 2>/dev/null || echo 'N/A')" >> "$info_file"
    
    echo -e "${GREEN}▶ Generated BUILD_INFO.txt${NC}"
}

# Platform-specific builds
case "$HOST_OS" in
    linux|wsl)
        echo -e "${BLUE}━━━ Building for Linux ━━━${NC}"
        
        # Try full build with AppImage + deb - may fail if linuxdeploy has issues
        if ! build_tauri "" "Linux x86_64"; then
            echo -e "${YELLOW}▶ Full bundle build failed, trying deb only...${NC}"
            # Fall back to deb-only build
            cd "$TAURI_DIR"
            npx tauri build --bundles deb || true
            cd "$APP_DIR"
        fi
        
        copy_artifacts "linux" ""
        
        # Also copy the raw binary as fallback
        RAW_BINARY="$PROJECT_ROOT/target/release/libretune-app"
        if [[ -f "$RAW_BINARY" ]]; then
            cp "$RAW_BINARY" "$RELEASES_DIR/linux/"
            chmod +x "$RELEASES_DIR/linux/libretune-app"
            echo -e "${GREEN}  ✓ Raw binary (libretune-app)${NC}"
        fi
        
        # If AppImage failed but AppDir exists, try manual squashfs
        APPDIR="$PROJECT_ROOT/target/release/bundle/appimage/libretune-app.AppDir"
        APPIMAGE_OUT="$RELEASES_DIR/linux/libretune-app_${VERSION}_amd64.AppImage"
        if [[ -d "$APPDIR" ]] && ! ls "$RELEASES_DIR/linux/"*.AppImage 1>/dev/null 2>&1; then
            echo -e "${YELLOW}▶ AppImage bundle failed, attempting manual creation...${NC}"
            if command -v appimagetool &>/dev/null; then
                appimagetool "$APPDIR" "$APPIMAGE_OUT" && echo -e "${GREEN}  ✓ AppImage (manual)${NC}"
            elif command -v mksquashfs &>/dev/null; then
                echo -e "${YELLOW}  appimagetool not found, AppImage not created${NC}"
                echo -e "${YELLOW}  Install: https://github.com/AppImage/AppImageKit/releases${NC}"
            else
                echo -e "${YELLOW}  Note: Install appimagetool to create AppImage${NC}"
            fi
        fi
        ;;
        
    macos)
        echo -e "${BLUE}━━━ Building for macOS (Universal Binary) ━━━${NC}"
        
        # Ensure both targets are installed
        echo -e "${GREEN}▶ Adding Rust targets for universal binary...${NC}"
        rustup target add x86_64-apple-darwin 2>/dev/null || true
        rustup target add aarch64-apple-darwin 2>/dev/null || true
        
        # Build for Intel
        echo -e "${GREEN}▶ Building for x86_64 (Intel)...${NC}"
        build_tauri "x86_64-apple-darwin" "Intel"
        
        # Build for ARM64
        echo -e "${GREEN}▶ Building for aarch64 (Apple Silicon)...${NC}"
        build_tauri "aarch64-apple-darwin" "Apple Silicon"
        
        # Create universal binary using lipo
        echo -e "${GREEN}▶ Creating universal binary with lipo...${NC}"
        
        INTEL_APP="$PROJECT_ROOT/target/x86_64-apple-darwin/release/bundle/macos/libretune-app.app"
        ARM_APP="$PROJECT_ROOT/target/aarch64-apple-darwin/release/bundle/macos/libretune-app.app"
        UNIVERSAL_APP="$RELEASES_DIR/macos/libretune-app.app"
        
        # Copy ARM app as base (or Intel, doesn't matter for structure)
        if [[ -d "$ARM_APP" ]]; then
            cp -R "$ARM_APP" "$UNIVERSAL_APP"
            
            # Find the main executable and merge with lipo
            INTEL_BIN="$INTEL_APP/Contents/MacOS/libretune-app"
            ARM_BIN="$ARM_APP/Contents/MacOS/libretune-app"
            UNIVERSAL_BIN="$UNIVERSAL_APP/Contents/MacOS/libretune-app"
            
            if [[ -f "$INTEL_BIN" && -f "$ARM_BIN" ]]; then
                lipo -create "$INTEL_BIN" "$ARM_BIN" -output "$UNIVERSAL_BIN"
                echo -e "${GREEN}  ✓ Universal binary created${NC}"
                
                # Verify
                echo -e "${GREEN}  Architectures:${NC} $(lipo -archs "$UNIVERSAL_BIN")"
            else
                echo -e "${YELLOW}  Warning: Could not find binaries for lipo, copying ARM app only${NC}"
            fi
        else
            echo -e "${YELLOW}  Warning: ARM build not found, copying Intel app${NC}"
            cp -R "$INTEL_APP" "$UNIVERSAL_APP" 2>/dev/null || true
        fi
        
        # Copy DMG (use ARM version as it will work on both via Rosetta if needed)
        ARM_DMG="$PROJECT_ROOT/target/aarch64-apple-darwin/release/bundle/dmg/"
        if ls "$ARM_DMG"*.dmg 1>/dev/null 2>&1; then
            cp "$ARM_DMG"*.dmg "$RELEASES_DIR/macos/"
            echo -e "${GREEN}  ✓ .dmg (ARM64)${NC}"
        fi
        ;;
        
    windows)
        echo -e "${BLUE}━━━ Building for Windows ━━━${NC}"
        build_tauri "" "Windows x64"
        copy_artifacts "windows" ""
        ;;
esac

# Generate build info
generate_build_info

# Summary
echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}  ✓ Build complete!${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo -e "${GREEN}Artifacts in:${NC} $RELEASES_DIR/"
echo ""

# List created files
echo -e "${GREEN}Created files:${NC}"
find "$RELEASES_DIR" -type f -newer "$RELEASES_DIR/BUILD_INFO.txt" -o -name "BUILD_INFO.txt" 2>/dev/null | \
    grep -v "README.md" | \
    while read -r file; do
        size=$(du -h "$file" 2>/dev/null | cut -f1)
        echo "  $(basename "$file") ($size)"
    done

echo ""
