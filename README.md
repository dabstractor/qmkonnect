# QMKonnect

Detects window changes and tells your QMK keyboard what app you're using so it can switch layers automatically.

*📖 [Complete documentation](docs/llms_full.txt) - All guides in one comprehensive file*

## Overview

QMKonnect watches which window is active and sends that info to your QMK keyboard. Your keyboard can then switch layers or run commands based on what app you're using.

This tool is part of a broader ecosystem:
- **[qmk-notifier](https://github.com/dabstractor/qmk-notifier)**: QMK module that receives commands and handles layer/feature toggling on your keyboard
- **[qmk_notifier](https://github.com/dabstractor/qmk_notifier)**: The Rust transport library that QMKonnect links to send commands to your keyboard via Raw HID
- **QMKonnect** (this tool): Cross-platform desktop daemon that detects window changes and streams them to your keyboard through `qmk_notifier`

## Features

- **Cross-Platform Support**:
  - Windows
  - macOS
  - Linux: Arch/Hyprland only

- **Core Functionality**:
  - Detects window changes in real-time
  - Sends app name and window title to your QMK keyboard
  - Low resource usage
  - Debug logging when you need it

- **Configuration**:
  - Easy to configure
  - Reloads settings automatically

- **Host-Side Window Rules**:
  - **Change layers & callbacks without reflashing** — edit a `rules.toml` file
    on your computer, then click **Reload rules** in the tray/menu bar; no
    firmware rebuild needed
  - Host rules **stack on top of** your board's existing `DEFINE_SERIAL_*`
    rules (the board's rules run first, then host rules apply on top)
  - Requires firmware that advertises the typed-command capability
    (`proto_ver == 2`); legacy firmware keeps working in today's string-only mode
  - Full schema, CLI flags (`--list-callbacks`, `--validate-rules`), and
    per-OS file location: see the [Configuration Guide](docs/configuration.md)
    (firmware-side setup: [QMK Integration Guide](docs/qmk-integration.md))

## Installation

### Windows

> **Requirements:** Windows **10/11, 64-bit**. Not supported on 32-bit Windows or
> on Windows 8.1/8/7 and earlier. No Administrator rights or extra runtimes
> needed. Full details: [installer guide](packaging/windows/inno/README.md#supported-platforms--requirements).

1. Download **`QMKonnect-Setup.exe`** from the
   [latest release](https://github.com/dabstractor/qmkonnect/releases).
2. Double-click it — no Administrator needed (per-user install). If Windows
   shows "Unknown publisher," click **More info → Run anyway**.
3. The installer launches QMKonnect and enables **Open at Login** by default.

To build the installer yourself instead, see
[`packaging/windows/inno/README.md`](packaging/windows/inno/README.md).

### Arch Linux

```bash
git clone https://github.com/dabstractor/qmkonnect.git
cd qmkonnect/packaging/linux/arch
makepkg -si
```

### Other Linux Systems
Download the release binary: [qmkonnect](https://github.com/dabstractor/qmkonnect/releases/latest)

If you want it to start automatically, install the service file and start the service:
```
curl https://raw.githubusercontent.com/dabstractor/qmkonnect/refs/heads/main/packaging/linux/systemd/qmkonnect.service.template | sudo tee /usr/lib/systemd/user/qmkonnect.service
systemctl --user enable --now qmkonnect.service
```
If you want automatic permissions (and the service to start/stop with the
keyboard), install the static udev rule and its helper. Once your firmware is
configured (see [QMK Firmware Setup](#qmk-firmware-setup-required) — **required**),
the desktop app needs **no vendor/product-ID configuration** for a single
standard QMK keyboard: it auto-discovers it by the standard Raw HID usage page
(0xFF60 / 0x61).
```
# From a source checkout (builds qmkonnect + the qmkonnect-hid-id helper):
cargo build --release
sudo install -m755 target/release/qmkonnect        /usr/local/bin/qmkonnect
sudo install -m755 target/release/qmkonnect-hid-id /usr/lib/udev/qmkonnect-hid-id
sudo install -m644 packaging/linux/udev/69-qmkonnect-rawhid.rules \
                    /usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules
sudo udevadm control --reload && sudo udevadm trigger
```
Only set `vendor_id`/`product_id` (in `~/.config/qmk-notifier/config.toml`) to
disambiguate among multiple QMK keyboards, then install the matching rule:
```bash
qmkonnect -c          # writes a commented-out default config (edit as needed)
sudo qmkonnect -r     # root-aware: finds your config even under sudo
```

### macOS

1. Download QMKonnect.app from the [releases page](https://github.com/dabstractor/qmkonnect/releases/latest)
2. Copy QMKonnect.app to your Applications folder
3. Launch QMKonnect from Applications folder
4. It starts automatically at login by default — toggle it from the menu-bar icon → **Launch at Login**.

### From Source

**Windows:**
```bash
git clone https://github.com/dabstractor/qmkonnect.git
cd qmkonnect
cargo build --release
powershell -NoProfile -ExecutionPolicy Bypass -File packaging/windows/inno/build.ps1
#    -> packaging/windows/inno/Output/QMKonnect-Setup.exe
```

The tray-app installer is in [`packaging/windows/inno/`](packaging/windows/inno/) (Inno Setup → `QMKonnect-Setup.exe`, per-user, no admin).

**macOS:**
```bash
git clone https://github.com/dabstractor/qmkonnect.git
cd qmkonnect/packaging/macos
./clean.sh && ./build.sh && ./install.sh
```

`build.sh` produces `QMKonnect.app` and `QMKonnect.dmg`; `clean.sh` resets stale copies/permissions and `install.sh` copies the app to `/Applications`. For the full process (and why `clean.sh` matters), see the **[macOS install guide](docs/installation.md#macos)**.

**Linux:**
```bash
git clone https://github.com/dabstractor/qmkonnect.git
cd qmkonnect
cargo build --release
```



## QMK Firmware Setup (REQUIRED)

**IMPORTANT**: QMKonnect will not work at all without proper QMK firmware configuration. You must add the qmk-notifier module to your keyboard's firmware first.

### 1. Add the QMK Notifier Module

In your QMK keymap directory:

```bash
git submodule add https://github.com/dabstractor/qmk-notifier.git qmk-notifier
```

### 2. Include the Module's Build Rules

In your keymap's `rules.mk`, include the notifier's own `rules.mk` (path is
relative to the `qmk_firmware` root). That single line pulls in both
`RAW_ENABLE = yes` and `SRC += qmk-notifier/notifier.c` for you:

```make
include keyboards/handwired/<manufacturer>/<keyboard>/qmk-notifier/rules.mk
```

> Adjust the path to wherever your keyboard lives under `qmk_firmware/keyboards/`. The module's `rules.mk` (which you can read in the
> [qmk-notifier](https://github.com/dabstractor/qmk-notifier) repo) is what
> actually compiles `notifier.c` — without this `include`, the build will fail
> to link `hid_notify`.

### 3. Configure Your Keymap

Add this to your `keymap.c`:

```c
#include QMK_KEYBOARD_H
#include "./qmk-notifier/notifier.h"

void raw_hid_receive(uint8_t *data, uint8_t length) {
    hid_notify(data, length);
}

// Your keymap definitions here...
```

### 4. Set Up Layer Switching

Create your layer definitions and serial commands. See the [Examples](https://dabstractor.github.io/qmkonnect/examples) for the correct implementation using `DEFINE_SERIAL_LAYERS` and `DEFINE_SERIAL_COMMANDS` macros.

### 5. Flash Your Keyboard

Build and flash your updated firmware to your keyboard. **QMKonnect cannot communicate with your keyboard until this firmware is installed.**

## Configuration

> **Prerequisite — firmware setup is required.** QMKonnect only *sends* window
data to your keyboard over Raw HID. Your keyboard can't act on it unless the
[**qmk-notifier**](https://github.com/dabstractor/qmk-notifier) module is built
into your firmware. See [QMK Firmware Setup](#qmk-firmware-setup-required).
> Everything below covers only the *desktop-side* configuration.

Once your firmware is set up, the **desktop app needs no vendor/product-ID
configuration** for a single standard QMK keyboard — QMKonnect auto-discovers it
via the Raw HID usage page (0xFF60 / 0x61). You only set a Vendor/Product ID to
disambiguate among multiple QMK keyboards.

> **Config file locations** (historical naming is preserved so existing installs keep working):
> - **Linux**: `~/.config/qmk-notifier/config.toml`
> - **Windows**: `%APPDATA%\QMKonnect\config.toml`
> - **macOS**: `~/Library/Application Support/QMKonnect/config.toml`

Don't know your keyboard's IDs? Discover them with read-only enumeration:

```bash
qmkonnect --list-devices
```

### Windows & macOS

1. Right-click the QMKonnect system tray icon
2. Select "Settings"
3. Enter your keyboard's Vendor ID (hex format, e.g., feed)
4. Enter your keyboard's Product ID (hex format, e.g., 0000)
5. Click OK to save

### Linux

Edit the configuration file at `~/.config/qmk-notifier/config.toml`.

If no file exists, create it:

```bash
qmkonnect -c
```

Only set these to pin a specific keyboard (otherwise leave them commented out for auto-discovery):
```
# vendor_id = 0xfeed
# product_id = 0x0000
```

Then reload (writes the matching udev rule and reloads udev — run as root):

```bash
sudo qmkonnect -r
```

If you aren't root, `qmkonnect -r` prints the exact udev rule and the commands to install it.

## Usage

### Windows

The application starts automatically with Windows and runs in the background with a system tray icon.

- **Start manually**: Run "QMKonnect" from Start Menu
- **Exit**: Right-click the system tray icon and select "Quit"

### macOS

- **Start**: Launch QMKonnect from Applications folder
- **Exit**: Right-click the menu bar icon and select "Quit"

### Linux

The application should start automatically when your keyboard is plugged in.
If not, you can start it manually:
```bash
qmkonnect & disown
```

## Technical Requirements

### Windows Implementation

- **Tray app, not a service**: runs as a per-user interactive application with a
  system-tray icon (built on `tray-icon`/`muda`). It is **not** a Windows service
  — a Session-0 service can't show a tray icon in your interactive session.
- **Background Operation**: no console window (`windows_subsystem = "windows"`).
- **Automatic Startup**: **Open at Login** via the HKCU `Run` key — default on,
  toggleable from the tray (`src/autostart.rs`).
- **Installer**: per-user Inno Setup installer — `QMKonnect-Setup.exe`, no admin
  (`packaging/windows/inno/`). The executable statically links the C runtime, so
  **no Visual C++ Redistributable** is required.
- **Singleton Pattern**: a named-mutex single-instance lock prevents multiple
  instances from running simultaneously.
- **Window Monitoring**: detects foreground-window focus changes.

### Core Functionality

- **Window Detection**: Monitors active window changes across platforms
- **QMK Integration**: Sends window information to QMK keyboards
- **Configuration Management**: User-configurable settings
- **Error Handling**: Graceful handling of errors and edge cases

## Integration with QMK

This tool works in conjunction with:
- The [qmk-notifier](https://github.com/dabstractor/qmk-notifier) QMK module running on your keyboard
- The [qmk_notifier](https://github.com/dabstractor/qmk_notifier) transport library that QMKonnect links to handle the Raw HID communication

When a window focus change is detected, this application formats the data as:
`{application_class}{GS}{window_title}` where `{GS}` is the Group Separator character (0x1D).

## Default Configuration

This is the **desktop-side** config only (your firmware still needs qmk-notifier — see above). QMKonnect auto-discovers standard QMK keyboards, so the default config leaves these commented out (uncomment only to pin a specific keyboard):

```toml
# Your QMK keyboard's vendor ID (in hex)
# vendor_id = 0xfeed

# Your QMK keyboard's product ID (in hex)
# product_id = 0x0000
```

## Example Use Cases

- Automatically switch to a coding layer when your IDE is focused
- Enable media controls when music or video applications are active
- Activate application-specific macros based on the current window
- Create context-aware keyboard layouts that adapt to your workflow

## Contributing

Contributions are welcome! Feel free to submit issues or pull requests on GitHub.

## License

[MIT License](LICENSE)
