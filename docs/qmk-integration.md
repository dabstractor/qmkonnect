---
layout: default
title: QMK Integration
permalink: /qmk-integration/
---

# QMK Integration Guide

**This step is required.** QMKonnect only *sends* window information to your
keyboard over Raw HID. Nothing will happen on the keyboard side until the
[**qmk-notifier**](https://github.com/dabstractor/qmk-notifier) module is built
into your firmware and you've defined how your keyboard should respond.

> The instructions below mirror the upstream
> [qmk-notifier README](https://github.com/dabstractor/qmk-notifier). That repo
> is the source of truth for the firmware module; this page ties it to QMKonnect.

## Overview

QMKonnect works with the QMK ecosystem through Raw HID communication:

1. **QMKonnect** (this application) detects the active window and sends
   `{application_class}{GS}{window_title}` (where `{GS}` is the Group Separator,
   `0x1D).
2. **qmk-notifier** (a QMK module in your firmware) receives the message, checks
   for its `0x81 0x9F` header, reassembles strings longer than the 32-byte HID
   packet limit, and pattern-matches the result against your rules.
3. Your **QMK keymap** switches layers and/or runs callbacks via the
   `DEFINE_SERIAL_LAYERS` / `DEFINE_SERIAL_COMMANDS` macros the module provides.

The QMKonnect desktop app and the qmk-notifier firmware module are **companion
projects** — you need both.

## Integration Steps

### Step 1: Add qmk-notifier as a submodule to your keymap

```bash
cd /path/to/qmk_firmware/keyboards/your_keyboard
git submodule add https://github.com/dabstractor/qmk-notifier.git
```

This clones the module into `qmk-notifier/` inside your keyboard directory.

### Step 2: Include the module in your `rules.mk`

Add the module's own `rules.mk` to your keymap's `rules.mk`. The path is relative
to the `qmk_firmware` root:

```make
include keyboards/handwired/[manufacturer]/[keyboard_name]/qmk-notifier/rules.mk
```

That single line is what actually wires the module up. The included
[`rules.mk`](https://github.com/dabstractor/qmk-notifier/blob/main/rules.mk)
contains:

```make
RAW_ENABLE = yes
SRC += qmk-notifier/notifier.c
```

So it enables Raw HID **and** compiles `notifier.c` for you. Do **not** add
`SRC += ... qmk_notifier.c` yourself — that source file does not exist, and
duplicating `SRC += qmk-notifier/notifier.c` will cause a double-definition
error.

### Step 3: Include the module in your `keymap.c`

```c
#include QMK_KEYBOARD_H
#include "qmk-notifier/notifier.h"

// Forward Raw HID packets from the host (QMKonnect) into the notifier.
void raw_hid_receive(uint8_t *data, uint8_t length) {
    hid_notify(data, length);
}
```

`QMK_KEYBOARD_H` already pulls in the QMK headers you need (including Raw HID), so
there is no need to `#include "raw_hid.h"` separately.

### Step 4: Define layer and command rules

These two macros are the entire API. Define them anywhere in your keymap (a
dedicated `.c` file `#include`-d from `keymap.c` is a common pattern):

```c
DEFINE_SERIAL_LAYERS({
    { "*calculator",       _NUMPAD },
    { WT("*chrome*", "*jitsi*"), _JITSI },
    { WT("tty$", "^terminal$"),  _TERMINAL },
    { "*iterm*",           _TERMINAL },
    { "chrome*",           _BROWSER },
    { WT("brave-browser", "*"),  _BROWSER },
    { "steam_app*",        _GAMING },
});

DEFINE_SERIAL_COMMANDS({
    { "neovide", &disable_vim },
    { WT("steam_app*", "*"), &disable_vim },
    { WT("*chrome*", "*claude*"), &vim_lazy_insert, &disable_vim },
});
```

See the [Examples]({{ site.baseurl }}/examples) page for complete keymaps, and
the upstream qmk-notifier docs for the full
[pattern matching syntax](https://github.com/dabstractor/qmk-notifier#pattern-matching-syntax)
(`*`, `^`, `$`, and `WT(class, title)`).

### Step 5: Build and flash

```bash
qmk compile -kb your_keyboard -km your_keymap
qmk flash   -kb your_keyboard -km your_keymap
```

**QMKonnect cannot communicate with your keyboard until this firmware is
flashed.**

## About `RAW_USAGE_PAGE` / `RAW_USAGE_ID`

You do **not** normally need to set these. QMK's defaults are already
`0xFF60` / `0x61`, which is exactly what both qmk-notifier and QMKonnect expect,
and what QMKonnect auto-discovers. Only set `RAW_USAGE_PAGE` / `RAW_USAGE_ID` in
your `config.h` if your firmware deliberately overrides them — and in that case
you must tell the QMKonnect desktop app the matching values (see the
[Configuration Guide]({{ site.baseurl }}/configuration#configuration-reference)).

## Testing Your Integration

### Basic verification

1. **Compile and flash** your firmware with the integration above.
2. **Install QMKonnect** — for a single standard QMK keyboard it needs no IDs
   (auto-discovery); see the
   [Configuration Guide]({{ site.baseurl }}/configuration).
3. **Switch between applications** and confirm your layers change / callbacks
   fire.

### Debugging on the keyboard side

The notifier has no built-in debug callback. To see what your keyboard receives,
add your own `printf` inside a callback or temporarily inside `hid_notify`, with
`CONSOLE_ENABLE = yes` in your `rules.mk`:

```c
#ifdef CONSOLE_ENABLE
// Example: log every time the vim-disable callback fires
void disable_vim(void) {
    printf("qmk-notifier: disabling vim\n");
    // ...your real logic...
}
#endif
```

Then watch the QMK console:

```bash
qmk console
```

On the host side, run QMKonnect with verbose/debug logging to confirm it is
sending the expected `{app_class}{GS}{title}` payload (see the
[Troubleshooting Guide]({{ site.baseurl }}/troubleshooting)).

## Common Issues

If your integration isn't working:

1. **Won't compile / `hid_notify` undefined**: you're missing the
   `include .../qmk-notifier/rules.mk` line in your `rules.mk` (Step 2). Do not
   hand-write `SRC += lib/...` or point at a non-existent `qmk_notifier.c`.
2. **No communication**: confirm QMKonnect detects the keyboard
   (`qmkonnect -v` / `--list-devices`), and that Raw HID is enabled (it is, once
   the module's `rules.mk` is included).
3. **No layer switching**: verify your `DEFINE_SERIAL_LAYERS` patterns actually
   match the `{app_class}{GS}{title}` string QMKonnect sends — check it with
   `qmkonnect -v`.
4. **Submodule not present after clone**: run `git submodule update --init`
   in your keyboard directory.

For detailed troubleshooting, see the
[troubleshooting guide]({{ site.baseurl }}/troubleshooting).

---

## Next Steps

- [Configure the QMKonnect desktop app]({{ site.baseurl }}/configuration)
- [Learn how to use QMKonnect]({{ site.baseurl }}/usage)
- [See real-world examples]({{ site.baseurl }}/examples)
