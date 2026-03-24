#!/usr/bin/env bash
set -euo pipefail

# ── Configuration ────────────────────────────────────────────────────
# Set these, or export them before running this script.
DEVELOPER_ID="${DEVELOPER_ID:?Set DEVELOPER_ID to your signing identity, e.g. 'Developer ID Application: Your Name (TEAMID)'}"
APPLE_ID="${APPLE_ID:?Set APPLE_ID to your Apple ID email}"
TEAM_ID="${TEAM_ID:?Set TEAM_ID to your 10-character team ID}"
APP_PASSWORD="${APP_PASSWORD:?Set APP_PASSWORD to an app-specific password from appleid.apple.com}"

APP_NAME="Zwift Power Overlay"
BUNDLE_DIR="target/release/bundle/osx"
APP_PATH="$BUNDLE_DIR/$APP_NAME.app"
ZIP_PATH="$BUNDLE_DIR/$APP_NAME.zip"

# ── 1. Build release binary ─────────────────────────────────────────
echo "==> Building release binary..."
cargo build --release

# ── 2. Create .app bundle ───────────────────────────────────────────
echo "==> Creating .app bundle..."
cargo bundle --release

# ── 3. Patch Info.plist with Bluetooth permission ────────────────────
echo "==> Patching Info.plist with Bluetooth usage description..."
/usr/libexec/PlistBuddy -c \
  "Add :NSBluetoothAlwaysUsageDescription string 'Zwift Power Overlay needs Bluetooth to connect to your power meter.'" \
  "$APP_PATH/Contents/Info.plist" 2>/dev/null \
|| /usr/libexec/PlistBuddy -c \
  "Set :NSBluetoothAlwaysUsageDescription 'Zwift Power Overlay needs Bluetooth to connect to your power meter.'" \
  "$APP_PATH/Contents/Info.plist"

# ── 4. Codesign with hardened runtime ────────────────────────────────
# Sign the inner binary first, then the bundle (never use --deep)
echo "==> Codesigning binary..."
codesign --force --sign "$DEVELOPER_ID" \
  --options runtime \
  --entitlements entitlements.plist \
  --timestamp \
  "$APP_PATH/Contents/MacOS/zwift_overlay"

echo "==> Codesigning bundle..."
codesign --force --sign "$DEVELOPER_ID" \
  --options runtime \
  --entitlements entitlements.plist \
  --timestamp \
  "$APP_PATH"

echo "==> Verifying signature..."
codesign --verify --verbose=2 "$APP_PATH"

# ── 5. Create zip for notarization ──────────────────────────────────
echo "==> Creating zip for notarization..."
ditto -c -k --keepParent "$APP_PATH" "$ZIP_PATH"

# ── 6. Submit for notarization ──────────────────────────────────────
echo "==> Submitting for notarization (this may take a few minutes)..."
xcrun notarytool submit "$ZIP_PATH" \
  --apple-id "$APPLE_ID" \
  --team-id "$TEAM_ID" \
  --password "$APP_PASSWORD" \
  --wait

# ── 7. Staple the ticket ────────────────────────────────────────────
echo "==> Stapling notarization ticket..."
xcrun stapler staple "$APP_PATH"

# ── 8. Re-zip the stapled app for distribution ──────────────────────
echo "==> Creating final distributable zip..."
rm -f "$ZIP_PATH"
ditto -c -k --keepParent "$APP_PATH" "$ZIP_PATH"

echo ""
echo "==> Done! Distributable at:"
echo "    $ZIP_PATH"
