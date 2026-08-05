# PRP — P1.M2.T2.S1: Change `.len()` → `.chars().count()` + add unit tests for `should_ignore_window`

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`) — Rust tray/menu-bar daemon.
> **One file edited:** `src/platforms/windows.rs` — **one line changed** (L306) + a
> `#[cfg(test)] mod tests` block appended at end-of-file (the file currently has NONE).
> **Scope:** Bug 5 / PRD ID 5 ONLY. Do NOT touch the blocklist, the allow-empty-title
> list, `types.rs`, or any other file.
> **Parallel context:** P1.M2.T1.S3 (parallel) edits `src/linux_tray.rs` — a **different
> file**, zero overlap. No dependency on it.

---

## ⚠️ READ FIRST — the dominant constraint: this file is Windows-only

`src/platforms/windows.rs` line 1 is `#![cfg(target_os = "windows")]`. The ENTIRE file —
including the new `#[cfg(test)] mod tests` — compiles **only on a Windows target**. On
the Linux dev box (`cargo build`/`cargo test`) this file is **not compiled at all**, so
those commands will NOT catch any error in the fix or the new tests (the tests simply
don't exist for the Linux build). Cross-`check` against `x86_64-pc-windows-gnu` FAILS
here (no `x86_64-w64-mingw32-gcc` for a build script) and `x86_64-pc-windows-msvc` has no
linker/build-tools on Linux.

➡️ **The canonical validation gate is a WINDOWS box** (AGENTS.md Windows dev loop:
MSVC toolchain → `cargo test --bin qmkonnect -- --test-threads=1`).
➡️ **For one-pass confidence WITHOUT a Windows box**, `should_ignore_window` is PURE
logic (no Windows API calls — only `String ==`, `.is_empty()`, `.chars().count()`), so a
**throwaway standalone Linux harness** (copy the function body + the 6 tests into a
`/tmp` file) proves the test assertions + fix logic are correct before the Windows run.
Both tracks are specified in the Validation Loop below.

---

## Goal

**Feature Goal**: Make `should_ignore_window()` (src/platforms/windows.rs) ignore window
titles shorter than 2 **Unicode characters** regardless of UTF-8 byte length, so a
1-char emoji title (e.g. "😀", 4 bytes) is ignored just like a 1-char ASCII title
("x", 1 byte) — closing the inconsistent byte-vs-char heuristic (Bug 5 / PRD ID 5).

**Deliverable**: `src/platforms/windows.rs` with (1) the one-line fix at L306
(`.len()` → `.chars().count()`), and (2) a new `#[cfg(test)] mod tests` block (appended
at end-of-file) with **6 unit tests** for `should_ignore_window` covering: empty title +
non-allowed class, empty title + allowed class, 1-char ASCII, 1-char emoji (the fix's
regression case), 2-char title, and an internal class name.

**Success Definition**:
- L306 reads `window_info.title.chars().count() < 2 && !window_info.title.is_empty()`.
- The 6 tests are appended as `#[cfg(test)] mod tests` at end-of-file, all asserting the
  verified-expected values (table below).
- On a **Windows box**: `cargo test --bin qmkonnect should_ignore -- --test-threads=1`
  shows the 6 new tests **PASS**; `cargo test --bin qmkonnect -- --test-threads=1` is
  fully green (no regression).
- (Optional, no-Windows-box path) the standalone Linux harness (Validation Level 2b)
  confirms the 6 assertions + fix logic.
- `git diff --stat` shows ONLY `src/platforms/windows.rs`.

## User Persona (if applicable)

**Target User**: a Windows user whose foreground app has a very short title — most
notably a single emoji or a single non-ASCII character. Today such a window is
*processed* (sent to the keyboard) while a single ASCII character is *ignored*, an
inconsistent heuristic. After the fix, the heuristic is consistent: any title under 2
characters is treated as "likely not a real application" and ignored, regardless of
encoding.

**Use Case**: A window with title "😀" (4 UTF-8 bytes) gains focus. Before the fix,
`should_ignore_window` returns `false` (4 ≥ 2 bytes → kept) so the (class, "😀") pair is
sent to the keyboard and matched against `rules.toml`. After the fix it returns `true`
(1 char < 2 → ignored), matching how a 1-char ASCII "x" is treated.

**Pain Points Addressed**: removes a byte-vs-char inconsistency that could let a junk
1-char-emoji title through to rule matching (and a future rules author copying it as a
pattern). Pure internal-heuristic polish; no user-facing config/API change.

## Why

- **Closes Bug 5 / PRD ID 5.** `String::len()` is byte length; the heuristic's intent is
  "short titles", which is a *character* notion. The one-line fix aligns the check with
  its intent. The PRD "Recommendations" explicitly call this out: *"Investigate and fix
  the Windows title-length heuristic to use character count"*.
- **First unit tests for `should_ignore_window`.** `windows.rs` currently has NO test
  module. `should_ignore_window(&WindowInfo)` is a **pure function** (no Windows API
  calls), so it is trivially unit-testable. The 6 tests lock in the fix AND guard the
  surrounding blocklist/empty-title behavior against future regressions.
- **Minimal + scoped.** One line changed, no new imports in production code (`String`
  is already in scope), one test module appended. No change to `types.rs`, the blocklist,
  or the allow-empty-title list.

## What

### Edit 1 — the fix (src/platforms/windows.rs, line 306)
```rust
// BEFORE (current, lines 305-306):
    // Ignore very short titles that are likely not real applications
    if window_info.title.len() < 2 && !window_info.title.is_empty() {

// AFTER:
    // Ignore very short titles that are likely not real applications
    // (chars().count() = Unicode scalar count, so a 1-char emoji (4 bytes) is
    // ignored just like a 1-char ASCII title — Bug 5 / PRD ID 5).
    if window_info.title.chars().count() < 2 && !window_info.title.is_empty() {
```
**The ONLY code change is `.len()` → `.chars().count()`.** (Updating the comment to cite
Bug 5 is recommended but optional.) The `&& !window_info.title.is_empty()` guard and
`return true;` are UNCHANGED.

### Edit 2 — append the test module (end-of-file, after `create_config_dir`'s closing brace at L529)
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::WindowInfo;

    // Helper: build a WindowInfo from &str class + &str title.
    fn wi(class: &str, title: &str) -> WindowInfo {
        WindowInfo::new(class.to_string(), title.to_string())
    }

    #[test]
    fn test_ignore_empty_title_non_allowed_class() {
        // (a) empty title + non-allowed class => ignored (empty-title rule, step 2)
        assert!(should_ignore_window(&wi("SomeApp", "")));
    }

    #[test]
    fn test_keep_empty_title_allowed_class() {
        // (b) empty title + allowed class (Chrome_WidgetWin_1) => kept
        assert!(!should_ignore_window(&wi("Chrome_WidgetWin_1", "")));
    }

    #[test]
    fn test_ignore_one_char_ascii_title() {
        // (c) 1-char ASCII ("x", 1 byte) => ignored (short-title rule, 1 char < 2)
        assert!(should_ignore_window(&wi("SomeApp", "x")));
    }

    #[test]
    fn test_ignore_one_char_emoji_title() {
        // (d) 1-char emoji ("😀", 4 bytes) => ignored AFTER the fix (1 char < 2).
        //     BEFORE the fix this was KEPT (len()=4 >= 2). THIS IS THE REGRESSION CASE.
        assert!(should_ignore_window(&wi("SomeApp", "😀")));
    }

    #[test]
    fn test_keep_two_char_title() {
        // (e) 2-char title ("ab") => kept (chars().count()=2, not < 2)
        assert!(!should_ignore_window(&wi("SomeApp", "ab")));
    }

    #[test]
    fn test_ignore_internal_class_name() {
        // (f) internal class (Shell_TrayWnd) => ignored via blocklist (step 1),
        //     regardless of title.
        assert!(should_ignore_window(&wi("Shell_TrayWnd", "Taskbar")));
    }
}
```

### Success Criteria
- [ ] L306 uses `.chars().count()` (was `.len()`).
- [ ] A `#[cfg(test)] mod tests` block exists at end-of-file with the 6 tests above.
- [ ] The test module has `use super::*;` AND `use crate::core::types::WindowInfo;`
      (explicit — do not rely on glob-import-of-private-use).
- [ ] All 6 tests assert the verified-expected values (table below).
- [ ] On Windows: `cargo test --bin qmkonnect should_ignore -- --test-threads=1` → 6 passed.
- [ ] No change to the blocklist, the allow-empty-title list, `types.rs`, or any other file.
- [ ] `git diff --stat` shows ONLY `src/platforms/windows.rs`.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge can make the one-line fix + append the test
module using only the exact code above, the verified expected-value table, the
end-of-file placement, and the two-track validation (Windows-box canonical + Linux
standalone harness) — all present in this PRP.

### Documentation & References

```yaml
# MUST READ — the bug being fixed
- docfile: plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/architecture/bug_findings.md
  why: "Bug 5 root cause + the EXACT recommended one-line fix (.len() → .chars().count());
        confirms should_ignore_window is a pure function, easily unit-tested; no test module
        currently exists in windows.rs"
  section: "## Bug 5 (Minor, PRD ID 5): Windows title-length heuristic uses byte length"
  critical: "the fix is to CHARACTER COUNT, NOT to remove the heuristic (PRD 'Recommendations'
             floats 'or remove it' — the CONTRACT is to FIX it)."
- url: spec/PRD.md (heading h2.3/h3.4 "Issue 2: Windows should_ignore_window title-length heuristic uses byte length")
  why: user-facing repro (1-char ASCII ignored, 1-char emoji kept)

# MUST READ — the file & function being edited
- file: src/platforms/windows.rs
  why: "the bug line + the full should_ignore_window logic (needed for correct test expected
        values) + the cfg gate (line 1) + the end-of-file insertion point"
  pattern: "L1 #![cfg(target_os = \"windows\")]; L3 use crate::core::types::WindowInfo;
            L250 fn should_ignore_window(&WindowInfo) -> bool; L267-287 ignore_classes blocklist;
            L290-304 empty-title rule + allow_empty_title[]; L306 the bug line; L529 end of file
            (after create_config_dir)"
  gotcha: "the ENTIRE file is Windows-only (#![cfg(target_os=\"windows\")]) — Linux cargo
           build/test does NOT compile it, so it will NOT catch errors in the fix or tests."

# MUST READ — the WindowInfo type (test input)
- file: src/core/types.rs
  why: "WindowInfo { app_class: String, title: String } derives Debug/Clone/PartialEq;
        WindowInfo::new(app_class: String, title: String) -> Self is the constructor."
  pattern: "#[cfg(test)] mod tests with `use super::*` (the file's own test convention to mirror)"
  gotcha: "WindowInfo::new takes OWNED String — in tests use `.to_string()` on &str literals."

# REFERENCE — another cfg(windows) file with a test module (placement/structure convention)
- file: src/autostart.rs
  why: "L11 #![cfg(target_os = \"windows\")]; L120 #[cfg(test)] mod tests — proves the
        end-of-file `#[cfg(test)] mod tests` pattern works inside a Windows-only file, and
        shows the convention to follow."
  pattern: "test module is the LAST item in the file; `use super::*;` at top."
  gotcha: "autostart.rs's tested fns are in the same file (no cross-module import needed);
           here the test ALSO needs `use crate::core::types::WindowInfo;` (explicit)."

# REFERENCE — the parallel sibling (different file, no overlap — for context only)
- docfile: plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/P1M2T1S3/PRP.md
  why: "edits src/linux_tray.rs (handshake reset); confirms it does NOT touch windows.rs"
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
src/platforms/windows.rs   # EDIT: L306 (.len() → .chars().count()) + append #[cfg(test)] mod tests after L529
  - L1     #![cfg(target_os = "windows")]              (ENTIRE file Windows-only)
  - L3     use crate::core::types::WindowInfo;         (private import — test needs its OWN explicit use)
  - L250   fn should_ignore_window(window_info: &WindowInfo) -> bool
  - L267-287 ignore_classes blocklist (ForegroundStaging, …, Shell_TrayWnd, Shell_SecondaryTrayWnd)
  - L290-304 empty-title rule + allow_empty_title[] (CASCADIA_HOSTING_WINDOW_CLASS, Chrome_WidgetWin_1)
  - L306   if window_info.title.len() < 2 …            ← THE BUG LINE (EDIT)
  - L529   }  (end of create_config_dir = end of file) ← TEST MODULE APPENDED AFTER THIS
src/core/types.rs          # READ ONLY: WindowInfo struct + WindowInfo::new (already correct)
src/autostart.rs           # READ ONLY: reference for cfg(windows) + end-of-file mod tests convention
```

### Desired Codebase tree
**Only `src/platforms/windows.rs` changes** — one line edited (L306) + a test module
appended at end-of-file. No new files, no `types.rs` change, no Cargo.toml change.

### Known Gotchas of our codebase & Library Quirks
```rust
// CRITICAL (platform gate): windows.rs is #![cfg(target_os = "windows")] — the WHOLE file,
//   including the new #[cfg(test)] mod tests, compiles ONLY on a Windows target. On the Linux
//   dev box `cargo build`/`cargo test` do NOT compile this file at all (the tests don't exist
//   for the Linux build). So Linux cargo will NOT catch errors in the fix or tests. Validate
//   on a Windows box (canonical) and/or via the standalone Linux harness (Validation Level 2b).

// CRITICAL (the fix is char-count, NOT removal): the PRD "Recommendations" says "use character
//   count OR remove it". The CONTRACT is to FIX it to char-count (.chars().count()). Do NOT
//   delete the short-title heuristic.

// CRITICAL (test expected values — verified against the function logic this session):
//   (d) the emoji test asserts TRUE (ignored). This is the regression case: BEFORE the fix
//   "😀".len()=4 >= 2 => KEPT (false); AFTER the fix "😀".chars().count()=1 < 2 => IGNORED
//   (true). If you forget the .len()→.chars().count() edit, test (d) FAILS — exactly the guard
//   we want. (a) empty+non-allowed=true; (b) empty+allowed=false; (c) "x"=true; (e) "ab"=false;
//   (f) Shell_TrayWnd=true (blocklist, step 1, before title is examined).

// GOTCHA (test imports): the test module needs BOTH `use super::*;` (brings the private
//   should_ignore_window into scope — child sees parent privates) AND an explicit
//   `use crate::core::types::WindowInfo;`. The parent's L3 `use crate::core::types::WindowInfo;`
//   is a PRIVATE import — do NOT rely on glob-import-of-private-use semantics. Explicit is robust.

// GOTCHA (neutral class for title-length tests): for cases (c)/(d)/(e) use a class like "SomeApp"
//   that is NOT in ignore_classes and NOT in allow_empty_title, so the short-title rule is the
//   only thing under test. (Title is non-empty in these, so allow_empty_title doesn't apply, but
//   a neutral class keeps the intent unambiguous.)

// GOTCHA (should_ignore_window is PURE): it calls NO Windows APIs — only String == , .is_empty(),
//   .chars().count(). That is WHY the standalone Linux harness (copy the body + tests) is a valid
//   logic check. Do not add any windows crate / Win32 call to this function.
```

## Implementation Blueprint

### Data models and structure
None. `WindowInfo` (in `core/types.rs`) is unchanged. The fix + tests operate on
`&WindowInfo`'s existing `app_class: String` / `title: String` fields.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: FIX src/platforms/windows.rs line 306 (the bug)
  - OLD (exact, unique): "    if window_info.title.len() < 2 && !window_info.title.is_empty() {"
  - NEW: "    if window_info.title.chars().count() < 2 && !window_info.title.is_empty() {"
  - ONLY `.len()` → `.chars().count()`. Update the comment above it to cite Bug 5 (optional).
  - PRESERVE: the `&& !window_info.title.is_empty()` guard, the `return true;`, the
    blocklist above, and the empty-title rule above.

Task 2: APPEND #[cfg(test)] mod tests at end-of-file (src/platforms/windows.rs, after L529)
  - IMPLEMENT: the test module verbatim from the "What → Edit 2" block above (6 #[test] fns +
    a `wi(class, title)` helper).
  - IMPORTS: `use super::*;` + `use crate::core::types::WindowInfo;` (explicit).
  - NAMING: test_ignore_empty_title_non_allowed_class / test_keep_empty_title_allowed_class /
    test_ignore_one_char_ascii_title / test_ignore_one_char_emoji_title /
    test_keep_two_char_title / test_ignore_internal_class_name.
  - PLACEMENT: after the closing brace of create_config_dir (currently the last item, L529).
  - COVERAGE: the 6 verified cases (a)-(f). Test (d) is THE regression case for the fix.

Task 3: VALIDATE (no edits) — two tracks
  - TRACK A — WINDOWS BOX (canonical; the file is Windows-only):
        cargo test --bin qmkonnect should_ignore -- --test-threads=1   # 6 new tests PASS
        cargo test --bin qmkonnect -- --test-threads=1                 # full suite green
    (AGENTS.md Windows loop: MSVC toolchain. --test-threads=1 is REQUIRED — shared debouncer.)
  - TRACK B — LINUX DEV BOX (no Windows box available; verifies fix LOGIC + assertions):
    1. cargo build                                          # confirms NO regression to non-windows code
                                                            # (windows.rs is cfg-gated OUT on Linux, so
                                                            # this does NOT compile the fix/tests — see 2b)
    2. standalone harness (Validation Level 2b): copy should_ignore_window's BODY + the 6 tests
       into a /tmp file; run on Linux. Proves the assertions + .chars().count() logic are correct.
  - git diff --stat                                         # ONLY src/platforms/windows.rs

Task 4: NEVER do these (out of scope / forbidden)
  - DO NOT remove the short-title heuristic — FIX it to .chars().count() (contract).
  - DO NOT edit the ignore_classes blocklist or the allow_empty_title list.
  - DO NOT edit src/core/types.rs (WindowInfo is already correct).
  - DO NOT add any Windows/Win32 API call to should_ignore_window (keep it pure).
  - DO NOT rely on Linux `cargo build`/`cargo test` to validate the fix — windows.rs is cfg-gated
    out on Linux. Use the Windows box (Track A) and/or the standalone harness (Track B).
  - DO NOT edit src/linux_tray.rs (parallel P1.M2.T1.S3) or any other file.
  - DO NOT edit PRD.md, any tasks.json, prd_snapshot.md, or Cargo.toml.
```

### Implementation Patterns & Key Details
```rust
// PATTERN: a pure helper under test is exercised with a tiny `wi(class, title)` builder that
//   wraps WindowInfo::new(class.to_string(), title.to_string()), keeping each #[test] one line.
//   Mirrors src/core/types.rs's `use super::*` + direct construction convention.

// PATTERN: the test module is the LAST item in a cfg(windows) file (see src/autostart.rs L120).
//   Append after create_config_dir's closing brace (L529).

// WHY test (d) is the regression case: "😀" is 4 UTF-8 bytes. .len()=4 (>=2 ⇒ kept, the bug);
//   .chars().count()=1 (<2 ⇒ ignored, the fix). The assertion `assert!(should_ignore_window(...))`
//   fails on the OLD code and passes on the NEW code — a perfect one-line regression guard.

// ANTI-PATTERN: do NOT replace the whole should_ignore_window body or reorder its checks. The
//   blocklist (step 1) and empty-title rule (step 2) MUST stay above the short-title rule (step 3)
//   — the 6 tests' expected values depend on that order (e.g. test f returns at step 1; test b
//   returns false only because step 2 falls through AND step 3's `&& !empty` is false).
```

### Integration Points
```yaml
IMPORTS: none new in production code (String already in scope; .chars().count() is a std method).
TEST IMPORTS: the new #[cfg(test)] mod tests adds `use super::*;` + `use crate::core::types::WindowInfo;`.
CARGO:   none. No Cargo.toml change (no new dependency; .chars().count() is std).
CALLERS: should_ignore_window is called ONLY at src/platforms/windows.rs:206 (handle_focus_change
         filter). Its callers' behavior is unchanged for all titles EXCEPT multi-byte short titles
         (now correctly ignored) — which is the intended fix.
DOWNSTREAM: none (P1.M2.T1.S3 edits a different file; P1.M2.T3 docs task may note the heuristic).
PLATFORM: windows.rs is #![cfg(target_os = "windows")]; the edit + tests are inert on macOS/Linux.
```

## Validation Loop

> Toolchain: Rust (`cargo`). Project has no ruff/mypy. Tests MUST run single-threaded
> (AGENTS.md — shared debouncer state). **This file is Windows-only** (line 1 cfg gate),
> so the canonical validation is a Windows box; the Linux box can only verify the LOGIC.

### Level 1: Syntax & Style (compile gate — Windows box)
```bash
# On a WINDOWS box (MSVC toolchain — AGENTS.md Windows loop):
cd <repo>
cargo build --bin qmkonnect
# Expected: clean build; the fix (.chars().count()) + the new test module type-check.
# (On the Linux dev box this file is NOT compiled — `cargo build` there proves nothing about
#  the fix/tests; use Level 2b instead.)
```

### Level 2a: Unit Tests (Component Validation — Windows box, canonical)
```bash
# On a WINDOWS box:
cd <repo>
cargo test --bin qmkonnect should_ignore -- --test-threads=1
# Expected: 6 passed, 0 failed:
#   test_ignore_empty_title_non_allowed_class  (a) => true
#   test_keep_empty_title_allowed_class        (b) => false
#   test_ignore_one_char_ascii_title           (c) => true
#   test_ignore_one_char_emoji_title           (d) => true  (THE FIX regression case)
#   test_keep_two_char_title                   (e) => false
#   test_ignore_internal_class_name            (f) => true
```

### Level 2b: Standalone logic harness (Linux dev box — when NO Windows box is available)
```bash
cd /home/dustin/projects/qmkonnect
# should_ignore_window is PURE (no Win32 calls). Copy its body + the 6 tests into a standalone
# file that compiles+runs on Linux to PROVE the fix logic + assertions before the Windows run.
cat > /tmp/siw_test.rs <<'EOF'
// Standalone mirror of should_ignore_window (windows.rs:250-310) with the FIX applied.
fn should_ignore_window(app_class: &str, title: &str) -> bool {
    let ignore_classes = [
        "ForegroundStaging","XamlExplorerHostIslandWindow",
        "Windows.UI.Composition.DesktopWindowContentBridge","Windows.UI.Input.InputSite.WindowClass",
        "TaskSwitcherWnd","TaskSwitcherOverlayWnd","TopLevelWindowForOverflowXamlIsland",
        "NotifyIconOverflowWindow","Shell_TrayWnd","Shell_SecondaryTrayWnd",
    ];
    if ignore_classes.iter().any(|&c| app_class == c) { return true; }
    if title.is_empty() {
        let allow_empty_title = ["CASCADIA_HOSTING_WINDOW_CLASS","Chrome_WidgetWin_1"];
        if !allow_empty_title.iter().any(|&c| app_class == c) { return true; }
    }
    if title.chars().count() < 2 && !title.is_empty() { return true; }  // THE FIX
    false
}
fn main() {
    let mut pass = 0u32; let mut fail = 0u32;
    macro_rules! ck { ($e:expr, $want:expr, $name:expr) => {{
        let got = $e; let want = $want;
        if got == want { pass += 1; println!("ok   {} => {}", $name, got); }
        else { fail += 1; println!("FAIL {} => {} (want {})", $name, got, want); }
    }}}
    ck!(should_ignore_window("SomeApp", ""),            true,  "(a) empty + non-allowed");
    ck!(should_ignore_window("Chrome_WidgetWin_1", ""), false, "(b) empty + allowed");
    ck!(should_ignore_window("SomeApp", "x"),           true,  "(c) 1-char ASCII");
    ck!(should_ignore_window("SomeApp", "😀"),          true,  "(d) 1-char emoji (FIX case)");
    ck!(should_ignore_window("SomeApp", "ab"),          false, "(e) 2-char");
    ck!(should_ignore_window("Shell_TrayWnd", "Taskbar"), true, "(f) internal class");
    println!("\n{} passed, {} failed", pass, fail);
    std::process::exit(if fail == 0 { 0 } else { 1 });
}
EOF
rustc -O /tmp/siw_test.rs -o /tmp/siw_test 2>&1 | head && /tmp/siw_test
rm -f /tmp/siw_test /tmp/siw_test.rs
# Expected: "6 passed, 0 failed", exit 0. (Proves the fix + assertions; does NOT compile windows.rs.)
```

### Level 3: Full Suite (Regression — Windows box)
```bash
# On a WINDOWS box:
cd <repo>
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL existing tests still pass + the 6 new should_ignore tests. --test-threads=1 REQUIRED.
```

### Level 4: Scope/Build Hygiene (any box)
```bash
cd /home/dustin/projects/qmkonnect
git diff --stat                 # Expected: ONLY src/platforms/windows.rs.
git diff Cargo.toml             # Expected: empty.
# Confirm the one-line fix landed and the test module is present:
grep -n 'title.chars().count() < 2' src/platforms/windows.rs   # expect ONE match (L306)
grep -n 'title.len() < 2' src/platforms/windows.rs             # expect NO matches (old form gone)
grep -n 'mod tests' src/platforms/windows.rs                   # expect ONE match (the new module)
grep -c '#\[test\]' src/platforms/windows.rs                   # expect 6
```

## Final Validation Checklist

### Technical Validation
- [ ] **Windows box**: `cargo build --bin qmkonnect` clean; `cargo test --bin qmkonnect
      should_ignore -- --test-threads=1` → 6 passed; `cargo test --bin qmkonnect -- --test-threads=1` green.
- [ ] **(or) Linux box**: standalone harness (Level 2b) → "6 passed, 0 failed"; `cargo build`
      shows no regression to non-windows code.
- [ ] `git diff --stat` shows ONLY `src/platforms/windows.rs`.

### Feature Validation
- [ ] L306 uses `.chars().count()` (not `.len()`); the old form is gone.
- [ ] Test (d) `"😀"` asserts **true** (ignored) — the fix's regression case (was false before).
- [ ] Tests (a)/(c)/(f) assert true; (b)/(e) assert false — all verified against the function logic.
- [ ] Blocklist (step 1) and empty-title rule (step 2) UNCHANGED (tests f and b depend on them).

### Code Quality Validation
- [ ] Test module appended at end-of-file with `use super::*;` + explicit `use crate::core::types::WindowInfo;`.
- [ ] `wi(class, title)` helper keeps each test one line (mirrors types.rs simplicity).
- [ ] No Windows/Win32 API added to `should_ignore_window` (kept pure).
- [ ] No edit to `types.rs`, the blocklist, the allow-empty-title list, Cargo.toml, or any other file.

### Documentation & Deployment
- [ ] No user-facing / config / API / CLI change (internal heuristic — DOCS: none per contract).
- [ ] Inline comment on L305-306 cites Bug 5 / char-count rationale (recommended).

---

## Anti-Patterns to Avoid
- ❌ Don't remove the short-title heuristic — FIX it to `.chars().count()` (the contract; the PRD's "or remove it" is explicitly NOT chosen).
- ❌ Don't rely on Linux `cargo build`/`cargo test` to validate this fix — `windows.rs` is `#![cfg(target_os="windows")]`, so it is NOT compiled on Linux; those commands prove nothing about the fix/tests. Use the Windows box (Level 2a) and/or the standalone harness (Level 2b).
- ❌ Don't reorder or rewrite `should_ignore_window`'s checks — the blocklist (step 1) and empty-title rule (step 2) must stay above the short-title rule (step 3); the 6 tests' expected values depend on that order.
- ❌ Don't add a Windows/Win32 API call to `should_ignore_window` — keep it pure (that's what makes it unit-testable + standalone-harness-verifiable).
- ❌ Don't rely on glob-import-of-private-use for `WindowInfo` in the test module — add an explicit `use crate::core::types::WindowInfo;`.
- ❌ Don't use an internal/allowed class for the title-length tests (c)/(d)/(e) — use a neutral class like `"SomeApp"` so the short-title rule is the only thing under test.
- ❌ Don't edit `src/core/types.rs`, `src/linux_tray.rs` (parallel task), Cargo.toml, PRD.md, tasks.json, or any file other than `src/platforms/windows.rs`.

---

## Confidence Score: 9/10

The fix is a one-character-class change (`.len()` → `.chars().count()`) on a pure function,
and the 6 test expected values were each verified this session against the actual
`should_ignore_window` logic (including the load-bearing regression case (d) where "😀" flips
from kept→ignored). The score is 9 rather than 10 ONLY because the file is Windows-only and
cannot be compiled/run on the Linux dev box — so the canonical test run requires a Windows
box. That risk is fully mitigated for implementers without a Windows box by the standalone
Linux harness (Validation Level 2b), which proves the fix logic + all 6 assertions compile
and pass on Linux before any Windows run. The one residual uncertainty is whether the
implementer's environment can execute the Windows test (Track A) — but either track
independently establishes correctness.