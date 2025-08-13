---
layout: default
icon: fas fa-keyboard
order: 5
---

# QMK Integration Guide

Learn how to integrate QMKonnect with your QMK keyboard firmware to enable layer switching and context-aware features.

## Overview

QMKonnect works with the QMK ecosystem through Raw HID communication:

1. **QMKonnect** (this application) detects window changes
2. **qmk_notifier** library handles Raw HID communication
3. **qmk-notifier** QMK module receives and processes notifications
4. Your **QMK keymap** responds to window change events

## Required Components

### 1. QMK Firmware Module

Add the qmk-notifier module to your QMK firmware:

```bash
# In your QMK keymap directory
git submodule add https://github.com/dabstractor/qmk-notifier.git lib/qmk-notifier
```

### 2. Enable Raw HID

In your `rules.mk`:

```make
# Enable Raw HID support
RAW_ENABLE = yes

# Include the notifier module
SRC += lib/qmk-notifier/qmk_notifier.c
```

### 3. Configure Keyboard IDs

In your `config.h`, ensure your keyboard has unique IDs:

```c
#define VENDOR_ID    0xFEED  // Your vendor ID
#define PRODUCT_ID   0x0001  // Your product ID
#define DEVICE_VER   0x0001  // Device version

// Enable Raw HID
#define RAW_USAGE_PAGE 0xFF60
#define RAW_USAGE_ID   0x61
```

## Correct Integration Method

The recommended way to integrate with QMKonnect uses the framework's macros:

### keymap.c
```c
#include QMK_KEYBOARD_H
#include "raw_hid.h"
#include "./qmk-notifier/notifier.h"

void raw_hid_receive(uint8_t *data, uint8_t length) {
    hid_notify(data, length);
}

#define _QWERTY      0
#define _NEOVIM      1
#define _BROWSER     2
#define _TERMINAL    3
#define _JITSI       4
#define _CLICKUP     5
#define _MATTERHORN  6
#define _INKSCAPE    7
#define _GAMING      8

// Your keymap definitions here...

DEFINE_SERIAL_COMMANDS({
    { "neovide", &disable_vim_mode },
    { "alacritty", &disable_vim_mode },
    { "*iterm*", &disable_vim_mode },
    { "*claude*", &vim_lazy_insert, &disable_vim_mode },
    { WT("*chrome*", "*claude*"), &vim_insert, &disable_vim_mode },
    { WT("*chrome*", "*chatgpt*"), &vim_insert, &disable_vim_mode },
    { WT("*chrome*", "*deepseek*"), &vim_insert, &disable_vim_mode },
    { WT("*chrome*", "*gemini*"), &vim_insert, &disable_vim_mode },
    { WT("*brave*", "*claude*"), &vim_insert, &disable_vim_mode },
    { WT("*brave*", "*chatgpt*"), &vim_insert, &disable_vim_mode },
    { WT("*brave*", "*deepseek*"), &vim_insert, &disable_vim_mode },
    { WT("*brave*", "gemini*"), &vim_insert, &disable_vim_mode },
    { WT("*brave*", "*ai*studio*"), &vim_insert, &disable_vim_mode },
    { WT("*", "*orderlands*"), &disable_vim_mode },
    { WT("steam_app*", "*"), &disable_vim_mode },
    { WT("cs2", "Counter-Strike 2"), &disable_vim_mode },
});

DEFINE_SERIAL_LAYERS({
    { "*calculator", _NUMPAD },
    { WT("*chrome*", "*jitsi*"), _JITSI },
    { WT("alacritty", "terminal"), _TERMINAL },
    { WT("alacritty", "alacritty"), _TERMINAL },
    { "*iterm*", _TERMINAL },
    { WT("*alacritty*", "*matterhorn*"), _MATTERHORN },
    { "*chrome*", _BROWSER },
    { "*brave*", _BROWSER },
    { WT("org.gnome.Nautilus", "*"), _BROWSER },
    { "inkscape", _INKSCAPE },
    { WT("steam_app*", "*"), _GAMING },
});
```

## Framework Elements

The framework provides these essential macros and helpers:

- **`DEFINE_SERIAL_LAYERS`**: Maps window patterns to keyboard layers
- **`DEFINE_SERIAL_COMMANDS`**: Maps window patterns to command functions  
- **`WT(class, title)`**: Helper macro to match both window class and title
- **Wildcard matching**: Use `*` for pattern matching (e.g., `"*chrome*"`)

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

// Match specific game by both class and title
{ WT("cs2", "Counter-Strike 2"), _GAMING }
```

## Testing Your Integration

### 1. Verify Raw HID

```bash
# Test if keyboard accepts Raw HID
qmkonnect --test-connection
```

### 2. Debug Mode

```bash
# See what data is being sent
qmkonnect --debug
```

### 3. QMK Console

Enable console output in your QMK firmware:

```c
#ifdef CONSOLE_ENABLE
void qmk_notifier_notify(const char* app_class, const char* window_title) {
    printf("App: %s, Title: %s\n", app_class, window_title);
    // Your handling code here
}
#endif
```

## Troubleshooting

### Common Issues

1. **No communication with keyboard**:
   - Verify Raw HID is enabled in `rules.mk`
   - Check vendor/product IDs match
   - Ensure qmk-notifier module is included

2. **Notifications not received**:
   - Verify `qmk_notifier_notify()` function is implemented
   - Check QMK console output
   - Test with simple debug prints

3. **Layer switching not working**:
   - Verify layer definitions
   - Check layer switching logic
   - Use `layer_state_set_user()` for debugging

### Debug Tips

```c
// Add debug output to your keymap
void qmk_notifier_notify(const char* app_class, const char* window_title) {
    #ifdef CONSOLE_ENABLE
    printf("Received: app='%s', title='%s'\n", app_class, window_title);
    printf("Current layer: %d\n", get_highest_layer(layer_state));
    #endif
    
    // Your notification handling
}
```

## Performance Considerations

- Keep notification handlers lightweight
- Avoid blocking operations in callbacks
- Use layer overlays instead of full layer switches when possible
- Cache frequently used strings to avoid repeated comparisons