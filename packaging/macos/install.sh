#!/bin/bash
#
# install.sh — install the just-built QMKonnect into /Applications.
#
# Installs from the release artifact (QMKonnect.dmg produced by build.sh) so you
# exercise the exact disk image your users install from. Run build.sh first.
#
# The app registers itself to launch at login on its first run (in-app
# SMAppService, on by default) — there is nothing extra to configure here.
#
# Usage:  ./install.sh
#
set -uo pipefail

# Resolve the .dmg relative to this script (packaging/macos/QMKonnect.dmg),
# independent of the caller's working directory.
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
DMG="$SCRIPT_DIR/QMKonnect.dmg"

if [ ! -f "$DMG" ]; then
    echo "❌ no .dmg at $DMG — run ./build.sh first." >&2
    exit 1
fi

# Clean out any previous install so LaunchServices can't resurrect it.
rm -rf "/Applications/QMKonnect.app"

MNT=$(mktemp -d)
echo "mounting $DMG …"
if ! hdiutil attach "$DMG" -nobrowse -mountpoint "$MNT" >/dev/null 2>&1; then
    echo "❌ failed to mount $DMG" >&2
    rmdir "$MNT" 2>/dev/null || true
    exit 1
fi

echo "copying QMKonnect.app → /Applications …"
cp -R "$MNT/QMKonnect.app" "/Applications/"
hdiutil detach "$MNT" >/dev/null 2>&1 || true
rmdir "$MNT" 2>/dev/null || true

echo "✅ installed at /Applications/QMKonnect.app"
echo "   launch with:  open /Applications/QMKonnect.app"
echo "   (grant the one Screen-Recording prompt on first launch)"
