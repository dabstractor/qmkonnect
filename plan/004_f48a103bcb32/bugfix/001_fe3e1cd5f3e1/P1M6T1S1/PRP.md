# PRP — P1.M6.T1.S1: Update README.md and docs/troubleshooting.md for the P1.M1–M1.M5 changeset

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`) — Rust menu-bar/tray daemon.
> **Files touched (2, both DOCS, zero source):** `docs/llms_full.txt` (REGENERATE) +
> `REMAINING_ISSUES.md` (mark 5 items resolved).
> **Files VERIFIED but NOT edited (3):** `README.md`, `docs/troubleshooting.md`,
> `docs/configuration.md` — all already accurate (research §docs_audit.md).
>
> **Scope:** Mode-B documentation catch-all (per the work-item contract). Sync the
> repo's documentation with the P1.M1–P1.M5 remediation changeset. The ONE user-visible
> behavior change in the whole changeset is P1.M4.T1.S2: Windows `rules.toml`-invalid
> notifications are now auto-dismissing WinRT **toasts** (Action Center) instead of
> focus-stealing modal `MessageBoxW` dialogs. M1 (debounce panic), M2 (atomic writes),
> M3 (device lifecycle), M5 (config cache) are all internal with no user-facing surface.
>
> **Source of truth for this design:** `research/docs_audit.md` (git-verified audit of
> every doc file: which are stale, which are already correct, exact line numbers, and
> the code evidence that the 5 REMAINING_ISSUES items are genuinely fixed).

---

## Goal

**Feature Goal**: Bring the documentation into sync with the post-changeset reality.
Concretely: (1) regenerate the stale checked-in generated doc `docs/llms_full.txt` so it
reflects the toast paragraph now in `docs/troubleshooting.md`; (2) mark the 5
REMAINING_ISSUES.md items that the bug-hunt validated as fixed (#4, #5, #7, #13, #14)
as resolved, so the tracking doc stops claiming they're open; (3) verify — and deliberately
leave untouched — the three doc files that are already accurate.

**Deliverable** (exactly two edited files, zero source):
1. `docs/llms_full.txt` — regenerated via `bash docs/generate_llms_full.sh`; the toast
   paragraph now appears in the rules.toml-parse-error section (was missing).
2. `REMAINING_ISSUES.md` — a one-line `> ✅ Resolved.` note inserted under each of the
   headings for items #4, #5, #7, #13, #14 (original audit text kept intact for history).

**Success Definition**:
- `git diff --stat` shows **exactly two files**: `docs/llms_full.txt` and `REMAINING_ISSUES.md`.
  ZERO source files (`src/**`), ZERO `Cargo.toml`, ZERO `packaging/**`.
- `grep -n "toast" docs/llms_full.txt` returns ≥1 hit (was 0 before regen) — the toast
  paragraph from troubleshooting.md is now present.
- `REMAINING_ISSUES.md` items #4/#5/#7/#13/#14 each carry a `✅ Resolved.` marker backed
  by a code-evidence one-liner; items NOT in the validated-fixed set (#1, #2, #3, #6,
  #8–#12, #15–#25) are **unchanged**.
- `README.md`, `docs/troubleshooting.md`, `docs/configuration.md` are **byte-identical**
  to HEAD (`git diff --name-only` lists none of them) — they were already correct.
- No source touched → `cargo test --bin qmkonnect -- --test-threads=1` passes unchanged
  (sanity gate only; this task cannot affect it).

## User Persona (if applicable)

**Target User**: a future reader of the docs — an end user consulting
`docs/llms_full.txt` (the agent/LLM-oriented combined doc) or a maintainer consulting
`REMAINING_ISSUES.md` (the open-issues tracker). Both currently read stale information
(llms_full.txt omits the toast note; REMAINING_ISSUES.md claims 5 fixed bugs are open).
**Use Case**: a Windows user hits a broken `rules.toml`, sees a toast, and looks it up
in the docs to understand the behavior — the docs must match what they saw. A maintainer
triaging REMAINING_ISSUES.md must not re-investigate 5 already-fixed items.
**Pain Points Addressed**: stale/distracting docs that contradict the shipped behavior.

## Why

- **Generated docs drift silently.** `docs/llms_full.txt` is checked-in (NOT gitignored)
  and is the canonical single-file reference for agents/LLMs. Its generator script says
  "Run after editing README.md or any docs/*.md" — but the last regen (`2e8f706`) predates
  the toast paragraph (`17e4f6f`). The combined doc now contradicts the source doc.
- **A stale issue tracker wastes future effort.** h2.2 of the bug-hunt explicitly
  validated #4/#5/#7/#13/#14 as fixed, and the code confirms it (research §Finding 5),
  but REMAINING_ISSUES.md still lists them under "Critical"/"Correctness". Leaving them
  makes the tracker misleading; the contract (point d) directs attention to exactly these.
- **Minimalism is correctness here.** The three user-facing docs (README, troubleshooting,
  configuration) are already accurate — editing them would only introduce risk. This PRP's
  discipline is: change exactly what's stale, verify (don't touch) what's correct.

## What

### Approach: regenerate the generated artifact + annotate the tracker; verify the rest

- **`docs/llms_full.txt`**: it is GENERATED from `README.md` + `docs/*.md`. **Never
  hand-edit it** — the generator is canonical (script comment: "Regenerate … Run after
  editing … any docs/*.md"). Run the script; review the diff.
- **`REMAINING_ISSUES.md`**: a hand-written tracker. Mark the 5 validated-fixed items
  resolved with a one-line `> ✅ Resolved.` blockquote under each heading, citing code
  evidence. Keep the original audit prose intact (history). Do NOT renumber, restructure,
  or create new sections (contract: "err on the side of minimal, accurate updates").
- **README.md / docs/troubleshooting.md / docs/configuration.md**: read-only VERIFY.
  The contract explicitly says README needs no change if notifications aren't mentioned
  (they aren't); troubleshooting already has the toast paragraph; configuration.md
  documents only the (unchanged) file format.

### Success Criteria

- [ ] `docs/llms_full.txt` regenerated; `grep -n "toast" docs/llms_full.txt` ≥1 hit.
- [ ] `REMAINING_ISSUES.md` items #4, #5, #7, #13, #14 each have a `✅ Resolved.` note;
      items not in that set are unchanged.
- [ ] `README.md`, `docs/troubleshooting.md`, `docs/configuration.md` unchanged (verify
      via `git diff --name-only`).
- [ ] `git diff --stat` = exactly `docs/llms_full.txt` + `REMAINING_ISSUES.md`.
- [ ] The implementing agent re-ran the 5 code-verification greps (research §Finding 5)
      before marking each REMAINING_ISSUES item (guards against a regression landing).

## All Needed Context

### Context Completeness Check

_Pass._ An agent with no prior knowledge can complete this from: the audit table in
`research/docs_audit.md` (which file is stale, which is correct, exact line numbers, and
the code evidence for each of the 5 resolved items), the verbatim `✅ Resolved.` note
text (given verbatim in Tasks 3–7), the exact regeneration command + verification greps
(Level 1/2), and the explicit "do not touch" list (Task 8). No judgment calls remain:
every edit is pinned to a specific heading/section with before/after text.

### Documentation & References

```yaml
# MUST READ — the git-verified audit this PRP is built on (verbatim findings, line numbers, code evidence)
- docfile: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/P1M6T1S1/research/docs_audit.md
  why: "the authoritative record of which docs are stale vs correct. §Finding 1 = troubleshooting.md
        ALREADY correct (no edit). §Finding 2 = llms_full.txt STALE (regenerate). §Finding 3 = README
        no change. §Finding 4 = configuration.md no change. §Finding 5 = the 5 REMAINING_ISSUES items
        with the exact code-evidence one-liner for each. §Finding 6 = no other stale refs."
  section: "all — every finding is a deliverable or a verify-only gate"
  critical: "the only two files to EDIT are docs/llms_full.txt and REMAINING_ISSUES.md. The other three
        are VERIFY-ONLY (git diff must not list them). Do not be tempted to 'improve' accurate docs."

# MUST READ — the bug-hunt report (the changeset's reason for being + the h2.2 validation of #4/#5/#7/#13/#14)
- docfile: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/prd_snapshot.md
  why: "h2.0 = the critical debounce panic (M1). h2.1 = the 5 lower-severity findings the changeset
        addresses (M2=#1 atomic writes, M5=#2 re-read cache, M4=#3 Windows toast, M3=#5 device race).
        h2.2 = the validation that #4/#5/#7/#13/#14 + config VID/PID preservation are 'all fixed' —
        this is the authority for crossing them off in REMAINING_ISSUES.md."
  section: "h2.2 ('✅ Validated as correct / already fixed')"
  critical: "h2.2's list of fixed items (#4 udev, #5 static mut, #7 Hyprland backoff, #13 macOS screen
        recording, #14 X11 stub, + config VID/PID preservation) is EXACTLY the set to mark resolved.
        Do not mark anything else resolved (#1,#2,#3,#6,#8–#12,#15–#25 stay open)."

# MUST READ — the file being regenerated (understand WHY regeneration is the only correct edit)
- file: docs/generate_llms_full.sh
  why: "establishes llms_full.txt is GENERATED (concatenates README.md + docs/*.md, strips Jekyll
        front-matter), is the canonical source for the combined doc, and its header says 'Run after
        editing README.md or any docs/*.md'. Hand-editing the output would be silently overwritten
        on the next regen — so REGENERATE, never hand-edit."
  pattern: "bash docs/generate_llms_full.sh && git diff --stat docs/llms_full.txt"
  gotcha: "the script is idempotent (re-running with no source change produces no diff). Run it from
        the repo root. It writes to docs/llms_full.txt in place."

# REFERENCE — the file being edited (REMAINING_ISSUES.md): structure + the 5 headings to annotate
- file: REMAINING_ISSUES.md
  why: "hand-written tracker. Heading lines (verified): #4 at line 21, #5 at 31, #7 at 40, #13 at 64,
        #14 at 67. Each `✅ Resolved.` note goes on a new line directly UNDER the heading, before the
        existing audit prose (which is kept verbatim for history). The file has NO existing resolved/
        strikethrough markers (verified) — this PRP introduces the convention."
  pattern: "under each heading insert a blank line + `> ✅ **Resolved.** <one-line evidence>`. Keep the
        original body intact below it."
  gotcha: "do NOT touch items #1,#2,#3 (Critical, still open), #6 (pre-existing test failures — separate),
        #8–#12, #15–#25. Only #4,#5,#7,#13,#14 are validated fixed (h2.2). Do NOT renumber headings."

# REFERENCE — the source doc the regen pulls from (already correct; read to confirm the toast paragraph)
- file: docs/troubleshooting.md
  why: "lines 533–543 hold the toast paragraph (committed in 17e4f6f). Confirm it is present and accurate
        BEFORE regenerating llms_full.txt — regen can only propagate what's in the source. The paragraph
        reads: 'On Windows this is a toast that auto-dismisses … Action Center (it is no longer a modal
        dialog you must click away) …'. This is the exact text that must appear in llms_full.txt after regen."
  gotcha: "do NOT edit troubleshooting.md. It is already correct (P1.M4.T1.S2 wrote this paragraph).
        Editing it would be out of scope and could desync the Jekyll-rendered site."

# REFERENCE — the code that backs each resolved REMAINING_ISSUES item (the evidence you will cite + re-verify)
- file: packaging/linux/udev/69-qmkonnect-rawhid.rules
  why: "evidence for #4: the static rule (single line, ENV{ID_QMKONNECT}==\"1\"-guarded, MODE=\"0660\"
        + TAG+=\"uaccess\"). Re-grep before marking #4 resolved."
- file: src/platforms/windows.rs
  why: "evidence for #5: line 22 comment 'Thread-safe replacements for the former `static mut` globals
        (issue #5)'. `grep -rn 'static mut' src/platforms/windows.rs src/platforms/macos.rs` must return
        ONLY comments (no real `static mut` declaration). Re-verify before marking #5."
- file: src/platforms/hyprland.rs
  why: "evidence for #7: line 26 'on its loss the backoff is reset to the initial value (#7)'; lines
        198–202 `delay_ms = INITIAL_RECONNECT_MS`. Re-verify before marking #7."
- file: src/platforms/macos.rs
  why: "evidence for #13: lines 85–101 `ensure_screen_recording_permission()` — does NOT block, runs
        app-name-only + redacts titles until granted (graceful degradation). Re-verify before marking #13."
- file: src/platforms/x11.rs
  why: "evidence for #14: lines 25–53 real `xprop` (`_NET_ACTIVE_WINDOW` → `WM_CLASS`/`_NET_WM_NAME`),
        comment cites 'issue #14'. Re-verify before marking #14."
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
README.md                       # VERIFY only — features list has NO notification mention (research §Finding 3)
docs/
  troubleshooting.md            # VERIFY only — toast paragraph already present at lines 533-543 (§Finding 1)
  configuration.md              # VERIFY only — documents (unchanged) file format only (§Finding 4)
  llms_full.txt                 # EDIT — REGENERATE via generate_llms_full.sh (stale; §Finding 2)
  generate_llms_full.sh         # the generator (read-only; run it, don't edit it)
REMAINING_ISSUES.md             # EDIT — mark #4/#5/#7/#13/#14 resolved (§Finding 5)
  :21  ### 4. udev update path is fragile and insecure
  :31  ### 5. `static mut` data races (UB)
  :40  ### 7. Hyprland reconnect backoff never resets
  :64  ### 13. macOS screen-recording permission handling
  :67  ### 14. X11 monitor is a stub sending garbage
# ZERO source files touched (src/**, Cargo.toml, packaging/** all unchanged)
```

### Desired Codebase tree with files added/changed

```bash
docs/llms_full.txt              # regenerated (1 generated file updated in place)
REMAINING_ISSUES.md             # +5 one-line "✅ Resolved." blockquotes under headings #4,#5,#7,#13,#14
# (no new files; no source; no packaging; no Cargo)
```

### Known Gotchas of our codebase & Library Quirks

```text
# CRITICAL (llms_full.txt is GENERATED — never hand-edit): docs/generate_llms_full.sh concatenates
#   README.md + docs/*.md and strips Jekyll front-matter. Hand-edits are silently overwritten on the
#   next regen. The ONLY correct way to update it is `bash docs/generate_llms_full.sh`. Its script
#   header literally says "Run after editing README.md or any docs/*.md".

# CRITICAL (llms_full.txt is checked-in, NOT gitignored): verified `grep llms_full docs/.gitignore
#   .gitignore` → nothing. So regenerating produces a real, committable diff. (If it WERE gitignored,
#   regen would be a local-only step and there'd be nothing to commit — but it is not.)

# CRITICAL (only 5 REMAINING_ISSUES items are validated fixed): h2.2's "all fixed" list = #4 (udev),
#   #5 (static mut), #7 (Hyprland backoff), #13 (macOS screen recording), #14 (X11 stub), PLUS "config
#   field preservation across VID/PID saves" (which is NOT a numbered REMAINING_ISSUES.md item — it's a
#   passim note; leave it). Do NOT mark #1/#2/#3 (Critical, open), #6 (test failures), or any of
#   #8–#12/#15–#25. Marking an unfixed item resolved would make the tracker worse, not better.

# CRITICAL (re-verify each item in code before marking it): a future regression could re-introduce a
#   `static mut`, undo the udev fix, etc. The PRP's verification greps (Tasks 3–7) MUST pass before the
#   ✅ note is added. If a grep fails for an item, DO NOT mark that item resolved — leave it open and
#   note the discrepancy (the h2.2 validation may no longer hold).

# GOTCHA (troubleshooting.md / README.md / configuration.md are already correct — do NOT edit): editing
#   accurate docs only introduces risk. The contract explicitly says README needs no change if
#   notifications aren't mentioned (they aren't). troubleshooting.md's toast paragraph was written by
#   P1.M4.T1.S2. configuration.md documents only the (unchanged) file format. `git diff --name-only`
#   must NOT list any of these three.

# GOTCHA (troubleshooting.md and the docs/*.md site files are Jekyll-rendered): they carry YAML
#   front-matter (`---\nlayout: default\n…\n---`) and Liquid (`{{ site.baseurl }}`). Do not strip or
#   alter front-matter — the generate_llms_full.sh script strips it for the combined doc, but the source
#   files must keep it for the GitHub Pages site. (You are not editing these files anyway.)

# GOTCHA (the generate script is idempotent): re-running it with no source change since the last regen
#   yields no diff. So if `git diff docs/llms_full.txt` is empty after running it, either you're already
#   up to date (check `grep toast docs/llms_full.txt`) or the script didn't run (check exit code / cwd).

# GOTCHA (run the script from the repo root, or it self-locates): generate_llms_full.sh uses
#   BASH_SOURCE to find its own dir, so cwd doesn't strictly matter, but run from repo root for clarity.

# GOTCHA (no source change ⇒ cargo test is a no-op sanity gate): this task edits ZERO .rs files, so the
#   test suite cannot regress from it. `cargo test --bin qmkonnect -- --test-threads=1` is included only
#   as a belt-and-suspenders confirmation that you didn't accidentally touch source (AGENTS.md mandates
#   single-threaded tests for this repo due to shared global debouncer state).
```

## Implementation Blueprint

### Data models and structure
None. This is a pure-documentation task — no structs, no config, no API. The only
"structure" is two markdown edits (a regenerated file + 5 one-line blockquote notes).

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: VERIFY the source doc is correct BEFORE regenerating (no edit)
  - Read docs/troubleshooting.md lines ~528-545 and confirm the toast paragraph is present and accurate:
      "On Windows this is a toast that auto-dismisses after a few seconds and lands in Action Center
       (it is no longer a modal dialog you must click away); Linux uses `notify-send` and macOS uses a
       Notification Center alert. (On Windows the toast requires the installed Start Menu shortcut …)"
  - If present + accurate (it is, per research §Finding 1): proceed. regen will propagate it.
  - If somehow MISSING or wrong: STOP — that means troubleshooting.md itself needs fixing first, which
    is out of scope for THIS task (it was P1.M4.T1.S2's job and is committed). Do NOT invent a new
    paragraph; surface the discrepancy instead.
  - WHY FIRST: generate_llms_full.sh can only propagate what's in the source. Verifying the source
    first makes the regen diff predictable (exactly the toast paragraph).

Task 2: EDIT docs/llms_full.txt — REGENERATE (the primary deliverable)
  - Run, from the repo root:
      bash docs/generate_llms_full.sh
  - VERIFY the regen landed the toast paragraph (it was absent before — research §Finding 2):
      grep -n "toast" docs/llms_full.txt
      # Expected BEFORE: no hits. AFTER: ≥1 hit inside the rules.toml-parse-error section.
  - VERIFY the diff is scoped (only the source-doc change since the last regen should appear):
      git diff --stat docs/llms_full.txt
      # Expected: a small insertion (the ~10-line toast paragraph + any formatting) in the
      # rules.toml-parse-error section. Since 2e8f706 the ONLY docs/README change was troubleshooting.md
      # (+10 lines, the toast paragraph), so the llms_full.txt diff should be correspondingly small.
  - WHY: llms_full.txt is GENERATED and checked-in; regen is the only correct way to update it.
  - DO NOT hand-edit docs/llms_full.txt. The next regen would overwrite it.

Task 3: EDIT REMAINING_ISSUES.md — mark item #4 resolved (heading at line 21)
  STEP 3a — RE-VERIFY the code evidence first (guards against regression):
      test -f packaging/linux/udev/69-qmkonnect-rawhid.rules && \
      grep -n 'MODE="0660"\|ENV{ID_QMKONNECT}' packaging/linux/udev/69-qmkonnect-rawhid.rules && \
      ! grep -n 'MODE="0666"' src/platforms/linux.rs
      # Expected: the static rule file exists, is 0660 + ID_QMKONNECT-guarded, and linux.rs has no 0666.
  STEP 3b — if (and only if) 3a passes, insert DIRECTLY UNDER the `### 4. …` heading (line 21), before
  the existing `` `src/platforms/linux.rs::update_udev_rules`: `` body:
      > ✅ **Resolved.** The fragile `/tmp` + `sudo mv` + `MODE=0666` path is gone: device access now
      > goes through the static `packaging/linux/udev/69-qmkonnect-rawhid.rules` (single line,
      > `ENV{ID_QMKONNECT}=="1"`-guarded, `MODE="0660"` + `TAG+="uaccess"` — no `/tmp` race, no
      > world-writable node), and `-r`/`--reload` renders the same safe form and auto-repairs the
      > dangerous legacy rule. *(Bug-hunt h2.2; re-verified: static rule present, no `MODE=0666`.)*
  - KEEP the original audit body intact below the note (history).
  - IF 3a FAILS: do NOT mark #4 resolved — leave it open and note the discrepancy in your summary.

Task 4: EDIT REMAINING_ISSUES.md — mark item #5 resolved (heading at line 31)
  STEP 4a — RE-VERIFY:
      grep -rn "static mut" src/platforms/windows.rs src/platforms/macos.rs
      # Expected: ONLY comment lines (e.g. windows.rs:22 / macos.rs:43 "former `static mut` …").
      # No real `static mut` declaration should appear.
  STEP 4b — if 4a passes, insert under `### 5. …`:
      > ✅ **Resolved.** The `static mut` globals are replaced with atomics / `OnceLock` / `Mutex`
      > (`src/platforms/windows.rs:22`, `src/platforms/macos.rs:43` reference "the former `static mut`").
      > *(Bug-hunt h2.2; re-verified: `grep -rn "static mut" src/platforms/{windows,macos}.rs` returns
      > only comments.)*

Task 5: EDIT REMAINING_ISSUES.md — mark item #7 resolved (heading at line 40)
  STEP 5a — RE-VERIFY:
      grep -n "delay_ms = INITIAL_RECONNECT_MS\|backoff is reset" src/platforms/hyprland.rs
      # Expected: the reset at ~line 202 + the doc comment at ~line 26.
  STEP 5b — if 5a passes, insert under `### 7. …`:
      > ✅ **Resolved.** The backoff now resets to the initial value after a connection stays up a while,
      > so long-uptime sessions no longer get stuck at the 10s cap (`src/platforms/hyprland.rs:198-202`,
      > doc comment cites "#7"). *(Bug-hunt h2.2.)*

Task 6: EDIT REMAINING_ISSUES.md — mark item #13 resolved (heading at line 64)
  STEP 6a — RE-VERIFY:
      grep -n "ensure_screen_recording_permission\|redact\|app name" src/platforms/macos.rs
      # Expected: the non-blocking permission helper (~line 90) that redacts titles until granted.
  STEP 6b — if 6a passes, insert under `### 13. …`:
      > ✅ **Resolved.** `ensure_screen_recording_permission()` no longer hard-fails: it runs the app
      > sending the app-name only and redacts window titles until Screen Recording is granted, then picks
      > them up — graceful degradation (`src/platforms/macos.rs:85-101`). *(Bug-hunt h2.2.)*

Task 7: EDIT REMAINING_ISSUES.md — mark item #14 resolved (heading at line 67)
  STEP 7a — RE-VERIFY:
      grep -n "xprop\|_NET_ACTIVE_WINDOW\|WM_CLASS\|_NET_WM_NAME" src/platforms/x11.rs
      # Expected: the real xprop implementation (~lines 25-53), comment cites "issue #14".
  STEP 7b — if 7a passes, insert under `### 14. …`:
      > ✅ **Resolved.** The X11 monitor now shells out to real `xprop` (`_NET_ACTIVE_WINDOW` →
      > `WM_CLASS` / `_NET_WM_NAME`) instead of sending literal stub strings
      > (`src/platforms/x11.rs:25-53`, comment cites "issue #14"). *(Bug-hunt h2.2.)*

Task 8: VERIFY the three accurate docs are untouched (no edit — defensive)
  - Run:
      git diff --name-only README.md docs/troubleshooting.md docs/configuration.md
      # Expected: EMPTY (none of these changed).
  - If any appears: you accidentally edited an already-correct doc. Revert it. The contract is explicit
    that these need no change (README: notifications not mentioned; troubleshooting: toast para already
    correct; configuration: file format unchanged).

Task 9: VALIDATE (no edits)
  - git diff --stat
      # Expected: EXACTLY docs/llms_full.txt + REMAINING_ISSUES.md. Nothing else.
  - git diff --stat -- src/ Cargo.toml packaging/
      # Expected: EMPTY (zero source/packaging/Cargo change).
  - grep -n "toast" docs/llms_full.txt          # ≥1 hit (the regen propagated the paragraph).
  - grep -c "Resolved" REMAINING_ISSUES.md      # 5 (one per marked item).
  - cargo test --bin qmkonnect -- --test-threads=1   # green, unchanged (no source touched; sanity only).

Task 10: NEVER do these (out of scope / forbidden)
  - DO NOT hand-edit docs/llms_full.txt (it is generated; regen only). 
  - DO NOT edit README.md, docs/troubleshooting.md, or docs/configuration.md (all already accurate).
  - DO NOT mark any REMAINING_ISSUES item resolved other than #4, #5, #7, #13, #14 (h2.2's fixed set).
  - DO NOT skip the per-item code re-verification (Tasks 3a–7a) before marking resolved.
  - DO NOT renumber, restructure, or create new sections in REMAINING_ISSUES.md (minimal, accurate updates).
  - DO NOT edit the generate_llms_full.sh script.
  - DO NOT touch any .rs file, Cargo.toml, packaging/**, or the Jekyll front-matter of any docs/*.md.
  - DO NOT edit PRD.md, tasks.json, prd_snapshot.md, or .gitignore.
```

### Implementation Patterns & Key Details

```markdown
<!-- PATTERN: the resolved-note blockquote under a REMAINING_ISSUES heading -->
### N. <original heading text>

> ✅ **Resolved.** <one- or two-sentence summary of the fix> (`<file>:<lines>`). *(Bug-hunt h2.2;
> re-verified: <the grep/test that confirms it>.)

<original audit body, kept verbatim>

<!-- PATTERN: regenerating the combined doc (idempotent, checked-in artifact) -->
bash docs/generate_llms_full.sh && git diff --stat docs/llms_full.txt
# then verify propagation:  grep -n "toast" docs/llms_full.txt   # was 0 hits, now ≥1
```

```text
# WHY "Resolved." blockquotes and not strikethrough/renumbering:
#   - Markdown headings don't strikethrough cleanly; a visible ✅ blockquote is unambiguous.
#   - Keeping the original audit body preserves the historical reasoning (useful if a fix is ever
#     reverted or revisited).
#   - Not renumbering means cross-references to "#4"/"#5"/etc. elsewhere stay valid.

# WHY re-verify in code before each mark:
#   - h2.2 is the bug-hunt's snapshot. A later commit could regress an item. Marking a regressed item
#     "resolved" would hide a live bug. The grep guards cost ~5 seconds and prevent that.

# WHY regen is the only correct llms_full.txt edit:
#   - The file's own generator header says "Run after editing README.md or any docs/*.md". Hand-edits
#     are overwritten on the next regen, so they'd be silently lost and the doc would drift again.
```

### Integration Points

```yaml
GENERATED DOC (docs/llms_full.txt):
  - update via: `bash docs/generate_llms_full.sh` (never hand-edit; checked-in, not gitignored)
  - downstream: consumed by agents/LLMs as the canonical single-file doc; also regenerated by maintainers
TRACKER (REMAINING_ISSUES.md):
  - convention introduced: `> ✅ **Resolved.** …` blockquote under a heading = item closed (no prior
    convention existed; this PRP establishes it minimally)
  - DO NOT propagate this convention to the open items (#1,#2,#3,#6,#8–#12,#15–#25) — they stay as-is
CONSUMES (from the changeset, already landed):
  - P1.M4.T1.S2's toast paragraph in docs/troubleshooting.md (commit 17e4f6f) — regen propagates it
  - h2.2's validation that #4/#5/#7/#13/#14 are fixed — the authority for marking them resolved
PARALLEL / SIBLING (zero conflict):
  - P1.M5.T1.S1 (config cache) edits src/core/* only — no docs. This task edits no source. Merge clean.
  - All P1.M1–M5 implementation is source-only; none touch docs. This task is the sole docs touch.
```

## Validation Loop

### Level 1: Scope hygiene (the most important gate for a docs task)

```bash
cd /home/dustin/projects/qmkonnect
git diff --stat
# Expected: EXACTLY two files — docs/llms_full.txt and REMAINING_ISSUES.md. If ANYTHING else appears
#   (a source file, Cargo.toml, packaging/**, or — critically — README.md / troubleshooting.md /
#   configuration.md) you overstepped scope. Revert the stray change.
git diff --stat -- src/ Cargo.toml packaging/
# Expected: EMPTY. This task touches ZERO source.
git diff --name-only -- README.md docs/troubleshooting.md docs/configuration.md
# Expected: EMPTY. These three were already accurate; editing them is forbidden (Task 8).
```

### Level 2: Content correctness (the edits actually say the right thing)

```bash
cd /home/dustin/projects/qmkonnect
# (a) llms_full.txt now carries the toast paragraph (it was absent before regen):
grep -n "toast" docs/llms_full.txt
# Expected: ≥1 hit, inside the rules.toml-parse-error section. (Before regen this was 0.)

# (b) All 5 resolved markers present, and ONLY those 5:
grep -c "✅ \*\*Resolved" REMAINING_ISSUES.md
# Expected: 5.
grep -n "✅ \*\*Resolved" REMAINING_ISSUES.md
# Expected: 5 lines, under headings #4, #5, #7, #13, #14 (verify the preceding ### heading each).

# (c) No open item accidentally marked / no heading renumbered:
grep -n "^### " REMAINING_ISSUES.md | head -30
# Expected: headings 1..25 still present and in order; #4/#5/#7/#13/#14 each followed by the ✅ note.
```

### Level 3: Re-verification of the 5 resolved items (defense against regression)

```bash
cd /home/dustin/projects/qmkonnect
# Each command must succeed (evidence the item is STILL fixed). If any fails, un-mark that item.
test -f packaging/linux/udev/69-qmkonnect-rawhid.rules && grep -q 'MODE="0660"' packaging/linux/udev/69-qmkonnect-rawhid.rules && echo "#4 OK"
! grep -rq 'MODE="0666"' src/platforms/linux.rs && echo "#4 no-0666 OK"
grep -rn "static mut" src/platforms/windows.rs src/platforms/macos.rs | grep -v 'former `static mut`' | grep -v '^.*:#' ; echo "#5: above should be empty (only comments allowed)"
grep -q "delay_ms = INITIAL_RECONNECT_MS" src/platforms/hyprland.rs && echo "#7 OK"
grep -q "ensure_screen_recording_permission" src/platforms/macos.rs && echo "#13 OK"
grep -q "_NET_ACTIVE_WINDOW" src/platforms/x11.rs && echo "#14 OK"
# Expected: each echoes its "OK". If any item is NOT OK, its ✅ note must be removed (leave it open).
```

### Level 4: No-source sanity (belt-and-suspenders; this task cannot affect tests)

```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: green, IDENTICAL pass count to before this task (no source was touched). This is purely a
#   confirmation that you didn't accidentally edit a .rs file. AGENTS.md mandates single-threaded tests
#   for this repo (shared global debouncer state). If a test fails, you touched source — find and revert it.
```

## Final Validation Checklist

### Technical Validation
- [ ] Level 1: `git diff --stat` = exactly `docs/llms_full.txt` + `REMAINING_ISSUES.md`; zero source.
- [ ] Level 2: `grep -n toast docs/llms_full.txt` ≥1 hit; `grep -c "✅ \*\*Resolved" REMAINING_ISSUES.md` = 5.
- [ ] Level 3: all 5 re-verification greps pass (each item is still fixed in code).
- [ ] Level 4: `cargo test --bin qmkonnect -- --test-threads=1` green, unchanged.

### Feature (Docs) Validation
- [ ] `docs/llms_full.txt` regenerated (not hand-edited); toast paragraph now present.
- [ ] `REMAINING_ISSUES.md` items #4, #5, #7, #13, #14 marked resolved with code evidence; originals kept.
- [ ] No open item (#1,#2,#3,#6,#8–#12,#15–#25) was marked resolved or renumbered.
- [ ] `README.md` / `docs/troubleshooting.md` / `docs/configuration.md` are byte-identical to HEAD.
- [ ] The changeset's one user-visible change (Windows toast) is accurately reflected in BOTH the source
      doc (troubleshooting.md — already done) and the generated combined doc (llms_full.txt — this task).

### Code Quality Validation
- [ ] Followed the repo's convention: regenerated artifacts via their generator (never hand-edited).
- [ ] Introduced a minimal, consistent `✅ Resolved.` convention without restructuring the tracker.
- [ ] No new dependencies, no Cargo.toml change, no packaging change, no source change.
- [ ] Resolved notes are backed by re-verified code evidence (not just the h2.2 assertion).

### Documentation
- [ ] The generated combined doc no longer contradicts the source doc (toast note consistent).
- [ ] The tracker no longer claims fixed bugs are open.

---

## Anti-Patterns to Avoid

- ❌ Don't hand-edit `docs/llms_full.txt` — it's generated; regen only, or your edit is silently lost on the next regen.
- ❌ Don't "improve" the already-accurate docs (README.md, troubleshooting.md, configuration.md) — editing them only adds risk; the contract explicitly says they need no change.
- ❌ Don't mark a REMAINING_ISSUES item resolved without re-verifying it in code first — a regression could have re-opened it, and a false "resolved" hides a live bug.
- ❌ Don't mark items outside the h2.2 fixed set (#4,#5,#7,#13,#14) — the others are genuinely open.
- ❌ Don't renumber or restructure REMAINING_ISSUES.md — cross-references to "#4"/"#5" elsewhere must stay valid, and the contract says minimal updates.
- ❌ Don't touch any `.rs` file, `Cargo.toml`, `packaging/**`, or Jekyll front-matter — this is a docs-only task.
- ❌ Don't run `cargo test` parallel (omit `--test-threads=1`) — AGENTS.md mandates single-threaded for this repo's shared global state.
- ❌ Don't create new documentation sections "to be thorough" — the contract says "err on the side of minimal, accurate updates."