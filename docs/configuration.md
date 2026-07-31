---
layout: default
title: Configuration
permalink: /configuration/
---

# Configuration Guide

> **Firmware setup is a hard prerequisite.** QMKonnect only *sends* window
data to your keyboard over Raw HID — your keyboard cannot react to it unless the
[**qmk_notifier**](https://github.com/dabstractor/qmk_notifier) module is built
into your firmware and you've defined layer/command rules there. See the
[QMK Integration Guide]({{ site.baseurl }}/qmk-integration) for that setup. This
page covers only the **desktop-side** configuration.

This guide covers configuring the QMKonnect desktop app to communicate with your
keyboard. Once your firmware is set up, **no vendor/product-ID configuration is
needed for a single standard QMK keyboard** — QMKonnect auto-discovers any board
using the default QMK Raw HID signature (usage page `0xFF60` / usage `0x61`).
"Zero-config" refers *only* to these desktop IDs; you still must configure your
firmware. You only set a vendor/product ID when you have *multiple* QMK keyboards
connected and need to disambiguate which one QMKonnect targets.

## Platform-Specific Configuration

### Windows & macOS - GUI Settings

Both Windows and macOS use a settings dialog through the system tray:

1. **Find the system tray icon** (QMKonnect icon in your system tray/menu bar)
2. **Right-click the icon** and select "Settings"
3. **Enter your keyboard IDs** (both fields are optional — leave either blank for auto-discovery):
   - **Vendor ID**: Your keyboard's vendor ID in hex format (e.g., `feed`)
   - **Product ID**: Your keyboard's product ID in hex format (e.g., `0000`)
4. **Click OK** to save

Settings are saved automatically and work right away - no restart needed.

### Linux - Configuration File

Linux uses a TOML configuration file located at `~/.config/qmkonnect/config.toml`.

#### Creating the Configuration File

```bash
qmkonnect -c
```

This creates a default configuration file with every device-identifying field commented out, so QMKonnect auto-discovers any standard QMK keyboard out of the box:

```toml
# QMKonnect Configuration
#
# All fields are OPTIONAL. By default QMKonnect auto-discovers any QMK
# keyboard using the standard Raw HID usage page (0xFF60 / 0x61). Set
# vendor_id/product_id only to disambiguate among multiple QMK keyboards,
# or usage_page/usage to target a board that overrode RAW_USAGE_PAGE /
# RAW_USAGE_ID in its firmware.
#
# usage_page = 0xff60
# usage      = 0x61
#
# Debounce window (ms) for coalescing rapid window-change bursts before
# sending to the keyboard. 0 disables debouncing entirely. Default 50.
# debounce_ms = 50
#
# (Hyprland only) periodic active-window poll interval (ms).
# 0 disables. Default 0.
# poll_interval_ms = 0

# vendor_id  = 0xfeed   # unset: auto-discovery
# product_id = 0x0000   # unset: auto-discovery
```

#### Editing the Configuration

Edit the file with your preferred text editor:

```bash
# Using nano
nano ~/.config/qmkonnect/config.toml

# Using vim
vim ~/.config/qmkonnect/config.toml
```

Update the values:
```toml
vendor_id = 0x1234  # Replace with your keyboard's vendor ID
product_id = 0x5678  # Replace with your keyboard's product ID
```

#### Reloading Configuration (Linux Only)

After editing the configuration file, reload it:

```bash
sudo qmkonnect -r
```

This rewrites the matching udev rule under `/etc/udev/rules.d` and reloads udev. Writing the rule requires root, so run it with `sudo`; without root, `qmkonnect -r` only prints the rule without installing it. This is only needed when you set an explicit vendor/product ID to disambiguate multiple keyboards — default keyboards need no rule and no reload.

## Finding Your Keyboard IDs

Finding your keyboard's IDs is **optional** — you only need them to disambiguate among multiple QMK keyboards. Skip this section unless auto-discovery picks the wrong board. (Run `qmkonnect --list-devices` to see the exact IDs of connected keyboards.)

### Method 1: QMK Configuration

If you have your QMK configuration, look for these values in your `config.h`:

```c
#define VENDOR_ID    0xFEED
#define PRODUCT_ID   0x0000
```

### Method 2: System Tools

#### Windows
```powershell
# Using PowerShell
Get-WmiObject -Class Win32_USBHub | Where-Object {$_.Name -like "*keyboard*"}

# Or use Device Manager:
# 1. Open Device Manager
# 2. Expand "Keyboards" or "Human Interface Devices"
# 3. Right-click your keyboard → Properties → Details
# 4. Select "Hardware Ids" from dropdown
```

#### Linux
```bash
# List USB devices
lsusb

# More detailed info
lsusb -v | grep -A 5 -B 5 "keyboard\|Keyboard"

# Check hidraw devices
ls -la /dev/hidraw*
cat /sys/class/hidraw/hidraw*/device/uevent
```

#### macOS
```bash
# System Information
system_profiler SPUSBDataType | grep -A 10 -B 10 "keyboard\|Keyboard"

# Or use ioreg
ioreg -p IOUSB | grep -A 10 -B 10 "keyboard\|Keyboard"
```

## Reloading Configuration

After modifying the configuration file, reload it without restarting:

```bash
sudo qmkonnect -r
```

This rewrites the udev rule (requires root) and reloads it. Only needed when you set an explicit vendor/product ID — see the note above.

### Linux Additional Steps

On Linux, if you modified udev rules or systemd services, also run:

```bash
# Reload udev rules
sudo udevadm control --reload && sudo udevadm trigger

# Restart systemd service
systemctl --user restart qmkonnect
```

## Configuration Examples

### Zero desktop config (Linux)

This is the **desktop-side** default — no IDs are set, so QMKonnect
auto-discovers any single QMK keyboard. (Your firmware still needs qmk_notifier
built in — see [QMK Integration]({{ site.baseurl }}/qmk-integration).)
```toml
# vendor_id  = 0xfeed   # unset: auto-discovery
# product_id = 0x0000   # unset: auto-discovery
```

### Disambiguate multiple keyboards (Linux)
```toml
# Pin a specific board when more than one QMK keyboard is connected
vendor_id  = 0x1234
product_id = 0x5678
```

### Tuning (Linux, Hyprland)
```toml
# Coalesce rapid window-switch bursts (default 50 ms); poll for the active
# window every 200 ms on Hyprland (default 0 = rely on IPC events).
debounce_ms      = 50
poll_interval_ms = 200
```

## Configuration Reference

All keys are optional. With your firmware already running qmk_notifier, QMKonnect auto-discovers any standard QMK keyboard by the QMK Raw HID signature, so a single-keyboard desktop install needs no IDs set. (The firmware itself is *not* optional — see the [QMK Integration Guide]({{ site.baseurl }}/qmk-integration).)

| Key | Default | Description |
| --- | --- | --- |
| `vendor_id` | unset (any) | USB vendor ID (hex). Set only to disambiguate among multiple QMK keyboards. |
| `product_id` | unset (any) | USB product ID (hex). Set only to disambiguate among multiple QMK keyboards. |
| `usage_page` | `0xff60` | HID usage page. Set only if your firmware overrode `RAW_USAGE_PAGE`. |
| `usage` | `0x61` | HID usage. Set only if your firmware overrode `RAW_USAGE_ID`. |
| `debounce_ms` | `50` | Window (ms) for coalescing rapid window-change bursts before sending to the keyboard. `0` disables debouncing. |
| `poll_interval_ms` | `0` | (Hyprland only) periodic active-window poll interval (ms). `0` relies on IPC events instead of polling. |

### CLI flags

| Flag | Description |
| --- | --- |
| `-c`, `--config` | Create a default (commented-out) `config.toml` **and** a commented `rules.toml` template (no-op if either exists). |
| `-r`, `--reload` | Re-read the config and write the matching udev rule (Linux; requires root). |
| `-l`, `--list` | List the platforms supported by this build. |
| `--list-devices` | List connected HID devices (VID/PID discovery). |
| `--list-callbacks` | Handshake the connected keyboard and print its callback name→id table (sorted by id). With legacy firmware prints a string-only-mode notice; with no board prints a no-device notice (always exits 0). |
| `--validate-rules` [`--rules-path <path>`] | Parse `rules.toml` and report schema/callback-name errors. A parse/schema error (or a `--rules-path` that doesn't exist) exits non-zero; unknown callback names are warnings (exit 0); a missing `rules.toml` is informational (exit 0). Use `--rules-path` to validate a file outside the default location. |
| `--rules-path <path>` | Override the `rules.toml` location (used with `--validate-rules`). |
| `-v`, `--verbose` | Enable verbose logging. |
| `-h`, `--help` | Show help. |

## Host Window Rules (`rules.toml`)

Host rules let your keyboard switch layers and run callbacks based on the active
window — **without reflashing**. They live in an editable `rules.toml` file on
your computer and are matched by QMKonnect on the host. Host rules **stack on
top of** your board's existing `DEFINE_SERIAL_LAYERS` / `DEFINE_SERIAL_COMMANDS`
rules (the board's rules always run first, then host rules apply on top), unless
a rule opts into "replace" mode (see
[Stack vs. replace](#stack-vs-replace-disable_firmware_config) below).

> **Firmware prerequisite.** Host rules require firmware that advertises the
typed-command capability (`proto_ver == 2` + the `APPLY_HOST_CONTEXT` feature
flag). With legacy firmware (or while the keyboard is disconnected) QMKonnect
silently falls back to today's string-only behavior — your board's existing
rules keep working unchanged. See the
[QMK Integration Guide]({{ site.baseurl }}/qmk-integration) for the firmware
side (including the `DEFINE_HOST_CALLBACKS` callback registry and migrating
`DEFINE_*` rules to the host).

### File location

`rules.toml` lives **in the same directory as `config.toml`**, with the filename
swapped from `config.toml` to `rules.toml`:

| OS | `rules.toml` path |
| --- | --- |
| Linux | `~/.config/qmkonnect/rules.toml` (also honors `$XDG_CONFIG_HOME` and `/etc/qmkonnect/`) |
| Windows | `%APPDATA%\QMKonnect\rules.toml` |
| macOS | `~/Library/Application Support/QMKonnect/rules.toml` |

The file is **optional**. If it is absent, host rules are disabled and QMKonnect
behaves exactly as it does today (string-only). An empty or all-default
`rules.toml` is also a no-op — host rules only activate when a rule matches the
active window.

### Schema reference

`rules.toml` has one optional table and two table-arrays:

| Section / Key | Required | Default | Description |
| --- | :---: | --- | --- |
| `[host]` table | no | — | Global host defaults. |
| `[host] disable_firmware_config` | no | `false` | Global stack/replace default. `false` = the board runs its own rules too (**stack**); `true` = the host takes over (**replace**). Per-rule `disable_firmware_config` overrides this. See [Stack vs. replace](#stack-vs-replace-disable_firmware_config). |
| `[[layer_rules]]` table-array | no | `[]` | Layer rules. **First match wins**; one host layer is active at a time. |
| `[[layer_rules]] match` | **yes** | — | Window pattern. A bare string (`"alacritty"`) matches the **window class only**; a two-element array (`["*chrome*", "*youtube*"]`) matches **class and title** (equivalent to the firmware `WT(class, title)`). Supports `*`, `^`, `$`, `+`, character classes (`\d \w \s …`), and `.` — full parity with the firmware matcher. |
| `[[layer_rules]] layer` | **yes** | — | The host layer number to activate. Must be in **[224, 254]**: host layers are reserved `>= 224`, and `255` / `0xFF` is the wire "clear layer" sentinel (writing it explicitly would silently *clear* the host layer — the opposite of intent), so `parse_rules`/`--validate-rules` reject it. |
| `[[layer_rules]] case_sensitive` | no | `false` | Whether `match` is case-sensitive. |
| `[[layer_rules]] disable_firmware_config` | no | inherits `[host]` | Per-rule stack/replace override. Absent ⇒ uses the `[host]` default. |
| `[[callback_rules]]` table-array | no | `[]` | Callback rules. **All matches fire.** Names come from your keyboard's callback registry (run `qmkonnect --list-callbacks` to see them). |
| `[[callback_rules]] match` | **yes** | — | Window pattern (same form as `[[layer_rules]] match`). |
| `[[callback_rules]] enable` | no | `[]` | Callback names to enable on focus-in. |
| `[[callback_rules]] disable` | no | `[]` | Callback names to force off (**explicit exclusion**, order-independent: a `disable` in any matching rule always wins over an `enable` in any other matching rule). Focus-out `on_disable` also fires automatically when a callback leaves the active set. |
| `[[callback_rules]] case_sensitive` | no | `false` | Whether `match` is case-sensitive. |
| `[[callback_rules]] disable_firmware_config` | no | inherits `[host]` | Per-rule stack/replace override. |

**Creating the file.** Run `qmkonnect -c` to seed a fully-commented `rules.toml`
template alongside your `config.toml` (it is a no-op if `rules.toml` already
exists). The seeded template is the schema below with every active line commented
out (`#`), so it parses to an all-default ruleset — host rules stay disabled until
you uncomment and edit the lines. You can also hand-write the file. After editing,
validate it with `qmkonnect --validate-rules` (see [CLI flags](#cli-flags)).

Here is the complete annotated example (from `spec/HOST_RULES.md` §9) — what an
**active** `rules.toml` looks like once you uncomment and edit it:

```toml
# rules.toml — host-side window rules.
# disable_firmware_config chooses, per window, whether the board runs its own
# rules (stack) or is cleared and driven solely by the host (replace). Global
# default under [host]; per-rule override below. Host layers are >= 224.
# Run `qmkonnect --validate-rules` after editing.

[host]
disable_firmware_config = false   # global default: false = stack (board runs), true = replace
# On no match the host layer is always cleared and all host callbacks disabled.

# Layer rules: FIRST match wins. One host layer active at a time (>= 224).
[[layer_rules]]
match = "alacritty"                       # class-only pattern
layer = 224
disable_firmware_config = true           # optional override (default inherits [host])

[[layer_rules]]
match = ["*chrome*", "*youtube*"]         # [class_pattern, title_pattern] (== WT())
layer = 225
case_sensitive = false                    # optional, default false

# Callback rules: ALL matches fire. Names come from the keyboard's registry
# (run `qmkonnect --list-callbacks` to see them). The disable list is an
# explicit-exclusion override; focus-out on_disable fires automatically via the
# desired-set diff.
[[callback_rules]]
match = "neovide"
enable = ["vim_lazy", "disable_vim"]      # run on focus-in
disable = ["vim_lazy"]                    # optional: force-off override

[[callback_rules]]
match = ["*chrome*", "*claude*"]
enable = ["vim_lazy", "disable_vim"]
disable_firmware_config = true           # for this window, skip the string -> board can't match
```

For ready-made `rules.toml` recipes, see the
[Examples]({{ site.baseurl }}/examples).

### Stack vs. replace (`disable_firmware_config`)

`disable_firmware_config` decides, **per window**, whether your board runs its
own rules:

- **Stack** (`false`, the default): the board runs its own `DEFINE_*` rules.
  QMKonnect sends the window string first (the board matches it and runs its
  layer/command rules), then applies the host layer **on top** and syncs the host
  callbacks. Board layer first → host layer on top; board callbacks first → host
  callbacks after.
- **Replace** (`true`): the host takes over for this window. QMKonnect sends
  **no** string (so the board can't match), and the firmware clears its own board
  layer/command before applying only the host layer + host callbacks.

The value is resolved per rule:

- The **global default** is `[host] disable_firmware_config` (default `false`).
- Each `[[layer_rules]]` / `[[callback_rules]]` may set an optional
  `disable_firmware_config` to override it. **Absent ⇒ inherits the `[host]`
  default.**
- A rule's **effective** flag is its override if set, else the `[host]` default.

The per-window decision is an **AND** over all rules that matched that window:

- The window is **replace** iff **every** matched rule's effective flag is `true`
  — *or* the board has no rules of its own.
- If even **one** matched rule is non-disabling, the window is **stack**.
- **No match:** the host layer is always cleared (`255`) and all host callbacks
  are disabled. (There is no "keep" option.)

**Rule of thumb:** set `disable_firmware_config = true` on a rule when you want
the host to fully own that window (e.g. a browser site where the board's app
rules would interfere); leave it `false` when you want the board's own rules to
keep running underneath.

## Validation

After configuration, test that QMKonnect can detect your keyboard:

### Windows & macOS
Check the system tray/menu bar icon - it should show as connected.

### Linux
```bash
# Test with verbose output to see if keyboard is detected
qmkonnect -v
```

If you see "Keyboard detected" messages, you're ready to use QMKonnect.

## Troubleshooting

If your keyboard isn't detected:

1. **Double-check your vendor/product IDs** - they must match exactly
2. **Verify Raw HID is enabled** in your QMK firmware 
3. **Check permissions** (Linux users may need to be in the `input` group; the udev rule also grants `uaccess`)

For detailed troubleshooting steps, see the [troubleshooting guide]({{ site.baseurl }}/troubleshooting).

---

## QMK Firmware Configuration

Once QMKonnect can detect your keyboard, configure your QMK firmware to respond to window changes.

The qmk_notifier framework provides two main configuration macros:

### Layer Switching with DEFINE_SERIAL_LAYERS

Automatically switch layers based on active windows:

```c
DEFINE_SERIAL_LAYERS({
    // Basic application matching
    { "*calculator*", _NUMPAD },
    { "*chrome*", _BROWSER },
    { "*terminal*", _TERMINAL },
    
    // Specific window title matching
    { WT("*chrome*", "*jitsi*"), _JITSI },
    { WT("alacritty", "terminal"), _TERMINAL },
    
    // Gaming applications
    { "steam_app*", _GAMING },
    { WT("cs2", "Counter-Strike 2"), _GAMING },
});
```

### Custom Commands with DEFINE_SERIAL_COMMANDS

Execute custom functions based on window changes:

```c
DEFINE_SERIAL_COMMANDS({
    // Disable vim mode for specific applications
    { "neovide", &disable_vim_mode },
    { "alacritty", &disable_vim_mode },
    
    // Multiple commands for AI chat interfaces
    { WT("*chrome*", "*claude*"), &vim_insert, &disable_vim_mode },
    { WT("*chrome*", "*chatgpt*"), &vim_insert, &disable_vim_mode },
    
    // Gaming applications
    { WT("steam_app*", "*"), &disable_vim_mode },
});
```

## Framework Elements

### Available Macros

- **`DEFINE_SERIAL_LAYERS`**: Maps window patterns to keyboard layers
- **`DEFINE_SERIAL_COMMANDS`**: Maps window patterns to command functions  
- **`WT(class, title)`**: Helper macro to match both window class and title
- **Wildcard matching**: Use `*` for pattern matching (e.g., `"*chrome*"`)

### Understanding Window Matching

QMKonnect sends window information in this format:
```
{application_class}{GS}{window_title}
```
Where `{GS}` is the Group Separator character (ASCII 0x1D).

Examples:
- VS Code: `code{GS}main.rs - qmkonnect`
- Firefox: `firefox{GS}GitHub - Mozilla Firefox`
- Terminal: `terminal{GS}~/projects/qmkonnect`

## Pattern Matching Examples

```c
// Match any calculator app
{ "*calculator", _NUMPAD }

// Match specific browser with specific site
{ WT("*chrome*", "*jitsi*"), _JITSI }

// Match terminal with specific title
{ WT("alacritty", "terminal"), _TERMINAL }

// Match any Steam game
{ "steam_app*", _GAMING }
```

---

## Next Steps

- [Learn how to use QMKonnect]({{ site.baseurl }}/usage)
- [See real-world examples]({{ site.baseurl }}/examples)
