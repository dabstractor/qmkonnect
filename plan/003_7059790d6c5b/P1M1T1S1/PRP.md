# PRP — P1.M1.T1.S1: Fix `qmk-notifier_notify` → `qmk_notifier_notify` in docs/troubleshooting.md

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`) — THIS repo.
> **Scope:** A single-word documentation edit. No source, build, or behavior change.
> **This IS the documentation fix** (Mode A) — the file being edited is the doc.

---

## Goal

**Feature Goal**: Correct one stale firmware-naming reference in
`docs/troubleshooting.md` so it follows the v0.2.8 naming convention. The
illustrative callback name `qmk-notifier_notify` (hyphen) must become
`qmk_notifier_notify` (underscore), because it names a **firmware C callback**
and firmware uses the underscore form.

**Deliverable**: `docs/troubleshooting.md` line 647 changed from

```
   (there is no built-in `qmk-notifier_notify` callback — the firmware API is the
```

to

```
   (there is no built-in `qmk_notifier_notify` callback — the firmware API is the
```

— a single-word replacement (`qmk-notifier_notify` → `qmk_notifier_notify`) within
the existing line. No other text changes. No new files.

**Success Definition**: `docs/troubleshooting.md:647` reads
``qmk_notifier_notify`` (backticks, underscore); `grep -rn 'qmk-notifier_notify'
docs/troubleshooting.md` returns **zero** hits after the edit; the surrounding
prose + ```c code block are byte-for-byte unchanged otherwise; the markdown still
renders as before (it is a `code`-span identifier inside a sentence).

## User Persona (if applicable)

**Target User**: A developer/user reading the troubleshooting guide who is
debugging the firmware side and needs to know whether a built-in debug callback
exists (it does not).

**Use Case**: The reader follows §"Debugging the Firmware Side" and is told to
add their own `printf` inside a callback. The illustrative callback name must use
the correct firmware convention so it is not mistaken for a real symbol.

**Pain Points Addressed**: The hyphenated form `qmk-notifier_notify` contradicts
the v0.2.8 naming table (underscore = firmware) and could mislead a reader into
thinking the firmware module uses hyphens. Aligning it removes the contradiction.

## Why

- **Naming-correctness drift.** v0.2.8 standardized the convention (PRD §1.1 table
  + §13 glossary + the "Naming hazard" callout): `qmk_notifier` (underscore) is the
  **firmware C module** (`dabstractor/qmk_notifier`); `qmk-notifier` (hyphen) is
  the **Rust transport crate** (`dabstractor/qmk-notifier`, tag v0.3.0). A firmware
  *callback* is a C symbol in the firmware module, so it must use the underscore
  form. The current text uses the hyphen form — residual drift caught by the delta
  verification pass (`plan/003_7059790d6c5b/architecture/delta_verification.md`
  §"Residual Drift").
- **It is a throwaway example** ("there is no built-in … callback"), purely
  illustrative — so the fix is cosmetic, not behavioral. Still, accuracy matters:
  a reader copy-pasting or grepping the firmware tree for `qmk_notifier_notify`
  should find nothing (correct), whereas `qmk-notifier_notify` is malformed and
  confusing.
- **It blocks the downstream sibling.** `docs/llms_full.txt:2622` mirrors this line
  verbatim and must be regenerated after the source is fixed (that regeneration is
  P1.M1.T1.S2 — a separate subtask, **out of scope here**).

## What

A single in-place word replacement on **one line** of `docs/troubleshooting.md`.

### Exact change (line 647)

```diff
-   (there is no built-in `qmk-notifier_notify` callback — the firmware API is the
+   (there is no built-in `qmk_notifier_notify` callback — the firmware API is the
```

- Change ONLY `qmk-notifier_notify` → `qmk_notifier_notify`.
- Keep the surrounding backticks (` ` ` `), the em-dash (`—`), the leading
  indentation (3 spaces), and all other text byte-for-byte identical.
- Do NOT touch any other line, including the ```c fenced block that follows.

### Context (lines 642–649 — for orientation only, do not edit)

```markdown
   To watch what the keyboard receives, add your own `printf` inside a callback
   (there is no built-in `qmk_notifier_notify` callback — the firmware API is the
   `DEFINE_SERIAL_LAYERS` / `DEFINE_SERIAL_COMMANDS` macros):
   ```c
   #ifdef CONSOLE_ENABLE
```

### Success Criteria

- [ ] `docs/troubleshooting.md:647` contains `` `qmk_notifier_notify` `` (backticks,
      underscore form).
- [ ] `grep -rn 'qmk-notifier_notify' docs/troubleshooting.md` → **zero** hits.
- [ ] No other line in `docs/troubleshooting.md` is modified.
- [ ] No file other than `docs/troubleshooting.md` is modified
      (`docs/llms_full.txt` is regenerated in P1.M1.T1.S2 — do NOT hand-edit it here).

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed
> to implement this successfully?"_ — **Yes.** The exact file, exact line, exact
> before/after text, the naming convention that justifies it, and the grep-based
> verification command are all below. No codebase exploration is required beyond
> opening the one file.

### Documentation & References

```yaml
# MUST READ — the verification doc that identified the drift (explains the "why")
- file: /home/dustin/projects/qmkonnect/plan/003_7059790d6c5b/architecture/delta_verification.md
  why: "§'Residual Drift (Actionable)' names exactly this line (docs/troubleshooting.md:647)
        as the primary source of the stale hyphen form, and notes llms_full.txt:2622 mirrors
        it (handled by the sibling subtask). Confirms this is throwaway/illustrative, not a
        real symbol."
  section: "Residual Drift (Actionable)"
  critical: "Two files carry the drift; THIS subtask fixes only the SOURCE (troubleshooting.md).
             The mirror (llms_full.txt) is regenerated in P1.M1.T1.S2. Do not hand-edit llms_full.txt."

# MUST READ — the file being edited (confirm the exact line before editing)
- file: /home/dustin/projects/qmkonnect/docs/troubleshooting.md
  why: "Contains the target line 647. Read lines ~640–650 to confirm the exact text and
        indentation before making the replacement."
  pattern: "Jekyll/Liquid markdown doc (note `{{ site.baseurl }}` link syntax nearby). The
            target is a `code`-span (`backticks`) inside a prose sentence."
  gotcha: "Preserve the leading 3-space indentation and the em-dash (—, U+2014). Use an
           exact-text replacement, not a regex that could match the wrong line."

# REFERENCE — the naming convention that governs the fix (source of truth)
- file: /home/dustin/projects/qmkonnect/PRD.md
  why: "§1.1 'The broader ecosystem' table + the 'Naming hazard' callout + §13 Glossary define:
        qmk_notifier (underscore) = firmware C module; qmk-notifier (hyphen) = Rust transport
        crate. Since the illustrative token is a FIRMWARE callback, the underscore form is correct."
  section: "1.1 The broader ecosystem" and "13. Glossary"
  critical: "Only the firmware module uses underscores; the Rust crate uses hyphens but is
             aliased to qmk_notifier in Rust source. A C callback name belongs to the firmware
             module, so it is qmk_notifier_<callback>, never qmk-notifier_<callback>."
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/                      # THIS repo
├── docs/
│   ├── troubleshooting.md      # <-- FILE TO EDIT (line 647). 742 lines total.
│   └── llms_full.txt           # mirrors troubleshooting.md:647 at line 2622 (regenerated in S2 — DO NOT edit here)
├── PRD.md                      # §1.1 naming table + §13 glossary (convention source of truth)
└── plan/003_7059790d6c5b/
    └── architecture/
        └── delta_verification.md   # §Residual Drift identifies this exact line
```

### Desired Codebase tree with files to be modified

```bash
docs/
└── troubleshooting.md   # MODIFIED ONLY — one word on line 647 (qmk-notifier_notify → qmk_notifier_notify)
```

> No new files. `docs/llms_full.txt` is NOT edited in this subtask (regeneration
> is the next sibling, P1.M1.T1.S2).

### Known Gotchas of our codebase & Library Quirks

```text
# CRITICAL: use an EXACT-text replacement scoped to line 647, not a project-wide regex.
#   The string `qmk-notifier_notify` is unique to docs/troubleshooting.md:647 in the docs/
#   tree (confirmed: also appears in docs/llms_full.txt:2622, which is the mirror — out of
#   scope here). A blind `sed` across docs/ would also hit llms_full.txt; constrain the edit
#   to troubleshooting.md only.

# CRITICAL: the fix is firmware-name-specific, NOT a blanket hyphen→underscore rule.
#   Do NOT "fix" other hyphenated occurrences. The Rust crate name `qmk-notifier` (hyphen)
#   is CORRECT and appears legitimately throughout docs/ (e.g. PRD §1.1 table, crate dep
#   references). Only THIS illustrative firmware callback token is wrong. The rule: a token
#   that names a firmware C symbol (module/callback/function) uses underscore; the crate
#   package/repo name uses hyphen.

# NOTE: `qmk_notifier_notify` is NOT a real symbol — it is illustrative ("there is no
#   built-in … callback"). The fix makes the illustrative name consistent with firmware
#   naming; it does not create or reference a real function. Grepping the firmware tree
#   for it will (correctly) find nothing.

# NOTE: docs/troubleshooting.md is Jekyll-rendered (Liquid `{{ site.baseurl }}` links).
#   The target line is plain markdown prose with a `code` span — no Liquid tags on the
#   line — so a plain text replacement is safe and will not affect templating.

# NOTE: the file uses an em-dash (—, U+2014), not a hyphen-minus, between "callback" and
#   "the firmware API". Preserve it exactly; do not "normalize" it to `--` or `-`.
```

## Implementation Blueprint

### Data models and structure

Not applicable — this is a documentation text edit with no data models, types, or
code. The "data" is a single backtick-quoted identifier string.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: CONFIRM the exact target text before editing
  - READ: /home/dustin/projects/qmkonnect/docs/troubleshooting.md lines ~640–650.
  - CONFIRM line 647 reads EXACTLY:
          "   (there is no built-in `qmk-notifier_notify` callback — the firmware API is the"
          (3-space indent, backticks around qmk-notifier_notify, em-dash U+2014).
  - CONFIRM via: grep -n 'qmk-notifier_notify' docs/troubleshooting.md  → exactly one hit (line 647).
  - GOAL: anchor the replacement so it cannot drift if the file was recently reformatted.

Task 2: REPLACE the single word on line 647 (and ONLY line 647)
  - EDIT: docs/troubleshooting.md
  - OLD TEXT (exact, unique): "   (there is no built-in `qmk-notifier_notify` callback — the firmware API is the"
  - NEW TEXT (exact):          "   (there is no built-in `qmk_notifier_notify` callback — the firmware API is the"
  - CHANGE: qmk-notifier_notify → qmk_notifier_notify (one hyphen → one underscore).
  - PRESERVE: leading 3 spaces, both backticks, the em-dash (—), and all following text.
  - DO NOT: touch any other line. DO NOT edit docs/llms_full.txt (sibling subtask S2).
  - DO NOT: run any blanket hyphen→underscore replace across the repo (see gotchas).

Task 3: VALIDATE (do not skip — the grep IS the contract check)
  - RUN: grep -rn 'qmk-notifier_notify' docs/troubleshooting.md
          → MUST print nothing (zero hits) after the edit.
  - RUN: grep -n 'qmk_notifier_notify' docs/troubleshooting.md
          → MUST print exactly one hit (line 647).
  - RUN: git -C /home/dustin/projects/qmkonnect diff --stat docs/troubleshooting.md
          → expect exactly 1 file, and `git diff docs/troubleshooting.md` shows only the
          one hyphen→underscore change on line 647.
  - CONFIRM the diff is a single line change (no accidental whitespace/line-ending churn).
```

### Implementation Patterns & Key Details

```text
# Use an exact-string edit tool (the repo's edit primitive), NOT a global sed.
# Anchor on the full unique line so the replacement is unambiguous:

# OLD (unique in the file):
#   (there is no built-in `qmk-notifier_notify` callback — the firmware API is the
# NEW:
#   (there is no built-in `qmk_notifier_notify` callback — the firmware API is the
#                              ^ hyphen-minus replaced with underscore
```

### Integration Points

```yaml
SOURCE FILES:
  - modify: "docs/troubleshooting.md ONLY (line 647)"

DEPENDENCIES / BUILD:
  - none. This is a markdown prose edit. No cargo/rust/rebuild implication.
  - The project does not gate docs on a build step; there is no mdbook/mkdocs link check
    in CI for inline `code` spans, so this change cannot break a link/lint.

DOWNSTREAM CONSUMER (do NOT implement here — next sibling):
  - P1.M1.T1.S2: "Regenerate docs/llms_full.txt so its mirror of line 647 (currently at
                  llms_full.txt:2622) picks up the corrected qmk_notifier_notify. Do NOT
                  hand-edit llms_full.txt as part of THIS subtask."
  - P1.M1.T1.S3: "Full-tree verification grep + cargo check (covers this fix + the regen)."

VERIFICATION-ONLY CONSUMERS:
  - plan/003_7059790d6c5b/architecture/delta_verification.md §Residual Drift: this task
    closes item #1 of that list.
```

## Validation Loop

> All commands run from the repo root: `/home/dustin/projects/qmkonnect`

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect

# Confirm the edit landed on exactly the right token, on exactly one line.
grep -n 'qmk_notifier_notify' docs/troubleshooting.md
# Expected: exactly ONE line, numbered 647 (±0 if the file is unchanged elsewhere):
#   647:   (there is no built-in `qmk_notifier_notify` callback — the firmware API is the

# Confirm NO stale hyphen form remains in the edited file.
grep -rn 'qmk-notifier_notify' docs/troubleshooting.md
# Expected: NO output (exit code 1). Any hit means the edit missed or hit the wrong line.

# Confirm the diff is minimal (single line, single token).
git diff -- docs/troubleshooting.md
# Expected: one hunk, one `-`/`+` pair differing only in the one character (hyphen → underscore).
```

### Level 2: Unit Tests (Component Validation)

```text
NOT APPLICABLE — this is a markdown documentation edit. There are no unit tests for prose
`code` spans in docs/. The grep checks in Level 1 ARE the component verification.
```

### Level 3: Integration Testing (System Validation)

```bash
cd /home/dustin/projects/qmkonnect

# Render sanity: the line is plain markdown prose (a `code` span inside a sentence), so
# it cannot break Jekyll/Liquid or any link. Confirm the only changed file is the doc:
git status --short docs/
# Expected: only  M docs/troubleshooting.md   (NOT docs/llms_full.txt — that is S2)

# Whole-doc structure intact (heading/code-fence balance unchanged):
grep -cE '^```' docs/troubleshooting.md
# Expected: an EVEN number (fences balanced) — same count as before the edit (the change
# does not add or remove a fence). If the count is odd, the edit accidentally broke a fence.
```

### Level 4: Creative & Domain-Specific Validation

```bash
cd /home/dustin/projects/qmkonnect

# Domain check: the corrected name follows the firmware convention. Confirm no OTHER
# legitimate hyphen-form crate references were accidentally touched:
grep -rn 'qmk-notifier' docs/troubleshooting.md | head
# Expected: the Rust-crate references (hyphen) REMAIN intact (e.g. any "qmk-notifier v0.3.0"
# crate dep mention). Only the firmware-callback token changed. The fix is surgical.

# Confirm the illustrative token is now consistent with firmware naming (underscore):
grep -on 'qmk_notifier_notify' docs/troubleshooting.md
# Expected: 647:qmk_notifier_notify  (one match, underscore form).
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1 passed: `grep -n 'qmk_notifier_notify' docs/troubleshooting.md` → exactly one hit (line 647).
- [ ] Level 1 passed: `grep -rn 'qmk-notifier_notify' docs/troubleshooting.md` → zero hits.
- [ ] Level 1 passed: `git diff docs/troubleshooting.md` → one line, one token changed.
- [ ] Level 3 passed: `git status --short docs/` → only `docs/troubleshooting.md` modified.
- [ ] Level 3 passed: code-fence count in the file is unchanged (still balanced).

### Feature Validation

- [ ] Line 647 reads `` `qmk_notifier_notify` `` (underscore, backticks preserved).
- [ ] Em-dash (—) and 3-space indentation preserved.
- [ ] No other line in the file changed.
- [ ] No file other than `docs/troubleshooting.md` modified (llms_full.txt untouched — S2).

### Code Quality Validation

- [ ] Edit follows the v0.2.8 naming convention (underscore = firmware, hyphen = Rust crate).
- [ ] Replacement is exact-text and line-scoped (no blanket hyphen→underscore sweep).
- [ ] Legitimate `qmk-notifier` (Rust crate) references in the file are untouched.

### Documentation & Deployment

- [ ] The doc itself is the deliverable (Mode A — no separate doc task).
- [ ] Markdown still renders as before (a `code` span change only).
- [ ] No environment variables or config affected.

---

## Anti-Patterns to Avoid

- ❌ Don't run a repo-wide or `docs/`-wide `sed s/qmk-notifier/qmk_notifier/g` — it would
  wrongly rewrite legitimate Rust-crate references (`qmk-notifier` v0.3.0) and hit
  `docs/llms_full.txt` (out of scope; regenerated in S2). Scope the edit to line 647 only.
- ❌ Don't "fix" other hyphenated `qmk-notifier` occurrences — the hyphen is correct for the
  Rust crate/package/repo. Only a *firmware* symbol uses underscore.
- ❌ Don't hand-edit `docs/llms_full.txt` here — it is a generated mirror; regenerating it is
  the next sibling subtask (P1.M1.T1.S2).
- ❌ Don't drop or alter the backticks, the em-dash (—), or the leading indentation — the edit
  is a single-character change inside the existing token.
- ❌ Don't skip the validation grep — `grep -rn 'qmk-notifier_notify' docs/troubleshooting.md`
  returning nothing is the proof the fix landed and is complete.
- ❌ Don't treat `qmk_notifier_notify` as a real symbol to implement — it is explicitly
  illustrative ("there is no built-in … callback"); grepping the firmware tree for it should
  find nothing.

---

**Confidence Score: 10/10** for one-pass implementation success. The deliverable is a
single, fully-specified character-level edit (one hyphen → one underscore) on one
identified line of one file, with the exact before/after text, the naming convention
that justifies it, and a deterministic grep-based verification that proves completion.
There is no build, no test, and no dependency involved — the only risk (a blanket
sweep) is called out and explicitly forbidden.