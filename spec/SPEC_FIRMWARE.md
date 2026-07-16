# SPEC — Firmware Integration (qmk-notifier module)

> Companion to `PRD.md` / `SPEC_PROTOCOL.md`. The **keyboard-side** contract: the
> `qmk-notifier` C module (companion repo `dabstractor/qmk-notifier`), how a
> user's keymap integrates it, the pattern-matching syntax, and the reference
> keymap this PRD was validated against. QMKonnect the desktop app does **not**
> implement any of this — it is documented here so a dev agent understands the
> complete end-to-end system and the strings the desktop must produce.

---

## 1. The Module at a Glance

`qmk-notifier` (hyphen) is a QMK **module** (a git submodule under a keyboard
directory). It provides:
- **`notifier.c`** — receives Raw HID, validates the magic header, reassembles
  multi-report messages, sanitizes, and dispatches to the user's maps.
- **`notifier.h`** — the `command_map_t` / `layer_map_t` structs and the
  `DEFINE_SERIAL_COMMANDS` / `DEFINE_SERIAL_LAYERS` / `WT(...)` macros.
- **`pattern_match.c/.h`** — wildcard + anchor + escape-sequence matcher.
- **`rules.mk`** — the single line that wires it in:
  ```make
  RAW_ENABLE = yes
  SRC += qmk-notifier/notifier.c
  ```

The module is **coexistence-safe**: it inspects only messages beginning with the
magic bytes `0x81 0x9F` and ignores everything else, so other Raw HID modules
(e.g. `qmk-field-kit`) can share the same interface.

---

## 2. Integration Steps (the user's keymap)

### Step 1 — add the submodule
```bash
cd <qmk_firmware>/keyboards/<your_keyboard>   # e.g. handwired/dactyl_manuform/5x7_1
git submodule add https://github.com/dabstractor/qmk-notifier.git qmk-notifier
```

### Step 2 — include the module's `rules.mk` (in your keymap's `rules.mk`)
```make
include keyboards/handwired/<manufacturer>/<keyboard>/qmk-notifier/rules.mk
```
That single line enables `RAW_ENABLE` **and** compiles `notifier.c`. Do **not**
hand-write `SRC += lib/...` or point at a non-existent `qmk_notifier.c` — that
fails to link.

> The reference keymap (`keyboards/handwired/dactyl_manuform/5x7_1/rules.mk`)
> pulls in three modules this way:
> ```make
> include keyboards/handwired/dactyl_manuform/5x7_1/qmk-vim/rules.mk
> include keyboards/handwired/dactyl_manuform/5x7_1/qmk-notifier/rules.mk
> include keyboards/handwired/dactyl_manuform/5x7_1/qmk-field-kit/rules.mk
> SERIAL_DRIVER = vendor
> ```
> (The `field_kit_process_message` call in `raw_hid_receive` below is from
> qmk-field-kit — a separate module sharing the interface.)

### Step 3 — wire `raw_hid_receive` (in your `keymap.c`)
```c
#include QMK_KEYBOARD_H
#include "./qmk-notifier/notifier.h"

void raw_hid_receive(uint8_t *data, uint8_t length) {
    hid_notify(data, length);   // qmk-notifier entry point
    // (other Raw HID modules can be tried first/after; qmk-notifier
    //  ignores anything not starting with 0x81 0x9F)
}
```

The reference keymap does both field-kit and notifier:
```c
void raw_hid_receive(uint8_t *data, uint8_t length) {
    field_kit_process_message(data, length);
    hid_notify(data, length);
}
```

### Step 4 — define your rules (anywhere `#include`-d from `keymap.c`)

Using the two macros (full syntax in §3):
```c
DEFINE_SERIAL_LAYERS({
    { "*calculator",           _NUMPAD },
    { WT("*chrome*", "*jitsi*"), _JITSI },
    { WT("tty$", "^terminal$"),  _TERMINAL },
    { "steam_app*",            _GAMING },
});
DEFINE_SERIAL_COMMANDS({
    { "neovide", &disable_vim },
    { WT("*chrome*", "*claude*"), &vim_lazy_insert, &disable_vim },
});
```

### Step 5 — build & flash
```bash
qmk compile -kb <your_keyboard> -km <your_keymap>
qmk flash   -kb <your_keyboard> -km <your_keymap>
```
**QMKonnect cannot communicate with the keyboard until this firmware is flashed.**

---

## 3. The Module API (macros & structs)

From `notifier.h`:
```c
typedef void (*callback_t)(void);

typedef struct {
    const char *pattern;
    callback_t on_enable;
    callback_t on_disable;      // may be NULL
    const bool case_sensitive;
} command_map_t;

typedef struct {
    const char *pattern;
    const int layer;
    const bool case_sensitive;
} layer_map_t;

#define GS_DELIMITER      "\x1D"                 // ASCII 31 (Group Separator)
#define ETX_TERMINATOR    "\x03"                 // ASCII 3 (End of Text)
#define WINDOW_TITLE(classname, title)  classname GS_DELIMITER title
#define WT(...) WINDOW_TITLE(__VA_ARGS__)

#define DEFINE_SERIAL_COMMANDS(...)   /* defines user_command_map[] + getters */
#define DEFINE_SERIAL_LAYERS(...)     /* defines user_layer_map[] + getters   */
```

### 3.1 `DEFINE_SERIAL_LAYERS({ … })`
An array of `{ pattern, layer, case_sensitive }`. On a match, the matched layer
is activated (the previously-activated notifier layer is deactivated first, so
only one notifier layer is active at a time).

### 3.2 `DEFINE_SERIAL_COMMANDS({ … })`
An array of `{ pattern, on_enable, on_disable, case_sensitive }`. The 4th field
(`case_sensitive`) is **optional** in the layer macro but the command struct
declares it; the example keymaps omit it in some rows (aggregate-init zero-fills
→ `false`/NULL). On a match, `on_enable()` runs; the previous command's
`on_disable()` (if any) runs first. `on_disable` may be `NULL`.

### 3.3 `WT(class, title)` / `WINDOW_TITLE(class, title)`
Expands to the literal `class "\x1D" title` — i.e. a pattern containing the GS
delimiter. The matcher then requires **both** halves to match against the
class and title respectively. A bare pattern (no `WT`) matches only the
`application_class` part of the message.

---

## 4. Pattern-Matching Syntax (`pattern_match.c`)

`bool pattern_match(const char *pattern, const char *str, bool case_sensitive)`:

| Construct | Meaning |
|---|---|
| `*` | Wildcard — any sequence (including empty). Combinable with anchors. |
| `^` at start | Anchor to the beginning of the string. |
| `$` at end | Anchor to the end of the string. |
| `^…$` together | Exact full-string match. |
| `\^` `\$` `\*` `\\` | Literal escaped character. |
| `\d \D` | Digit / non-digit. |
| `\w \W` | Word char / non-word char. |
| `\s \S` | Whitespace / non-whitespace. |
| `\b \B` | Word boundary / non-boundary. |
| `.` | Any char except newline. |

No anchors ⇒ **substring** match (backward-compatible). Case sensitivity is per
-row (the `case_sensitive` field).

### 4.1 The delimiter-aware matcher (`match_pattern` in `notifier.c`)
- If the **pattern** has a GS delimiter but the **message** doesn't (or vice
  versa), matching is done on the appropriate side only.
- If both have it, both halves must match (`pattern_left` vs `msg_left` AND
  `pattern_right` vs `msg_right`).
- First-match-wins in each map (scan order = definition order).

---

## 5. Firmware Reception Flow (`hid_notify` → `process_full_message`)

This is what runs on the MCU for every Raw HID report QMKonnect sends (full
byte detail in `SPEC_PROTOCOL.md` §5):

1. **Guard:** `length < 2 || data[0] != 0x81 || data[1] != 0x9F` ⇒ discard.
2. Strip the 2 header bytes; iterate the rest into the static 256-byte
   `msg_buffer` until an **ETX** (`0x03`):
   - On ETX: NUL-terminate, `sanitize_string` (strip non-ASCII/non-essential),
     reset index, call `process_full_message(buffer)`, break.
   - On overflow (`msg_index >= 255`): reset index (drop the message).
3. `process_full_message`:
   - Always `disable_command()` first (run the previous command's `on_disable`).
   - Scan `command_map` (first match) → remember; scan `layer_map` (first match)
     → remember.
   - `deactivate_layer()` (the previous notifier layer).
   - If a command matched: `enable_command(cmd)`.
   - If a layer matched: `activate_layer(layer)` (`layer_on`).
4. **Ack:** `raw_hid_send(response[32])` with `response[0] = (match ? 1 : 0)`.
   (QMK silently drops this today due to the `length == RAW_EPSIZE` guard — see
   `SPEC_PROTOCOL.md` §2.5.)

**Key invariants:**
- Only **one** notifier layer is active at a time (the previous is always
  deactivated before a new one activates, and an unmatched message deactivates).
- `sanitize_string` keeps only printable ASCII (32–126) plus tab/newline/CR/GS/ETX.
  So any non-ASCII in a window title (emoji, accented chars) is stripped before
  matching — patterns should be ASCII.

---

## 6. The Reference Keymap (validated against this PRD)

A real-world keymap — the maintainer's Dactyl-Manuform 5×7 (RP2040, split,
`SERIAL_DRIVER = vendor`), in a `<keyboard>/keymaps/default/` directory alongside
a `serial_command.c` — is the canonical example of both macros in real use:

```c
DEFINE_SERIAL_COMMANDS({
    { "neovide", &disable_vim },
    { WT("*tty$", "^terminal$"), &disable_vim },
    { WT("*tty$", "*tty"), &disable_vim },
    { "*iterm*", &disable_vim },
    { WT("^Claude$", "^Claude$"), &vim_lazy_insert, &disable_vim }, // claude desktop
    { WT("*chrome*", "*claude*"), &vim_lazy_insert, &disable_vim }, // claude.ai
    { WT("*chrome*", "*chatgpt*"), &vim_lazy_insert, &disable_vim },
    { WT("*chrome*", "*deepseek*"), &vim_lazy_insert, &disable_vim },
    { WT("*chrome*", "*gemini*"), &vim_lazy_insert, &disable_vim },
    { WT("^brave-browser$", "*Claude - Brave$"), &vim_lazy_insert, &disable_vim },
    { WT("^brave-browser$", "*ChatGPT - Brave$"), &vim_lazy_insert, &disable_vim },
    { WT("^brave-browser$", "*Deepseek - Brave$"), &vim_lazy_insert, &disable_vim },
    { WT("^brave-browser$", "gemini*"), &vim_lazy_insert, &disable_vim },
    { WT("^brave-browser$", "*ai*studio*"), &vim_lazy_insert, &disable_vim },
    { WT("^brave-browser$", "^zoho mail"), &vim_lazy_insert, &disable_vim },
    { "Mulletware Wiki", &vim_lazy_insert, &disable_vim },
    { WT("*", "*orderlands*"), &disable_vim },
    { WT("steam_app*", "*"), &disable_vim },
    { WT("cs2", "Counter-Strike 2"), &disable_vim },
});

DEFINE_SERIAL_LAYERS({
    { "*calculator", _NUMPAD },
    { WT("*chrome*", "*jitsi*"), _JITSI },
    { WT("tty$", "^terminal$"), _TERMINAL },
    { WT("tty$", "tty"), _TERMINAL },
    { "*iterm*", _TERMINAL },
    { WT("*alacritty*", "*matterhorn*"), _MATTERHORN },
    { "*clickup*", _CLICKUP },
    { "*neovide*", _NEOVIM },
    { "chrome*", _BROWSER },
    { WT("brave-browser", "*"), _BROWSER },
    { WT("firefox", "*"), _BROWSER },
    { WT("org.gnome.Nautilus", "*"), _BROWSER },
    { "*inkscape*", _INKSCAPE },
    { "blender", _BLENDER },
    { "borderlands*", _GAMING },
    { WT("steam_app*", "*orderlands*"), _GAMING },
    { "steam_app*", _GAMING },
    { WT("cs2", "Counter-Strike 2"), _GAMING },
});
```

**What this demonstrates** (and what the desktop app must therefore produce):
- Bare patterns match the **class** alone: `"chrome*"`, `"blender"`,
  `"*neovide*"`, `"steam_app*"`.
- `WT(class, title)` matches both: `WT("brave-browser", "*Claude - Brave$")`.
- Anchors for precision: `WT("^Claude$", "^Claude$")` (exact class+title for the
  Claude desktop app, so a browser tab titled "Claude" doesn't trip it).
- Case sensitivity is off by default (`"Counter-Strike 2"` matches case-insensitively).
- Commands and layers are **independent** — a window can match both a command
  (toggle vim) and a layer (switch keymap) simultaneously.

### 6.1 Hardware (`keyboard.json`)
Dactyl-Manuform 5×7-1, manufacturer `dabstractor`, MCU RP2040 (`bootloader
rp2040`), split with `SERIAL_DRIVER = vendor`, features include `raw_hid`,
`encoder_map`, `tri_layer`, `caps_word`, `leader`, `nkro`, `os_detection`,
`console`. The user also has `qmk-vim` and `qmk-field-kit` modules integrated.

---

## 7. Desktop ↔ Firmware Contract Summary

For QMKonnect to drive this keymap, it must, on every focus change, send a Raw
HID burst whose reassembled logical message is exactly:

```
0x81 0x9F  <class bytes…>  0x1D  <title bytes…>  0x03
```

where `<class>` is one of the strings the keymap matches (`neovide`, `firefox`,
`brave-browser`, `steam_app12345`, …) and `<title>` is the window title (or
empty). Everything in `SPEC_PROTOCOL.md` exists to produce exactly that. The
"Show Window Information" dialog (`SPEC_UI.md` §3) exists so users can see the
exact `<class>`/`<title>` to put in their `DEFINE_SERIAL_*` rules.

---

## 8. Debugging the Firmware Side
- No built-in debug callback. Add your own `printf` inside a callback (or
  temporarily inside `hid_notify`) with `CONSOLE_ENABLE = yes`, then `qmk console`.
- On the desktop side, `qmkonnect -v` prints the sanitized payload (`\x1D`
  shown as `|`) and send timing, confirming what's on the wire.

---

*This concludes the specification set. Return to `PRD.md` for the product-level
overview and the document map.*
