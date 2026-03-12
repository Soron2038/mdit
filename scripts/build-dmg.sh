#!/usr/bin/env bash
# Build a release DMG for mdit with drag-to-/Applications installer layout.
#
# Usage: ./scripts/build-dmg.sh
#
# Output: dist/mdit-<VERSION>.dmg

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$SCRIPT_DIR/.."
APP_NAME="mdit"
VERSION="0.1.0"
DMG_NAME="${APP_NAME}-${VERSION}.dmg"
VOLUME_NAME="mdit"

echo "→ Building release binary…"
cargo build --release --manifest-path "$ROOT/Cargo.toml"

echo "→ Updating app bundle…"
cp "$ROOT/target/release/$APP_NAME" \
   "$ROOT/dist/$APP_NAME.app/Contents/MacOS/$APP_NAME"
cp "$ROOT/ressources/Info.plist" \
   "$ROOT/dist/$APP_NAME.app/Contents/Info.plist"
cp "$ROOT/ressources/mdit-app-icon.icns" \
   "$ROOT/dist/$APP_NAME.app/Contents/Resources/mdit-app-icon.icns"
# Clean up old icon name if present
rm -f "$ROOT/dist/$APP_NAME.app/Contents/Resources/AppIcon.icns"

echo "→ Creating DMG staging area…"
STAGING=$(mktemp -d)
trap 'rm -rf "$STAGING"' EXIT

cp -R "$ROOT/dist/$APP_NAME.app" "$STAGING/"
# Symlink so users can drag the app straight into /Applications
ln -s /Applications "$STAGING/Applications"

echo "→ Building DMG…"
hdiutil create \
    -volname  "$VOLUME_NAME" \
    -srcfolder "$STAGING" \
    -ov \
    -format UDZO \
    "$ROOT/dist/$DMG_NAME"

echo "✓  dist/$DMG_NAME"
