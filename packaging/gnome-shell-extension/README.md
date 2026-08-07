# QMKonnect GNOME Shell extension

This extension runs **inside** `gnome-shell` and republishes the focused window's WM class and
title onto the session D-Bus (`io.mulletware.QMKonnect`), where the **QMKonnect app** (the
daemon, installed separately) reads it to drive layer/keymap switching. GNOME (Mutter) cannot
report the active window to client processes, so this small extension is the only reliable
bridge on GNOME/Wayland.

You need **both** this extension **and** the QMKonnect daemon installed for window-driven
switching to work.

## Install (recommended): extensions.gnome.org

Search for **QMKonnect** on <https://extensions.gnome.org> and toggle it on. (Until the listing
is published, install from the Release `.zip` below.)

## Install from a Release `.zip`

```sh
gnome-extensions install qmkonnect@mulletware.shell-extension.zip
gnome-extensions enable qmkonnect@mulletware
```

Then reload GNOME Shell so the extension loads:

- **X11:** press <kbd>Alt</kbd>+<kbd>F2</kbd>, type `r`, press <kbd>Enter</kbd>.
- **Wayland:** log out and back in (Wayland has no live shell restart).

## Install from source

```sh
git clone https://github.com/dabstractor/qmkonnect.git
cd qmkonnect/packaging/gnome-shell-extension
gnome-extensions pack
gnome-extensions install qmkonnect@mulletware.shell-extension.zip
gnome-extensions enable qmkonnect@mulletware
```

Reload GNOME Shell as described above.

## Compatibility

Tested with **GNOME Shell 45, 46, 47, 48, 49, and 50** (matching the `shell-version` in
`metadata.json`). GNOME 45 is the first ES-modules release and is the hard minimum floor.

## Troubleshooting

- **"Extension did not disable" / no events appear:** re-enable the extension from the
  Extensions app. On Wayland you **must** log out and back in — there is no live shell restart.
- **Verify it's running:** the well-known name should be owned and the object exported:
  ```sh
  gdbus introspect --session \
    --dest io.mulletware.QMKonnect \
    --object-path /io/mulletware.QMKonnect
  ```
  You should see the `io.mulletware.QMKonnect.WindowMonitor` interface with `GetActiveWindow`,
  `ActiveWindowChanged`, `AppClass`, and `Title`.
- **The QMKonnect daemon must also be installed** (separate package) and running. Its first-run
  GNOME notification points you to this extension; install + enable it, reload the shell, and
  the daemon's GNOME backend activates automatically.