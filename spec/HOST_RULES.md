# SPEC — Host-Side Window Rules & Callbacks

> Companion to `PRD.md`. Design for the feature that moves app→layer and
> app→callback matching onto the host so
> rules can change **without reflashing**. Host rules **stack on top of** the
> board's `DEFINE_*` rules (board first, host on top; board callbacks first,
> host callbacks after). Read alongside `PROTOCOL.md` (wire framing),
> `FIRMWARE.md` (the qmk_notifier module), `CONFIG.md` (config schema), and
> `ARCHITECTURE.md` (the notification pipeline). Spans **three repos**: the
> `qmk-notifier` Rust crate, the `qmk_notifier` firmware, and `qmkonnect`.
> Registered in the PRD feature table (F11/F12), §12 Future Work, and the
> Document Map.

---

## 1. Goal & Deliverables

**Goal.** Let users define **app → layer** and **app → callback** rules in an
editable file on their computer (`rules.toml`), with the matching done by
QMKonnect on the host — so rules change **without reflashing firmware**. Both
layer switching *and* arbitrary firmware callbacks (the existing `command_map`
`on_enable`/`on_disable` pattern) are supported. Host rules **stack on top of**
the keyboard's existing firmware rules (`DEFINE_SERIAL_LAYERS` /
`DEFINE_SERIAL_COMMANDS`): the board's rules always run first, then host rules
apply on top. Nothing existing is removed or deprecated.

**Deliverables.**

- **`qmk-notifier` crate:** typed-command framing + response parsing, and new
  `RunCommand` variants. (Transport-only — the matcher is NOT here; it lives in
  `qmkonnect`.)
- **`qmk_notifier` firmware:** a named callback registry
  (`DEFINE_HOST_CALLBACKS`), a separate host-layer tracker, host-callback enable
  state, typed-command dispatch inside `hid_notify()`, and handlers for
  `QUERY_INFO` / `QUERY_CALLBACK` / `APPLY_HOST_CONTEXT`.
- **`qmkonnect`:** `rules.toml` parsing + validation, host-side rule evaluation,
  a startup capability/name handshake, `notify_qmk` extended to send the host
  context after the legacy string, CLI flags (`--list-callbacks`,
  `--validate-rules`), an "Edit rules" tray item (opens `rules.toml` in the
  system editor, seeding it from the template if absent), and per-platform rules-file
  paths.
- **Docs:** updates to `docs/qmk-integration.md`, `docs/configuration.md`,
  `docs/examples.md`, `docs/troubleshooting.md`, `Readme.md`, and a regenerated
  `docs/llms_full.txt`, plus the migration subsection (§10).

**Success definition.**

- A user can add/change a layer or callback rule by editing `rules.toml`; it
  hot-reloads on the next window change (or is opened via the "Edit rules" tray
  item) — **no reflash**, and no manual reload step.
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

## 2. Context: How It Works Today & the Three-Repo Reality

Today QMKonnect builds `{app_class}{GS}{title}` and calls
`qmk_notifier::run(RunParameters::new(RunCommand::SendMessage(msg), …))` (see
`ARCHITECTURE.md` §5, `PROTOCOL.md` §4). The **`qmk-notifier` crate** owns all
wire framing (the `0x81 0x9F` header, 32-byte chunking, the `0x03` ETX
terminator, and the response read). The **`qmk_notifier` firmware** (`notifier.c`)
receives in `hid_notify()`, reassembles to ETX, and `process_full_message()`
matches `command_map` (first match → `on_enable`; previous `on_disable` first)
and `layer_map` (first match → `activate_layer`/`layer_on`; previous
`deactivate_layer`/`layer_off` first), tracking a **single** `activated_layer`.
It always replies with a 32-byte report whose `response[0] = matched` (0/1).

Because the wire protocol is shared, this feature touches **all three repos**:

| Repo | Role | Change here |
| --- | --- | --- |
| `qmk-notifier` (Rust crate) | Host-side framing + HID I/O | Typed commands, response parsing, matcher module |
| `qmk_notifier` (firmware C) | On-keyboard receiver/matcher | Registry, host-layer tracker, typed-command dispatch |
| `qmkonnect` (this app) | Window detection + rules | `rules.toml`, host matcher, handshake, sequencing |

## 3. Locked Design Decisions

> **Design.** The wire contract is owned by the firmware spec
> (`dabstractor/qmk_notifier`, `PRD.md` §4.6 — **canonical**); the transport by
> the `qmk-notifier` crate (`PRD.md` §10); the host-side orchestration by this
> document.

- **B1 — Coexistence = per-window stack-or-replace, host-chosen via
  `clear_board`.** The firmware offers **both**: with `clear_board=0` the board
  runs its rules (board layer first → host layer on top; board callbacks first →
  host callbacks after); with `clear_board=1` the firmware clears its board
  layer/command first and the host context drives the board. The host selects per
  window from `disable_firmware_config` (C10). Board rules are never silently
  discarded — the host decides whether they run.
- **B2 — Callback identity = firmware registry + startup name query.** Firmware
  declares named callbacks (`DEFINE_HOST_CALLBACKS`); IDs are declaration order;
  QMKonnect queries names at (re)connect and the rules file references callbacks
  by **name**. Re-querying on every reconnect makes cross-flash renumbering
  harmless.
- **B3 — "Arbitrary callback" = firmware-registered C functions only** (the
  existing `on_enable`/`on_disable` pattern). Host-side actions (shell/launch)
  and host-driven keyboard macros are **out of scope**.
- **C1 — Format: TOML.** C2 — separate `rules.toml` next to `config.toml`. C3 —
  hot-reload by re-parsing `rules.toml` on every window focus change (no fs
  watch, no manual reload) + an "Edit rules" tray item (system editor;
  seed-if-absent); a parse failure is never silent (desktop notification +
  string-only fallback — §7). **C4 — full matcher parity**: port
  the firmware `pattern_match.c` to Rust **including** `+` and the classes
  (`\d \D \w \W \s \S \b \B .`) — they are linear-time in the firmware NFA, so
  there is no perf reason to subset. C5 — capability handshake with graceful
  fallback (gated on `proto_ver == 2`). C6 — VIA coexistence is a future phase
  (feature_flags bit `0x04` reserved). **C7 — host no-match ⇒ clear host only**
  (the `on_no_match = "keep"` option is dropped; the host layer is cleared and
  host callbacks' `on_disable` fires via the desired-set diff — **host silo
  only**; the board is untouched, see C13). C8 — **all matching
  callbacks fire**; layers are exclusive (first-match-wins, one host layer). C9 —
  one global ruleset for v1 (per-keyboard overrides later). **C10 —
  `disable_firmware_config` per-rule** (default `false`, global default under
  `[host]`, per-rule override on `[[layer_rules]]`/`[[callback_rules]]`): a
  matched rule with it `true` contributes to a **replace** decision for that
  window. **C11 — host layer is a raw QMK layer index** (no fixed reserved range): the
  firmware applies it verbatim via `layer_on()`/`layer_off()` and performs **no**
  range validation, so the only reserved value is `255` (`LAYER_UNSET`/clear),
  which the host rejects as a rule target. The index must fit the firmware's
  `layer_state_t` (a bitmask: default 16-bit ⇒ layers 0–15, `LAYER_STATE_32BIT`
  ⇒ 0–31; `layer_on(n)` with `n ≥` the width is UB), and to win in **stack**
  mode it must exceed the highest board layer active for that window (QMK's
  highest-set-bit rule); in **replace** mode the board is cleared first, so any
  valid index wins. *(The earlier "≥ 224" reservation is withdrawn:
  `layer_state` cannot hold bit 224 even at 32-bit, and `layer_on(224)` is UB
  that on typical compilers wraps to bit `224 mod 32 = 0`, silently activating
  the base layer.)* **C12 — host is
  the OS source of truth** while connected: `SET_OS` once at connect
  (host-authoritative; firmware `OS_DETECTION` is the offline fallback). **C13 —
  independent silos**: board rules (`DEFINE_*`, driven by the window string) and
  host rules (`rules.toml`, driven by `APPLY_HOST_CONTEXT`) each run in their own
  silo. The host sends the window string for every window that is not in explicit
  "replace" mode — **including host no-match windows** — so the board's silo
  always runs (it self-clears on its own no-match). A host no-match clears
  **only** the host layer/callbacks (`APPLY_HOST_CONTEXT{layer:0xFF,
  clear_board:false}`); it never suppresses or clears the board. The sole
  cross-silo action is an explicit per-window "replace"
  (`disable_firmware_config=true` on a matched rule → no string +
  `clear_board=1`), a deliberate opt-out — not a no-match side effect.

## 4. Architecture & Coexistence Model

Per-window-change data flow (the `disable_firmware_config` / `clear_board` model):

```
window focus changes
        │
        ▼
debounce (existing, configurable ms)
        │
        ▼
build string  s = "{app_class}\x1D{title}"        (existing)
        │
        ▼
(if host-capable AND rules.toml present)
evaluate host rules against s
   • layer_rules   : first match → L_h  (else none)
   • callback_rules: ALL matches → desired callback id set
   • window is "replace" iff EVERY matched rule has disable_firmware_config=true
        │
   ┌──── replace, OR board has no rules ────┐   ┌── stack (>=1 rule non-disabling) AND board has rules ──┐
   ▼                                        ▼   ▼                                                         ▼
 ② APPLY_HOST_CONTEXT{L_h, set,            ① Send STRING_MATCH(s) ──► firmware runs BOARD rules
      clear_board=1}  (NO string sent)         (disable prev cmd/layer, enable matched) ◄─ response[0]=matched
   ──► firmware clears board layer/cmd,    ② APPLY_HOST_CONTEXT{L_h, set, clear_board=0}
       then applies host layer + callbacks   ──► firmware applies host layer on top, syncs host callbacks
   ◄── response[0]=0x51 ack                  ◄── response[0]=0x51 ack
        │
        ▼
on no host match (not replace) ⇒ ① Send STRING_MATCH(s)   (board silo runs — sets/clears its OWN activated_layer/cmd from the string)
                             + ② APPLY_HOST_CONTEXT{layer:0xFF, set=empty, clear_board=false}  (clears HOST layer+callbacks ONLY; board untouched — C13)
update host state for next diff/logging
```

**Coexistence semantics (precise):**

- The host sends the **window string** for every window that is not in explicit
  "replace" mode — including host no-match windows — so the board's silo always
  runs (C13). Only an explicit per-window "replace" (every matched rule
  `disable_firmware_config=true`) withholds the string and sets `clear_board=1`;
  a host no-match sends the string **and** `APPLY_HOST_CONTEXT{layer:0xFF,
  clear_board:false}` (clears host only, never the board).
  The string is shared by both board lanes, so it is sent at most once.
- Firmware maintains **two independent layer trackers**: `activated_layer`
  (board, selected per-OS via round-A multi-OS) and `host_layer` (driven by
  `APPLY_HOST_CONTEXT`). They are orthogonal but share one QMK `layer_state`
  bitmask (each calls `layer_on`/`layer_off` on it). There is no fixed reserved
  host range (C11): in **stack** mode the host layer wins only if its index
  exceeds the board layer QMK would otherwise resolve to (highest-set-bit); in
  **replace** mode the board tracker is cleared for that window first (the host's
  `clear_board` flag) so any valid index wins, and it re-engages on the next
  string send.
- Callbacks: in stack mode board callbacks fire during string processing, then
  host callbacks during `APPLY_HOST_CONTEXT`. In replace mode only host callbacks
  fire. The `disable` field in a callback rule is an **explicit-exclusion**
  override; the natural focus-out `on_disable` comes free from the desired-set
  diff (a callback leaving the desired set is disabled by the firmware).
- If `rules.toml` is absent or the keyboard is legacy (`proto_ver != 2`), only ①
  the legacy string runs — today's behavior, bit-for-bit. Host rules are gated on
  `proto_ver == 2`.

## 5. Wire Protocol (typed commands)

> **Canonical: firmware `PRD.md` §4.6.** This section summarizes the
> transport-relevant detail; the firmware owns the byte layout and this document
> defers to it on disagreement. See `PROTOCOL.md` §8 for the desktop mirror and
> the `qmk-notifier` crate `PRD.md` §10 for the transport API.

- **Discriminator:** `data[2] == 0xF0` ⇒ typed command; else legacy string
  (unchanged). `0xF0` can never begin a real matched string (sanitizer allows
  only `0x20–0x7E`), so **legacy firmware safely ignores typed commands**.
- **Framing:** `[0x81][0x9F][0xF0][cmd_id][ args… ][0x03]`, **ETX-framed and
  multi-report** like strings (chunked at 30 payload bytes/report). This removes
  the earlier "≤26 callbacks per report" cap — `APPLY_HOST_CONTEXT` may span
  reports. (The old v1 single-report/≤26 limit is withdrawn.)
- **Responses:** legacy `[matched(0|1)]…`; typed `[0x51][cmd_id_echo][payload]…`;
  no reply ⇒ `Timeout` ⇒ host stays string-only.

**Command table** (firmware §4.6 is authoritative for field definitions):

| `cmd_id` | Name | Request args | Response payload |
| --- | --- | --- | --- |
| `0x01` | `QUERY_INFO` | none | `[proto_ver][feature_flags][callback_count][board_rules_present]` |
| `0x02` | `QUERY_CALLBACK` | `[index]` | `[index][name, NUL-padded]` |
| `0x03` | `SET_OS` | `[os_byte]` | `[ack]` |
| `0x04` | *(reserved — VIA, Phase E)* | — | — |
| `0x05` | `APPLY_HOST_CONTEXT` | `[layer][flags][count][id…]` | `[ack]` |

- `proto_ver`: `1` = legacy string-only firmware; `2` = typed-command capable. Firmware-owned.
- `feature_flags`: `0x01` `APPLY_HOST_CONTEXT`; `0x02` callback registry; `0x04`
  *(reserved)* VIA.
- `os_byte`: `0 UNSURE · 1 LINUX · 2 WINDOWS · 3 MACOS · 4 IOS`. The host sends
  `SET_OS` once at connect; while connected the host OS is **authoritative** for
  `current_os` (firmware `OS_DETECTION` is the offline fallback).
- `APPLY_HOST_CONTEXT.layer`: desired host-layer number — a **raw QMK layer
  index** (`0..=254`; no fixed floor, bounded by the firmware's `layer_state_t`
  width — see C11) — or `0xFF` (clear). `flags` bit 0 = **`clear_board`** ⇒
  firmware clears its board
  `activated_layer` + current command before applying the host context (the
  per-window "replace"). `id…` = the full desired enabled set; firmware diffs
  (disable-before-enable).

**Handshake & `has_been_queried`:** at (re)connect the host sends `QUERY_INFO`
**at most once per board boot** — the firmware sets `has_been_queried` on the
first `QUERY_INFO`, so a mid-session HID re-enumeration against **legacy** firmware
cannot clear an active board layer (legacy walks `QUERY_INFO` as a no-match
string and `process_full_message` always disables/deactivates first — harmless
only when board state is fresh). If `response[0]==0x51` & `proto_ver==2` &
`flags & 0x01` ⇒ `QUERY_CALLBACK` sweep → `name→id` map → validate `rules.toml`.
Else (`response[0] != 0x51` or timeout) ⇒ legacy ⇒ string-only; **never send typed
commands**. (Typed commands bypass `process_full_message`, so they have no
board side effect on `proto_ver==2` firmware.)

## 6. Firmware Spec (`qmk_notifier`)

> **Canonical: firmware `PRD.md` §14 (+ §4.6 wire, §4.7 OS).** This section is a
> desktop-facing summary; the firmware repo owns the authoritative spec.

Firmware requirements:

- **Named callback registry** — `DEFINE_HOST_CALLBACKS({ … })` + weak-default
  accessors (`get_host_callbacks`/`_size`). `ID = array index`, stable per build;
  re-queried by name on every reconnect. Bounded by `HOST_CALLBACK_MAX` (static
  array; the wire no longer caps the id list — multi-report — but the firmware's
  static ceiling is real, so the host must not reference ids ≥
  `HOST_CALLBACK_MAX`; `QUERY_INFO.callback_count` reports the true count).
  ```c
  typedef struct { const char *name; callback_t on_enable; callback_t on_disable; } host_callback_t;
  host_callback_t* get_host_callbacks(void);
  size_t           get_host_callbacks_size(void);
  ```
- **Second layer tracker** `host_layer` (independent of board `activated_layer`)
  + `host_cb_enabled[]`. `set_host_layer(layer)`: `layer_on/off` the host tracker
  only; `0xFF` ⇒ clear. `apply_host_callbacks(ids, count)`: disable-before-enable
  diff (fire `on_disable` for ids leaving the set, `on_enable` for ids entering).
- **Typed dispatch** at the top of `hid_notify()`: `data[2]==0xF0` ⇒
  `handle_typed_command()` (return; **no** `process_full_message` side effect);
  else legacy string (unchanged). Handlers:
  - `QUERY_INFO` / `QUERY_CALLBACK` — answerable before any string seen; the
    firmware sets `has_been_queried` on the first `QUERY_INFO`.
  - `APPLY_HOST_CONTEXT` — honor `clear_board` (flags bit 0): if set,
    `deactivate_layer()` the board `activated_layer` + `disable_command()` the
    board command **first**, then `set_host_layer()` + `apply_host_callbacks()`.
  - `SET_OS` (`0x03`) — update `current_os` (host-authoritative while a host is
    connected; firmware `OS_DETECTION` resumes as the offline fallback).
- **Tests:** `set_host_layer` (on/off/clear; independence from board layer),
  `apply_host_callbacks` (diff ordering; idempotence), typed-command round-trips,
  `clear_board` clearing, `SET_OS` updating `current_os`.

## 7. Crate Spec (`qmk-notifier`, Rust)

> **Canonical: the crate `PRD.md` §10.** This section is a summary. The crate is
> **transport-only** — it does no matching (the matcher lives in `qmkonnect`, §8).

API additions (`run()` returns `CommandResponse` instead of `()`):

```rust
pub enum RunCommand {
    SendMessage(String),                                                // legacy string
    ListDevices,
    QueryInfo,                                                          // 0x01
    QueryCallback(u8),                                                  // 0x02
    SetOs(HostOs),                                                      // 0x03
    ApplyHostContext { layer: Option<u8>, callbacks: Vec<u8>, clear_board: bool }, // 0x05
}

#[repr(u8)]
pub enum HostOs { Unsure = 0, Linux = 1, Windows = 2, Macos = 3, Ios = 4 }  // mirrors os_variant_t

pub enum CommandResponse {
    Legacy { matched: bool },              // response[0] in {0,1}
    Info { proto_ver: u8, feature_flags: u8, callback_count: u8, board_rules_present: bool },
    CallbackName { index: u8, name: Option<String> },
    Ack { ok: bool },
    Timeout,
}

pub fn run(params: RunParameters) -> Result<CommandResponse, QmkError>;
```

- **Framing:** `SendMessage` keeps the existing header+chunk+ETX path. Typed
  variants build `[0x81,0x9F,0xF0,cmd, args…]` and reuse the **same ETX-framed,
  multi-report chunking** as strings — so `APPLY_HOST_CONTEXT` may span reports
  (no fixed callback-id cap). The device cache + retry logic are unchanged.
- **Response parse:** after a typed burst, read one 32-byte IN report;
  `response[0]==0x51` ⇒ typed (decode by `cmd_echo`); `in {0,1}` ⇒ `Legacy`; no
  reply ⇒ `Timeout`.

**Release:** tag the release; `qmkonnect/Cargo.toml` pins the crate by git tag.

## 8. QMKonnect Spec (this repo)

**(1) `rules.toml`** — new module `src/core/rules.rs`, alongside `config.toml`
(Linux `~/.config/qmkonnect/rules.toml`, Windows `%APPDATA%\QMKonnect\`,
macOS `~/Library/Application Support/QMKonnect/`). Absent ⇒ host rules disabled
(string-only). Schema in §9; CLI seeding in (6).

**(2) Host matcher — in qmkonnect, NOT the crate.** Port the firmware
`pattern_match.c` to Rust at `src/core/pattern.rs` (**full parity**: `* ^ $ WT +`
and `\d \D \w \W \s \S \b \B .` — all linear-time). Port the firmware test corpus
as parity tests. Semantics:
- `Pattern::Single(p)`: match `p` against **app_class only** (firmware parity:
  a pattern with no GS vs. a message with GS matches the `msg_left` portion).
  The title is never consulted for `Single`; use `Pattern::Parts(c, t)` to
  match the title.
- `Pattern::Parts(c, t)`: both halves must match.

**(3) Per-window evaluation** (`src/core/notifier.rs`). After debounce:
1. **Layer:** first matching `layer_rule` ⇒ `L_h` (else none).
2. **Callbacks:** **all** matching `callback_rules` ⇒ desired enabled id set;
   each rule's `disable` names are an **explicit exclusion** (removed from the
   desired set, so the firmware's diff fires their `on_disable`).
3. **Stack-vs-replace:** the window is **replace** iff every matched rule's
   effective `disable_firmware_config` is `true`.

**(4) `notify_qmk` send logic** (the `disable_firmware_config` / `clear_board`
model). For one debounced window change:
- **Stack** (board has rules AND ≥1 matched rule non-disabling): send the
  **string** first (`RunCommand::SendMessage`), await its `CommandResponse`, then
  `ApplyHostContext { layer: L_h, callbacks, clear_board: false }`.
- **Replace** (all matched rules disabling, OR board has no rules): send **only**
  `ApplyHostContext { layer: L_h, callbacks, clear_board: true }` (no string →
  board can't match → firmware clears its board layer/cmd via the flag).
- **No host match:** send the **string** first (the board silo still runs — it
  sets/clears its own `activated_layer`/command from the string, C13), then
  `ApplyHostContext { layer: None (0xFF), callbacks: empty, clear_board: false }`
  — clears the **host** layer + callbacks only (`clear_board: false` ⇒ board
  untouched). A host no-match never suppresses or clears the board.
- The `Notifier` trait / `QmkNotifier` gain the capability so the test mock
  asserts ordering (string before context). Retry/cache parity with `SendMessage`.

**(5) Startup handshake + `SET_OS`.** Near `startup_device_probe`, once a device
is connected:
```text
resp = run(QueryInfo)
match resp {
  Info { proto_ver: 2, feature_flags, callback_count, .. } if flags & 0x01 => {
      run(SetOs(host_os))                                  // host is OS-authoritative at connect
      for i in 0..callback_count { name_to_id.insert(run(QueryCallback(i)).name, i) }
      validate rules.toml names against name_to_id         // warn, don't fail
      capable = true
  }
  _ => capable = false   // legacy/offline → string-only
}
```
The handshake runs **at most once per board boot** — the firmware's
`has_been_queried` guards against mid-session-reconnect side effects on legacy
firmware, and host-rules are gated on `proto_ver == 2`. Re-trigger only on a real
device transition via the existing `is_device_connected()` poll, deduped by the
`capable`/`has_been_queried` state.

**(6) CLI:** `--list-callbacks` (handshake → name→id table, or "legacy");
`--validate-rules [--rules-path <p>]` (parse + schema check; flag unknown callback
names; non-zero exit on error); `--rules-path`. `-c`/`--config` seeds a commented
`rules.toml` template.

**(7) Tray/UX:** add **"Edit rules"** to all three menus — seed `rules.toml` from
the commented template if absent (same body as `-c`), then open it in the system
default editor (`xdg-open` / `open` / `cmd /C start`). Rule changes apply
automatically — `rules.toml` is re-parsed on every window focus change, so there
is **no apply button**. **Validation is automatic, not manual:** if `rules.toml`
fails to parse, fire a **desktop notification** (`notify-send` on Linux,
`NSUserNotification` on macOS, toast on Windows) carrying the parse error and
fall back to string-only — never silent. The deliberate on-demand check remains
`--validate-rules` (CLI). (The former "Reload rules" item is withdrawn: redundant
for applying rules, and its validation feedback was log-only.)

**(8) Backward compatibility:** no `rules.toml` ⇒ identical to today; legacy
firmware (`proto_ver != 2` / timeout) ⇒ string-only, board rules unaffected; new
firmware + old QMKonnect ⇒ old app sends only the string, typed commands never
arrive.

## 9. `rules.toml` Schema Reference

```toml
# rules.toml — host-side window rules.
# disable_firmware_config chooses, per window, whether the board runs its own
# rules (stack) or is cleared and driven solely by the host (replace). Global
# default under [host]; per-rule override below.
# Run `qmkonnect --validate-rules` after editing.

[host]
disable_firmware_config = false   # global default: false = stack (board runs), true = replace
# On no match the host layer is always cleared and all host callbacks disabled.

# Layer rules: FIRST match wins. One host layer active at a time.
# `layer` is a RAW QMK layer index (no fixed floor): must be < your layer_state
# width (<=15 default, <=31 with LAYER_STATE_32BIT), > your highest board layer
# to win in stack mode, and != 255 (the wire "clear" sentinel, rejected).
[[layer_rules]]
match = "alacritty"                       # class-only pattern
layer = 10
disable_firmware_config = true           # optional override (default inherits [host])

[[layer_rules]]
match = ["*chrome*", "*youtube*"]         # [class_pattern, title_pattern] (== WT())
layer = 11
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
    #[serde(default)] pub disable_firmware_config: bool,   // default false (stack)
}
impl Default for HostDefaults { fn default() -> Self { Self { disable_firmware_config: false } } }

#[derive(Debug, Deserialize)]
pub struct LayerRule {
    #[serde(rename = "match")] pub pattern: Pattern,
    pub layer: u8,
    #[serde(default)] pub case_sensitive: bool,
    #[serde(default)] pub disable_firmware_config: Option<bool>,  // None => inherit [host]
}

#[derive(Debug, Deserialize)]
pub struct CallbackRule {
    #[serde(rename = "match")] pub pattern: Pattern,
    #[serde(default)] pub enable: Vec<String>,
    #[serde(default)] pub disable: Vec<String>,
    #[serde(default)] pub case_sensitive: bool,
    #[serde(default)] pub disable_firmware_config: Option<bool>,  // None => inherit [host]
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Pattern {
    Single(String),                 // "foo"  -> class only
    Parts(String, String),          // ["cls","ttl"]
}
```

A rule's effective `disable_firmware_config` = its override if `Some`, else the
`[host]` default. The window is **replace** iff it matched ≥1 rule **and** every
matched rule's effective flag is `true`; only then is the string withheld. The
string is sent for every other window — stack matches **and host no-match
windows** — so the board silo always runs (C13; a host no-match clears host state
only, never the board). Match semantics are a
**full-parity port** of the firmware `pattern_match.c` (incl. `+` and
`\d \D \w \W \s \S \b \B .` — all linear-time in the NFA). `case_sensitive` per
rule (default `false`).

## 10. Migration from `DEFINE_*`

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
4. Iterate by editing `rules.toml` — changes hot-reload on the next window
   change (or use the "Edit rules" tray item) — no reflashing.

## 11. Implementation Breakdown (by repo)

One coordinated change across the three repos:

- **`qmk-notifier` crate:** typed-command framing (multi-report),
  `CommandResponse` reply parsing, `HostOs`, `run()` → `CommandResponse`. Tag the
  release. *(The matcher is NOT added here — it lives in qmkonnect.)*
- **`qmk_notifier` firmware:** `DEFINE_HOST_CALLBACKS`,
  `host_layer`/`host_cb_enabled`, typed dispatch, `QUERY_INFO`/`QUERY_CALLBACK`/
  `SET_OS`/`APPLY_HOST_CONTEXT` (with `clear_board`), `has_been_queried`, tests.
- **`qmkonnect`:** pin the crate; `src/core/rules.rs` + `src/core/pattern.rs`
  (full-parity matcher + ported corpus); handshake + `SET_OS`; the `notify_qmk`
  `disable_firmware_config`/`clear_board` send logic + state; CLI flags; tray
  "Edit rules" + parse-failure desktop notification; config-path integration;
  tests.
- **Docs:** `Readme.md`, `docs/qmk-integration.md`, `docs/configuration.md`,
  `docs/examples.md`, `docs/troubleshooting.md`, regenerated `docs/llms_full.txt`.
- **VIA coexistence (separate, out of scope here):** a dispatching
  `raw_hid_receive` (`0x81 0x9F`+`0xF0` → notifier, else → VIA).

## 12. Testing Plan

**`qmk-notifier` crate:** unit-test framing of each `RunCommand` (incl.
multi-report `APPLY_HOST_CONTEXT`) and response decoding (`0x51` typed vs `0`/`1`
legacy vs `Timeout`).

**`qmk_notifier` firmware:** unit-test `set_host_layer` (on/off/clear;
independence from board `activated_layer`) and `apply_host_callbacks`
(disable-before-enable; idempotent re-apply; unknown ids ignored); integration:
typed-command round-trips, `clear_board` clearing, `SET_OS` updating `current_os`.

**`qmkonnect`:** unit-test (`src/core/rules.rs`) TOML parse success/error, matcher
first-match (layers) vs all-match (callbacks), `disable` exclusion, unknown
callback names skipped; unit-test (`src/core/pattern.rs`) **full matcher parity**
by porting the firmware `pattern_match` corpus (wildcards, `^`/`$`, `WT`, `+`,
classes, case sensitivity) and asserting identical results; unit-test handshake
parsing (`Info { proto_ver: 2 }` ⇒ capable; legacy/timeout ⇒ string-only) and the
`disable_firmware_config` ⇒ stack/replace send decision; unit-test ordering — the
`Notifier` mock records calls and asserts string-before-context (stack) and
context-only (replace); integration per `AGENTS.md`.

## 13. Risks & Open Questions

- **R1 — HID round-trips per change.** Stack mode = two sends (string + context)
  per debounced change; replace mode = one. Mitigated by the existing debounce.
- **R2 — `APPLY_HOST_CONTEXT` size — RESOLVED.** Typed commands are ETX-framed /
  multi-report (like strings), so the callback-id list is uncapped; the earlier
  "≤26 ids per report" v1 limit is withdrawn. (`HOST_CALLBACK_MAX` remains the
  firmware's static array ceiling; the host validates against
  `QUERY_INFO.callback_count`.)
- **R3 — HID exclusivity.** Another Raw HID app (VIA) holding the device blocks
  QMKonnect. Phase E.
- **R4 — ID stability across flashes.** Mitigated by re-querying names on every
  reconnect (IDs positional, names stable).
- **R5 — Multiple keyboards.** v1 = one global ruleset; per-keyboard overrides
  deferred.
- **R6 — Legacy handshake side effect — RESOLVED.** The firmware sets
  `has_been_queried` on the first `QUERY_INFO`, and host-rules are gated on
  `proto_ver == 2`; the host handshakes at most once per board boot. Legacy
  firmware never receives typed commands.
- **Q1 — `default_layer` / a "default" no-match mode.** Reserved in the schema
  but not wired (`on_no_match` is always `clear`). Add if a use case appears.
- **Q2 — `disable` list semantics — RESOLVED.** `disable` = explicit exclusion
  (removed from the desired enabled set; the firmware's diff fires `on_disable`).
  Focus-out `on_disable` also fires automatically when a callback leaves the
  desired set across window changes.
- **Q3 — Board matcher stays first-match.** Host callbacks use all-match (C8);
  board `DEFINE_SERIAL_COMMANDS` keeps first-match for backward compatibility.

## 14. Appendix — File Layout Touched & Pattern Subset

**File layout:**

```
qmkonnect/
  Cargo.toml                              # pin qmk-notifier crate by git tag
  src/core/notifier.rs                    # notify_qmk extension, handshake, SET_OS, state
  src/core/rules.rs                       # NEW: rules.toml model + evaluation
  src/core/pattern.rs                     # NEW: full-parity matcher (ported from firmware)
  src/core/mod.rs                         # wire rules into config/startup
  src/main.rs                             # --list-callbacks / --validate-rules
  src/tray.rs / src/linux_tray.rs         # "Edit rules" menu item + parse-failure notification
  Readme.md, docs/*.md, docs/llms_full.txt
qmk-notifier/  (external crate)
  src/lib.rs / src/core.rs                # RunCommand variants, HostOs, CommandResponse, run()
qmk_notifier/  (external firmware)
  notifier.h / notifier.c                 # host_callback_t, DEFINE_HOST_CALLBACKS,
                                          #   host_layer, host_cb_enabled, typed dispatch,
                                          #   SET_OS, clear_board, has_been_queried
```

**Pattern matching semantics** — a **full-parity** port of the firmware
`pattern_match.c` into `qmkonnect::pattern` (not a subset): `*` wildcard; `^`/`$`
anchors; two-part `WT(class,title)` / `Pattern::Parts` (delimiter `0x1D`, GS); `X+`
quantifier; classes `\d \D \w \W \s \S \b \B`; `.`; escapes. All linear-time
(Thompson NFA). `case_sensitive` per rule (default `false`). The firmware matcher
+ its test corpus are the single source of truth for match semantics.

---

*The wire contract is canonical in the firmware `PRD.md` §4.6; transport in the
`qmk-notifier` crate `PRD.md` §10. Return to `PRD.md` for the product-level
overview and the Document Map.*
