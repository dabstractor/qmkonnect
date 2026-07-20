---
layout: default
title: Examples
permalink: /examples/
---

# Real-World Examples

Complete examples showing how to configure your **QMK firmware** for different use cases.

> **These are firmware examples, not desktop-app configuration.** They assume
> you've already integrated the
> [qmk_notifier](https://github.com/dabstractor/qmk_notifier) module into your
> firmware (see the [QMK Integration Guide]({{ site.baseurl }}/qmk-integration)).
> QMKonnect only sends the active-window string; these `DEFINE_SERIAL_LAYERS` /
> `DEFINE_SERIAL_COMMANDS` rules are what make your keyboard actually react.
> Want to drive layer/callback switching from the desktop without reflashing?
> See [Example 4: Host-Side Rules (`rules.toml`)](#example-4-host-side-rules-rulestoml) below.

## Example 1: Developer Setup

**Scenario**: A developer who uses VS Code, Terminal, and browsers, wanting different layouts for each context.

### QMK Keymap Configuration

```c
#include QMK_KEYBOARD_H
#include "qmk_notifier/notifier.h"

void raw_hid_receive(uint8_t *data, uint8_t length) {
    hid_notify(data, length);
}

// Layer definitions
#define _QWERTY   0
#define _CODE     1
#define _TERMINAL 2
#define _BROWSER  3

DEFINE_SERIAL_LAYERS({
    // Development environments
    { "*code*", _CODE },           // VS Code, VSCodium
    { "*neovim*", _CODE },         // Neovim
    { "*jetbrains*", _CODE },      // IntelliJ, PyCharm, etc.
    
    // Terminal applications  
    { "*terminal*", _TERMINAL },   // Generic terminal
    { "*alacritty*", _TERMINAL },  // Alacritty
    { "*iterm*", _TERMINAL },      // iTerm (macOS)
    
    // Browsers
    { "*chrome*", _BROWSER },      // Chrome
    { "*firefox*", _BROWSER },     // Firefox  
    { "*safari*", _BROWSER },      // Safari
});

const uint16_t PROGMEM keymaps[][MATRIX_ROWS][MATRIX_COLS] = {
    [_QWERTY] = LAYOUT(
        // Your standard QWERTY layout
    ),
    
    [_CODE] = LAYOUT(
        // Coding-optimized layout with easy access to:
        // - Brackets, braces, parentheses
        // - Common programming symbols
        // - Function keys for debugging
    ),
    
    [_TERMINAL] = LAYOUT( 
        // Terminal-focused layout:
        // - Easy access to Ctrl combinations
        // - Arrow keys, Page Up/Down
        // - Common shell shortcuts
    ),
    
    [_BROWSER] = LAYOUT(
        // Browser navigation layout:
        // - Tab management shortcuts
        // - Bookmarks, history navigation  
        // - Zoom controls
    ),
};
```

**Result**: Your keyboard automatically switches to the appropriate layer when you switch between VS Code (coding layer), Terminal (terminal layer), and your browser (browser layer).

## Example 2: Gaming & Productivity

**Scenario**: A user who games and does office work, wanting to disable certain keys during games.

### QMK Keymap Configuration

```c
#include QMK_KEYBOARD_H
#include "qmk_notifier/notifier.h"

void raw_hid_receive(uint8_t *data, uint8_t length) {
    hid_notify(data, length);
}

#define _QWERTY  0
#define _GAMING  1 
#define _OFFICE  2

// Custom functions
void enable_gaming_mode(void) {
    // Disable Windows key, Alt-Tab, etc.
    // Implementation depends on your needs
}

void disable_gaming_mode(void) {
    // Re-enable all keys
}

DEFINE_SERIAL_LAYERS({
    // Gaming applications
    { "steam_app*", _GAMING },         // Any Steam game
    { "*minecraft*", _GAMING },        // Minecraft
    { WT("cs2", "*"), _GAMING },       // Counter-Strike 2
    { "*valorant*", _GAMING },         // Valorant
    
    // Office applications  
    { "*word*", _OFFICE },             // Microsoft Word
    { "*excel*", _OFFICE },            // Microsoft Excel
    { "*powerpoint*", _OFFICE },       // PowerPoint
    { "*outlook*", _OFFICE },          // Outlook
});

DEFINE_SERIAL_COMMANDS({
    // Enable gaming mode for any game
    { "steam_app*", &enable_gaming_mode },
    { "*minecraft*", &enable_gaming_mode },
    { WT("cs2", "*"), &enable_gaming_mode },
    
    // Disable gaming mode for productivity apps
    { "*word*", &disable_gaming_mode },
    { "*excel*", &disable_gaming_mode },
});

const uint16_t PROGMEM keymaps[][MATRIX_ROWS][MATRIX_COLS] = {
    [_QWERTY] = LAYOUT(
        // Standard layout
    ),
    
    [_GAMING] = LAYOUT(
        // Gaming layout:
        // - WASD optimized positioning  
        // - Easy access to F1-F12
        // - Disabled Windows key
        // - Gaming-specific macros
    ),
    
    [_OFFICE] = LAYOUT(
        // Office productivity:
        // - Common shortcuts (Ctrl+C, Ctrl+V, etc.)
        // - Number row easily accessible
        // - Office-specific function keys
    ),
};
```

**Result**: When you launch any Steam game, your keyboard switches to gaming mode and disables problematic keys. When you switch to office applications, it switches to a productivity-focused layout.

## Example 3: Content Creation Setup

**Scenario**: A content creator using different applications for video editing, streaming, and social media.

### QMK Keymap Configuration

```c
#include QMK_KEYBOARD_H
#include "qmk_notifier/notifier.h"

void raw_hid_receive(uint8_t *data, uint8_t length) {
    hid_notify(data, length);
}

#define _QWERTY    0
#define _VIDEO     1
#define _STREAMING 2
#define _SOCIAL    3

DEFINE_SERIAL_LAYERS({
    // Video editing
    { "*premiere*", _VIDEO },          // Adobe Premiere Pro
    { "*davinci*", _VIDEO },           // DaVinci Resolve  
    { "*final*cut*", _VIDEO },         // Final Cut Pro
    
    // Streaming software
    { "*obs*", _STREAMING },           // OBS Studio
    { "*streamlabs*", _STREAMING },    // Streamlabs
    
    // Social media & communication
    { "*discord*", _SOCIAL },          // Discord
    { "*slack*", _SOCIAL },            // Slack
    { "*twitter*", _SOCIAL },          // Twitter/X
    { WT("*chrome*", "*youtube*"), _SOCIAL }, // YouTube in browser
});

const uint16_t PROGMEM keymaps[][MATRIX_ROWS][MATRIX_COLS] = {
    [_QWERTY] = LAYOUT(
        // Standard layout
    ),
    
    [_VIDEO] = LAYOUT(
        // Video editing shortcuts:
        // - Timeline navigation
        // - Cut, copy, paste optimized
        // - Playback controls
        // - Common effects shortcuts
    ),
    
    [_STREAMING] = LAYOUT(
        // Streaming controls:
        // - Scene switching
        // - Mute/unmute shortcuts
        // - Stream start/stop
        // - Chat interaction keys
    ),
    
    [_SOCIAL] = LAYOUT(
        // Social media optimized:
        // - Emoji shortcuts
        // - Quick reactions
        // - Navigation shortcuts
        // - Typing-focused layout
    ),
};
```

**Result**: Your keyboard adapts to your workflow - switching to video editing shortcuts in Premiere Pro, streaming controls in OBS, and social media optimizations in Discord or when managing YouTube.

## Example 4: Host-Side Rules (`rules.toml`)

**Scenario**: You want the same app→layer / app→callback behavior as Examples 1–3,
but editable from your desktop **without reflashing**. Host rules live in an
optional `rules.toml` file; QMKonnect does the matching on the host and pushes
layer/callback decisions to the keyboard over Raw HID.

> Host rules **stack on top of** your board's `DEFINE_SERIAL_*` rules (the board's
> rules run first, then host rules apply on top) — unless a rule opts into
> "replace" mode. They require firmware that advertises the typed-command
> capability (`proto_ver == 2`); legacy firmware silently falls back to today's
> string-only behavior. The host matcher is a **full-parity port** of the firmware
> `pattern_match.c`, so the patterns from Examples 1–3 (`*`, `WT(class,title)`,
> `^`, `$`, `+`, `\d`, …) translate directly. See the
> [Configuration Guide]({{ site.baseurl }}/configuration) for the complete schema,
> and the [QMK Integration Guide]({{ site.baseurl }}/qmk-integration) for the
> one-time firmware change and the full migration procedure.

### Before: firmware rules (Example 2, baked into your keymap)

This is the gaming/office setup from Example 2, expressed as firmware rules. To
enable host rules you add a **one-time** named callback registry and keep your
existing `DEFINE_SERIAL_*` rules:

```c
// One-time firmware change: name the callbacks you already use.
static void enable_gaming_mode(void)  { /* disable Win key, Alt-Tab, … */ }
static void disable_gaming_mode(void) { /* re-enable all keys */ }

DEFINE_HOST_CALLBACKS({
    { "enable_gaming",  &enable_gaming_mode,  &disable_gaming_mode },
    { "disable_gaming", &disable_gaming_mode, &enable_gaming_mode  },  // on_disable may be NULL
});

DEFINE_SERIAL_LAYERS({
    { "steam_app*", _GAMING }, { WT("cs2", "*"), _GAMING }, { "*word*", _OFFICE },
});
DEFINE_SERIAL_COMMANDS({
    { "steam_app*", &enable_gaming_mode }, { "*word*", &disable_gaming_mode },
});
```

The `DEFINE_HOST_CALLBACKS` row is `{ name, on_enable, on_disable }` — `name` is
the string you'll reference from `rules.toml`, `on_disable` may be `NULL`, and the
id is just the array index. The pattern (`"steam_app*"`, `WT("cs2","*")`, …) is
**not** part of the registry — it moves to `rules.toml`.

### After: the same rules as `rules.toml`

Move those rules into `rules.toml` (alongside `config.toml`). Host layers are
reserved **≥ 224** (so they resolve above your board layers), so the firmware
`_GAMING = 1` becomes host `layer = 224`:

```toml
[host]
disable_firmware_config = false   # global default: STACK (board rules still run)

# Layer rules — FIRST match wins. Host layers are >= 224.
[[layer_rules]]
match = "steam_app*"                       # class-only pattern (board rules also run)
layer = 224

[[layer_rules]]
match = ["cs2", "Counter-Strike 2"]        # [class, title]  == WT(class, title)
layer = 224

# Replace: for this window the host takes over and the board is skipped.
[[layer_rules]]
match = ["*chrome*", "*youtube*"]
layer = 225
disable_firmware_config = true

# Callback rules — ALL matches fire. Names come from DEFINE_HOST_CALLBACKS.
[[callback_rules]]
match = "steam_app*"
enable = ["enable_gaming"]

[[callback_rules]]
match = "*word*"
enable = ["disable_gaming"]
```

After moving a rule, **remove it from `DEFINE_SERIAL_LAYERS` /
`DEFINE_SERIAL_COMMANDS`** (keeping a layer rule in both is harmless but
confusing; keeping a callback rule in both fires it twice). See the
[QMK Integration Guide]({{ site.baseurl }}/qmk-integration) for the full 4-step
procedure, and run `qmkonnect --validate-rules` after editing.

### What happens at runtime

The stack-vs-replace decision is made **per window**: a window is *replace* only
when **every** rule that matches it is disabling — a single non-disabling matched
rule makes the whole window *stack* (the board's rules run, then the host layer
applies on top).

- **A Steam game** (`steam_app_*`) → **stack**. The board runs its own
  `DEFINE_SERIAL_*` rules for that window, the host layer `224` applies **on top**,
  and the host `enable_gaming` callback fires after the board callbacks.
- **YouTube in Chrome** (`*chrome*` + `*youtube*`) → **replace** (`disable_firmware_config = true`).
  No window string is sent, so the board can't match; the firmware clears its own
  board layer/command and applies only the host layer `225`.
- **MS Word** (`*word*`) → only a callback rule matches (no layer rule), so the
  host layer is unchanged and the board runs normally; the host `disable_gaming`
  callback fires. If **no** rule matches a window, the host layer is cleared and
  all host callbacks are disabled.

Run `qmkonnect --list-callbacks` to see your keyboard's real callback names, and
see the [Configuration Guide]({{ site.baseurl }}/configuration) for every field.

## Pattern Matching Tips

### Understanding Window Matching

- **Wildcards**: Use `*` to match partial strings
  - `"*chrome*"` matches Google Chrome, Chromium, Chrome Canary
  - `"steam_app*"` matches any Steam game
  
- **Window Title Matching**: Use `WT(class, title)` for specific windows
  - `WT("*chrome*", "*youtube*")` - YouTube in Chrome
  - `WT("*code*", "*.rs")` - Rust files in VS Code

### Common Patterns

```c
// Match any calculator app
{ "*calc*", _NUMPAD }

// Match specific browser + website
{ WT("*chrome*", "*github*"), _CODING }

// Match file manager
{ "*nautilus*", _FILES }
{ "*finder*", _FILES }   
{ "*explorer*", _FILES }

// Match any terminal
{ "*term*", _TERMINAL }

// Match specific game by exact title
{ WT("cs2", "Counter-Strike 2"), _GAMING }
```

## Testing Your Configuration

1. **Compile and flash** your firmware
2. **Install QMKonnect** — for a single standard keyboard it needs no IDs (auto-discovery); set vendor/product IDs only to disambiguate among multiple QMK keyboards
3. **Test layer switching** by switching between applications
4. **Check QMK console** output for debugging:
   ```bash
   qmk console
   ```

---

## Next Steps

- [Learn about troubleshooting]({{ site.baseurl }}/troubleshooting)
- [Contribute examples](https://github.com/dabstractor/qmkonnect)
