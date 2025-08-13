---
layout: default
title: Examples
permalink: /examples/
---

# Real-World Examples

Complete examples showing how to configure QMKonnect and QMK firmware for different use cases.

## Example 1: Developer Setup

**Scenario**: A developer who uses VS Code, Terminal, and browsers, wanting different layouts for each context.

### QMK Keymap Configuration

```c
#include QMK_KEYBOARD_H
#include "raw_hid.h"
#include "./lib/qmk-notifier/notifier.h"

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
#include "raw_hid.h"
#include "./lib/qmk-notifier/notifier.h"

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
#include "raw_hid.h" 
#include "./lib/qmk-notifier/notifier.h"

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
2. **Configure QMKonnect** with your keyboard's vendor/product IDs
3. **Test layer switching** by switching between applications
4. **Check QMK console** output for debugging:
   ```bash
   qmk console
   ```

---

## Next Steps

- [Learn about troubleshooting]({{ site.baseurl }}/troubleshooting)
- [Contribute examples](https://github.com/dabstractor/qmkonnect)
