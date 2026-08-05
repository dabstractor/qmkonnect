# PRP — P1.M1.T1.S1: Change list_foreground_windows() to use initial_class instead of class

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). ALL edits in ONE file:
> `src/platforms/hyprland.rs`, inside ONE function: `list_foreground_windows()`.
> **Scope:** A two-token bug fix (`c.class` → `c.initial_class` AND `active.class` →
> `active.initial_class`). Makes the "Show Window Information" dialog report the
> SAME identifier the notify path sends to the firmware. **No new tests** (the
> function does live Hyprland IPC — unverifiable without a running compositor).

---

## Goal

**Feature Goal**: Make `list_foreground_windows()` in `src/platforms/hyprland.rs`
return `initial_class` (the stable, set-once-at-creation identifier) as the first
tuple element, instead of `class` (the mutable, runtime-changeable field). This
makes the "Show Window Information" dialog display the **same** value the notify
path (`poll_window_state` L398, `handle_window_state_change` L479) sends to the
keyboard firmware — so a user who copies the dialog's class into a rule gets a
value that actually matches, instead of one that can silently differ for apps
that mutate `class` at runtime (some Electron apps).

**Deliverable**: `src/platforms/hyprland.rs` with two exact token swaps inside
`list_foreground_windows()` (L571 and L577) — no other change.

**Success Definition**: `cargo build` (default features, which compile the
`hyprland` platform) succeeds with zero warnings; `cargo clippy --all-targets --
-D warnings` is clean; `cargo fmt --check` is clean; the single-threaded test suite
still passes (no regression — the change is type-identical, `String` → `String`);
on a live Hyprland session, the dialog's reported class equals what the notify path
sends for an app where `class != initial_class`.

## User Persona (if applicable)

**Target User**: A Hyprland user authoring host-side rules (`rules.toml`) or board
rules (`DEFINE_SERIAL_LAYERS`) who uses the "Show Window Information" dialog to
discover an app's class string.

**Use Case**: User opens the dialog, reads the class for their editor/browser,
pastes it into a `match = "…"` rule, and expects the rule to fire.

**User Journey**: (before) dialog shows `class` (mutable) → user copies it → rule
silently fails to match because the firmware receives `initial_class` (stable) →
frustration. (after) dialog shows `initial_class` → user copies it → rule matches.

**Pain Points Addressed**: Eliminates the silent class/initial_class mismatch that
made Hyprland rules appear broken for apps that change `class` at runtime. The fix
makes the dialog truthful and consistent with the existing docs (which already say
"window class" — `initial_class` is the stable interpretation).

## Why

- **Rule correctness.** The notify path is the source of truth for what the firmware
  matches, and it uses `initial_class` (the PRD-mandated stable identifier —
  `bug_findings.md` Issue 1). The dialog using the mutable `class` is a real defect:
  for apps where the two differ, the dialog misleads the user and rules authored from
  it silently fail. This is a one-line-per-site correctness fix.
- **It is the smallest possible change.** Two token swaps in one function; both
  fields are `String` on the same `Client` struct, so it is type-identical and cannot
  break compilation. No API, config, or wire-protocol change.
- **It matches the sibling recommendation.** `bug_findings.md` §Recommendations:
  "Fix Finding 1 (Hyprland) … immediately as [a] clear one-liner affecting rule
  correctness." (Finding 2, the X11 off-by-one, is a separate sibling task
  P1.M1.T2 — out of scope here.)

## What

### The two edits (EXACT before → after), both inside `list_foreground_windows()` (L559)

```rust
// L571 — building the dialog rows from the enumerated clients:
-        .map(|c| (c.class.clone(), c.title.clone()))
+        .map(|c| (c.initial_class.clone(), c.title.clone()))

// L577 — building the active-window key to move it to the front:
-        let key = (active.class.clone(), active.title.clone());
+        let key = (active.initial_class.clone(), active.title.clone());
```

> **⚠ BOTH lines must change together** (see Gotchas): L577's `key` is matched
> against L571's `rows` via `rows.iter().position(|r| *r == key)`. If only one
> changes, the row uses one field and the key the other → for any app where
> `class != initial_class` the position lookup silently fails and the active window
> is no longer moved to the front. The two edits are a coupled pair.

### Full target function (for orientation — only the two marked lines change)

```rust
pub fn list_foreground_windows() -> Vec<(String, String)> {
    let clients = match Clients::get() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to enumerate Hyprland clients: {}", e);
            return Vec::new();
        }
    };

    let mut rows: Vec<(String, String)> = clients
        .iter()
        .filter(|c| c.mapped)
        .map(|c| (c.initial_class.clone(), c.title.clone()))   // ← L571 (was c.class)
        .collect();

    // Move the active window to the front so callers taking `.next()` report
    // the focused window (parity with the macOS/Windows "active window" notion).
    if let Ok(Some(active)) = Client::get_active() {
        let key = (active.initial_class.clone(), active.title.clone());  // ← L577 (was active.class)
        if let Some(pos) = rows.iter().position(|r| *r == key) {
            rows.swap(0, pos);
        }
    }

    rows
}
```

### Success Criteria

- [ ] L571 uses `c.initial_class.clone()` (not `c.class.clone()`).
- [ ] L577 uses `active.initial_class.clone()` (not `active.class.clone()`).
- [ ] `handle_window_state_change` (L479) and `poll_window_state` (L398) are
      **unchanged** (they already use `initial_class` — do not touch them).
- [ ] No other line in `hyprland.rs` (or any other file) is modified.
- [ ] `cargo build` (default features) compiles with zero warnings.
- [ ] `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt --check` clean.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` passes (no regression).

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed
> to implement this successfully?"_ — **Yes.** The two exact before→after edits,
> the full target function for orientation, the coupling rationale (why both lines
> change together), the verified crate-API confirmation (`Client` has both fields),
> the no-unit-test rationale, and the verified validation commands are all below.

### Documentation & References

```yaml
# MUST READ — the bug finding that defines this fix
- file: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/architecture/bug_findings.md
  why: "Issue 1 (Major) names exactly L571 + L577 as the dialog sites using `class` while the notify
        path uses `initial_class`, and prescribes the one-line-per-site fix. §Recommendations prioritizes
        it as a clear rule-correctness fix."
  section: "Issue 1: Hyprland \"Show Window Information\" reports class..." and "Recommendations"
  critical: "The notify paths (L398 poll_window_state, L479 handle_window_state_change) ALREADY use
             initial_class — do NOT touch them. Only the dialog (list_foreground_windows) is wrong."

# MUST READ — the file being edited (confirm exact current code before editing)
- file: /home/dustin/projects/qmkonnect/src/platforms/hyprland.rs
  why: "Contains list_foreground_windows() at L559, with the two .class.clone() sites at L571 and L577.
        The notify paths at L398 and L479 already use initial_class — confirming the consistency target."
  pattern: "WindowInfo/app_class is populated from active_window.initial_class.clone() in the notify
            path. The dialog (list_foreground_windows) must match."
  gotcha: "L571 builds the rows; L577 builds the active-window key that is matched AGAINST those rows
           (position(|r| *r == key)). Both must use the SAME field or the active-window-to-front lookup
           silently breaks. Change BOTH, never just one."

# REFERENCE — the hyprland crate Client struct (confirms both fields exist and are String)
- file: ~/.cargo/registry/src/.../hyprland-0.4.0-beta.3/src/data/regular.rs
  why: "Defines `pub initial_class: String` (L239) and `pub class: String` (L241) on the Client struct.
        Confirms c.initial_class.clone() / active.initial_class.clone() compile (both are String — the
        change is type-identical). initial_class is set once at window creation; class is mutable at runtime."
  section: "struct Client { ... }"
  critical: "Both fields are `String`. The change cannot alter any type signature — Vec<(String,String)>
             stays the same. No downstream caller is affected type-wise."

# REFERENCE — the docs that already say "window class" (the fix makes the dialog match them)
- file: /home/dustin/projects/qmkonnect/docs/troubleshooting.md
  why: "Existing user-facing guidance refers to 'window class'. The fix makes the dialog's reported value
        consistent with that wording AND with what the firmware receives (initial_class = the stable class)."
  section: "window-class guidance (verified/updated in P1.M2.T3.S1 — sibling doc task)"
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/                       # THIS repo
├── Cargo.toml                   # `hyprland` is a DEFAULT feature → `cargo build` compiles hyprland.rs
└── src/platforms/
    └── hyprland.rs              # <-- FILE TO EDIT. list_foreground_windows() @559; fix sites @571,577.
                                 #     notify paths @398, @479 (already initial_class — leave alone).
```

### Desired Codebase tree with files to be modified

```bash
src/platforms/
└── hyprland.rs   # MODIFIED ONLY — two token swaps in list_foreground_windows() (L571, L577).
```

> No new files. No test file (the function uses live IPC — see Gotchas). No other platform file
> (`windows.rs`/`macos.rs` use their own class resolution and are unaffected by this Hyprland bug).

### Known Gotchas of our codebase & Library Quirks

```text
# CRITICAL: change BOTH L571 and L577 in the same edit — they are coupled.
#   L571 builds the rows (first element = the class field). L577 builds the active-window key whose
#   first element is matched against the rows via `position(|r| *r == key)`. If you change only one,
#   the row and key use DIFFERENT fields; for any app where class != initial_class the position lookup
#   returns None and the active window is NOT moved to the front (silent regression). Always edit both.

# CRITICAL: do NOT touch the notify paths (L398, L479).
#   poll_window_state (L398) and handle_window_state_change (L479) already use active_window.initial_class.
#   They are the source of truth the dialog is being aligned TO. Leave them byte-for-byte unchanged.

# CRITICAL: there is NO unit test for list_foreground_windows() — and you cannot add one here.
#   It calls Clients::get() and Client::get_active() (static methods on the hyprland crate that do live
#   socket IPC to a running Hyprland compositor). There is no injection point in the signature, and the
#   contract scopes this to the two token swaps (no refactor to add a seam). Verification is MANUAL, on a
#   real Hyprland session, with an app where class != initial_class (some Electron apps, or any app that
#   calls setprop/xprop to mutate WM_CLASS at runtime). Do NOT add a #[cfg(test)] test that calls this fn.

# NOTE: both `initial_class` and `class` are `String` on the hyprland Client struct.
#   So `c.initial_class.clone()` and `active.initial_class.clone()` are type-identical to the originals.
#   The function signature Vec<(String, String)> is unchanged; no caller is affected.

# NOTE: `hyprland` is a DEFAULT Cargo feature.
#   `cargo build` / `cargo build --release` (default features) compiles src/platforms/hyprland.rs, so the
#   edit IS checked by the normal build gate. (`cargo build --no-default-features` excludes it — irrelevant
#   to this file, but the gate still passes since the change is isolated to a feature-gated module.)

# NOTE: tests are single-threaded (`cargo test --bin qmkonnect -- --test-threads=1`) per AGENTS.md
#   (shared global debouncer state). This change adds no tests, but the suite must still pass — confirming
#   the type-identical swap didn't regress anything.
```

## Implementation Blueprint

### Data models and structure

No data-model change. The function returns `Vec<(String, String)>` (class, title)
both before and after — only which `String` field populates the first element
changes (`class` → `initial_class`). The hyprland `Client` struct (crate-owned) is
untouched.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: CONFIRM the exact current code at the two sites
  - READ: src/platforms/hyprland.rs list_foreground_windows() (L559-584). Confirm L571 is
          `.map(|c| (c.class.clone(), c.title.clone()))` and L577 is
          `let key = (active.class.clone(), active.title.clone());`.
  - CONFIRM: grep -nE '\.class\.clone\(\)' src/platforms/hyprland.rs returns EXACTLY L571 and L577
          (the only two .class.clone() sites; the notify paths use .initial_class.clone()).
  - GOAL: anchor the two edits so neither misses (and neither hits the wrong line).

Task 2: APPLY the two token swaps (in the SAME edit)
  - EDIT L571:  c.class.clone()  ->  c.initial_class.clone()
  - EDIT L577:  active.class.clone()  ->  active.initial_class.clone()
  - DO BOTH TOGETHER (they are coupled — see Gotchas). Leave .title.clone() untouched on both lines.
  - DO NOT: touch L398, L479 (notify paths), the function signature, any other line, or any other file.

Task 3: VALIDATE
  - RUN: cargo build            (default features compile hyprland.rs; expect zero warnings)
  - RUN: cargo clippy --all-targets -- -D warnings   (expect clean)
  - RUN: cargo fmt --check      (expect exit 0)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1   (expect all pass — no regression)
  - RUN: grep -nE '\.class\.clone\(\)' src/platforms/hyprland.rs   (expect ZERO matches — both swapped)
  - RUN: grep -nE '\.initial_class\.clone\(\)' src/platforms/hyprland.rs   (expect 4: L398, L479, L571, L577)
```

### Implementation Patterns & Key Details

```rust
// === THE COUPLING — why both lines change together ===
// L571: rows are built with the (class-field, title) of each mapped client.
// L577: key  is built with the (class-field, title) of the active client.
// L578: rows.iter().position(|r| *r == key)  ← compares row[0] == key[0] (and row[1] == key[1]).
// If row[0] uses initial_class but key[0] uses class (or vice versa), equality fails for any app
// where the two differ → active window not promoted to front. Both must use initial_class.

// === WHY NO UNIT TEST ===
// Clients::get() / Client::get_active() are hyprland-crate statics doing UNIX-socket IPC to a live
// compositor. No mock seam; no compositor in CI. The type-identical swap is checked by `cargo build`
// (both fields are String); behavioral correctness is checked manually on Hyprland.
```

### Integration Points

```yaml
SOURCE FILES:
  - modify: "src/platforms/hyprland.rs ONLY (two token swaps in list_foreground_windows)"

PUBLIC API SURFACE:
  - unchanged: "list_foreground_windows() -> Vec<(String,String)> signature. The first tuple element
                now carries initial_class instead of class (same type, more-correct value)."

DEPENDENCIES / Cargo.toml:
  - none. No new deps. The hyprland crate (0.4.0-beta.3) already exposes both fields.

TESTS:
  - none added. "list_foreground_windows() requires live Hyprland IPC; no unit test (contract scope).
                 Verify manually: run on Hyprland, open an app where class != initial_class, confirm the
                 dialog's class equals the notify path's app_class."

RELATED (sibling tasks — NOT this subtask):
  - P1.M1.T2.S1: "X11 WM_CLASS off-by-one (bug_findings Issue 2) — different file (x11.rs)."
  - P1.M2.T3.S1: "docs/troubleshooting.md window-class guidance — verify wording still accurate."
```

## Validation Loop

> All commands run from the repo root: `/home/dustin/projects/qmkonnect`.

### Level 1: Syntax & Style

```bash
cd /home/dustin/projects/qmkonnect

cargo build 2>&1 | tee /tmp/build.log
# Expected: "Finished" — zero warnings. (Both fields are String; the swap is type-identical. If it fails,
# you hit the wrong line — re-check L571/L577 are the only edits.)

cargo clippy --all-targets -- -D warnings 2>&1 | tee /tmp/clippy.log | grep -iE 'warning|error' || echo "clippy clean"
# Expected: clean (no new lint).

cargo fmt --check
# Expected: exit 0. (Two token swaps don't affect formatting; if non-zero, run `cargo fmt`.)
```

### Level 2: Unit Tests (regression only — no new test)

```bash
cd /home/dustin/projects/qmkonnect

cargo test --bin qmkonnect -- --test-threads=1 2>&1 | tail -3
# Expected: "test result: ok. <N> passed; 0 failed; ...". The swap is type-identical and isolated to a
# feature-gated dialog function with no unit tests, so the existing suite is unaffected. A failure here
# would mean the edit accidentally touched something else — re-check the diff.
```

### Level 3: Behavioral verification (MANUAL — requires a Hyprland session)

```text
This is the ONLY way to verify the fix's behavior (the function does live IPC; no unit test possible).

On a machine running Hyprland:
1. Build: cargo build --release
2. Run the app (or just invoke the dialog path): open "Show Window Information".
3. Launch an app where class != initial_class (some Electron apps, or any app that mutates WM_CLASS via
   xprop/hyprctl setprop at runtime).
4. Compare the dialog's reported class against what the notify path sends:
   - Enable verbose (-v) and watch the sanitized payload log (the app_class portion, shown before the
     GS separator). It uses initial_class (notify path).
   - The dialog's first column must EQUAL that logged app_class.
   (Before the fix they could differ; after the fix they are identical.)
5. Also confirm the active window is still sorted to the top of the dialog (proves the L571+L577 coupling
   is intact — if you'd changed only one line, the active window would no longer promote for differing
   apps).
```

### Level 4: Scope-preservation grep (prove the change is exactly the two swaps)

```bash
cd /home/dustin/projects/qmkonnect

# (a) Zero remaining .class.clone() in hyprland.rs (both swapped out).
grep -nE '\.class\.clone\(\)' src/platforms/hyprland.rs
# Expected: NO output. (Any match = a site was missed or a wrong line edited.)

# (b) Exactly 4 .initial_class.clone() sites: the 2 notify paths (unchanged) + the 2 dialog sites (fixed).
grep -nE '\.initial_class\.clone\(\)' src/platforms/hyprland.rs
# Expected: 4 lines — L398, L479 (notify, unchanged), L571, L577 (dialog, fixed).

# (c) The diff is exactly two lines (one char-token each).
git diff -- src/platforms/hyprland.rs
# Expected: one hunk, two `-`/`+` pairs, differing only by `class` → `initial_class`. Title lines unchanged.

# (d) No other file touched.
git status --short
# Expected: only `M src/platforms/hyprland.rs`.
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1: `cargo build` zero warnings; `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt --check` exit 0.
- [ ] Level 2: `cargo test --bin qmkonnect -- --test-threads=1` all pass (no regression).
- [ ] Level 4 (a): `grep '\.class\.clone()' src/platforms/hyprland.rs` → zero matches.
- [ ] Level 4 (b): `grep '\.initial_class\.clone()' src/platforms/hyprland.rs` → exactly 4 lines (398/479/571/577).
- [ ] Level 4 (c): `git diff` → two `-`/`+` pairs, only `class`→`initial_class`.
- [ ] Level 4 (d): only `src/platforms/hyprland.rs` modified.

### Feature Validation

- [ ] L571 uses `c.initial_class.clone()`.
- [ ] L577 uses `active.initial_class.clone()` (both changed together — coupling intact).
- [ ] L398 and L479 (notify paths) unchanged.
- [ ] Manual Hyprland check: dialog class == notify-path app_class for an app where they differ.
- [ ] Manual: active window still sorts to dialog top (coupling not broken).

### Code Quality Validation

- [ ] The fix aligns the dialog with the notify path (single source of truth: `initial_class`).
- [ ] No new pattern introduced — uses the same `.initial_class.clone()` idiom as L398/L479.
- [ ] No refactor / no test seam added (contract scopes to the two swaps).
- [ ] No other platform file (`windows.rs`/`macos.rs`) touched.

### Documentation & Deployment

- [ ] No user-facing/config/API surface change (the dialog already documents "window class"; this makes
      the reported value the stable one — MORE consistent with existing docs).
- [ ] `docs/troubleshooting.md` window-class guidance verified by the sibling doc task P1.M2.T3.S1.

---

## Anti-Patterns to Avoid

- ❌ Don't change only ONE of L571/L577 — they are coupled (the L577 key is matched against the L571 rows).
  Changing one alone silently breaks the active-window-to-front promotion for differing apps. Edit both.
- ❌ Don't touch the notify paths (L398 `poll_window_state`, L479 `handle_window_state_change`) — they
  already use `initial_class` and are the consistency target, not the bug.
- ❌ Don't add a unit test for `list_foreground_windows()` — it calls live Hyprland IPC (`Clients::get()`,
  `Client::get_active()`); there is no compositor in CI and no mock seam. Verification is manual. (Adding
  a test that calls it would either fail in CI or require a refactor outside this subtask's scope.)
- ❌ Don't refactor the function to inject the client list "to make it testable" — the contract scopes this
  to the two token swaps. A seam-injection is a larger change and belongs in a different task.
- ❌ Don't change the `.title.clone()` portions — titles are not affected by the class/initial_class bug.
- ❌ Don't edit `windows.rs` or `macos.rs` — this is a Hyprland-specific defect (those platforms resolve
  class differently and are unaffected).
- ❌ Don't assume `class == initial_class` always — they differ for some Electron apps and any app that
  mutates WM_CLASS at runtime. That difference is the entire reason the bug exists.
- ❌ Don't drop `--test-threads=1` from the test command — shared global debouncer state (AGENTS.md).

---

**Confidence Score: 10/10** for one-pass implementation success. The fix is two
type-identical token swaps (`String` field → `String` field) in one function,
verified against the live code (L571/L577 are the only `.class.clone()` sites;
L398/L479 already use `initial_class`) and the crate API (`Client` exposes both
`initial_class` and `class` as `String`). The one non-obvious correctness point —
that L571 and L577 are coupled by the `position(|r| *r == key)` active-window
lookup and must change together — is called out twice. The only thing that cannot
be automated is the behavioral check (live Hyprland IPC); the type/build/clippy/
grep gates make the structural correctness deterministic.