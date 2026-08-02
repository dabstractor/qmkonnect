# Bug Hunt Report — C8 Schema Unification (PRD end-to-end validation)

Scope: validate the implementation against the PRD/`HOST_RULES.md` contract after the
F11/F12 host-rules work and the C8 `[[rule]]` schema unification. Baseline before
hunting: `cargo test --bin qmkonnect -- --test-threads=1` → **338 passed, 0 failed**.

Findings are ordered by severity. Each has file:line, repro, and the spec it violates.

---

## 🔴 HIGH — Settings dialog silently clobbers non-VID/PID config fields

**Where:** `src/core/mod.rs:96` (`render_config_body(vendor_id, product_id)`) called
from the Settings save path on **all three platforms**:
- `src/tray.rs:869` (Windows: `show_settings_dialog`)
- `src/tray.rs:1259` (macOS: `show_macos_settings_dialog`)
- `src/linux_tray.rs:807` (`write_config`)

**Bug:** The Settings dialog reads the *full* `Config` (`parse_config`, e.g.
`src/tray.rs:759`) to pre-fill the VID/PID fields, but on **save** it rewrites the
entire `config.toml` via `render_config_body(vid, pid)`. That function emits only
`vendor_id`/`product_id` and hardcodes `usage_page`, `usage`, `debounce_ms`, and
`poll_interval_ms` as **commented-out defaults**. So any user-set value for those
four keys is silently reset the moment the user opens Settings and clicks Save —
even if they only changed (or didn't change) VID/PID.

**Repro (verified via probe round-trip):**
```toml
# before — user's config.toml
vendor_id  = 0x1234
product_id = 0x5678
usage_page = 0xff61      # board overrode RAW_USAGE_PAGE
debounce_ms = 120
poll_interval_ms = 250
```
Open Settings → Save (no edit) → `config.toml` becomes:
```toml
# usage_page = 0xff60     ← user's 0xff61 GONE
# usage      = 0x61
# debounce_ms = 50        ← user's 120 GONE
# poll_interval_ms = 0    ← user's 250 GONE
vendor_id  = 0x1234
product_id = 0x5678
```
Result: `configured_filter()` now matches the wrong usage page (the board stops being
discovered), and debounce/poll revert to defaults.

**Spec violated:** PRD §7 ("Configuration is hot … editing the file or saving the
Settings dialog takes effect … with no restart") implies saving the dialog must not
destroy unrelated config. PRD §8 documents all six keys as user-configurable.

**Fix direction:** the save path must preserve the existing non-VID/PID fields. Either
(a) read the current `Config`, overwrite only `vendor_id`/`product_id`, and serialize
the full struct, or (b) widen `render_config_body` to take all fields. The Linux
`write_config` (and the macOS/Windows handlers) should pass the current parsed config
through rather than reconstructing a VID/PID-only body.

---

## 🟡 LOW–MED — `--validate-rules` miscounts unified (combined) rules

**Where:** `src/main.rs:456-461`:
```rust
let layer_count = rs.rules.iter().filter(|r| r.layer.is_some()).count();
let cb_count    = rs.rules.iter().filter(|r| r.layer.is_none()).count();
println!("rules.toml valid: {} layer rules, {} callback rules.", layer_count, cb_count);
```

**Bug:** Under the unified `[[rule]]` schema a single rule may set **both** `layer`
**and** callbacks (the headline capability of C8). But the summary counts a rule as
*either* a layer rule *or* a callback rule based on `layer.is_some()`, so a combined
rule is bucketed only as a "layer rule" and its callbacks are invisible in the count.

**Repro (verified):** a single rule `match="kitty" layer=9 enable=["vim_lazy"]`
prints `rules.toml valid: 1 layer rules, 0 callback rules.` — the user wrote a
callback but is told there are zero callback rules.

**Spec violated:** the delta PRD's P1.M1.T1.S2 contract explicitly requires the listing
to reflect that "each rule can now carry both a `layer` and callbacks." The count
contradicts the unification it is supposed to report on.

**Fix direction:** count "rules with a layer", "rules with callbacks", and optionally
"combined rules"; or report `N rules (M with layer, K with callbacks)`. The simplest
honest summary is just `{} rules.` plus the breakdown of how many set layer / callbacks.

---

## 🟢 LOW — Hyprland `activewindow` event handler does not dedup

**Where:** `src/platforms/hyprland.rs:450` `handle_window_state_change` (used by the
`activewindow`, `windowclosed`, layer open/close, and workspace handlers) vs.
`src/platforms/hyprland.rs:380` `poll_window_state` (which *does* dedup at :408).

**Bug:** `handle_window_state_change` unconditionally updates `last_window_state` and
calls `notify_qmk` without comparing to the previous state. `poll_window_state`
explicitly computes `window_changed` and skips identical states. So a spurious or
duplicate `activewindow` event for the *currently-focused* window (which Hyprland can
emit in focus-management edge cases, and which `windowclosed`/layer handlers also
trigger) re-sends the same payload. Because the first notification of a burst is sent
immediately (PRD §6.3) rather than debounced, the same window can be transmitted twice
to the keyboard.

**Impact:** mostly harmless (the keyboard's desired-set diff is idempotent for an
identical `ApplyHostContext`), but it wastes USB bandwidth and can re-fire
`on_enable` callbacks on firmware that doesn't dedup. The `last_window_state` it
already maintains makes the dedup a one-line guard.

**Fix direction:** mirror `poll_window_state`'s comparison in
`handle_window_state_change` (skip when the resolved `(class,title)` equals
`last_window_state`).

---

## ⚪ OBSERVATIONS / GAPS (not regressions from C8)

1. **Malformed `config.toml` silently falls back to defaults.** `configured_filter()`
   (`src/core/notifier.rs`) and `configured_timing()` (`src/core/mod.rs`) both do
   `.and_then(|p| parse_config(&p).ok())`, swallowing parse errors. PRD §2.1 goal 4
   ("Typo'd config? Probe once at startup and say so clearly") is only half-met:
   `startup_device_probe` warns when the *device* is missing but never reports that
   `config.toml` failed to parse (it just uses default usage page/usage). There is no
   `--validate-config`. Low severity / graceful-degradation by design.

2. **Desktop notification for a malformed `rules.toml` fires only on the first window
   change** (`host_context_for_window`, `src/core/notifier.rs`), not at handshake time.
   If no window changes after the breakage, the user sees only a stderr warning, not
   the `notify()` desktop popup the spec (HOST_RULES.md §7) calls for. Minor timing gap.

3. **`platforms::notify` on Windows** uses a modal `MessageBoxW` (blocking, requires a
   click) rather than a transient toast — documented as a "dep-free stand-in." Intrusive
   for a background tray app. UX, not correctness.

4. **Crate-level (not QMKonnect):** `qmk-notifier` `build_command_data` clamps
   `callbacks.len().min(255)` for the count byte but still `extend_from_slice(callbacks)`
   the full vector (`f26893e/src/core.rs:~545`). If >255 callbacks were ever sent, the
   count byte would lie and the firmware would parse-drift. Unreachable in practice —
   QMKonnect caps the registry sweep at `MAX_HOST_CALLBACKS=64` — so it is a latent
   crate bug, not a QMKonnect defect.

---

## What was verified CORRECT (no bug)

- Wire contract: `ApplyHostContext { layer: None }` → `0xFF`, `clear_board` → flags
  bit 0 (crate `core.rs` + its tests).
- `evaluate()` one-pass semantics: first-match-wins layer (exclusive), all-match
  callbacks, order-independent `disable` exclusion (two-set difference), C13 no-match
  (`clear_board:false`), C11 `0xFF` rejection, §9 validity (≥1 of layer/enable/disable).
- Stack-vs-replace dispatch (`dispatch_window_send`): string-before-context ordering,
  replace suppresses the string, no-match always sends the string (board silo runs).
- Handshake lifecycle: `handshake_action` transition table; `BOARD_HAS_RULES` set before
  `HOST_CAPABLE` (no stale-read window); `HAS_HANDSHAKED` dedup with transient
  Timeout/error release; `CALLBACK_SWEEP_DEADLINE` bounds the mutex hold.
- Pattern matcher: wrong-arity `match` arrays / non-string values are clean serde
  errors; `layer = 0..=254` accepted, `255` rejected; empty-core short-circuit.
- Runner handshake ordering is identical and safe on all three platforms
  (handshake → monitor.start → tray).
- udev rendering is a single safe line with a leading match key (issue #4 resolved);
  X11 monitor is a real `xprop` implementation (issue #14 resolved); debounce is a
  single worker thread (issue #11 resolved); `debounce_ms = 0` correctly disables
  debouncing.