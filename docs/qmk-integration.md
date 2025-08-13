---
layout: default
title: QMK Integration
permalink: /qmk-integration/
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

## Integration Steps

### Step 1: Add the QMK Notifier Module

In your QMK keymap directory:

```bash
git submodule add https://github.com/dabstractor/qmk-notifier.git lib/qmk-notifier
```

### Step 2: Update rules.mk

Add to your keymap's `rules.mk`:

```make
# Enable Raw HID support
RAW_ENABLE = yes

# Include the notifier module
SRC += lib/qmk-notifier/qmk_notifier.c
```

### Step 3: Update config.h

Add to your keymap's `config.h`:

```c
// Enable Raw HID
#define RAW_USAGE_PAGE 0xFF60
#define RAW_USAGE_ID   0x61
```

### Step 4: Basic keymap.c Setup

```c
#include QMK_KEYBOARD_H
#include "raw_hid.h"
#include "./qmk-notifier/notifier.h"

// Forward Raw HID data to the notifier
void raw_hid_receive(uint8_t *data, uint8_t length) {
    hid_notify(data, length);
}

// Define your layers
#define _QWERTY      0
#define _BROWSER     1
#define _TERMINAL    2
#define _GAMING      3
// Add more layers as needed

// Your keymap definitions here...
const uint16_t PROGMEM keymaps[][MATRIX_ROWS][MATRIX_COLS] = {
    // Your layer definitions
};
```

## Configuring Window-Based Behavior

With the qmk-notifier module integrated, you can configure how your keyboard responds to window changes.

For detailed configuration of the `DEFINE_SERIAL_LAYERS` and `DEFINE_SERIAL_COMMANDS` macros, see the [Configuration Guide]({{ site.baseurl }}/configuration#qmk-firmware-configuration).

## Compiling Your Firmware

After adding the qmk-notifier integration:

```bash
# In your QMK firmware directory
qmk compile -kb your_keyboard -km your_keymap

# Flash to keyboard
qmk flash -kb your_keyboard -km your_keymap
```

## Testing Your Integration

### Basic Verification

1. **Compile and flash** your firmware with the integration
2. **Install and configure** QMKonnect with your keyboard's vendor/product IDs
3. **Test communication** by switching between different applications

### Debug Output

Add console output to your keymap for debugging:

```c
#ifdef CONSOLE_ENABLE
void qmk_notifier_notify(const char* app_class, const char* window_title) {
    printf("App: %s, Title: %s\n", app_class, window_title);
    // Your layer switching logic here
}
#endif
```

Then monitor QMK console output:
```bash
qmk console
```

## Common Issues

If your integration isn't working:

1. **No communication**: Check that Raw HID is enabled and vendor/product IDs match
2. **No layer switching**: Verify your layer definitions and DEFINE_SERIAL_LAYERS syntax  
3. **Compilation errors**: Ensure qmk-notifier submodule is properly added and SRC is updated

For detailed troubleshooting, see the [troubleshooting guide]({{ site.baseurl }}/troubleshooting).

---

## Next Steps

- [Configure QMKonnect with your keyboard IDs]({{ site.baseurl }}/configuration)
- [Learn how to use QMKonnect]({{ site.baseurl }}/usage)
- [See real-world examples]({{ site.baseurl }}/examples)
