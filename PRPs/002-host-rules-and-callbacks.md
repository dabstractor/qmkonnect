# PRP: Host-Side Window Rules — Layer Switching & Arbitrary Callbacks

**Status:** Draft / Approved-for-implementation
**Affects repos:** `qmkonnect` (this repo), `qmk_notifier` (Rust crate), `qmk-notifier` (QMK firmware module)
**Predecessors:** none (builds on the existing Raw HID notifier protocol)

---

## Feature Goal

Let users define **app → layer** and **app → callback** rules in an editable
file on their computer (`rules.toml`), with the matching done by QMKonnect on the
host — so rules can change **without reflashing firmware**. Both layer switching
*and* arbitrary firmware callbacks (the existing `command_map`
on_enable/on_disable pattern) are supported.

Crucially, host rules **stack on top of** the keyboard's existing firmware rules
(`DEFINE_SERIAL_LAYERS` / `DEFINE_SERIAL_COMMANDS`): the board's rules always run
first, then host rules apply on top. Nothing existing is removed or deprecated.

## Deliverable

- **`qmk_notifier` crate (v0.3.0):** typed-command framing + response parsing, a
  public pattern-matcher module (ported from the firmware matcher), and new
  `RunCommand` variants.
- **`qmk-notifier` firmware (v0.3.0):** a named callback registry
  (`DEFINE_HOST_CALLBACKS`), a separate host-layer tracker, host-callback enable
  state, typed-command dispatch inside `hid_notify()`, and handlers for
  `QUERY_INFO` / `QUERY_CALLBACK` / `APPLY_HOST_CONTEXT`.
- **`qmkonnect`:** `rules.toml` parsing + validation, host-side rule evaluation,
  a startup capability/name handshake, `notify_qmk` extended to send the host
  context after the legacy string, CLI flags (`--list-callbacks`,
  `--validate-rules`), a "Reload rules" tray item, and per-platform rules-file
  paths.
- **Docs:** updates to `docs/qmk-integration.md`, `docs/configuration.md`,
  `docs/examples.md`, `docs/troubleshooting.md`, `Readme.md`, and a regenerated
  `docs/llms_full.txt`, plus a migration section.

## Success Definition

- A user can add/change a layer or callback rule by editing `rules.toml` and
  clicking "Reload rules" — **no reflash**.
- Board (`DEFINE_*`) rules keep working unchanged; host rules apply on top in the
  documented order (board layer first → host layer on top; board callbacks first
  → host callbacks after).
- Old firmware (current release) + new QMKonnect continues to work in
  string-only mode (graceful fallback via the handshake); no host commands are
  sent to firmware that doesn't advertise support.
- New firmware + old QMKonnect keeps working (old app only sends the legacy
  string; new typed commands are simply never sent).
- All existing tests pass; new unit/integration tests cover matcher parity, the
  state machine, handshake fallback, and wire framing.

## Context

### How it works today

- QMKonnect builds `{app_class}{GS}{title}` (`GS` = `0x1D`) and calls
  `qmk_notifier::run(RunParameters::new(RunCommand::SendMessage(msg), …))`
  (`src/core/notifier.rs:notify_qmk`, `impl Notifier for QmkNotifier`).
- The **`qmk_notifier` crate** (pinned at tag `v0.2.1` in `Cargo.toml`) owns all
  wire framing: it prepends the `0x81 0x9F` header, chunks across 32-byte Raw HID
  reports, appends the `0x03` (ETX) terminator, sends, and reads the response.
- The **`qmk-notifier` firmware** (`notifier.c`) receives in `hid_notify()`:
  - Rejects anything not starting with `0x81 0x9F`.
  - Reassembles the string until ETX, then `process_full_message()`:
    - `command_map` (from `DEFINE_SERIAL_COMMANDS`): first match → call
      `on_enable`; previous command's `on_disable` called first.
    - `layer_map` (from `DEFINE_SERIAL_LAYERS`): first match → `activate_layer()`
      (`layer_on` + tracked); previous layer `deactivate_layer()` (`layer_off`)
      called first.
  - Always replies with a 32-byte report whose `response[0] = matched` (0/1).
- `activate_layer`/`deactivate_layer` wrap QMK's native `layer_on`/`layer_off`
  and track a **single** `activated_layer`.

### The three-repo reality

The wire protocol is shared by three repos and this feature touches all three:

| Repo | Role | Change here |
| --- | --- | --- |
| `qmk_notifier` (Rust crate) | Host-side framing + HID I/O | Typed commands, response parsing, matcher module |
| `qmk-notifier` (firmware C) | On-keyboard receiver/matcher | Registry, host-layer tracker, typed-command dispatch |
| `qmkonnect` (this app) | Window detection + rules | `rules.toml`, host matcher, handshake, sequencing |

### Locked design decisions (from requirements gathering)

- **B1 — Coexistence = stack, not replace.** Board rules always run; host rules
  apply on top. **Board layer first, then host layer on top.** **Board callbacks
  first, then host callbacks.**
- **B2 — Callback identity = firmware registry + startup name query.** Firmware
  declares named callbacks (`DEFINE_HOST_CALLBACKS`); IDs are declaration order;
  QMKonnect queries names at connect and the rules file references callbacks by
  **name**.
- **B3 — "Arbitrary callback" = firmware-registered C functions only** (the
  existing `on_enable`/`on_disable` pattern). Host-side actions (shell/launch)
  and host-driven keyboard macros are **out of scope** for this PRP.
- **C1 — Format: TOML.** C2 — separate `rules.toml` next to `config.toml`.
  C3 — hot-reload (fs watch + tray "Reload rules"). C4 — port the firmware
  matcher to Rust for 1:1 parity, **stable subset only** in v1 (`*`, `^`, `$`,
  two-part `WT`); regex classes (`\d \w \s \b .`) deferred. C5 — protocol
  version/capability handshake with graceful fallback. C6 — VIA coexistence
  included as a **future phase**, not core. C7 — no-match ⇒ clear host
  contributions (configurable `keep`). C8 — **all matching callbacks fire**;
  layers are exclusive (first-match-wins, one host layer on top). C9 — one global
  ruleset for v1 (per-keyboard overrides later).

---

## Architecture

### Per-window-change data flow

```
window focus changes
        │
        ▼
debounce (existing, configurable ms)
        │
        ▼
build string  s = "{app_class}\x1D{title}"        (existing)
        │
        ▼  ① Send STRING_MATCH(s)  ──►  firmware runs BOARD rules:
        │                                 disable prev board cmd, deactivate
        │                                 prev board layer, enable matched
        │                                 board cmd, activate matched board layer
        │  ◄── response[0] = matched(bool)
        ▼
(if host-capable AND rules.toml present)
evaluate host rules against s
   • layer_rules   : first match → L_h  (else none)
   • callback_rules: ALL matches  → desired callback id set
        │
        ▼  ② Send APPLY_HOST_CONTEXT{layer=L_h, callbacks=set}  ──►  firmware:
        │                                 set host_layer (on top of board's),
        │                                 sync host callbacks (enable in-set,
        │                                 disable not-in-set)
        │  ◄── response[0]=0xF0 (typed), ack
        ▼
update host state for next diff/logging
```

**Ordering guarantee (B1):** the legacy string is always sent **first** and the
host context **after** its round-trip completes, so board callbacks finish before
host callbacks run, and the host layer stacks above the board layer.

### Coexistence semantics (precise)

- The **string is always sent**, even when host rules exist, so board
  `DEFINE_*` rules keep firing exactly as today.
- Firmware maintains **two independent layer trackers**:
  - `activated_layer` (board) — managed by the string/matcher path (unchanged).
  - `host_layer` (new) — managed by `APPLY_HOST_CONTEXT`.
  They never touch each other's state, so board deactivate/activate and host
  set/clear are orthogonal. Net effect: host layer sits above board layer.
- Callbacks: board callbacks fire during string processing; host callbacks fire
  during `APPLY_HOST_CONTEXT`. Both run; board first.
- If `rules.toml` is absent or the keyboard is legacy (no host support), only ①
  runs — i.e., today's behavior, bit-for-bit.

---

## Wire Protocol Specification

### Framing

All messages live in the `0x81 0x9F` qmk-notifier namespace. The byte immediately
after the header is a discriminator:

- **`0xF0`** ⇒ **typed command** (new). Layout (single 32-byte report for v1):
  ```
  [0x81][0x9F][0xF0][cmd_id][ args... ][0x03]
  ```
- **anything else** ⇒ **legacy string** (unchanged): bytes are string chars until
  `0x03` (ETX), chunked across reports by the crate.

`0xF0` is chosen because the firmware sanitizer only allows bytes `0x20–0x7E`
plus `0x09/0x0A/0x0D/0x1D/0x03`; `0xF0` can never begin a real matched string, so
the discriminator is unambiguous and **legacy firmware safely ignores typed
commands** (it walks them as string chars, finds no pattern match, replies
`response[0]=0`).

### Responses (32-byte reports)

- **Legacy string response (unchanged):** `[matched(0|1)][padding…]`.
- **Typed response:** `[0x51][cmd_id_echo][payload…][padding]`.

Typed responses use marker `0x51` (never `0`/`1`), so the host distinguishes a
typed ack from a legacy match bool without ambiguity.

> **Note on the response marker:** earlier discussion floated `0xF0` for
> responses too, but legacy responses are already constrained to `0`/`1`, so any
> value `≥2` is safe. `0x51` is used to avoid confusing "request marker" with
> "response marker." (Either works; pick one and keep it consistent — this PRP
> uses `0xF0` request / `0x51` response.)

### Command table

| `cmd_id` | Name | Request args | Response payload (`[0x51][cmd_echo]` then:) |
| --- | --- | --- | --- |
| `0x01` | `QUERY_INFO` | none | `[proto_ver][feature_flags][callback_count][board_rules_present]` |
| `0x02` | `QUERY_CALLBACK` | `[index]` | `[index][name bytes, NUL-padded]` (name absent ⇒ `[index][0x00]`) |
| `0x05` | `APPLY_HOST_CONTEXT` | `[layer][count][id0][id1]…` | `[ack]` (`1`=applied) |

Where:

- `proto_ver` = `2` for this release (`1` = legacy string-only firmware).
- `feature_flags` bitmask: `0x01` = `APPLY_HOST_CONTEXT` supported;
  `0x02` = callback registry present; `0x04` = (reserved) VIA-coexist dispatch.
- `callback_count` = number of entries in the firmware registry (0 if none).
- `board_rules_present` = `1` if the keymap defined `DEFINE_SERIAL_LAYERS`/`…COMMANDS`.
- `APPLY_HOST_CONTEXT.layer`: `0xFF` ⇒ clear host layer; else the layer number.
  `callbacks` = the **full desired enabled set** (ids to ENABLE); the firmware
  diffs against its current enabled set and calls `on_enable`/`on_disable`
  accordingly. (v1 cap: `count ≤ 26` to fit one report — see Risks.)

### Handshake & graceful fallback

1. On device connect, QMKonnect sends `QUERY_INFO`.
2. **New firmware** replies `[0x51][0x01][proto=2][flags][count][…]`.
3. **Legacy firmware** walks the typed bytes as a string, matches nothing,
   replies `[0x00…]` (or times out). QMKonnect treats `response[0] != 0x51` (or
   timeout) as **legacy ⇒ string-only mode**: it continues to send ① the string
   (so board rules still work) and **never sends host commands**.
4. If new + `flags & APPLY_HOST_CONTEXT`: for `i in 0..count` send
   `QUERY_CALLBACK(i)` and build the **name→id** map; validate `rules.toml`
   callback names against it (warn + skip unknown).

> **Handshake timing constraint (correctness gotcha).** Against **legacy**
> firmware, `QUERY_INFO` is walked as a no-match string, and `process_full_message`
> *always* calls `deactivate_layer()`/`disable_command()` before failing to match.
> This is harmless **only when firmware state is fresh** — i.e. at (re)connect,
> where `activated_layer == LAYER_UNSET` and `current_command == NULL`, so both
> are no-ops. **Therefore: run the handshake exclusively at (re)connect, never as
> a periodic poll.** New firmware has no such side effect (typed commands bypass
> `process_full_message` entirely), so this constraint only protects the legacy
> fallback path.

---

## Firmware Spec (`qmk-notifier`)

### 1. Named callback registry

Add to `notifier.h`:

```c
typedef struct {
    const char   *name;
    callback_t    on_enable;   // may be NULL
    callback_t    on_disable;  // may be NULL
} host_callback_t;

host_callback_t* get_host_callbacks(void);
size_t           get_host_callbacks_size(void);

#define DEFINE_HOST_CALLBACKS(...) \
    host_callback_t user_host_callbacks[] = __VA_ARGS__; \
    const size_t user_host_callbacks_size = \
        sizeof(user_host_callbacks) / sizeof(user_host_callbacks[0]); \
    host_callback_t* get_host_callbacks(void) { return user_host_callbacks; } \
    size_t get_host_callbacks_size(void) { return user_host_callbacks_size; }
```

`notifier.c` adds weak empty defaults (mirroring the existing `command_map`
pattern) so a keymap without `DEFINE_HOST_CALLBACKS` still compiles.

Keymap usage (the functions already exist; this just registers them by name):

```c
DEFINE_HOST_CALLBACKS({
    { "disable_vim", disable_vim,    enable_vim    },
    { "vim_lazy",    vim_lazy_insert, disable_vim   },
    { "encoder_figma", set_rotary_encoder_figma, reset_rotary_encoder },
});
```

**ID = array index**, stable for a given firmware build. Re-querying names on
every reconnect makes ID renumbering across flashes harmless.

### 2. Host-layer tracker + host-callback enable state

In `notifier.c`, alongside the existing `activated_layer` (rename conceptually to
"board layer" in comments; keep the variable for backward compat):

```c
#define LAYER_UNSET 255
static uint8_t host_layer = LAYER_UNSET;
// host_cb_enabled[i] = true while host callback i is currently "enabled"
static bool host_cb_enabled[HOST_CALLBACK_MAX];   // HOST_CALLBACK_MAX = 64 (v1)

static void set_host_layer(uint8_t layer) {
    if (host_layer != LAYER_UNSET) layer_off(host_layer);
    host_layer = (layer == 0xFF) ? LAYER_UNSET : layer;
    if (host_layer != LAYER_UNSET) layer_on(host_layer);
}

// id_list = the desired ENABLED set (count entries). Diff + call callbacks.
static void apply_host_callbacks(const uint8_t *id_list, uint8_t count) {
    size_t n = get_host_callbacks_size();
    // disable-first (mirror board's disable-then-enable ordering)
    for (size_t i = 0; i < n; i++) {
        if (host_cb_enabled[i] && !in_list(i, id_list, count)) {
            host_cb_enabled[i] = false;
            if (get_host_callbacks()[i].on_disable)
                get_host_callbacks()[i].on_disable();
        }
    }
    // then enable newly-in-set
    for (size_t i = 0; i < n; i++) {
        if (!host_cb_enabled[i] && in_list(i, id_list, count)) {
            host_cb_enabled[i] = true;
            if (get_host_callbacks()[i].on_enable)
                get_host_callbacks()[i].on_enable();
        }
    }
}
```

> `HOST_CALLBACK_MAX` bounds static state; `QUERY_INFO.callback_count` tells the
> host how many exist. If a registry exceeds the cap, firmware should still
> report the true `callback_count` and the host must not reference ids ≥ cap
> (validate + warn).

### 3. Typed-command dispatch in `hid_notify()`

Patch the top of `hid_notify()` (currently `notifier.c`):

```c
void hid_notify(uint8_t *data, uint8_t length) {
    if (length < 2 || data[0] != 0x81 || data[1] != 0x9F) return;

    if (length >= 3 && data[2] == 0xF0) {
        handle_typed_command(data, length);   // NEW — see below
        return;
    }

    // ----- legacy string path (UNCHANGED) -----
    data += 2; length -= 2;
    ... existing reassembly + process_full_message() ...
}
```

`handle_typed_command()` parses `data[3]` = cmd_id and dispatches; always replies
with a 32-byte report starting `[0x51][cmd_id]`. `QUERY_INFO`/`QUERY_CALLBACK`
must be answerable even before any string has been seen. `APPLY_HOST_CONTEXT`
calls `set_host_layer()` + `apply_host_callbacks()` then acks.

### 4. Tests (firmware)

- Unit-test `set_host_layer` (on/off/clear, independence from board layer).
- Unit-test `apply_host_callbacks` diff semantics (enable/disable ordering, idempotence).
- Extend the existing host-side test harness (the `pattern_match.c` tests) with
  typed-command framing round-trips. Keep `notifier.c`'s `printf` debug behind
  `CONSOLE_ENABLE` as today.

---

## Crate Spec (`qmk_notifier`, Rust)

### Public API additions

```rust
pub enum RunCommand {
    SendMessage(String),                                    // legacy string (unchanged)
    QueryInfo,
    QueryCallback(u8),
    ApplyHostContext { layer: Option<u8>, callbacks: Vec<u8> },
}

pub enum CommandResponse {
    Legacy { matched: bool },                              // response[0] ∈ {0,1}
    Info { proto_ver: u8, feature_flags: u8, callback_count: u8, board_rules_present: bool },
    CallbackName { index: u8, name: Option<String> },
    Ack { ok: bool },
    Timeout,
}

// run() now returns the parsed response (it already reads the 32-byte report;
// surface it instead of discarding).
pub fn run(params: RunParameters) -> Result<CommandResponse, ...>;
```

- **Framing:** `SendMessage` keeps the existing header+chunk+ETX path. The typed
  variants build `[0x81,0x9F,0xF0,cmd, args…]` in a single report with a trailing
  `0x03`.
- **Response parse:** `response[0] == 0x51` ⇒ typed (decode by `cmd_echo`);
  `response[0] ∈ {0,1}` ⇒ `Legacy { matched }`; otherwise/no reply ⇒ `Timeout`.
- **Matcher module:** add `pub mod pattern` porting the firmware's stable subset
  (`*`, `^`, `$`, two-part class+title). Public so `qmkonnect` reuses it (single
  source of truth for match semantics). Port the firmware's test corpus as Rust
  unit tests for parity.

### Release

- Bump to **v0.3.0**, tag, and update `qmkonnect/Cargo.toml`:
  `qmk_notifier = { git = "...", tag = "v0.3.0" }`.

---

## QMKonnect Spec (this repo)

### 1. `rules.toml` — schema & parsing

New module `src/core/rules.rs`. File location: alongside `config.toml` via the
existing `platforms::get_config_paths()` directory (Linux
`~/.config/qmk-notifier/rules.toml`, Windows
`%APPDATA%\QMKonnect\rules.toml`, macOS
`~/Library/Application Support/QMKonnect/rules.toml`). Absent ⇒ host rules
disabled (string-only, today's behavior).

```toml
# rules.toml — host-side window rules.
# Board (DEFINE_*) rules ALWAYS run; these stack on top (layers) and
# additionally (callbacks). Run `qmkonnect --validate-rules` after editing.

[host]
on_no_match = "clear"        # "clear" (default) | "keep"
# default_layer = 2          # optional; only meaningful with future "default" mode

# Layer rules: FIRST match wins. One host layer active at a time, on top of the
# board's layer.
[[layer_rules]]
match = "alacritty"                       # class-only pattern
layer = 2

[[layer_rules]]
match = ["*chrome*", "*youtube*"]         # [class_pattern, title_pattern] (== WT())
layer = 4
case_sensitive = false                    # optional, default false

# Callback rules: ALL matches fire. Names come from the keyboard's registry
# (run `qmkonnect --list-callbacks` to see them).
[[callback_rules]]
match = "neovide"
enable = ["vim_lazy", "disable_vim"]      # run on focus-in
disable = ["vim_lazy"]                    # optional: run on focus-out

[[callback_rules]]
match = ["*chrome*", "*claude*"]
enable = ["vim_lazy", "disable_vim"]
```

Rust model (`src/core/rules.rs`):

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub struct RuleSet {
    #[serde(default)] pub host: HostDefaults,
    #[serde(default, rename = "layer_rules")]    pub layer_rules: Vec<LayerRule>,
    #[serde(default, rename = "callback_rules")] pub callback_rules: Vec<CallbackRule>,
}

#[derive(Debug, Deserialize)]
pub struct HostDefaults {
    #[serde(default)] pub on_no_match: OnNoMatch,   // default Clear
}
impl Default for HostDefaults { fn default() -> Self { Self { on_no_match: OnNoMatch::Clear } } }

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum OnNoMatch { Clear, Keep }

#[derive(Debug, Deserialize)]
pub struct LayerRule {
    #[serde(rename = "match")] pub pattern: Pattern,
    pub layer: u8,
    #[serde(default)] pub case_sensitive: bool,
}

#[derive(Debug, Deserialize)]
pub struct CallbackRule {
    #[serde(rename = "match")] pub pattern: Pattern,
    #[serde(default)] pub enable: Vec<String>,
    #[serde(default)] pub disable: Vec<String>,
    #[serde(default)] pub case_sensitive: bool,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Pattern {
    Single(String),                 // "foo"  -> class only
    Parts(String, String),          // ["cls","ttl"]
}
```

### 2. Host matcher & evaluation

- Use `qmk_notifier::pattern` for matching, with parity semantics:
  - `Pattern::Single(p)`: if the window has a title, match `p` against the
    **app_class only** (mirrors firmware: a delimiter-less pattern matches the
    class part). If no title, match against the whole string.
  - `Pattern::Parts(c, t)`: both `c` (against class) and `t` (against title) must
    match.
- Per window change (after debounce, after ① string send completes):
  1. **Layer:** iterate `layer_rules` in order; first match ⇒ `L_h`. None ⇒
     `None` (clear).
  2. **Callbacks:** iterate **all** `callback_rules`; for every match add its
     `enable` names to the desired set. (`disable` names are recorded as the
     "leave" set for the transition — see state machine below.)
- `on_no_match = Clear` (default): if no layer match ⇒ `layer=None`; if no
  callback match ⇒ desired set empty (firmware disables all host callbacks).
  `Keep`: re-send the previous context (or skip the send entirely).

### 3. Host state machine (`src/core/notifier.rs`)

Track across window changes:

```rust
struct HostContext {
    capable: bool,                         // from handshake
    name_to_id: HashMap<String, u8>,       // from QUERY_CALLBACK sweep
    current_layer: Option<u8>,
    current_enabled: HashSet<u8>,          // for logging/diagnostics
    pending_disable: HashSet<u8>,          // names → ids collected from rules
}
```

Per window change, after ①:

```text
desired_enable_ids = { resolve(name) for each enable name in all matched callback_rules }
desired_disable_ids = { resolve(name) for each disable name in all matched callback_rules }
L_h = first matching layer_rule.layer (else None)

if on_no_match == Clear and no layer match:    layer_arg = None
if on_no_match == Clear and no callback match: desired_enable_ids = {}

// The firmware diffs desired_enable_ids against its own state (disable-then-enable).
// To honor per-rule `disable` semantics too, send a preceding APPLY_HOST_CONTEXT
// with those ids removed from the enabled set, OR (simpler v1) fold `disable`
// names by NOT including them and letting them drop via the diff. See "Open
// questions" Q2 — recommended v1: treat `disable` as "remove from desired set".
send ApplyHostContext { layer: L_h, callbacks: desired_enable_ids }
current_layer, current_enabled = ...
```

> Unknown callback names (not in `name_to_id`) are logged and skipped, not fatal.

### 4. `notify_qmk` extension (`src/core/notifier.rs`)

Today `notify_qmk(window_info, verbose)` builds the string and sends it. Extend
it (and the `Notifier` trait / `QmkNotifier` impl) to, **after** the string's
round-trip succeeds, evaluate host rules and send `ApplyHostContext` via the
crate's new `RunCommand`. Keep the debounce worker unchanged; the host-context
send happens within the same debounced "send" step so one window change ⇒ one
string + one context.

The trait gains the capability so the test `Notifier` mock can assert ordering
(string before context). See Testing.

### 5. Startup handshake

In the startup path (near `startup_device_probe`), once a device is connected:

```text
resp = qmk_notifier::run(QueryInfo)
match resp {
  Info { proto_ver: 2, feature_flags, callback_count, .. } if flags & APPLY_HOST_CONTEXT => {
      for i in 0..callback_count {
          name = run(QueryCallback(i)) -> CallbackName.name
          name_to_id.insert(name, i)
      }
      validate rules.toml callback names against name_to_id  // warn, don't fail
      capable = true
  }
  _ => capable = false   // legacy or offline → string-only
}
```

Re-run on reconnect (the tray already polls `is_device_connected()`).

### 6. CLI

| Flag | Effect |
| --- | --- |
| `--list-callbacks` | Connect, run the handshake, print the `name → id` table (or "legacy firmware"). |
| `--validate-rules` `[--rules-path <p>]` | Parse `rules.toml`; report TOML/schema errors; if a keyboard is connected, flag callback names not in its registry. Exits non-zero on errors. |
| `--rules-path <path>` | Override the rules file location (mainly for testing/`--validate-rules`). |

`-c`/`--config` should also seed an empty commented `rules.toml` template next to
`config.toml` (discoverability).

### 7. Tray / UX

- Add **"Reload rules"** to the Windows tray, macOS menu-bar, and Linux SNI menus
  (re-read `rules.toml`, re-validate, re-run handshake if needed). Mirrors the
  existing `-r` reload pattern.
- Optional: status line shows `proto v2 · N callbacks` when connected & capable.

### 8. Backward compatibility guarantees

- No `rules.toml` ⇒ identical to today (string-only).
- Legacy firmware ⇒ handshake falls back ⇒ string-only; board rules unaffected.
- New firmware + old QMKonnect ⇒ old app sends only the string; new firmware's
  `hid_notify` still runs the legacy string path unchanged, so board rules work.
  (Typed commands simply never arrive.)

---

## Migration from `DEFINE_*`

Board rules keep working, so migration is **incremental and optional**:

1. **Expose callbacks by name** (one-time firmware change): add
   `DEFINE_HOST_CALLBACKS({ … })` listing the functions you already use in
   `DEFINE_SERIAL_COMMANDS`. Reflash once.
2. **Move a layer rule to the host:** add a `[[layer_rules]]` entry to
   `rules.toml`; **remove** it from `DEFINE_SERIAL_LAYERS` to avoid the same
   layer being driven by both trackers (harmless but confusing). No reflash
   needed for future edits.
3. **Move a callback rule to the host:** add a `[[callback_rules]]` entry;
   **remove** it from `DEFINE_SERIAL_COMMANDS` (callbacks are additive — if kept
   in both, the same `on_enable` would fire twice).
4. Iterate by editing `rules.toml` + "Reload rules" — no reflashing.

A `docs/examples.md` "host-rules" example and a `docs/qmk-integration.md` section
"Host-side rules (no reflash)" will be added (Phase D).

---

## Phased Rollout

- **Phase A — `qmk_notifier` crate v0.3.0:** typed-command framing, response
  parsing, `pattern` matcher module + ported tests. Tag.
- **Phase B — `qmk-notifier` firmware v0.3.0:** `DEFINE_HOST_CALLBACKS`,
  `host_layer` tracker, host-callback enable state, `hid_notify` dispatch,
  `QUERY_INFO`/`QUERY_CALLBACK`/`APPLY_HOST_CONTEXT` handlers, tests.
- **Phase C — `qmkonnect`:** pin crate v0.3.0; `src/core/rules.rs`; host matcher;
  handshake; `notify_qmk` extension + state; CLI flags; tray "Reload rules";
  config-path integration; tests.
- **Phase D — docs:** `Readme.md`, `docs/qmk-integration.md`,
  `docs/configuration.md`, `docs/examples.md`, `docs/troubleshooting.md`,
  regenerated `docs/llms_full.txt`, migration section.
- **Phase E (future, separate PRP):** VIA coexistence — a dispatching
  `raw_hid_receive` (`0x81 0x9F`+`0xF0` → notifier, else → VIA), plus the
  HID-exclusivity caveat in docs. Out of scope here.

---

## Testing Plan

**`qmk_notifier` crate**
- Unit: framing of each `RunCommand`; response decoding (`0x51` typed vs `0/1`
  legacy vs timeout).
- Unit: `pattern` parity — port the firmware `pattern_match` test corpus
  (wildcards, `^`/`$`, two-part `WT`, case sensitivity) and assert identical
  results.

**`qmk-notifier` firmware**
- Unit: `set_host_layer` (on/off/clear; independence from board `activated_layer`).
- Unit: `apply_host_callbacks` diff (disable-before-enable; idempotent re-apply;
  unknown ids ignored).
- Integration: typed-command round-trips via the existing host-side test harness.

**`qmkonnect`**
- Unit (`src/core/rules.rs`): TOML parse success/error; matcher first-match
  (layers) vs all-match (callbacks); `on_no_match` Clear vs Keep; unknown
  callback names skipped.
- Unit: handshake parsing — `Info` ⇒ capable; legacy/timeout ⇒ string-only.
- Unit: ordering — the `Notifier` mock records calls and asserts string is sent
  before `ApplyHostContext`.
- Integration: against a real keyboard on each platform (manual, per
  `AGENTS.md` loops): edit `rules.toml`, "Reload rules", switch apps, confirm
  layer + callbacks fire in order; confirm legacy firmware still works.

---

## Risks & Open Questions

- **R1 — HID round-trips per change.** Two sends (string + context) per debounced
  window change. Mitigated by the existing debounce; future: a persistent HID
  connection (today each `run()` opens/closes) and/or packing both into one
  logical transaction.
- **R2 — `APPLY_HOST_CONTEXT` fits one 32-byte report.** Header(2)+`0xF0`(1)+cmd(1)
  +layer(1)+count(1) = 6 bytes ⇒ ≤26 callback ids. Most users have <10. If a
  registry needs more, chunk across reports with ETX (like strings) in a later
  iteration; v1 caps `count ≤ 26` and the host validates/warns.
- **R3 — HID exclusivity.** Another Raw HID app (VIA browser/desktop) holding the
  device will block QMKonnect. Documented in Phase E; not solved here.
- **R4 — ID stability across flashes.** Mitigated by re-querying names on every
  reconnect; IDs are positional, names are stable.
- **Q1 — `on_no_match = "default"` with `default_layer`.** Reserved in the schema
  but not wired in v1 (only `clear`/`keep`). Add when a use case appears.
- **Q2 — `disable` list semantics on the host.** Recommended v1 behavior: names
  in a matched rule's `disable` are treated as "remove from desired enabled set
  this cycle" (so the firmware's diff calls their `on_disable`). A richer
  "explicitly disable now, regardless of prior state" mode is deferred. Confirm
  during Phase C review.
- **Q3 — Board matcher stays first-match.** Host callbacks use all-match (C8);
  the board's existing `DEFINE_SERIAL_COMMANDS` keeps first-match for backward
  compatibility. Flip to all-match only if explicitly requested later.
- **R5 — Multiple keyboards.** v1 uses one global ruleset; per-keyboard overrides
  deferred (C9).
- **R6 — Legacy handshake side effect.** See the "Handshake timing constraint"
  note: handshake must run only at (re)connect, not on a poll, to avoid the
  legacy no-match path deactivating an active board layer/command.

---

## Appendix A — Pattern Matching Semantics (v1 stable subset)

Ported to Rust in `qmk_notifier::pattern`, mirroring `pattern_match.c`:

- `*` — wildcard, any chars (including none).
- `^` — anchor to start.
- `$` — anchor to end.
- Two-part (`WT(class, title)` / `Pattern::Parts`): class pattern matched
  against app_class, title pattern against title; both must match. Delimiter is
  `0x1D` (GS).
- Single pattern against a `{class}{GS}{title}` message: matches the **class**
  part only (firmware parity).
- `case_sensitive` per rule (default `false`).
- **Deferred:** `\d \D \w \W \s \S \b \B` and `.` (regex classes) — not in v1.

## Appendix B — File layout touched

```
qmkonnect/
  Cargo.toml                              # bump qmk_notifier tag -> v0.3.0
  src/core/notifier.rs                    # notify_qmk extension, handshake, state
  src/core/rules.rs                       # NEW: rules.toml model + evaluation
  src/core/mod.rs                         # wire rules into config/startup
  src/main.rs                             # --list-callbacks / --validate-rules
  src/tray.rs / src/linux_tray.rs         # "Reload rules" menu item
  Readme.md, docs/*.md, docs/llms_full.txt# Phase D docs
qmk_notifier/  (external crate)
  src/lib.rs (or new module)              # RunCommand variants, run() response
  src/pattern.rs                          # NEW: matcher module
qmk-notifier/  (external firmware)
  notifier.h                              # host_callback_t, DEFINE_HOST_CALLBACKS
  notifier.c                              # host_layer, host_cb_enabled, dispatch
```
