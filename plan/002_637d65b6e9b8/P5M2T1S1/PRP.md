# PRP — P5.M2.T1.S1: Add "Reload rules" `MenuItem` to `src/tray.rs` (macOS / Windows)

> **Repo under change:** the **qmkonnect** desktop app (Rust binary) at
> `/home/dustin/projects/qmkonnect`. This 1-point task adds a single **"Reload
> rules"** menu item to the macOS/Windows tray (`src/tray.rs`) — PRD §7 of the
> host-rules spec (`h2.86`): *"Tray/UX: add 'Reload rules' to all three menus
> (re-read `rules.toml`, re-validate, re-handshake if needed)."* The Linux SNI
> menu is the sibling task **P5.M2.T1.S2** (`src/linux_tray.rs`); this task does
> the macOS/Windows half.
>
> **File touched (1):** `src/tray.rs` — one new `MenuItem`, one new
> `UserEvent::RulesReloaded` variant + small `ReloadResult` payload struct, a
> background-thread worker (`do_reload_rules`) + a pure formatter
> (`format_reload_result`) + the event-loop wiring (click arm + result arm), and
> the file's first `#[cfg(test)] mod tests` (4 pure tests). **No Cargo, no
> notifier.rs, no rules.rs, no platforms/, no linux_tray.rs, no main.rs, no
> runners** (`setup_tray(verbose)` already exists — the runners already pass
> `self.verbose`, so they are untouched).
>
> **Consumes (all MERGED + verified at research time):**
> `rules::parse_rules` / `rules::get_rules_paths` / `RuleSet` (P3.M1 — LANDED);
> `perform_handshake` (notifier.rs L265) / `host_capable` (L441) / `callback_names`
> (L448) / `reset_handshake_state` (L457) / `is_device_connected` (L171)
> (P4.M2.T1.S1 — MERGED). Re-uses the **existing** `EventLoopProxy` + `UserEvent`
> + background-thread pattern already proven in this file (the device-status poll
> thread, `tray.rs` L374-397, which already runs `perform_handshake` off the
> event loop).
>
> **Consumed downstream by:** P5.M2.T1.S2 (Linux SNI "Reload rules" — mirrors the
> same semantics) and P6 docs.
>
> **Build status:** `cargo check --bin qmkonnect` is CLEAN at research time (the
> earlier v0.3.0-tag blocker is resolved; P4 is fully merged). No halt-condition.
>
> **PARALLEL items (zero conflict):**
> - **P5.M1.T1.S1** edits `src/main.rs` + `src/core/mod.rs` (CLI flags). This
>   task edits `src/tray.rs`. **Zero file overlap.**
> - **P4.M2 / P4.M3** edited `src/core/notifier.rs` (now MERGED). This task is a
>   **read-only consumer** of notifier.rs's public API. **Zero overlap.**
> - **P5.M2.T1.S2** edits `src/linux_tray.rs`. **Zero overlap** (different tray
>   module; this PRP defines the shared reload semantics S2 should mirror).

---

## ⚠ READ FIRST — three non-obvious traps

1. **NEVER run `perform_handshake` on the event-loop thread.** It does BLOCKING
   HID I/O (QueryInfo → SetOs → a QueryCallback sweep — potentially 1-5 s with
   timeouts). Running it on the GUI main thread freezes the tray and causes a
   macOS beachball. The reload + handshake MUST run on a `std::thread::spawn`'d
   background thread, reporting back via a new `UserEvent::RulesReloaded` variant
   — the exact pattern the device-status poll thread already uses (it runs
   `perform_handshake(verbose)` on its own thread at `tray.rs` L387), and the one
   the item description anticipates: *"Consider a UserEvent variant for async
   reload status feedback."* See D2/G4.

2. **`reset_handshake_state()` MUST come before `perform_handshake()` — otherwise
   "Reload rules" does nothing.** `perform_handshake` is **idempotent**: its first
   line is `if HAS_HANDSHAKED.swap(true, SeqCst) { return; }` (notifier.rs L266-273).
   So calling `perform_handshake` again after the startup handshake is a **no-op**
   and `CALLBACK_NAMES` stays stale. To force the refresh the user expects on
   "Reload rules", call `reset_handshake_state()` first (it clears `HAS_HANDSHAKED`),
   then `perform_handshake(verbose)`. This is the documented device-transition
   re-trigger path. See D4.

3. **Use the plumbed `verbose`, don't hardcode it.** `setup_tray(verbose: bool)`
   (L250) already receives the user's `-v` flag (the runners pass `self.verbose`).
   Both the poll-thread closure and the `event_loop.run` closure capture `verbose`
   (it's `Copy`), so it is **in scope inside the `MenuEvent` arm**. Pass it to
   `perform_handshake(verbose)`. Do NOT add a `verbose` parameter, and do NOT
   touch the runners (scope is strictly `src/tray.rs`). See D5/D6.

---

## Goal

**Feature Goal**: Add a **"Reload rules"** menu item to the macOS/Windows tray so
a user can, after editing `rules.toml`, re-validate it and force-refresh the
firmware's callback-name table without restarting the app — getting immediate,
human-readable feedback (valid + rule counts / parse error / handshake outcome)
in the app log.

**Deliverable** (1 file — `src/tray.rs`):
- One new `MenuItem` (`reload_rules_i`), placed in the "prefs" group right after
  **Settings** (and after "Open at Login" on Windows), before the separator that
  precedes "Show Window Information…".
- One new `UserEvent::RulesReloaded(ReloadResult)` variant + the small `Send`
  `ReloadResult` payload struct (rules parse outcome + handshake outcome).
- A background-thread worker `fn do_reload_rules(verbose: bool) -> ReloadResult`
  (re-reads `rules.toml` via `parse_rules`; if a device is connected,
  `reset_handshake_state()` + `perform_handshake(verbose)` to force-refresh
  `CALLBACK_NAMES`).
- A pure `fn format_reload_result(&ReloadResult) -> String` (the testable core).
- Event-loop wiring: a `reload_rules_i.id()` click arm that spawns the worker, and
  a `UserEvent::RulesReloaded` arm that logs the formatted result.
- The file's first `#[cfg(test)] mod tests` (4 pure tests on `format_reload_result`).

**Success Definition**:
- `cargo build --bin qmkonnect` is clean (it already is at research time).
- The tray menu shows **"Reload rules"** between Settings and the Show-Window-Info
  separator, on macOS and Windows.
- Clicking it (with a connected v2 board) logs e.g.
  `Reload rules: rules.toml valid: 2 layer rule(s), 3 callback rule(s).` +
  `Reload rules: handshake OK — 5 callback(s) discovered.`
- With a malformed `rules.toml` it logs the parse error (and still attempts the
  handshake if a device is present). With no device it logs
  `no device connected — handshake skipped.`
- `cargo test --bin qmkonnect -- --test-threads=1` is green (4 new + all existing).
- `git diff --stat` = `src/tray.rs` ONLY.

## User Persona (if applicable)

**Target User**: the end user who edits `rules.toml` (adds/renames callback rules,
toggles `disable_firmware_config`) and the developer integrating a new keyboard.

**Use Case**: "I just edited my `rules.toml`. Before trusting it, I click 'Reload
rules' to confirm it parses and to refresh the firmware's callback table — without
quitting and relaunching QMKonnect."

**User Journey**: edit `rules.toml` in an editor → click tray "Reload rules" →
read the log (valid + counts, or the parse error; handshake OK/legacy/no-device) →
the next window switch uses the new rules + refreshed callbacks automatically.

**Pain Points Addressed**: today there is NO way to re-validate `rules.toml` or
re-trigger the handshake short of restarting the whole app; a typo silently
disables host rules with no feedback.

## Why

- **PRD §7 (`h2.86`)** — "Reload rules" is the canonical tray contract for the
  host-rules feature (re-read, re-validate, re-handshake).
- **PRD §1.1 (`h3.54`)** — the macOS/Windows menu layout this item extends.
- **PRD §8(8) (`h2.86`)** (backward compat) — reload is purely additive; with no
  `rules.toml` it logs "host rules disabled" and changes nothing.
- **Complements** P5.M1.T1.S1's `--validate-rules` CLI (same `parse_rules` path)
  with an in-app, no-terminal equivalent.
- **Unblocks** P5.M2.T1.S2 (Linux SNI mirrors the same reload semantics) and P6 docs.

## What

A single new menu item + its click-to-background-thread-to-event-loop pipeline.
No change to the runner, notifier, rules evaluator, debounce, platforms, or any
packaging. The only observable behavior change is the new menu entry and the log
lines it produces on click.

### Success Criteria
- [ ] `reload_rules_i = MenuItem::new("Reload rules", true, None)` exists, ungated,
      pushed into `menu_items` after `settings_i` (+ `open_at_login_i` on Windows)
      and before the `sep_wininfo`/`window_info_i` block.
- [ ] `UserEvent::RulesReloaded(ReloadResult)` variant + `ReloadResult` struct
      (`rules_ok: bool`, `rules_detail: String`, `capable: Option<bool>`,
      `callback_count: Option<usize>`) exist; `ReloadResult` is auto-`Send`.
- [ ] Click arm: `if event.id == reload_rules_i.id()` → `std::thread::spawn`
      running `do_reload_rules(verbose)`, then `proxy.clone().send_event(
      UserEvent::RulesReloaded(result))`. The handshake NEVER runs on the loop thread.
- [ ] `do_reload_rules`: re-reads via `get_rules_paths().find(exists)` + `parse_rules`
      (valid + counts or the error); if `is_device_connected()` →
      `reset_handshake_state()` + `perform_handshake(verbose)` → records
      `host_capable()` + `callback_names().len()`; else `capable=None`.
- [ ] Result arm: `Event::UserEvent(UserEvent::RulesReloaded(r))` →
      `format_reload_result(&r)` printed (stdout for success, stderr for parse error).
- [ ] 4 new pure tests pass; all existing tests green; `--test-threads=1`.
- [ ] `git diff --stat` = `src/tray.rs` ONLY.

## All Needed Context

### Context Completeness Check

_Pass_: an agent with no prior knowledge can implement this using only this PRP +
`research/notes.md`, because: (a) the verbatim CURRENT `UserEvent` enum, the menu
construction block (`menu_items` L336-363 with exact push order), the `MenuEvent`
arm (L461), and the result-delivery arms (L493-507) are in research §1; (b) the
EXACT async pattern to copy (the device-status poll thread L374-397 — which already
runs `perform_handshake` on a background thread) is in research §1.5; (c) the
dependency contracts (`parse_rules`/`get_rules_paths`, `perform_handshake`/
`host_capable`/`callback_names`/`reset_handshake_state`/`is_device_connected`) with
**verified merged signatures + line numbers** are in research §0; (d) the cfg map
(where tray.rs compiles/runs) and how to validate on the Linux dev box vs
macOS/Windows are in research §2; (e) the 7 design decisions (D1-D7) + 8 gotchas
(G1-G8) + 4-test plan + verified validation commands are in research §4-§7.

### Documentation & References

```yaml
# MUST READ — the verbatim research (THIS task's full contract + design + safety)
- file: plan/002_637d65b6e9b8/P5M2T1S1/research/notes.md
  why: "§0 = the dependency contracts (rules P3.M1 LANDED; handshake P4.M2 MERGED,
        with verified line numbers). §1 = verbatim CURRENT tray.rs anchors (UserEvent
        L36, setup_tray(verbose) L250, menu_items L336-363, MenuEvent arm L461, result
        arms L493, poll-thread exemplar L374-397). §2 = cfg map + dev-machine
        validation. §3 = why no rules-cache to update (P4.M3 reads live). §4 = D1-D7.
        §5 = G1-G8. §6 = 4-test plan. §7 = validation."

# MUST READ — the spec sources of truth (selected sections are in this PRP's header)
- file: spec/HOST_RULES.md
  why: "§8(7) is the CANONICAL tray contract: 'Reload rules' to all three menus
        (re-read rules.toml, re-validate, re-handshake if needed). §8(5) is the
        handshake pseudocode this task re-triggers."
  section: "§8(7) (Tray/UX), §8(5) (handshake)"

# MUST READ — the file THIS task edits (the only one)
- file: src/tray.rs
  why: "UserEvent enum (L36) gets the new variant. setup_tray(verbose) (L250) creates
        the MenuItem + builds menu_items (L336-363) — insert reload_rules_i after
        settings_i (L349) / open_at_login_i (L354), before the sep_wininfo block
        (L357). The MenuEvent arm (L461) gets the click handler; a new arm beside
        DeviceStatus (L493) handles RulesReloaded. proxy (L274) + verbose are captured
        by the run-closure (used in Init on macOS / by the poll thread) so both are
        available inside arms. The poll thread (L374-397) is the EXACT exemplar for
        off-loop perform_handshake + send_event."
  pattern: "MenuItem::new(label, enabled, None); menu_items.push(&item);
            event.id == item.id() inside UserEvent::MenuEvent; EventLoopProxy
            background-thread + send_event (copy the poll thread L374-397)."
  gotcha: "do NOT touch the icon loaders, settings/window-info dialogs, autostart,
           objc code, runners, or the poll thread. MenuItem is !Send — only
           read/mutate on the loop thread (we only read .id() there)."

# MUST READ — the handshake (MERGED in notifier.rs — read-only consumer)
- file: src/core/notifier.rs
  why: "perform_handshake(verbose) (L265) is IDEMPOTENT (HAS_HANDSHAKED swap L267) —
        so reset_handshake_state() (L457) MUST precede it to force a refresh.
        host_capable() (L441), callback_names() (L448), is_device_connected() (L171).
        perform_handshake already re-validates callback names internally (L335) on the
        capable path — so 'Reload rules' re-validates names for free."
  section: "perform_handshake, host_capable, callback_names, reset_handshake_state,
            is_device_connected"

# MUST READ — the rules module (P3.M1, LANDED — read-only consumer)
- file: src/core/rules.rs
  why: "parse_rules(&Path)->Result<RuleSet,Box<dyn Error>> (L230, STRICT: missing
        match/layer + malformed TOML => Err). get_rules_paths()->Vec<PathBuf> (L248).
        RuleSet{layer_rules:Vec, callback_rules:Vec} (L68) for the counts we log."
  section: "parse_rules, get_rules_paths, RuleSet"

# REFERENCE — the async pattern's authoritative source (already used in this file)
- url: https://crates.io/crates/tray-icon
  why: "tray-icon docs: 'forward the tray icon events to the event loop by using
        EventLoopProxy' — confirms the spawn-thread + send_event pattern this task
        reuses (already implemented for the poll thread at tray.rs L374-397)."
  critical: "EventLoopProxy is Send+Sync+Clone; send_event from a spawned thread is
             the documented pattern. No new dependency."
```

### Current Codebase tree (relevant subset)

```bash
src/
  tray.rs            # ← THIS TASK EDITS (the ONLY file): +MenuItem +UserEvent variant
                     #   +ReloadResult +do_reload_rules +format_reload_result +event-loop
                     #   wiring +#[cfg(test)] mod tests (first in this file).
  core/
    notifier.rs      # P4.M2 handshake (perform_handshake/host_capable/callback_names/
                     #   reset_handshake_state MERGED @ L265/L441/L448/L457) + P4.M3 send.
                     #   UNCHANGED (consumed read-only).
    rules.rs         # P3.M1 parse_rules/get_rules_paths/RuleSet. UNCHANGED (read-only).
  runners/
    macos.rs   # calls tray::setup_tray(self.verbose) — UNCHANGED.
    windows.rs # calls tray::setup_tray(self.verbose) — UNCHANGED.
    linux.rs   # calls tray::setup_tray() only in --no-default-features — UNCHANGED.
  linux_tray.rs      # P5.M2.T1.S2 (Linux SNI "Reload rules"). UNCHANGED by THIS task.
spec/HOST_RULES.md   # §8(7) tray + §8(5) handshake. READ-ONLY reference.
Cargo.toml           # qmk_notifier tag="v0.3.0" (RESOLVED). UNCHANGED.
```

### Desired Codebase tree with files to be changed

```bash
src/tray.rs          # MODIFIED: +reload_rules_i MenuItem, +UserEvent::RulesReloaded,
                     #   +ReloadResult struct, +do_reload_rules (bg worker),
                     #   +format_reload_result (pure), +click arm + result arm,
                     #   +#[cfg(test)] mod tests (4 pure tests).
# EVERYTHING ELSE UNCHANGED. No Cargo, no notifier.rs, no rules.rs, no platforms/,
# no runners/, no linux_tray.rs, no main.rs/core/mod.rs (P5.M1 owns those).
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (G4 — never block the GUI thread): perform_handshake does BLOCKING HID I/O
//   (QueryInfo/SetOs/QueryCallback sweep; 1-5s w/ timeouts). Running it on the
//   event-loop thread freezes the tray (macOS beachball). ALWAYS std::thread::spawn it
//   and report back via UserEvent::RulesReloaded (the poll thread at L374-397 already
//   does exactly this — copy its pattern).

// CRITICAL (D4 — reset BEFORE perform_handshake): perform_handshake is idempotent
//   (notifier.rs L267: `if HAS_HANDSHAKED.swap(true, SeqCst) { return; }`). Without
//   reset_handshake_state() first, re-running it is a NO-OP and CALLBACK_NAMES is
//   NOT refreshed — defeating the whole point of "Reload rules".

// CRITICAL (G3 — MenuItem is !Send): muda/tray-icon MenuItem holds Rc<RefCell<…>>.
//   Create it, read .id(), and (never needed here) mutate it ONLY inside event_loop.run.
//   Our worker thread touches NO MenuItem — pure data work + proxy.send_event.

// GOTCHA (G5 — proxy AND verbose are in scope inside the closure): proxy (L274) and
//   verbose (Copy bool) are both moved into event_loop.run (proxy used in the Init arm
//   on macOS; verbose captured by the poll thread). So proxy.clone() + verbose are
//   available inside the MenuEvent arm — no plumbing, no runner edits.

// GOTCHA (G6 — reset_handshake_state is an acceptable transient): while reload resets
//   state, the debounce worker may briefly fall back to string-only for ONE window;
//   the next window re-evaluates fresh. The poll thread also calls perform_handshake
//   on a device Gain — if a Gain and a user-reload race, the worst case is two
//   back-to-back handshakes (idempotent outcome; NOTIFIER Mutex serializes HID writes).
//   Harmless.

// GOTCHA (G7 — dev box is Linux; tray.rs cfg'd out by default): cargo test on this box
//   runs linux_tray.rs tests, NOT tray.rs. Compile-check tray.rs via --no-default-features;
//   full validation (click the item) on macOS/Windows per AGENTS.md. --test-threads=1
//   MANDATORY for the whole bin (AGENTS.md; shared globals in other bin tests).

// GOTCHA (G8 — binary-only crate; Mode-A rustdoc uses ```rust,ignore fences).
```

## Implementation Blueprint

### Data models and structure

```rust
// ── in src/tray.rs (add near the UserEvent enum, ~L36) ──

/// Outcome of a "Reload rules" click. Produced on a background thread (the
/// handshake does blocking HID I/O), consumed on the event-loop thread. Every
/// field is `Send`, so the struct is auto-`Send` and can travel through the
/// `EventLoopProxy<UserEvent>`. Carried by [`UserEvent::RulesReloaded`].
struct ReloadResult {
    /// `true` if `rules.toml` parsed cleanly (or was absent — host-rules-disabled
    /// is valid); `false` on a hard parse/schema error.
    rules_ok: bool,
    /// One-line detail: rule counts on success, or the parse error message.
    rules_detail: String,
    /// `Some(host_capable())` if a device was connected and the handshake (re)ran;
    /// `None` if no device (handshake skipped).
    capable: Option<bool>,
    /// `callback_names().len()` when a handshake ran; `None` otherwise.
    callback_count: Option<usize>,
}

enum UserEvent {
    MenuEvent(MenuEvent),
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    DeviceStatus(bool),
    #[cfg(target_os = "macos")]
    AutostartSync,
    /// "Reload rules" finished on a background thread: re-read `rules.toml` and
    /// (if a device is present) re-ran the capability handshake. The event-loop
    /// arm logs a single clean summary. Ungated — works on every platform
    /// `tray.rs` compiles for (macOS/Windows/fallback-X11-Linux).
    RulesReloaded(ReloadResult),
}
```

```rust
// ── worker + formatter (add near handle_settings_click / handle_window_info_click) ──

/// Re-read `rules.toml` and, if a device is connected, force a fresh capability
/// handshake. Runs on a **background thread** (the handshake does blocking HID
/// I/O that would freeze the tray if run on the event-loop thread). The caller
/// delivers the returned [`ReloadResult`] back to the event loop via
/// `EventLoopProxy::send_event(UserEvent::RulesReloaded(..))`.
///
/// There is no `RuleSet` cache to update: the debounce worker re-reads
/// `rules.toml` live on every window change (P4.M3). This function's value is
/// (a) immediate validation feedback and (b) a forced refresh of the firmware
/// callback table (reset defeats the handshake's once-per-session idempotency
/// guard so the QueryCallback sweep actually re-runs).
fn do_reload_rules(verbose: bool) -> ReloadResult {
    // 1. Re-read + validate rules.toml (the strict parse IS the schema check).
    let (rules_ok, rules_detail) =
        match crate::core::rules::get_rules_paths().into_iter().find(|p| p.exists()) {
            None => (true, "No rules.toml (host rules disabled)".to_string()),
            Some(p) => match crate::core::rules::parse_rules(&p) {
                Ok(rs) => (
                    true,
                    format!(
                        "rules.toml valid: {} layer rule(s), {} callback rule(s)",
                        rs.layer_rules.len(),
                        rs.callback_rules.len()
                    ),
                ),
                Err(e) => (false, format!("rules.toml invalid: {e}")),
            },
        };

    // 2. If a device is present, force-refresh the firmware callback table.
    //    reset_handshake_state() clears the once-per-session guard so perform_handshake
    //    actually re-sweeps (otherwise it short-circuits and CALLBACK_NAMES is stale).
    let (capable, callback_count) = if crate::core::notifier::is_device_connected() {
        crate::core::notifier::reset_handshake_state();
        crate::core::notifier::perform_handshake(verbose); // verbose respects the user's -v flag
        let names = crate::core::notifier::callback_names();
        (
            Some(crate::core::notifier::host_capable()),
            Some(names.len()),
        )
    } else {
        (None, None)
    };

    ReloadResult { rules_ok, rules_detail, capable, callback_count }
}

/// Render the two-line reload summary (pure — unit-tested). Line 1 is the rules
/// detail; line 2 is the handshake outcome (no device / legacy-timeout / OK +
/// callback count). The caller decides stdout-vs-stderr from `rules_ok`.
fn format_reload_result(result: &ReloadResult) -> String {
    let handshake = match result.capable {
        None => "no device connected — handshake skipped.".to_string(),
        Some(false) => "handshake ran — legacy/timeout (string-only mode).".to_string(),
        Some(true) => format!(
            "handshake OK — {} callback(s) discovered.",
            result.callback_count.unwrap_or(0)
        ),
    };
    format!("{}\n{}", result.rules_detail, handshake)
}
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: ADD ReloadResult struct + UserEvent::RulesReloaded variant
  - ADD `struct ReloadResult { rules_ok: bool, rules_detail: String, capable:
    Option<bool>, callback_count: Option<usize> }` just ABOVE the `enum UserEvent`
    (L36). Add a Mode-A rustdoc comment (```rust,ignore fence — G8).
  - ADD variant `RulesReloaded(ReloadResult)` to `enum UserEvent` (after AutostartSync).
    Add a doc comment noting it's ungated + produced on a bg thread.
  - VERIFY: grep -n 'struct ReloadResult\|RulesReloaded' src/tray.rs -> 3 (struct def + variant + use).

Task 2: ADD the reload_rules_i MenuItem + push it into menu_items
  - ADD (beside `let settings_i = MenuItem::new("Settings", true, None);` at L285):
        let reload_rules_i = MenuItem::new("Reload rules", true, None);
  - ADD the push between settings/open_at_login and the sep_wininfo block. The
    CURRENT block (L347-360) is:
        #[cfg(target_os = "macos")] menu_items.push(&launch_at_login_i);
        menu_items.push(&settings_i);
        #[cfg(target_os = "windows")] menu_items.push(&open_at_login_i);
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        { menu_items.push(&sep_wininfo); menu_items.push(&window_info_i); }
    INSERT (ungated, D1) right BEFORE the `#[cfg(any(macos,windows))]` sep_wininfo block:
        // "Reload rules" — re-read rules.toml + refresh the firmware callback
        // table (host-rules feature). Sits in the prefs group with Settings.
        menu_items.push(&reload_rules_i);
    Resulting order: macOS = …settings, reload_rules, sep_wininfo, window_info…;
    Windows = …settings, open_at_login, reload_rules, sep_wininfo, window_info…;
    fallback X11-Linux = …settings, reload_rules, sep_before_quit… (no wininfo group).
  - FOLLOW pattern: settings_i (L285) / the ungated menu_items.push(&settings_i) (L349).
  - NAMING: reload_rules_i (matches settings_i / quit_i / window_info_i convention).
  - GOTCHA G3: create + push only; never move reload_rules_i across threads.
  - VERIFY: grep -n 'reload_rules_i' src/tray.rs -> ≥3 (let + push + id()).

Task 3: ADD do_reload_rules(verbose) + format_reload_result() helper fns
  - ADD (beside handle_settings_click / handle_window_info_click, e.g. after
    handle_window_info_click or near the other handle_* fns): the two fns from
    "Data models and structure" above, verbatim. `do_reload_rules` takes `verbose:
    bool` (passed from the click arm — see D5; do NOT close over a global).
  - DEPENDENCIES: crate::core::rules::{get_rules_paths, parse_rules, RuleSet}
    + crate::core::notifier::{is_device_connected, reset_handshake_state,
    perform_handshake, host_capable, callback_names}. All read-only consumers
    (MERGED — verify: grep 'pub fn perform_handshake\|pub fn host_capable\|pub fn
    callback_names\|pub fn reset_handshake_state' src/core/notifier.rs -> 4).
  - NAMING: do_reload_rules (returns ReloadResult), format_reload_result (pure).
  - GOTCHA D4: reset_handshake_state() BEFORE perform_handshake (force refresh).
  - GOTCHA D5: pass the plumbed `verbose` straight through to perform_handshake.
  - VERIFY: grep -n 'fn do_reload_rules\|fn format_reload_result' src/tray.rs -> 2.

Task 4: WIRE the click arm (spawn the bg thread) in the MenuEvent handler
  - EDIT the MenuEvent arm (L461). Extend the quit/settings if/else-if chain with
    one more `else if` (ungated — D1):
        Event::UserEvent(UserEvent::MenuEvent(event)) => {
            if event.id == quit_i.id() {
                // … existing quit body unchanged …
            } else if event.id == settings_i.id() {
                handle_settings_click();
            } else if event.id == reload_rules_i.id() {
                // Re-read rules.toml + re-handshake on a background thread: the
                // handshake does BLOCKING HID I/O (G4) that would freeze the tray
                // if run here. Report back via UserEvent::RulesReloaded (G5).
                // Same pattern as the device-status poll thread above.
                let rp = proxy.clone();
                std::thread::spawn(move || {
                    let result = do_reload_rules(verbose);
                    let _ = rp.send_event(UserEvent::RulesReloaded(result));
                });
            }
            // … existing cfg-wrapped window_info / launch_at_login / open_at_login
            //   `if` blocks UNCHANGED …
        }
  - FOLLOW pattern: the device-status poll thread (L374-397) — proxy.clone() +
    std::thread::spawn + (blocking notifier call) + send_event. proxy + verbose are
    already captured by the run-closure (G5).
  - GOTCHA G4: the handshake runs ONLY inside the spawned thread, never on the loop.
  - GOTCHA G3: reload_rules_i.id() is read on the loop thread (safe).
  - VERIFY: grep -n 'do_reload_rules(verbose)\|RulesReloaded(result)' src/tray.rs -> 2.

Task 5: WIRE the result arm (log the outcome) beside DeviceStatus/AutostartSync
  - ADD a new arm (ungated — D1) in the event-loop match, beside the DeviceStatus
    (L493) / AutostartSync (L502) arms and BEFORE the `_ => {}` catch-all:
        Event::UserEvent(UserEvent::RulesReloaded(result)) => {
            // stdout for success, stderr for a parse error (matches --validate-rules).
            let summary = format_reload_result(&result);
            if result.rules_ok {
                println!("Reload rules: {}", summary.replace('\n', "\nReload rules: "));
            } else {
                eprintln!("Reload rules: {}", summary.replace('\n', "\nReload rules: "));
            }
        }
    (The replace prefixes both lines with "Reload rules: " for grep-able log output.
     Simpler alt: print the two lines directly. Keep it deterministic for the tests,
     which assert format_reload_result substrings, not the println prefix.)
  - GOTCHA: this is the ONLY place we print, so output is deterministic (never
    interleaved with the HID handshake, which runs on another thread).
  - VERIFY: grep -n 'UserEvent::RulesReloaded(result)' src/tray.rs -> 1 (the arm).

Task 6: MID-POINT build gate
  - RUN: cargo build --bin qmkonnect   (expect clean — it already is at research time).
    On the Linux dev box use: cargo build --no-default-features --bin qmkonnect
    (default features cfg tray.rs out — research §2).

Task 7: ADD the 4 pure tests (tray.rs's FIRST #[cfg(test)] mod tests)
  - ADD `#[cfg(test)] mod tests { use super::*; ... }` at the END of src/tray.rs
    with 4 tests asserting `format_reload_result` substrings (research §6):
      test_format_reload_parse_error        (rules_ok:false, capable:None)
      test_format_reload_valid_no_device    (rules_ok:true, capable:None)
      test_format_reload_valid_capable      (rules_ok:true, capable:Some(true),
                                             callback_count:Some(3) -> "handshake OK" + "3 callback")
      test_format_reload_valid_legacy       (capable:Some(false) -> "legacy/timeout")
    Build ReloadResult literals inline (plain struct — no IO). Assert the returned
    String contains the contracted substrings (stable; not exact full-string).
  - FOLLOW pattern: the existing notifier.rs `#[cfg(test)] mod tests` style +
    P5.M1.T1.S1's pure-helper approach.
  - NAMING: test_format_reload_<scenario>.
  - GOTCHA G7: these run wherever tray.rs compiles (mac/win + Linux --no-default-features).
  - VERIFY: cargo test --bin qmkonnect format_reload -- --test-threads=1 -> 4 passed
    (on macOS/Windows; or --no-default-features on Linux).

Task 8: VALIDATE (build + full suite + scope)
  - cargo build --bin qmkonnect         (mac/win)  OR  cargo build --no-default-features
    --bin qmkonnect (Linux compile-check).
  - cargo test --bin qmkonnect -- --test-threads=1     # MANDATORY single-threaded (AGENTS.md). All green.
  - cargo clippy --bin qmkonnect --no-deps 2>/dev/null || true   # no NEW warnings.
  - git diff --stat                     # expect src/tray.rs ONLY.
```

### Implementation Patterns & Key Details

```rust
// THE MenuItem + ungated push (mirrors settings_i — D1):
let reload_rules_i = MenuItem::new("Reload rules", true, None);   // (label, enabled, accelerator)
// … later, in menu_items build, ungated, after settings/open_at_login, before sep_wininfo:
menu_items.push(&reload_rules_i);

// THE click-to-background-thread pipeline (copies the poll thread, G5):
} else if event.id == reload_rules_i.id() {
    let rp = proxy.clone();                       // proxy is captured by the run-closure
    std::thread::spawn(move || {                  // handshake is BLOCKING HID (G4) -> off-loop
        let result = do_reload_rules(verbose);    // verbose is captured too (Copy bool)
        let _ = rp.send_event(UserEvent::RulesReloaded(result));
    });
}

// THE forced-refresh sequence (D4 — reset defeats the idempotency guard @ notifier.rs L267):
if crate::core::notifier::is_device_connected() {
    crate::core::notifier::reset_handshake_state();   // clear HAS_HANDSHAKED/HOST_CAPABLE/CALLBACK_NAMES
    crate::core::notifier::perform_handshake(verbose); // verbose respects -v; re-sweeps QueryInfo->SetOs->QueryCallback
    let capable = crate::core::notifier::host_capable();
    let n = crate::core::notifier::callback_names().len();
    (Some(capable), Some(n))
} else {
    (None, None)   // no device -> skip handshake
}

// THE pure formatter (the testable core, D7):
fn format_reload_result(r: &ReloadResult) -> String {
    let hs = match r.capable {
        None => "no device connected — handshake skipped.".to_string(),
        Some(false) => "handshake ran — legacy/timeout (string-only mode).".to_string(),
        Some(true) => format!("handshake OK — {} callback(s) discovered.", r.callback_count.unwrap_or(0)),
    };
    format!("{}\n{}", r.rules_detail, hs)
}
```

### Integration Points

```yaml
MODULE REGISTRATION: NONE. `mod tray` is long-standing. This task adds items to
  the BODY of tray.rs (1 MenuItem + 1 enum variant + 1 struct + 2 fns + 2 arms + tests).

DEPENDENCIES (this task): std::thread, crate::core::rules::{get_rules_paths,
  parse_rules}, crate::core::notifier::{is_device_connected, reset_handshake_state,
  perform_handshake, host_capable, callback_names}. NO new Cargo deps.
  tray_icon::menu::MenuItem + tao EventLoopProxy are already imported/used in this file.

UPSTREAM (consumed unchanged — all MERGED + verified):
  - rules::parse_rules/get_rules_paths/RuleSet (P3.M1 LANDED).
  - perform_handshake/host_capable/callback_names/reset_handshake_state/is_device_connected
    (P4.M2.T1.S1 MERGED @ notifier.rs L265/L441/L448/L457/L171).

DOWNSTREAM CONSUMERS:
  - P5.M2.T1.S2 (Linux SNI "Reload rules") — mirrors these EXACT semantics (parse_rules
    + reset + perform_handshake + log) so the UX is consistent across all three menus.
  - P6.M1.T1.S3 (docs/troubleshooting.md) — references "Reload rules" tray action.

CONFIG: none new. ROUTES: none. DATABASE: none. CLI: none (P5.M1 owns the CLI flags).
  TRAY: this task. PACKAGING: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# EXPECT: clean (it already is at research time).
# On the Linux DEV box (tray.rs cfg'd out by default), compile-check instead:
cargo build --no-default-features --bin qmkonnect

# Confirm the edits landed at the right anchors:
grep -n 'struct ReloadResult\|RulesReloaded' src/tray.rs            # struct def + variant + use (≥3)
grep -n 'reload_rules_i' src/tray.rs                                 # let + push + id() (≥3)
grep -n 'fn do_reload_rules\|fn format_reload_result' src/tray.rs   # 2

cargo clippy --bin qmkonnect --no-deps 2>/dev/null || true   # optional; no NEW warnings.
```

### Level 2: Unit Tests (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# tray.rs's FIRST #[cfg(test)] mod tests — pure formatter tests:
cargo test --bin qmkonnect format_reload -- --test-threads=1            # 4 passed
# (On the Linux dev box: cargo test --bin qmkonnect --no-default-features format_reload -- --test-threads=1)
```

### Level 3: Manual tray smoke (System Validation — macOS or Windows)

> Needs the app running with a real tray. Follow the AGENTS.md build/install loop
> for your platform (macOS: `packaging/macos/build.sh && install.sh && open`;
> Windows: `cargo build --release` + run the exe in your session). Then:

```text
1. Open the tray menu.
   EXPECT: "Reload rules" appears in the prefs group, right under Settings
           (macOS) / under Settings + "Open at Login" (Windows), and ABOVE the
           "Show Window Information…" separator.

2. With a connected v2-capable board, click "Reload rules".
   EXPECT (app log): two lines, e.g.
     Reload rules: rules.toml valid: 2 layer rule(s), 3 callback rule(s).
     Reload rules: handshake OK — 5 callback(s) discovered.

3. With a connected LEGACY board (proto_ver != 2 / no 0x01 flag / timeout):
   EXPECT: line 2 = "handshake ran — legacy/timeout (string-only mode)."

4. With NO board connected, click "Reload rules".
   EXPECT: line 2 = "no device connected — handshake skipped."

5. Make rules.toml malformed (e.g. echo 'not = valid = toml' > <config>/rules.toml),
   click "Reload rules".
   EXPECT: line 1 = "rules.toml invalid: …" (stderr); the handshake arm still
           reflects the real device state.
```

### Level 4: Full-crate regression + scope gate

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# EXPECT: ALL bin tests green — the 4 new + handshake (P4.M2) + send (P4.M3) +
#   CLI (P5.M1) + debounce + rules (P3) + pattern (P2) + types + linux_tray.
#   Proves the new item/variant/helper didn't regress anything.

git status --short && git diff --stat
# EXPECT: exactly src/tray.rs. NOTHING in Cargo.toml, notifier.rs, rules.rs,
#   main.rs, core/mod.rs, platforms/, runners/, linux_tray.rs.
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean (it already is at research time).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` green (4 new + all existing; AGENTS.md).
- [ ] `git diff --stat` = `src/tray.rs` ONLY (scope gate).
- [ ] (optional) `cargo clippy --bin qmkonnect --no-deps` introduces no NEW warnings.

### Feature Validation (contract fidelity — PRD §8(7) / HOST_RULES.md §8(7))
- [ ] "Reload rules" appears between Settings and the Show-Window-Info separator (mac/win).
- [ ] Click with a capable board ⇒ "valid: N layer, M callback" + "handshake OK — K callback(s)".
- [ ] Click with a legacy/timeout board ⇒ "handshake ran — legacy/timeout (string-only mode).".
- [ ] Click with no board ⇒ "no device connected — handshake skipped.".
- [ ] Click with a malformed rules.toml ⇒ the parse error is logged (stderr); handshake still runs.
- [ ] The handshake NEVER runs on the event-loop thread (G4) — no tray freeze/beachball.
- [ ] `reset_handshake_state()` precedes `perform_handshake()` (D4 — a real refresh happens).

### Code Quality Validation
- [ ] `reload_rules_i` follows the `MenuItem::new(label, true, None)` + ungated-push idiom (D1).
- [ ] `do_reload_rules`/`format_reload_result` mirror the existing `handle_*_click` helper style.
- [ ] `ReloadResult` is `Send` (all fields `Send`) — verified by the EventLoopProxy send compiling.
- [ ] The bg-thread + `UserEvent::RulesReloaded` pipeline mirrors the poll thread (D2).
- [ ] Uses the plumbed `verbose` for `perform_handshake(verbose)` (D5) — no hardcoding, no runner edits.
- [ ] No out-of-scope work: no Cargo/notifier.rs/rules.rs/main.rs/platforms/runners/linux_tray edits.
- [ ] Did NOT reimplement the handshake (consumed via `crate::core::notifier::`).

### Documentation & Deployment
- [ ] New fns/struct/variant have Mode-A rustdoc (`rust,ignore` fences — binary crate, G8).
- [ ] Log wording matches PRD §8(7) intent (re-read, re-validate, re-handshake).
- [ ] Commit message notes: "adds 'Reload rules' tray item (macOS/Windows) that re-reads
      rules.toml + force-refreshes the firmware callback table on a background thread;
      reports via a new UserEvent::RulesReloaded variant."

---

## Anti-Patterns to Avoid

- ❌ Don't run the handshake on the event-loop thread — it's blocking HID I/O that freezes
  the tray (macOS beachball). Always `std::thread::spawn` it + report via the UserEvent (G4/D2).
- ❌ Don't forget `reset_handshake_state()` before `perform_handshake()` — without it the
  handshake short-circuits (idempotent `HAS_HANDSHAKED` guard @ notifier.rs L267) and
  `CALLBACK_NAMES` is NOT refreshed, defeating the whole point of "Reload rules" (D4).
- ❌ Don't reimplement the handshake (`perform_handshake`/`host_capable`/`callback_names`/
  `reset_handshake_state`) — they're MERGED in notifier.rs; consume them via
  `crate::core::notifier::`. Do NOT stub or re-derive.
- ❌ Don't add a `RuleSet` cache or try to "push" the reloaded rules into the notifier —
  the debounce worker re-reads `rules.toml` live on every window change (P4.M3). "Reload
  rules" only adds validation feedback + a forced callback refresh (research §3).
- ❌ Don't `cfg`-gate `reload_rules_i` to macOS/Windows — it's a general action like
  Settings (ungated), and the fallback X11-Linux build benefits from it too (D1). Only
  `device_status_i`/`window_info_i` are mac/win-gated (they're mac/win-specific UI).
- ❌ Don't hardcode the `verbose` arg or add a `verbose` parameter to `setup_tray` —
  `setup_tray(verbose: bool)` already exists (L250) and the runners already pass it. Just
  use the in-scope `verbose` in the click arm (D5/D6).
- ❌ Don't touch the runners, Cargo.toml, notifier.rs, rules.rs, main.rs, core/mod.rs,
  linux_tray.rs, or the poll thread — this is a 1-file change (`src/tray.rs`) only.
- ❌ Don't use `handshake_action()` for the reload — that helper is for device-presence
  TRANSITIONS (poll thread); an explicit user reload wants a FORCED refresh, so use the
  direct `reset_handshake_state()` + `perform_handshake(verbose)` path (D4).
- ❌ Don't unit-test `do_reload_rules` (it does file IO + HID) or the event-loop wiring —
  test the pure `format_reload_result` helper instead (D7/G7).
- ❌ Don't run tests multi-threaded — `--test-threads=1` is mandatory (AGENTS.md; shared
  globals in other bin tests).