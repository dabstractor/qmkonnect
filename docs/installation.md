---
layout: default
title: Installation
permalink: /installation/
---

# Installation Guide

> **Before you start:** QMKonnect only *sends* window data to your keyboard. Your
> keyboard must be running the companion [**qmk-notifier**](https://github.com/dabstractor/qmk-notifier)
> firmware module for anything to happen — that setup is **required**. Install
> QMKonnect below, then follow the [QMK Integration Guide]({{ site.baseurl }}/qmk-integration).

QMKonnect has different installation methods for each platform.

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

Build the package from the local `PKGBUILD` (there is no AUR package):

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
sudo udevadm control --reload && sudo udevadm trigger
```

   Only set `vendor_id`/`product_id` to disambiguate among multiple QMK
   keyboards; then generate the matching rule (root-aware, works under sudo):

```bash
qmkonnect -c          # writes a commented-out default config (edit as needed)
sudo qmkonnect -r
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
app talks to the keyboard — your **firmware** must also have qmk-notifier set up,
see [QMK Integration]({{ site.baseurl }}/qmk-integration)):

1. **Check if running**:
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
