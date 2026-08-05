# PRP — P1.M1.T3.S1: Quote the exe path in `src/autostart.rs current_exe_wide()`

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). ALL edits in ONE file:
> **`src/autostart.rs`** (Bug 3A / PRD ID 4 — the **app-side** Run-key writer).
> **Scope:** (1) modify `current_exe_wide()` so the null-terminated UTF-16 buffer
> wraps the path in double-quotes (`"<path>"\0`); (2) add the file's FIRST
> `#[cfg(test)] mod tests` with a test asserting the buffer starts with `0x0022` and
> the char before the final NUL is `0x0022`. `enable()`/`is_enabled()`/`disable()` are
> UNCHANGED (forwarding/presence/delete semantics make quoting transparent to them).
> **Verified baseline (research, this session):** the quoting logic is proved
> byte-correct in a standalone `/tmp` rustc repro; the default Linux build links
> (autostart.rs is cfg'd out here).
> **⚠ cfg gotcha:** autostart.rs line 11 is `#![cfg(target_os = "windows")]` (an INNER
> attribute gating the whole file) → the file + its test compile/run **only on
> Windows**. On the Linux dev box the file is cfg'd out: `cargo test` won't compile it
> or run the test. Validate the LOGIC via the repro; the test executes on a Windows
> host/CI. (Mirrors the sibling x11.rs PRP's cfg situation, inverted platform.)

---

## Goal

**Feature Goal**: Make the HKCU `Run` autostart value a **quoted** path so an install
path containing spaces (e.g. `C:\Users\John Doe\…\QMKonnect.exe`) is written as the
`REG_SZ` value `"C:\Users\John Doe\…\QMKonnect.exe"` — Windows then resolves it
correctly at login and it is no longer an unquoted-service-path security vector.
Root cause: `current_exe_wide()` builds a null-terminated UTF-16 buffer of the RAW
path (no quotes); `enable()` writes that buffer's bytes as `REG_SZ`. Fix: chain a
leading `"` (U+0022) + the path + a trailing `"` (U+0022) + the NUL.

**Deliverable**: `src/autostart.rs` with (a) `current_exe_wide()` rewritten so its
iterator chains `once(0x0022) → encode_wide → once(0x0022) → once(0)` (and a doc
comment explaining the quoting); (b) a new `#[cfg(test)] mod tests` block with a test
asserting the buffer starts with `0x0022`, ends with `0x0000`, and has `0x0022` at
`buf[buf.len()-2]`. No other change.

**Success Definition**:
- On a **Windows** host: `cargo test --bin qmkonnect current_exe_wide -- --nocapture`
  runs the new test and it PASSES (buffer starts with `0x0022`; char before the final
  NUL is `0x0022`); `cargo test --bin qmkonnect -- --test-threads=1` is green; `cargo
  build --release` succeeds.
- On **any** host: the quoting LOGIC is verified (standalone repro in research/notes.md
  — proved byte-correct); the default build still links (`cargo build` → Finished).
- `enable()`/`is_enabled()`/`disable()` and the `RegSetValueExW` call site are
  byte-identical to before; no file other than `src/autostart.rs` is modified.

## User Persona (if applicable)

**Target User**: A Windows user whose username (or install path) contains a space
(`John Doe`, `Program Files`, a custom install dir).

**Use Case**: User installs QMKonnect under a spaced path, enables "Open at Login",
reboots, and QMKonnect actually launches. Before the fix the unquoted `Run` value may
fail to execute (Windows mis-parses `C:\Users\John` as the exe) — autostart silently
breaks for any spaced path.

**User Journey**: (before) spaced path → unquoted `REG_SZ` → login launch fails
silently OR is an exploit vector. (after) spaced path → quoted `REG_SZ` → launches
reliably + no vector.

**Pain Points Addressed**: Silent autostart failure on spaced paths + the
unquoted-service-path security exposure (PRD Issue 3 / Major).

## Why

- **Login reliability + security on spaced Windows paths.** An unquoted `Run`-key
  value with a space is the textbook "Unquoted Service Path" defect: Windows may
  resolve the wrong executable or fail entirely, and a writable ancestor dir becomes
  a privilege-escalation vector. Quoting is the standard, one-line fix. Flagged Major
  in the bugfix PRD (Issue 3) + bug_findings.md §Bug 3A.
- **It's the smallest correct change.** The quoting belongs in `current_exe_wide()`
  (the buffer builder) — `enable()` forwards the buffer's bytes unchanged to
  `RegSetValueExW`, so quoting the builder fixes the value with zero downstream edits.
  `is_enabled()` is presence-based (not content), `disable()` deletes (content-agnostic).
- **Pairs with the installer fix (S2).** This is the APP-side writer; the
  INSTALLER-side writer (`QMKonnect.iss`) is P1.M1.T3.S2. Both must quote for the bug
  to be fully closed, but each is independent (different file).

## What

Two edits to `src/autostart.rs`:

### (a) Rewrite `current_exe_wide()` to quote the path

**OLD (exact current text, the body after the `Err` arm — the `OsStr::new(&path)…`
block):**
```rust
    OsStr::new(&path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
```

**NEW:**
```rust
    // Wrap the path in double-quotes (U+0022) before the NUL terminator, so a path
    // containing spaces (e.g. `C:\Users\John Doe\…`) is written to the HKCU `Run`
    // key as a quoted REG_SZ that Windows resolves correctly at login — and is not
    // an unquoted-service-path vector. Buffer layout: [0x0022, …path UTF-16…, 0x0022, 0x0000].
    std::iter::once(0x0022)
        .chain(OsStr::new(&path).encode_wide())
        .chain(std::iter::once(0x0022))
        .chain(std::iter::once(0))
        .collect()
```

Also update the function's `///` doc (currently "*`std::env::current_exe()` as a
null-terminated UTF-16 buffer — the layout `REG_SZ` expects. …*") to say it returns a
**quoted**, null-terminated buffer and why (spaces / security). Suggested:
```rust
/// `std::env::current_exe()` as a **quoted**, null-terminated UTF-16 buffer — the
/// layout the HKCU `Run` key's `REG_SZ` value expects. The path is wrapped in
/// double-quotes (`"…"`, U+0022) so a path containing spaces (e.g.
/// `C:\Users\John Doe\…`) resolves correctly at login and is not an unquoted-
/// service-path vector. Kept local rather than reusing `tray.rs`'s `to_wide_string`
/// so this module stays self-contained and the macOS branch merges cleanly.
```

> **Leave the error arm alone:** `Err(_) => return vec![0]` stays as-is (you can't
> quote a path you failed to resolve; a NUL-only `REG_SZ` write is benign + rare). Do
> NOT change it to `vec![0x0022, 0x0022, 0]` "for symmetry" — that would write a
> value containing only `""`, which is worse than an empty value.

### (b) Add the `#[cfg(test)] mod tests` block (at end of file)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_exe_wide_is_quoted() {
        // Bug 3A: the HKCU Run-key REG_SZ value must be a QUOTED path so a spaced
        // install path resolves at login. Assert the buffer layout:
        // [0x0022 …path… 0x0022 0x0000].
        let buf = current_exe_wide();

        assert!(!buf.is_empty(), "buffer must not be empty");
        assert_eq!(
            buf[0], 0x0022,
            "REG_SZ path must START with a double-quote (0x0022)"
        );
        // At least [quote, one path unit, quote, NUL].
        assert!(
            buf.len() >= 4,
            "buffer must be at least [0x0022, <char>, 0x0022, 0x0000]"
        );
        assert_eq!(
            *buf.last().unwrap(),
            0u16,
            "buffer must END with a NUL terminator (0x0000)"
        );
        assert_eq!(
            buf[buf.len() - 2],
            0x0022,
            "the char BEFORE the final NUL must be a double-quote (0x0022)"
        );
    }

    #[test]
    fn current_exe_wide_quotes_the_actual_exe_path() {
        // Stronger round-trip: the quoted content (between the quotes) must equal
        // std::env::current_exe() — i.e. the quotes wrap the REAL path, not garbage.
        let buf = current_exe_wide();
        // Drop the leading quote (buf[0]) and the trailing quote+NUL (last 2 units).
        let inner = &buf[1..buf.len() - 2];
        let decoded = String::from_utf16_lossy(inner);
        let expected = std::env::current_exe()
            .expect("current_exe must resolve under cargo test")
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            decoded, expected,
            "the quoted buffer must wrap the actual current_exe() path"
        );
    }
}
```

> The second test is a bonus round-trip that proves the quotes wrap the **real** path
> (not just that quotes are present). Both run only on Windows (the file's cfg gate).

### Success Criteria

- [ ] `current_exe_wide()` chains `once(0x0022) → encode_wide → once(0x0022) → once(0)`.
- [ ] The `Err` arm still returns `vec![0]` (unchanged).
- [ ] `current_exe_wide()`'s `///` doc mentions the quoting + the spaces/security rationale.
- [ ] A `#[cfg(test)] mod tests` block exists at end of file with
      `current_exe_wide_is_quoted` (the contract's minimal assertion) [+ the optional
      round-trip test].
- [ ] `enable()` / `is_enabled()` / `disable()` / the `RegSetValueExW` call are byte-identical.
- [ ] On Windows: `cargo test --bin qmkonnect current_exe_wide -- --nocapture` → pass.
- [ ] On any host: `cargo build` → Finished (regression; autostart.rs is cfg'd out on
      non-Windows, so this mainly confirms nothing else broke).
- [ ] No file other than `src/autostart.rs` is modified.

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed to
> implement this successfully?"_ — **Yes.** The exact buggy block (the `OsStr::new…
> .chain(once(0))` tail of `current_exe_wide`), the verbatim replacement, the verbatim
> doc update, the verbatim test module, the cfg-gating rationale (why the test runs
> only on Windows), the proof that enable/is_enabled/disable need no change, and the
> verified build/test commands are all below. The quoting logic was proved byte-correct
> in a standalone rustc repro during research.

> **BASELINE ALERT.** The bug is live in the committed tree (HEAD). autostart.rs has
> **no** test module today (confirmed by grep across the Windows-only files). The fix
> is one function body + one added test module; `enable()`/`is_enabled()`/`disable()`
> are untouched.

### Documentation & References

```yaml
# MUST READ — the authoritative bug analysis + fix recommendation
- file: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/architecture/bug_findings.md
  why: "§Bug 3 (A) walks the root cause (current_exe_wide returns the raw path;
        enable writes it as REG_SZ) and prescribes the exact fix: 'prepend \" (U+0022)
        and append \" before the NUL terminator'. Confirms is_enabled() presence-check
        still matches once quoted. This PRP implements that recommendation verbatim."
  section: "Bug 3 (Major, PRD ID 4) → Root Cause A + Fix A + Testing"
  critical: "The fix is QUOTES + NUL (in that order): [0x0022, path, 0x0022, 0x0000].
             Do NOT drop the NUL (REG_SZ requires it) and do NOT quote the error path."

# MUST READ — the file being edited (read current code before editing)
- file: /home/dustin/projects/qmkonnect/src/autostart.rs
  why: "~110 lines, all #[cfg(target_os=\"windows\")]. current_exe_wide() is the LAST
        fn (L99-110). enable() (L73) forwards its buffer's raw bytes to RegSetValueExW
        (no change needed). is_enabled() (L36) is presence-based (len>0, no change).
        disable() (L86) deletes (no change). No #[cfg(test)] mod exists today."
  pattern: "test-mod placement: at EOF, after current_exe_wide()'s closing brace.
            Private fn — the child test mod reaches it via `use super::*;` (no pub needed)."
  gotcha: "Line 11 `#![cfg(target_os = \"windows\")]` (INNER attr) gates the WHOLE file.
           On non-Windows the file is absent from the build — `cargo test` won't compile
           or run the new test. Validate logic via repro; execute test on Windows."

# MUST READ — the bugfix PRD (severity + repro)
- file: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/prd_snapshot.md
  why: "Issue 3 (Major): repro = install on a spaced username, enable autostart, check
        HKCU\\…\\Run for the unquoted path. Confirms severity + the user-visible symptom."
  section: "Major Issues → Issue 3"

# REFERENCE — the sibling PRP (S2: the INSTALLER-side fix; different file → no collision)
- file: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/P1M1T3S2/PRP.md
  why: "S2 (Planned) fixes packaging/windows/inno/QMKonnect.iss [Registry] — the
        INSTALLER writer of the SAME Run-key value (ValueData → quoted). DIFFERENT
        file from autostart.rs → no overlap. Both must land for a fully-fixed bug,
        but each is independent. Referenced for scope boundary only."
  critical: "Do NOT edit QMKonnect.iss here — that's S2. Do NOT edit enable/is_enabled/
             disable — quoting is transparent to them."

# REFERENCE — the parallel PRP (x11.rs Bug 2; different file → no collision; same cfg-test pattern)
- file: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/P1M1T2S1/PRP.md
  why: "T2.S1 (parallel) fixes src/platforms/x11.rs (Bug 2). DIFFERENT file → no
        overlap. It documents the SAME cfg-gate validation pattern (file cfg'd out of
        the default build → test runs only under a specific host/feature combo). Here
        the analog is: autostart.rs runs only on Windows (vs x11.rs runs only under
        --no-default-features on Linux)."
  section: "Known Gotchas (cfg gate), Validation Loop"

# REFERENCE — the Windows dev test loop (the host where the test actually runs)
- file: /home/dustin/projects/qmkonnect/AGENTS.md
  why: "The Windows loop: `cargo test --bin qmkonnect -- --test-threads=1` then
        `cargo build --release` (single-threaded tests for shared debouncer state).
        This is the command that compiles autostart.rs + runs the new test on Windows."
  section: "Windows dev test loop (PowerShell)"

# REFERENCE — research notes (verified quoting logic + cfg constraint + /tmp repro)
- docfile: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/P1M1T3S1/research/notes.md
  why: "The exact buggy block + the verbatim fix, the byte-correct /tmp repro proof
        (buf[0]=0x0022, buf[-2]=0x0022, buf[-1]=0), the cfg-gate analysis (Linux dev
        box can't run the test), and the is_enabled/disable/enable no-change rationale."
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/
└── src/
    ├── autostart.rs            # <-- EDIT (ONLY): current_exe_wide() L99-110 + add #[cfg(test)] mod tests at EOF
    │                           #     line 11: #![cfg(target_os = "windows")] → WHOLE file Windows-only
    ├── tray.rs                 # macOS SMAppService autostart (different mechanism) — DO NOT TOUCH
    ├── platforms/{windows,hyprland,x11,…}.rs  # unrelated — DO NOT TOUCH (x11.rs is T2.S1's scope)
    └── …
packaging/windows/inno/QMKonnect.iss   # S2's scope (installer Run-key value) — DO NOT TOUCH
```

### Desired Codebase tree with files to be modified

```bash
src/
└── autostart.rs   # MODIFIED ONLY:
                   #   (a) current_exe_wide() body → quotes + NUL (+ doc update)
                   #   (b) + #[cfg(test)] mod tests at EOF (current_exe_wide_is_quoted + round-trip)
# (no new files; enable/is_enabled/disable unchanged; QMKonnect.iss is S2's)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL: autostart.rs is cfg-gated OUT of NON-Windows builds.
//   Line 11: #![cfg(target_os = "windows")]  (INNER attribute — gates the WHOLE file)
//   ⇒ on Linux/macOS the file (and its test mod) is absent from the build. `cargo
//   build` / `cargo test` (default) neither compile nor run autostart.rs: a syntax
//   error in the fix won't surface and the new test won't run on a Linux host.
//   VALIDATE: (1) logic via the /tmp repro (proved byte-correct); (2) test execution
//   on a Windows host: `cargo test --bin qmkonnect current_exe_wide`. The default
//   `cargo build` here is a regression-only check (autostart.rs absent, nothing else
//   changed). (Verified: `uname -s` = Linux; `cargo build` → Finished, exit 0.)

// CRITICAL: the buffer layout MUST be [0x0022, …path…, 0x0022, 0x0000] — quotes
//   OUTSIDE the path, NUL LAST. Do NOT put the NUL before the trailing quote, do NOT
//   drop the NUL (REG_SZ REQUIRES a terminating NUL — enable()'s comment explicitly
//   notes current_exe_wide "already appends one"), and do NOT quote the error path.

// CRITICAL: leave the `Err(_) => return vec![0]` arm UNCHANGED. current_exe() failing
//   is rare (test binary always resolves it); on that path returning a NUL-only buffer
//   is benign (empty REG_SZ). "Quoting" it to [0x0022, 0x0022, 0] would write a value
//   containing just `""`, which is worse.

// CRITICAL: enable()/is_enabled()/disable() need NO change.
//   - enable() forwards current_exe_wide()'s buffer bytes to RegSetValueExW — once the
//     buffer is quoted, the written value is quoted automatically.
//   - is_enabled() is PRESENCE-based (RegGetValueW .is_ok() && len>0) — a quoted value
//     is still present + non-empty ⇒ still detected.
//   - disable() deletes the value (RegDeleteValueW) — content-agnostic.
//   Editing them is unnecessary AND risks regressing the (working) presence/forwarding
//   semantics. The fix is a SINGLE-function edit.

// GOTCHA: private fn is visible to the child test mod — no `pub` needed.
//   `mod tests` is a descendant of the autostart module, so `fn current_exe_wide`
//   (private) is reachable via `use super::*;`. Do NOT add pub/pub(crate).

// GOTCHA: rustfmt under default features on Linux may skip the cfg'd-out file.
//   Use the DIRECT check: `rustfmt --edition 2021 --check src/autostart.rs`
//   (or run `cargo fmt` on a Windows host where the file is in the module graph).

// NOTE: line numbers (L99-110) are anchors, not contracts. A later commit could shift
//   them. Match on the TEXT (the `OsStr::new(&path).encode_wide().chain(once(0))…`
//   tail of current_exe_wide) when editing.
```

## Implementation Blueprint

### Data models and structure

No new data models. The only symbol touched is `current_exe_wide()` (signature
unchanged: `fn current_exe_wide() -> Vec<u16>`). The buffer's CONTENT changes from
`[…path…, 0x0000]` to `[0x0022, …path…, 0x0022, 0x0000]`.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: READ the current code + confirm anchors
  - READ: src/autostart.rs in full (~110 lines). Confirm current_exe_wide() is the LAST
          fn with the `OsStr::new(&path).encode_wide().chain(std::iter::once(0)).collect()`
          tail; enable()/is_enabled()/disable() are as described; NO existing
          #[cfg(test)] mod.
  - CONFIRM: line 11 is `#![cfg(target_os = "windows")]` (so on a Linux host the file is
          cfg'd out — the test will run only on Windows).

Task 2: REWRITE current_exe_wide()'s buffer-building tail (edit a)
  - REPLACE the `OsStr::new(&path).encode_wide().chain(std::iter::once(0)).collect()`
          tail with the quoted form (see What-(a)): once(0x0022) → encode_wide →
          once(0x0022) → once(0) → collect.
  - UPDATE the fn's /// doc to say "quoted, null-terminated" + the spaces/security
          rationale (verbatim suggestion in What-(a)).
  - PRESERVE: the `use std::ffi::OsStr; use std::os::windows::ffi::OsStrExt;` imports,
          the `let path = match std::env::current_exe() { … }`, and the
          `Err(_) => return vec![0]` arm — ALL unchanged.

Task 3: ADD the test module at EOF (edit b)
  - APPEND the `#[cfg(test)] mod tests { use super::*; … }` block (see What-(b)) after
          current_exe_wide()'s closing brace. Include current_exe_wide_is_quoted (the
          contract's minimal assertion) + the optional round-trip test.
  - NAMING: snake_case test_<fn>_<scenario>. Use `use super::*;` (no pub on the fn).

Task 4: VALIDATE
  - ON WINDOWS (where the file compiles + the test runs):
      cargo test --bin qmkonnect current_exe_wide -- --nocapture   → expect pass
      cargo test --bin qmkonnect -- --test-threads=1               → all green
      cargo build --release                                         → succeeds
  - ON LINUX / non-Windows (file cfg'd out — test CANNOT run here):
      cargo build                                                   → Finished (regression)
      (logic already proved byte-correct in the /tmp repro — see research/notes.md)
      rustfmt --edition 2021 --check src/autostart.rs               → clean
  - GREP: enable/is_enabled/disable are unchanged:
      grep -nA3 'fn enable\|fn is_enabled\|fn disable' src/autostart.rs  → unchanged bodies
```

### Implementation Patterns & Key Details

```rust
// === WHY quote in current_exe_wide() (not enable()) ===
//   enable() does `let exe = current_exe_wide(); … RegSetValueExW(.., data)` where
//   `data` is exe's raw bytes. It's a pure forwarder — it has no concept of "quoted".
//   Putting the quotes in the buffer builder means enable() writes the quoted value
//   with ZERO downstream changes, and the test can assert on current_exe_wide()
//   directly (which is what the contract specifies).

// === WHY is_enabled() still works (presence, not content) ===
//   is_enabled() calls RegGetValueW and checks `result.is_ok() && len > 0`. A quoted
//   value is still a present, non-empty REG_SZ ⇒ the check still returns true. The
//   tray checkbox continues to reflect the value's existence. (bug_findings.md confirms.)

// === WHY the round-trip test is safe ===
//   current_exe() under `cargo test` resolves to the test binary
//   (target/…/qmkonnet-<hash>.exe) — a clean ASCII/UTF-16 path. String::from_utf16_lossy
//   on the inner bytes == current_exe().to_string_lossy() exactly. (On the astronomically
//   rare non-UTF-8 exe path the lossy compare still agrees because both sides lossify
//   identically.)

// === THE CFG GATE (the #1 one-pass risk on a Linux dev box) ===
//   #![cfg(target_os = "windows")]  (line 11, INNER attr)  ⇒  non-Windows builds EXCLUDE
//   the whole file. On Linux, `cargo test` won't compile autostart.rs, won't run the
//   test, and won't catch a syntax error in the edit. ALWAYS (1) prove the logic with
//   the repro, (2) run the test on a Windows host/CI. The default `cargo build` on
//   Linux is regression-only (the file is simply absent).
```

### Integration Points

```yaml
SOURCE FILES:
  - modify: "src/autostart.rs ONLY"
  - do NOT modify: "src/tray.rs (macOS SMAppService — different autostart mechanism),
                    packaging/windows/inno/QMKonnect.iss (S2's scope), any platform
                    file (x11.rs is T2.S1's), Cargo.toml, docs/* (separate P1.M2.T3 sweep)"

PUBLIC API SURFACE:
  - none changed. current_exe_wide()'s signature is unchanged (fn() -> Vec<u16>); it's
    private. set_enabled/is_enabled are pub and unchanged. Only the buffer CONTENT
    changes (now quoted).

UPSTREAM/DOWNSTREAM:
  - UPSTREAM: std::env::current_exe() → the running exe PathBuf.
  - DOWNSTREAM: enable() writes the buffer as REG_SZ to HKCU\…\Run\QMKonnect. After the
    fix the value is `"<path>"`; Windows' login resolver handles the quoted form
    correctly for spaced paths. is_enabled()/disable() unaffected.

DEPENDENCIES / Cargo.toml:
  - none. Pure std (iter::once, encode_wide). No new deps.

VALIDATION CONSUMERS:
  - `cargo test --bin qmkonnect current_exe_wide` (Windows) is THE gate. On Linux the
    file is cfg'd out; the /tmp repro + default-build regression stand in.
```

## Validation Loop

> All commands run from the repo root: `/home/dustin/projects/qmkonnect`.
> **The test runs ONLY on Windows** (autostart.rs line 11 `#![cfg(target_os="windows")]`
> gates the whole file out of non-Windows builds). On the Linux dev box the file is
> absent; validate logic via the repro + the default-build regression, and execute the
> test on a Windows host/CI.

### Level 1: Compile + run the new test (THE gate — Windows host)

```bash
cd /home/dustin/projects/qmkonnect
# ON WINDOWS:
cargo test --bin qmkonnect current_exe_wide -- --nocapture
# Expected (Windows): current_exe_wide_is_quoted (+ current_exe_wide_quotes_the_actual_exe_path) PASS.
# ON LINUX: this prints "0 passed" / the test doesn't appear — the file is cfg'd out.
#   That is EXPECTED here, not a failure. The logic is proved by the /tmp repro.

# Full suite (Windows) — single-threaded for the shared debouncer (AGENTS.md):
cargo test --bin qmkonnect -- --test-threads=1
# Expected (Windows): all green, incl. the new tests.
```

### Level 2: Regression — default build still links (any host)

```bash
cd /home/dustin/projects/qmkonnect
cargo build 2>&1 | tail -2
# Expected: "Finished `dev` profile …" (no warnings, no errors). On non-Windows this
#   confirms nothing OUTSIDE the cfg'd-out autostart.rs broke. On Windows it compiles
#   autostart.rs too (catching a syntax error in the edit).
```

### Level 3: Lint + format

```bash
cd /home/dustin/projects/qmkonnect
# Clippy (Windows compiles autostart.rs; on Linux it's cfg'd out but the rest lints):
cargo clippy --bin qmkonnect -- -D warnings
# Expected: zero warnings. (Watch for: the iter chain is idiomatic — no needless_collect.)

# Direct rustfmt check (default-features `cargo fmt` may skip the cfg'd-out file on Linux):
rustfmt --edition 2021 --check src/autostart.rs
# Expected: no diff (exit 0). If it diffs, run `rustfmt --edition 2021 src/autostart.rs`.
```

### Level 4: Targeted correctness spot-checks (the regression intent)

```bash
cd /home/dustin/projects/qmkonnect

# Confirm the quoting chain is present in current_exe_wide.
grep -nA6 'fn current_exe_wide' src/autostart.rs
# Expected: the body chains once(0x0022) → encode_wide → once(0x0022) → once(0).

# Confirm the error arm is UNCHANGED (still vec![0]).
grep -n 'return vec!\[0\]' src/autostart.rs   # Expected: 1 hit (the Err arm).

# Confirm enable/is_enabled/disable are byte-identical (only current_exe_wide changed).
grep -nA3 'fn enable\|fn is_enabled\|fn disable' src/autostart.rs
# Expected: unchanged bodies (forwarding / presence / delete).

# Confirm exactly one #[cfg(test)] mod (the new one — autostart.rs had none before).
grep -c '#\[cfg(test)\]' src/autostart.rs   # Expected: 1.

# Confirm only autostart.rs changed.
git status --short
# Expected: only src/autostart.rs listed.
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1 (Windows): `cargo test --bin qmkonnect current_exe_wide -- --nocapture` → pass.
- [ ] Level 1 (Windows): `cargo test --bin qmkonnect -- --test-threads=1` → all green.
- [ ] Level 2: `cargo build` → Finished (regression; on Linux autostart.rs is cfg'd out).
- [ ] Level 3: `cargo clippy --bin qmkonnect -- -D warnings` → clean.
- [ ] Level 3: `rustfmt --edition 2021 --check src/autostart.rs` → clean.

### Feature Validation

- [ ] `current_exe_wide()` returns `[0x0022, …path…, 0x0022, 0x0000]` (proved by test +
      the /tmp repro).
- [ ] `buf[0] == 0x0022` and `buf[buf.len()-2] == 0x0022` and `buf.last() == 0`.
- [ ] The quoted content equals `std::env::current_exe()` (round-trip test).
- [ ] The error arm still returns `vec![0]` (unchanged).

### Code Quality Validation

- [ ] `current_exe_wide()`'s buffer-building tail is the 4-stage quote→path→quote→NUL chain.
- [ ] The `Err` arm, the `use` imports, and the `let path = match …` are unchanged.
- [ ] `current_exe_wide()`'s `///` doc explains the quoting + spaces/security rationale.
- [ ] The test mod uses `use super::*;` (no `pub` on `current_exe_wide`); snake_case names.
- [ ] `enable()`/`is_enabled()`/`disable()` + the `RegSetValueExW` call are byte-identical.
- [ ] Only `src/autostart.rs` modified; `tray.rs`, `QMKonnect.iss` (S2), platform files untouched.

### Documentation & Deployment

- [ ] DOCS = none per contract (internal quoting fix; `docs/troubleshooting.md` L265 shows
      a `reg query` to CHECK the entry, not the value format — no change). A separate
      P1.M2.T3 sweep verifies docs.
- [ ] No Cargo.toml, config, or environment-variable change.

---

## Anti-Patterns to Avoid

- ❌ Don't expect the test to run on a Linux dev box — autostart.rs line 11 is
  `#![cfg(target_os = "windows")]` (an INNER attribute gating the WHOLE file), so on
  non-Windows the file is absent from the build: `cargo test` won't compile it, won't
  run the test, and won't catch a syntax error in the edit. Validate logic via the /tmp
  repro (proved byte-correct) and EXECUTE the test on a Windows host/CI. The default
  `cargo build` on Linux is regression-only. (This is the platform-inverse of the
  sibling x11.rs PRP's `--no-default-features` requirement.)
- ❌ Don't drop the NUL terminator — `REG_SZ` REQUIRES it, and `enable()`'s comment
  explicitly relies on `current_exe_wide` appending one. The chain MUST end with
  `once(0)`: `[0x0022, …path…, 0x0022, 0x0000]`. Putting the NUL before the trailing
  quote truncates the value.
- ❌ Don't "quote" the error path (`Err(_) => return vec![0]`). current_exe() failing is
  rare and a NUL-only write is benign; changing it to `[0x0022, 0x0022, 0]` would write
  a value containing only `""`. Leave the error arm as-is.
- ❌ Don't edit `enable()` / `is_enabled()` / `disable()` — quoting is transparent to
  them: `enable()` forwards the buffer's bytes to `RegSetValueExW` (quoted buffer ⇒
  quoted value, no code change); `is_enabled()` is presence-based (`is_ok() && len>0`,
  still true for a quoted value); `disable()` deletes (content-agnostic). Editing them
  is unnecessary and risks regressing working semantics.
- ❌ Don't put the quoting in `enable()` instead of `current_exe_wide()` — the contract's
  test asserts on `current_exe_wide()` directly (it must START with 0x0022 and have
  0x0022 before the NUL). Putting quotes in `enable()` would make that test impossible.
- ❌ Don't add `pub`/`pub(crate)` to `current_exe_wide()` — it's an internal helper; the
  child `mod tests` reaches it via `use super::*;`. Leaking it pollutes the module API.
- ❌ Don't edit `packaging/windows/inno/QMKonnect.iss` — that's P1.M1.T3.S2 (the
  installer-side writer of the same Run-key value). Different file, independent task.
- ❌ Don't edit `src/tray.rs` (macOS SMAppService autostart — a different mechanism) or
  any platform file (`x11.rs` is the parallel T2.S1's scope).
- ❌ Don't rely on `cargo fmt --all` alone to format-check autostart.rs on Linux — under
  default features the cfg'd-out file may be skipped. Use the direct
  `rustfmt --edition 2021 --check src/autostart.rs`.
- ❌ Don't treat line numbers (L99-110) as contracts — match on the TEXT (the
  `OsStr::new(&path).encode_wide().chain(std::iter::once(0)).collect()` tail of
  `current_exe_wide`) when editing. A later commit could shift them.
- ❌ Don't treat this single-function fix as closing the whole bug — the INSTALLER side
  (`QMKonnect.iss`, S2) writes the SAME Run-key value unquoted and must be fixed too.
  This task is the app-side half; both land independently.

---

**Confidence Score: 9/10** for one-pass implementation success. The deliverable is a
single-file, two-edit bug fix: a verbatim 4-stage iterator rewrite of
`current_exe_wide()`'s buffer tail (quotes → path → quotes → NUL, proved byte-correct
in a standalone rustc repro: `buf[0]=0x0022`, `buf[-2]=0x0022`, `buf[-1]=0`) plus a
verbatim Windows-only test module asserting exactly the contract's invariant. The fix
is transparent to `enable()` (forwarder), `is_enabled()` (presence-based), and
`disable()` (delete) — all confirmed unchanged. The one residual risk — the test
running ONLY on Windows (the file's `#![cfg(target_os="windows")]` inner gate excludes
it from the Linux dev-box build) — is pre-empted by the /tmp logic repro + the default
`cargo build` regression + explicit Windows-host execution instructions, mirroring the
sibling x11.rs PRP's proven cfg-gate handling (platform-inverse). No API change, no new
deps, no doc change (per contract); the sibling S2 (installer) and parallel T2.S1 (x11)
edit different files, so there's no collision. (Score 9 not 10 only because actual test
execution requires a Windows host the Linux dev box cannot provide — the logic itself is
verified.)