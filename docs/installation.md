---
layout: default
title: Installation
permalink: /installation/
---

# Installation Guide

> **Before you start:** QMKonnect only *sends* window data to your keyboard. Your
> keyboard must be running the companion [**qmk_notifier**](https://github.com/dabstractor/qmk_notifier)
> firmware module for anything to happen — that setup is **required**. Install
> QMKonnect below, then follow the [QMK Integration Guide]({{ site.baseurl }}/qmk-integration).

QMKonnect has different installation methods for each platform.

## Installation Methods

QMKonnect ships through a **direct installer** (recommended) and, on each platform, one or more
**community package-manager channels** that keep it updated automatically. Pick one per platform —
the exact commands and caveats are in each platform's **Package Managers** section below.
(Full compatibility matrix: PRD §5.)

| Platform | Direct installer (recommended) | Community channels |
| --- | --- | --- |
| **Windows** 10/11 (x64) | Inno `.exe` (per-user, no admin) | Scoop · Winget |
| **macOS** 13+ | `.dmg` (universal) | Homebrew Cask |
| **Linux** (Hyprland) | binary / Arch PKGBUILD | AUR · Nix |

**mise / asdf** are cross-platform version managers that install the prebuilt release binary:
**Linux** (full app) and **macOS** (**CLI only — no menu-bar tray**); not available on Windows.
See the per-platform sections.

## Windows

### Installer (Recommended)

1. Download **`QMKonnect-Setup.exe`** from the [latest release](https://github.com/dabstractor/qmkonnect/releases).
2. Double-click it — **no Administrator needed** (per-user install to `%LOCALAPPDATA%`). If Windows shows "Unknown publisher," click **More info → Run anyway**.
3. The installer launches QMKonnect and enables **Open at Login** by default.

> **Requirements:** Windows **10/11, 64-bit**. 32-bit Windows and 8.1/8/7 and
> earlier are **not supported**. No extra runtimes needed (the C runtime is
> statically linked, so there's no Visual C++ Redistributable to install).

The installer:
- Installs to `%LOCALAPPDATA%\Programs\QMKonnect` (per-user, no admin)
- Enables autostart via the HKCU `Run` key (toggle it in the tray: **Open at Login**)
- Sets up the system-tray icon
- Uninstalls cleanly via Add/Remove Programs

### Package Managers

Community package managers on Windows fetch the same Inno installer and keep it updated
automatically. Both are **per-user — no Administrator** needed.

**Scoop** (extracts the installer; no publisher prompt):

```bash
scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnect
scoop install qmkonnect
scoop update qmkonnect      # pull a later release
```

Because Scoop *extracts* the installer via `innounp` instead of running it, **autostart is off
by default** — enable **"Open at Login"** in QMKonnect's tray menu (the app writes the same HKCU
`Run` value itself). There is no Add/Remove-Programs entry; manage the app with `scoop update` /
`scoop uninstall qmkonnect`.

**Winget** (runs the installer; same result as the direct `.exe`):

```powershell
winget install dabstractor.QMKonnect      # or: winget install qmkonnect
winget upgrade dabstractor.QMKonnect      # keep current
```

The installer is **not code-signed**, so the first `winget install` (and Windows SmartScreen)
shows an **"unverified publisher"** prompt — choose *More info → Run anyway*. This is the expected
beta state, identical to running the unsigned direct installer, and goes away once QMKonnect has a
stable code-signing certificate. (Scoop is unaffected — it extracts rather than runs, so it never
trips the publisher check.)

### Build from Source

```bash
git clone https://github.com/dabstractor/qmkonnect.git
cd qmkonnect
cargo build --release
powershell -NoProfile -ExecutionPolicy Bypass -File packaging/windows/inno/build.ps1
```

This creates `packaging/windows/inno/Output/QMKonnect-Setup.exe`. See
[`packaging/windows/inno/README.md`](https://github.com/dabstractor/qmkonnect/blob/main/packaging/windows/inno/README.md)
for the full release-build & installation procedure.

---

## Linux

### Linux (Hyprland Only)

**Note**: QMKonnect currently only supports Hyprland on Linux. Other window managers are not supported yet. Please contribute support for your window manager!

#### Arch Linux

Build from the **source** `PKGBUILD` (or install the prebuilt binary from the AUR — see
**Package Managers** below):

```bash
git clone https://github.com/dabstractor/qmkonnect.git
cd qmkonnect/packaging/linux/arch
makepkg -si
systemctl --user enable --now qmkonnect # if you want it to start on hotplug
```

#### Other Linux Distributions

1. Download the release binary: [qmkonnect](https://github.com/dabstractor/qmkonnect/releases/download/v0.1.0/qmkonnect)
2. Install the binary:

```bash
# Make executable and copy to PATH
chmod +x qmkonnect
sudo cp qmkonnect /usr/local/bin/
```

3. Set up systemd service (optional but recommended):

```bash
# Install service file
curl https://raw.githubusercontent.com/dabstractor/qmkonnect/refs/heads/main/packaging/linux/systemd/qmkonnect.service.template | sudo tee /usr/lib/systemd/user/qmkonnect.service

# Enable and start the service
systemctl --user enable --now qmkonnect.service
```

### Autostart at login

On Linux, QMKonnect can start at login two ways, and the packages set both up:

- **systemd user service** (primary on systemd distros) — started by the
  static udev rule's `SYSTEMD_USER_WANTS` when your keyboard is present, with
  a `BindsTo` lifecycle that stops/restarts it on unplug/replug. Enable with
  `systemctl --user enable --now qmkonnect.service` (the Arch/AUR/Debian/RPM
  packages enable it globally on install).
- **XDG autostart entry** (`/etc/xdg/autostart/qmkonnect.desktop`) — a
  universal fallback honored by GNOME, KDE Plasma, XFCE, COSMIC, MATE,
  Cinnamon, LXQt, Budgie and the session-managed tail. It starts the daemon
  at **login on every desktop — systemd or not** (MX, Artix, Void, Gentoo),
  where it is the load-bearing path. On systemd distros it is redundant-but-
  harmless (the daemon's own single-instance lock dedupes the two launches).

The trade-off: the `.desktop` is login-only-start and loses the systemd
plug/unplug lifecycle, so on systemd distros the service stays primary. To
disable the autostart entry, copy it to
`~/.config/autostart/qmkonnect.desktop` and set `Hidden=true` (the per-user
copy overrides the system one). The shipped file's `NoDisplay=true` only hides
it from application menus — it does **not** disable autostart; use `Hidden=true`
for that. Note: pure wlroots compositors (Sway, Hyprland without a session
manager) do not run `/etc/xdg/autostart` natively — install `dex` or enable the
systemd `xdg-autostart-generator` there.

4. Set up udev rules for automatic keyboard detection:

   **Default QMK keyboards need no udev configuration.** Install the static
   rule (and its helper) once and any device exposing the QMK Raw HID
   signature (usage page 0xFF60 / usage 0x61) gets permissions automatically:

```bash
# From a source checkout (builds qmkonnect + the qmkonnect-hid-id helper):
cargo build --release
sudo install -m755 target/release/qmkonnect        /usr/local/bin/qmkonnect
sudo install -m755 target/release/qmkonnect-hid-id /usr/lib/udev/qmkonnect-hid-id
sudo install -m644 packaging/linux/udev/69-qmkonnect-rawhid.rules \
                    /usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules
sudo install -m644 packaging/linux/xdg/qmkonnect.desktop /etc/xdg/autostart/
sudo udevadm control --reload && sudo udevadm trigger
```

   You rarely set `vendor_id`/`product_id` by hand: the **Settings →
   discovered-device picker** lists each connected QMK board (with a
   ✓/✗ qmk_notifier-capable marker) and writes the IDs for you when several
   boards are present. Set them manually only to disambiguate among
   multiple QMK keyboards, then generate the matching rule (root-aware,
   works under sudo):

```bash
qmkonnect -c          # writes a commented-out default config (edit as needed)
sudo qmkonnect -r
```

### GNOME (optional Shell extension)

GNOME (Mutter) advertises neither Wayland foreign-toplevel protocol and exposes
no client API for the active window, so QMKonnect detects windows on GNOME via
the **`qmkonnect@mulletware`** Shell extension. The extension reads
`global.display.focus_window` inside `gnome-shell` and republishes the active
window `(app_class, title)` over the session D-Bus as the well-known name
`io.mulletware.QMKonnect`; the daemon's GNOME backend subscribes to it.
(See `spec/PLATFORMS.md` §8 for the authoritative spec.)

> On every other desktop (Hyprland, Sway, KDE Plasma 6, COSMIC, …) QMKonnect
> uses the Wayland foreign-toplevel protocol directly — **no extension needed.**

**Install the extension** (GNOME 45–50):

1. Download the `qmkonnect@mulletware` extension from
   [extensions.gnome.org](https://extensions.gnome.org) (search
   "qmkonnect"), **or** grab the release `.zip` from the
   [GitHub Releases](https://github.com/dabstractor/qmkonnect/releases) and
   install it locally:
   ```bash
   gnome-extensions install --force qmkonnect@mulletware.shell-extension.zip
   ```
2. Enable it in the **Extensions** app (or `gnome-extensions enable
   qmkonnect@mulletware`). On a Wayland session, **log out and back in** the
   first time so `gnome-shell` picks up the new extension.
3. Run QMKonnect verbose and confirm the GNOME backend is selected:
   ```bash
   qmkonnect -v
   # …→ 'gnome' available, selected   (then [<ms>] gnome: <app> | <title> on focus changes)
   ```

The daemon auto-selects the GNOME backend whenever the extension's D-Bus name is
owned (installed **and** enabled); no config change is required. If you later
*disable* the extension mid-session, the daemon switches to its no-backend
posture within ~1 s (the tray and device pipeline keep running) and re-acquires
state automatically when you re-enable it.

**AUR (Arch)** — `qmkonnect-bin` is the prebuilt-binary package: it downloads the GitHub release
tarball (no Rust toolchain or build dependencies). It is the `-bin` sibling of the source `PKGBUILD`
above — both install to the same paths and reuse the same pacman hooks (udev reload, systemd-template
instantiation, global enable).

```bash
yay -S qmkonnect-bin          # or: paru -S qmkonnect-bin
```

The pacman hooks run automatically on install/upgrade, so default QMK keyboards then need **no
configuration** — QMKonnect auto-discovers them via the Raw HID usage page (`0xFF60` / `0x61`) and
the shipped static udev rule already grants permissions.

**Nix** (NixOS, or Nix on another distro) — the flake builds from source against pinned Nixpkgs:

```bash
nix profile install github:dabstractor/qmkonnect   # add to your profile
# …or run ad-hoc without installing:
nix run github:dabstractor/qmkonnect
```

On **NixOS**, prefer the flake's module — add `qmkonnect.nixosModules.default` to your config and:

```nix
services.qmkonnect.enable = true;   # udev rule + systemd user service + PATH
```

On **non-NixOS** (Nix on Arch/Ubuntu/Fedora/…), Nix can't install the udev rule system-wide, so do
the one-time HID-permissions setup (install the static rule, symlink the `qmkonnect-hid-id` helper
the package ships, reload udev) — see the
[Nix flake README](https://github.com/dabstractor/qmkonnect/blob/main/packaging/nix/README.md).

**mise / asdf** — cross-platform version managers. The same `asdf-qmkonnect` plugin serves both
(mise runs asdf plugin scripts unchanged). **Linux is fully supported** — install the binary, then
run the one-time udev/systemd setup the plugin documents:

```bash
# asdf:
asdf plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect
asdf install qmkonnect latest
# mise:
mise plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect
mise install qmkonnect@latest
```

---

## macOS

> **This is the source-of-truth guide for building, installing, and running QMKonnect on macOS.**
> The README links here.

### Install from a release (for end users)

1. Download `QMKonnect.dmg` from the [releases page](https://github.com/dabstractor/qmkonnect/releases).
2. Open the `.dmg` and drag **QMKonnect.app** into the **Applications** folder.
3. Eject the disk image.
4. Launch QMKonnect from Applications. The first time, macOS Gatekeeper will warn that the app *cannot be verified* (it is ad-hoc signed and not notarized). Right-click the app → **Open** → **Open** to proceed.
5. Grant the **Screen Recording** prompt when it appears — this is required to read window titles (see [Troubleshooting → Screen Recording]({{ site.baseurl }}/troubleshooting/) if it keeps reappearing).

### Package Managers

**Homebrew Cask** — installs the universal `QMKonnect.app` into `/Applications` and keeps it updated
with `brew upgrade`. It ships through a **custom tap** (`mulletware/qmkonnect`), not the official
`homebrew-cask`, until the DMG is Developer-ID-signed + notarized:

```bash
brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect
brew install --cask qmkonnect
```

> **Quarantine caveat (ad-hoc / unnotarized DMG):** the released DMG is **ad-hoc signed and not
> notarized**, so Homebrew quarantines it and Gatekeeper blocks the first launch ("'QMKonnect' is
> damaged / can't be opened"). Bypass quarantine for now:
> ```bash
> brew install --cask --no-quarantine qmkonnect
> # …or, after a normal install:
> xattr -dr com.apple.quarantine /Applications/QMKonnect.app
> ```
> Once the DMG is notarized this flag is unnecessary and the cask can graduate to the official
> `homebrew-cask` repo. The **Screen Recording** prompt (for window titles) is still required either
> way — see [Troubleshooting]({{ site.baseurl }}/troubleshooting/).

Uninstall with `brew uninstall --cask qmkonnect` (add `--zap` to also remove the per-user config
under `~/Library/Application Support/QMKonnect/`).

**mise / asdf — CLI only (no menu-bar tray).** These install the raw Mach-O binary from the DMG,
which runs CLI flags (`--help`, `--list-callbacks`, `-r`, …) but **not** the menu-bar tray/icon —
that needs the full `.app` bundle. For the complete macOS app, use the **Homebrew cask** above or
the **direct DMG** instead:

```bash
asdf plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect
asdf install qmkonnect latest        # CLI only — no menu-bar app
```

### Launch at login

QMKonnect starts automatically when you log in — **enabled by default** on first launch, so it just works out of the box.

- Toggle it any time from the menu-bar icon → **Launch at Login**.
- It's a real macOS login item: manage or disable it in **System Settings → General → Login Items** (under *Open at Login*); QMKonnect honors changes you make there.
- It launches the app **bundle** (with its tray icon), not a background daemon, and **Quit** in the menu always quits — there is no auto-restart.

This uses `SMAppService` (macOS 13 Ventura and later). On older macOS the menu item is inert — the app runs normally, it just won't auto-start. `packaging/macos/uninstall.sh` removes the entry on uninstall.

### Build from source (for developers)

Everything you need ships in `packaging/macos/` as a set of scripts that form one loop:

```bash
git clone https://github.com/dabstractor/qmkonnect.git
cd qmkonnect/packaging/macos
./clean.sh && ./build.sh && ./install.sh
open /Applications/QMKonnect.app     # grant the one Screen-Recording prompt
```

| Script | What it does |
| --- | --- |
| `clean.sh` | Stops the app, ejects stale `.dmg` mounts, unregisters & deletes old `QMKonnect.app` copies from LaunchServices, and resets Screen-Recording permission. Run **before every reinstall**. |
| `build.sh` | `cargo build --release`, assembles `QMKonnect.app`, ad-hoc code-signs it, and packages `QMKonnect.dmg` (the same image users install). |
| `install.sh` | Mounts the `.dmg` and copies `QMKonnect.app` into `/Applications`. |
| `uninstall.sh` | Fully removes the app, its **Launch at Login** entry, and per-user config. |

#### Prerequisites

- [Rust](https://rustup.rs/) **1.88 or later** (latest stable recommended; enforced by `rust-version` in `Cargo.toml`). Older toolchains fail with a cryptic transitive-dependency error.
- Apple's Command Line Tools (install once): `xcode-select --install`.

#### Why you must `clean` before reinstalling (read this once)

`build.sh` **always** compiles current source — your binary is correct. The "old version" problem happens at **launch**, not build time, because of two macOS behaviors:

1. **LaunchServices remembers every copy.** macOS keeps a registry of every `QMKonnect.app` it has ever seen — old `/Applications` installs, trashed copies, and apps left inside a mounted `.dmg`. When you launch, macOS can hand you a **stale** copy instead of the one you just built.
2. **Ad-hoc signing vs. Screen Recording.** Local builds are ad-hoc signed, so the app's signature (`cdhash`) changes on every rebuild. macOS keys the Screen-Recording grant to that signature, so it re-prompts every build — *even though System Settings still lists QMKonnect as already granted*.

That is exactly what `clean.sh` undoes — clear the old copies and reset the permission, then build/install/launch. Skipping it is the #1 cause of "I rebuilt but nothing changed."

#### Tips

- **Launch the bundle, not the binary.** Always start the app with `open /Applications/QMKonnect.app`. Do **not** test the menu bar by running `target/release/qmkonnect` directly — outside a real app bundle the menu-bar icon and template path don't work. The raw binary is fine for CLI subcommands (`--list-devices`, `-c`, `-r`).
- **If a stale copy keeps winning** after `clean.sh`, unregister it explicitly and list what macOS still remembers:
  ```bash
  LSR=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
  "$LSR" -u /Applications/QMKonnect.app          # unregister one path
  "$LSR" -dump | grep -i 'path:.*QMKonnect\.app' # list every registered copy
  ```
- **Stop the Screen-Recording re-prompt loop for good** by signing with a stable identity: `CODESIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" ./build.sh`. A release DMG should be signed this way and notarized.

---

## Build from Source (Linux Only)

For Linux users who want to build from source:

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable version)
- Platform dependencies:
  - **Ubuntu/Debian**: `sudo apt install libxdo-dev libudev-dev`
  - **Fedora**: `sudo dnf install libxdo-devel systemd-devel`

### Build Steps

```bash
# Clone the repository
git clone https://github.com/dabstractor/qmkonnect.git
cd qmkonnect

# Build the project
cargo build --release

# The binary will be available at target/release/qmkonnect
```

---

## Verification

After installation, verify QMKonnect is working (this only confirms the desktop
app talks to the keyboard — your **firmware** must also have qmk_notifier set up,
see [QMK Integration]({{ site.baseurl }}/qmk-integration)):

1. **Check the tray/menu-bar icon** — it shows one of three device states:
   - **● Device Connected** — a qmk_notifier-capable board is present (you're set).
   - **⚠ QMK board found — no qmk_notifier module (flash it)** — a QMK board
     is attached but isn't running qmk_notifier; flash it (see the
     [QMK Integration Guide]({{ site.baseurl }}/qmk-integration)).
   - **○ No Device Connected** — no QMK board detected.

   Platform quick-checks that the process is running:
   - Windows: Look for the system tray icon
   - Linux: `systemctl --user status qmkonnect`
   - macOS: Check Activity Monitor

2. **Test configuration**:
   - Windows: Right-click system tray icon → Settings
   - Linux: `qmkonnect -c` then `qmkonnect -v`
   - macOS: Right-click menu bar icon → Settings

3. **Check logs**:
   - Windows: System tray interface
   - Linux: `journalctl --user -u qmkonnect`
   - macOS: System menu bar interface

---

## Next Steps

After installation:

1. [Set up your QMK firmware]({{ site.baseurl }}/qmk-integration)
2. [Configure your keyboard settings]({{ site.baseurl }}/configuration)
3. [Start using QMKonnect]({{ site.baseurl }}/usage)

---

📖 **[Complete Documentation]({{ site.baseurl }}/llms_full.txt)** - All guides in one comprehensive file
