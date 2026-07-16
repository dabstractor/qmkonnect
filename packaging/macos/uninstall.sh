#!/bin/bash
#
# uninstall.sh — fully remove QMKonnect from macOS.
#
# Removes: the running app, the "Launch at Login" registration (SMAppService),
# the app bundle, its per-user config/state, and the old /tmp logs.
#
# Usage:  ./uninstall.sh
#
set -u

echo "1/4  stopping QMKonnect…"
pkill -f "QMKonnect.app" 2>/dev/null || true
sleep 1

echo "2/4  removing the “Launch at Login” entry (SMAppService)…"
# Best-effort: System Events exposes modern login items too, so this clears the
# registration. It may prompt once for Automation permission and is a no-op if
# the app was never launched (and thus never self-registered).
osascript -e 'tell application "System Events" to delete login item "QMKonnect"' 2>/dev/null || true

echo "3/4  deleting the app bundle…"
sudo rm -rf "/Applications/QMKonnect.app"

echo "4/4  removing per-user config/state and old logs…"
rm -rf "$HOME/Library/Application Support/QMKonnect"
rm -f /tmp/qmkonnect.{out,err}.log

echo "✅ QMKonnect completely uninstalled"
echo "   (Screen-Recording permission lingers harmlessly; reset it with:"
echo "    tccutil reset ScreenCapture io.mulletware.qmkonnect)"
