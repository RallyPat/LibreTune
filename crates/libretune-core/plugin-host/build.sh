#!/bin/bash
# Build script for plugin-host (no Gradle required)
# Downloads Gson dependency and compiles to a fat JAR
#
# NOTE: This uses our own open-source stub implementations of the 
# TunerStudio Plugin API interfaces, based on public documentation.
# We do NOT use any proprietary TunerStudio code.

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

# Configuration
GSON_VERSION="2.10.1"
GSON_URL="https://repo1.maven.org/maven2/com/google/code/gson/gson/${GSON_VERSION}/gson-${GSON_VERSION}.jar"
LIB_DIR="lib"
BUILD_DIR="build/classes"
JAR_DIR="build/libs"
JAR_NAME="plugin-host.jar"

echo "=== LibreTune Plugin Host Builder ==="

# Create directories
mkdir -p "$LIB_DIR" "$BUILD_DIR" "$JAR_DIR"

# Download Gson if not present
if [ ! -f "$LIB_DIR/gson-${GSON_VERSION}.jar" ]; then
    echo "Downloading Gson ${GSON_VERSION}..."
    curl -sL "$GSON_URL" -o "$LIB_DIR/gson-${GSON_VERSION}.jar"
fi

# Find all Java source files (includes our API stubs)
SOURCES=$(find src -name "*.java")

echo "Compiling Java sources..."
javac -d "$BUILD_DIR" -cp "$LIB_DIR/*" $SOURCES

# Extract Gson classes into build dir for fat JAR
echo "Creating fat JAR..."
cd "$BUILD_DIR"
jar xf "../../$LIB_DIR/gson-${GSON_VERSION}.jar"
# Remove dependency manifests
rm -rf META-INF

cd "$SCRIPT_DIR"

# Create manifest
mkdir -p "$BUILD_DIR/META-INF"
cat > "$BUILD_DIR/META-INF/MANIFEST.MF" << EOF
Manifest-Version: 1.0
Main-Class: io.libretune.pluginhost.Main
EOF

# Create the JAR
jar cfm "$JAR_DIR/$JAR_NAME" "$BUILD_DIR/META-INF/MANIFEST.MF" -C "$BUILD_DIR" .

echo "Build complete: $JAR_DIR/$JAR_NAME"
echo "Size: $(du -h "$JAR_DIR/$JAR_NAME" | cut -f1)"
