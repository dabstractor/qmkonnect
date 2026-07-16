#!/bin/bash
set -e

cargo build --release

# Create app bundle with human-readable name
rm -rf "QMKonnect.app"

mkdir -p "QMKonnect.app/Contents/MacOS"
cp ../../target/release/qmkonnect "QMKonnect.app/Contents/MacOS/"

mkdir -p "QMKonnect.app/Contents/Resources"
cp Icon.icns "QMKonnect.app/Contents/Resources/"
cp ../IconTemplate.png "QMKonnect.app/Contents/Resources/" 2>/dev/null || echo "   (IconTemplate.png absent — menu-bar icon will fall back to the generated default)"

# Generate Info.plist
cat << EOF > "QMKonnect.app/Contents/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>qmkonnect</string>
    <key>CFBundleIdentifier</key>
    <string>io.mulletware.qmkonnect</string>
    <key>CFBundleName</key>
    <string>QMKonnect</string>
    <key>CFBundleDisplayName</key>
    <string>QMKonnect</string>
    <key>CFBundleIconFile</key>
    <string>Icon.icns</string>
    <key>LSUIElement</key>
    <true/>
</dict>
</plist>
EOF

# Code sign (handles spaces in app name).
#
# macOS keys privacy permissions (e.g. Screen Recording) to the app's code
# signature. With ad-hoc signing (`-`) the only thing TCC can key on is the
# cdhash, which changes on every rebuild — so System Settings shows the grant
# but the app re-prompts each launch (the classic ad-hoc/TCC mismatch).
#
# For distribution, set CODESIGN_IDENTITY to a stable identity, e.g.:
#   CODESIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" ./build.sh
# This makes the designated requirement identifier/team-based so grants
# persist across releases. Falls back to ad-hoc for local builds.
SIGN_IDENTITY="${CODESIGN_IDENTITY:--}"
codesign --deep --force --sign "$SIGN_IDENTITY" "QMKonnect.app"

echo "✅ App built: packaging/macos/QMKonnect.app"
if [ "$SIGN_IDENTITY" = "-" ]; then
    echo "⚠️  Ad-hoc signed (local build). Screen Recording permission must be"
    echo "    re-granted after every rebuild (cdhash changes). Set CODESIGN_IDENTITY"
    echo "    for a stable, distribution-ready signature."
else
    echo "🔐 Signed with identity: $SIGN_IDENTITY"
fi

# Create a DMG file containing the app bundle
DMG_NAME="QMKonnect.dmg"
VOLNAME="QMKonnect Installer"
TEMP_DIR=$(mktemp -d)

# Create a symbolic link to /Applications
ln -s "/Applications" "$TEMP_DIR/Applications"

# Copy the app bundle to the temporary directory
cp -R "QMKonnect.app" "$TEMP_DIR/"

# Create the DMG file with a compressed, read-only format (UDZO)
hdiutil create -volname "$VOLNAME" -srcfolder "$TEMP_DIR" -ov -format UDZO "$DMG_NAME"

# Clean up the temporary directory
rm -rf "$TEMP_DIR"

echo "✅ DMG built: $DMG_NAME"
