#!/bin/bash
# LibreTune AppImage post-build patcher
# Injects custom AppRun wrapper into AppImage .AppDir before final packaging
# This ensures Wayland/EGL compatibility fixes are included in released AppImages

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Path to the AppImage build directory (created by Tauri bundler)
APPIMAGE_DIR="${1:-}"
APPRUN_TEMPLATE="$SCRIPT_DIR/AppRun.in"
BUILD_TARGET="${2:-x86_64-unknown-linux-gnu}"

if [ -z "$APPIMAGE_DIR" ]; then
    echo "Usage: $0 <appimage-dir> [build-target]"
    echo ""
    echo "This script patches the AppImage .AppDir with a custom AppRun wrapper."
    echo "Typically called as a post-build hook after Tauri's bundler completes."
    echo ""
    echo "Arguments:"
    echo "  <appimage-dir>  - Path to Tauri's .AppDir (e.g., target/x86_64-unknown-linux-gnu/release/bundle/appimage/libretune-app.AppDir)"
    echo "  [build-target]  - Rust build target (default: x86_64-unknown-linux-gnu)"
    exit 1
fi

if [ ! -d "$APPIMAGE_DIR" ]; then
    echo "Error: AppImage directory not found: $APPIMAGE_DIR"
    exit 1
fi

if [ ! -f "$APPRUN_TEMPLATE" ]; then
    echo "Error: AppRun template not found: $APPRUN_TEMPLATE"
    exit 1
fi

# Find the binary path from the AppDir structure
# Typical structure: AppDir/usr/bin/<app-binary>
BINARY_NAME="libretune-app"
BINARY_PATH="$APPIMAGE_DIR/usr/bin/$BINARY_NAME"

if [ ! -f "$BINARY_PATH" ]; then
    # Try alternative location
    BINARY_PATH=$(find "$APPIMAGE_DIR/usr/bin" -type f -executable | head -1)
    if [ -z "$BINARY_PATH" ] || [ ! -f "$BINARY_PATH" ]; then
        echo "Error: Could not find executable in $APPIMAGE_DIR/usr/bin/"
        echo "Available files:"
        ls -la "$APPIMAGE_DIR/usr/bin/" || true
        exit 1
    fi
fi

BINARY_NAME=$(basename "$BINARY_PATH")

# Replace the @EXEC@ placeholder with the actual binary path (relative to AppDir)
# The AppRun script runs from AppDir, so we use usr/bin/binary-name
APPRUN_CONTENT=$(cat "$APPRUN_TEMPLATE" | sed "s|@EXEC@|usr/bin/$BINARY_NAME|g")

# Write the custom AppRun to the AppDir
APPRUN_PATH="$APPIMAGE_DIR/AppRun"
echo "$APPRUN_CONTENT" > "$APPRUN_PATH"
chmod +x "$APPRUN_PATH"

echo "✓ Custom AppRun injected into $APPIMAGE_DIR"
echo "  - Wayland detection: enabled"
echo "  - Graphics library cleanup: enabled"
echo "  - WebKit library path fix: enabled"
echo "  - ICU library path: configured"
echo ""
echo "The AppImage will now handle Wayland/EGL conflicts at runtime."
