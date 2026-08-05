# Delta PRD — Two-Tier Device Discovery & VIA Coexistence (F13/F14)

**Base:** v0.2.8 (session 004 snapshot) → **Target:** v0.2.8 (session 005 snapshot)
**Scope:** `qmkonnect` repo only. The wire contract and the `QUERY_INFO` transaction are **unchanged** → the `qmk-notifier` crate and `qmk_notifier` firmware are **not** modified by this delta (one possible crate touch is called out as *research-gated and likely deferred* — see §2).
**Size:** Large — a new user-facing capability layer (F13) plus a correctness-guarantee layer (F14), spanning core discovery, a three-platform tray/Settings surface, invariant tests, and small config/CLI/doc cleanups. Full PRD structure is warranted.

---

## 1. What Actually Changed (diff analysis)

The diff `plan/004_*/prd_snapshot.md` → `plan/005_*/prd_snapshot.md` adds a **new companion spec** (`DEVICE_DISCOVERY.md`, ~24 KB, entirely new) and threads F13/F14 through `PRD.md`, `ARCHITECTURE.md`, `PROTOCOL.md`, `UI.md`, `CONFIG.md`, and `HOST_RULES.md`. The changes fall into six themes:

| # | Spec change | Where | Code status |
|---|---|---|---|
| **D1** | **F13 — Two-tier device discovery + capability selection.** Tier-1 = HID usage-page presence (`0xFF60`/`0x61`); Tier-2 = a `0x81 0x9F` `QUERY_INFO` probe that classifies each candidate `Capable` vs `NotQmkNotifier`. Adds `classify_devices()`, a three-state device status (`Connected` / `No module` / `Disconnected`), a discovered-device Settings picker, and multi-board broadcast. | `PRD.md` §2.1 Goal 1, §2.2 non-goal, §4 (F13), §8, §12, §13 glossary (5 terms), §14 map; `DEVICE_DISCOVERY.md` §1–§5,§7–§9; `ARCHITECTURE.md` §5.2,§5.6,§10(#9); `PROTOCOL.md` §3.5; `UI.md` §2.0,§4; `CONFIG.md` §1.1,§7.1 | ❌ **NOT implemented** — no `classify_devices`, no `DeviceStatus`/`DeviceKind`/`ClassifiedDevice`, no three-state tray text, no Settings picker. Tray status is still the two-state boolean. |
| **D2** | **F14 — VIA coexistence guarantee (R-COEX).** QMKonnect opens every HID handle **shared / non-seize** and reads only in bounded drains around its own writes, so the intermittently-used VIA app can always open the device. New must-preserve invariant. | `PRD.md` §4 (F14), §12 (moved from future-work to shipped), §13 (R-COEX, shared-open); `DEVICE_DISCOVERY.md` §6; `ARCHITECTURE.md` §10(#10); `PROTOCOL.md` §3.6 | ⚠️ **True-by-construction but unasserted.** hidapi's default `open_path` is already non-seize everywhere; the read discipline (bounded `IN_DRAIN_MAX` drains) already exists. **Missing:** documenting comments at the open sites, and the invariant *tests* the spec requires (§9). |
| D3 | **`0xFEED` config-template cleanup.** The seeded `# vendor_id = 0xfeed` hint (historically misread as "0xFEED is the default") → `# vendor_id = 0x????   # unset: auto-discover`. VID/PID reframed as an *Advanced override* (the picker writes them). | `DEVICE_DISCOVERY.md` §7.2; `CONFIG.md` §1.1,§2 (`0xFEED cleanup` note); `PROTOCOL.md` §3.4 ("matching-dead") | ❌ **NOT implemented** — `src/core/mod.rs:238-239` still renders the literal `0xfeed`/`0x0000`. |
| D4 | **CLI `--list-devices` gains a `kind` column** (one-shot `classify_devices` → `qmk_notifier` / `qmk-only` / etc.). | `DEVICE_DISCOVERY.md` §8 | ❌ **NOT implemented** — `list_devices()` (`notifier.rs:129`) prints `vid:pid page:usage product` only. |
| D5 | **HOST_RULES cross-references** — the `QUERY_INFO` is noted as the *same transaction* reused as the Tier-2 probe; R3 (HID exclusivity) marked RESOLVED-by-R-COEX; R5 (multiple keyboards) reframed as v1 broadcast; §11 VIA-coexistence updated. | `HOST_RULES.md` §5 (probe note), §11, §13 R3/R5 | ✅ **Doc-only.** The handshake (`perform_handshake`) already sends `QUERY_INFO` and sets `HOST_CAPABLE` — that *is* the Tier-2 probe (see §2). No code change; the spec just records the equivalence. |
| D6 | **PLATFORMS.md title-change detection + hot-config** — Windows `EVENT_OBJECT_NAMECHANGE` hook; macOS 500 ms title-change poller + `NOTIFY_TX` off-main-thread worker; Hyprland atomic dedup+update and live `poll_interval_ms` hot-config. | `PLATFORMS.md` §2.1,§3.1,§5.2,§5.4; `CONFIG.md` §1.2 | ✅ **Already in code** (commit `2f8d622 "Detect title changes and hot-reload intervals"`). **Spec catch-up — generates NO tasks.** |

**Implementation work = D1 + D2 + D3 + D4.** D5 is doc-only (the equivalence is already true in code); D6 was landed in a prior session and is noted for awareness only.

---

## 2. Implementation Gaps, Key Constraints & Scoping Decisions

### 2.1 The headline value is cheap: three-state status derives from *existing* state

The spec (§3) frames the three-state tray status as a client of a new `classify_devices()`. But the codebase **already classifies capability on every device transition**: the status-poll thread (`tray.rs:380-405`, mirrored in `linux_tray.rs:260+`) already computes `handshake_action(last, connected)` and calls `perform_handshake` on a `Gain`, which sends `QUERY_INFO` and sets the global `HOST_CAPABLE` (`notifier.rs:270`, read via `host_capable()`, reset on `Loss`). Therefore:

| Status state | Derivation from existing state |
|---|---|
| **Disconnected** | `!is_device_connected()` (0 Tier-1 boards) |
| **No module** | `is_device_connected() && !host_capable()` (≥1 Tier-1, 0 capable — the pure-VIA case) |
| **Connected** | `is_device_connected() && host_capable()` (≥1 capable) |

So P1's three-state status can ship **without a new per-path pinging function** — it reads two booleans the app already maintains. `classify_devices()` (per-candidate ping + `CLASSIFICATION_CACHE`) is still wanted for the **Settings picker** (D1 §5 — needs to *name* each board with a ✓/✗ marker) and is scoped to P3.

> **Transient-state caveat (to handle, not block):** right after a `Gain`, `host_capable()` is `false` until `perform_handshake` finishes the round-trip, so the line may briefly read "No module" before flipping to "Connected". This is acceptable (sub-second) and matches the spec's "classification runs once per device appearance." P1 should ensure the handshake's completion flips the line (the poll's transition-gating already re-sends on the next tick).

### 2.2 Scoping decision: multi-board *write narrowing* is research-gated (likely DEFERRED)

`DEVICE_DISCOVERY.md` §4.2 wants the write match set to become "Tier-1 **AND** `kind == Capable`" so magic bursts stop going to pure-VIA boards. **Two facts bound this:**

1. The `qmk-notifier` crate's `MatchKey` is **filter-keyed** (`{vid?, pid?, usage_page, usage}` — `core.rs:641`), **not path-keyed**, and exposes **no per-path send**. The app cannot tell the crate "only the capable paths" without a crate API addition.
2. It is **already harmless in practice**: VIA firmware ignores `0x81 0x9F`-prefixed input (the magic header is the coexistence guard, `FIRMWARE.md` §1), so bursts to a pure-VIA board are silently dropped. The narrowing is a *politeness/traffic* optimization, **not a correctness requirement**.

**Decision for this delta:** treat write-narrowing as a **research-gated** item inside P2 (R-COEX). The expected resolution is **defer** — true per-board narrowing needs a coordinated `qmk-notifier` crate change (new `MatchKey` field or a path-scoped send), which is out of scope for a one-repo delta and buys only "don't write to boards that ignore us anyway." P2 lands the **in-repo** R-COEX guarantee (invariant tests + comments) regardless. If research finds a clean app-only narrowing (e.g. VID/PID resolution suffices for the single-board common case), it may be included; otherwise it is documented as a follow-up.

### 2.3 The picker is the largest single piece

The discovered-device Settings picker (D1 §5) is genuine new UI across three native toolkits (Win32 `LISTBOX`, macOS `NSStackView`, Linux `zenity --list`/GTK), plus an "Advanced / manual override" disclosure that relocates the existing VID/PID hex fields. It depends on `classify_devices()` to populate rows (per-candidate capability + self-described `product_name`). This is scoped to P3 and is the milestone most likely to surface per-device-classification mechanics (the crate constraint in §2.2 applies).

### 2.4 What is NOT touched (already correct / out of scope)

- `perform_handshake`, `HOST_CAPABLE`, `host_capable()`, `handshake_action`, `is_device_connected()` — **reused as-is** (they *are* the Tier-2 mechanism). No change to the handshake itself.
- The `qmk-notifier` crate and `qmk_notifier` firmware — **unchanged** (the typed `QUERY_INFO`/`CommandResponse` API already exists; see §2.2 for the one research-gated exception).
- The debounce pipeline, host-rules (`rules.toml`) evaluation, and the typed-command wire bytes — **unchanged**.
- Title-change detection & hot-config intervals (D6) — **already shipped**; no tasks.

---

## 3. Backlog

### Phase P1 — Truthful three-state device status (the F13 headline)

Deliver the user-visible point of F13: a pure-VIA board no longer shows a false-green "Connected". Derive a three-state `DeviceStatus` from the existing `is_device_connected()` + `host_capable()` (§2.1 — no new pinging function required for the status line), and render it in both tray surfaces. This phase has **no dependency on `classify_devices()`**.

#### Milestone P1.M1 — Three-state status resolver + both tray surfaces

**Task P1.M1.T1 — Add a `DeviceStatus` resolver and wire three-state rendering into macOS/Windows + Linux trays**

- **P1.M1.T1.S1 — Add `DeviceStatus` (three-state) to `src/core/notifier.rs` and a `device_status()` resolver** *(story points: 2)*
  - **CONTRACT:**
    1. **RESEARCH:** `src/core/notifier.rs` — `is_device_connected()` @216 (Tier-1 pure enumerate), `host_capable()` @689 (reads `HOST_CAPABLE` @270, set by `perform_handshake` on a `Gain`, reset by `reset_handshake_state` on a `Loss`). The status-poll threads (`tray.rs:380-405`, `linux_tray.rs:~260`) already drive `handshake_action` and thus keep `host_capable()` correct on every transition. Target wording: `DEVICE_DISCOVERY.md` §3 (the three-state table + the Linux one-shot `notify-send` on Disconnected→No-module) and `UI.md` §4.
    2. **INPUT:** `src/core/notifier.rs`, `spec/DEVICE_DISCOVERY.md` §3, `spec/UI.md` §4.
    3. **LOGIC:** Add `pub enum DeviceStatus { Connected(usize), NoModule, Disconnected }` (or the simpler `{ Connected, NoModule, Disconnected }` if per-board count isn't cheaply available without `classify_devices` — prefer the simpler form for P1; the "N Devices" pluralization in the spec can defer to P3's `classify_devices`). Add `pub fn device_status() -> DeviceStatus` deriving from the two existing booleans per the §2.1 table. Do **not** add per-path pinging here. Keep `is_device_connected()` (still used by the write-path/broadcast decision and the picker's Tier-1 pass).
    4. **OUTPUT:** `DeviceStatus` + `device_status()` in `notifier.rs` with unit tests for the three derivations (incl. the `is_device_connected && !host_capable` ⇒ `NoModule` case). No UI change yet (S2/S3).
    5. **DOCS (Mode A):** update the `device_status()`/`DeviceStatus` doc-comment to cite `DEVICE_DISCOVERY.md` §3 and explain the derivation from existing state (so a future reader doesn't assume a per-path ping).

- **P1.M1.T1.S2 — Render three-state status in `src/tray.rs` (macOS/Windows)** *(story points: 2)*
  - **CONTRACT:**
    1. **RESEARCH:** `src/tray.rs` — `UserEvent::DeviceStatus(bool)` @41, `device_status_text(connected: bool)` @660, the status-poll thread @380-405 (sends `UserEvent::DeviceStatus(connected)`), the event-loop arm @508-509 (`device_status_i.set_text(device_status_text(connected))`), and the synchronous first-paint @316. Target: `UI.md` §4 three-state table (● Connected / ⚠ No module / ○ Disconnected).
    2. **INPUT:** `DeviceStatus` from S1, `spec/UI.md` §4, `spec/DEVICE_DISCOVERY.md` §3.
    3. **LOGIC:** Change `UserEvent::DeviceStatus(bool)` → `UserEvent::DeviceStatus(DeviceStatus)`; rewrite `device_status_text` to a three-branch `fn device_status_text(DeviceStatus) -> String` (●/⚠/○ glyphs per the spec table); the poll thread calls `device_status()` instead of `is_device_connected()` and sends the enum (the `handshake_action`/`perform_handshake`/`reset` lifecycle stays keyed on the Tier-1 `is_device_connected()` transition, unchanged — do not gate the handshake on the three-state value). First-paint calls `device_status()`.
    4. **OUTPUT:** macOS/Windows tray line 2 is three-state. Single-threaded `cargo test --bin qmkonnect -- --test-threads=1` passes (per `AGENTS.md`). Note: the "No module" warning glyph must still be a `disabled` `MenuItem` (parity with today).
    5. **DOCS (Mode A):** the `device_status_text` doc-comment cites `UI.md` §4.

- **P1.M1.T1.S3 — Render three-state status in `src/linux_tray.rs` + the Disconnected→No-module one-shot `notify-send`** *(story points: 2)*
  - **CONTRACT:**
    1. **RESEARCH:** `src/linux_tray.rs` — `QmkTray.device_connected: bool` @68, the status-line build @138-154 (●/○), the icon-dim-on-disconnect @170, the poll thread @260+ (calls `is_device_connected()`, `handshake_action`, `handle.update`). Target: `DEVICE_DISCOVERY.md` §3 (three-state + the one-shot `notify-send` on the **Disconnected→No-module** transition with a link to `docs/qmk-integration.md`) and `UI.md` §4. Note Linux already shells out to `notify-send` elsewhere (the rules-parse-failure notification) — reuse that pattern; `notify-rust` is deliberately avoided (nested tokio runtime panics in ksni's handler thread, §7.3).
    2. **INPUT:** `DeviceStatus` from S1.
    3. **LOGIC:** Replace `device_connected: bool` with a `DeviceStatus` (or add a parallel field) on `QmkTray`; the menu line + icon-alpha branch on three states (⚠ glyph full-alpha for No-module; ○ dimmed for Disconnected; ● full-alpha for Connected). The poll thread tracks the previous `DeviceStatus` and, on a `Disconnected → NoModule` transition **only**, fires a one-shot `notify-send` (guarded so it fires once per entry into No-module, not every tick — mirror the `RULES_INVALID_NOTIFIED` once-guard pattern in `notifier.rs`). The handshake lifecycle stays keyed on the Tier-1 transition.
    4. **OUTPUT:** Linux SNI tray is three-state; the one-shot notification fires exactly once on the transition. `cargo test` + `cargo clippy --all-targets -- -D warnings` clean.
    5. **DOCS (Mode A):** the `QmkTray` status doc-comment cites `DEVICE_DISCOVERY.md` §3 + the one-shot rationale.

### Phase P2 — VIA coexistence guarantee (R-COEX, F14)

Make the already-true shared-open behavior an **asserted, documented invariant** (the load-bearing guarantee that the always-on QMKonnect never locks out VIA). Includes the research-gated write-narrowing decision (§2.2 — expected to defer).

#### Milestone P2.M1 — Shared-open invariant: comments + tests

**Task P2.M1.T1 — Assert R-COEX: non-seize opens, bounded reads, first-byte-`0x81` emission**

- **P2.M1.T1.S1 — Add R-COEX invariant comments at the open sites + invariant tests** *(story points: 2)*
  - **CONTRACT:**
    1. **RESEARCH:** QMKonnect links `hidapi = "2.6"` and opens via the crate's default `open_path` (non-seize everywhere: `FILE_SHARE_READ|WRITE` Windows, `kIOHIDOptionsTypeNone` macOS, plain `hidraw open` Linux — `DEVICE_DISCOVERY.md` §6.2). The app does **not** call any seize path; the `hidapi` crate exposes none. Reads are bounded drains (`IN_DRAIN_MAX = 32`) around writes — already the case (the typed-command path reads exactly one IN report per burst). The app's own first emitted payload byte is always `0x81` (the magic header). The crate source is at `~/.cargo/git/checkouts/qmk-notifier-*`; confirm `MatchKey`/`open_matching_devices` never pass a seize option. Target: `DEVICE_DISCOVERY.md` §6 (R-COEX requirement + the three must-preserve invariants) and `ARCHITECTURE.md` §10 #10.
    2. **INPUT:** `src/core/notifier.rs` (the `QmkNotifier::send_command`/`notify` open path), `spec/DEVICE_DISCOVERY.md` §6, `spec/ARCHITECTURE.md` §10.
    3. **LOGIC:** (a) Add a "R-COEX — must preserve" doc-comment at the `send_command`/transport boundary in `notifier.rs` stating: never introduce a seize/exclusive open; never a perpetual blocking read; the first payload byte is always `0x81`. (b) Add unit tests asserting (i) QMKonnect **never emits VIA-shaped bytes** — every constructed report's first payload byte is `0x81` (construct the `SendMessage`/`ApplyHostContext`/`QueryInfo` framing via the crate and assert; or assert on the app-side builders if the crate exposes them), and (ii) the read discipline issues **no read except bounded drains after a write** (a structural/static assertion or a comment-blocked invariant test documenting it). Where a behavior can't be unit-tested without HID hardware, add an inline invariant comment + a `// R-COEX:` marker so a future change can't silently violate it.
    4. **OUTPUT:** `notifier.rs` invariant comments + the emission test passing. No runtime behavior change (this phase *asserts* what's already true).
    5. **DOCS (Mode A):** the invariant comments *are* the Mode-A doc for this phase (they live in code). No `docs/*.md` change beyond what P4 covers.

- **P2.M1.T1.S2 — Research: can multi-board write-narrowing land app-side, or does it defer?** *(story points: 1)*
  - **CONTRACT:**
    1. **RESEARCH:** The crate's `MatchKey` (`core.rs:641`) is `{vid?, pid?, usage_page, usage}` — **not** path-keyed; `run()`/`send_command` open **all** devices matching the filter. There is no per-path send API. `DEVICE_DISCOVERY.md` §4.2/§8 want writes restricted to `kind == Capable`. Confirm whether the app can express "capable only" via VID/PID narrowing (only viable when capable boards have a distinct VID/PID from VIA boards — not generally true), and whether the common single-board case is already correct (VID/PID unset ⇒ write to all `0xFF60`; the one capable board receives it; any co-present VIA board ignores the magic — harmless, §2.2).
    2. **INPUT:** crate `core.rs`, `spec/DEVICE_DISCOVERY.md` §4/§8, `spec/PROTOCOL.md` §3.5.
    3. **LOGIC:** Produce a short findings note: either (a) **defer** — narrowing needs a coordinated crate API addition (path-scoped send or capability in `MatchKey`), out of scope for this one-repo delta, and is harmless today (VIA ignores magic) → record as a follow-up; or (b) **land a minimal app-side narrowing** if a clean mechanism exists. Default expectation: **defer**.
    4. **OUTPUT:** A one-page decision record in `plan/005_*/architecture/` (the breakdown agent's research dir). If "defer," no code change and P2 closes with S1 only; if "land," a follow-on subtask is added.
    5. **DOCS (Mode A):** none (research record only).

### Phase P3 — Discovered-device Settings picker (the elaborate F13 piece)

The new primary Settings surface (D1 §5): a live, self-populating list of `0xFF60` devices with ✓/✗ capability markers, plus an "Advanced / manual override" disclosure that relocates the existing VID/PID hex fields. Depends on a real `classify_devices()` (per-candidate ping) — the one place the crate constraint (§2.2/§2.3) bites.

#### Milestone P3.M1 — `classify_devices()` + cache, and the per-candidate classification mechanism

**Task P3.M1.T1 — Implement `classify_devices()` with a TTL cache, resolving the per-device ping mechanism**

- **P3.M1.T1.S1 — `ClassifiedDevice` / `DeviceKind` / `classify_devices()` / `CLASSIFICATION_CACHE`** *(story points: 3)*
  - **CONTRACT:**
    1. **RESEARCH:** `src/core/notifier.rs` `list_devices()` @129 enumerates via `hidapi::HidApi` (Tier-1: `usage_page==0xFF60 && usage==0x61` + optional vid/pid). The handshake (`perform_handshake` @348) sends `QUERY_INFO` via `n.send_command(RunCommand::QueryInfo, &filter)` and matches `CommandResponse::Info{proto_ver:2,…}` ⇒ capable. **The open question (§2.3):** `send_command` takes a `DeviceFilter` (vid/pid/page/usage), not a path, and the crate opens **all** matches — so classifying a *specific* candidate by path requires either (a) narrowing the filter to the candidate's vid/pid (works only if vid/pid is unique on the bus), or (b) a crate path-scoped send (crate change — out of scope), or (c) enumerating in-app and pinging each candidate with a filter narrowed to its vid/pid. Target: `DEVICE_DISCOVERY.md` §2 (`classify_devices`, `ClassifiedDevice`, `DeviceKind`, `CLASSIFICATION_CACHE`, `CLASSIFICATION_TTL` default 5 s) and §2.3's algorithm.
    2. **INPUT:** `src/core/notifier.rs`, `spec/DEVICE_DISCOVERY.md` §2.
    3. **LOGIC:** Add `pub enum DeviceKind { Capable { proto_ver, feature_flags, callback_count, board_rules_present }, NotQmkNotifier }` and `pub struct ClassifiedDevice { path, vendor_id, product_id, product_name: Option<String>, usage_page, usage, kind }`. Implement `classify_devices(verbose) -> Vec<ClassifiedDevice>`: enumerate Tier-1 candidates; for each, classify by sending `QUERY_INFO` and matching the `CommandResponse` (`Info{proto_ver:2}` ⇒ `Capable`; `Legacy`/`Timeout`/anything-else ⇒ `NotQmkNotifier` — a pure-VIA board Times Out, §2.2). Resolve the per-candidate ping via the cleanest in-app mechanism the research identifies (likely filter-narrow-by-vid/pid; document the multi-same-vid/pid limitation in a comment). Cache results in `CLASSIFICATION_CACHE: Mutex<HashMap<String,(DeviceKind,Instant)>>` keyed by `path`, TTL 5 s, invalidated on device disappearance. **No board is harmed** (VIA ignores magic; §2.2). Also feed the cache from the existing handshake so the status path stays single-ping-per-appearance.
    4. **OUTPUT:** `classify_devices()` + cache in `notifier.rs`, with unit tests over a fake/mock HID layer (capable / legacy / timeout ⇒ correct `DeviceKind`; cache hit/miss/TTL). The mock infra may need a small extension to return per-device responses — reuse the existing test `Notifier` mock pattern.
    5. **DOCS (Mode A):** `classify_devices` doc-comment cites `DEVICE_DISCOVERY.md` §2 and records the per-candidate mechanism chosen + its limitation.

#### Milestone P3.M2 — Three-platform picker + Advanced disclosure

**Task P3.M2.T1 — Discovered-device picker on Windows (Win32), macOS (NSAlert), and Linux (zenity/GTK)** *(story points: 5)*

- **P3.M2.T1.S1 — Windows Win32 picker (`LISTBOX`) + Advanced group box** *(story points: 2)*
  - **CONTRACT:**
    1. **RESEARCH:** `src/tray.rs` — `show_settings_dialog` @753, `DIALOG_RESULT: Mutex<Option<(Option<u16>,Option<u16>)>>` @51, control ids `1001`/`1002` (VID/PID `EDIT`) / `1003`/`1004` (OK/Cancel), `parse_id_field` @67, the save path via `render_config_body`. Target: `DEVICE_DISCOVERY.md` §5.1/§5.3 (picker rows = `product_name` + vid:pid + ✓/✗; one-capable-no-vid/pid ⇒ read-only "Detected: <name>" line, no picker; `[ Rescan ]` invalidates the cache) and the Advanced disclosure (§5.2 — the two `EDIT`s move under an "Advanced ▸" group box).
    2. **INPUT:** `classify_devices()` from P3.M1, `spec/DEVICE_DISCOVERY.md` §5.
    3. **LOGIC:** Add a `LISTBOX` (or `ListView`) of discovered devices above the VID/PID fields; populate from `classify_devices()` (✓/✗ from `kind`). Single-capable-board-no-vid/pid case ⇒ hide the list, show a static "Detected: <name>" line. `[ Rescan ]` button ⇒ `CLASSIFICATION_CACHE` clear + re-run. Relocate the two `EDIT`s under a group box "Advanced / manual override". Extend `DIALOG_RESULT` to `struct { chosen: Option<(u16,u16)>, manual: Option<(Option<u16>,Option<u16>)> }`; the save path applies `chosen` first (via `render_config_body`), else `manual`, else leaves VID/PID as-is. Keep `parse_id_field` shared.
    4. **OUTPUT:** Windows Settings dialog has the picker + Advanced disclosure; selecting a row writes that board's vid/pid.
    5. **DOCS (Mode A):** the dialog doc-comment cites `DEVICE_DISCOVERY.md` §5.

- **P3.M2.T1.S2 — macOS NSAlert picker (`NSStackView` of rows) + Advanced toggle** *(story points: 2)*
  - **CONTRACT:**
    1. **RESEARCH:** `src/tray.rs` — `show_macos_settings_dialog` (NSAlert + accessory `NSView` with two `NSTextField`s, `NSAutoreleasePool` wrap). Target: `DEVICE_DISCOVERY.md` §5.3 (an `NSStackView` of rows in the accessory view; an "Advanced" `NSButton` toggles the `NSTextField` pair).
    2. **INPUT:** `classify_devices()` from P3.M1, `spec/DEVICE_DISCOVERY.md` §5.
    3. **LOGIC:** Build the discovered-device rows (label = `product_name` + vid:pid + ✓/✗) in an `NSStackView`; selection writes the chosen vid/pid. Single-capable case ⇒ static "Detected:" line. "Advanced" toggle shows/hides the two `NSTextField`s. Extend the dialog result struct (parity with S1). `parse_id_field` shared.
    4. **OUTPUT:** macOS Settings dialog has the picker + Advanced toggle.
    5. **DOCS (Mode A):** dialog doc-comment cites `DEVICE_DISCOVERY.md` §5.

- **P3.M2.T1.S3 — Linux zenity `--list` picker + Advanced `--forms` (or GTK popup)** *(story points: 1)*
  - **CONTRACT:**
    1. **RESEARCH:** `src/linux_tray.rs` — `show_settings_dialog_linux` (zenity `--forms` with two entries; save ⇒ `write_config` + `apply_device_rule` via pkexec). Target: `DEVICE_DISCOVERY.md` §5.3 (a `zenity --list` for the discovered devices + a second `zenity --forms` for the Advanced vid/pid; or the native GTK popup already used for window-info). The save path's `apply_device_rule`/pkexec flow is unchanged.
    2. **INPUT:** `classify_devices()` from P3.M1, `spec/DEVICE_DISCOVERY.md` §5.
    3. **LOGIC:** Add a `zenity --list` (columns: device, vid:pid, capability) populated from `classify_devices()`; selecting a row returns its vid/pid. Keep the existing `--forms` as the "Advanced" manual entry. Extend the result handling (chosen first, else manual).
    4. **OUTPUT:** Linux Settings has the picker + Advanced.
    5. **DOCS (Mode A):** dialog doc-comment cites `DEVICE_DISCOVERY.md` §5.

### Phase P4 — Config/CLI cleanup + Mode-A documentation

Small finishing work: the `0xFEED` template cleanup (D3), the `--list-devices` kind column (D4), and the user-facing doc updates that ride with D1/D3.

#### Milestone P4.M1 — `0xFEED` cleanup, `--list-devices` kind column, Mode-A docs

**Task P4.M1.T1 — `0xFEED` → `0x????` template + `--list-devices` kind column + Mode-A doc sync**

- **P4.M1.T1.S1 — Config template `0xFEED` cleanup + `--list-devices` kind column** *(story points: 1)*
  - **CONTRACT:**
    1. **RESEARCH:** `src/core/mod.rs` — `render_default_config_template` @218 renders `# vendor_id  = 0xfeed   # unset: auto-discovery` / `# product_id = 0x0000` @238-239 (the historically-misread literal). `src/core/notifier.rs` `list_devices()` @129 prints `vid:pid page:usage product` only. `src/main.rs:114-115` dispatches `--list-devices`. Target: `DEVICE_DISCOVERY.md` §7.2 (template → `0x????` "unset: auto-discover any QMK keyboard (recommended)") and §8 (`--list-devices` gains a `kind` column from a one-shot `classify_devices()`).
    2. **INPUT:** `classify_devices()` from P3.M1 (for the kind column), `spec/DEVICE_DISCOVERY.md` §7.2/§8, `spec/CONFIG.md` §2 (`0xFEED cleanup` note).
    3. **LOGIC:** (a) In `render_default_config_template`, replace the `0xfeed`/`0x0000` literals with `0x????` + the "auto-discover any qmk_notifier-capable board (recommended)" comment (match `CONFIG.md` §2). (b) In `list_devices()`, after the Tier-1 enumerate, run `classify_devices()` once and print an added `kind` column (`qmk_notifier` / `qmk-only` etc.). Guard the classification so a `--list-devices` against no devices / no capable board still prints cleanly. Update any test asserting the old template text.
    4. **OUTPUT:** seeded config has no literal `0xfeed`; `--list-devices` shows capability. `cargo test --bin qmkonnect -- --test-threads=1` clean.
    5. **DOCS (Mode A):** the template comment *is* the user-facing doc here; no separate `docs/*.md` edit for the literal.

- **P4.M1.T1.S2 — Mode-A user-facing doc sync for the picker / Advanced / status** *(story points: 1)*
  - **CONTRACT:**
    1. **RESEARCH:** `docs/configuration.md` documents the VID/PID fields (and the `0xfeed` example — needs the same cleanup as S1); `docs/installation.md` / `docs/troubleshooting.md` mention `--list-devices` / VID-PID discovery. No user-facing doc yet describes the three-state status or the picker (they're new). The canonical source is `spec/DEVICE_DISCOVERY.md` §3/§5 (mirror it).
    2. **INPUT:** `spec/DEVICE_DISCOVERY.md` §3/§5, `spec/CONFIG.md` §1.1/§2.
    3. **LOGIC:** In `docs/configuration.md`: reframe `vendor_id`/`product_id` as an **Advanced override** (the picker writes them), and replace any `0xfeed` example with `0x????`. In `docs/installation.md` / `docs/troubleshooting.md`: mention the three-state status ("No module" ⇒ flash qmk_notifier) and the discovered-device Settings picker where VID/PID discovery is discussed. Keep edits minimal and mirror the spec wording.
    4. **OUTPUT:** `docs/configuration.md`, `docs/installation.md`, `docs/troubleshooting.md` consistent with F13/F14. Verify `grep -rn '0xfeed\|0xFEED' docs/ src/` returns zero hits (excluding `docs/vendor/`).
    5. **DOCS (Mode A):** this IS the Mode-A doc work.

#### Milestone P4.M2 — Changeset-level documentation (Mode B final sweep)

**Task P4.M2.T1 — Sync README.md + cross-cutting overviews for F13/F14** *(story points: 1)*

- **P4.M2.T1.S1 — Audit README.md and top-level overviews; regenerate `docs/llms_full.txt`** *(story points: 1)*
  - **CONTRACT:**
    1. **RESEARCH:** `README.md` and `docs/index.md` / `docs/usage.md` may have a one-line discovery/VID-PID blurb. `spec/` companions (`ARCHITECTURE.md`, `PROTOCOL.md`, `UI.md`, `HOST_RULES.md`, `DEVICE_DISCOVERY.md`) are **already at the target wording** (they ARE the spec this delta implements) — verify, don't rewrite. `docs/llms_full.txt` is a committed concatenation built by `docs/generate_llms_full.sh` (8 files); it must be regenerated after P4.M1 edits. Depends on all prior phases.
    2. **INPUT:** completed P1–P4.M1; `spec/*` (already correct).
    3. **LOGIC:** (a) `grep -rn 'false-green\|0xFF60\|VIA\|device.status\|list-devices\|vendor_id' README.md docs/index.md docs/usage.md` — if any describes discovery/VID-PID/status, update to the two-tier + Advanced-override + three-state framing; if it's a version-agnostic one-liner, leave it. (b) Run `bash docs/generate_llms_full.sh` from the repo root. (c) Verify `grep -rn 'classify_devices\|DeviceStatus\|No module' docs/llms_full.txt` shows the regenerated content and the `0xfeed` literal is gone.
    4. **OUTPUT:** README + overviews consistent with F13/F14 (or a no-op verification report with grep evidence); regenerated `docs/llms_full.txt`.
    5. **DOCS (Mode B):** this IS the changeset-level sweep. Runs LAST (depends on P1 + P3 + P4.M1).

---

## 4. Documentation Impact Summary

**Mode A — doc-with-work (rides with each requirement; not standalone tasks):**
- P1.M1.T1.S1 — `DeviceStatus`/`device_status()` doc-comment (`notifier.rs`) → cite `DEVICE_DISCOVERY.md` §3.
- P1.M1.T1.S2/S3 — `device_status_text` (`tray.rs`) and `QmkTray` status (`linux_tray.rs`) doc-comments → cite `UI.md` §4 / `DEVICE_DISCOVERY.md` §3.
- P2.M1.T1.S1 — R-COEX invariant comments *in code* at the transport boundary (`notifier.rs`) — these are the Mode-A doc for F14.
- P3.M1.T1.S1 — `classify_devices` doc-comment → cite `DEVICE_DISCOVERY.md` §2 + record the per-candidate mechanism + limitation.
- P3.M2.T1.S1/S2/S3 — per-platform Settings-dialog doc-comments → cite `DEVICE_DISCOVERY.md` §5.
- P4.M1.T1.S1 — config-template comment (the `0x????` rewrite) + `--list-devices` kind column.
- P4.M1.T1.S2 — `docs/configuration.md`, `docs/installation.md`, `docs/troubleshooting.md` (the explicit Mode-A user-facing doc sync).

**Mode B — changeset-level docs (final sweep, depends on all above):**
- P4.M2.T1.S1 — README.md + `docs/index.md`/`docs/usage.md` audit, and `docs/llms_full.txt` regeneration. The `spec/*.md` companions are **already at target** (they are the spec being implemented) and are verified, not rewritten.

**Already-correct, NOT edited:** `spec/DEVICE_DISCOVERY.md`, `spec/PRD.md`, `spec/ARCHITECTURE.md`, `spec/PROTOCOL.md`, `spec/UI.md`, `spec/CONFIG.md`, `spec/HOST_RULES.md` — these *are* the v0.2.8 target spec; this delta implements code + user-facing docs to match them. No spec doc is rewritten.