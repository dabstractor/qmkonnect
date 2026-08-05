# Research Notes — P1.M1.T3.S1: Quote the exe path in `src/autostart.rs current_exe_wide()`

## The bug (Bug 3A / PRD ID 4, Major)

`src/autostart.rs::current_exe_wide()` builds a **null-terminated UTF-16 buffer of
the raw exe path** (no quotes). `enable()` writes that buffer's raw bytes directly
as `REG_SZ` data to the HKCU `Run` key via `RegSetValueExW`. If the install path
contains a space (`C:\Users\John Doe\…\QMKonnect.exe`), Windows' `Run`-key resolver
may mis-parse the unquoted "service path" (treating `C:\Users\John` as the exe and
the rest as args) → autostart silently fails at login — and it is also the classic
**unquoted-service-path** security vector. Fix: wrap the path in double-quotes so
the `REG_SZ` value is `"C:\Users\John Doe\…\QMKonnect.exe"`.

## The fix — one function, `current_exe_wide()` (src/autostart.rs:99-110)

### Current code (exact)
```rust
fn current_exe_wide() -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    let path = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return vec![0],
    };
    OsStr::new(&path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
```

### Fixed code
```rust
fn current_exe_wide() -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    let path = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return vec![0],
    };
    // Wrap the path in double-quotes (U+0022) BEFORE the NUL terminator, so a path
    // containing spaces is written to the HKCU `Run` key as a quoted REG_SZ that
    // Windows resolves correctly at login (and is not an unquoted-service-path
    // vector). Buffer layout: [0x0022, …path UTF-16…, 0x0022, 0x0000].
    std::iter::once(0x0022)
        .chain(OsStr::new(&path).encode_wide())
        .chain(std::iter::once(0x0022))
        .chain(std::iter::once(0))
        .collect()
}
```
Net change: the iterator chains a **leading `"` (0x0022)** before the path's
`encode_wide()`, then the **trailing `"` (0x0022)**, then the existing **NUL (0)**.
The error path (`Err(_) => return vec![0]`) is UNCHANGED (can't quote a path we
failed to resolve; a NUL-only write is benign and rare).

## Verified byte-correct (standalone repro, /tmp/quote_test)

The quoting core is platform-independent, so I proved it on the Linux dev box:
```
fn current_exe_wide_quoted(path) = once(0x0022).chain(encode_wide).chain(once(0x0022)).chain(once(0))
```
For `C:\Users\John Doe\…\QMKonnect.exe`:
- `buf[0] == 0x0022` ✓ (starts with quote)
- `buf[buf.len()-2] == 0x0022` ✓ (char before final NUL is a quote)
- `buf.last() == 0` ✓ (NUL terminator)
- decoded (minus NUL): `"C:\Users\John Doe\AppData\Local\Programs\QMKonnect\QMKonnect.exe"` ✓
- min-length `"x"` ⇒ `[0x0022, 'x', 0x0022, 0]` ✓

All assertions PASS. The fix logic is correct.

## ⚠ The cfg constraint — the test is WINDOWS-ONLY (mirrors the x11.rs sibling, inverted)

`src/autostart.rs` line 11 is `#![cfg(target_os = "windows")]` (an **inner** attribute
gating the WHOLE file). So:
- **On Windows:** the file (and any `#[cfg(test)] mod tests` inside it) compiles + runs.
  Gate: `cargo test --bin qmkonnect current_exe_wide` (the AGENTS.md Windows loop).
- **On Linux (THIS dev box — `uname -s` = Linux):** the file is cfg'd OUT entirely.
  `cargo build`/`cargo test` (default) neither compile nor run autostart.rs — a syntax
  error in the fix WON'T surface, and the new test WON'T run. (Verified: `cargo build`
  here → Finished, exit 0, with autostart.rs absent.)

**Implication for one-pass success:** on a Linux host the implementer CANNOT execute
the test. The PRP therefore (a) validates the LOGIC via the standalone repro (proved
byte-correct above), (b) confirms the default Linux build still links (regression —
autostart.rs is absent, nothing else changes), and (c) defers actual test EXECUTION to
a Windows host / Windows CI. This is the EXACT pattern the sibling x11.rs PRP used
(inverted: x11.rs needs `--no-default-features` on Linux; autostart.rs needs Windows).

## Why `is_enabled()` / `disable()` / `enable()` need NO change

- `is_enabled()` (L36) checks value **PRESENCE** via `RegGetValueW` (`result.is_ok()
  && len > 0`), NOT content. A quoted value is still present + non-empty → still
  detected. ✓ (bug_findings.md confirms: "The existing is_enabled() presence check
  will still match (the value exists, just quoted).")
- `disable()` (L86) deletes the value (`RegDeleteValueW`) — content-agnostic. ✓
- `enable()` (L73) writes `current_exe_wide()`'s buffer raw bytes via
  `RegSetValueExW` — it forwards whatever `current_exe_wide()` returns, so once that
  buffer is quoted, enable() writes the quoted value with NO code change. ✓ (The
  comment "`current_exe_wide` already appends one [NUL]" stays accurate — it still
  appends a NUL, now after the trailing quote.)

So the fix is a SINGLE-function edit: `current_exe_wide()`.

## Scope boundaries (vs siblings)
- **This task (S1):** `src/autostart.rs` ONLY (the app-side Run-key writer).
- **P1.M1.T3.S2 (sibling, Planned):** `packaging/windows/inno/QMKonnect.iss`
  `[Registry]` section — the INSTALLER-side writer (`ValueData: "{app}\{#MyAppExeName}"`
  → `ValueData: """{app}\{#MyAppExeName}"""`). DIFFERENT file → no collision. Both
  must land for the bug to be fully fixed (app + installer agree), but each is
  independent.
- **P1.M1.T2.S1 (parallel, x11.rs):** different file, different bug → no overlap.

## DOCS
Per the contract: **none** — internal quoting fix. `docs/troubleshooting.md` L265
shows a `reg query` command to *check* the entry, not the value format; no change.
(A separate P1.M2.T3 sweep verifies docs remain accurate.)