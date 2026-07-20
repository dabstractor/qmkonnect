# PRP — P5.M2.T1.S2: Add "Reload rules" `StandardItem` to `src/linux_tray.rs` (Linux SNI)

> **Repo under change:** the **qmkonnect** desktop app (Rust binary) at
> `/home/dustin/projects/qmkonnect`. This 1-point task adds a single **"Reload
> rules"** menu item to the **Linux StatusNotifierItem (SNI)** tray
> (`src/linux_tray.rs`, feature `linux-tray`) — PRD §8(7) (`h2.86`): *"add 'Reload
> rules' to all three menus (re-read `rules.toml`, re-validate, re-handshake if
> needed)."* The macOS/Windows half is the sibling task **P5.M2.T1.S1**
> (`src/tray.rs`); this task mirrors the SAME reload semantics on Linux for UX
> consistency across all three menus.
>
> **File touched (1):** `src/linux_tray.rs` — one new `StandardItem` (in the
> `menu()` builder), a spawned-thread worker `fn do_reload_rules()` + a pure
> formatter `fn format_reload_result(...)`, and 3 pure tests appended to the file's
> existing `#[cfg(test)] mod tests`. **No Cargo, no notifier.rs, no rules.rs, no
> platforms/, no tray.rs, no main.rs.**
>
> **Consumes (all LANDED + VERIFIED in HEAD):**
> `rules::parse_rules` / `rules::get_rules_paths` / `RuleSet` (P3.M1); `is_device_connected`
> (notifier.rs L171) / `perform_handshake` (L265) / `host_capable` (L441) /
> `callback_names` (L448) / `reset_handshake_state` (L457) (P4.M2.T1.S1). **The
> build passes today** (`cargo build --bin qmkonnect` ⇒ EXIT 0) — the v0.3.0 crate
> tag and the handshake are both merged (P4.M1.T4.S1 + P4.M2.T1.S1 Complete). See G1.
>
> **Consumed downstream by:** P6 docs (references the "Reload rules" tray action).
>
> **PARALLEL items (zero conflict):**
> - **P5.M2.T1.S1** edits `src/tray.rs` (macOS/Windows tray). This task edits
>   `src/linux_tray.rs`. **Zero file overlap** (mutually-exclusive cfg modules;
>   neither imports the other's private helpers — each re-implements the tiny
>   ~15-line shared logic). Match S1's log WORDING for cross-platform parity (D1).
> - **P5.M1.T1.S1** edits `src/main.rs` + `src/core/mod.rs` (CLI flags). **Zero overlap.**

---

## ⚠ READ FIRST — the one non-obvious trap

1. **🔴 NEVER run `perform_handshake` in the `activate` closure (it would wedge the
   tray).** `perform_handshake` is a 1-5 s **blocking HID sweep** (QueryInfo → SetOs
   → QueryCallback loop). The `activate` closure runs on **ksni's D-Bus thread** —
   the single thread that also services the poll thread's `handle.update()` (menu
   re-serialization + icon repaint) and every menu interaction. **Two independent
   authorities forbid running it there:**
   - ksni 0.3.5 `src/menu.rs` L113-129: *"avoid blocking operations here or the
     menu will freeze. … or `spawn` a new task and keep this handler lightweight."*
   - **This codebase's OWN invariant** — `src/linux_tray.rs` `spawn()` L267-270
     (added by P4.M2.T1.S2): the poll thread runs `perform_handshake` on ITS thread,
     *"NEVER inside poll_handle.update, whose closure executes on ksni's D-Bus
     thread (HID I/O there would wedge the tray icon)."* The `activate` closure is
     that same thread.
   **⇒ The closure must do ONLY `std::thread::spawn(do_reload_rules)` and return.**
   This resolves the item-description hint: *"perform_handshake may need care if it
   accesses the device."* The "care" = spawn it. See D3/G2. (Same conclusion as S1,
   but S1 spawns off the tao GUI loop to avoid a beachball; S2 spawns off ksni's
   D-Bus thread to avoid wedging the tray. Different thread, same fix.)

> **Note on preconditions:** an earlier draft of this PRP (when P4.M2.T1.S1 + the
> v0.3.0 tag were unmerged) said to HALT on a missing tag / unresolved handshake
> symbols. **Both are now LANDED and verified** — `cargo build --bin qmkonnect`
> finishes clean (EXIT 0). If a checkout fails to build for those reasons it is
> simply behind HEAD: sync and rebuild. Do NOT modify `Cargo.toml` or reimplement
> the handshake (consume via `crate::core::notifier::`). See G1.

---

## Goal

**Feature Goal**: Add a **"Reload rules"** menu item to the Linux SNI tray so a
user can, after editing `rules.toml` (or reflashing firmware with new callbacks),
re-validate it and force-refresh the firmware's callback-name table without
restarting the app — getting immediate, human-readable feedback (valid + rule
counts / parse error / handshake outcome) in the app log. Mirrors the macOS/Windows
"Reload rules" item (P5.M2.T1.S1) for cross-platform UX consistency.

**Deliverable** (1 file — `src/linux_tray.rs`):
- One new `StandardItem` (`label: "Reload rules"`) in `QmkTray::menu()`, placed in
  the **same visual group as Settings** (right after it, before the existing
  separator (B) that precedes "Show Window Information").
- A spawned-thread worker `fn do_reload_rules()` (re-reads `rules.toml` via
  `parse_rules`; if a device is connected, `reset_handshake_state()` +
  `perform_handshake(false)` to force-refresh `CALLBACK_NAMES`; logs a two-line
  summary). Runs on a **detached thread** because ksni forbids blocking in
  `activate` (D3/G2).
- A pure `fn format_reload_result(rules_ok, rules_detail, capable, callback_count)
  -> String` (the testable core; mirrors S1's wording for cross-platform parity).
- 3 pure tests appended to the file's existing `#[cfg(test)] mod tests`.

**Success Definition**:
- `cargo build --bin qmkonnect` is clean (it is today — G1 resolved).
  `linux-tray` is in the **default feature set**, so this compiles AND tests
  `linux_tray.rs` directly on the Linux dev box (unlike S1's tray.rs, cfg'd out
  on Linux).
- The SNI menu shows **"Reload rules"** right under **Settings…** (same group),
  above the "Show Window Information" separator.
- Clicking it (with a connected v2 board) logs e.g.
  `Reload rules: rules.toml valid: 2 layer rule(s), 3 callback rule(s).` +
  `Reload rules: handshake OK — 5 callback(s) discovered.`
- With a malformed `rules.toml` it logs the parse error (stderr) and still attempts
  the handshake if a device is present. With no device it logs
  `no device connected — handshake skipped.`
- The menu does **NOT** freeze on click (the handshake runs on a spawned thread).
- `cargo test --bin qmkonnect -- --test-threads=1` is green (3 new + all existing).
- `git diff --stat` = `src/linux_tray.rs` ONLY (strict scope).

## User Persona (if applicable)

**Target User**: the end user who edits `rules.toml` (adds/renames callback rules,
toggles `disable_firmware_config`) or reflashes their keyboard with new callback
definitions, and the developer integrating a new keyboard — on Linux (Waybar / KDE /
GNOME+AppIndicator / SwayNC / ironbar / Quickshell).

**Use Case**: "I just edited my `rules.toml` / reflashed my firmware with new
callbacks. Before trusting it, I click tray 'Reload rules' to confirm it parses and
to refresh the firmware's callback table — without quitting and relaunching QMKonnect."

**User Journey**: edit `rules.toml` (or reflash firmware) → click SNI tray "Reload
rules" → read the log (valid + counts, or the parse error; handshake OK / legacy /
no-device) → the next window switch uses the new rules + refreshed callbacks
automatically. The menu stays responsive throughout (no freeze).

**Pain Points Addressed**: today there is NO way to re-validate `rules.toml` or
re-trigger the handshake short of restarting the whole app; a typo silently disables
host rules with no feedback.

## Why

- **PRD §8(7) (`h2.86`)** — "Reload rules" is the canonical tray contract for the
  host-rules feature (re-read, re-validate, re-handshake) and must appear on **all
  three menus** (macOS/Windows/Linux) for UX parity.
- **PRD §1.2 (`h3.55`)** — the Linux SNI menu layout this item extends.
- **PRD §8(8) (`h2.86`)** (backward compat) — reload is purely additive; with no
  `rules.toml` it logs "No rules.toml (host rules disabled)" and changes nothing.
- **Complements** P5.M1.T1.S1's `--validate-rules` CLI (same `parse_rules` path)
  with an in-app, no-terminal equivalent; mirrors P5.M2.T1.S1's macOS/Windows item.
- **Unblocks** P6 docs (references the tray "Reload rules" action).

## What

A single new menu `StandardItem` + its click-to-spawned-thread pipeline. No change
to the runner, notifier, rules evaluator, debounce, platforms, tray.rs (macOS/Win),
or any packaging. The only observable behavior change is the new menu entry and the
log lines it produces on click.

### Success Criteria
- [ ] A `StandardItem { label: "Reload rules".to_string(), activate: Box::new(|_| {
      std::thread::spawn(do_reload_rules); }), ..Default::default() }` is pushed in
      `menu()` right after the Settings `StandardItem` and before the existing
      `MenuItem::Separator` (B, L190) that precedes "Show Window Information".
- [ ] `fn do_reload_rules()` re-reads via `get_rules_paths().into_iter().find(exists)`
      + `parse_rules` (logs valid + counts or the error); if `is_device_connected()`
      → `reset_handshake_state()` + `perform_handshake(false)` → records
      `host_capable()` + `callback_names().len()`; else `capable=None`. Prints the
      summary via `format_reload_result` (stdout for success, stderr for parse error).
- [ ] The handshake **NEVER** runs in the `activate` closure (ksni forbids blocking —
      G2); it runs only inside `std::thread::spawn(do_reload_rules)`.
- [ ] `fn format_reload_result(rules_ok, rules_detail, capable, callback_count)
      -> String` produces the two-line summary (rules detail + handshake outcome).
- [ ] 3 new pure tests pass; all existing tests green; `--test-threads=1`.
- [ ] `git diff --stat` = `src/linux_tray.rs` ONLY.

## All Needed Context

### Context Completeness Check

_Pass_: an agent with no prior knowledge can implement this using only this PRP +
`research/notes.md`, because: (a) the verbatim CURRENT `menu()` push block (Settings
`StandardItem` L183-189 + separators A/B/C at L180/190/202) is in research §1;
(b) the decisive ksni `activate` doc quote AND the codebase's own P4.M2.T1.S2
invariant (spawn() L267-270: "HID I/O there would wedge the tray icon") are in
research §1; (c) the verified dependency signatures with line numbers (rules P3.M1;
notifier handshake P4.M2 — all LANDED) are in research §0; (d) the threading-safety
proof (NOTIFIER Mutex serializes; no deadlock; poll thread doesn't lock NOTIFIER) is
in research §3; (e) the menu-placement rationale (join Settings group — no double
separator) is in research §4; (f) D1-D6 + G1-G5 + the 3-test plan + verified
validation commands are in research §5-§8.

### Documentation & References

```yaml
# MUST READ — the verbatim research (THIS task's full contract + design + safety)
- file: plan/002_637d65b6e9b8/P5M2T1S2/research/notes.md
  why: "§0 = verified dependency contracts (rules P3.M1; handshake P4.M2 — all LANDED;
        build passes EXIT 0). §1 = verbatim CURRENT linux_tray.rs menu() push block
        (Settings StdItem L183-189 + separators A/B/C L180/190/202) + activate sig
        Box<dyn Fn(&mut T)+Send> + the codebase's OWN P4.M2.T1.S2 invariant (spawn L267-270:
        'HID I/O there would wedge the tray icon'). §2 = the two-citation spawn rule.
        §3 = NOTIFIER Mutex safety proof (no deadlock). §4 = menu placement (Option Y).
        §5 = D1-D6. §6 = G1-G5. §7 = 3-test plan. §8 = validation."

# MUST READ — the spec source of truth (selected sections are in this PRP's header)
- file: spec/HOST_RULES.md
  why: "§8(7) is the CANONICAL tray contract: 'Reload rules' to all three menus
        (re-read rules.toml, re-validate, re-handshake if needed). §8(5) is the
        handshake pseudocode this task re-triggers."
  section: "§8(7) (Tray/UX), §8(5) (handshake)"

# MUST READ — the file THIS task edits (the only one)
- file: src/linux_tray.rs
  why: "QmkTray::menu() (L137) builds Vec<MenuItem<QmkTray>>. The Settings StandardItem
        is pushed at L183-189; separator (B) at L190 precedes 'Show Window Information'
        (L194-201). INSERT the 'Reload rules' StandardItem between L189 (Settings push
        close) and L190 (separator B) — same group as Settings (research §4). Every
        activate in this file is Box::new(|_| { …free_fn()… }); the new one spawns a
        thread. The poll thread in spawn() (L231) ALREADY runs perform_handshake on ITS
        thread (L267-280) and documents the 'NEVER on ksni's D-Bus thread' rule this task
        follows. #[cfg(test)] mod tests is at L909-953 — append 3 pure tests."
  pattern: "StandardItem { label, activate: Box::new(|_| {…}), ..Default::default() };
            items.push(MenuItem::Standard(item)). File gates itself with
            #![cfg(all(target_os = \"linux\", feature = \"linux-tray\"))]."
  gotcha: "do NOT touch the icon loaders (icon_pixmap/decode_icon/dim_icon), the GTK
           dialog module, the settings/window-info fns, detect_dark_mode, the poll thread
           in spawn(), or the status/hidden-toggle items. ksni activate closures run on
           ksni's D-Bus thread and MUST stay non-blocking (research §1/§2, G2)."

# MUST READ — ksni's own activate documentation (decisive design constraint #1)
- file: ~/.cargo/registry/src/index.crates.io-*/ksni-0.3.5/src/menu.rs
  why: "L113-129 documents StandardItem::activate: 'avoid blocking operations here or
        the menu will freeze. Hand off work… or spawn a new task.' L129: the field is
        Box<dyn Fn(&mut T) + Send> (closure receives &mut QmkTray, ignored as |_|)."
  section: "StandardItem::activate doc (L113-129) + field type (L129)"

# MUST READ — the macOS/Windows sibling (mirrors its semantics; NOT edited by this task)
- file: plan/002_637d65b6e9b8/P5M2T1S1/PRP.md
  why: "Defines the reload semantics this task MIRRORS on Linux: parse_rules + reset +
        perform_handshake + two-line log. S1 uses EventLoopProxy/UserEvent (tao); S2
        does NOT (ksni has no proxy — logs via eprintln! from the spawned thread, D5).
        Match S1's log WORDING for cross-platform parity."
  section: "Goal + 'Implementation Patterns' (do_reload_rules / format_reload_result wording)"

# MUST READ — the handshake implementation this task calls (P4.M2.T1.S1, LANDED)
- file: src/core/notifier.rs
  why: "perform_handshake(verbose) (L265): HAS_HANDSHAKED swap short-circuits on entry
        ⇒ reset_handshake_state() BEFORE it (D4). It locks NOTIFIER across the whole
        sweep then drop()s before publishing CALLBACK_NAMES (the threading-safety fact,
        research §3). host_capable() (L441), callback_names()->HashMap (L448, CLONES),
        reset_handshake_state() (L457), is_device_connected() (L171, fresh hidapi enum —
        does NOT lock NOTIFIER). perform_handshake(false) is QUIET (verbose-gated logs)."
  section: "perform_handshake (L265-354), host_capable/callback_names/reset_handshake_state (L441-470)"

# MUST READ — the rules module (P3.M1, LANDED — read-only consumer)
- file: src/core/rules.rs
  why: "parse_rules(&Path)->Result<RuleSet,Box<dyn Error>> (L230, STRICT: missing
        match/layer + malformed TOML => Err). get_rules_paths()->Vec<PathBuf> (L248).
        RuleSet{layer_rules:Vec<LayerRule>, callback_rules:Vec<CallbackRule>} (L68/75/79)
        for the counts we log."
  section: "parse_rules (L230), get_rules_paths (L248), RuleSet (L68)"

# REFERENCE — why no rules-cache update is needed (P4.M3 reads live)
- file: plan/002_637d65b6e9b8/P4M3T1S1/research/notes.md
  why: "host_context_for_window re-reads rules.toml via get_rules_paths()+parse_rules
        on EVERY debounced window change. So 'Reload rules' doesn't push any cache —
        the file change is picked up automatically; reload only adds immediate
        validation feedback + a forced callback refresh."
```

### Current Codebase tree (relevant subset)

```bash
src/
  linux_tray.rs      # ← THIS TASK EDITS (the ONLY file): +StandardItem in menu()
                     #   +do_reload_rules (spawned worker) +format_reload_result (pure)
                     #   +3 tests in the existing #[cfg(test)] mod tests.
                     #   Gated: #![cfg(all(target_os="linux", feature="linux-tray"))].
  core/
    notifier.rs      # perform_handshake (L265), host_capable (L441), callback_names (L448),
                     #   reset_handshake_state (L457), is_device_connected (L171), handshake_action.
                     #   UNCHANGED (consumed read-only).
    rules.rs         # P3.M1 parse_rules/get_rules_paths/RuleSet. UNCHANGED (read-only).
  tray.rs            # P5.M2.T1.S1 (macOS/Windows "Reload rules"). UNCHANGED by THIS task.
  runners/linux.rs   # calls linux_tray (default) OR tray::setup_tray (X11 fallback). UNCHANGED.
spec/HOST_RULES.md   # §8(7) tray + §8(5) handshake. READ-ONLY reference.
Cargo.toml           # L19 qmk_notifier tag="v0.3.0" (RESOLVED). default=["hyprland","macos","linux-tray"] (linux-tray IS default). UNCHANGED.
```

### Desired Codebase tree with files to be changed

```bash
src/linux_tray.rs    # MODIFIED: +"Reload rules" StandardItem in QmkTray::menu()
                     #   (after Settings, before separator B), +do_reload_rules (spawned
                     #   worker: parse_rules + reset + perform_handshake + log),
                     #   +format_reload_result (pure, tested), +3 tests in mod tests.
# EVERYTHING ELSE UNCHANGED. No Cargo, no notifier.rs, no rules.rs, no tray.rs, no
# platforms/, no runners/, no main.rs/core/mod.rs (P5.M1 owns those).
```

### Known Gotchas of our codebase & Library Quirks

```rust
// (G1 — RESOLVED) build preconditions: the v0.3.0 crate tag AND the P4.M2.T1.S1
//   handshake are BOTH LANDED (cargo build --bin qmkonnect => EXIT 0 today). If a
//   checkout fails on "failed to find tag v0.3.0" or unresolved perform_handshake/
//   host_capable/callback_names/reset_handshake_state, it is behind HEAD — sync +
//   rebuild. Do NOT touch Cargo.toml or reimplement the handshake.

// 🔴 CRITICAL (G2 — ksni forbids blocking in activate): the activate closure runs on
//   ksni's D-Bus thread. ksni 0.3.5 menu.rs L113-129: "avoid blocking operations here
//   or the menu will freeze." This codebase's OWN P4.M2.T1.S2 invariant (linux_tray.rs
//   spawn() L267-270) says the same: the poll thread runs perform_handshake on ITS
//   thread, "NEVER inside poll_handle.update, whose closure executes on ksni's D-Bus
//   thread (HID I/O there would wedge the tray icon)." perform_handshake is a 1-5s
//   BLOCKING HID sweep. ALWAYS std::thread::spawn(do_reload_rules) from the closure;
//   return immediately. parse_rules (std, microseconds) is ALSO moved into the spawned
//   worker so the closure stays trivially lightweight + the log is one clean block.

// GOTCHA (G3 — NOTIFIER contention is safe): perform_handshake holds the global
//   NOTIFIER Mutex (notifier.rs L249) across its sweep; the debounce worker serializes
//   on the SAME Mutex. Safe std::sync::Mutex serialization — NO deadlock (the worker
//   drops STATE before locking NOTIFIER; perform_handshake never holds STATE). The poll
//   thread's is_device_connected() does a fresh hidapi enum and does NOT lock NOTIFIER.
//   Observable effect: ONE notification may be delayed <=5s during a reload, then
//   flushed. Do NOT add extra locking to "fix" this — it is correct.

// GOTCHA (G4 — --test-threads=1 MANDATORY): AGENTS.md; shared STATE/COND/WORKER/
//   NOTIFIER/handshake globals in other bin tests. Every test run uses it.

// GOTCHA (G5 — binary-only crate; Mode-A rustdoc uses ```rust,ignore fences).
//   Match the existing doc comments (e.g. notifier.rs perform_handshake doc L255).
```

## Implementation Blueprint

### Data models and structure

```rust
// ── in src/linux_tray.rs (add as free fns near show_settings_dialog_linux /
//    show_window_info_linux, e.g. right after show_window_info_linux_zenity or
//    beside the other show_* helpers — keep the menu-action helpers together) ──

/// Re-read `rules.toml` and, if a device is connected, force a fresh capability
/// handshake. Runs on a **detached background thread** spawned from the "Reload
/// rules" menu item's `activate` closure — ksni runs `activate` on its D-Bus
/// thread and forbids blocking there (the menu would freeze and the poll
/// thread's `handle.update()` would stall for the whole 1-5 s HID sweep). This
/// mirrors the invariant the poll thread in [`spawn`] already follows
/// ("HID I/O there would wedge the tray icon").
///
/// There is no `RuleSet` cache to update: the debounce worker re-reads
/// `rules.toml` live on every window change (P4.M3). This function's value is
/// (a) immediate validation feedback and (b) a forced refresh of the firmware
/// callback table (`reset_handshake_state` defeats the once-per-boot guard so
/// `perform_handshake` actually re-sweeps `QueryCallback`).
fn do_reload_rules() {
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
    //    reset_handshake_state() clears the once-per-boot guard so perform_handshake
    //    actually re-sweeps (otherwise it short-circuits and CALLBACK_NAMES is stale).
    let (capable, callback_count) = if crate::core::notifier::is_device_connected() {
        crate::core::notifier::reset_handshake_state();
        crate::core::notifier::perform_handshake(false); // quiet; we log our own summary
        (
            crate::core::notifier::host_capable(),
            crate::core::notifier::callback_names().len(),
        )
    } else {
        (None, 0)
    };

    // 3. Log the two-line summary (stdout for success, stderr for a parse error —
    //    matches --validate-rules). Mirrors the macOS/Windows tray wording (S1)
    //    so feedback is identical across platforms.
    let summary = format_reload_result(rules_ok, &rules_detail, capable, callback_count);
    // Prefix every line with "Reload rules: " for grep-able log output.
    let prefixed = summary.replace('\n', "\nReload rules: ");
    if rules_ok {
        println!("Reload rules: {prefixed}");
    } else {
        eprintln!("Reload rules: {prefixed}");
    }
}

/// Render the two-line "Reload rules" summary (pure — unit-tested). Line 1 is the
/// rules detail (valid + counts / parse error / no-rules); line 2 is the handshake
/// outcome (no device / legacy-timeout / OK + callback count). Mirrors the
/// macOS/Windows tray's `format_reload_result` wording for cross-platform parity.
fn format_reload_result(
    rules_ok: bool,
    rules_detail: &str,
    capable: Option<bool>,
    callback_count: usize,
) -> String {
    let _ = rules_ok; // rules_ok steers stdout/stderr at the call site, not the text.
    let handshake = match capable {
        None => "no device connected — handshake skipped.".to_string(),
        Some(false) => "handshake ran — legacy/timeout (string-only mode).".to_string(),
        Some(true) => {
            format!("handshake OK — {callback_count} callback(s) discovered.")
        }
    };
    format!("{rules_detail}\n{handshake}")
}
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: ADD do_reload_rules() + format_reload_result() helper fns
  - ADD the two fns from "Data models and structure" above, as free fns (not impl
    methods) near show_window_info_linux / show_settings_dialog_linux (e.g. right
    after show_window_info_linux_zenity, before the gtk_dialog mod, or beside the
    other show_* helpers — pick a spot that keeps menu-action helpers together).
  - DEPENDENCIES: crate::core::rules::{get_rules_paths, parse_rules} (RuleSet for
    .layer_rules.len()/.callback_rules.len()) + crate::core::notifier::{
    is_device_connected, reset_handshake_state, perform_handshake, host_capable,
    callback_names}. All read-only consumers (all LANDED — G1).
  - NAMING: do_reload_rules (returns (), logs directly), format_reload_result (pure,
    returns String).
  - GOTCHA G1: all five notifier fns resolve in HEAD (verified). Call via crate::core::notifier::.
  - GOTCHA D4: reset_handshake_state() BEFORE perform_handshake (force refresh).
  - GOTCHA D5: perform_handshake(false) — we log our own summary.
  - GOTCHA G5: Mode-A rustdoc (```rust,ignore fences).
  - VERIFY: grep -n 'fn do_reload_rules\|fn format_reload_result' src/linux_tray.rs -> 2.

Task 2: ADD the "Reload rules" StandardItem in QmkTray::menu()
  - EDIT menu() (L137). The CURRENT Settings push + following separator (L183-190):
        items.push(MenuItem::Standard(StandardItem {
            label: "Settings…".to_string(),
            activate: Box::new(|_| {
                show_settings_dialog_linux();
            }),
            ..Default::default()
        }));
        items.push(MenuItem::Separator);   // (B) L190 — keep; it will divide the prefs
                                            //      group {Settings, Reload rules} from Show Window Info
    INSERT a new StandardItem push BETWEEN the Settings push's closing `}));` (L189)
    and that `items.push(MenuItem::Separator);` (L190):
        // "Reload rules" — re-read rules.toml + refresh the firmware callback
        // table (host-rules feature). Sits in the prefs group with Settings
        // (parity with the macOS/Windows tray). ksni runs `activate` on its
        // D-Bus thread and forbids blocking (the menu would freeze), so the
        // 1-5 s handshake runs on a spawned thread (G2 — same invariant the poll
        // thread in spawn() already follows: "HID I/O there would wedge the tray icon").
        items.push(MenuItem::Standard(StandardItem {
            label: "Reload rules".to_string(),
            activate: Box::new(|_| {
                std::thread::spawn(do_reload_rules);
            }),
            ..Default::default()
        }));
    Resulting order: …Settings…, Reload rules | (sep B) | Show Window Information | (sep C) | Quit.
  - FOLLOW pattern: the Settings StandardItem (L183) + every Box::new(|_| {…}) in
    this file. `std::thread::spawn(do_reload_rules)` passes the fn pointer (no args,
    returns ()) — no closure capture, trivially Fn + Send (satisfies ksni's Box<dyn Fn(&mut T)+Send>).
  - NAMING: label "Reload rules" (matches S1 + the item description exactly).
  - GOTCHA G2: the closure does ONLY the spawn; NO parse_rules/perform_handshake inline.
  - GOTCHA (placement, §4): do NOT add a second separator — the existing separator
    (B) already follows Settings; adding another would create a double separator.
  - VERIFY: grep -n 'Reload rules' src/linux_tray.rs -> 1 (label) + 1 (comment).

Task 3: ADD the 3 pure tests to the existing #[cfg(test)] mod tests (L909-953)
  - APPEND (inside `mod tests`, after the existing 5 tests, before the closing `}`):
        #[test]
        fn format_reload_parse_error() {
            let s = format_reload_result(false, "rules.toml invalid: bad toml", None, 0);
            assert!(s.contains("rules.toml invalid: bad toml"));
            assert!(s.contains("no device connected — handshake skipped."));
        }

        #[test]
        fn format_reload_valid_capable() {
            let s = format_reload_result(
                true,
                "rules.toml valid: 2 layer rule(s), 3 callback rule(s)",
                Some(true),
                5,
            );
            assert!(s.contains("valid: 2 layer"));
            assert!(s.contains("handshake OK"));
            assert!(s.contains("5 callback(s) discovered."));
        }

        #[test]
        fn format_reload_legacy() {
            let s = format_reload_result(
                true,
                "rules.toml valid: 1 layer rule(s), 0 callback rule(s)",
                Some(false),
                0,
            );
            assert!(s.contains("legacy/timeout (string-only mode)."));
        }
  - FOLLOW pattern: the existing pure tests (parse_id_handles_prefix_case_and_auto,
    color_scheme_parser_matches_spec) — substring asserts, no IO.
  - NAMING: format_reload_<scenario> (matches parse_id_handles_* convention; the
    module already has bare `#[test] fn name()` without a test_ prefix).
  - GOTCHA: these run on THIS Linux dev box by default (linux-tray is default-on).
  - VERIFY: cargo test --bin qmkonnect format_reload -- --test-threads=1 -> 3 passed.

Task 4: VALIDATE (build + full suite + scope)
  - cargo build --bin qmkonnect                      # clean (G1 RESOLVED; linux-tray is default).
  - cargo test --bin qmkonnect -- --test-threads=1   # MANDATORY single-threaded (AGENTS.md). All green.
  - cargo clippy --bin qmkonnect --no-deps 2>/dev/null || true   # no NEW warnings.
  - git diff --stat                                  # expect src/linux_tray.rs ONLY.
```

### Implementation Patterns & Key Details

```rust
// THE menu item — same StandardItem idiom as Settings, but the activate SPAWNS
// (ksni forbids blocking in activate; the handshake is a 1-5s blocking HID sweep):
items.push(MenuItem::Standard(StandardItem {
    label: "Reload rules".to_string(),
    activate: Box::new(|_| {
        // Lightweight: hand the blocking work to a fresh thread (G2/D3).
        std::thread::spawn(do_reload_rules);
    }),
    ..Default::default()
}));

// THE forced-refresh sequence (D4 — reset defeats the once-per-boot HAS_HANDSHAKED guard):
if crate::core::notifier::is_device_connected() {
    crate::core::notifier::reset_handshake_state();   // clear HAS_HANDSHAKED/HOST_CAPABLE/CALLBACK_NAMES
    crate::core::notifier::perform_handshake(false);  // quiet; re-sweeps QueryInfo->SetOs->QueryCallback
    let capable = crate::core::notifier::host_capable();
    let n = crate::core::notifier::callback_names().len();
    (capable, n)
} else {
    (None, 0)   // no device -> skip handshake
}

// THE pure formatter (the testable core, D6):
fn format_reload_result(rules_ok: bool, rules_detail: &str, capable: Option<bool>, callback_count: usize) -> String {
    let _ = rules_ok;
    let hs = match capable {
        None => "no device connected — handshake skipped.".to_string(),
        Some(false) => "handshake ran — legacy/timeout (string-only mode).".to_string(),
        Some(true) => format!("handshake OK — {callback_count} callback(s) discovered."),
    };
    format!("{rules_detail}\n{hs}")
}
```

### Integration Points

```yaml
MODULE REGISTRATION: NONE. `mod linux_tray` is long-standing (feature linux-tray).
  This task adds items to the BODY of linux_tray.rs (1 StandardItem + 2 free fns + 3 tests).

DEPENDENCIES (this task): std::thread (spawn), crate::core::rules::{get_rules_paths,
  parse_rules}, crate::core::notifier::{is_device_connected, reset_handshake_state,
  perform_handshake, host_capable, callback_names}. NO new Cargo deps. ksni::menu::
  {StandardItem, MenuItem} are already imported/used in this file.

UPSTREAM (consumed unchanged — all LANDED + verified):
  - rules::parse_rules/get_rules_paths/RuleSet (P3.M1).
  - is_device_connected (notifier.rs L171) + perform_handshake (L265)/host_capable (L441)/
    callback_names (L448)/reset_handshake_state (L457) (P4.M2.T1.S1).

DOWNSTREAM CONSUMERS:
  - P6.M1.T1.S3 (docs/troubleshooting.md) — references the "Reload rules" tray action.

CONFIG: none new. ROUTES: none. DATABASE: none. CLI: none (P5.M1 owns the CLI flags).
  TRAY: this task. PACKAGING: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# EXPECT: clean. linux-tray is DEFAULT-on → compiles linux_tray.rs directly (no --features
#   flag). G1 is RESOLVED (build passes today). If a checkout fails on "failed to find
#   tag v0.3.0" or unresolved perform_handshake/host_capable/callback_names/
#   reset_handshake_state, it is behind HEAD — sync + rebuild (do NOT touch Cargo.toml).

# Confirm the edits landed at the right anchors:
grep -n 'Reload rules' src/linux_tray.rs                          # label (+ comment)
grep -n 'fn do_reload_rules\|fn format_reload_result' src/linux_tray.rs   # 2

cargo clippy --bin qmkonnect --no-deps 2>/dev/null || true   # optional; no NEW warnings.
```

### Level 2: Unit Tests (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# 3 pure formatter tests appended to linux_tray.rs's mod tests:
cargo test --bin qmkonnect format_reload -- --test-threads=1            # 3 passed
# (Runs on THIS Linux dev box by default — linux-tray is default-on; unlike S1's
#  tray.rs tests, which are cfg'd out on Linux.)
```

### Level 3: Manual tray smoke (System Validation — Linux with an SNI host)

> Needs the app running with a real SNI host (Waybar / KDE Plasma / GNOME with the
> AppIndicator/KStatusNotifierItem extension / SwayNC / ironbar / Quickshell). On
> the Linux dev box: `cargo build --release && ./target/release/qmkonnect` (or the
> systemd user service). Then:

```text
1. Open the QMKonnect SNI menu.
   EXPECT: "Reload rules" appears right under "Settings…" (same group), ABOVE the
           "Show Window Information" separator. No double separator.

2. With a connected v2-capable board, click "Reload rules".
   EXPECT (app log): two lines, e.g.
     Reload rules: rules.toml valid: 2 layer rule(s), 3 callback rule(s).
     Reload rules: handshake OK — 5 callback(s) discovered.
   EXPECT: the menu does NOT freeze / hang (the handshake ran on a spawned thread);
           the device-status line + icon keep refreshing (poll thread unstalled).

3. With a connected LEGACY board (proto_ver != 2 / no 0x01 flag / timeout):
   EXPECT: line 2 = "handshake ran — legacy/timeout (string-only mode)."

4. With NO board connected, click "Reload rules".
   EXPECT: line 2 = "no device connected — handshake skipped."

5. Make rules.toml malformed (e.g. echo 'not = valid = toml' > ~/.config/qmk-notifier/rules.toml),
   click "Reload rules".
   EXPECT: line 1 = "rules.toml invalid: …" (stderr); the handshake arm still
           reflects the real device state.

6. With no rules.toml at all (rm ~/.config/qmk-notifier/rules.toml), click "Reload rules".
   EXPECT: line 1 = "No rules.toml (host rules disabled)"; line 2 reflects device state.
```

### Level 4: Full-crate regression + scope gate

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# EXPECT: ALL bin tests green — the 3 new (linux_tray format_reload) + handshake
#   (P4.M2) + send (P4.M3) + CLI (P5.M1) + debounce + rules (P3) + pattern (P2) +
#   types + tray.rs format_reload (S1, if it compiled) + the 5 pre-existing
#   linux_tray tests. Proves the new item/helper didn't regress anything.

git status --short && git diff --stat
# EXPECT: exactly src/linux_tray.rs. NOTHING in Cargo.toml, notifier.rs, rules.rs,
#   tray.rs, main.rs, core/mod.rs, platforms/, runners/.
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean (G1 RESOLVED).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` green (3 new + all existing; AGENTS.md).
- [ ] `git diff --stat` = `src/linux_tray.rs` ONLY (scope gate).
- [ ] (optional) `cargo clippy --bin qmkonnect --no-deps` introduces no NEW warnings.

### Feature Validation (contract fidelity — PRD §8(7) / HOST_RULES.md §8(7))
- [ ] "Reload rules" appears right under "Settings…" (same group), above the
      Show-Window-Info separator. No double separator.
- [ ] Click with a capable board ⇒ "valid: N layer, M callback" + "handshake OK — K callback(s)".
- [ ] Click with a legacy/timeout board ⇒ "handshake ran — legacy/timeout (string-only mode).".
- [ ] Click with no board ⇒ "no device connected — handshake skipped.".
- [ ] Click with a malformed rules.toml ⇒ the parse error is logged (stderr); handshake still runs.
- [ ] Click with no rules.toml ⇒ "No rules.toml (host rules disabled)" + device-state handshake line.
- [ ] The menu does NOT freeze on click (G2 — handshake on a spawned thread); the poll
      thread's device-status/icon refresh stays responsive.

### Code Quality Validation
- [ ] The "Reload rules" `StandardItem` follows the existing `Box::new(|_| {…})` idiom.
- [ ] The `activate` closure does ONLY `std::thread::spawn(do_reload_rules)` — no blocking
      HID/file work inline (G2/D3).
- [ ] `do_reload_rules`/`format_reload_result` mirror the existing `show_*` helper style.
- [ ] `reset_handshake_state()` precedes `perform_handshake()` (D4 — forces a real refresh).
- [ ] No out-of-scope work: no Cargo/notifier.rs/rules.rs/tray.rs/main.rs/platforms/runners edits.
- [ ] Did NOT reimplement the handshake (consumed via `crate::core::notifier::`).

### Documentation & Deployment
- [ ] New fns have Mode-A rustdoc (`rust,ignore` fences — binary crate, G5).
- [ ] Log wording matches PRD §8(7) intent AND S1's macOS/Windows wording (cross-platform parity).
- [ ] Commit message notes: "adds 'Reload rules' SNI tray item (Linux) that re-reads
      rules.toml + force-refreshes the firmware callback table on a spawned thread
      (ksni forbids blocking in activate); mirrors the macOS/Windows item."

---

## Anti-Patterns to Avoid

- ❌ Don't run `perform_handshake` (or `parse_rules` after a slow path, or any blocking
  HID/file op) **in the `activate` closure** — ksni's own doc (menu.rs L113-129) AND this
  codebase's P4.M2.T1.S2 invariant (spawn() L267-270) both forbid it: it "would wedge the
  tray icon" + freeze the menu + stall the poll thread's `handle.update()`. ALWAYS
  `std::thread::spawn(do_reload_rules)` from the closure (G2/D3). This is THE trap; the
  item description's literal "call perform_handshake [in the closure]" is resolved by spawning.
- ❌ Don't reimplement the handshake (`perform_handshake`/`host_capable`/`callback_names`/
  `reset_handshake_state`) — they are LANDED (P4.M2.T1.S1); consume via
  `crate::core::notifier::` (G1).
- ❌ Don't forget `reset_handshake_state()` before `perform_handshake()` — without it the
  handshake short-circuits (once-per-boot `HAS_HANDSHAKED` guard) and `CALLBACK_NAMES` is
  NOT refreshed, defeating the whole point of "Reload rules" (D4).
- ❌ Don't add a `RuleSet` cache or try to "push" the reloaded rules into the notifier —
  the debounce worker re-reads `rules.toml` live on every window change (P4.M3). "Reload
  rules" only adds validation feedback + a forced callback refresh.
- ❌ Don't copy S1's `EventLoopProxy`/`UserEvent::RulesReloaded`/`ReloadResult` machinery —
  ksni has no event-loop proxy. The spawned worker logs via `println!`/`eprintln!` directly
  (D5). Re-implement the ~15-line shared logic locally (different cfg modules; can't import
  tray.rs's private helpers). DO match S1's log WORDING for cross-platform parity (D1).
- ❌ Don't add a SECOND separator after "Reload rules" — a `MenuItem::Separator` (B, L190)
  already follows Settings. Adding another creates a visually-broken double separator. The
  existing separator divides the prefs group {Settings, Reload rules} from Show Window Info
  (§4/D2). The item description's "Separator +" is satisfied by that existing separator.
- ❌ Don't unit-test `do_reload_rules` (it does file IO + HID + thread spawn) or the
  activate closure — test the pure `format_reload_result` helper instead (D6).
- ❌ Don't run tests multi-threaded — `--test-threads=1` is mandatory (AGENTS.md; shared
  globals in other bin tests).
- ❌ Don't edit notifier.rs/rules.rs/tray.rs/main.rs/core/mod.rs/platforms/runners — this is
  a 1-file change (src/linux_tray.rs) only.