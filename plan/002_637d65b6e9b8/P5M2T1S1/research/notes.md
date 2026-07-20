# Research Notes — P5.M2.T1.S1: Add "Reload rules" MenuItem to `src/tray.rs` (macOS/Windows)

> **Scope:** `src/tray.rs` ONLY (the macOS/Windows — and fallback-X11-Linux — tray
> built on `tray-icon` + `tao`). Adds one `MenuItem` ("Reload rules"), a
> background-thread handler that re-reads `rules.toml` + force-refreshes the
> capability handshake, a new `UserEvent` variant to report the outcome back to
> the event-loop thread, and the file's first `#[cfg(test)] mod tests`.
> **No Cargo, no notifier.rs, no rules.rs, no platforms/, no linux_tray.rs, no
> runners/** (`setup_tray` already takes `verbose`, so the runners are untouched).
>
> **Environment note:** at research time P4 is fully COMPLETE — the handshake API
> is MERGED in notifier.rs and `cargo check --bin qmkonnect` is CLEAN. The earlier
> v0.3.0-tag / handshake-not-merged blockers that gated prior planning are RESOLVED.

---

## §0 — Dependency contracts (all MERGED + verified at research time)

### §0.1 `parse_rules` + `get_rules_paths` (P3.M1 LANDED) — `src/core/rules.rs`
```
L230  pub fn parse_rules(path: &Path) -> Result<RuleSet, Box<dyn Error>>
L248  pub fn get_rules_paths() -> Vec<PathBuf>
L68   pub struct RuleSet { host, layer_rules: Vec<LayerRule>, callback_rules: Vec<CallbackRule> }
```
- `parse_rules` is STRICT: a missing `match`/`layer` field or malformed TOML ⇒ `Err`
  (this strictness IS the validation). Absent file is not an error at the
  `get_rules_paths` level — it returns candidates; the caller checks `.exists()`.
- `RuleSet.layer_rules.len()` / `.callback_rules.len()` give the counts we log.

### §0.2 The capability handshake (P4.M2.T1.S1 MERGED) — `src/core/notifier.rs`
Verified public surface (exact signatures + line numbers in the MERGED file):
```
L171  pub fn is_device_connected() -> bool
L265  pub fn perform_handshake(verbose: bool)        // idempotent (HAS_HANDSHAKED swap @ L267)
L441  pub fn host_capable() -> bool                  // reads AtomicBool HOST_CAPABLE
L448  pub fn callback_names() -> HashMap<String,u8>  // clones Mutex<HashMap> CALLBACK_NAMES
L457  pub fn reset_handshake_state()                 // clears HOST_CAPABLE/BOARD_HAS_RULES/CALLBACK_NAMES/HAS_HANDSHAKED
```
- **Idempotency (verified, L266-273):** `perform_handshake` does
  `if HAS_HANDSHAKED.swap(true, SeqCst) { return; }` on entry ⇒ a no-op if already
  handshooked this session. To FORCE a refresh on "Reload rules" you MUST call
  `reset_handshake_state()` first (clears HAS_HANDSHAKED) — this is the documented
  device-transition re-trigger path (reset's doc comment, ~L451).
- **It also validates callback names internally (L335):** on the capable path,
  `perform_handshake` calls `validate_rules_callback_names(verbose)` (warns on
  unknown `rules.toml` names — never fatal). So "Reload rules" re-validates names
  for free when it triggers the handshake.
- **Thread-safety:** HOST_CAPABLE/HAS_HANDSHAKED/BOARD_HAS_RULES are `AtomicBool`;
  CALLBACK_NAMES is `Mutex<HashMap>`. The sweep collects names locally and
  publishes after `drop(n)` (the NOTIFIER lock) — no nested locks. Safe to call
  from a background thread. **`perform_handshake` does BLOCKING HID I/O** (QueryInfo
  → SetOs → a QueryCallback sweep; can total 1–5 s with timeouts) ⇒ MUST run off
  the event-loop thread or it freezes the tray.
- Extra lifecycle helper (also MERGED, `src/core/notifier.rs` ~L470):
  `handshake_action(last: Option<bool>, connected: bool) -> HandshakeAction`
  (`None`/`Gain`/`Loss`) — designed for device-presence TRANSITIONS, NOT for an
  explicit user reload. "Reload rules" uses the direct `reset + perform_handshake`
  path instead (D4), because it wants a forced refresh regardless of transition.

### §0.3 Build precondition — RESOLVED
`cargo check --bin qmkonnect` finishes CLEAN at research time (P1.M1.T4.S1 tagged
v0.3.0; P4.M1.T2.S1 pinned it; the dependency resolves). The earlier "failed to
find tag v0.3.0" blocker is gone. No halt-condition.

---

## §1 — `src/tray.rs` verbatim CURRENT anchors (2350 lines; the file THIS task edits)

### §1.1 The `UserEvent` enum (L36-49) — EDIT: add one variant
```rust
enum UserEvent {
    MenuEvent(MenuEvent),
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    DeviceStatus(bool),
    #[cfg(target_os = "macos")]
    AutostartSync,
}
```
- Variants travel through `tao::event_loop::EventLoopProxy<UserEvent>` from
  background threads to the event loop. A new variant's payload must be `Send`.

### §1.2 `setup_tray(verbose: bool)` (L250) — verbose IS plumbed (runners pass `self.verbose`)
- Menu items created L285-334. Each `MenuItem::new(label, enabled, None)`:
  `settings_i` (L285), `quit_i` (L286); `device_status_i` (L311, mac/win, disabled);
  `window_info_i` (L321, mac/win); `about_i`/`sep_*` (PredefinedMenuItem).
- `menu_items: Vec<&dyn tray_icon::menu::IsMenuItem>` built L336-363 in this order:
  ```
  about_i; [device_status_i mac/win]; sep_about;
  [launch_at_login_i mac]; settings_i; [open_at_login_i win];
  { sep_wininfo; window_info_i } mac/win;       ← the "separator" the item means
  sep_before_quit; quit_i;
  ```
- `tray_menu.append_items(&menu_items)` (L365).
- `proxy` (L274, `EventLoopProxy<UserEvent>`) is captured by the `event_loop.run`
  move-closure (used in the Init arm: `autostart_first_run_default_on(proxy.clone())`
  on macOS, L457) ⇒ `proxy.clone()` is available inside every event-loop arm. ✓
- `verbose` is `Copy` (bool) and is captured by BOTH the poll-thread spawn closure
  (used at L387) AND the `event_loop.run` closure ⇒ `verbose` is in scope inside the
  MenuEvent arm. ✓ (No need to plumb anything; no runner edits.)

### §1.3 The `MenuEvent` arm (L461) — uses if/else-if + separate cfg `if` blocks
```rust
Event::UserEvent(UserEvent::MenuEvent(event)) => {
    if event.id == quit_i.id() { ... }
    else if event.id == settings_i.id() { handle_settings_click(); }
    // then cfg-wrapped `if event.id == window_info_i.id() { handle_window_info_click(); }`
    // then cfg-wrapped launch_at_login / open_at_login handlers
}
```
- `handle_settings_click` / `handle_window_info_click` are free helper fns
  (sibling pattern to follow for `do_reload_rules`).

### §1.4 The result-delivery arms (L493-507) — the exemplar for our new arm
```rust
#[cfg(any(target_os = "macos", target_os = "windows"))]
Event::UserEvent(UserEvent::DeviceStatus(connected)) => {
    device_status_i.set_text(device_status_text(connected));   // mutate item on the loop thread (ONLY safe place)
}
#[cfg(target_os = "macos")]
Event::UserEvent(UserEvent::AutostartSync) => {
    launch_at_login_i.set_checked(autostart::is_enabled());
}
_ => {}
```

### §1.5 The async background-thread exemplar (L374-397) — COPY this pattern
```rust
#[cfg(any(target_os = "macos", target_os = "windows"))]
{
    let status_proxy = proxy.clone();
    std::thread::spawn(move || {
        let mut last: Option<bool> = None;
        loop {
            let connected = crate::core::notifier::is_device_connected();
            if last != Some(connected) {
                match crate::core::notifier::handshake_action(last, connected) {
                    crate::core::notifier::HandshakeAction::Gain => {
                        crate::core::notifier::perform_handshake(verbose);   // ← handshake on a BG thread (non-blocking to UI)
                    }
                    crate::core::notifier::HandshakeAction::Loss => {
                        crate::core::notifier::reset_handshake_state();
                    }
                    crate::core::notifier::HandshakeAction::None => {}
                }
                last = Some(connected);
                let _ = status_proxy.send_event(UserEvent::DeviceStatus(connected));
            }
            std::thread::sleep(std::time::Duration::from_secs(3));
        }
    });
}
```
- This PROVES the pattern this task needs: spawn a thread → do blocking
  `perform_handshake` there (off the event loop) → deliver state back via
  `proxy.send_event(UserEvent::…)`. `EventLoopProxy` is `Send + Sync + Clone`
  (tao/winit docs: "wake up an EventLoop from another thread"); tray-icon docs:
  "forward events to the event loop by using EventLoopProxy."

---

## §2 — Where `tray.rs` actually compiles/runs (cfg map) — affects validation

File header: `#![cfg(not(all(target_os = "linux", feature = "hyprland")))]`.
`Cargo.toml` features (L100): `default = ["hyprland", "macos", "linux-tray"]`.

| Platform | Tray used at runtime | `tray.rs` compiled? |
|---|---|---|
| macOS | `tray::setup_tray(self.verbose)` (runners/macos.rs) | yes |
| Windows | `tray::setup_tray(self.verbose)` (runners/windows.rs) | yes |
| Linux + `hyprland` (default) | `linux_tray::spawn()` (SNI) | **no** |
| Linux + not-hyprland + `linux-tray` (default X11) | `linux_tray::spawn()` (SNI) | **no** |
| Linux + not-hyprland + not-`linux-tray` (`--no-default-features`) | `tray::setup_tray()` (runners/linux.rs) | **yes** |

**Consequence for the DEV MACHINE (Linux):** with default features, `tray.rs` is
cfg'd OUT — `cargo build`/`cargo test` on this box compiles `linux_tray.rs`, not
`tray.rs`. So:
- To **compile-check** `tray.rs` edits on Linux:
  `cargo build --no-default-features --bin qmkonnect` (drops hyprland + linux-tray
  ⇒ `tray.rs` compiles; `linux_tray.rs` is cfg'd out).
- To **fully test** (run tray.rs tests + click the item): macOS or Windows
  (per AGENTS.md dev loops). The new `#[cfg(test)] mod tests` (pure
  `format_reload_result`) runs wherever tray.rs compiles — incl. `--no-default-features` on Linux.

---

## §3 — How "Reload rules" actually takes effect (why there's no rules-cache to update)

P4.M3.T1.S1's `host_context_for_window` (MERGED in notifier.rs) reads `rules.toml`
**LIVE** on every debounced window change:
```rust
let path = crate::core::rules::get_rules_paths().into_iter().find(|p| p.exists())?;
let rules = crate::core::rules::parse_rules(&path)?;   // re-read each evaluation
```
⇒ There is NO global `RuleSet` cache in notifier.rs. The debounce worker always
sees the current file. So **"Reload rules" does NOT need to push a RuleSet into
any cache** — the file change is picked up automatically on the next window event.

The TWO things "Reload rules" actually provides:
1. **Immediate validation feedback** (parse_rules success/error + rule counts) — so
   the user knows their edit is valid without waiting for a window change.
2. **A forced callback-table refresh** (`reset_handshake_state()` +
   `perform_handshake(verbose)`) — repopulates `CALLBACK_NAMES` from the firmware,
   useful if the initial handshake timed out, the device connected late, or the
   user suspects a stale table. (perform_handshake also re-validates callback names
   internally on the capable path — §0.2.)

This matches the item description verbatim: "re-read rules.toml via parse_rules
(log success/error), if HOST_CAPABLE and device present, re-run perform_handshake
to refresh CALLBACK_NAMES."

---

## §4 — Design decisions (D1-D7)

- **D1 — Ungated `reload_rules_i`.** Created with no `cfg` (matches the literal
  item line `MenuItem::new("Reload rules", true, None)` and the UNGATED
  `settings_i`/`quit_i` precedent). Shows on macOS, Windows, AND the fallback
  X11-Linux build. Only `device_status_i`/`window_info_i` are mac/win-gated
  (they're mac/win-specific UI); "Reload rules" is a general action like Settings.
  (Sibling P5.M2.T1.S2 covers the SNI `linux_tray.rs` path; the fallback X11 path
  gets it here for free.)
- **D2 — Async (background thread + new `UserEvent::RulesReloaded`).** The
  handshake does BLOCKING HID I/O (1-5 s) — running it on the event-loop thread
  freezes the tray (beachball on macOS). The codebase ALREADY runs
  `perform_handshake` on a background thread (the poll thread, §1.5). The item
  explicitly suggests "Consider a UserEvent variant for async reload status
  feedback." ⇒ one `std::thread::spawn` runs `do_reload_rules()`, then
  `proxy.send_event(UserEvent::RulesReloaded(result))`; the event-loop arm logs it.
- **D3 — `ReloadResult` is a small `Send` struct** carried by the UserEvent:
  `{ rules_ok: bool, rules_detail: String, capable: Option<bool>,
  callback_count: Option<usize> }`. All fields `Send` ⇒ struct auto-`Send` ⇒
  crosses the EventLoopProxy. `capable`/`callback_count` are `Option` (`None` when
  no device ⇒ handshake skipped).
- **D4 — `reset_handshake_state()` BEFORE `perform_handshake(verbose)`.** The
  handshake is idempotent (verified: HAS_HANDSHAKED swap @ notifier.rs L267).
  Without reset, "re-run perform_handshake" is a no-op (CALLBACK_NAMES unchanged).
  Reset forces a fresh QueryCallback sweep — the documented re-trigger path.
  Gated on `is_device_connected()` (skip the handshake when no device).
- **D5 — `perform_handshake(verbose)` with the PLUMBED verbose.** `setup_tray(verbose)`
  exists (L250); the runners already pass `self.verbose`. So the reload handler
  uses the in-scope `verbose` (respects the user's `-v` flag) — do NOT hardcode.
  The event-loop handler ALSO logs a clean summary from `host_capable()` +
  `callback_names().len()`.
- **D6 — Scope is `tray.rs` ONLY.** `setup_tray` already takes `verbose`, so the
  runner call-sites need NO change. Every edit lands in `src/tray.rs`.
- **D7 — `format_reload_result(&ReloadResult) -> String` is the pure testable
  core.** `do_reload_rules` (IO + HID) and the event-loop wiring are GUI/runtime
  code validated manually (build + click). The formatting helper is pure ⇒ the
  file's FIRST `#[cfg(test)] mod tests` (mirrors P5.M1's pure-helper approach).

---

## §5 — Gotchas (G1-G8)

- **G1 — Build is CLEAN (RESOLVED).** `cargo check --bin qmkonnect` passes at
  research time. No v0.3.0 halt-condition. (If it ever regresses with "failed to
  find tag v0.3.0", that's an env issue — halt, do NOT touch Cargo.toml.)
- **G2 — Handshake is MERGED (RESOLVED).** `perform_handshake`/`host_capable`/
  `callback_names`/`reset_handshake_state` exist in notifier.rs (L265/L441/L448/L457).
  Call via `crate::core::notifier::`. Do NOT reimplement.
- **G3 — `MenuItem` is `!Send`** (`Rc<RefCell<…>>` inside muda/tray-icon). It is
  created, `.id()`-compared, and (if ever) mutated ONLY inside `event_loop.run`
  (the event-loop thread). Our reload handler never touches a `MenuItem` on the
  worker thread — it only reads `reload_rules_i.id()` on the loop thread to match
  the click, then spawns a thread that does pure data work. ✓
- **G4 — NEVER run the handshake on the event-loop thread.** Blocking HID I/O ⇒
  tray freeze / macOS beachball. Always `std::thread::spawn` it (D2) — exactly as
  the poll thread already does (§1.5).
- **G5 — `proxy` AND `verbose` are in scope inside the run-closure.** Both are
  captured by `event_loop.run(move |…|)` (`proxy` used in Init; `verbose` is
  `Copy`). `proxy.clone()` for the spawned reload thread is the same move the
  DeviceStatus thread makes.
- **G6 — `reset_handshake_state()` mid-flight is an acceptable transient.** While
  reload resets state, the debounce worker may briefly see capable=false and fall
  back to string-only for ONE window; the next window re-evaluates fresh. The poll
  thread also calls `perform_handshake` on a device Gain — if a Gain and a
  user-reload race, the worst case is two back-to-back handshakes (idempotent
  outcome; the NOTIFIER Mutex serializes per-message HID writes). Harmless.
- **G7 — dev box is Linux; `tray.rs` is cfg'd out by default.** `cargo test` on
  this box runs `linux_tray.rs` tests, NOT tray.rs. Compile-check tray.rs via
  `--no-default-features`; full validation (click the item) on macOS/Windows
  (AGENTS.md loops). `--test-threads=1` is MANDATORY for the whole bin (AGENTS.md;
  shared globals in other bin tests).
- **G8 — binary-only crate; Mode-A rustdoc uses ` ```rust,ignore ` fences**
  (no `cargo test --doc` on a bin). Match rules.rs/pattern.rs/notifier.rs.

---

## §6 — Test plan (4 pure tests in tray.rs's FIRST `#[cfg(test)] mod tests`)

`format_reload_result(&ReloadResult) -> String` returns a 2-line summary; test:
1. `test_format_reload_parse_error` — `{rules_ok:false, detail:"rules.toml invalid: …",
   capable:None, callback_count:None}` ⇒ contains the error detail + "no device …".
2. `test_format_reload_valid_no_device` — `{rules_ok:true, detail:"rules.toml valid: 1 layer, 2 callback",
   capable:None, …}` ⇒ contains the detail + "no device connected — handshake skipped.".
3. `test_format_reload_valid_capable` — `{rules_ok:true, detail:"…valid…", capable:Some(true),
   callback_count:Some(3)}` ⇒ contains "handshake OK" + "3 callback".
4. `test_format_reload_valid_legacy` — `capable:Some(false)` ⇒ contains
   "legacy/timeout (string-only mode).".
(Tests assert stable substrings that are part of the user-facing contract.)

`do_reload_rules` (IO + HID) and the event-loop wiring are validated MANUALLY
(build + click + observe log) — not unit-testable without mocking HID.

---

## §7 — Validation (project dev loop, AGENTS.md)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect                    # macOS/Windows: tray.rs compiles here. CLEAN at research time.
cargo test --bin qmkonnect -- --test-threads=1 # MANDATORY single-threaded (AGENTS.md).
# On the Linux dev box (tray.rs cfg'd out by default), compile-check it via:
cargo build --no-default-features --bin qmkonnect
git diff --stat                                # expect src/tray.rs ONLY.
```
Manual (macOS or Windows, per AGENTS.md build/install loop): launch the app →
tray menu → click "Reload rules" → observe the log line (valid+counts, or parse
error; handshake OK/legacy/no-device). Confirm the item sits between Settings
and the Show-Window-Information separator.