---
layout: default
title: QMK Integration
permalink: /qmk-integration/
---

# QMK Integration Guide

**This step is required.** QMKonnect only *sends* window information to your
keyboard over Raw HID. Nothing will happen on the keyboard side until the
[**qmk_notifier**](https://github.com/dabstractor/qmk_notifier) module is built
into your firmware and you've defined how your keyboard should respond.

> The instructions below mirror the upstream
> [qmk_notifier README](https://github.com/dabstractor/qmk_notifier). That repo
> is the source of truth for the firmware module; this page ties it to QMKonnect.

## Overview

QMKonnect works with the QMK ecosystem through Raw HID communication:

1. **QMKonnect** (this application) detects the active window and sends
   `{application_class}{GS}{window_title}` (where `{GS}` is the Group Separator,
   `0x1D).
2. **qmk_notifier** (a QMK module in your firmware) receives the message, checks
   for its `0x81 0x9F` header, reassembles strings longer than the 32-byte HID
   packet limit, and pattern-matches the result against your rules.
3. Your **QMK keymap** switches layers and/or runs callbacks via the
   `DEFINE_SERIAL_LAYERS` / `DEFINE_SERIAL_COMMANDS` macros the module provides.

The QMKonnect desktop app and the qmk_notifier firmware module are **companion
projects** — you need both.

## Integration Steps

### Step 1: Add qmk_notifier as a submodule to your keymap

```bash
cd /path/to/qmk_firmware/keyboards/your_keyboard
git submodule add https://github.com/dabstractor/qmk_notifier.git
```

This clones the module into `qmk_notifier/` inside your keyboard directory.

### Step 2: Include the module in your `rules.mk`

Add the module's own `rules.mk` to your keymap's `rules.mk`. The path is relative
to the `qmk_firmware` root:

```make
include keyboards/handwired/[manufacturer]/[keyboard_name]/qmk_notifier/rules.mk
```

That single line is what actually wires the module up. The included
[`rules.mk`](https://github.com/dabstractor/qmk_notifier/blob/main/rules.mk)
contains:

```make
RAW_ENABLE = yes
SRC += qmk_notifier/notifier.c
```

So it enables Raw HID **and** compiles `notifier.c` for you. Do **not** add
`SRC += ... qmk-notifier.c` yourself — that source file does not exist, and
duplicating `SRC += qmk_notifier/notifier.c` will cause a double-definition
error.

### Step 3: Include the module in your `keymap.c`

```c
#include QMK_KEYBOARD_H
#include "qmk_notifier/notifier.h"

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
the upstream qmk_notifier docs for the full
[pattern matching syntax](https://github.com/dabstractor/qmk_notifier#pattern-matching-syntax)
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
`0xFF60` / `0x61`, which is exactly what both qmk_notifier and QMKonnect expect,
and what QMKonnect auto-discovers. Only set `RAW_USAGE_PAGE` / `RAW_USAGE_ID` in
your `config.h` if your firmware deliberately overrides them — and in that case
you must tell the QMKonnect desktop app the matching values (see the
[Configuration Guide]({{ site.baseurl }}/configuration#configuration-reference)).

## Host-Side Rules (Optional): Moving Window Rules to the Desktop

So far this guide covers the **board-side** rules — `DEFINE_SERIAL_LAYERS` and
`DEFINE_SERIAL_COMMANDS`, baked into your firmware. QMKonnect can also drive
layer and callback decisions **from the host**, in an editable `rules.toml`
file, so you can change them **without reflashing**. Host rules are an opt-in
overlay: your existing board rules keep working, and host rules either stack on
top of them or take over per window (see [Stack vs. replace](#stack-vs-replace)
below).

> **Firmware prerequisite.** Host rules require firmware that advertises the
typed-command capability (`proto_ver == 2` with the `APPLY_HOST_CONTEXT`
feature). With legacy firmware — or while the keyboard is disconnected —
QMKonnect silently falls back to today's string-only behavior, and your
board's existing `DEFINE_*` rules keep working exactly as before. Nothing
breaks if you don't opt in.

### The three repositories

Host rules span three companion repos. You only ever edit two of them (your
firmware keymap, and `rules.toml` on the host):

| Repo | What it is | Its job for host rules |
| --- | --- | --- |
| [`qmkonnect`](https://github.com/dabstractor/qmkonnect) | The desktop daemon (this project) | Detects the window; owns `rules.toml`, the host matcher, the capability handshake, and the send sequencing |
| [`qmk-notifier`](https://github.com/dabstractor/qmk-notifier) *(hyphen)* | The Rust transport crate QMKonnect links | Frames the typed commands over Raw HID and parses the replies |
| [`qmk_notifier`](https://github.com/dabstractor/qmk_notifier) *(underscore)* | The C firmware module in your keymap | Receives typed commands, tracks a separate host layer/callback set, and exposes your callbacks by name |

> **Naming hazard:** `qmk_notifier` (underscore) is the firmware C module;
`qmk-notifier` (hyphen) is the Rust transport crate. They talk over the
fixed Raw HID wire protocol described in the
[qmk_notifier README](https://github.com/dabstractor/qmk_notifier).

### Expose callbacks by name: `DEFINE_HOST_CALLBACKS` (one-time firmware change)

Host rules reference your callbacks **by name** (a string), not by pointer. So
that the host can look them up, you declare a small named registry in your
firmware. This is a **one-time** change — add it once, reflash once, and you
never touch it again when iterating on rules.

Each row is `{ name, on_enable, on_disable }`, where `on_disable` may be `NULL`.
The callback's `id` is its **array index** (stable per build); QMKonnect queries
the names at every (re)connect, so renumbering across flashes is harmless. It
needs **no `rules.mk` change** — define it anywhere `#include`-d from
`keymap.c`, just like `DEFINE_SERIAL_COMMANDS`:

```c
static void mute_on(void)  { /* unmute / show mute OSD */ }
static void mute_off(void) { /* restore */ }

DEFINE_HOST_CALLBACKS({
    { "mute", &mute_on, &mute_off },
});
```

These are the **same C functions** you already pass to `DEFINE_SERIAL_COMMANDS`
— you're just listing them by name so the host can address them. Omit the macro
entirely and the firmware provides empty defaults (`callback_count == 0`), the
feature stays off, and your keymap behaves byte-for-byte as it does today.

### Migration: from `DEFINE_*` to `rules.toml`

Migration is **incremental and optional** — move one rule at a time, or none at
all. Board rules keep working throughout.

1. **Expose your callbacks by name** — add `DEFINE_HOST_CALLBACKS({ … })`
   (above) listing the functions you already use in `DEFINE_SERIAL_COMMANDS`.
   Reflash **once**. (This is the only step that ever requires a reflash.)
2. **Move a layer rule to the host** — add a `[[layer_rules]]` entry to
   `rules.toml`, then **remove** the matching row from `DEFINE_SERIAL_LAYERS`.
   (Keeping it in both isn't harmful, but it means the same layer is driven by
   two trackers at once, which is confusing.) No reflash needed for this or any
   later edit.
3. **Move a callback rule to the host** — add a `[[callback_rules]]` entry,
   then **remove** the matching row from `DEFINE_SERIAL_COMMANDS`. Here removal
   matters: callbacks are additive, so if a rule stays in both, the same
   `on_enable` would fire twice.
4. **Iterate without reflashing** — edit `rules.toml` and click the tray's
   **Reload rules** (or restart QMKonnect). Every future rule change is a host
   edit, no firmware rebuild.

For example, the firmware callback rule
`{ WT("steam_app*", "*"), &disable_vim }` (already listed in
`DEFINE_HOST_CALLBACKS` as `{ "disable_vim", &disable_vim, NULL }`) becomes a
host rule:

```toml
[[callback_rules]]
match = ["steam_app*", "*"]        # [class, title]  == WT(class, title)
enable = ["disable_vim"]
```

The full `rules.toml` schema (every field, the layer range, the `match`
string-vs-array forms), the per-OS file location, and the
`--validate-rules` / `--list-callbacks` / `--rules-path` CLI flags are
documented in the [Configuration Guide]({{ site.baseurl }}/configuration).

### Stack vs. replace

The firmware keeps **two independent state planes**: your **board** layer and
callbacks (driven by the legacy string + `DEFINE_*`, exactly as today) and a
separate **host** layer and callback set (driven by `rules.toml`). A host layer
is a **raw QMK layer index** (no reserved range): pick one above your highest
board layer so it wins in stack mode, within your `layer_state` width (≤15 by
default, ≤31 with `LAYER_STATE_32BIT`); `255` clears the host layer. The two
planes touch only at two seams, and QMKonnect
picks which one per window:

- **Stack** (the default): QMKonnect sends the window **string first** — so your
  board runs its own `DEFINE_*` rules as usual — and then applies the host
  layer **on top** and syncs the host callbacks. Board callbacks fire first,
  then host callbacks.
- **Replace**: for a window where you want the host to fully take over,
  QMKonnect sends **only** the host context (no string, so the board can't
  match). The firmware clears its board layer/command for that window and
  applies only the host layer + callbacks. Board rules re-engage normally on
  the host's next string send.

Which mode a window uses is decided by each rule's `disable_firmware_config`
flag in `rules.toml`: a window is *replace* only when **every** rule that
matches it is disabling (or the board has no rules of its own); if even one
matched rule is non-disabling, the window *stacks*. See the
[Configuration Guide]({{ site.baseurl }}/configuration) for the field-level
detail. If no host rule matches, only the host layer is cleared and host
callbacks disabled — the board's own rules still run (host and board are
independent silos; there is no "keep" option).

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
    printf("qmk_notifier: disabling vim\n");
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
   `include .../qmk_notifier/rules.mk` line in your `rules.mk` (Step 2). Do not
   hand-write `SRC += lib/...` or point at a non-existent `qmk-notifier.c`.
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
- [Move rules to the host (optional)]({{ site.baseurl }}/qmk-integration#host-side-rules-optional-moving-window-rules-to-the-desktop)
