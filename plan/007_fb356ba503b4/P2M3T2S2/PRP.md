# PRP — P2.M3.T2.S2: First-run GNOME notification when extension missing

> **RE-PLAN (attempt 2/3) of a previously-failed PRP.**
> Attempt 1 hit a hard scope conflict: it told the implementer to call
> `crate::platforms::gnome::probe_available(false)` from `src/runners/linux.rs`,
> but at that time `mod gnome;` was **private** in `src/platforms/mod.rs`
> (E0603 "module `gnome` is private"), and the PRP forbade editing `mod.rs`.
> That conflict is **now resolved in the committed tree**: the one-word
> visibility fix (Option A — `pub(crate) mod gnome;`) has landed, the full
> notification implementation is committed and compiles, and all 8 unit tests
> pass. **Therefore this PRP's primary NEW deliverable is the one contract point
> still unmet: the GNOME extension coverage in `docs/qmk-integration.md`.**

---

## Goal

**Feature Goal**: On a GNOME session where the `io.mulletware.QMKonnect` Shell
extension is not installed/enabled at startup, fire a single one-shot
`notify-send` desktop notification pointing the user at the docs, at most once
per launch — AND ensure `docs/qmk-integration.md` (the doc the notification
points at) actually covers the GNOME extension.

**Deliverable**:
1. *(Already implemented + committed — verify only, do not rewrite)* the one-shot
   GNOME first-run notification in `src/runners/linux.rs`, gated by a static
   `AtomicBool`, reusing `gnome::probe_available` + `platforms::notify`.
2. *(PRIMARY new work)* A new **"GNOME Shell Extension (Linux Desktop
   Prerequisite)"** section in `docs/qmk-integration.md` that explains the
   extension, links to the detailed install/troubleshooting pages, and documents
   the first-run notification + AT-SPI interim.

**Success Definition**:
- `cargo check --features gnome` is clean (it already is).
- The 8 GNOME one-shot unit tests in `runners::linux::tests` pass
  (`cargo test --bin qmkonnect -- --test-threads=1` — they already do).
- `docs/qmk-integration.md` contains a discoverable GNOME Shell Extension
  section naming the UUID `qmkonnect@mulletware`, the install source
  (extensions.gnome.org), and cross-links to `installation` + `troubleshooting`.

## Why

- **User impact**: a GNOME user who installs QMKonnect without the Shell
  extension gets *silent* no-window-detection on the one DE that has no client
  API for the active window. The one-shot notification tells them exactly what's
  missing and where to look, so the "it doesn't work" support ticket never opens.
- **Integration with existing features**: the notification reuses the existing
  `platforms::notify` (notify-send shell-out) and S1's `gnome::probe_available`
  (Ok ⇔ D-Bus name owned ⇔ extension installed+enabled). The daemon keeps
  running (tray + device pipeline); the AT-SPI fallback (P2.M4) may run meanwhile
  as a best-effort interim.
- **Boundary respect**: this is the *UX hint* layer only. It does NOT install or
  load the extension (the app cannot run inside `gnome-shell` — PLATFORMS.md §8.5);
  it does NOT change backend selection; it does NOT touch the firmware module.

## What

### User-visible behavior
On launch, **if** `$XDG_CURRENT_DESKTOP` contains `GNOME` (case-insensitive —
covers `GNOME`, `ubuntu:GNOME`, `GNOME-classic`) **and** the well-known name
`io.mulletware.QMKonnect` is **not** owned on the session bus, the daemon fires
**one** desktop notification (app-name `QMKonnect`, keyboard icon) titled
"QMKonnect needs the GNOME Shell extension" with a body pointing to
extensions.gnome.org and the docs. It never fires again this launch, even across
backend-selection Ok/Err branches or monitor restarts.

### Success Criteria
- [ ] On a GNOME session with the extension missing, exactly one notification
      fires per process (verified by the `consume_gnome_hint_shot_is_one_shot`
      test — the AtomicBool guard is never re-armed).
- [ ] When the extension IS present (`probe_available` → Ok), **no**
      notification fires (verified by the `probe_available(false).is_ok()`
      early-return branch).
- [ ] On a non-GNOME session, nothing fires (`gnome_session()` returns false).
- [ ] `cargo check --features gnome` clean; all GNOME one-shot unit tests pass.
- [ ] `docs/qmk-integration.md` documents the extension (UUID, install source,
      first-run notification, AT-SPI interim) with cross-links.

## All Needed Context

### Context Completeness Check
_"If someone knew nothing about this codebase, would they have everything
needed to implement this successfully?"_ — **Yes.** The code is already written,
committed, and verified below; the implementer's job is to **confirm** that
state and then **write the docs section**. Exact file paths, the resolved
visibility fix, the test names, the existing doc cross-links, and the install
instructions to reference are all specified below.

### Documentation & References

```yaml
# MUST READ — the authoritative contract for THIS task
- url: spec/PLATFORMS.md §8.4 "First-run UX on GNOME" (lines ~453-462)
  why: Defines the exact trigger (GNOME session AND name not owned), the
       one-shot-per-launch invariant, the daemon-keeps-running + AT-SPI-interim
       semantics, and the message text.
  critical: "fires at most once per launch" is STRICT (never re-armed), unlike
            linux_tray.rs::NO_MODULE_NOTIFIED which re-arms on state transitions.

- url: spec/PLATFORMS.md §8.5 "Distribution of the extension" (lines ~464-471)
  why: The app does NOT install/load the extension — it only points users to it
       (extensions.gnome.org + GitHub Release .zip). The docs must reflect this.

# ALREADY-LANDED implementation — VERIFY, do not rewrite
- file: src/runners/linux.rs
  why: Contains the COMPLETE first-run notification: GNOME_FIRST_RUN_FIRED
        (static AtomicBool), gnome_session(), consume_gnome_hint_shot(),
        maybe_gnome_first_run_notify(verbose), the call site at the top of
        start() (before create_monitor — covers both Ok/Err branches), and the
        8 unit tests under #[cfg(test)] mod tests.
  pattern: read maybe_gnome_first_run_notify (~lines 179-207); note the
           #[cfg(not(feature="gnome"))] early-return (GOTCHA-3) and the
           #[cfg(feature="gnome")] probe+notify block (GOTCHA-4).
  gotcha: This file COMPILES and its 8 tests PASS as committed. Do NOT revert or
          rewrite it. If you must touch it, only an OPTIONAL message-text tweak
          is permitted (see Task 3) — never edit src/platforms/mod.rs.

- file: src/platforms/mod.rs (line 25)
  why: The Option A visibility fix that unblocked attempt 1 is ALREADY HERE:
        `pub(crate) mod gnome;` under `#[cfg(all(target_os="linux", feature="gnome"))]`.
        The preceding comment (lines 21-23) documents WHY it is pub(crate): the
        Linux runner reaches gnome::probe_available across module branches.
  gotcha: LEAVE THIS AS-IS. Attempt 1's E0603 is resolved by this line. Editing
          it is neither needed nor permitted.

- file: src/platforms/gnome.rs (line 304)
  why: S1's `pub(crate) fn probe_available(verbose: bool) -> Result<(), String>` —
        one name_has_owner round-trip; Ok ⇔ name owned ⇔ extension installed+enabled.
        Reused verbatim by the runner (GOTCHA-4). Do NOT duplicate this probe.

- file: src/platforms/mod.rs (line 263)
  why: `pub fn notify(title: &str, body: &str)` — the existing notify-send
        shell-out (app-name=QMKonnect, icon=input-keyboard), fire-and-forget,
        swallows failure. Reused verbatim. Do NOT reimplement.

# PRIMARY NEW-WORK target doc
- file: docs/qmk-integration.md
  why: The doc the notification points at ("see the QMKonnect docs"). It
        currently covers ONLY firmware integration + host-side rules.toml and has
        ZERO GNOME coverage (confirmed: grep -i gnome|extension returns nothing).
  pattern: Follow the doc's existing Jekyll cross-link style:
           `[…]({{ site.baseurl }}/installation)`,
           `[…]({{ site.baseurl }}/troubleshooting)`.
  gotcha: Do NOT duplicate the detailed install steps — installation.md and
          troubleshooting.md ALREADY have them (see below). Cross-link instead.

# Existing GNOME coverage to CROSS-LINK (do not duplicate)
- file: docs/installation.md §"GNOME (optional Shell extension)" (~line 191)
  why: Authoritative step-by-step install (extensions.gnome.org OR GitHub Release
        .zip, gnome-extensions install --force, enable in Extensions app / log out
        & back in on Wayland, qmkonnect -v verification). LINK to this.
- file: docs/troubleshooting.md §"Linux (GNOME)" (~line 216)
  why: Authoritative troubleshooting (qmkonnect -v | grep gnome, gnome-extensions
        show, gdbus call to verify the D-Bus name, auto-re-acquire on toggle). It
        ALREADY mentions the one-shot first-run notification. LINK to this.
```

### Current Codebase tree (relevant slice)

```bash
src/
  platforms/
    mod.rs          # line 25: pub(crate) mod gnome;  (RESOLVED — leave as-is)
                    # line 263: pub fn notify(title, body)  (reuse)
    gnome.rs        # line 304: pub(crate) fn probe_available  (S1, reuse)
  runners/
    linux.rs        # DONE: maybe_gnome_first_run_notify + helpers + 8 tests
docs/
  qmk-integration.md   # ← PRIMARY EDIT TARGET: add GNOME Shell Extension section
  installation.md      # already has GNOME install steps (cross-link)
  troubleshooting.md   # already has GNOME troubleshooting (cross-link)
spec/
  PLATFORMS.md     # §8.4 (contract), §8.5 (distribution) — READ-ONLY reference
```

### Desired Codebase tree with files to be added/changed

```bash
docs/qmk-integration.md   # MODIFY: insert new "## GNOME Shell Extension
                          #          (Linux Desktop Prerequisite)" section
# (src/runners/linux.rs and src/platforms/mod.rs are ALREADY correct — no edit
#  required; an OPTIONAL message-text tweak in runners/linux.rs is the only
#  permitted code change — see Task 3.)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (attempt-1 lesson): the E0603 "module `gnome` is private" conflict
// is RESOLVED. src/platforms/mod.rs:25 is `pub(crate) mod gnome;`. Do NOT touch
// mod.rs and do NOT re-introduce a private `mod gnome;`. The committed code
// compiles — `cargo check --features gnome` finished in 0.75s on re-plan.

// GOTCHA-3 (gnome feature absent): maybe_gnome_first_run_notify has a
//   #[cfg(not(feature = "gnome"))] early-return BEFORE the probe — a trayless
//   --no-default-features build must not point users at an extension no client
//   can consume. Preserve this guard.

// GOTCHA-4 (reuse, don't reimplement): the Ok/Err of gnome::probe_available is
//   the single source of truth for "extension present". Ok ⇔ name owned ⇔
//   installed+enabled. Never call notify-send directly here, never re-probe via
//   a second zbus round-trip — reuse probe_available + platforms::notify.

// GOTCHA (one-shot semantics differ from the tray): GNOME_FIRST_RUN_FIRED is
//   fire-once-and-stay (never re-armed), unlike linux_tray.rs::NO_MODULE_NOTIFIED
//   which re-arms on a state transition. PLATFORMS.md §8.4 = "at most once per
//   launch", period.

// GOTCHA (notify-send link rendering): notify-send bodies do NOT render
//   clickable URLs reliably across DEs, so the message says "(see the QMKonnect
//   docs)" rather than a raw URL. THAT is why the docs (qmk-integration.md) must
//   cover the extension — the user reads the message then goes looking in docs.
```

## Implementation Blueprint

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: VERIFY the already-landed notification (NO edit — confirm only)
  - RUN: cargo check --features gnome            # expect: Finished, zero errors
  - RUN: cargo test --bin qmkonnect -- --test-threads=1 \
            gnome_session consume_gnome_hint     # expect: 8 passed, 0 failed
  - INSPECT src/runners/linux.rs: confirm maybe_gnome_first_run_notify exists
            (~line 179), is called near the top of start() BEFORE create_monitor
            (~line 62), and the #[cfg(test)] mod tests block has 8 tests.
  - INSPECT src/platforms/mod.rs:25: confirm `pub(crate) mod gnome;` is present
            (the attempt-1 blocker is GONE).
  - DO NOT edit anything in Task 1. If cargo check or tests FAIL, STOP and report
    — that means the tree regressed and the conflict is back.

Task 2: ADD the "GNOME Shell Extension" section to docs/qmk-integration.md
  - CREATE: a new section `## GNOME Shell Extension (Linux Desktop Prerequisite)`
  - PLACEMENT: insert immediately BEFORE the existing `## Common Issues` heading
            (so the new section sits between "Testing Your Integration" and
            "Common Issues"). This keeps the firmware-flow uninterrupted while
            making the section discoverable to a user who arrives via the
            notification.
  - CONTENT (concise — cross-link, do NOT duplicate installation/troubleshooting):
      a. One short paragraph: GNOME (Mutter) advertises neither Wayland
         foreign-toplevel protocol and exposes no client API for the active
         window, so on GNOME QMKonnect detects windows via the
         `qmkonnect@mulletware` Shell extension. On every other desktop
         (Hyprland, Sway, KDE Plasma 6, COSMIC, …) **no extension is needed**.
      b. First-run notification: on a GNOME session where the extension is not
         installed/enabled, QMKonnect fires a single one-shot desktop
         notification pointing here. The daemon keeps running (tray + device
         status); the AT-SPI backend (PLATFORMS.md §9) may provide best-effort
         window detection meanwhile.
      c. Install: link to the detailed steps —
         `[Installation → GNOME]({{ site.baseurl }}/installation#gnome-optional-shell-extension)`.
         Mention the two sources (extensions.gnome.org search "qmkonnect", OR the
         `qmkonnect@mulletware.shell-extension.zip` from GitHub Releases) in one
         sentence; do NOT copy the `gnome-extensions install --force` block.
      d. Verify: one line — `qmkonnect -v` should report 'gnome' available,
         selected (link to troubleshooting for the full grep).
      e. Troubleshoot: link to
         `[Troubleshooting → Linux (GNOME)]({{ site.baseurl }}/troubleshooting#linux-gnome)`.
      f. Spec reference: "See `spec/PLATFORMS.md` §8 for the authoritative spec."
  - NAMING/STYLE: match the doc's existing tone and the Jekyll `{{ site.baseurl }}`
            link convention used elsewhere in the file (e.g. the Next Steps list).
  - DO NOT: duplicate the `gnome-extensions install --force …` block, the gdbus
            verification snippet, or the `gnome-extensions show` table — those
            live in installation.md / troubleshooting.md. This section is a
            signpost + rationale, not a second copy.

Task 3 (OPTIONAL — only if you judge the message imprecise): align notification text
  - IF AND ONLY IF you want the body to point more precisely, you MAY edit ONLY
    src/runners/linux.rs maybe_gnome_first_run_notify's platforms::notify body
    string to read e.g. "(see docs → QMK Integration → GNOME Shell Extension)".
  - DO NOT change the title, the trigger logic, the AtomicBool guard, or the
    cfg(feature="gnome") structure.
  - DO NOT edit src/platforms/mod.rs or src/platforms/gnome.rs under any
    circumstances — they are correct and committed.
  - If in doubt, SKIP Task 3 entirely (the current message already satisfies the
    contract). Skipping is the safe default.
```

### Implementation Patterns & Key Details

```rust
// The committed maybe_gnome_first_run_notify (src/runners/linux.rs) — REFERENCE.
// It is DONE. This is shown so the implementer recognizes correct code and
// does NOT rewrite it. The load-bearing properties:
fn maybe_gnome_first_run_notify(verbose: bool) {
    if !consume_gnome_hint_shot(&GNOME_FIRST_RUN_FIRED) { return; } // once/launch
    #[cfg(not(feature = "gnome"))] { let _ = verbose; return; }     // GOTCHA-3
    #[cfg(feature = "gnome")] {
        if crate::platforms::gnome::probe_available(false).is_ok() {
            return; // extension present — nothing to hint (Success Criterion #2)
        }
        if verbose { println!("GNOME session without the Shell extension — firing one-shot hint"); }
        crate::platforms::notify(
            "QMKonnect needs the GNOME Shell extension",
            "Window detection needs the QMKonnect GNOME Shell extension — install it \
             from extensions.gnome.org (see the QMKonnect docs).",
        );
    }
}
// Call site (start(), before create_monitor): maybe_gnome_first_run_notify(self.verbose);
```

### Integration Points

```yaml
CODE: none required (already wired). The notification is called once at the top
      of runners/linux.rs::start(), before backend selection, so it covers BOTH
      the create_monitor Ok and Err branches (Success Criterion + the
      "daemon keeps running" invariant).

DOCS:
  - modify: docs/qmk-integration.md   (PRIMARY deliverable — Task 2)
  - cross-link targets (already exist, do not edit):
      - docs/installation.md §"GNOME (optional Shell extension)"
      - docs/troubleshooting.md §"Linux (GNOME)"

NO database, NO config schema, NO Cargo.toml, NO CI, NO packaging changes.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
# Rust: the only code change permitted is the OPTIONAL Task 3 string tweak.
# Markdown: validate the new section's headings/links.
cargo check --features gnome                      # expect: Finished, 0 errors
cargo fmt --check                                 # expect: no diff (skip if Task 3 untouched)
# Markdown lint (if mdformat available; else visual check of heading nesting):
grep -n '^#' docs/qmk-integration.md              # expect a clean ## GNOME Shell Extension … heading
```

### Level 2: Unit Tests (Component Validation)

```bash
# The 8 GNOME one-shot tests MUST still pass (they encode the contract):
cargo test --bin qmkonnect -- --test-threads=1 gnome_session consume_gnome_hint
# expect: test result: ok. 8 passed; 0 failed

# Full single-threaded suite (AGENTS.md contract — shared debouncer state):
cargo test --bin qmkonnect -- --test-threads=1
# expect: all green; no regressions from any optional Task 3 tweak
```

### Level 3: Docs & Scope Validation (the real gate for THIS task)

```bash
# The new section exists and is well-formed:
grep -ni 'GNOME Shell Extension\|qmkonnect@mulletware\|extensions.gnome.org' docs/qmk-integration.md
# expect: at least the section heading + the UUID + the install source all present

# It cross-links (does not duplicate) the authoritative pages:
grep -n 'site.baseurl.*(installation\|site.baseurl.*(troubleshooting' docs/qmk-integration.md
# expect: the two new cross-links present

# It does NOT duplicate the detailed shell commands (those live elsewhere):
grep -c 'gnome-extensions install --force' docs/qmk-integration.md   # expect: 0

# Scope guard — confirms the attempt-1 conflict is NOT re-introduced and the
# only files changed are docs (+ optionally the one runner string):
git diff --stat
# expect (Task 3 skipped):   docs/qmk-integration.md   only
# expect (Task 3 applied):   docs/qmk-integration.md + src/runners/linux.rs  ONLY
# MUST NOT contain: src/platforms/mod.rs, src/platforms/gnome.rs, Cargo.toml,
#                   any packaging/, any release.yml
git diff --name-only | grep -E 'src/platforms/(mod|gnome)\.rs|Cargo.toml' && \
  { echo "VIOLATION: forbidden file touched"; exit 1; } || echo "scope OK"

# Confirm the visibility fix that unblocked attempt 1 is intact:
grep -n 'pub(crate) mod gnome;' src/platforms/mod.rs   # expect: one match, line ~25
```

### Level 4: Creative & Domain-Specific Validation

```bash
# Manual end-to-end on a real GNOME box (best-effort; skip if unavailable):
#   1. Ensure the extension is NOT enabled: gnome-extensions disable qmkonnect@mulletware
#   2. XDG_CURRENT_DESKTOP=GNOME qmkonnect -v
#   3. Expect exactly ONE desktop notification on launch; expect the log line
#      "GNOME session without the Shell extension — firing one-shot hint".
#   4. Restart qmkonnect: the notification fires AGAIN (once-per-launch, not
#      once-per-install — this is correct per PLATFORMS.md §8.4).
#   5. Enable the extension + restart: NO notification (probe_available → Ok).
#   6. Click through the docs link path a user would follow: open qmk-integration.md,
#      find the new GNOME section, follow the installation cross-link — it must
#      land on the GNOME install steps.
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo check --features gnome` clean (Level 1).
- [ ] All 8 GNOME one-shot unit tests pass (Level 2); full single-threaded suite green.
- [ ] `git diff --stat` shows ONLY `docs/qmk-integration.md` (+ optionally
      `src/runners/linux.rs` for the Task 3 string). **Never** `src/platforms/mod.rs`
      or `src/platforms/gnome.rs`.

### Feature Validation
- [ ] `src/platforms/mod.rs:25` still reads `pub(crate) mod gnome;` (attempt-1
      blocker stays resolved).
- [ ] `maybe_gnome_first_run_notify` still called once in `start()` before
      `create_monitor` (not removed/rewritten).
- [ ] `docs/qmk-integration.md` has the new GNOME Shell Extension section with
      the UUID `qmkonnect@mulletware`, extensions.gnome.org source, the
      first-run-notification + AT-SPI-interim note, and cross-links to
      `installation` + `troubleshooting`.
- [ ] The detailed `gnome-extensions install --force` block is NOT duplicated in
      qmk-integration.md (cross-linked instead).

### Documentation & Deployment
- [ ] New section uses the doc's existing Jekyll `{{ site.baseurl }}` link style.
- [ ] No new env vars, config keys, CI jobs, or packaging artifacts.

---

## Anti-Patterns to Avoid

- ❌ **Do NOT re-open the attempt-1 conflict.** `src/platforms/mod.rs` already has
  `pub(crate) mod gnome;`. Do not revert it, do not re-privatize it, do not edit
  it at all.
- ❌ Do NOT rewrite `maybe_gnome_first_run_notify` or its helpers/tests — they are
  committed, compile, and pass. Verify them; do not replace them.
- ❌ Do NOT reimplement the `name_has_owner` zbus probe inline in the runner or
  the docs. Reuse `gnome::probe_available`.
- ❌ Do NOT duplicate the GNOME install/troubleshooting steps into
  qmk-integration.md. Cross-link `installation` + `troubleshooting`.
- ❌ Do NOT change the one-shot semantics to re-arm on backend state transitions.
  PLATFORMS.md §8.4 is strictly "at most once per launch".
- ❌ Do NOT skip a validation level because "the code already works." The Level 3
  scope + docs gates are the actual deliverable for this re-plan.

---

## Confidence Score: 9.5 / 10

**Why high**: the code half of this task is *already implemented, committed,
compiling, and passing its 8 tests* — attempt 1's only blocker (the private
`gnome` module) is resolved by the landed `pub(crate) mod gnome;`. The remaining
work is a single, well-specified docs section with explicit content, placement,
and cross-link targets, guarded by mechanical grep gates.

**The 0.5 residual risk**: the optional Task 3 string tweak. If the implementer
unnecessarily edits `runners/linux.rs` and introduces a typo, the Level 2 tests
catch it; if they erroneously touch `src/platforms/mod.rs`, the Level 3 scope
guard catches it. Both are detectable, hence the high score. Defaulting to
**skipping Task 3** removes even that residual.