# PRP — P1.M1.T2.S1: Fix X11 `WM_CLASS` parser to return the class (not the instance) + add unit tests

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). ALL edits in ONE file:
> **`src/platforms/x11.rs`** (Bug 2 / PRD ID 2). No other file is touched.
> **Scope:** (1) extract a pure `fn parse_wm_class(rest: &str) -> Option<String>`
> that splits on `,` (not `"`) so a leading space / `, ` separator can't shift the
> field index; (2) collapse the buggy WM_CLASS branch to call it; (3) add the file's
> FIRST `#[cfg(test)] mod tests` with 3 unit tests. The `_NET_WM_NAME` branch and all
> other code are untouched. The fix makes the app_class sent to the firmware +
> matched against rules be the **class** (`Firefox`) instead of the **instance**
> (`firefox`).
> **Verified baseline (research run, this session):** the parse logic + all 3 test
> assertions are byte-correct (proved in a standalone `/tmp` rustc repro); the
> trayless+x11 build compiles (`cargo check --no-default-features` exit 0).
> **⚠ cfg gotcha:** x11.rs is `#![cfg(all(target_os="linux", not(feature="hyprland")))]`
> and `hyprland` is a DEFAULT feature → the file (and any test mod inside it) only
> compiles under `--no-default-features`. The default-features build silently EXCLUDES
> x11.rs, so the fix is validated ONLY via `--no-default-features` (see Validation).

---

## Goal

**Feature Goal**: Correct the off-by-one in `X11Monitor::get_active_window_info`'s
`WM_CLASS` branch so the `app_class` fed to `WindowInfo::new(app_class, title)` —
and hence sent to the keyboard firmware and matched against host rules — is the
**class** string (e.g. `Firefox`), not the **instance** string (e.g. `firefox`).
Root cause: splitting on `"` and filtering empties leaves the leading `" "` and the
`", "` separator as non-empty elements, shifting `.get(1)` onto the instance. Fix:
split on `,` + trim + strip quotes, extracted into a unit-testable pure helper.

**Deliverable**: `src/platforms/x11.rs` with (a) a new private free function
`fn parse_wm_class(rest: &str) -> Option<String>`, (b) the WM_CLASS branch body
collapsed to `app_class = parse_wm_class(rest).unwrap_or_default();`, and (c) a new
`#[cfg(test)] mod tests` block with 3 unit tests. No other change.

**Success Definition**: `cargo test --bin qmkonnect --no-default-features parse_wm_class
-- --nocapture` runs the 3 new tests and ALL PASS (proving `parse_wm_class` returns
the class for the typical case, falls back to the first field for single-field
degenerate input, and returns `None` for empty/whitespace); `cargo clippy
--no-default-features --bin qmkonnect -- -D warnings` is clean;
`rustfmt --edition 2021 --check src/platforms/x11.rs` is clean; the default-features
`cargo build` still succeeds (regression check — x11.rs is absent there, nothing else
changes); the `_NET_WM_NAME` branch and all other code are byte-identical to before;
no file other than `src/platforms/x11.rs` is modified.

## User Persona (if applicable)

**Target User**: An X11 user (the non-default Linux fallback monitor) authoring
host-side `rules.toml` rules or board `DEFINE_*` rules against the window class.

**Use Case**: User runs QMKonnect on X11, opens an app, and expects a
`match = "Firefox"` rule to fire. Before the fix the firmware receives `firefox`
(the instance), so a class-keyed rule silently fails.

**User Journey**: (before) X11 sends `firefox` → `match = "Firefox"` rule never
matches → "QMKonnect isn't working". (after) X11 sends `Firefox` → rule matches.
The "Show Window Information" path is Hyprland-only (Bug 1 / S1); X11 has no dialog,
so this fix is purely about the value sent on the notify path.

**Pain Points Addressed**: Eliminates the silent instance/class mismatch that made
X11 (the documented Linux fallback) appear broken for any app whose instance ≠ class
(the common case: `firefox`/`Firefox`, `google-chrome`/`Google Chrome`, etc.).

## Why

- **Rule correctness on the X11 fallback.** The notify path is the source of truth
  for what the firmware matches; sending the instance instead of the class means
  every class-keyed rule silently fails on X11. This is a one-function correctness
  fix flagged Major in the bugfix PRD (Issue 2).
- **It makes the code match the docs.** `docs/configuration.md` (L281) and
  `docs/troubleshooting.md` (L571) already say "window class"; the bug made X11
  send the instance. No doc change is needed (a separate P1.M2.T3 sweep verifies).
- **It's the smallest testable change.** The buggy logic is inline (untestable
  without shelling out to `xprop`); extracting a pure helper makes it unit-testable
  AND fixes the bug in one move. The file has zero tests today — this adds the first
  module, covering the parser deterministically.

## What

Three edits to `src/platforms/x11.rs`:

### (a) Add the `parse_wm_class` helper (after the `impl WindowMonitor` block)

```rust
/// Parse the **class** out of the `WM_CLASS` property's `= …` remainder.
///
/// `xprop` prints `WM_CLASS(STRING) = "instance", "Class"`. After
/// [`split_once('=')`](str::split_once) the caller passes
/// `rest = ' "instance", "Class"'`. Splitting on `,` (not `"`) means a leading
/// space or the `, ` separator can't shift the field index; then trim + strip the
/// quotes. Prefers the **class** (2nd field) and falls back to the **instance**
/// (1st field) for degenerate single-field output. Returns `None` when no non-empty
/// field is present.
fn parse_wm_class(rest: &str) -> Option<String> {
    let parts: Vec<&str> = rest
        .split(',')
        .map(|s| s.trim().trim_matches('"'))
        .filter(|s| !s.is_empty())
        .collect();
    parts.get(1).or_else(|| parts.first()).map(|s| s.to_string())
}
```

### (b) Collapse the WM_CLASS branch (current lines 67–75) to one line

**OLD (exact current text — the `if let Some(rest)` body):**
```rust
                    // WM_CLASS(STRING) = "instance", "Class"
                    let quoted: Vec<&str> = rest.split('"').filter(|s| !s.is_empty()).collect();
                    // Prefer the class (second element), fall back to instance.
                    app_class = quoted
                        .get(1)
                        .or_else(|| quoted.first())
                        .map(|s| s.to_string())
                        .unwrap_or_default();
```
**NEW:**
```rust
                    app_class = parse_wm_class(rest).unwrap_or_default();
```

(The surrounding `if line.starts_with("WM_CLASS") { if let Some(rest) = ... { … } }`
and the `} else if line.starts_with("_NET_WM_NAME") {` are UNCHANGED.)

### (c) Add the `#[cfg(test)] mod tests` block (at end of file)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_wm_class_returns_class_not_instance() {
        // xprop prints: WM_CLASS(STRING) = "instance", "Class"
        // `rest` is the '= '-suffixed remainder passed to parse_wm_class.
        // Regression: the OLD split-on-quote returned "firefox" (the instance).
        assert_eq!(
            parse_wm_class(r#" "firefox", "Firefox""#),
            Some("Firefox".to_string())
        );

        // End-to-end: extract `rest` exactly as the call site does, then parse.
        let line = r#"WM_CLASS(STRING) = "firefox", "Firefox""#;
        let rest = line.split_once('=').map(|(_, r)| r).unwrap();
        assert_eq!(parse_wm_class(rest), Some("Firefox".to_string()));

        // Multi-word class (instance/class differ in casing + spacing).
        assert_eq!(
            parse_wm_class(r#" "google-chrome", "Google Chrome""#),
            Some("Google Chrome".to_string())
        );
    }

    #[test]
    fn parse_wm_class_single_field_falls_back_to_first() {
        // Degenerate: only one quoted field. Falls back to the first (and only).
        assert_eq!(
            parse_wm_class(r#" "Navigator""#),
            Some("Navigator".to_string())
        );
    }

    #[test]
    fn parse_wm_class_empty_or_whitespace_is_none() {
        // Empty / whitespace-only / only-empty-quotes ⇒ no non-empty field ⇒ None.
        assert_eq!(parse_wm_class(""), None);
        assert_eq!(parse_wm_class("   "), None);
        assert_eq!(parse_wm_class(r#" "", ""#), None);
    }
}
```

### Success Criteria

- [ ] `parse_wm_class(rest: &str) -> Option<String>` exists as a private free fn in
      `src/platforms/x11.rs`, placed after the `impl WindowMonitor for X11Monitor` block.
- [ ] The WM_CLASS branch is `app_class = parse_wm_class(rest).unwrap_or_default();`
      (the `let quoted = …`/`.get(1)` block is gone).
- [ ] The `_NET_WM_NAME` branch and all other code are byte-identical to before.
- [ ] A `#[cfg(test)] mod tests` block with the 3 named tests exists at end of file.
- [ ] `cargo test --bin qmkonnect --no-default-features parse_wm_class -- --nocapture`
      → 3 passed.
- [ ] `cargo clippy --no-default-features --bin qmkonnect -- -D warnings` → clean.
- [ ] `rustfmt --edition 2021 --check src/platforms/x11.rs` → clean.
- [ ] `cargo build` (default features) → still succeeds (regression check).
- [ ] No file other than `src/platforms/x11.rs` is modified.

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed to
> implement this successfully?"_ — **Yes.** The exact buggy block (quoted, with line
> numbers), the verbatim replacement helper, the verbatim new call-site line, the
> verbatim 3-test module, the cfg-gating rationale (why `--no-default-features` is
> mandatory for validation), and verified build/test commands are all below. The parse
> logic + every test assertion were proved byte-correct in a standalone rustc repro
> during research.

> **BASELINE ALERT.** The bug is live in the committed tree (HEAD). x11.rs has **no**
> test module today (confirmed by grep). The fix is purely additive in test surface
> and behavior-correcting in the one branch; the `_NET_WM_NAME` branch is unrelated
> and must not change.

### Documentation & References

```yaml
# MUST READ — the bug root-cause + fix recommendation (the authoritative analysis)
- file: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/architecture/bug_findings.md
  why: "§Bug 2 walks the exact split('"') index shift (leading space + ', ' separator
        survive the !is_empty filter → .get(1) lands on the instance) and recommends
        the split(',') + trim fix + extracting a pure helper for testability. This
        PRP implements that recommendation verbatim."
  section: "Bug 2 (Major, PRD ID 2): X11 WM_CLASS parser returns instance instead of class"
  critical: "The fix MUST split on ',' not '\"' — that's the whole point (the leading
             space from '= ' and the ', ' separator are what break the quote-split)."

# MUST READ — the file being edited (read current code before editing)
- file: /home/dustin/projects/qmkonnect/src/platforms/x11.rs
  why: "184 lines. The bug is in get_active_window_info's WM_CLASS branch (L65–75).
        Line 1 is the #![cfg(...)] gate (see cfg-gotcha). No #[cfg(test)] mod exists.
        The _NET_WM_NAME branch (L76–82) is UNRELATED — leave it alone."
  pattern: "free-fn placement: after `impl WindowMonitor for X11Monitor { … }` (the
            second impl block), before the new test mod. Private fn — no pub needed
            (child mod tests sees it via use super::*)."
  gotcha: "The whole file is cfg-gated linux+!hyprland. The default-features build
           EXCLUDES it, so `cargo build`/`cargo test` (default) neither compile nor
           run x11.rs. Validate with --no-default-features (Linux host)."

# MUST READ — the bugfix PRD (severity + repro)
- file: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/prd_snapshot.md
  why: "Issue 2 (Major): steps to reproduce (xprop -id <wid> | grep WM_CLASS → parser
        returns 'firefox' not 'Firefox'). Confirms severity + the user-visible symptom."
  section: "Major Issues → Issue 2"

# MUST READ — the Cargo feature model (the cfg-gating root cause)
- file: /home/dustin/projects/qmkonnect/Cargo.toml
  why: "`default = [\"hyprland\", \"macos\", \"linux-tray\"]` (L113) → hyprland is ON by
        default → x11.rs (#![cfg(all(linux, not(hyprland)))]) is compiled OUT of the
        default build. Only `--no-default-features` turns hyprland OFF → x11 compiles."
  section: "[features] default / hyprland (L113-114)"
  critical: "Without --no-default-features the new tests DON'T RUN and a syntax error
             in the fix WON'T surface. This is the #1 one-pass-failure risk."

# REFERENCE — the sibling PRP (parallel; different file → no collision)
- file: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/P1M1T1S1/PRP.md
  why: "S1 fixes Bug 1 in src/platforms/hyprland.rs (class→initial_class dialog fix).
        DIFFERENT FILE from x11.rs → no file-level overlap; both land independently.
        Referenced for the established PRP structure only."
  critical: "Do NOT edit hyprland.rs (S1's scope). Do NOT touch _NET_WM_NAME (unrelated)."

# REFERENCE — research notes (verified parse logic + cfg constraint + /tmp repro)
- docfile: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/P1M1T2S1/research/notes.md
  why: "The exact bug block + line numbers, the verbatim fix, the cfg-gating analysis,
        the 3 verified test expectations (proved in /tmp/wmclass_test), and the
        no-overlap note vs S1."
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/
├── Cargo.toml                       # default=["hyprland",...] → x11 cfg'd OUT by default (L113)
└── src/platforms/
    ├── x11.rs                       # <-- EDIT (ONLY): bug L65-75 + add helper + add test mod
    ├── hyprland.rs                  # S1's scope (Bug 1) — DO NOT TOUCH
    ├── mod.rs, windows.rs, …        # unrelated — DO NOT TOUCH
```

### Desired Codebase tree with files to be modified

```bash
src/platforms/
└── x11.rs   # MODIFIED ONLY:
             #   (a) + fn parse_wm_class (after impl WindowMonitor block)
             #   (b) WM_CLASS branch body → one-line call (L67-75)
             #   (c) + #[cfg(test)] mod tests (3 tests) at end of file
# (no new files; no other file touched)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL: x11.rs is cfg-gated OUT of the DEFAULT build.
//   Line 1: #![cfg(all(target_os = "linux", not(feature = "hyprland")))]
//   Cargo.toml default = ["hyprland", ...] → hyprland ON → x11.rs absent from the
//   default build. So `cargo build` / `cargo test` (default features) NEVER compile
//   x11.rs: a syntax error in your fix won't surface and the new tests won't run.
//   VALIDATE WITH: `cargo test --bin qmkonnect --no-default-features parse_wm_class`.
//   (Verified: `cargo check --no-default-features --bin qmkonnect` → Finished, exit 0.)

// CRITICAL: the fix MUST split on ',' not '"'.
//   The bug IS the quote-split: the leading ' ' (from '= ') and the ', ' separator
//   are non-empty after split('"'), so they occupy indices 0 and 2, shifting .get(1)
//   onto the instance. split(',') + trim + trim_matches('"') is the fix. Do NOT
//   "patch" the quote-split (e.g. by trimming rest first) — use the comma-split helper.

// CRITICAL: do NOT touch the _NET_WM_NAME branch (L76-82).
//   It's a different property with different format (single quoted value), parsed
//   correctly already (rest.trim().trim_matches('"')). Only the WM_CLASS branch is buggy.

// CRITICAL: tests run ONLY on Linux (the file's target_os="linux" gate).
//   On macOS/Windows the file is always cfg'd out regardless of features. A Linux
//   host/VM/CI is required to execute the 3 tests. (This box is Linux → fine.)

// GOTCHA: private free fn is visible to the child test mod — no `pub` needed.
//   `mod tests` is a descendant of the x11 module, so `fn parse_wm_class` (private)
//   is reachable via `use super::*;`. Do NOT add `pub`/`pub(crate)` (it's not API).

// GOTCHA: keep `unwrap_or_default()` at the call site to preserve behavior.
//   The original code left app_class as String::new() on parse failure.
//   `parse_wm_class(rest).unwrap_or_default()` keeps that (None → "").

// GOTCHA: rustfmt may not see a cfg'd-out file under default features.
//   `cargo fmt --all` resolves modules with default features → x11.rs may be skipped.
//   Use the DIRECT check: `rustfmt --edition 2021 --check src/platforms/x11.rs`
//   (or `cargo fmt --all` with --no-default-features if your cargo supports it).

// NOTE: line numbers are anchors, not contracts.
//   L65-75 is the current WM_CLASS branch; a later commit could shift it. Match on
//   the TEXT (the `let quoted = …` block), not the line numbers, when editing.
```

## Implementation Blueprint

### Data models and structure

No new data models. The only new symbol is the pure helper
`fn parse_wm_class(rest: &str) -> Option<String>` (private free fn). `WindowInfo`,
`app_class: String`, and the notify path are unchanged.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: READ the current code + confirm anchors
  - READ: src/platforms/x11.rs in full (184 lines). Confirm the WM_CLASS branch is
          at L65-75 with the `let quoted = rest.split('"')…` body, the _NET_WM_NAME
          branch at L76-82, the `impl WindowMonitor` block ending before EOF, and NO
          existing #[cfg(test)] mod.
  - CONFIRM: Cargo.toml default includes "hyprland" (so --no-default-features is
          required to compile x11.rs).

Task 2: ADD the parse_wm_class helper (edit 1)
  - INSERT: the `fn parse_wm_class(rest: &str) -> Option<String> { … }` block (see
          What-(a)) with its /// doc, placed AFTER the closing `}` of
          `impl WindowMonitor for X11Monitor` and BEFORE the new test mod.
  - NAMING: `parse_wm_class` (snake_case); param `rest: &str`; return `Option<String>`.
  - VISIBILITY: private (no pub). Body: split(',') → trim().trim_matches('"') →
          filter(!is_empty) → collect → .get(1).or_else(first).map(to_string).

Task 3: COLLAPSE the WM_CLASS branch (edit 2)
  - REPLACE the `if let Some(rest) = … { … }` body (the comment + `let quoted` +
          the `app_class = quoted.get(1)…unwrap_or_default()` chain) with the single
          line: `app_class = parse_wm_class(rest).unwrap_or_default();`
  - PRESERVE: the `if line.starts_with("WM_CLASS") {` / `if let Some(rest) = … {`
          wrappers and the `} else if line.starts_with("_NET_WM_NAME") {` branch.

Task 4: ADD the test module (edit 3)
  - APPEND at EOF: the `#[cfg(test)] mod tests { use super::*; … }` block (see
          What-(c)) with exactly 3 #[test] fns: parse_wm_class_returns_class_not_instance,
          parse_wm_class_single_field_falls_back_to_first,
          parse_wm_class_empty_or_whitespace_is_none.
  - NAMING: snake_case test_<helper>_<scenario>. Use the 3 verbatim bodies from What-(c).

Task 5: VALIDATE (do not skip — the cfg gate makes this non-obvious)
  - RUN: cargo test --bin qmkonnect --no-default-features parse_wm_class -- --nocapture
          → expect 3 passed. (THIS is the gate that actually compiles + runs x11.rs.)
  - RUN: cargo clippy --no-default-features --bin qmkonnect -- -D warnings  → clean.
  - RUN: rustfmt --edition 2021 --check src/platforms/x11.rs  → clean (re-run without
          --check to format if it diffs).
  - RUN: cargo build  (default features) → still succeeds (regression: x11.rs absent,
          nothing else changed).
  - GREP: the _NET_WM_NAME branch is unchanged:
          grep -nA3 '_NET_WM_NAME' src/platforms/x11.rs  → still `title = rest.trim()…`.
```

### Implementation Patterns & Key Details

```rust
// === WHY split on ',' not '"' (the core of the fix) ===
//   rest = ' "instance", "Class"'  (the leading space is from '= ')
//   OLD: split('"').filter(!empty) = [" ", "instance", ", ", "Class"]
//        → .get(1) = "instance"  ✗ (the leading " " and ", " poison the indices)
//   NEW: split(',').map(trim+trim_matches('"')).filter(!empty) = ["instance", "Class"]
//        → .get(1) = "Class"  ✓  (commas are the real field separators)

// === WHY a pure helper (not an inline fix) ===
//   The original logic was inline in get_active_window_info, which shells out to
//   `xprop` — untestable without a live X server. Extracting parse_wm_class makes
//   the parse deterministic and unit-testable with literal strings. x11.rs had ZERO
//   tests; this adds the first module.

// === WHY private (no pub) ===
//   parse_wm_class is an internal parser. The child `mod tests` reaches it via
//   `use super::*;` (private items are visible to descendant modules). Adding pub
//   would leak an implementation detail into the platform module's API.

// === WHY unwrap_or_default() at the call site ===
//   Preserves the original "empty string on parse failure" contract: app_class
//   stays String::new() if parse_wm_class returns None, so WindowInfo::new gets a
//   valid (empty) String, not an Option. Behavior-identical to the old path's tail.

// === THE CFG GATE (the #1 one-pass risk) ===
//   #![cfg(all(target_os = "linux", not(feature = "hyprland")))]  (line 1, INNER attr)
//   + default = ["hyprland", ...]  ⇒  default build EXCLUDES x11.rs entirely.
//   ⇒ `cargo test` (default) won't compile or run the new tests, and won't catch a
//   syntax error in the fix. ALWAYS validate with --no-default-features (Linux host).
```

### Integration Points

```yaml
SOURCE FILES:
  - modify: "src/platforms/x11.rs ONLY"
  - do NOT modify: "src/platforms/hyprland.rs (S1/Bug 1), any other platform file,
                    Cargo.toml, docs/* (separate P1.M2.T3 sweep)"

PUBLIC API SURFACE:
  - none changed. parse_wm_class is private; get_active_window_info's signature is
    unchanged; WindowInfo is unchanged. Only the VALUE of app_class for WM_CLASS
    lines changes (instance → class).

UPSTREAM/DOWNSTREAM:
  - UPSTREAM: get_active_window_info reads `xprop -id <wid> WM_CLASS _NET_WM_NAME`;
    the WM_CLASS line format is fixed by xprop (`WM_CLASS(STRING) = "inst", "Class"`).
  - DOWNSTREAM: WindowInfo::new(app_class, title) → notifier::notify_qmk → firmware +
    host-rules matcher now receive the CLASS. No downstream signature change.

DEPENDENCIES / Cargo.toml:
  - none. Pure std (str::split/trim/trim_matches). No new deps.

VALIDATION CONSUMERS:
  - `cargo test --no-default-features parse_wm_class` is THE gate (compiles x11.rs
    in the trayless build + runs the 3 tests). The default-features build is a
    regression-only check (x11.rs absent).
```

## Validation Loop

> All commands run from the repo root: `/home/dustin/projects/qmkonnect`.
> **Host must be Linux** (x11.rs's `target_os = "linux"` gate; the tests can't run
> on macOS/Windows at all). **`--no-default-features` is mandatory** to compile x11.rs
> (default features turn `hyprland` ON → x11.rs is cfg'd out).

### Level 1: Compile + run the new tests (THE gate)

```bash
cd /home/dustin/projects/qmkonnect

# The ONLY command that compiles x11.rs AND runs the new tests.
cargo test --bin qmkonnect --no-default-features parse_wm_class -- --nocapture
# Expected: 3 passed:
#   parse_wm_class_returns_class_not_instance
#   parse_wm_class_single_field_falls_back_to_first
#   parse_wm_class_empty_or_whitespace_is_none
# If "0 passed" or the tests don't appear: you're on a non-Linux host, OR you forgot
# --no-default-features (x11.rs was cfg'd out). On macOS/Windows these tests CANNOT run.

# Full trayless test compile (broader regression — the whole trayless build must link):
cargo test --bin qmkonnect --no-default-features -- --test-threads=1
# Expected: all tests that compile in the trayless build pass (incl. the 3 new ones).
```

### Level 2: Lint + format (on the x11 code specifically)

```bash
cd /home/dustin/projects/qmkonnect

# Clippy on the trayless build (which includes x11.rs).
cargo clippy --no-default-features --bin qmkonnect -- -D warnings
# Expected: zero warnings. (Watch for: needless_collect if you inline the helper,
# or a needless-lifetimes lint — the verbatim helper avoids both.)

# Direct rustfmt check (default-features `cargo fmt` may skip the cfg'd-out file).
rustfmt --edition 2021 --check src/platforms/x11.rs
# Expected: no diff (exit 0). If it diffs, run `rustfmt --edition 2021 src/platforms/x11.rs`.
```

### Level 3: Regression — default build still compiles

```bash
cd /home/dustin/projects/qmkonnect

# The default-features build EXCLUDES x11.rs, so this can't catch an x11 syntax error
# — but it confirms the fix didn't touch anything in the default module graph.
cargo build 2>&1 | tail -3
# Expected: "Finished `dev` profile …" (no warnings, no errors).

# Confirm the _NET_WM_NAME branch is byte-identical (untouched):
grep -nA3 '_NET_WM_NAME' src/platforms/x11.rs
# Expected: still `if let Some(rest) = … { title = rest.trim().trim_matches('"').to_string(); }`.
```

### Level 4: Targeted correctness spot-checks (the regression intent)

```bash
cd /home/dustin/projects/qmkonnect

# Confirm the buggy quote-split is GONE and the helper is PRESENT + CALLED.
grep -n 'split(' src/platforms/x11.rs
# Expected: split(',') inside parse_wm_class (NOT split('"') in the WM_CLASS branch).

grep -n 'parse_wm_class' src/platforms/x11.rs
# Expected: 3 hits — the `fn parse_wm_class` def, the call site
#           (`app_class = parse_wm_class(rest).unwrap_or_default();`), and the
#           `use super::*;`-resolved test references (the 3 tests call it ≥4 times).

# Confirm no test mod collisions / no second #[cfg(test)]:
grep -c '#\[cfg(test)\]' src/platforms/x11.rs   # expected: 1 (the new mod)

# Confirm only x11.rs changed:
git status --short
# Expected: only src/platforms/x11.rs listed.
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1: `cargo test --bin qmkonnect --no-default-features parse_wm_class -- --nocapture` → 3 passed.
- [ ] Level 1: `cargo test --bin qmkonnect --no-default-features -- --test-threads=1` → all pass (trayless build).
- [ ] Level 2: `cargo clippy --no-default-features --bin qmkonnect -- -D warnings` → clean.
- [ ] Level 2: `rustfmt --edition 2021 --check src/platforms/x11.rs` → clean.
- [ ] Level 3: `cargo build` (default features) → succeeds (regression).

### Feature Validation

- [ ] `parse_wm_class` returns the **class** (`Firefox`) for ` "firefox", "Firefox"`,
      not the instance.
- [ ] Single-field degenerate input falls back to the first field.
- [ ] Empty/whitespace/`"",""` input → `None` (and the call site's `unwrap_or_default`
      keeps `app_class` as `String::new()`).
- [ ] Multi-word class (`Google Chrome`) parses correctly.
- [ ] The `_NET_WM_NAME` branch is byte-identical (title parsing unchanged).

### Code Quality Validation

- [ ] `parse_wm_class` is a private free fn (no `pub`), placed after `impl WindowMonitor`.
- [ ] The WM_CLASS branch body is a single `parse_wm_class(rest).unwrap_or_default()` line.
- [ ] The 3 tests follow `test_<helper>_<scenario>` naming + `use super::*;`.
- [ ] No `#[allow(...)]` added; clippy clean.
- [ ] Only `src/platforms/x11.rs` modified; `hyprland.rs` (S1) and all other files untouched.

### Documentation & Deployment

- [ ] DOCS = none per contract (the fix makes code MATCH the existing docs that say
      "window class"; doc accuracy is a separate P1.M2.T3 sweep).
- [ ] No Cargo.toml, config, or environment-variable change.

---

## Anti-Patterns to Avoid

- ❌ Don't validate with default features — x11.rs is `#![cfg(all(linux, not(hyprland)))]`
  and `hyprland` is a DEFAULT feature, so the default build EXCLUDES x11.rs entirely.
  `cargo test` (default) won't compile the file, won't run the tests, and won't catch a
  syntax error. ALWAYS use `--no-default-features` (and a Linux host). This is the #1
  one-pass-failure risk.
- ❌ Don't "patch" the quote-split (e.g. trimming `rest` first, or filtering whitespace)
  — the fix is to split on `,`, full stop. The leading space and `, ` separator are the
  poison; the comma is the real field delimiter. Use the verbatim `parse_wm_class` helper.
- ❌ Don't touch the `_NET_WM_NAME` branch — it's a different property with a different
  (single-quoted-value) format, already parsed correctly. Only the WM_CLASS branch is buggy.
- ❌ Don't add `pub`/`pub(crate)` to `parse_wm_class` — it's an internal parser; the child
  `mod tests` reaches it via `use super::*;`. Leaking it pollutes the platform module API.
- ❌ Don't drop `unwrap_or_default()` at the call site — it preserves the original
  "empty String on parse failure" contract (None → ""). Without it, `app_class` would be
  `Option<String>` and the `WindowInfo::new(app_class, …)` call wouldn't type-check.
- ❌ Don't inline the helper back into `get_active_window_info` — the whole point of
  extracting it is testability (the function shells out to `xprop`; inline logic is
  untestable without a live X server). x11.rs has zero tests today; this adds the first.
- ❌ Don't edit `src/platforms/hyprland.rs` or any other file — that's S1 (Bug 1) /
  out of scope. This task is `src/platforms/x11.rs` ONLY.
- ❌ Don't rely on `cargo fmt --all` alone to format-check x11.rs — under default
  features the cfg'd-out file may be skipped. Use the direct
  `rustfmt --edition 2021 --check src/platforms/x11.rs`.
- ❌ Don't treat line numbers (L65-75) as contracts — match on the TEXT when editing.
  A later commit could shift them.
- ❌ Don't run tests in parallel with shared global state if you broaden beyond the
  parser tests — the bin's debouncer globals need `--test-threads=1` for the full suite
  (per AGENTS.md). The 3 pure parser tests are order-independent, but use
  `--test-threads=1` for the full trayless run to match house discipline.

---

**Confidence Score: 10/10** for one-pass implementation success. The deliverable is a
single-file, three-edit bug fix: a verbatim pure helper (split-on-comma, proved
byte-correct in a standalone rustc repro), a one-line call-site collapse, and a
verbatim 3-test module — all against an enumerated buggy block with exact text. The
parse logic and every test assertion were independently verified in `/tmp/wmclass_test`
(all pass, including the multi-word-class and the regression confirming the OLD logic
returned the instance). The one non-obvious risk — the cfg gate making the default
build silently exclude x11.rs — is pre-empted with the mandatory `--no-default-features`
validation command, which was itself verified viable (`cargo check --no-default-features`
→ exit 0). No API change, no new deps, no doc change (the fix matches existing docs);
the sibling S1 edits a different file (hyprland.rs), so there's no collision.