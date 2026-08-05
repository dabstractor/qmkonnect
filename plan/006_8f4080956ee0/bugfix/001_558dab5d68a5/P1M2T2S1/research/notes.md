# Research Notes — P1.M2.T2.S1: Fix should_ignore_window char-count + add unit tests

## Task in one line

One-line fix in `src/platforms/windows.rs:306`: change `window_info.title.len() < 2`
(byte count) → `window_info.title.chars().count() < 2` (char count). Then add the
file's FIRST `#[cfg(test)] mod tests` block with 6 cases for `should_ignore_window`.

## Repo under change

- **QMKonnect** Rust daemon, `/home/dustin/projects/qmkonnect`. Bug-fix release plan
  `plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/`.
- ONE file edited: `src/platforms/windows.rs` (one line changed + a test module
  appended at end-of-file).
- NO relation to the parallel P1.M2.T1.S3 (which edits `src/linux_tray.rs` — a
  different file). Zero overlap.

## The bug (Bug 5 / PRD ID 5) — root cause confirmed

`should_ignore_window()` (windows.rs:250-310) gates "very short titles" at L306:
```rust
// Ignore very short titles that are likely not real applications
if window_info.title.len() < 2 && !window_info.title.is_empty() {
    return true;
}
```
`String::len()` is BYTE length. A 1-char emoji like "😀" is 4 UTF-8 bytes → `4 >= 2`
→ NOT ignored (kept). A 1-char ASCII "x" is 1 byte → `1 < 2` → ignored. Inconsistent
heuristic. The fix is `.chars().count()` (Unicode scalar count): "😀" → 1 char < 2 →
ignored, matching the ASCII behavior. (bug_findings.md "Bug 5" confirms this verbatim.)

## The fix — exact anchor (grep/read-confirmed, unique in the file)

Line 306, current text:
```rust
    if window_info.title.len() < 2 && !window_info.title.is_empty() {
```
→
```rust
    if window_info.title.chars().count() < 2 && !window_info.title.is_empty() {
```
The ONLY change is `.len()` → `.chars().count()`. The comment on L305, the
`&& !window_info.title.is_empty()` guard, and the `return true;` are UNCHANGED.

## should_ignore_window full logic (verified — needed to get the 6 test expected values right)

Order of checks (windows.rs:250-310):
1. **Internal-class blocklist** (`ignore_classes[]` — ForegroundStaging,
   XamlExplorerHostIslandWindow, …, Shell_TrayWnd, Shell_SecondaryTrayWnd). If
   `app_class` matches any → return **true** (ignored). [L267-287]
2. **Empty-title rule**: `if title.is_empty()` → unless `app_class` is in
   `allow_empty_title[]` (CASCADIA_HOSTING_WINDOW_CLASS, Chrome_WidgetWin_1), return
   **true** (ignored). [L290-304]
3. **Short-title rule** (L306, the bug line): `if title.len() < 2 && !empty` → return
   **true** (ignored).
4. Else → return **false** (kept).

Note: `ApplicationFrameWindow` / `CoreWindow` are DELIBERATELY not in the blocklist
(`get_window_info` already resolves the UWP frame to its content window — re-filtering
would hide every UWP app). Do not "fix" this.

## WindowInfo type (the test input)

`src/core/types.rs` (NOT cfg-gated — available everywhere):
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct WindowInfo {
    pub app_class: String,
    pub title: String,
}
impl WindowInfo {
    pub fn new(app_class: String, title: String) -> Self { Self { app_class, title } }
}
```
Construct in tests via `WindowInfo::new("Class".to_string(), "title".to_string())`.

## The 6 tests — expected values verified against the logic above

| # | Title | Class | Expected | Why |
|---|---|---|---|---|
| a | `""` (empty) | `"SomeApp"` (not allowed) | **true** (ignored) | step 2: empty + class not in allow_empty_title |
| b | `""` (empty) | `"Chrome_WidgetWin_1"` (allowed) | **false** (kept) | step 2: allowed empty-title class → falls through; step 3: `&& !empty` is false |
| c | `"x"` (1 ASCII char, 1 byte) | `"SomeApp"` | **true** (ignored) | step 3: chars().count()=1 < 2 && not empty |
| d | `"😀"` (1 emoji char, 4 bytes) | `"SomeApp"` | **true** (ignored) | step 3: AFTER FIX chars().count()=1 < 2. **(BEFORE FIX: len()=4 ≥ 2 → false/kept — the regression case)** |
| e | `"ab"` (2 chars) | `"SomeApp"` | **false** (kept) | step 3: chars().count()=2, not < 2 → falls through → step 4 false |
| f | any (e.g. `"Taskbar"`) | `"Shell_TrayWnd"` (internal) | **true** (ignored) | step 1: blocklist match (returns before title is examined) |

**Critical for test (d):** this is THE test that proves the fix. Before the change it
would assert `false` (kept); after the change it must assert `true` (ignored). If the
implementer forgets the `.len()→.chars().count()` line edit, test (d) FAILS — exactly
the regression guard we want.

For cases (c)/(d)/(e) use a NEUTRAL app class (e.g. `"SomeApp"`) that is NOT in the
internal blocklist AND not in allow_empty_title — so the title-length check is the only
thing under test. (The title is non-empty in these cases, so the allow_empty_title list
doesn't apply anyway, but a neutral class keeps the intent unambiguous.)

## The cfg constraint — THE dominant risk for one-pass validation

- `src/platforms/windows.rs` line 1: `#![cfg(target_os = "windows")]` — the ENTIRE file
  (including any `#[cfg(test)] mod tests`) is compiled ONLY on a Windows target.
- Host triple here is `x86_64-unknown-linux-gnu`. So `cargo build`/`cargo test` on this
  Linux box does NOT compile windows.rs at all — it will NOT catch any error in the fix
  or the new tests. (The new tests simply don't exist for the Linux build.)
- Cross-`check` was attempted this session:
  `cargo check --target x86_64-pc-windows-gnu` → **FAILS**: a build-script (cc-rs)
  needs `x86_64-w64-mingw32-gcc`, which is not installed. The `x86_64-pc-windows-msvc`
  target likewise has no linker/build-tools on Linux. So windows.rs CANNOT be
  type-checked on this box without extra toolchain setup (out of scope).
- `rustup target list --installed` shows x86_64-pc-windows-{gnu,msvc} ARE installed, but
  the C-toolchain for the gnu build scripts is absent.

➡️ **Therefore the canonical validation gate is a WINDOWS box** (AGENTS.md Windows dev
loop: MSVC toolchain, `cargo test --bin qmkonnect -- --test-threads=1`). The new tests
compile + run there.

➡️ **For one-pass confidence WITHOUT a Windows box**, de-risk via a throwaway STANDALONE
Linux harness: copy the `should_ignore_window` BODY (it is PURE logic — no Windows API
calls, only `String ==`, `.is_empty()`, `.chars().count()`) + the 6 tests into a single
`/tmp` file and run it on Linux. This proves the test assertions + the fix logic are
correct before the Windows run. (See PRP Validation Level 2b.) This is the key value-add
of this PRP — it makes the Windows-only task verifiable in part on the dev box.

## Scope boundaries (do NOT do)

- Do NOT touch the `ignore_classes` blocklist or the `allow_empty_title` list.
- Do NOT remove the short-title heuristic (the PRD "Recommendations" floats "or remove it
  if it's not spec-required" — but the CONTRACT is to FIX it to char-count, not remove).
- Do NOT edit `src/core/types.rs` (WindowInfo is already correct).
- Do NOT edit `src/linux_tray.rs` (parallel P1.M2.T1.S3) or any other file.
- Do NOT change anything beyond the one `.len()→.chars().count()` line + the appended
  test module.

## Test-module placement + imports

- Append `#[cfg(test)] mod tests { … }` at END OF FILE (after `create_config_dir`'s
  closing brace, currently L529). Convention from `src/core/types.rs` (its `mod tests`
  is the last item) and `src/autostart.rs` (another `#![cfg(target_os="windows")]` file
  with a `#[cfg(test)] mod tests` at L120).
- Inside the test module:
  - `use super::*;` — brings the private `should_ignore_window` into scope (child module
    can see parent's private fns).
  - ALSO add `use crate::core::types::WindowInfo;` explicitly — the parent's
    `use crate::core::types::WindowInfo;` (windows.rs:3) is a PRIVATE import; do NOT rely
    on glob-import-of-private-use semantics. Explicit is robust and unambiguous.

## Conclusion

A one-line, pure-logic fix + 6 pure-logic unit tests. The dominant risk is that the file
is Windows-only (cannot be type-checked/run on the Linux dev box); mitigated by (1) the
canonical Windows-box gate and (2) a throwaway standalone Linux harness that proves the
test assertions + fix logic. All 6 expected values verified against the actual function
logic this session.