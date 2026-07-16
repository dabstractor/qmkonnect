#!/bin/bash
#
# clean.sh — reset macOS to a known-empty state before a fresh QMKonnect install.
#
# Run this BEFORE build.sh + install.sh whenever you've built/launched before.
# It clears the two things that make ad-hoc-signed macOS dev builds confusing:
#
#   1. LaunchServices remembers every QMKonnect.app copy it has ever seen
#      (old /Applications installs, trashed copies, mounted .dmg contents) and
#      will happily hand you a STALE one when you `open` the app. We unregister
#      and delete them.
#   2. Screen-Recording permission is keyed to the app's signature (cdhash),
#      which changes every ad-hoc rebuild, so macOS re-prompts every build even
#      though System Settings still lists it as granted. We reset TCC so you get
#      exactly one clean prompt on the next launch.
#
# This script is macOS-only and safe to re-run. It does NOT remove the
# "Launch at Login" registration (managed in-app via SMAppService) — that entry
# points at /Applications/QMKonnect.app and stays valid across reinstalls.
#
# Usage:  ./clean.sh

set -u

LSR=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister

echo "1/5  stopping any running QMKonnect…"
pkill -f "QMKonnect.app" 2>/dev/null || true
sleep 1

echo "2/5  ejecting any mounted QMKonnect disk images…"
ls /Volumes 2>/dev/null | grep -i qmkonnect | while IFS= read -r v; do
    hdiutil detach "/Volumes/$v" >/dev/null 2>&1 || true
done

echo "3/5  unregistering stale copies from LaunchServices…"
[ -x "$LSR" ] && "$LSR" -u "/Applications/QMKonnect.app" 2>/dev/null || true
[ -x "$LSR" ] && "$LSR" -u "$HOME/.Trash/QMKonnect.app" 2>/dev/null || true

echo "4/5  deleting old app bundles…"
rm -rf "/Applications/QMKonnect.app" "$HOME/.Trash/QMKonnect.app"

echo "5/5  resetting Screen-Recording permission (signature changed)…"
tccutil reset ScreenCapture io.mulletware.qmkonnect >/dev/null 2>&1 || true

echo "✅ clean. Next: ./build.sh && ./install.sh"
