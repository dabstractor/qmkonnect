# PRP — P3.M2.T1.S3: Linux zenity `--list` picker + Advanced `--forms` in `src/linux_tray.rs`

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This task restructures the **Linux** SNI
> Settings dialog (`show_settings_dialog_linux` in `src/linux_tray.rs:688`) so a
> live, self-populating **picker** of discovered devices (`classify_devices`)
> becomes the primary surface, run as a **`zenity --list`** dialog *before* the
> existing `zenity --forms`. Selecting a picker row writes that board's VID/PID
> to `config.toml` via the shared `render_config_body` and short-circuits the
> `--forms`; cancelling/no-selection falls through to the `--forms` as the
> **"Advanced / manual override"**. Source of truth: **`spec/DEVICE_DISCOVERY.md`
> §5** (the Discovered-Device Picker) + **`spec/UI.md` §2.0/§2.3/§2.4** (the
> picker as new primary surface + the Linux `zenity --forms` contract +
> `parse_id`). Windows (`P3.M2.T1.S1` — Complete) and macOS (`P3.M2.T1.S2` — in
> flight) pickers are separate sibling tasks; this PRP touches **Linux only**.
>
> **CONSUMES (read-only, already in tree — verified):**
> `classify_devices(verbose: bool) -> Vec<ClassifiedDevice>` (`notifier.rs:1116`),
> `pub enum DeviceKind { Capable{..}, NotQmkNotifier }` (`notifier.rs:816`),
> `pub struct ClassifiedDevice { path, vendor_id:u16, product_id:u16,
> product_name:Option<String>, usage_page, usage, kind }` (`notifier.rs:841`).
> All `pub` and currently `#[allow(dead_code)]` — **this task is a consumer** (the
> `allow` becomes satisfied; leave it).
>
> **DOES NOT TOUCH:** the write path (P2 DEFER), `classify_devices`/cache logic
> (P3.M1 — Complete), `device_status()` (P1 — Complete), the Windows dialog
> (`tray.rs::show_settings_dialog` — P3.M2.T1.S1), the macOS dialog
> (`tray.rs::show_settings_dialog_with_pool` — P3.M2.T1.S2), CLI flags (P4),
> `Cargo.toml` (zenity is shelled out; `std::process::Command` already used), the
> crate, or any `docs/*.md` (Mode A — P4.M2 owns user docs). Single-file change:
> `src/linux_tray.rs`.
>
> **PARALLEL-SAFETY:** the in-flight sibling **P3.M2.T1.S2 edits `src/tray.rs`
> (macOS)** — it does NOT touch `linux_tray.rs`. No task edits `linux_tray.rs`
> concurrently, so the line numbers below are stable. (`tray.rs` is shifting under
> S1/S2; `linux_tray.rs` is exclusively this task's.) There is **no shared
> symbol** with the siblings: each picker lives in its own platform-gated module,
> and `picker_columns`/`parse_vidpid`/`save_and_notify` are Linux-module-local
> free functions (no name collision with the Windows/macOS `picker_row_text`).
>
> **LINE-NUMBER NOTE:** line numbers are from research time (`show_settings_dialog_linux`
> @688, `ApplyOutcome` @778, `apply_device_rule` @795, `current_config_hex` @838,
> `write_config` @859, `parse_id` @883, `notify` @899, `mod tests` @1006). They
> are stable (no concurrent edit of this file), but always `grep -n` to confirm
> before editing.

---

## Goal

**Feature Goal**: Restructure `show_settings_dialog_linux` (`src/linux_tray.rs:688`)
so the Linux Settings dialog shows a **`zenity --list` picker** of discovered
devices (columns: Device / VID:PID / Capability, one row per `ClassifiedDevice`,
✓/✗ capability glyph) as the primary surface, run **before** the existing
`zenity --forms`. Selecting a picker row becomes the disambiguation: it writes
that board's `(vid,pid)` to `config.toml` via the shared `render_config_body`
renderer and **skips the `--forms`**. If the user cancels the `--list` or selects
nothing, it **falls through to the existing `--forms` as the "Advanced / manual
override"**. The zero-config case (one capable board, no VID/PID set) is
preserved: the picker is skipped and the `--forms` opens with a `Detected: <name>.
Auto-discovery is active.` note. `apply_device_rule`/pkexec flow is unchanged.

**Deliverable** (additive edits to `src/linux_tray.rs`, all implicitly
`#[cfg(all(target_os = "linux", feature = "linux-tray"))]` by module gate):
1. **`picker_columns(d: &ClassifiedDevice) -> (String, String, String)`** — the
   three list columns: Device name (or `(unnamed)`), `0xVID:0xPID` (uppercase),
   capability glyph+status (`✓ qmk_notifier` / `✗ QMK board, no module`).
2. **`parse_vidpid(s: &str) -> Option<(u16, u16)>`** — parse a `--print-column=2`
   selection (`0xFEED:0x0000`) back to `(vid,pid)`; reuses `parse_id` each half.
3. **`current_config_vidpid() -> (Option<u16>, Option<u16>)`** — the open-time
   raw VID/PID (for the clean-auto check); `current_config_hex` is refactored to
   derive its display strings from this (one config-read).
4. **`save_and_notify(vendor_id: Option<u16>, product_id: Option<u16>)`** — pure
   extraction of the existing `write_config` → `apply_device_rule` → notify /
   error-notify tail. Both the picker pick and the manual `--forms` call it.
5. **`show_settings_dialog_linux`** rewritten: `classify_devices(true)` → 3-case
   decision (empty / clean-auto / picker) → run `zenity --list` (picker case) →
   on selection `save_and_notify` + return; else fall through to the existing
   `--forms` (with case-specific `--text`).
6. **Mode-A doc-comment** on `show_settings_dialog_linux` citing
   `spec/DEVICE_DISCOVERY.md` §5 + `spec/UI.md` §2.3, noting the two-dialog
   design, the no-Rescan decision, and the `--list`-tiles-on-tiling-WMs tradeoff
   (vs the native GTK popup used by window-info).
7. **2 unit tests** (`test_picker_columns`, `test_parse_vidpid`) verifying the
   pure helpers (the zenity `Command` invocations are not unit-testable).

**Success Definition**:
- `cargo build --bin qmkonnect` clean (on Linux with `--features linux-tray`; no
  NEW warnings). On Windows/macOS the build is unchanged (the module is gated out).
- The dialog, when `classify_devices` returns ≥2 Tier-1 devices OR 1 non-capable
  board OR 1 capable board with VID/PID already set, shows the `--list` with one
  row per device (`✓`/`✗` glyph + name + `0xVID:0xPID` + status); selecting a row
  + OK writes that board's `(vid,pid)` to `config.toml` and fires the
  "settings saved" `notify-send` (skipping the `--forms`).
- When `classify_devices` returns exactly one `Capable` board AND the open-time
  config has no VID/PID, the `--list` is **skipped** and the `--forms` opens with
  text `Detected: <name>. Auto-discovery is active.` (OK writes None/None —
  auto-discovery; zero-config preserved).
- When `classify_devices` returns no devices, the `--list` is **skipped** and the
  `--forms` opens with text `No QMK keyboards detected. Enter IDs manually below.`
- When the user **cancels** the `--list` or selects nothing, the `--forms` opens
  (Advanced / manual override); typing a hex pair + OK writes it; empty/`auto` ⇒
  `None` ⇒ auto-discovery; Cancel ⇒ no write (existing behavior).
- `apply_device_rule`/pkexec behavior is byte-identical to today (both `None` ⇒
  no rule; ≥1 `Some` ⇒ pkexec install).
- `git status` = `src/linux_tray.rs` only.

## User Persona (if applicable)

**Target User**: a Linux user (Waybar / SwayNC / KDE / GNOME-SNI) with one or more
QMK keyboards who opens Settings (menu → Settings…) to confirm which board
QMKonnect sees or to disambiguate among several.

**Use Case**: the user has flashed `qmk_notifier` onto one board and has a second
QMK board (VIA/Vial only). They open Settings → the `--list` shows both:
`Dactyl | 0xFEED:0x0000 | ✓ qmk_notifier` and `Keychron | 0x3434:0x0123 | ✗ QMK
board, no module`. They click the capable row → OK → `config.toml` records that
board's VID/PID so notifications narrow to it, and a "settings saved" notification
fires. No `--forms` appears.

**Pain Points Addressed**: today the Linux dialog is two raw hex fields — the user
must already know their board's VID/PID and type it blind. The picker shows live
`product_name` from the HID descriptor (the device names itself; no curated
database) and makes selection a one-click disambiguation (`spec/UI.md` §2.0). The
manual hex fields remain available as Advanced for the rare off-bus override.

## Why

- **`DEVICE_DISCOVERY.md` §5.1 mandates the picker as the new primary surface.**
  Raw VID/PID hex entry becomes an "Advanced / manual override" disclosure. This
  task ships the Linux rendering of that picker (§5.3: "`zenity --list --column …`
  (the discovered list) + a second `zenity --forms` for the Advanced VID/PID; or
  the native GTK popup already used for window-info" — the contract picks the
  `--list`+`--forms` option).
- **`UI.md` §2.0/§2.3/§2.4 specify the Linux contract** that this dialog already
  implements (`zenity --forms --add-entry` × 2, `|`-split stdout, `parse_id` each,
  `write_config` → `apply_device_rule` via pkexec, `notify-send`). This task adds
  the picker *before* the `--forms` and relocates the `--forms` to the
  "Advanced / manual override" role while preserving that contract.
- **The zero-config promise must be preserved (§5.1).** The common case — one
  capable board, no VID/PID set — must NOT show a picker and must NOT write a
  VID/PID (auto-discovery is already correct). This task implements that branch.
- **Completes the three-platform picker parity** alongside the Windows (S1) and
  macOS (S2) siblings: same chosen-first-else-manual save precedence, adapted to
  Linux's sequential two-dialog model.
- **Consumes the classification API** shipped by P3.M1.T1.S1/S2 (Complete). This
  task is a real UI consumer; it exercises `classify_devices` end-to-end on Linux.

## What

Additive edits to `src/linux_tray.rs` (the module is already
`#![cfg(all(target_os = "linux", feature = "linux-tray"))]`, so every item is
Linux-gated). **No new Cargo deps** (zenity is shelled out via the already-used
`std::process::Command`; `classify_devices`/`ClassifiedDevice`/`DeviceKind` are
cross-platform `pub` in `notifier.rs`). No Windows/macOS behavior change.

**Platform-specific deviation, documented (Mode A):**
- **No `[Rescan]` button.** The two zenity dialogs run sequentially (there is no
  "open dialog" window to click a button within, unlike Windows' message loop).
  `classify_devices(true)` is called once per `show_settings_dialog_linux()`
  invocation; **re-opening Settings refreshes** (after the 5s cache TTL the probe
  re-runs). Mirrors macOS S2 (no Rescan; `runModal` blocks).
- **The picker uses `zenity --list`, which tiles on pure tiling WMs** (Sway/i3/
  hyprland), unlike `--forms` which floats. This is an accepted tradeoff: the
  device count is tiny (1-3 keyboards, so the list is short and usable tiled),
  and `--list` provides the exact single-select→print-selection semantics needed.
  The window-info dialog avoids tiling via a native GTK popup, but that
  heavyweight plumbing is unjustified for a 3-row device list.

### Success Criteria
- [ ] **`picker_columns`**: `Capable{..}` ⇒ vidpid `0x{:04X}:0x{:04X}` (uppercase)
      + cap `✓ qmk_notifier`; `NotQmkNotifier` ⇒ `✗ QMK board, no module`; name or
      `(unnamed)`.
- [ ] **`parse_vidpid`**: `"0xFEED:0x0000"` → `Some((0xFEED,0))`; `"feed:0x123"` →
      `Some((0xFEED,0x123))`; `""`/`"feed"`/`"feed:"`/`"garbage:x"` → `None`.
- [ ] **`current_config_vidpid`** returns `(Option<u16>, Option<u16>)` from the
      first existing config candidate (mirrors `current_config_hex`'s search);
      `current_config_hex` derives its strings from it (or stays independent —
      minimal-risk path allowed).
- [ ] **`save_and_notify(vid, pid)`** extracted verbatim from the old inline tail
      (`write_config` → `apply_device_rule` match → `notify`; error → `notify`).
      `apply_device_rule`/pkexec UNCHANGED.
- [ ] **`show_settings_dialog_linux`** calls `classify_devices(true)`, computes
      `clean_auto` (1 capable + no VID/PID), and runs `zenity --list` (3 columns,
      `--print-column=2`, one row per device) in the picker case.
- [ ] **Picker selection** (success + non-empty stdout) ⇒ `parse_vidpid` ⇒
      `save_and_notify(Some(v), Some(p))` ⇒ `return` (skips the `--forms`).
- [ ] **Picker cancel/no-selection** ⇒ fall through to the existing `--forms`.
- [ ] **`--forms` (Advanced / manual override)** reached by all three cases
      (empty / clean-auto / picker-fallthrough); `--text` reflects the case; OK ⇒
      `parse_id` each ⇒ `save_and_notify(v, p)`; Cancel ⇒ `return` (no write).
- [ ] **Mode-A doc-comment** cites `spec/DEVICE_DISCOVERY.md` §5 + `spec/UI.md`
      §2.3; notes the two-dialog design, no-Rescan, and the `--list` tiling tradeoff.
- [ ] **`test_picker_columns`** + **`test_parse_vidpid`** pass (pure helpers).
- [ ] `cargo build --bin qmkonnect --features linux-tray` clean on Linux.
      `cargo test --bin qmkonnect -- --test-threads=1` green (existing tests + the
      2 new ones). `git status` = `src/linux_tray.rs` only.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge can implement this using only this PRP,
because: (a) the exact current `show_settings_dialog_linux` code (verbatim, with
line numbers) is in `research/linux_picker_research.md` §1; (b) the exact
neighbor helpers (`current_config_hex`/`write_config`/`apply_device_rule`/
`parse_id`/`notify`) are summarized with signatures in §1; (c) the consumed
`classify_devices`/`DeviceKind`/`ClassifiedDevice` API is verified in-tree with
signatures in §2; (d) the zenity `--list`/`--print-column`/exit-code mechanics
(pinned from the GNOME man page) + the exact picker argv are in §3; (e) the 3-case
decision + the chosen-first-else-manual precedence + the save_and_notify refactor
are fully specified in §4; (f) 14 gotchas are pinned (G1-G14); (g) the 3 new pure
helpers + their tests are specified in §5.

### Documentation & References

```yaml
# MUST READ — the spec source of truth (the §5 picker UX + §5.3 Linux rendering)
- url: spec/DEVICE_DISCOVERY.md
  why: "§5.1 (3 picker cases: clean-auto ⇒ static 'Detected: <name>' + no picker;
        ≥2 Tier-1 ⇒ picker; no capable + ≥1 Tier-1 ⇒ picker with ✗). §5.2 (Advanced
        = the existing two hex fields relocated). §5.3 (Linux: 'zenity --list
        --column … (the discovered list) + a second zenity --forms for the Advanced
        VID/PID; or the native GTK popup already used for window-info' — the
        contract picks --list). §3 (the ✓/✗ + 'qmk_notifier'/'QMK board, no module'
        row semantics + the three-state tray text)."
  section: "## 5. The Discovered-Device Picker (Settings UX) (§5.1-§5.3, §3)"

# MUST READ — the UI spec (the Linux zenity --forms contract this task extends)
- url: spec/UI.md
  why: "§2.0 (picker as new primary surface; Advanced disclosure; chosen-first-
        else-manual-else-as-is save). §2.3 (the Linux --forms dialog: --add-entry
        VID/PID, '|'-split stdout, parse_id each, write_config → apply_device_rule
        via pkexec, notify-send — the EXISTING contract this task keeps as the
        Advanced fallback). §2.4 (parse_id: empty/auto ⇒ None)."
  section: "## 2. Settings Dialogs (§2.0, §2.3, §2.4)"

# MUST READ — the codebase verification (THIS task's exact edit sites, verbatim)
- file: plan/005_8b95ea464bd9/P3M2T1S3/research/linux_picker_research.md
  why: "§1 the verbatim current show_settings_dialog_linux (@688) + the neighbor
        helpers (ApplyOutcome @778, apply_device_rule @795, current_config_hex @838,
        write_config @859, parse_id @883, notify @899) with signatures + line
        numbers. §2 the in-tree notifier API (classify_devices/DeviceKind/
        ClassifiedDevice). §3 the zenity --list mechanics + the exact picker argv.
        §4 the 3-case decision + the save_and_notify refactor + why CASE B doesn't
        write a VID/PID. §5 the 3 new pure helpers + their tests. §6 the tiling
        tradeoff (Mode A). §7 the 10 locked design decisions. §8 the 14 gotchas."

# MUST READ — the file THIS task edits (every line referenced confirmed by reading)
- file: src/linux_tray.rs
  why: "show_settings_dialog_linux @688 (the fn to restructure; builds the zenity
        --forms + parses + saves). ApplyOutcome @778 + apply_device_rule @795
        (UNCHANGED; called via save_and_notify). current_config_hex @838 (refactor
        to derive from current_config_vidpid OR leave as-is). write_config @859 +
        parse_id @883 + notify @899 (reused unchanged). The existing tests mod @1006
        (add test_picker_columns + test_parse_vidpid here). The show_window_info_linux
        @383 + its code comment @387-392 (the documented --list-tiles observation —
        cite this in the doc-comment). The module gate #![cfg(all(target_os=linux,
        feature=linux-tray))] @line-1-area + the `use crate::core::notifier::DeviceStatus`
        @line 5 (add classify_devices/ClassifiedDevice/DeviceKind to this import)."
  pattern: "All zenity calls use Command::new(\"zenity\").args([...]).stdout(Stdio::piped())
            .stderr(Stdio::null()).output(); match on Ok(o) if o.status.success().
            Column values are pushed as separate args (Command does NOT go through a
            shell — ✓ glyph + spaces are fine, no quoting). The '|' is only special
            inside --forms stdout (split) and --list-values (join); --list column
            values are positional argv."
  gotcha: "G4 zenity cancel = exit 1 (guard success+non-empty-stdout). G6 --list
           tiles on tiling WMs (documented tradeoff, not a bug). G8 CASE B never
           writes a VID/PID (zero-config). G10 save_and_notify is a verbatim
           extraction (apply_device_rule/pkexec unchanged)."

# MUST READ — the consumer API (the classification functions this task calls)
- file: src/core/notifier.rs
  why: "classify_devices(verbose: bool) -> Vec<ClassifiedDevice> @1116 (enumerate
        Tier-1 + per-candidate QUERY_INFO + 5s-TTL cache; verbose=true for diagnostic
        eprintln). pub enum DeviceKind { Capable{..}, NotQmkNotifier } @816 (the ✓/✗
        discriminator). pub struct ClassifiedDevice { path, vendor_id:u16,
        product_id:u16, product_name:Option<String>, usage_page, usage, kind } @841
        (vendor_id/product_id are u16 — always Some — so a pick yields a concrete
        (u16,u16); product_name may be None ⇒ '(unnamed)')."
  pattern: "All three are #[allow(dead_code)] today (no consumer yet); this task is
            a consumer. Call as crate::core::notifier::classify_devices(true). The
            cache is warm from the status-poll thread + handshake; do NOT call
            classification_cache_clear() on Settings-open (parity with S1/S2)."

# Reference — zenity man page (the --list / --print-column / exit-code semantics)
- url: https://commandlinux.com/man-page/man1/zenity/
  why: "--print-column=N: 'Specify what column to print to standard output. The
        default is to return the first column. ALL may be used to print all columns.'
        Confirms --print-column=2 prints ONLY the VID:PID cell. Exit codes: OK=0,
        Cancel/close=1. Selected value on stdout; empty on Cancel/no-selection."
- url: https://linux.die.net/man/1/zenity
  why: "Same --list semantics (cross-reference). Confirms plain --list (no
        --checklist) is single-select; --width/--height affect a --list (real
        GtkTreeView) unlike the height-capped embedded list in --forms."
```

### Current Codebase tree (relevant subset)

```bash
src/
  linux_tray.rs       # Linux SNI tray + dialogs (~1110 lines, feature linux-tray).
                        # module gate #![cfg(all(linux, linux-tray))] @top;
                        # use crate::core::notifier::DeviceStatus @5;
                        # show_window_info_linux @383 (native GTK + zenity fallback;
                        #   its comment @387-392 documents "--list tiles");
                        # show_settings_dialog_linux @688 (RESTRUCTURE);
                        # ApplyOutcome @778; apply_device_rule @795;
                        # current_config_hex @838; write_config @859;
                        # parse_id @883; notify @899; mod tests @1006.
                        # <-- THIS TASK: + picker_columns + parse_vidpid +
                        #     current_config_vidpid + save_and_notify + REWRITE
                        #     show_settings_dialog_linux (classify + 3 cases +
                        #     zenity --list + --forms fallback) + 2 tests.
  core/
    notifier.rs       # classify_devices @1116; DeviceKind @816; ClassifiedDevice @841
                        # (CONSUMED, not edited)
    mod.rs            # Config @24 (vendor_id/product_id: Option<u16>);
                        # render_config_body; atomic_write; parse_config (CONSUMED)
  platforms/mod.rs    # get_config_paths() (CONSUMED, not edited)
spec/
  DEVICE_DISCOVERY.md # §5 = picker UX; §3 = row semantics (READ-ONLY)
  UI.md               # §2.0/§2.3/§2.4 = Linux dialog contract (READ-ONLY)
Cargo.toml            # no new dep (zenity shelled out; std::process::Command) — UNCHANGED
```

### Desired Codebase tree (files this task changes)

```bash
src/
  linux_tray.rs       # MODIFIED (Linux-only, additive by module gate):
                        #  + picker_columns(d) -> (String,String,String)
                        #  + parse_vidpid(s) -> Option<(u16,u16)>
                        #  + current_config_vidpid() -> (Option<u16>,Option<u16>)
                        #  + save_and_notify(vid, pid)   [verbatim extraction of old tail]
                        #  + REWRITE show_settings_dialog_linux body:
                        #    classify_devices + 3 cases + zenity --list picker +
                        #    --forms (Advanced) fallback + chosen-first-else-manual
                        #  + test_picker_columns + test_parse_vidpid
                        #  (optionally: refactor current_config_hex to derive from
                        #   current_config_vidpid — minimal-risk alternative: leave it)
    # EVERYTHING else unchanged (Cargo.toml, core/*, tray.rs, spec/*, packaging/*)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (G1 — Linux-only module): linux_tray.rs is
//   #![cfg(all(target_os = "linux", feature = "linux-tray"))]. ALL new code
//   inherits this gate. Zero impact on Windows/macOS builds (the module is cfg'd
//   out there). The new imports (classify_devices/ClassifiedDevice/DeviceKind)
//   are cross-platform pub in notifier.rs but only referenced inside this module.
//
// CRITICAL (G2 — two SEQUENTIAL zenity dialogs): --list THEN --forms, separate
//   Command::output() invocations. Never combine into one zenity call (--forms
//   caps list height at ~4 rows; --list is the proper selection widget). The
//   picker is a SEPARATE dialog, by explicit contract.
//
// CRITICAL (G3 — NO [Rescan]): the two dialogs are sequential — there is no
//   "open dialog" window to click a button within (unlike Windows' GetMessageW
//   loop). classify_devices(true) is called once per show_settings_dialog_linux()
//   call; RE-OPENING Settings refreshes (after the 5s cache TTL the probe
//   re-runs). Mirrors macOS S2 (no Rescan; runModal blocks). Document (Mode A).
//
// CRITICAL (G4 — zenity cancel = exit 1): guard a pick as
//   `status.success() && !stdout.trim().is_empty()`. Cancel/close → exit 1 →
//   fall through to --forms. OK-with-no-selection → exit 0 + EMPTY stdout → also
//   fall through. Do NOT treat empty-stdout-OK as a selection.
//
// CRITICAL (G5 — --print-column=2): prints the VID:PID cell of the selected row.
//   Parse it back with parse_vidpid. Do NOT print column 1 (Device name — not
//   unique; None/duplicate names) or column 3 (Capability).
//
// CRITICAL (G6 — --list TILES on tiling WMs): zenity --list is a normal toplevel
//   → Sway/i3/hyprland tile it (unlike --forms which floats). This is an
//   ACCEPTED, Mode-A-documented tradeoff: the device count is tiny (1-3 boards),
//   so a short tiled list is usable; and --list provides the exact single-select
//   → print-selection semantics. The window-info dialog avoids tiling via a
//   native GTK popup (show_window_info_linux @383, comment @387-392), but that
//   heavyweight plumbing is unjustified for a 3-row device list. NOT a bug.
//
// GOTCHA (G7 — blocking the ksni D-Bus thread): the existing dialog ALREADY
//   blocks the activate-thread (zenity Command::output() + apply_device_rule/
//   pkexec are synchronous). classify_devices(true) adds one more short block
//   (reads a WARM cache ⇒ ~free; only cold-classifies on a stale/empty cache).
//   Consistent with the existing pattern; do NOT spawn a thread (out of scope;
//   the contract scopes this to dialog logic only).
//
// CRITICAL (G8 — CASE B does NOT write a VID/PID): "auto-select" in the contract
//   = SKIP the picker (auto-discovery is already correct), NOT "write the single
//   capable board's VID/PID". The --forms opens with "Detected: <name>.
//   Auto-discovery is active." text; OK with blank/auto fields writes None/None.
//   Writing a VID/PID in CASE B would VIOLATE the zero-config promise (§5.1).
//
// CRITICAL (G9 — chosen vs manual types): chosen = (u16,u16) from parse_vidpid;
//   manual = (Option<u16>,Option<u16>) from parse_id each. save_and_notify takes
//   (Option<u16>,Option<u16>): chosen → (Some(v),Some(p)); manual → (v,p).
//
// CRITICAL (G10 — save_and_notify is a VERBATIM extraction): lift the existing
//   write_config → apply_device_rule match → notify / error-notify tail out of
//   show_settings_dialog_linux into a helper, byte-identical. Both paths call it.
//   apply_device_rule/pkexec is UNCHANGED (contract requirement). Pure refactor.
//
// GOTCHA (G11 — column argv as SEPARATE elements): push each column value as its
//   own args.push(...) element (Command::new("zenity").args([...])), NOT a shell
//   string. Rust's Command does NOT go through a shell — the ✓ glyph + spaces are
//   fine (no quoting). The existing --text=… with em-dashes (show_window_info_linux)
//   proves spaces/special-chars work as single args.
//
// GOTCHA (G12 — single-threaded tests, AGENTS.md): cargo test --bin qmkonnect --
//   --test-threads=1 (shared MockNotifier globals + DebounceState). The new pure
//   helpers (picker_columns, parse_vidpid, current_config_vidpid) are unit-
//   testable; the zenity Command invocations are NOT (they spawn a real GUI).
//
// GOTCHA (G13 — refactor OR leave current_config_hex): current_config_hex returns
//   display strings ("auto"/"feed"); the clean-auto check needs raw Option<u16>.
//   Preferred: add current_config_vidpid() and derive current_config_hex's strings
//   from it (DRY, one config-read). Minimal-risk alternative: add
//   current_config_vidpid() and leave current_config_hex untouched (two reads;
//   acceptable). Pick one; do NOT delete current_config_hex (still used for the
//   --forms display text).
//
// GOTCHA (G14 — the --forms --text must reflect the case): empty / clean-auto /
//   picker-fallthrough each get a distinct informative --text so the user
//   understands what they're seeing. The current values (current_config_hex) still
//   appear in all three (spec/UI.md §2.3 keeps the "Current: …" line).
//
// CRATE QUIRK: cargo test --bin qmkonnect -- --test-threads=1 (AGENTS.md). The
//   zenity dialogs are NOT unit-testable (they spawn a real GTK GUI); only the
//   pure helpers are. Manual Level-4 testing needs a Linux desktop with ≥1 QMK
//   board (Waybar/SwayNC/KDE/GNOME-SNI).
```

## Implementation Blueprint

### Data models and structure

```rust
// ── (1) the picker-row builder (pure; unit-tested) ──
/// One picker row's three `zenity --list` columns: the live `product_name` (or
/// `(unnamed)`), the `0xVID:0xPID` (uppercase, for parity with the spec example),
/// and the capability glyph + status. Built from a [`ClassifiedDevice`]
/// (`spec/DEVICE_DISCOVERY.md` §5.1 / §3). Pure; unit-tested.
fn picker_columns(d: &crate::core::notifier::ClassifiedDevice) -> (String, String, String) {
    use crate::core::notifier::DeviceKind;
    let (glyph, status) = match d.kind {
        DeviceKind::Capable { .. } => ("\u{2713}", "qmk_notifier"),         // ✓
        DeviceKind::NotQmkNotifier => ("\u{2717}", "QMK board, no module"), // ✗
    };
    let name = d.product_name.as_deref().unwrap_or("(unnamed)").to_string();
    let vidpid = format!("0x{:04X}:0x{:04X}", d.vendor_id, d.product_id);
    let cap = format!("{glyph} {status}");
    (name, vidpid, cap)
}

// ── (2) parse a --list --print-column=2 selection back to (vid,pid) (pure) ──
/// Parse the `zenity --list --print-column=2` stdout (`0xFEED:0x0000`) back to a
/// concrete `(u16, u16)`. Returns `None` on any malformed input (no colon,
/// non-hex, missing half). Reuses [`parse_id`] for each half (empty/auto ⇒ None
/// ⇒ None here). Pure; unit-tested.
fn parse_vidpid(s: &str) -> Option<(u16, u16)> {
    let mut it = s.trim().splitn(2, ':');
    let vid = parse_id(it.next()?).ok()??;   // ?? : Result→Option, Option<u16>→u16
    let pid = parse_id(it.next()?).ok()??;
    Some((vid, pid))
}

// ── (3) the open-time raw VID/PID (for the clean-auto check) ──
/// The currently-configured VID/PID as raw `Option<u16>` (the clean-auto check
/// needs the real Options, not the display strings). Mirrors `current_config_hex`'s
/// first-existing-candidate search. (Preferred: refactor `current_config_hex` to
/// derive its display strings from this — DRY, one config-read.)
fn current_config_vidpid() -> (Option<u16>, Option<u16>) {
    crate::platforms::get_config_paths()
        .into_iter()
        .find(|p| p.exists())
        .and_then(|p| crate::core::parse_config(&p).ok())
        .map(|cfg| (cfg.vendor_id, cfg.product_id))
        .unwrap_or((None, None))
}

// ── (4) the shared save+apply+notify tail (VERBATIM extraction of the old block) ──
/// Persist VID/PID, apply the udev device rule (pkexec), and notify the user.
/// Extracted verbatim from the old inline tail of `show_settings_dialog_linux` so
/// the picker path and the manual `--forms` path share identical save behavior
/// (incl. the `ApplyOutcome` notify detail). `apply_device_rule`/pkexec unchanged.
fn save_and_notify(vendor_id: Option<u16>, product_id: Option<u16>) {
    let vid_str = vendor_id
        .map(|v| format!("0x{v:04x}"))
        .unwrap_or_else(|| "auto".to_string());
    let pid_str = product_id
        .map(|p| format!("0x{p:04x}"))
        .unwrap_or_else(|| "auto".to_string());
    match write_config(vendor_id, product_id) {
        Ok(path) => {
            let outcome = apply_device_rule(vendor_id, product_id);
            let detail = match outcome {
                ApplyOutcome::AutoDiscovery => {
                    "Auto-discovery in effect (any standard QMK keyboard).".to_string()
                }
                ApplyOutcome::Applied => "Device rule applied.".to_string(),
                ApplyOutcome::NeedsManual(how) => how,
            };
            notify(
                "QMKonnect — settings saved",
                &format!(
                    "vendor_id = {vid_str}, product_id = {pid_str}\n{detail}\n{}",
                    path.display()
                ),
            );
        }
        Err(e) => {
            eprintln!("Settings: failed to write config: {}", e);
            notify("QMKonnect — could not save", &e.to_string());
        }
    }
}

// NOTE: NO DIALOG_RESULT static on Linux (G9/D9). The two zenity Command::output()
// calls are sequential synchronous blocks; the result is a plain local. Only the
// helpers above are module-level; the dialog logic is all inside
// show_settings_dialog_linux.
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: ADD the four helpers (picker_columns, parse_vidpid, current_config_vidpid, save_and_notify)
  - DO: add picker_columns + parse_vidpid + current_config_vidpid + save_and_notify
        (the exact bodies in "Data models and structure" above), placed near the
        existing helpers (e.g. just above show_settings_dialog_linux @688, or
        alongside current_config_hex/parse_id/notify). save_and_notify references
        write_config + apply_device_rule + ApplyOutcome + notify (all in scope).
  - G9: chosen=(u16,u16); manual=(Option<u16>,Option<u16>); save_and_notify takes
        (Option<u16>,Option<u16>). G10: save_and_notify is a VERBATIM extraction.
  - G13 (PREFERRED): after adding current_config_vidpid, refactor current_config_hex
        to derive its strings from it:
          fn current_config_hex() -> (String, String) {
              let (v, p) = current_config_vidpid();
              let fmt = |id: Option<u16>| match id {
                  Some(x) => format!("{x:04x}"),
                  None => "auto".to_string(),
              };
              (fmt(v), fmt(p))
          }
        (MINIMAL-RISK ALTERNATIVE: leave current_config_hex as-is; just add
        current_config_vidpid alongside. Both compile; pick the derive for DRY.)
  - NOTE: these compile standalone. Task 2 rewrites the dialog to use them.

Task 2: REWRITE show_settings_dialog_linux (@688) — classify + 3-case + picker + --forms fallback
  - DO: replace the body of show_settings_dialog_linux with:
        (a) classify + decide the case:
              use crate::core::notifier::{classify_devices, DeviceKind};
              let devices = classify_devices(true);
              let (cur_vid, cur_pid) = current_config_vidpid();
              let clean_auto = devices.len() == 1
                  && matches!(devices[0].kind, DeviceKind::Capable { .. })
                  && cur_vid.is_none() && cur_pid.is_none();
              let picker = !devices.is_empty() && !clean_auto;
            (G8: clean_auto = the zero-config case; picker is skipped for it AND
             for empty. G5/D5: NO classification_cache_clear() — read the warm cache.)
        (b) IF picker, run the zenity --list:
              let chosen = if picker { run_device_picker(&devices) } else { None };
            where run_device_picker builds the argv (Task 3) and returns
            Option<(u16,u16)> (Some on a real selection, None on cancel/no-selection).
            IF chosen.is_some():
              let (v, p) = chosen.unwrap();
              save_and_notify(Some(v), Some(p));   // G9: lift to Option
              return;                              // SKIP the --forms (D4)
            (else fall through to the --forms.)
        (c) the existing --forms (Advanced / manual override), reached by empty /
            clean_auto / picker-fallthrough. KEEP the zenity --forms invocation
            (Command::new("zenity").args(["--forms","--title=QMK Settings",
            &format!("--text={text}"),"--add-entry=Vendor ID (hex)",
            "--add-entry=Product ID (hex)"]).stdout(Stdio::piped()).stderr(Stdio::null())
            .output()). UPDATE the --text to reflect the case:
              let (cur_vid_h, cur_pid_h) = current_config_hex();
              let prefix = if devices.is_empty() {
                  "No QMK keyboards detected. Enter IDs manually below.".to_string()
              } else if clean_auto {
                  format!("Detected: {}. Auto-discovery is active.",
                      devices[0].product_name.as_deref().unwrap_or("(unnamed)"))
              } else {
                  "Advanced / manual override — enter hex VID/PID.".to_string()
              };
              let text = format!(
                  "{prefix}\nCurrent: vendor_id = 0x{cur_vid_h}   product_id = 0x{cur_pid_h}\n\
                   Enter hex values (the 0x prefix is optional; blank = auto-discovery):"
              );
            KEEP the cancel handling (Ok(_) => return), the zenity-missing notify,
            the '|' split, parse_id each, the invalid-input notify.
            REPLACE the write_config+apply_device_rule+notify tail with:
              save_and_notify(vid, pid);
            (G10: identical behavior, extracted.)
  - G2: two separate zenity calls. G4: --list guard success+non-empty-stdout.
        G8: CASE B never writes a VID/PID. G14: --text reflects the case.
  - DOC-COMMENT (Mode A): cite spec/DEVICE_DISCOVERY.md §5 + spec/UI.md §2.3;
        note the two-dialog design, the no-Rescan decision (G3), and the
        --list-tiles-on-tiling-WMs tradeoff (G6, citing show_window_info_linux @383).

Task 3: ADD run_device_picker(devices) -> Option<(u16,u16)> (the --list invocation)
  - DO: add a helper:
        fn run_device_picker(devices: &[crate::core::notifier::ClassifiedDevice])
            -> Option<(u16, u16)>
        {
            // Build argv: flags + 3 column headers + N×3 values (G11: separate args).
            let mut args: Vec<String> = vec![
                "--list".into(),
                "--title=QMK Settings".into(),
                "--print-column=2".into(),                 // G5: print the VID:PID cell
                "--hide-header".into(),                    // optional; 1-3 rows ⇒ no header noise
                "--width=520".into(),
                "--text=Select a detected keyboard (or Cancel for manual entry):".into(),
                "--column=Device".into(),
                "--column=VID:PID".into(),
                "--column=Capability".into(),
            ];
            for d in devices {
                let (name, vidpid, cap) = picker_columns(d);
                args.push(name);
                args.push(vidpid);
                args.push(cap);
            }
            let output = Command::new("zenity")
                .args(args.iter().map(String::as_str))
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output();
            let out = match output {
                Ok(o) if o.status.success() => o,          // G4: success gate
                _ => return None,                          // cancel/close/non-zero ⇒ no pick
            };
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.trim().is_empty() {
                return None;                               // G4: OK-with-no-selection
            }
            parse_vidpid(&stdout)                          // None if malformed (defensive)
        }
  - G5: --print-column=2. G11: each value a separate arg (✓ glyph + spaces OK).
        G4: success + non-empty-stdout ⇒ a pick; else None. NO notify on cancel
        (silent fall-through to --forms). NO notify on zenity-missing here — the
        --forms (which follows) has its own zenity-missing notify; a missing zenity
        would make BOTH dialogs fail, and the --forms's notify covers it. (Keep
        run_device_picker focused on the selection; error UX lives in the --forms.)

Task 4: ADD the 2 unit tests (pure helpers — the only testable pieces)
  - DO: in the existing `#[cfg(test)] mod tests` (@1006), add:
        #[test]
        fn test_picker_columns() {
            use crate::core::notifier::{ClassifiedDevice, DeviceKind};
            let capable = ClassifiedDevice {
                path: String::new(), vendor_id: 0xFEED, product_id: 0x0000,
                product_name: Some("Dactyl".into()), usage_page: 0xFF60, usage: 0x61,
                kind: DeviceKind::Capable { proto_ver: 2, feature_flags: 1,
                    callback_count: 0, board_rules_present: false },
            };
            let notqmk = ClassifiedDevice { kind: DeviceKind::NotQmkNotifier,
                vendor_id: 0x3434, product_id: 0x0123, product_name: Some("Keychron".into()),
                ..capable.clone() };
            let (n, vp, c) = picker_columns(&capable);
            assert_eq!(n, "Dactyl");
            assert_eq!(vp, "0xFEED:0x0000");
            assert!(c.starts_with('\u{2713}') && c.contains("qmk_notifier"), "cap: {c}");
            let unnamed = ClassifiedDevice { product_name: None, kind: DeviceKind::NotQmkNotifier,
                vendor_id: 0x3434, product_id: 0x0123, ..capable.clone() };
            let (n2, vp2, c2) = picker_columns(&unnamed);
            assert_eq!(n2, "(unnamed)");
            assert_eq!(vp2, "0x3434:0x0123");
            assert!(c2.starts_with('\u{2717}') && c2.contains("QMK board, no module"), "cap: {c2}");
        }

        #[test]
        fn test_parse_vidpid() {
            assert_eq!(parse_vidpid("0xFEED:0x0000"), Some((0xFEED, 0x0000)));
            assert_eq!(parse_vidpid("feed:0x123"), Some((0xFEED, 0x0123)));
            assert_eq!(parse_vidpid("  0xFEED:0x0000  "), Some((0xFEED, 0x0000))); // trimmed
            assert_eq!(parse_vidpid(""), None);
            assert_eq!(parse_vidpid("feed"), None);        // no colon
            assert_eq!(parse_vidpid("feed:"), None);       // missing pid
            assert_eq!(parse_vidpid(":123"), None);        // missing vid
            assert_eq!(parse_vidpid("garbage:x"), None);   // non-hex vid
            assert_eq!(parse_vidpid("0xFEED:0x0000|extra"), None); // splitn(2): pid half has stray chars
        }
  - NOTE: ClassifiedDevice derives Clone (notifier.rs:842) + DeviceKind derives
        Clone/PartialEq (notifier.rs:817) ⇒ `..capable.clone()` works.

Task 5: VERIFY build + tests (single-threaded, per AGENTS.md)
  - RUN: cargo build --bin qmkonnect --features linux-tray   (Linux: full dialog;
        Windows/macOS: module cfg'd out ⇒ unchanged).
  - RUN: cargo test --bin qmkonnect -- --test-threads=1   (existing tests +
        test_picker_columns + test_parse_vidpid pass).
  - CONFIRM git status shows EXACTLY one file: src/linux_tray.rs.
  - MANUAL (Linux only, per AGENTS.md dev loop): cargo build; run the tray
        (Waybar/SwayNC/KDE/GNOME-SNI); tray menu → Settings…; verify the 3 picker
        cases against real hardware (≥2 boards, 1 capable+no-VID/PID, 0 boards);
        verify a picker pick writes config.toml + fires the notify + skips --forms;
        verify cancel-falls-through-to-Advanced; verify manual entry still writes;
        verify apply_device_rule/pkexec still prompts.
```

### Implementation Patterns & Key Details

```rust
// The zenity --list argv (Task 3) — G5 --print-column=2, G11 separate args:
// let mut args: Vec<String> = vec![
//     "--list".into(), "--title=QMK Settings".into(), "--print-column=2".into(),
//     "--hide-header".into(), "--width=520".into(),
//     "--text=Select a detected keyboard (or Cancel for manual entry):".into(),
//     "--column=Device".into(), "--column=VID:PID".into(), "--column=Capability".into(),
// ];
// for d in devices {
//     let (name, vidpid, cap) = picker_columns(d);
//     args.push(name); args.push(vidpid); args.push(cap);
// }

// The selection gate (Task 3) — G4 success+non-empty-stdout:
// let out = match Command::new("zenity").args(args.iter().map(String::as_str))
//     .stdout(Stdio::piped()).stderr(Stdio::null()).output() {
//     Ok(o) if o.status.success() => o,
//     _ => return None,
// };
// let stdout = String::from_utf8_lossy(&out.stdout);
// if stdout.trim().is_empty() { return None; }
// parse_vidpid(&stdout)

// The dialog flow (Task 2) — chosen-first, else fall through to --forms:
// let chosen = if picker { run_device_picker(&devices) } else { None };
// if let Some((v, p)) = chosen {
//     save_and_notify(Some(v), Some(p));
//     return;                       // SKIP the --forms (the disambiguation is done)
// }
// // …fall through to the existing zenity --forms (Advanced / manual override)…

// The --text per case (Task 2c) — G14:
// let prefix = if devices.is_empty() {
//     "No QMK keyboards detected. Enter IDs manually below.".into()
// } else if clean_auto {
//     format!("Detected: {}. Auto-discovery is active.",
//         devices[0].product_name.as_deref().unwrap_or("(unnamed)"))
// } else {
//     "Advanced / manual override — enter hex VID/PID.".into()
// };
```

### Integration Points

```yaml
CODE (this task):
  - file: src/linux_tray.rs
    change: "Linux-only additive (by module gate) — picker_columns + parse_vidpid +
             current_config_vidpid + save_and_notify + run_device_picker + REWRITE
             show_settings_dialog_linux (classify + 3 cases + zenity --list picker +
             --forms Advanced fallback + chosen-first-else-manual) + 2 tests;
             optionally refactor current_config_hex to derive from current_config_vidpid."
    pattern: "zenity Command::output() matches the existing --forms idiom; picker_columns
              mirrors the siblings' picker_row_text (3 columns vs 1 line); save_and_notify
              is a verbatim extraction; current_config_vidpid mirrors current_config_hex's
              first-existing-candidate search."

DEPENDENCIES (this task): NONE new. No Cargo change. std::process::Command + std::io
                           already used. classify_devices/ClassifiedDevice/DeviceKind are
                           cross-platform pub in notifier.rs (imported inside the
                           linux-gated module).

UPSTREAM (consumed read-only):
  - crate::core::notifier::classify_devices(verbose: bool) -> Vec<ClassifiedDevice> (notifier.rs:1116).
  - crate::core::notifier::DeviceKind { Capable{..}, NotQmkNotifier } (notifier.rs:816).
  - crate::core::notifier::ClassifiedDevice { vendor_id:u16, product_id:u16, product_name:Option<String>, kind:DeviceKind, .. } (notifier.rs:841).
  - crate::core::parse_config / crate::platforms::get_config_paths (for current_config_vidpid).
  - write_config @859 + apply_device_rule @795 + parse_id @883 + notify @899 (reused; tail → save_and_notify).

DOWNSTREAM / SIBLINGS (do NOT implement them here):
  - P3.M2.T1.S1 (Windows Win32 picker — Complete): shares the chosen-first-else-manual
    SEMANTICS (Windows uses a static DIALOG_RESULT; Linux uses locals — no static needed).
  - P3.M2.T1.S2 (macOS NSAlert picker — in flight): same result shape; macOS uses locals too.
  - P4.M1.T1.S2 (Mode-A doc sync): will cite this dialog in README/UI docs.
  - P4.M2.T1.S1 (Mode-A doc sync): will cite this dialog in README/UI docs.

NO OVERLAP:
  - Windows/macOS dialogs (tray.rs): P3.M2.T1.S1/S2 — UNTOUCHED (separate file; picker_columns
    is linux-module-local, no symbol collision with the siblings' picker_row_text).
  - classify_devices / cache / DeviceStatus (notifier.rs): P3.M1 / P1 — Complete, read-only.
  - show_window_info_linux (linux_tray.rs:383): the native-GTK + zenity-fallback window-info
    dialog — UNTOUCHED (its comment is only CITED in the doc-comment).

CONFIG: none (no config schema change — VID/PID stays Option<u16>). ROUTES: none. DATABASE: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect --features linux-tray
# Expected: compiles clean on Linux. On Windows/macOS the module is cfg'd out (no change).
# If it fails on Linux: most likely a missing import (classify_devices/ClassifiedDevice/
# DeviceKind from crate::core::notifier), a borrow/type issue in run_device_picker
# (args.iter().map(String::as_str)), or a type mismatch on save_and_notify's args
# (chosen → (Some(v),Some(p)); manual → (v,p)) — READ + fix.

# Confirm the deliverables are present (the module is linux-gated — grep finds them
# regardless of host):
grep -n 'fn picker_columns' src/linux_tray.rs          # expect 1
grep -n 'fn parse_vidpid' src/linux_tray.rs            # expect 1
grep -n 'fn current_config_vidpid' src/linux_tray.rs   # expect 1
grep -n 'fn save_and_notify' src/linux_tray.rs         # expect 1
grep -n 'fn run_device_picker' src/linux_tray.rs       # expect 1
grep -c '\-\-print-column=2\|\-\-list\b' src/linux_tray.rs   # expect >=2 (--list + the forms --list-values is different)
grep -c 'classify_devices' src/linux_tray.rs           # expect >=1
grep -n 'fn show_settings_dialog_linux' src/linux_tray.rs    # expect 1
```

### Level 2: Unit Tests (the pure helpers)

```bash
cd /home/dustin/projects/qmkonnect
# Single-threaded is MANDATORY crate-wide (shared MockNotifier globals + DebounceState, AGENTS.md).
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL green — the existing tests (status_text_uses_parity_glyphs,
# parse_id_handles_prefix_case_and_auto, color_scheme_parser_matches_spec,
# embedded_icons_decode) + the new test_picker_columns + test_parse_vidpid.
# The zenity Command invocations (run_device_picker / show_settings_dialog_linux)
# are NOT unit-testable (they spawn a real GTK GUI); covered by Level-4 manual check.

cargo test --bin qmkonnect picker_columns -- --test-threads=1   # filter to the new tests
cargo test --bin qmkonnect parse_vidpid   -- --test-threads=1
```

### Level 3: Cross-platform regression (the new code is Linux-only)

```bash
cd /home/dustin/projects/qmkonnect
# On Windows/macOS (or a Linux build WITHOUT --features linux-tray): confirm the
# linux-gated additions don't break the build.
cargo build --bin qmkonnect
# Expected: clean — every new item is inside the #[cfg(all(linux, linux-tray))]
# module, so a non-Linux/no-linux-tray host compiles the rest unchanged.

# Confirm the change surface is exactly one file:
git status --short
# Expected: only src/linux_tray.rs modified. NOTHING in Cargo.toml, core/, tray.rs,
# architecture/, docs/, spec/, packaging/.
git diff --stat
# Expected: 1 file: src/linux_tray.rs.
```

### Level 4: Manual dialog testing (Linux only — per AGENTS.md dev loop)

```bash
# The zenity dialogs are real GUIs; they CANNOT be exercised by a unit test.
# Verify on a Linux desktop with an SNI-hosting bar (Waybar/SwayNC/KDE/GNOME-SNI):
cargo build --bin qmkonnect --features linux-tray
./target/debug/qmkonnect   # or run via your session; ensure the tray icon appears
# Then: tray menu → Settings… Verify against real hardware:
#  CASE A (≥2 Tier-1 boards, or 1 board + 1 VIA board): the --list pops with one
#         row per device: "<name> | 0xVID:0xPID | ✓ qmk_notifier" or "✗ QMK board,
#         no module". Click the capable row → OK → config.toml gets that VID/PID, a
#         "settings saved" notify fires, and NO --forms appears.
#  CASE B (1 capable board, config has no VID/PID): the --list is SKIPPED; the
#         --forms opens with text "Detected: <name>. Auto-discovery is active."
#         OK with blank fields writes None/None (auto) — config unchanged.
#  CASE C (0 boards): the --list is SKIPPED; the --forms opens with text "No QMK
#         keyboards detected. Enter IDs manually below."
#  Cancel the --list: the --forms (Advanced / manual override) opens. Type a hex
#         pair + OK → that pair is written + the pkexec prompt fires (≥1 Some).
#         Blank/auto + OK → None/None (auto-discovery, no pkexec). Cancel → no write.
#  Picker pick + verify precedence: a pick writes the row's VID/PID (chosen wins);
#         the --forms is skipped entirely, so there's no manual to conflict.
# Expected: all 3 cases render correctly; a pick writes the right VID/PID + skips
#         the --forms; cancel-falls-through-to-Advanced works; manual entry still
#         writes; apply_device_rule/pkexec behavior unchanged.
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect --features linux-tray` clean on Linux.
- [ ] `cargo build --bin qmkonnect` clean on Windows/macOS (module cfg'd out).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` green (existing + 2 new tests).
- [ ] `git status` shows exactly ONE modified file: `src/linux_tray.rs`.

### Feature Validation (contract fidelity)
- [ ] **`picker_columns`**: `Capable{..}` ⇒ `0xFEED:0x0000` + `✓ qmk_notifier`;
      `NotQmkNotifier` ⇒ `✗ QMK board, no module`; `(unnamed)` when `product_name` is `None`.
- [ ] **`parse_vidpid`**: `"0xFEED:0x0000"` → `Some((0xFEED,0))`; malformed → `None`.
- [ ] **3 picker cases**: empty ⇒ skip --list + `--forms` "No QMK keyboards detected…";
      clean-auto (1 capable + no VID/PID) ⇒ skip --list + `--forms` "Detected: <name>…"
      (NO VID/PID written — zero-config); picker ⇒ `zenity --list` shown.
- [ ] **Picker selection** ⇒ `save_and_notify(Some(v),Some(p))` + `return` (skips --forms).
- [ ] **Picker cancel/no-selection** ⇒ fall through to the `--forms` (Advanced / manual override).
- [ ] **chosen-first-else-manual precedence**: pick ⇒ chosen; cancel-fallthrough ⇒ manual;
      both cancelled ⇒ no write.
- [ ] **`apply_device_rule`/pkexec UNCHANGED** (contract): both None ⇒ no rule; ≥1 Some ⇒ pkexec.
- [ ] **`--text` reflects the case** (G14); current values (current_config_hex) shown in all.
- [ ] **Mode-A doc-comment** cites `spec/DEVICE_DISCOVERY.md` §5 + `spec/UI.md` §2.3; notes the
      two-dialog design, no-Rescan (G3), and the `--list` tiling tradeoff (G6).

### Code Quality Validation
- [ ] Follows existing `zenity` `Command::output()` idiom + the `parse_id`/`notify` patterns.
- [ ] `save_and_notify` is a verbatim extraction (no behavior change to the save path).
- [ ] All new code inside the `#[cfg(all(linux, linux-tray))]` module (zero Windows/macOS impact).
- [ ] No new Cargo deps; no `DIALOG_RESULT` static (Linux uses locals).
- [ ] Doc-comment is self-documenting (Mode A).

### Documentation & Deployment
- [ ] Code is self-documenting with clear variable/function names.
- [ ] The tiling + no-Rescan deviations are documented in the doc-comment (Mode A).

---

## Anti-Patterns to Avoid

- ❌ Don't combine the `--list` and `--forms` into one zenity call (impossible; `--forms`
  caps list height). They are two sequential `Command::output()` calls.
- ❌ Don't treat an empty-stdout `--list` OK as a selection (OK-with-no-selection ⇒ exit 0 +
  empty stdout ⇒ fall through, G4).
- ❌ Don't write a VID/PID in CASE B (clean-auto) — that breaks zero-config (§5.1). "auto-select"
  means skip the picker, NOT write the board's ID (G8).
- ❌ Don't print column 1 (Device name) or column 3 (Capability) — they're not unique/parseable
  back to a VID/PID. Use `--print-column=2` (G5).
- ❌ Don't change `apply_device_rule`/pkexec (contract: keep it unchanged). `save_and_notify`
  is a verbatim extraction (G10).
- ❌ Don't spawn a thread for the dialog (out of scope; the existing dialog already blocks the
  ksni thread; keep it consistent).
- ❌ Don't add `classification_cache_clear()` on Settings-open (parity with S1/S2; the cache is
  warm from the status poll; re-open after 5s TTL = fresh probe).
- ❌ Don't add a `[Rescan]` button (sequential dialogs; re-open to refresh — G3).
- ❌ Don't ignore failing tests — fix them.

---

## Confidence Score: 9/10

**Why high:** the edit site is a single self-contained function in a single file that no
parallel task touches (stable line numbers); the consumer API (`classify_devices`/
`ClassifiedDevice`/`DeviceKind`) is verified `pub` in-tree; the zenity `--list`/
`--print-column`/exit-code mechanics are pinned from the man page; the `save_and_notify`
refactor is a verbatim extraction (low risk); and the 3-case logic + chosen-first-else-
manual precedence are fully specified with a clear mapping to the macOS/Windows siblings.

**The 1-point residual risk:** the `--list` tiling behavior on pure tiling WMs (G6) is a
real UX wart that the contract explicitly accepts and the doc-comment documents; manual
Level-4 verification on a tiling compositor is the only way to confirm the tiled list is
still usable (it should be, for 1-3 rows). There is no code-level risk here — only a
documented UX tradeoff.