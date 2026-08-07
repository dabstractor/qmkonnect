# asdf-qmkonnect

[asdf](https://asdf-vm.com/) (and [mise](https://mise.jdx.dev/), which is asdf-compatible) plugin for
[**QMKonnect**](https://github.com/dabstractor/qmkonnect) — the cross-platform window-activity notifier
for QMK keyboards. Installs the pre-built release binaries from
[GitHub Releases](https://github.com/dabstractor/qmkonnect/releases).

> **The same plugin serves both managers.** mise runs an asdf plugin's `bin/*` scripts unchanged, so you
> do not need a separate mise backend.

## Setup

### asdf

```bash
asdf plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect
asdf install qmkonnect latest        # or a specific version, e.g. 0.2.8
asdf global qmkonnect latest         # set the default; or `asdf local` per-project
qmkonnect --help
```

### mise

```bash
mise plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect
mise install qmkonnect@latest
mise use -g qmkonnect@latest
qmkonnect --help
```

## Platform support

| OS / arch | Asset | Status |
|---|---|---|
| **Linux x86_64** | `qmkonnect-<ver>-linux-x86_64.tar.gz` | ✅ **Fully supported** — installs `qmkonnect` + `qmkonnect-hid-id` to the version dir's `bin/`. (Run the one-time udev/systemd setup below.) |
| **macOS** (arm64 / x86_64) | `QMKonnect-<ver>-macos.dmg` (universal2) | ⚠️ **CLI only** — installs the raw binary; the menu-bar tray needs the full `.app` bundle (see the caveat below). |
| Windows | `QMKonnect-<ver>-windows-x64.exe` | ❌ **Not supported via asdf/mise** — the asset is an Inno installer, not a portable binary. Use [Scoop](../scoop/README.md), [Winget](../winget/README.md), or the [Inno installer](../windows/README.md). |

## Linux — one-time system setup (udev + systemd)

asdf installs into a per-user version directory, so it cannot (and should not) run the root commands the
static udev rule and the systemd user service need. After `asdf install qmkonnect <ver>`, run this once
(the plugin stages these files under the version dir's `share/qmkonnect/`):

```bash
# Resolve the installed version dir (adjust if you pin a specific version).
SHARE="$(asdf where qmkonnect)/share/qmkonnect"

# 1. Static usage-page udev rule — grants hidraw perms for the QMK Raw HID
#    signature (usage page 0xFF60 / usage 0x61). Identical for every keyboard.
sudo install -m 644 "$SHARE/69-qmkonnect-rawhid.rules" /usr/lib/udev/rules.d/
sudo install -m 755 "$(asdf where qmkonnect)/bin/qmkonnect-hid-id" /usr/lib/udev/qmkonnect-hid-id
sudo udevadm control --reload-rules
sudo udevadm trigger

# 2. Per-user systemd service (starts at login once a matching device is present).
mkdir -p ~/.config/systemd/user
install -m 644 "$SHARE/qmkonnect.service.template" ~/.config/systemd/user/qmkonnect.service
systemctl --user daemon-reload
systemctl --user enable --now qmkonnect.service
```

Default QMK keyboards then need **no further configuration** — QMKonnect auto-discovers them by the
standard Raw HID usage page. Only disambiguate among multiple boards (or target one that overrode
`RAW_USAGE_PAGE`/`RAW_USAGE_ID` in firmware) with `qmkonnect -c` + `sudo qmkonnect -r`.

## macOS caveat — CLI only, not the menu-bar app

The macOS release is a full `QMKonnect.app` bundle inside a `.dmg`. This plugin mounts the DMG and
copies only the raw Mach-O binary (`QMKonnect.app/Contents/MacOS/qmkonnect`) into the version dir's
`bin/`. That binary runs **CLI flags** (`--help`, `--list-callbacks`, `--reload`, …) but the
**menu-bar tray/icon does not work** — that needs the bundle context (`Info.plist`, resources, template
icon paths).

For the full menu-bar app on macOS, use the [Homebrew cask](../homebrew/README.md) or the
[direct DMG installer](../../packaging/macos/install.sh) instead.

## Versioning

Versions come straight from the [GitHub Releases](https://github.com/dabstractor/qmkonnect/releases) API
at install time (`bin/list-all` scrapes the release tags and strips the leading `v`). There is no
hard-coded version in this plugin — `asdf list all qmkonnect` always reflects the published releases.

## How it works

- `bin/list-all` — queries `https://api.github.com/repos/dabstractor/qmkonnect/releases` and prints the
  versions (ascending; `latest` resolves to the newest).
- `bin/download` — maps `uname -s` / `uname -m` to the release asset, downloads it with `curl`, and
  verifies the SHA256 **if** a `<asset>.sha256` sidecar exists (none is published today; the hash is
  carried by the AUR/Homebrew/Scoop/Winget manifests instead).
- `bin/install` — Linux: `tar`-extracts the tarball; macOS: `hdiutil`-mounts the DMG; Windows: redirects
  to Scoop/Winget/Inno. Installs into `$ASDF_INSTALL_PATH/bin/`.

## License

MIT (QMKonnect is MIT-licensed). This plugin repo is MIT.