# SPEC — Device Discovery, Capability Selection & VIA Coexistence

> Companion to `PRD.md` / `PROTOCOL.md` / `ARCHITECTURE.md` / `UI.md`. Defines
> **how QMKonnect finds the right keyboard, how it proves the keyboard speaks the
> qmk_notifier protocol, how it behaves when several keyboards are present, and
> the guarantee that an always-on QMKonnect never locks out the intermittently-
> used VIA app.** Covers the two-tier discovery model, the capability probe, the
> three-state device-status indicator, the discovered-device Settings picker,
> multi-board broadcast, and the cross-platform shared-HID-access contract. Read
> alongside `PROTOCOL.md` §3 (match predicate) and `ARCHITECTURE.md` §5 (the
> notification pipeline + status probe).

---

## 1. Goal & the Two-Tier Model

QMKonnect is the **always-on** half of the system; the keyboard may also be
edited at runtime by **VIA** (a WebHID app, used only intermittently to change
the keymap). Two requirements follow:

1. **QMKonnect must find and select the correct keyboard with no user
   configuration in the common case**, and disambiguate sensibly when several
   QMK boards are plugged in — without forcing the user to know or type VID/PID.
2. **QMKonnect must never hold an exclusive HID lock**, because it runs
   continuously; VIA must always be able to open the device when the user wants
   to edit the keymap. (See §6 — this is the load-bearing coexistence guarantee.)

Discovery is therefore **capability-based, in two tiers**:

| Tier | Question answered | Mechanism | Breadth |
|---|---|---|---|
| **1 — Presence** | "Is *any* QMK Raw-HID board attached?" | Enumerate HID, filter usage page `0xFF60` / usage `0x61` | Every board with `RAW_ENABLE` (qmk_notifier, VIA, Vial, custom) |
| **2 — Capability** | "Does this board actually run **qmk_notifier**?" | Send a `0x81 0x9F`-prefixed `QUERY_INFO`; classify by the reply | qmk_notifier boards only |

Tier 1 finds the broad corpus (every cooperative QMK board on the bus); Tier 2
narrows to the boards QMKonnect can actually command. **VID/PID is neither tier
— it remains an optional, power-user narrowing axis** (see §7). This two-tier
model is why QMKonnect can be "zero-config for a single standard QMK keyboard"
(PRD §2.1 Goal 1) *and* scale to a desk with a VIA board and a qmk_notifier board
plugged in at once: Tier 1 sees both; Tier 2 selects the one that speaks back.

> **Canonical ownership.** The byte-level match predicate lives in
> `PROTOCOL.md` §3; the typed `QUERY_INFO` command and the handshake sequence
> live in `HOST_RULES.md` §5 (canonical: firmware `PRD.md` §4.6). This document
> defines the *discovery/selection* layer that sits on top of both: when to
> enumerate, when to ping, how to classify, and how to render the result to the
> user and the tray.

---

## 2. The Capability Probe

### 2.1 Why a second tier is needed

Tier 1 (usage-page presence) is necessary but not sufficient: a pure VIA board
(no qmk_notifier module) also exposes `0xFF60`/`0x61`. Without Tier 2 the tray
status would light green for such a board while nothing happens (VIA's firmware
ignores the `0x81 0x9F` magic), and QMKonnect would waste writes broadcasting
magic bursts to a board that will never act on them. The capability probe turns
"present" into "present *and responsive*."

### 2.2 What the probe sends

The probe reuses the **existing host-rules handshake** (`ARCHITECTURE.md` §5.7,
`HOST_RULES.md` §5): a single `QUERY_INFO` typed command
(`[0x81][0x9F][0xF0][0x01][0x03]` — the trailing `0x03` is ETX). The reply is
decoded by the `qmk-notifier` crate into a `CommandResponse`:

```rust
match run(QueryInfo, &filter) {
    Ok(CommandResponse::Info { proto_ver: 2, feature_flags, callback_count, board_rules_present }) => Capable { .. },
    Ok(CommandResponse::Legacy { .. })            => NotQmkNotifier,   // replied, but legacy/no typed cmd
    Ok(CommandResponse::Timeout)                  => NotQmkNotifier,   // pure VIA board: no reply to magic
    Ok(_) | Err(_)                                => NotQmkNotifier,   // anything else
}
```

A board is **qmk_notifier-capable** iff the reply is `Info { proto_ver: 2, .. }`.
Everything else — including a clean `Timeout` (the normal pure-VIA case: the VIA
firmware's `raw_hid_receive` never answers magic-prefixed input, so no IN report
arrives) — is classified `NotQmkNotifier`. **No board is ever harmed by the
probe:** the magic header is what makes qmk_notifier coexist with other Raw HID
modules (`FIRMWARE.md` §1), so VIA/Vial firmware silently ignores the probe.

### 2.3 `classify_devices(verbose) -> Vec<ClassifiedDevice>`

New function in `src/core/notifier.rs` (sibling of `is_device_connected`):

```rust
pub struct ClassifiedDevice {
    pub path: String,            // stable hidapi path (cache key)
    pub vendor_id: u16,
    pub product_id: u16,
    pub product_name: Option<String>,
    pub usage_page: u16,
    pub usage: u16,
    pub kind: DeviceKind,        // Capable { proto_ver, feature_flags, callback_count, board_rules_present } | NotQmkNotifier
}
```

Algorithm:
1. `HidApi::new()`; enumerate. Keep interfaces where `usage_page == 0xFF60 &&
   usage == 0x61` (plus the optional VID/PID narrowers from `configured_filter`
   — §7). This is the Tier-1 candidate set.
2. For each candidate: `open_path` (**shared** — §6), send one `QUERY_INFO`,
   read one IN report with a short timeout, classify, close. One transaction per
   candidate per classification pass.
3. Return the classified vector. Cache the result keyed by `path`
   (`CLASSIFICATION_CACHE: Mutex<HashMap<String, (DeviceKind, Instant)>>`) with
   a TTL of `CLASSIFICATION_TTL` (default 5 s) so the hot path does not re-ping
   on every status poll.

> **Probe cadence vs. presence cadence.** Presence (Tier 1) is cheap and polled
> frequently (macOS/Windows 3 s, Linux 1 s — unchanged). Classification (Tier 2)
> is **event-driven, not polled**: it runs once per device *appearance* (on a
> Tier-1 false→true transition), and the cached `DeviceKind` is reused until the
> device disappears or the TTL expires. This keeps the always-on daemon's HID
> traffic minimal and polite (§6.3).

### 2.4 Relationship to the host-rules handshake

The host-rules `perform_handshake` (`ARCHITECTURE.md` §5.7) and the discovery
probe are **the same `QUERY_INFO` transaction**; they share the crate call and
the `HOST_CAPABLE` semantics. The difference is *purpose and scope*:

- **Discovery probe** (`classify_devices`): runs **per candidate** to build the
  classified set (which board(s) can we command?).
- **Host-rules handshake** (`perform_handshake`): runs **once per board boot**
  against the capable set to negotiate typed-command support, sweep
  `QUERY_CALLBACK` names, and send `SET_OS` — gated on `proto_ver == 2`.

Implementation note: `perform_handshake`'s existing dedup guard
(`has_been_queried`, reset on a real device transition) is keyed against the
**capable set**, not the raw Tier-1 set. With multi-board broadcast (§4) the
callback-name sweep runs against a representative capable board (first by stable
`path`); heterogeneous multi-board (different callback registries) is a v1
limitation documented in §4.3.

---

## 3. Device-Status Semantics (three states)

The tray/menu-bar status line (`UI.md` §4) moves from a boolean (any `0xFF60`
interface present) to a **three-state** value derived from `classify_devices`:

| State | Condition | Tray text | Icon |
|---|---|---|---|
| **Connected** | ≥1 **capable** board present | `●  Device Connected` (or `●  N Devices Connected`) | solid `U+25CF`, full alpha |
| **No module** | ≥1 Tier-1 board present, **0 capable** | `⚠  QMK board found — no qmk_notifier module (flash it)` | warning glyph, full alpha |
| **Disconnected** | 0 Tier-1 boards present | `○  No Device Connected` | hollow `U+25CB`, dimmed (~35% alpha on Linux) |

The "No module" state is the whole point of Tier 2: it gives the user truthful,
actionable feedback ("you have a QMK board, but it isn't running the firmware
QMKonnect talks to") instead of a false-green "Connected" that silently does
nothing. On Linux this also fires a one-shot `notify-send` on the
Disconnected→No-module transition with the same message + a link to
`docs/qmk-integration.md`.

The status probe thread (`ARCHITECTURE.md` §5.6) is unchanged in cadence; it now
calls `classify_devices` (cache-backed) instead of the boolean
`is_device_connected`. Transitions (not every poll) drive the UI update, exactly
as today. `is_device_connected()` is retained as a Tier-1-only predicate used by
the device-presence snapshot and the broadcast write path (§4.2).

---

## 4. Multi-Board Policy

### 4.1 v1 decision: broadcast to all capable boards

When **more than one** qmk_notifier-capable board is present, QMKonnect
**broadcasts every window event to all of them.** This is a deliberate v1
policy (PRD §2.2/§12; HOST_RULES R5): the `qmk-notifier` crate already bursts
each message to every matching interface (`PROTOCOL.md` §4.2); the only change
is that the match set is now "capable boards" rather than "every `0xFF60`
interface," so magic bursts no longer go to pure-VIA boards that would ignore
them.

Rationale: the common multi-board case is a user with two qmk_notifier boards
(e.g. home + travel, or a split pair flashed independently) who wants *all* of
them to track the foreground app. Broadcast satisfies that with zero
configuration and no ambiguity.

### 4.2 What "broadcast" means concretely

- **Device filter for writes:** `configured_filter()` (VID/PID optional) AND
  `kind == Capable`. The crate's device cache (`LazyLock<Mutex<Option<DeviceCache>>>`,
  `PROTOCOL.md` §4.4) is keyed by this enriched `MatchKey`; it is invalidated on
  any write failure or on a classification change (a board entering/leaving the
  capable set).
- **Per window change:** the debounced pipeline sends the legacy string
  (`SendMessage`) **and**, when host-capable, the `APPLY_HOST_CONTEXT` typed
  command (`HOST_RULES.md` §4) — both burst to **all** capable boards. The
  capability/handshake dedup (`has_been_queried`) is evaluated against the
  capable set, not a single device.
- **Acknowledgements:** the crate drains bounded IN-side acks after each burst
  (`IN_DRAIN_MAX = 32`); with N boards there are N reply streams interleaved on
  the shared read, all magic-prefixed and self-describing (`0x51`+`cmd_echo`),
  so they are demultiplexed by content, not by source. (See §6.4 for the
  coexistence analogue.)

### 4.3 v1 limitation (documented, not fixed here)

There is **one global `rules.toml`** (HOST_RULES C9). Broadcast assumes the
capable boards run **equivalent firmware** (same callback registry, same layer
indices). If a user has two *heterogeneous* qmk_notifier boards (different
callback names / layer maps), the single global ruleset cannot address them
independently. **Per-keyboard rules + independent handshake per board are
deferred** (PRD §12). Heterogeneous setups still *work* (each board runs its own
board rules from the window string); only the host-rules layer assumes
homogeneity.

---

## 5. The Discovered-Device Picker (Settings UX)

The Settings dialog (`UI.md` §2) is restructured. **Raw VID/PID hex entry is no
longer the primary surface** — it becomes an "Advanced / manual override"
disclosure. The primary surface is a live, self-populating list of discovered
devices.

### 5.1 Primary surface

A read-only header line plus an optional picker, built from `classify_devices`:

```
Detected keyboard(s):
  ✓  Dactyl-Manuform (5x7-1)        0xFEED:0x0000   ← qmk_notifier
  ✗  Keychron Q1                     0x3434:0x0123   ← QMK board, no module
  [ Choose… ]      [ Rescan ]
```

- **One capable board, no VID/PID set** (the common case): the header reads
  `Detected: Dactyl-Manuform (5x7-1)` and no picker is shown. Auto-discovery is
  already correct; there is nothing to choose. This preserves the zero-config
  promise.
- **Multiple Tier-1 boards:** the picker appears. Rows are the live
  `product_name` (from the HID descriptor — the device names itself; **no curated
  database**), VID:PID, and a ✓/✗ capability marker. Selecting a row is the
  disambiguation: it writes that board's VID/PID into `config.toml` (via the
  shared `render_config_body` renderer) so subsequent matches narrow to it.
- **No capable board, ≥1 Tier-1 board:** the picker shows the board(s) with ✗ and
  the "No module" status message (§3); selecting one still records its VID/PID
  for when the user flashes qmk_notifier.

`[ Rescan ]` invalidates `CLASSIFICATION_CACHE` and re-runs `classify_devices`
(useful after flashing a board while the dialog is open).

### 5.2 Advanced / manual override (disclosure)

Collapsible. Contains the existing two hex fields (`vendor_id`, `product_id`)
for the rare case the user wants to target a board that isn't currently on the
bus, or override the picker. Editing these fields writes through
`render_config_body` exactly as today. Empty/`"auto"` ⇒ `None` ⇒ auto-discovery.

### 5.3 Per-platform rendering

| Platform | Picker widget | Replaces |
|---|---|---|
| **Windows** | Win32 `LISTBOX` (or `ListView`) in the `QMKSettingsDialog`; VID/PID fields under a "Advanced ▸" group box | the two `EDIT` controls as the primary surface (they move under Advanced) |
| **macOS** | `NSStackView` of rows in the `NSAlert` accessory view; an `NSButton` "Advanced" toggles the `NSTextField` pair | the two `NSTextField`s as primary |
| **Linux** | `zenity --list --column …` (the discovered list) + a second `zenity --forms` for the Advanced VID/PID; or the native GTK popup already used for window-info | the single `zenity --forms` with two entries |

The shared `DIALOG_RESULT` becomes `struct { chosen: Option<(u16,u16)>, manual: Option<(Option<u16>,Option<u16>)> }`; the save path applies `chosen` first, else `manual`, else leaves VID/PID as-is.

---

## 6. VIA Coexistence Guarantee (the headline requirement)

> **Requirement R-COEX.** QMKonnect is the always-on process; VIA is used only
> intermittently to edit the keymap. **QMKonnect must never hold an HID lock that
> prevents VIA from opening the device.** This is satisfied by construction
> (QMKonnect opens all HID handles **shared / non-seize**) and is asserted by
> tests. It is *not* dependent on VIA's cooperation.

### 6.1 Why this direction is the one that matters

Coexistence is symmetric in principle, but asymmetric in practice: QMKonnect
holds cached HID handles open **for the entire session** (the device cache,
`PROTOCOL.md` §4.4, keeps opened handles alive across notifications for
performance). VIA opens the device only while its UI is actively editing. So the
only realistic lock-out risk is **QMKonnect's long-lived open blocking VIA's
short-lived open** — never the reverse. The guarantee therefore places the burden
entirely on QMKonnect: keep every open **shared**, and VIA can always get in.

### 6.2 Shared open, on every platform (verified)

QMKonnect links `hidapi = "2.6"` (`Cargo.toml`); it opens devices with the
crate's default `open_path`, which is **non-exclusive everywhere**:

| Platform | hidapi open mode | Can it block another app? |
|---|---|---|
| **Linux** | `open(/dev/hidraw*, O_RDWR)` — shared by kernel design | **No** — multiple `open()` calls always coexist |
| **Windows** | `CreateFile(..., FILE_SHARE_READ \| FILE_SHARE_WRITE, ...)` on the HID device path | **No** — shared by request; and vendor-defined collections (`0xFF60`) are *shared by Windows HID policy* regardless |
| **macOS** | `IOHIDDeviceOpen(..., kIOHIDOptionsTypeNone)` — **non-seize** | **No** — only `kIOHIDOptionsTypeSeizeDevice` blocks others, and hidapi (hence QMKonnect) never sets it |

**The requirement on the code:** QMKonnect must never call any seize/exclusive
path. The `hidapi` crate does not expose a seize option, so this is enforced by
*not* introducing one. A unit test asserts the open flags / (on macOS) that the
IOKit option type is `kIOHIDOptionsTypeNone` (0); a comment at the open call site
documents that changing it violates R-COEX.

### 6.3 Polite read discipline

Holding a shared handle open does **not** block VIA (§6.2), but a *perpetual
blocking read* on the input endpoint could starve VIA's reads. QMKonnect
therefore **reads only in short windows around its own writes**: it sends a burst,
then drains a bounded number of pending IN reports (`IN_DRAIN_MAX = 32`,
`PROTOCOL.md` §4.2), then issues no further reads until the next notification.
Between debounced window changes (default `debounce_ms = 50`) QMKonnect is
quiescent on the read side. This is existing behavior; R-COEX makes it a
**must-preserve invariant** (`ARCHITECTURE.md` §10) — never introduce a
long-lived blocking read on the device handle.

### 6.4 Protocol demultiplexing (why overlapping traffic is harmless)

Even when QMKonnect and VIA transact simultaneously, their byte streams are
disjoint:

- **QMKonnect** writes `0x81 0x9F …` and reads `0x81`/`0x51`-marked replies.
- **VIA** writes its own command namespace (`0x01`–`0x15` per `quantum/via.h`)
  and reads VIA-shaped replies.

The `0x81 0x9F` magic header is the demultiplexer: each side ignores bytes that
do not match its own prefix. So even if the OS delivers a VIA reply to
QMKonnect's read (or vice versa), it is discarded harmlessly. A unit test
asserts QMKonnect **never emits** VIA-shaped bytes (`0x00`-leading report-ID
aside, the first payload byte is always `0x81`) — i.e. QMKonnect cannot
accidentally speak VIA.

### 6.5 Graceful degradation under contention

If a write fails because the device is momentarily unavailable (a rare,
transient condition — e.g. the OS briefly can't satisfy an open during a
hot-plug storm), the existing retry/backoff handles it (`ARCHITECTURE.md` §5.4:
up to 3 attempts, then log + `Ok` — never restart-loop). R-COEX adds no new
failure mode; it only documents that **shared open + polite reads mean VIA never
causes QMKonnect to fail, and QMKonnect never causes VIA to fail.**

### 6.6 Platform reality (why "one app locked the device" can't happen)

For completeness, the guarantee holds from VIA's side too, independent of
QMKonnect:

- **VIA is a WebHID app** (`the-via/app`, `src/shims/node-hid.ts` wraps
  `navigator.hid`). The WebHID API **offers no exclusive/seize open at all** —
  confirmed by [WICG/webhid#100](https://github.com/WICG/webhid/issues/100),
  where users request `open({exclusive:true})` and the Chrome WebHID implementer
  states it isn't possible cross-platform.
- **Windows HID policy** makes top-level collections exclusive or shared *by
  usage page*: Mouse/Keyboard/Generic-Desktop are OS-claimed; **vendor-defined
  (`0xFF00`+, including `0xFF60`) are shared.** So the interface is shared by OS
  policy regardless of what either app requests.

So the "one app grabbed it exclusively and the other can't connect" failure mode
**cannot occur** for a `0xFF60` interface on any supported platform, in either
direction. R-COEX is the user-facing statement of that fact with the burden
placed where it belongs (the always-on QMKonnect).

---

## 7. Config Implications

### 7.1 VID/PID is now an Advanced override

`config.toml`'s `vendor_id`/`product_id` (`CONFIG.md` §1) are unchanged in
*semantics* (`Option<u16>`, `None` = match any = auto-discovery) but are
re-framed in the docs and UI as an **Advanced / manual override** for
disambiguation, not the primary configuration path. The discovery picker (§5)
writes them on the user's behalf when they choose a specific board. New default
users never touch them.

### 7.2 `0xFEED` comment cleanup

The seeded template (`render_default_config_template`, `CONFIG.md` §2) currently
shows `# vendor_id = 0xfeed   # unset: auto-discovery`, which has historically
been misread as "0xFEED is the default." It is rewritten to remove the literal:

```toml
# vendor_id  = 0x????   # unset: auto-discover any QMK keyboard (recommended)
# product_id = 0x????   # unset: auto-discover any QMK keyboard (recommended)
```

The `qmk-notifier` crate's `DEFAULT_VENDOR_ID = 0xFEED` / `DEFAULT_PRODUCT_ID =
0x0000` constants (`PROTOCOL.md` §3.3) are explicitly documented as
**matching-dead** (used only as historical fallbacks in the crate CLI; `None`
always means wildcard in QMKonnect). A doc comment is added at their definition
pointing here.

---

## 8. Implementation Map (function-level)

| Area | Change | File |
|---|---|---|
| Classify discovered devices | **NEW** `classify_devices(verbose) -> Vec<ClassifiedDevice>`; `CLASSIFICATION_CACHE` (path→(kind, expiry)) | `src/core/notifier.rs` |
| Truthful status | Status probe calls `classify_devices` (cache-backed) instead of boolean; three-state enum `DeviceStatus { Connected(usize), NoModule, Disconnected }` | `src/core/notifier.rs`, `src/tray.rs`, `src/linux_tray.rs` |
| Multi-board broadcast | Write filter becomes `configured_filter() && kind==Capable`; cache `MatchKey` enriched; invalidate on classification change | `src/core/notifier.rs` (+ crate `MatchKey`) |
| Handshake scope | `perform_handshake`'s dedup keyed on the **capable set**; callback sweep against representative (first-by-path) capable board | `src/core/notifier.rs` |
| Settings picker | New discovered-device list widget + Advanced disclosure; `DIALOG_RESULT` extended | `src/tray.rs` (Win32/NSAlert), `src/linux_tray.rs` (zenity/GTK) |
| Shared-open invariant | Comment at every `open_path`; unit test asserting non-seize; `ARCHITECTURE.md` §10 invariant added | `src/core/notifier.rs` (+ crate) |
| `0xFEED` cleanup | `render_default_config_template` comment rewrite | `src/core/mod.rs` |
| CLI | `--list-devices` output gains a `kind` column (`qmk_notifier` / `qmk-only` / `via-only`-ish, from a one-shot `classify_devices`) | `src/core/notifier.rs`, `src/main.rs` |

> **Crate touch (small):** the `qmk-notifier` crate's `DeviceFilter`/`MatchKey`
> may need to carry the capability distinction so the cache can be keyed by
> "capable boards only." If the crate exposes the raw device list + a
> `send_command(QueryInfo, &filter)` (it does — `HOST_RULES.md` §7), classification
> can live entirely in `qmkonnect` and the crate need not change; prefer that.

---

## 9. Testing Plan

- **`classify_devices` unit/integration:** a fake HID layer returning (a) an
  `Info{proto_ver:2}` reply ⇒ `Capable`, (b) a `Legacy` reply ⇒ `NotQmkNotifier`,
  (c) `Timeout` ⇒ `NotQmkNotifier`; assert classification, cache hit/miss/TTL,
  and that a pure-`0xFF60` (no reply) board is `NotQmkNotifier`.
- **Status state machine:** transitions Disconnected↔Connected↔NoModule fire UI
  updates only on change; the Disconnected→NoModule one-shot notification on Linux.
- **Multi-board broadcast:** with two capable fake boards, one window change
  produces writes to **both**; with one capable + one `NotQmkNotifier`, only the
  capable board is written (no magic burst to the VIA-only board).
- **Shared-open invariant:** assert QMKonnect never calls a seize/exclusive open
  (static check: no `Seize`/`exclusive` at the open call site; on macOS the IOKit
  option is `kIOHIDOptionsTypeNone`). Assert the read side issues no read except
  bounded drains after a write (no perpetual blocking read).
- **Protocol demultiplex:** assert the first payload byte QMKonnect ever writes
  is `0x81` (it never emits VIA-shaped bytes).
- **Picker:** selecting a row writes that board's VID/PID via
  `render_config_body`; the common single-board case shows no picker; Rescan
  invalidates the cache.
- **`0xFEED` cleanup:** the seeded template contains no literal `0xfeed`.

---

## 10. Cross-References

- **`PROTOCOL.md` §3** — the Tier-1 match predicate (canonical); §3.5/§3.6 added
  for the capability tier + shared-open contract.
- **`ARCHITECTURE.md` §5.2/§5.6/§5.7** — `DeviceFilter`, the status probe, the
  handshake; §10 invariants gain the shared-open + capability-ping items.
- **`UI.md` §2/§4** — the discovered-device Settings picker and the three-state
  status line.
- **`CONFIG.md` §1/§2** — VID/PID as Advanced override; the `0xFEED` cleanup.
- **`HOST_RULES.md` §5/R3/§11** — the handshake is also the discovery probe; R3
  (HID exclusivity) is resolved by R-COEX; Phase-E firmware dispatch referenced.

---

*Continue with `SPEC_PLATFORMS.md`.*