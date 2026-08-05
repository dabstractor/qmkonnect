# PRP — P1.M2.T3.S2: Verify README.md and docs remain accurate for autostart and heuristic changes

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`) — Rust tray/menu-bar daemon.
> **Mode:** [Mode B] documentation-sync task — this IS the documentation task.
> **Expected outcome: ZERO file edits.** The contract defines a *verification* whose
> research-backed verdict (see `research/notes.md`) is **"no doc changes needed"** — all
> three target files are already accurate post-changeset. The implementing agent must
> **independently re-verify** this with the exact checks below, then record the verdict
> in the task completion / commit message and mark complete. **Do NOT invent edits.**
> **Scope:** `README.md`, `docs/installation.md`, `docs/usage.md` only (read + verify).
> Do NOT edit `docs/troubleshooting.md` (owned by sibling P1.M2.T3.S1), do NOT edit any
> `.rs` file, do NOT edit `docs/configuration.md` / `docs/qmk-integration.md`.

---

## ⚠️ READ FIRST — this is a VERIFY task whose primary path is NO-CHANGE

The contract (P1.M2.T3.S2, point #3 "LOGIC") is explicit:
> "Read `README.md` L275-285 and `docs/installation.md` L20-35. Confirm that no text
> describes the Run key value FORMAT (quoted vs unquoted) — if none does, no change is
> needed. Confirm that no README/overview text claims handshake behavior or title
> filtering behavior that the fixes contradict. **If everything is already accurate,
> note 'no doc changes needed' in the commit message and mark complete.**"

➡️ **Research verdict (full evidence in `research/notes.md`): NO-CHANGE.** All three
fixes are *internal correctness* fixes with **no documented behavior surface** in the
target files:
- **Autostart quoting (P1.M1.T3)** — wraps the HKCU Run-key `REG_SZ` value in quotes
  for spaced paths. The docs describe the **mechanism** ("via the HKCU Run key") and
  the **toggle** ("Open at Login") and "enabled by default" — never the value *format*.
- **Handshake reset (P1.M2.T1)** — adds `reset_handshake_state()`+`perform_handshake()`
  to the Settings VID/PID save path on all platforms. No README/installation/usage text
  describes handshake behavior, VID/PID-change handling, or multi-board state.
- **Title heuristic (P1.M2.T2)** — `.len()` → `.chars().count()` in
  `should_ignore_window`. No target doc describes title-length filtering, byte/char
  counting, or a "short title" rule; the sent data *format*
  (`{application_class}{GS}{window_title}`) is unchanged.

➡️ **The agent MUST still perform the verification itself** (Steps 1-3 below) — do not
blindly trust this verdict. Re-run the exact grep checks against the 3 files; only if
they come back as specified here is the no-change verdict confirmed. If (unexpectedly) a
contradicting claim IS found, apply the minimal targeted fix from the fallback table in
the "What" section — but the research finds none.

---

## Goal

**Feature Goal**: Verify that `README.md`, `docs/installation.md`, and `docs/usage.md`
contain **no prose claim that is falsified by** the P1.M1.T3 (autostart quoting),
P1.M2.T1 (handshake reset), or P1.M2.T2 (title heuristic) changesets, so the
documentation ships accurate alongside the code fixes.

**Deliverable**: A documented verification verdict. **Primary path (expected): no
edits** to the 3 target files — the verdict "no doc changes needed" is recorded in the
task completion / commit message and the task is marked complete. (Fallback path, only
if verification surfaces a real contradiction: a minimal surgical edit, specified per
finding in the table below.)

**Success Definition**:
- The three verification checks in the "What" section are each performed and recorded.
- Either (a) all three return "no contradicting claim found" → **no file edits**,
  verdict = "no doc changes needed"; OR (b) a specific contradicting claim is found and
  fixed with the minimal edit from the fallback table, leaving all other prose verbatim.
- `git diff --stat` for the 3 target files is **empty** in the no-change path.
- The task is NOT marked complete without the verification checks having been run.
- `docs/troubleshooting.md`, all `.rs` files, `docs/configuration.md`,
  `docs/qmk-integration.md`, `PRD.md`, `tasks.json`, `prd_snapshot.md`, `.gitignore`
  are **untouched** by this task.

## User Persona (if applicable)

**Target User**: a contributor/release reviewer (or future maintainer) reading the
README + install/usage docs alongside the bug-fix changeset, asking "did these code
changes invalidate anything the docs claim?"

**Use Case**: After the milestone lands, a reviewer sanity-checks that user-facing docs
still match reality. This task guarantees that check was done and recorded, so no
stale/false claim ships (e.g. the docs don't promise an autostart format, a handshake
guarantee, or a title-filtering behavior that the fixes quietly changed).

**User Journey**: reviewer opens README/installation/usage → expects them to describe
mechanisms (Run key, Open-at-Login toggle, data format) → verification confirms every
claim still holds → reviewer signs off; verdict recorded in the milestone history.

**Pain Points Addressed**: prevents the silent drift where an internal fix changes
behavior the docs happen to assert; gives an explicit, greppable "verified accurate"
record instead of an assumption.

## Why

- **Closes the docs-verification loop for the three remaining fixes.** P1.M1.T3,
  P1.M2.T1, P1.M2.T2 are code-complete; this task confirms they introduce no doc
  inaccuracy in README/installation/usage. (The fourth docs item — Hyprland/X11
  identifier accuracy in `docs/troubleshooting.md` — is owned by sibling S1.)
- **All three fixes are internal.** Quoting a registry value, resetting handshake state,
  and counting characters instead of bytes change *how* correct behavior is achieved, not
  *what* the docs describe. Verification (not rewriting) is the proportionate response.
- **Cheap, explicit, and auditable.** Three negative-grep checks + a targeted read beat a
  vague "looks fine"; the recorded verdict is greppable in the milestone history.
- **Scoped & non-conflicting.** Strictly the 3 named files; S1's
  `docs/troubleshooting.md` edit is untouched; no code touched.

## What

### Step A — Verification checks (RUN ALL THREE; this IS the task)

Perform each check against **only** `README.md`, `docs/installation.md`, `docs/usage.md`
(read-only; these are the contract-scope files). For each, record the result in the
verdict/commit message.

**Check 1 — Run-key value FORMAT (quoted vs unquoted).**
Read `README.md` L275-285 (the "Technical Requirements → Windows Implementation"
bullets, esp. the "Automatic Startup" bullet at L280-281), `docs/installation.md`
L20-35 (the Windows installer bullet list, esp. L30), and `docs/usage.md` ~L43-50
(Auto-Start on Boot → Windows). Confirm the prose describes the **mechanism** ("via the
HKCU Run key"), the **toggle** ("Open at Login"), and "enabled by default" — and
**does NOT** state or imply a value *format* (quoted/unquoted/`REG_SZ`/exact bytes).
```bash
# Expect: hits only about mechanism/toggle; NO hit about value format.
grep -niE 'quoted|unquoted|quote|value.?data|reg_sz|format of the .*key' README.md docs/installation.md docs/usage.md
# Expect: this returns the mechanism lines (accurate), nothing about format.
grep -niE 'run key|open at login|launch at login' README.md docs/installation.md docs/usage.md
```

**Check 2 — Handshake / VID/PID-change-reset / multi-board behavior.**
Confirm **no** sentence claims handshake behavior, VID/PID-change device-reset
semantics, or per-board handshake state. (The handshake-reset fix is internal; the only
VID/PID prose that exists is the discovered-device picker writing IDs for you, which is
a different feature and remains accurate.)
```bash
# Expect: at most README L219/L221 picker prose ("writes its VID/PID for you"); NO
# claim about handshake state, resetting on settings change, or per-board handshake.
grep -niE 'handshake|reset_handshake|perform_handshake|vid/pid change|pid change|multi-board|per-board|callback map|rebuild' README.md docs/installation.md docs/usage.md
```

**Check 3 — Window-title length filtering / heuristic / byte-vs-char.**
Confirm **no** sentence claims a title-length filter, a "short title" rule, byte length,
character count, or any window-ignore heuristic. (The data *format*
`{application_class}{GS}{window_title}` in README L303 is unchanged and accurate.)
```bash
# Expect: (none) — no prose about title length, short titles, or byte/char counting.
grep -niE 'title length|short title|ignore.*window|window.*filter|heuristic|byte length|character count|chars\(\)|\.len\(\)' README.md docs/installation.md docs/usage.md
```

### Step B — Decision

- **If all three checks return as specified above (no contradicting claim):**
  → **NO file edits.** Verdict = **"no doc changes needed."** Record the verdict and the
  three check results in the task completion / commit message; mark complete. (This is
  the expected and research-backed outcome.)
- **If a check surfaces a contradicting claim:** apply the **single minimal edit** from
  the fallback table below — change only the inaccurate clause, leave all other prose
  verbatim — then record the finding + edit in the commit message.

### Fallback edit table (only if verification unexpectedly finds a contradiction)

| Finding | File / anchor | Minimal edit |
| --- | --- | --- |
| A doc states/implies the Run-key value is written **unquoted** (or specifies the exact bytes/format) | `README.md` L280-281 **or** `docs/installation.md` L30 **or** `docs/usage.md` L48 | Delete or generalize the format-specific clause so only the **mechanism** ("via the HKCU Run key") + toggle remain. Do NOT add quoting detail — docs intentionally stay mechanism-level. |
| A doc claims handshake state is **not** reset on device change, or asserts single-board-only handshake | the relevant line in any of the 3 files | Generalize/remove the assertion; the reset is now standard behavior and needs no doc surface (leave mechanism-level only). |
| A doc describes a window-title **length/byte/character** filter rule | the relevant line | Remove the encoding/length-specific claim; docs do not document the ignore-heuristic. Leave the generic data-format line (`{application_class}{GS}{window_title}`) untouched. |

> **Note:** the research (Section 3 of `research/notes.md`) found **none** of these
> findings. The table exists so that if the live verification disagrees, the agent has a
> bounded, in-scope remedy rather than improvising or expanding scope.

### Success Criteria
- [ ] Check 1 performed and recorded: no Run-key value-**format** claim in the 3 files.
- [ ] Check 2 performed and recorded: no handshake/VID-PID-change/multi-board behavior
      claim that the reset fix contradicts.
- [ ] Check 3 performed and recorded: no title-length/byte/char/heuristic claim that the
      heuristic fix contradicts.
- [ ] Either (a) all three pass → **zero edits** to the 3 target files, verdict recorded;
      or (b) a specific finding is fixed with the minimal fallback edit and recorded.
- [ ] `docs/troubleshooting.md` unchanged by this task (owned by S1).
- [ ] No `.rs` / `PRD.md` / `tasks.json` / `prd_snapshot.md` / `.gitignore` /
      `docs/configuration.md` / `docs/qmk-integration.md` edited by this task.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge can complete this task using only the three
verification checks (exact grep commands + expected outcomes), the decision rule, the
fallback table, and the scope guardrails — all present in this PRP. No Rust knowledge is
required (the code references below justify *why* the docs are accurate, not what to
type).

### Documentation & References

```yaml
# MUST READ — the three target files at the contract-cited ranges (verify, do not edit unless a finding)
- file: README.md
  why: "contract point #3 cites L275-285; the 'Automatic Startup' bullet (L280-281) is
        the autostart claim to check (mechanism + toggle, NOT format)."
  pattern: "L280-281: '**Automatic Startup**: **Open at Login** via the HKCU `Run` key —
        default on, toggleable from the tray (`src/autostart.rs`).' — accurate as-is."
  gotcha: "L303 '{application_class}{GS}{window_title}' is the data FORMAT, unchanged by
        the heuristic; leave it. L219/L221 VID:PID prose is the device PICKER, not
        handshake-reset; leave it."

- file: docs/installation.md
  why: "contract point #3 cites L20-35; L30 'Enables autostart via the HKCU `Run` key
        (toggle it in the tray: Open at Login)' is the claim to check."
  pattern: "mechanism + toggle; no format claim."
  gotcha: "L22 'enables Open at Login by default' is accurate post-quoting-fix."

- file: docs/usage.md
  why: "contract point #1 cites L48; Auto-Start on Boot → Windows (L43-50) is the claim."
  pattern: "L48: 'Open at Login is enabled by default … backed by the HKCU Run key …'
        — mechanism + toggle + default-on; no format claim."
  gotcha: "DO NOT 'fix' the macOS Auto-Start section here — it is stale relative to
        installation.md (SMAppService) but that is a PRE-EXISTING inconsistency unrelated
        to the 3 fixes (the quoting fix is Windows Run-key only). Fixing it expands scope.
        Record as a residual only."

# MUST READ — why the docs are accurate (the three fixes' source of truth; READ ONLY)
- file: src/autostart.rs
  why: "proves the quoting fix changed only the Run-key VALUE format, not the mechanism,
        toggle, or 'enabled by default'. The docs never describe the value format."
  gotcha: "do NOT edit this file — it is the evidence, not the target."
- file: packaging/windows/inno/QMKonnect.iss
  why: "proves the installer-side quoting (P1.M1.T3.S2) is also format-only."
  gotcha: "do NOT edit."
- file: src/platforms/windows.rs   # should_ignore_window: .chars().count()
  why: "proves the heuristic fix is internal (UTF-8 bytes → scalars); the data format
        sent to the keyboard is unchanged, so the docs' format line stays accurate."
  gotcha: "do NOT edit."
- file: src/platforms/windows/tray.rs / src/platforms/macos/tray.rs / src/platforms/linux_tray.rs
  why: "prove handshake reset (P1.M2.T1) is internal (callback-map rebuild on save); no
        user-visible behavior, no UI, no toggle."
  gotcha: "do NOT edit."

# REFERENCE — sibling boundary
- file: docs/troubleshooting.md
  why: "OWNED BY S1 (one clarifying sentence in checklist item #3). S2 must NOT edit it."

# REFERENCE — the research that produced this verdict
- docfile: plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/P1M2T3S2/research/notes.md
  why: "full evidence: what each fix changed, the 3 target files' relevant lines, the
        negative-grep sweep results, the no-change verdict, and the out-of-scope residual."
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
README.md                 # VERIFY L275-285 (autostart), L303 (format), L219-221 (picker) — expected NO-CHANGE
docs/installation.md      # VERIFY L20-35 (Windows installer autostart)       — expected NO-CHANGE
docs/usage.md             # VERIFY L43-50 (Auto-Start on Boot → Windows)      — expected NO-CHANGE
docs/troubleshooting.md   # DO NOT TOUCH (owned by sibling P1.M2.T3.S1)
src/autostart.rs          # READ ONLY (evidence: quoting fix is format-only)
src/platforms/windows.rs  # READ ONLY (evidence: heuristic fix is internal)
src/platforms/*/tray*.rs  # READ ONLY (evidence: handshake reset is internal)
docs/configuration.md     # OUT OF SCOPE (no claim affected by the 3 fixes)
docs/qmk-integration.md   # OUT OF SCOPE
```

### Desired Codebase tree
**No file changes in the expected (no-change) path.** This is a verification task; the
"desired" state is "the 3 target files confirmed accurate and untouched." (Fallback path:
at most one minimal in-place clause edit per the table; no new files, no restructuring.)

### Known Gotchas of our codebase & Library Quirks
```text
# CRITICAL (task shape): this is a VERIFY task. The research-backed verdict is NO-CHANGE.
#   Do NOT edit a file just to "produce a diff." An empty diff for the 3 target files,
#   with the verification recorded, IS the successful deliverable. Editing prose that is
#   already accurate is a failure mode.

# CRITICAL (scope): touch ONLY README.md / docs/installation.md / docs/usage.md, and only
#   IF a verification check finds a genuine contradiction (none expected). NEVER edit
#   docs/troubleshooting.md (S1 owns it), any .rs file, PRD.md, tasks.json,
#   prd_snapshot.md, .gitignore, docs/configuration.md, or docs/qmk-integration.md.

# GOTCHA (macOS usage.md residual): docs/usage.md "Auto-Start on Boot → macOS" is stale
#   vs installation.md (SMAppService), but it is UNRELATED to the 3 fixes (quoting is
#   Windows Run-key only). Fixing it expands scope. Record it as a residual; do NOT fix.

# GOTCHA (no build/test gate): there is no docs build or linter in the dev loop. Do NOT
#   run cargo or the packaging scripts for this task. "Validation" = the grep checks +
#   git diff scope check (read-only shell).

# GOTCHA (picker vs handshake): README L219/L221 "writes its VID/PID for you" is the
#   discovered-DEVICE PICKER, a different feature from handshake-reset-on-settings-save.
#   It is accurate and must NOT be conflated with the handshake fix.

# GOTCHA (format vs filter): README L303 "{application_class}{GS}{window_title}" is the
#   data FORMAT sent to the keyboard; the heuristic fix only changes WHICH windows are
#   ignored, not the format. L303 stays accurate — do not touch it.
```

## Implementation Blueprint

### Data models and structure
None. Documentation-verification task (no code, no data).

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: READ the three target files at the contract-cited ranges
  - README.md L275-285 (autostart bullet L280-281), L219-221 (picker), L303 (format)
  - docs/installation.md L20-35 (installer bullet L30), L22 (default-on)
  - docs/usage.md L43-50 (Auto-Start on Boot → Windows, L48)
  - PURPOSE: eyeball-confirm the prose is mechanism/toggle/default-on level, not
    format/behavior-spec level. Record what each line actually says.

Task 2: RUN the three verification checks (Step A grep commands) against the 3 files
  - Check 1 (Run-key value FORMAT): expect NO format claim.
  - Check 2 (handshake / VID-PID-change / multi-board): expect NO contradicting claim.
  - Check 3 (title length / byte / char / heuristic): expect NO claim.
  - RECORD each check's actual grep output in your verdict notes.

Task 3: DECIDE per Step B
  - IF all three pass (expected): NO edits. Verdict = "no doc changes needed."
  - ELSE: apply the single minimal fallback edit for that finding ONLY.

Task 4: VERIFY scope (read-only git check)
  - SEE Validation Loop. Confirm docs/troubleshooting.md and all out-of-scope files are
    untouched by this task (the only in-flight change in the repo for this milestone is
    S1's docs/troubleshooting.md edit, which is NOT this task's).

Task 5: RECORD the verdict + mark complete
  - Commit/completion message states: "no doc changes needed" + the three check results
    (or, in the fallback path, the finding + edit). See commit-message guidance below.

Task 6: NEVER do these (out of scope / forbidden)
  - DO NOT edit README.md / docs/installation.md / docs/usage.md UNLESS a verification
    check found a genuine contradiction (none expected). An accurate file must stay as-is.
  - DO NOT edit docs/troubleshooting.md (owned by sibling P1.M2.T3.S1).
  - DO NOT edit any .rs file, Cargo.toml, PRD.md, tasks.json, prd_snapshot.md, .gitignore,
    docs/configuration.md, or docs/qmk-integration.md.
  - DO NOT run cargo build/test or the packaging scripts — no compile/test gate applies.
  - DO NOT fix the docs/usage.md macOS Auto-Start staleness (pre-existing, unrelated,
    out of scope — record as residual only).
  - DO NOT add quoting/format/handshake/heuristic detail to the docs to "reflect the fix"
    — the docs are intentionally mechanism-level; the fixes are internal.
```

### Implementation Patterns & Key Details
```text
# PATTERN: verify-then-decide, not edit-by-default. The contract's success path is a
#   recorded verdict with an empty diff. The three grep checks are the proof; the
#   fallback table is the bounded remedy only if a check fails.

# PATTERN: mechanism-level docs stay mechanism-level. The 3 fixes changed HOW correct
#   behavior is achieved (quote the value, reset state, count scalars), not WHAT the
#   docs describe. Adding fix-specific detail would be scope creep, not accuracy.

# WHY negative greps are the core check: a fix can only falsify a CLAIM. If no claim
#   about value-format/handshake-behavior/title-filtering exists in the 3 files, the
#   fixes cannot have falsified anything — ipso facto accurate. The greps prove the
#   absence of such claims deterministically.

# ANTI-PATTERN: "I'll add a sentence about quoting to be thorough." NO. The contract
#   explicitly says if everything is accurate, no change is needed. Adding unrequested
#   detail is a failure (scope creep + maintenance burden), not a success.
```

### Integration Points
```yaml
FILES:       README.md, docs/installation.md, docs/usage.md (verify only; expected no edit).
SIBLING:     docs/troubleshooting.md is P1.M2.T3.S1's exclusive edit target — do not touch.
CODE:        none touched (the 3 fixes are already committed: 789dbc9, 1f34529, e013d4d,
             1896c11, 68aa7ea, d7c0a13).
BUILD:       none — no docs build / linter in the dev loop; no cargo/packaging for this task.
DOWNSTREAM:  completing this task closes P1.M2.T3 (docs sync) and is the last subtask of
             the P1 milestone's docs-sync parent; the milestone can then be marked Complete.
```

## Validation Loop

> **Docs-only verification task. No compiler, test runner, or linter applies.** All
> validation is **manual + read-only shell checks**. Do NOT run `cargo` or packaging.

### Level 1: Verification checks (this IS the core task — perform & record all three)
```bash
cd /home/dustin/projects/qmkonnect
# Check 1 — Run-key value FORMAT: expect NO output (no format claim anywhere).
grep -niE 'quoted|unquoted|quote|value.?data|reg_sz|format of the .*key' README.md docs/installation.md docs/usage.md
# Check 2 — handshake / VID-PID-change / multi-board: expect at most README L219/L221
#           picker prose ("writes its VID/PID for you"), which is NOT a handshake claim.
grep -niE 'handshake|reset_handshake|perform_handshake|vid/pid change|pid change|multi-board|per-board|callback map|rebuild' README.md docs/installation.md docs/usage.md
# Check 3 — title length / byte / char / heuristic: expect NO output.
grep -niE 'title length|short title|ignore.*window|window.*filter|heuristic|byte length|character count|chars\(\)|\.len\(\)' README.md docs/installation.md docs/usage.md
# Expected (research-backed): Check 1 empty; Check 2 = picker prose only; Check 3 empty.
```

### Level 2: Accuracy cross-check (read-only — confirm the fixes really are internal)
```bash
cd /home/dustin/projects/qmkonnect
# Autostart quoting touched only the Run-key VALUE format (mechanism unchanged):
git show --stat 789dbc9 1f34529 | grep -E 'autostart.rs|\.iss'        # only src/autostart.rs + QMKonnect.iss
# Title heuristic is internal (should_ignore_window; data format unchanged):
git show d7c0a13 -- src/platforms/windows.rs | grep -E 'chars\(\)\.count\(\)|\.len\(\)'
# Handshake reset is internal (tray save path; no user-visible behavior):
git show --stat e013d4d 1896c11 68aa7ea | grep -E 'tray'             # only tray.rs / linux_tray.rs
# Expected: each fix is confined to its internal surface → docs need no behavior update.
```

### Level 3: Scope check (read-only)
```bash
cd /home/dustin/projects/qmkonnect
# In the expected no-change path: the 3 target files have NO diff from THIS task.
# (S1's docs/troubleshooting.md edit is a separate, pre-existing working-tree change —
#  it is NOT this task's; do not commit/revert it here.)
git diff --stat -- README.md docs/installation.md docs/usage.md       # expect EMPTY in no-change path
# Confirm the forbidden files are untouched by this task:
git diff --name-only | grep -E 'troubleshooting.md|\.rs$|PRD.md|tasks.json|prd_snapshot.md|\.gitignore|configuration.md|qmk-integration.md' || echo "scope-clean"
# Expected: "scope-clean" (no forbidden file touched by this task).
```

### Level 4: Prose / rendered review (manual, no-change path)
```text
# Read README L280-281, installation.md L30/L22, usage.md L48 once more. Confirm each:
#  - States the mechanism (HKCU Run key) + the toggle (Open at Login) + default-on.
#  - Makes NO claim about value format, handshake/reset behavior, or title filtering.
# If so: verdict = "no doc changes needed." Record it. Done.
```

## Final Validation Checklist

### Technical Validation
- [ ] Level 1: Check 1 (format) returns no format claim; Check 2 returns only picker
      prose (no handshake claim); Check 3 returns no title-filter claim.
- [ ] Level 2: `git show` confirms each of the 3 fixes is confined to its internal
      surface (autostart.rs/.iss, windows.rs heuristic, tray*.rs reset).
- [ ] Level 3: `git diff --stat -- README.md docs/installation.md docs/usage.md` is
      EMPTY (no-change path); forbidden files untouched ("scope-clean").

### Feature Validation
- [ ] The 3 target files are confirmed accurate post-changeset (no falsified claim).
- [ ] Verdict recorded: "no doc changes needed" (or, in fallback, the finding + minimal
      edit) with the three check results.
- [ ] The pre-existing docs/usage.md macOS Auto-Start staleness is recorded as a
      residual (NOT fixed — out of scope).

### Code Quality Validation
- [ ] No file was edited unless a verification check found a genuine contradiction.
- [ ] No scope creep: no fix-specific (quoting/handshake/heuristic) detail added to docs
      that are intentionally mechanism-level.
- [ ] No out-of-scope file touched (troubleshooting.md, .rs, PRD.md, tasks.json,
      prd_snapshot.md, .gitignore, configuration.md, qmk-integration.md).

### Documentation & Deployment
- [ ] Commit/completion message states the verdict and the three check results.
- [ ] Task marked complete only after the verification checks were run and recorded.

### Commit message (guidance — no-change path, expected)
```text
docs: verify README/installation/usage accurate after autostart+handshake+heuristic fixes

Verified the three user-facing docs against the P1.M1.T3 (autostart quoting),
P1.M2.T1 (handshake reset), and P1.M2.T2 (title heuristic) changesets. All three
fixes are internal correctness changes with no documented behavior surface in
README.md / docs/installation.md / docs/usage.md:

 - Autostart quoting: docs describe the HKCU Run-key MECHANISM + the "Open at
   Login" toggle + "enabled by default"; none state the Run-key value FORMAT, so
   the quoting fix falsifies nothing.
 - Handshake reset: no README/installation/usage prose claims handshake state,
   VID/PID-change-reset, or multi-board behavior.
 - Title heuristic: no prose claims a title-length/byte/character filter; the
   sent data format ({application_class}{GS}{window_title}) is unchanged.

Negative-grep checks (format / handshake / title-filter) against the three files
confirm no contradicting claim. No doc changes needed. (Sibling P1.M2.T3.S1 owns
the docs/troubleshooting.md identifier clarification; not touched here.)

Residual (out of scope): docs/usage.md "Auto-Start on Boot → macOS" is stale vs
installation.md (SMAppService) but is unrelated to these three fixes.
```

---

## Anti-Patterns to Avoid
- ❌ Don't edit an already-accurate file to "produce a diff" — an empty diff with a
  recorded verdict is the successful deliverable here.
- ❌ Don't add quoting/handshake/heuristic detail to the docs "to reflect the fix" — the
  docs are intentionally mechanism-level; the fixes are internal. Adding detail is scope
  creep, not accuracy.
- ❌ Don't edit `docs/troubleshooting.md` — it is sibling P1.M2.T3.S1's exclusive target.
- ❌ Don't fix the `docs/usage.md` macOS Auto-Start staleness — it's pre-existing and
  unrelated to the 3 fixes (quoting is Windows Run-key only). Record as residual only.
- ❌ Don't conflate the README L219/L221 device-PICKER VID/PID prose with the
  handshake-reset-on-settings-save fix — different features; the picker text is accurate.
- ❌ Don't touch README L303's `{application_class}{GS}{window_title}` format line — the
  heuristic changes *which* windows are ignored, not the data format.
- ❌ Don't run `cargo build`/`test` or the packaging scripts — no compile/test gate.
- ❌ Don't skip the verification checks and just assert "looks fine" — run the three greps
  and record their actual output.

---

## Confidence Score: 9/10

The verdict (no-change) is backed by direct evidence read this session: the three target
files' relevant lines (README L280-281/L303, installation.md L22/L30, usage.md L48), a
deterministic negative-grep sweep across all three for format/handshake/title-filter
claims (all empty or non-contradicting), and `git show --stat` confirming each fix is
confined to its internal surface. The agent still re-runs the checks independently, so
the result is auditable rather than assumed. The score is 9 rather than 10 only because
there is no automated docs gate (verification rests on the explicit, cheap grep checks),
and because a determined reviewer could argue the macOS usage.md residual "should" be
fixed here — but doing so would expand scope beyond the contract's "verify the 3 fixes"
mandate, so it is correctly left as a recorded residual.