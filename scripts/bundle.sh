#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# Read version from Cargo.toml
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
echo "Version: $VERSION"

# Build release binary
echo "Building release binary..."
cargo build --release

# Detect target dir (handles custom target-dir in .cargo/config.toml)
TARGET_DIR=$(cargo metadata --format-version 1 --no-deps 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin)['target_directory'])" 2>/dev/null || echo "$REPO_ROOT/target")
BINARY="$TARGET_DIR/release/dif"

if [ ! -f "$BINARY" ]; then
    echo "Error: binary not found at $BINARY"
    exit 1
fi

# Create .app bundle structure
APP_DIR="$REPO_ROOT/build/Dif.app"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

# Copy binary
cp "$BINARY" "$APP_DIR/Contents/MacOS/dif"

# Copy and patch Info.plist
sed "s/__VERSION__/$VERSION/g" "$REPO_ROOT/Info.plist" > "$APP_DIR/Contents/Info.plist"

echo "Built: $APP_DIR"

# Optional: install to /Applications
if [ "${1:-}" = "--install" ]; then
    echo "Installing to /Applications..."
    rm -rf "/Applications/Dif.app"
    cp -R "$APP_DIR" "/Applications/Dif.app"
    echo "Installed to /Applications/Dif.app"
fi
