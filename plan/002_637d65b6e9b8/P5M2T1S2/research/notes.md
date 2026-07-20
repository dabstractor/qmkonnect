# Research Notes — P5.M2.T1.S2: Add "Reload rules" to `src/linux_tray.rs` (Linux SNI)

> Scope: **`src/linux_tray.rs` ONLY** (the Linux StatusNotifierItem tray, feature
> `linux-tray`). Adds a single **"Reload rules"** `StandardItem` to the SNI menu —
> PRD §8(7) (`h2.86`): *"add 'Reload rules' to all three menus (re-read
> `rules.toml`, re-validate, re-handshake if needed)."* This is the Linux half;
> the macOS/Windows half is the sibling task **P5.M2.T1.S1** (`src/tray.rs`).
>
> **PARALLEL with S1** (currently being implemented): S1 defines the macOS/Windows
> reload semantics; this task **mirrors the SAME semantics on the Linux SNI menu**
> for UX consistency. The two tasks edit **different files** (tray.rs vs
> linux_tray.rs) → **zero file overlap**. Neither imports the other's private
> helpers (different modules, mutually-exclusive cfg), so each re-implements the
> tiny shared logic (~15 lines).
>
> **STATUS CHANGE vs a prior draft of this PRP:** at planning time P4.M2.T1.S1
> (handshake) and the v0.3.0 crate tag were unmerged CONTRACTS (a prior draft
> said to HALT on them). **Both are now LANDED and VERIFIED** — see §0. The build
> passes (`cargo build --bin qmkonnect` ⇒ EXIT 0). All handshake signatures below
> are read from the ACTUAL merged source, not a contract.

---

## §0 — Dependency contracts (all LANDED + VERIFIED in HEAD — consume, don't reimplement)

### `crate::core::notifier` — `src/core/notifier.rs` (P4.M2.T1.S1 + P4.M2.T1.S2 MERGED)
VERIFIED present (grep + read of actual source):
```
L171  pub fn is_device_connected() -> bool                 # fresh hidapi enumeration; does NOT lock NOTIFIER
L265  pub fn perform_handshake(verbose: bool)              # the capability sweep (QueryInfo→SetOs→QueryCallback)
L441  pub fn host_capable() -> bool                        # reads HOST_CAPABLE AtomicBool
L448  pub fn callback_names() -> HashMap<String, u8>       # CLONES CALLBACK_NAMES
L457  pub fn reset_handshake_state()                       # clears HOST_CAPABLE/BOARD_HAS_RULES/CALLBACK_NAMES/HAS_HANDSHAKED
      pub enum HandshakeAction { None, Gain, Loss }        # device-transition helper (~L460); NOT used by reload
      pub fn handshake_action(prev, cur) -> HandshakeAction
```
**`perform_handshake` internals (verified L265-354):**
- `HAS_HANDSHAKED.swap(true, SeqCst)` on entry → **short-circuits if already true**
  ("at most once per board boot"). ⇒ Reload MUST call `reset_handshake_state()`
  FIRST or the handshake is a silent no-op (CALLBACK_NAMES stays stale). (Same as
  S1's D4.)
- `let n = get_notifier(); let n = n.lock().unwrap();` → **holds the global
  NOTIFIER `Arc<Mutex<Box<dyn Notifier>>>` (L249) across the ENTIRE sweep**
  (QueryInfo + SetOs + the QueryCallback loop), then `drop(n)` before publishing
  `CALLBACK_NAMES`. This is the KEY fact for the threading-safety proof (§3).
- `perform_handshake(false)` is QUIET (only `if verbose` branches log). We pass
  `false` and log our own reload summary.
- `perform_handshake` ALREADY re-validates `rules.toml` callback names internally
  (`validate_rules_callback_names`, L367): it warns (not fails) on unknown names
  and skips a malformed file. ⇒ Our explicit `parse_rules`+log in `do_reload_rules`
  is still required for **user-facing feedback** (valid + counts / parse error /
  no-rules) — the handshake's validation is silent with `verbose=false`.

### `crate::core::rules` (P3.M1 LANDED) — `src/core/rules.rs`
```
L68   pub struct RuleSet { pub layer_rules: Vec<LayerRule> (L75), pub callback_rules: Vec<CallbackRule> (L79), … }
L230  pub fn parse_rules(path: &Path) -> Result<RuleSet, Box<dyn Error>>   # STRICT: missing match/layer + bad TOML ⇒ Err
L248  pub fn get_rules_paths() -> Vec<PathBuf>                              # platform config dirs; first-existing wins
```

### Build status (VERIFIED): `cargo build --bin qmkonnect` ⇒ `Finished dev profile in 0.13s`, EXIT 0.
- `Cargo.toml:19` `qmk_notifier { … tag = "v0.3.0" }` resolves
  (checkout `~/.cargo/git/checkouts/qmk_notifier-a54e3247c1b61fcf` exists).
- `linux-tray` is in the DEFAULT feature set (`default = ["hyprland","macos","linux-tray"]`)
  ⇒ plain `cargo build`/`cargo test` **compile AND exercise linux_tray.rs on this
  Linux dev box** (unlike S1's tray.rs, which is cfg'd out on Linux).

---

## §1 — Verbatim CURRENT `src/linux_tray.rs` anchors (953 lines, feature `linux-tray`)

### Menu builder — `impl ksni::Tray for QmkTray { fn menu(&self) -> Vec<MenuItem<QmkTray>> }` (L137)
CURRENT push order (exact, L178-211):
```rust
        items.push(MenuItem::Separator);                       // (A) L180 — after status block
        // Settings dialog (zenity) — writes config.toml on save. Parity with
        // the macOS/Windows "Settings" entry.
        items.push(MenuItem::Standard(StandardItem {           //        L183
            label: "Settings…".to_string(),                    //        L184
            activate: Box::new(|_| { show_settings_dialog_linux(); }),   // L185-188
            ..Default::default()
        }));                                                   //        L189
        items.push(MenuItem::Separator);                       // (B) L190 — after Settings, before Show Window Info
        // §7 opt 2: surface the active window's class/title …
        items.push(MenuItem::Standard(StandardItem {           //        L194
            label: "Show Window Information".to_string(),      //        L196
            activate: Box::new(|_| { show_window_info_linux(); }),
            ..Default::default()
        }));
        items.push(MenuItem::Separator);                       // (C) L202
        items.push(MenuItem::Standard(StandardItem { label: "Quit".to_string(), activate: Box::new(|_| { std::process::exit(0); }), .. }));
```
- **INSERTION POINT (D2)**: insert the "Reload rules" `StandardItem` BETWEEN the
  Settings push's closing `}));` (L189) and separator (B) at L190. Puts "Reload
  rules" in the **same visual group as Settings** (no separator between them) —
  matches S1's macOS/Windows placement (§8(7) cross-platform consistency). The
  existing separator (B) then divides the prefs group {Settings, Reload rules}
  from Show Window Info. **Do NOT add a second separator** — the item-description
  phrasing "Separator + StandardItem" is satisfied by the *existing* separator (B);
  another would create a visually-broken double separator (§4).

### 🔴 ksni's own `activate` doc forbids blocking — `~/.cargo/.../ksni-0.3.5/src/menu.rs` L113-129
```text
/// Callback invoked when the item is activated
/// … so AVOID BLOCKING OPERATIONS HERE OR THE MENU WILL FREEZE. Hand off work to
/// your main application logic (e.g., via channels… ) or `spawn` a new task and
/// keep this handler lightweight. If you need to update the tray after doing
/// work elsewhere, call [`crate::Handle::update`].
pub activate: Box<dyn Fn(&mut T) + Send>,     // T = QmkTray; closure receives &mut QmkTray (ignored as |_|)
```

### 🔴🔴 THE codebase's OWN invariant for THIS tray — `src/linux_tray.rs` spawn() L267-280
The poll thread (P4.M2.T1.S2) ALREADY drives the handshake on device transitions
and encodes the exact rule this task must follow:
```rust
            // Handshake lifecycle on a real device transition. Runs on THIS poll
            // thread — NEVER inside poll_handle.update, whose closure executes on
            // ksni's D-Bus thread (HID I/O there would wedge the tray icon).
            if last_device != Some(connected) {
                match crate::core::notifier::handshake_action(last_device, connected) {
                    crate::core::notifier::HandshakeAction::Gain => { crate::core::notifier::perform_handshake(verbose); }
                    crate::core::notifier::HandshakeAction::Loss => { crate::core::notifier::reset_handshake_state(); }
                    crate::core::notifier::HandshakeAction::None => {}
                }
            }
```
- The `activate` closure runs on the **same ksni D-Bus thread** that
  `poll_handle.update`'s closure runs on. The codebase already established that
  HID I/O (perform_handshake) there "would wedge the tray icon." ⇒ **The activate
  closure must NOT call perform_handshake inline; it must `std::thread::spawn`.**
- This is now TWO independent citations for D3/G3: (1) ksni's own doc, (2) the
  codebase's own P4.M2.T1.S2 invariant for this exact tray. No ambiguity.

### Existing test module — `#[cfg(test)] mod tests` (L909-953)
5 pure tests: `status_text_uses_parity_glyphs`, `new_tray_probes_initial_state`,
`parse_id_handles_prefix_case_and_auto`, `color_scheme_parser_matches_spec`,
`embedded_icons_decode`. **Convention: pure helpers only** (no IO, no closures).
We append 3 pure tests for the new `format_reload_result` helper (§8).

---

## §2 — Why spawn (not inline): two converging citations

`perform_handshake` is a 1-5 s **blocking HID sweep** (QueryInfo → SetOs → a
QueryCallback loop, each a HID round-trip with timeouts). The `activate` closure
runs on **ksni's D-Bus thread** — the single thread that also services the poll
thread's `handle.update()` (menu re-serialization + icon repaint) and every menu
interaction. Two independent authorities forbid running it there:

1. **ksni 0.3.5 doc** (menu.rs L113-129, §1): *"avoid blocking operations here or
   the menu will freeze. … or `spawn` a new task and keep this handler lightweight."*
2. **This codebase's own P4.M2.T1.S2 invariant** (linux_tray.rs spawn() L267-270):
   the poll thread runs `perform_handshake` on ITS thread, *"NEVER inside
   poll_handle.update, whose closure executes on ksni's D-Bus thread (HID I/O
   there would wedge the tray icon)."* The `activate` closure is that same thread.

**⇒ CONCLUSION (D3/G3): the activate closure does ONLY `std::thread::spawn(do_reload_rules)`
and returns immediately.** This resolves the item-description hint: *"perform_handshake
may need care if it accesses the device."* The "care" = spawn it; never inline.
(Compare S1: it spawns off the tao GUI loop to avoid a macOS beachball. Same fix,
different thread. S2 does NOT copy S1's `EventLoopProxy`/`UserEvent` machinery —
ksni has no event-loop proxy; the spawned worker just `eprintln!`s, exactly as the
item description specifies.)

---

## §3 — Threading safety: why `perform_handshake` on a spawned thread is safe

`perform_handshake` holds the global `NOTIFIER: Lazy<Arc<Mutex<Box<dyn Notifier>>>>`
(notifier.rs L249) for the whole sweep (verified §0). Two other threads touch state:

- **Debounce worker** (`debounce_worker`, notifier.rs): locks `STATE` in a scoped
  block, `take()`s the message, **drops STATE**, THEN locks `NOTIFIER` only during
  the burst `notifier.notify(message)`. → Never holds STATE + NOTIFIER together.
- **Poll thread** (`linux_tray::spawn`): calls `is_device_connected()` which does a
  FRESH `hidapi::HidApi::new()` enumeration (notifier.rs L171) — it does **NOT**
  lock `NOTIFIER`. Its only ksni interaction is `handle.update()`, a separate channel.

**Lock-ordering (no deadlock):** `perform_handshake` = NOTIFIER (sweep) → drop →
CALLBACK_NAMES (publish). Debounce worker = STATE (scoped) → NOTIFIER (burst). The
two never hold a lock the other needs while waiting ⇒ **no deadlock**, only safe
`Mutex` serialization.

**Observable effect during a reload:** if a window change arrives mid-sweep, the
debounce worker blocks briefly at `NOTIFIER.lock()` until the sweep finishes, then
flushes the latest pending message (the debouncer coalesces — only the newest
survives). Net: a ≤5 s delay in ONE notification during a manual reload. Acceptable
(reload is user-initiated and rare). No lost messages, no HID corruption.

**No race with the poll thread's handshake_action:** reload does
`reset_handshake_state()` + `perform_handshake()`. The poll thread keys on
`last_device != Some(connected)`; a reload does NOT change device presence, so
`handshake_action` returns `None` ⇒ the poll thread won't fire a *second* handshake.
The two paths are disjoint by trigger (device transition vs. user click).

---

## §4 — Menu placement: "Reload rules" joins the Settings group (Option Y)

The item description says *"add after the Settings item: a Separator + StandardItem
with label 'Reload rules'."* But a `MenuItem::Separator` (B, L190) ALREADY follows
Settings. A literal "add Separator + item" would produce a **double separator**
(`Settings… | NEW-sep | Reload rules | existing-sep(B) | Show Window Info`) — a
visual bug. The description's "Separator +" is therefore satisfied by the **existing**
separator (B); only the `StandardItem` is added.

**Chosen placement (D2) — matches S1 for §8(7) cross-platform consistency:**
```
●/○  Device status                                              (line 1)
(hidden structural toggle, if connected)
───── (A) L180
Settings…
Reload rules            ← NEW (same group as Settings; no separator between)
───── (B) L190 existing
Show Window Information
───── (C) L202
Quit
```
"Reload rules" is a prefs/utility action that naturally groups with Settings (S1
placed it identically: `…settings, reload_rules, sep_wininfo, window_info…`). One
push, no separator juggling, no double-separator risk.

---

## §5 — Design decisions (D1-D6)

- **D1 — Mirror S1's reload semantics (cross-platform parity).** Same steps:
  (1) re-read+validate `rules.toml` via `parse_rules` (explicit user-facing log:
  valid + counts / parse error / no-rules); (2) if `is_device_connected()`,
  `reset_handshake_state()` + `perform_handshake(false)`; (3) log a two-line
  summary. Same wording as S1 so feedback is identical across macOS/Windows/Linux.
- **D2 — "Reload rules" joins the Settings group** (§4): one `items.push(StandardItem{…})`
  between the Settings push close (L189) and separator (B) (L190). No new separator.
- **D3 — 🔴 Spawn the handshake off ksni's D-Bus thread** (§2). The activate closure
  does ONLY `std::thread::spawn(do_reload_rules)` and returns. Backed by ksni's own
  doc AND the codebase's P4.M2.T1.S2 invariant for this tray ("HID I/O there would
  wedge the tray icon"). `parse_rules` (std, microseconds) is ALSO moved into the
  spawned worker for a single clean log block + a trivially-lightweight closure.
- **D4 — `reset_handshake_state()` BEFORE `perform_handshake()`** (§0). Defeats the
  `HAS_HANDSHAKED` once-per-boot guard so the callback table actually refreshes
  (the whole point of "Reload rules").
- **D5 — No `EventLoopProxy` / `UserEvent` (ksni has none).** The spawned worker logs
  via `println!` (success) / `eprintln!` (parse error) directly — matches the item
  description ("logs success/error via eprintln!") and is deterministic (one print site).
- **D6 — Extract a pure `fn format_reload_result(...) -> String` + test it** (matches
  the file's pure-helper test convention + S1's approach). `do_reload_rules` (file IO
  + HID + thread spawn) is NOT unit-tested — only the pure formatter is (consistent
  with how this file tests `parse_id`/`parse_color_scheme`, never the zenity/dialog
  closures).

---

## §6 — Gotchas (G1-G5)

- **G1 — (RESOLVED) build preconditions.** The v0.3.0 crate tag AND P4.M2.T1.S1
  handshake are BOTH LANDED and verified (`cargo build --bin qmkonnect` ⇒ EXIT 0).
  If, in some checkout, `cargo build` fails on `failed to find tag v0.3.0` or on
  `perform_handshake`/`host_capable`/`callback_names`/`reset_handshake_state` being
  unresolved, that checkout is behind HEAD — sync and rebuild. Do NOT touch
  `Cargo.toml` or reimplement the handshake (consume via `crate::core::notifier::`).
- **G2 — 🔴 NEVER run `perform_handshake` (or any blocking HID op) in the activate
  closure.** ksni's doc (§1) AND this codebase's own P4.M2.T1.S2 invariant (§1) both
  forbid it: it runs on ksni's D-Bus thread and would "wedge the tray icon" + freeze
  the menu + stall the poll thread's `handle.update()`. ALWAYS `std::thread::spawn`.
  (This is THE trap; the item description's literal "call perform_handshake [in the
  closure]" is resolved by D3.)
- **G3 — NOTIFIER contention is safe (brief delay, no deadlock).** §3. The debounce
  worker serializes on the same `Mutex`; during a reload ONE notification may be
  delayed ≤5 s then flushed. Do NOT add extra locking/synchronization — it's correct.
- **G4 — `--test-threads=1` MANDATORY** (AGENTS.md; shared `STATE`/`COND`/`WORKER`/
  `NOTIFIER`/handshake globals in other bin tests). Every test run uses it.
- **G5 — Binary-only crate; Mode-A rustdoc uses ` ```rust,ignore ` fences** (no
  `cargo test --doc` on a bin; `use qmkonnect::…` won't resolve). Match existing doc
  comments in this file (e.g. the `perform_handshake` doc at notifier.rs L255).

---

## §7 — Test plan (3 pure tests, appended to the existing `mod tests` at L910)

All assert **substrings** of `format_reload_result(...)` (stable; not exact full-string):
1. `format_reload_parse_error` — `(false, "rules.toml invalid: bad toml", None, 0)`
   → contains "rules.toml invalid: bad toml" AND "no device connected — handshake skipped.".
2. `format_reload_valid_capable` — `(true, "rules.toml valid: 2 layer rule(s), 3 callback rule(s)", Some(true), 5)`
   → contains "valid: 2 layer" AND "handshake OK" AND "5 callback(s) discovered.".
3. `format_reload_legacy` — `(true, "rules.toml valid: 1 layer rule(s), 0 callback rule(s)", Some(false), 0)`
   → contains "legacy/timeout (string-only mode).".
- Naming: `format_reload_<scenario>` (matches `parse_id_handles_*` / `color_scheme_parser_*`).
- These run wherever `linux-tray` compiles — i.e. **on this Linux dev box by default**,
  so they're actually executed here (unlike S1's tray.rs tests).

---

## §8 — Validation (project dev loop, AGENTS.md; Linux dev box — fully runnable)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect                        # clean (G1 RESOLVED; linux-tray is default → compiles linux_tray.rs)
grep -n 'Reload rules\|fn do_reload_rules\|fn format_reload_result' src/linux_tray.rs   # item + 2 helpers present
cargo test --bin qmkonnect format_reload -- --test-threads=1     # 3 new pure tests pass
cargo test --bin qmkonnect -- --test-threads=1                   # MANDATORY single-threaded; ALL bin tests green
cargo clippy --bin qmkonnect --no-deps 2>/dev/null || true       # no NEW warnings
git diff --stat                                                   # expect src/linux_tray.rs ONLY
```
Manual (Level 3 — needs a real SNI host: Waybar / KDE / GNOME+AppIndicator):
run the app → open the SNI menu → "Reload rules" sits right under "Settings…" → click:
with a v2 board → log shows "rules.toml valid: N layer, M callback" + "handshake OK — K callback(s)";
legacy/offline → "legacy/timeout (string-only mode)."; no board → "no device connected — handshake skipped.";
malformed rules.toml → the parse error (stderr); the menu does NOT freeze (spawned).