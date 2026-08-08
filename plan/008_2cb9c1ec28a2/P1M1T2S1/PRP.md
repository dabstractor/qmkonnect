# PRP — P1.M1.T2.S1: Verify README.md + spec docs remain correctly synced (mise/asdf removal)

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`).
> **This is the FINAL CHANGESET-LEVEL DOCUMENTATION VERIFICATION (Mode B)** for the
> mise/asdf channel removal (plan/008). It runs LAST, after S1
> (`docs/installation.md` cleanup — Complete) and S2 (regenerate `docs/llms_full.txt`
> — parallel/in-progress). Expected outcome: **VERIFIED — no documentation drift**
> (a no-op). The runbook reproduces that verdict with exact grep commands; if (and
> only if) a mise/asdf channel-advertising ref or a dead `asdf-qmkonnect` link
> appears in a file NOT covered by S1/S2, remove it in place (Mode B) and document it.
> **No `src/`, `spec/`, or `docs/vendor/` edits under any branch** — those hits are
> either intentional (spec/ "NOT a channel" decision) or false positives
> ("promise" ⊃ "mise"; Ruby-gem noise). `git status` for `src/ spec/` must stay clean.
> **⚠ Depends on S2.** If S2 hasn't landed, `docs/llms_full.txt` still shows the
> stale hits — that's S2-pending, NOT a finding here.

---

## Goal

**Feature Goal**: Independently re-confirm — by a grep sweep of the whole tree —
that the mise/asdf channel removal is **complete across every user-facing doc**
and that **no dead `asdf-qmkonnect` plugin-repo link survives** anywhere outside
gitignored planning artifacts. Confirm the user docs are free of mise/asdf
channel-advertising, AND that the intentional `spec/PRD.md` + `spec/PACKAGING.md`
"mise/asdf are NOT a channel" references are present and correct (they document
the exclusion decision, not advertise the channel).

**Deliverable**: A one-line verdict — **`VERIFIED: no documentation drift`**
(expected) — plus a short evidence summary (the gate-grep counts + the
intentional/false-positive classification) captured under
`plan/008_2cb9c1ec28a2/P1M1T2S1/research/`. If an unexpected stale ref is found
in a file not owned by S1/S2, edit that doc in place (Mode B) and list it.

**Success Definition**: The four gate greps (§Validation) reproduce the
research-verified counts: (a) `mise|asdf` in authored user docs (README.md +
docs/*.md, excl `docs/vendor/` + `llms_full.txt`) = 0; (b) `mise|asdf` AND
`asdf-qmkonnect` in `docs/llms_full.txt` = 0 (post-S2); (c) `asdf-qmkonnect`
repo-wide (excl `.git/`, `target/`, `node_modules/`, `.pi-subagents/`, `plan/`,
`docs/vendor/`) = 0; (d) `mise|asdf` in README.md = 0. The spec/ mise/asdf hits
are explicitly classified as intentional (PRD §2.1/F15/§5, PACKAGING §6.4) or
false positive (DEVICE_DISCOVERY "promise"). Verdict stated. `git status --short
src/ spec/` clean (no code/spec touched).

## User Persona (if applicable)

**Target User**: The release/maintainer who needs documented, independent
confirmation that the mise/asdf channel removal swept the entire user-facing doc
surface (not just the two files S1/S2 touched) — so a user reading any doc, or an
LLM ingesting `llms_full.txt`, never sees a non-channel advertised with a broken
`asdf-qmkonnect` link.

**Use Case**: Before declaring the mise/asdf removal changeset fully done
(S1 installation.md + S2 llms_full.txt + **this repo-wide verification**),
confirm no stray mise/asdf ad or dead link lingers in a sibling doc
(usage.md, examples.md, …) that S1/S2 didn't touch.

**Pain Points Addressed**: Removes the risk that a mise/asdf reference or dead
`asdf-qmkonnect` link survives in a doc the per-file tasks (S1/S2) didn't own —
which would re-advertise a removed, non-existent channel.

## Why

- **Closes the changeset.** S1 + S2 fix the two known-stale artifacts. This task
  is the belt-and-suspenders sweep that proves the WHOLE tree is clean — including
  docs S1/S2 didn't touch — and that the intentional spec references (which a naive
  grep would flag) are correctly classified as "NOT a channel" decision docs.
- **It's cheap and self-verifying.** A few deterministic greps reproduce the
  verdict; no build, no tests, no code. The likely outcome (research-confirmed) is
  a no-op "VERIFIED."
- **It distinguishes signal from noise.** The repo has ~60+ `mise|asdf` false
  positives (`docs/vendor/` Ruby gems; "promise" in prose/code) and intentional
  spec references. A naive grep looks dirty; this task applies the correct
  exclusions + classification so "clean" means actually clean.

## What

A four-grep verification + a spot-read of the README Package-Managers table +
a classification of the spec/ hits. The pre-verified result (greps run this
session, post-S1, pre-S2 for llms_full):

### Success Criteria

- [ ] Gate (a): `grep -rin 'mise\|asdf' README.md docs/*.md` (the `docs/*.md` glob
      excludes `docs/vendor/`; this also excludes `llms_full.txt` since it's not
      `.md`) → **0 hits**.
- [ ] Gate (b): `grep -in 'mise\|asdf' docs/llms_full.txt` → **0** AND
      `grep -in 'asdf-qmkonnect' docs/llms_full.txt` → **0** (post-S2).
- [ ] Gate (c): `grep -rn 'asdf-qmkonnect' . | grep -vE '\.git/|/target/|node_modules/|\.pi-subagents/|/plan/|docs/vendor/'`
      → **0 hits**.
- [ ] Gate (d): `grep -in 'mise\|asdf' README.md` → **0** (README Package-Managers
      table has no mise/asdf row).
- [ ] Verdict stated: **`VERIFIED: no documentation drift`** (expected) OR the
      specific file/line edited (drift-found branch).
- [ ] The spec/ mise/asdf hits classified: `spec/PRD.md` (§2.1 L97, F15 L152, §5
      L169) + `spec/PACKAGING.md` §6.4 = **intentional "NOT a channel"**;
      `spec/DEVICE_DISCOVERY.md` L272 "promise." = **false positive**.
- [ ] (Either branch) `git status --short src/ spec/` clean; no `docs/vendor/`
      or `plan/` edit.

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed
> to run this verification successfully?"_ — **Yes.** The four exact gate greps
> (with the critical `docs/vendor/` exclusion + the noise-dir exclusion list), the
> expected counts, the intentional-vs-false-positive classification of every spec/
> hit, the S2 dependency, and the no-edit-unless-drift contract are all below.

> **BASELINE ALERT + S2 DEPENDENCY.** S1 (`docs/installation.md`) is Complete and
> verified clean (0 mise|asdf). S2 (`docs/llms_full.txt` regeneration) is in
> progress in parallel; until it lands, `docs/llms_full.txt` still shows 14
> mise|asdf hits + 4 `asdf-qmkonnect` links (the PRE-S2 state). This task runs
> AFTER S2, so at its runtime llms_full.txt is clean. If S2 hasn't landed when
> this task runs, gate (b) fails — that's an S2-pending state, NOT a finding for
> this task (re-run after S2 lands). All other gates are independent of S2.

### Documentation & References

```yaml
# MUST READ — the audit that established "everything except installation.md/llms_full.txt is already synced"
- file: /home/dustin/projects/qmkonnect/plan/008_2cb9c1ec28a2/architecture/stale_content_audit.md
  why: "§3 ('All Other Files — ALREADY SYNCED') is the table this task re-confirms: spec/PACKAGING.md §6.4 +
        spec/PRD.md F15 = intentional 'NOT a channel'; README.md = 0 hits; release.yml = 0; packaging/asdf/ =
        removed; other docs/*.md = 0. §1-§2 enumerate the ONLY stale artifacts (installation.md 3 blocks;
        llms_full.txt 14 hits) — both owned by S1/S2, not this task. §4 documents the generator (concatenates
        8 authored files, NEVER reads docs/vendor/)."
  section: "3. All Other Files — ALREADY SYNCED" (+ 1, 2, 4)
  critical: "The §3 table is the assertion this task independently re-confirms. Agreement => VERIFIED.
             docs/vendor/ is NOT a real doc surface (Ruby gems) — the §3 audit and the generator both ignore it."

# MUST READ — the S2 contract (the parallel task this depends on)
- file: /home/dustin/projects/qmkonnect/plan/008_2cb9c1ec28a2/P1M1T1S2/PRP.md
  why: "S2 regenerates docs/llms_full.txt via `bash docs/generate_llms_full.sh`, making both
        `grep mise|asdf docs/llms_full.txt` and `grep asdf-qmkonnect docs/llms_full.txt` return 0.
        This task's gate (b) IS the S2 success criterion re-confirmed at the changeset level. If S2
        hasn't landed, gate (b) fails — wait for S2, don't hand-edit llms_full.txt."
  section: "Goal (Success Definition)" + "Validation"
  critical: "Do NOT hand-edit docs/llms_full.txt — it's GENERATED. If it's stale, S2 regenerates it;
             this task only VERIFIES the post-S2 state."

# MUST READ — the S1 contract (the completed prerequisite)
- file: /home/dustin/projects/qmkonnect/plan/008_2cb9c1ec28a2/P1M1T1S1/PRP.md
  why: "S1 deleted the 3 mise/asdf blocks from docs/installation.md (Complete). Confirms docs/installation.md
        is already 0 mise|asdf / 0 asdf-qmkonnect. This task re-confirms S1's gate as part of the repo-wide sweep."

# MUST READ — the intentional "NOT a channel" spec passages (so they're classified correct, not drift)
- file: /home/dustin/projects/qmkonnect/spec/PACKAGING.md
  why: "§6.4 'mise / asdf — NOT a channel (category mismatch)' is the authoritative decision doc. Its 5
        mise|asdf hits are INTENTIONAL — they document WHY mise/asdf is excluded (no autostart; 'switch
        versions' is meaningless; updates re-wire autostart), not advertise the channel. A naive grep flags
        them; this task classifies them as correct."
  section: "6.4 mise / asdf — NOT a channel"
- file: /home/dustin/projects/qmkonnect/spec/PRD.md
  why: "§2.1 (L97 'mise/asdf … explicitly NOT a channel'), F15 (L152 'mise/asdf are a category mismatch
        and are NOT a channel'), §5 (L169 'mise/asdf are not channels'). All intentional exclusion-decision
        docs. Correct, not drift."

# REFERENCE — the files under verification (read/grep before concluding)
- file: /home/dustin/projects/qmkonnect/README.md
  why: "The Package-Managers table must have NO mise/asdf row. Research: 0 mise|asdf hits (already clean).
        Spot-read the install/distribution table to confirm."
- file: /home/dustin/projects/qmkonnect/docs/installation.md
  why: "S1's output — must be 0 mise|asdf / 0 asdf-qmkonnect (verified clean this session)."
- file: /home/dustin/projects/qmkonnect/docs/llms_full.txt
  why: "S2's output — must be 0 mise|asdf / 0 asdf-qmkonnect POST-S2. (Pre-S2: 14 + 4 — S2-pending.)"

# REFERENCE — research notes (the verified gate greps + classification + S2 dependency)
- docfile: /home/dustin/projects/qmkonnect/plan/008_2cb9c1ec28a2/P1M1T2S1/research/notes.md
  why: "The ground-truth grep results, the docs/vendor/ false-positive analysis (60 files), the
        spec/ intentional-vs-false-positive classification (DEVICE_DISCOVERY L272 = 'promise.'), the
        S2 dependency, and the VERIFIED verdict."
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/
├── README.md                         # VERIFY (Package-Managers table — no mise/asdf row)
├── docs/
│   ├── installation.md               # VERIFY (S1 output — 0 mise|asdf)
│   ├── llms_full.txt                 # VERIFY (S2 output — 0 mise|asdf + 0 asdf-qmkonnect, POST-S2)
│   ├── index.md, qmk-integration.md, configuration.md, usage.md, examples.md, troubleshooting.md
│   │                                 # VERIFY (authored — 0 mise|asdf each)
│   └── vendor/                       # EXCLUDE (Ruby Jekyll bundle — 60 false-positive files; NOT QMKonnect content)
├── spec/
│   ├── PRD.md                        # CLASSIFY (§2.1/F15/§5 — intentional "NOT a channel")
│   ├── PACKAGING.md                  # CLASSIFY (§6.4 — intentional "NOT a channel")
│   └── DEVICE_DISCOVERY.md           # CLASSIFY (L272 "promise." — FALSE POSITIVE)
├── src/                              # OUT OF SCOPE (linux_tray.rs/tray.rs "promise" false positives; not docs)
└── packaging/asdf/                   # CONFIRM ABSENT (removed — must not exist)
```

### Desired Codebase tree with files to be modified

```bash
# NO-DRIFT BRANCH (expected): NOTHING modified. Pure verification.
#   git status --short   → clean (no edits anywhere).
# DRIFT-FOUND BRANCH (only if a mise/asdf ad or dead link is in a file S1/S2 didn't own):
#   docs/<overview>.md   # edited IN PLACE (Mode B) to remove the stray ref
#   (src/, spec/, docs/vendor/, plan/ are NEVER touched by this task)
```

### Known Gotchas of our codebase & Library Quirks

```text
# CRITICAL: docs/vendor/ is a 60-file false-positive mine — EXCLUDE it from the gate.
#   docs/vendor/bundle/ruby/3.4.0/gems/... (the Jekyll site's Ruby deps) matches mise|asdf via
#   "proMISE"/"comPROMISE" and "asdf" in gem test fixtures (kramdown/nokogiri/sass/rouge/...).
#   A naive `grep -rin 'mise\|asdf' docs/` returns ~60+ false-positive FILES. The gate MUST
#   scope to `README.md docs/*.md` (the shell glob does NOT descend into docs/vendor/'s nested
#   dirs) and/or explicitly `grep -v docs/vendor/`. The generator (generate_llms_full.sh) also
#   never reads vendor/ — it's not a real doc surface.

# CRITICAL: the spec/ mise/asdf hits are INTENTIONAL — don't flag them as drift.
#   spec/PACKAGING.md §6.4 + spec/PRD.md §2.1/F15/§5 DOCUMENT the "mise/asdf are NOT a channel"
#   decision. They use the words mise/asdf to say it's excluded. A naive whole-tree grep flags
#   them; they are CORRECT (the item contract: "intentional and correct"). Never edit spec/.

# CRITICAL: "promise" is a false positive — it contains "mise".
#   spec/DEVICE_DISCOVERY.md:272 "promise.", src/linux_tray.rs + src/tray.rs "zero-config promise"
#   all match `mise` via the substring. They are English/prose, NOT channel refs. src/ is out of
#   scope (docs verification); the DEVICE_DISCOVERY hit is a substring accident.

# CRITICAL: docs/llms_full.txt is GENERATED — never hand-edit it.
#   It's the output of `bash docs/generate_llms_full.sh` (concatenates 8 authored files). If it's
#   stale, S2 regenerates it. This task only VERIFIES the post-S2 state (gate b). Hand-editing it
#   would be overwritten on the next regeneration and risks merge entropy.

# CRITICAL: this task NEVER edits src/, spec/, docs/vendor/, or plan/.
#   src/ false positives + spec/ intentional refs are out of scope. docs/vendor/ is third-party.
#   plan/ research artifacts (which DO contain asdf-qmkonnect in their prose) are gitignored
#   planning notes — correctly excluded from gate (c). `git status --short src/ spec/` clean.

# GOTCHA: gate (c) excludes plan/ — the asdf-qmkonnect hits there are research notes, not shipped.
#   The plan/008 research (notes/PRPs) extensively references asdf-qmkonnect (it's the subject of
#   the prior plan/007 asdf work + this plan's audit). Those are planning artifacts, not product
#   content; excluding plan/ from the dead-link gate is correct (the item contract lists
#   .pi-subagents/ but plan/ is the same category — gitignored working area).

# GOTCHA: this task DEPENDS ON S2. If S2 hasn't landed, gate (b) fails on the pre-S2 state.
#   That is NOT a finding for this task — it means S2 is still in progress. Re-run gate (b) after
#   S2 lands. Do NOT hand-fix llms_full.txt to make the gate pass.

# NOTE: agreement with stale_content_audit.md §3 IS the success signal.
#   §3 asserts "All Other Files — ALREADY SYNCED." This task independently re-confirms it. If the
#   gate greps agree (0 user-doc hits; spec/ hits classified intentional/false-positive), verdict
#   = VERIFIED. A real mise/asdf AD or dead link in an authored doc S1/S2 didn't touch would be a
#   genuine finding (Mode-B in-place fix) — research says none exist.

# NOTE: there is no build/test for this task. It's grep + spot-read + classify. The deliverable is
#   the verdict + evidence summary.
```

## Implementation Blueprint

### Data models and structure

Not applicable — no data models, no code. This is a read-and-verify sweep that
produces a one-line verdict (+ optional in-place doc fix only in the drift-found
branch).

### Implementation Tasks (ordered: gate greps → classify → verdict → optional fix)

```yaml
Task 1: CONFIRM prerequisites (S1 done; S2 done OR pending)
  - RUN: grep -ic 'mise\|asdf' docs/installation.md   # S1 gate — expect 0 (S1 Complete).
  - RUN: grep -ic 'asdf-qmkonnect' docs/installation.md  # expect 0.
  - CHECK S2: grep -ic 'mise\|asdf' docs/llms_full.txt   # if >0, S2 hasn't landed yet.
  - IF llms_full.txt >0: S2 is still in progress — run Tasks 2,3,4 (they're S2-independent)
          and NOTE gate (b) as S2-pending; do NOT hand-edit llms_full.txt. Re-run gate (b) post-S2.
  - IF llms_full.txt ==0: S2 has landed — proceed with all four gates.

Task 2: GATE (a) — authored user docs have ZERO mise/asdf channel advertising
  - RUN: grep -rin 'mise\|asdf' README.md docs/*.md
  - EXPECT: 0 hits. (The docs/*.md glob does NOT descend into docs/vendor/; llms_full.txt is not .md.)
  - IF a hit appears: it's either (i) a real stray mise/asdf ad in an authored doc S1/S2 didn't
          own → Task 5 (Mode-B in-place fix); or (ii) a false positive ("promise") → classify + note.

Task 3: GATE (c) — dead asdf-qmkonnect links repo-wide (the strongest signal)
  - RUN: grep -rn 'asdf-qmkonnect' . | grep -vE '\.git/|/target/|node_modules/|\.pi-subagents/|/plan/|docs/vendor/'
  - EXPECT: 0 hits. (Excludes plan/ research artifacts + docs/vendor/ gems + build/tool dirs.)
  - IF a hit appears in an authored doc or spec/: Task 5 (remove the dead link in place).
  - CONFIRM: ls packaging/asdf/ 2>/dev/null → "No such file or directory" (the dir is removed).

Task 4: CLASSIFY the spec/ mise/asdf hits (correct vs false positive)
  - RUN: grep -in 'mise\|asdf' spec/PRD.md spec/PACKAGING.md spec/DEVICE_DISCOVERY.md
  - EXPECT + CLASSIFY:
        spec/PRD.md L97/L152/L169 → INTENTIONAL ("NOT a channel" — §2.1 Goals, F15 row, §5 channels).
        spec/PACKAGING.md (§6.4, ~5 hits) → INTENTIONAL ("mise / asdf — NOT a channel (category mismatch)").
        spec/DEVICE_DISCOVERY.md L272 "promise." → FALSE POSITIVE (English word ⊃ "mise").
  - RECORD the classification in the verdict evidence. These are NOT drift; do NOT edit spec/.

Task 5: (DRIFT-FOUND BRANCH ONLY) remove an unexpected stale ref in place
  - ONLY if Task 2 or 3 found a mise/asdf channel-advertising ref OR a dead asdf-qmkonnect link in
          a file NOT owned by S1/S2 (e.g. docs/usage.md, docs/examples.md): edit that doc IN PLACE
          (Mode B) to remove it. Mirror the spec/PACKAGING.md §6.4 "NOT a channel" stance if a
          replacement mention is needed (likely just delete the stale line/block).
  - DO NOT edit src/, spec/, docs/vendor/, docs/llms_full.txt, or plan/.
  - Research baseline: this branch is NOT reached — no stray refs exist outside S1/S2's files.

Task 6: GATE (b) — docs/llms_full.txt is clean (POST-S2; re-confirm S2's success criterion)
  - RUN (only meaningful once S2 has landed): grep -in 'mise\|asdf' docs/llms_full.txt        # expect 0
  - RUN:                                                grep -in 'asdf-qmkonnect' docs/llms_full.txt  # expect 0
  - IF >0 and S2 hasn't landed: S2-pending (see Task 1) — not a finding here.
  - IF >0 and S2 HAS landed: S2 regressed — flag back to S2 (do NOT hand-fix here).

Task 7: GATE (d) + README spot-read, then STATE THE VERDICT
  - RUN: grep -in 'mise\|asdf' README.md   # expect 0.
  - SPOT-READ: README.md Package-Managers / distribution table → confirm no mise/asdf row.
  - WRITE the verdict to plan/008_2cb9c1ec28a2/P1M1T2S1/research/verdict.md:
        "VERIFIED: no documentation drift"  (expected)
        + evidence: gate (a)=0, (b)=0/0 post-S2, (c)=0, (d)=0; spec/ classified
          (PRD+PACKAGING intentional; DEVICE_DISCOVERY "promise" false positive); packaging/asdf/ absent.
        OR (drift-found): the file/line edited + before→after.
  - RE-RUN: git status --short src/ spec/  → MUST be clean. git status --short docs/vendor/ plan/ → clean.
```

### Implementation Patterns & Key Details

```text
# === WHY docs/*.md (glob) excludes docs/vendor/ ===
#   Shell globs don't recurse: `docs/*.md` matches only files directly in docs/, not
#   docs/vendor/bundle/.../*.rb. So `grep -rin 'mise\|asdf' README.md docs/*.md` cleanly
#   scopes to the 7 authored doc files + README, sidestepping the 60 vendor false positives.
#   (If you instead use `grep -r ... docs/`, you MUST add `| grep -v docs/vendor/`.)

# === THE INTENTIONAL spec/ REFERENCES (the project's "we decided NOT to" record) ===
#   spec/PACKAGING.md §6.4 + spec/PRD.md §2.1/F15/§5 exist to DOCUMENT the mise/asdf exclusion.
#   They say "mise/asdf are NOT a channel because …". Removing them would DELETE the decision
#   record. They are correct. A naive grep flags them; classify + leave them.

# === THE asdf-qmkonnect DEAD-LINK gate is the strongest signal ===
#   mise|asdf has false positives ("promise", Ruby gems). `asdf-qmkonnect` is an exact,
#   unambiguous token — any hit outside plan/ research + docs/vendor/ is a real dead link
#   to the removed plugin repo. Gate (c) is the cleanest pass/fail.

# === WHY plan/ IS EXCLUDED from gate (c) ===
#   plan/008's own research (notes/PRPs) + plan/007's asdf-plugin work extensively reference
#   asdf-qmkonnect — it's the SUBJECT of the audit. Those are gitignored planning artifacts,
#   not shipped product content. Excluding plan/ (like .pi-subagents/) is correct.

# === THE VERDICT IS THE DELIVERABLE ===
#   "VERIFIED: no documentation drift" + evidence, at plan/.../P1M1T2S1/research/verdict.md.
#   No src/spec edit. No doc edit (no-drift branch). git status clean.
```

### Integration Points

```yaml
SOURCE FILES:
  - verify (grep + read): "README.md, docs/*.md (authored), docs/llms_full.txt"
  - classify (no edit): "spec/PRD.md, spec/PACKAGING.md, spec/DEVICE_DISCOVERY.md"
  - EXCLUDE from all gates: "docs/vendor/ (Ruby gems), plan/ (research), src/ (out of scope)"
  - NEVER modify: "src/, spec/, docs/vendor/, docs/llms_full.txt (generated), plan/"

DEPENDENCIES / BUILD:
  - none. Pure grep + read. No cargo, no build, no tests.

UPSTREAM CONTEXT:
  - S1 (Complete): docs/installation.md cleaned (3 mise/asdf blocks deleted).
  - S2 (parallel): docs/llms_full.txt regenerated (gate b is S2's success criterion re-confirmed).
  - stale_content_audit.md §3: "the 'all other files synced' assertion this task re-confirms."

DOWNSTREAM CONSUMERS:
  - The orchestrator reads verdict.md to flip P1.M1.T2.S1 → Complete and P1.M1 → Done
    (the mise/asdf doc-removal changeset is verified-complete). A VERIFIED verdict + clean
    git status closes the changeset.

OUT OF SCOPE:
  - Editing src/, spec/, docs/vendor/, plan/, or docs/llms_full.txt under any branch.
  - Re-running S1/S2 (they own installation.md / llms_full.txt).
  - Fixing the "promise" false positives (they're correct English/prose, not refs).
```

## Validation Loop

> The Validation Loop for THIS subtask IS the four gate greps (Tasks 2, 3, 6, 7) +
> the spec/ classification (Task 4) + the no-src/spec-edit invariant. No build, no tests.

### Level 1: The gate greps reproduce "clean" (the verification itself)

```bash
cd /home/dustin/projects/qmkonnect

# (a) Authored user docs — ZERO mise/asdf channel advertising.
grep -rin 'mise\|asdf' README.md docs/*.md
# Expected: no output. (docs/*.md glob excludes docs/vendor/; llms_full.txt is not .md.)
# Any hit => classify: real ad (Task 5) or false positive ("promise" — note + leave).

# (b) Generated artifact — ZERO mise/asdf + ZERO dead links (POST-S2).
grep -in 'mise\|asdf' docs/llms_full.txt          # Expected: 0 (post-S2).
grep -in 'asdf-qmkonnect' docs/llms_full.txt      # Expected: 0 (post-S2).
# If >0 and S2 hasn't landed => S2-pending (not a finding). If >0 post-S2 => S2 regressed.

# (c) Dead asdf-qmkonnect links repo-wide (excl noise + plan research + vendor).
grep -rn 'asdf-qmkonnect' . | grep -vE '\.git/|/target/|node_modules/|\.pi-subagents/|/plan/|docs/vendor/'
# Expected: no output. Any hit in an authored doc/spec => Task 5.

# (d) README — no mise/asdf row in the Package-Managers table.
grep -in 'mise\|asdf' README.md                   # Expected: no output.

# packaging/asdf/ must be ABSENT (removed).
ls packaging/asdf/ 2>/dev/null && echo "STILL EXISTS (regression!)" || echo "absent (correct)"
# Expected: "absent (correct)".
```

### Level 2: The spec/ classification (correct vs false positive)

```bash
cd /home/dustin/projects/qmkonnect

# Every spec/ mise|asdf hit must be classified — intentional "NOT a channel" OR false positive.
grep -in 'mise\|asdf' spec/PRD.md spec/PACKAGING.md spec/DEVICE_DISCOVERY.md
# Expected + classify:
#   spec/PRD.md:97,152,169            -> INTENTIONAL (§2.1 Goals, F15 row, §5 channels: "NOT a channel")
#   spec/PACKAGING.md (§6.4, ~5 hits) -> INTENTIONAL ("mise / asdf — NOT a channel (category mismatch)")
#   spec/DEVICE_DISCOVERY.md:272      -> FALSE POSITIVE ("promise." — English word ⊃ "mise")
# Record this classification in the verdict. Do NOT edit spec/.
```

### Level 3: No-edit invariant (verification, not implementation)

```bash
cd /home/dustin/projects/qmkonnect
git status --short src/ spec/
# Expected: empty (clean). A docs verification must not modify code or spec.
git status --short docs/vendor/ plan/
# Expected: empty. Third-party gems + planning artifacts are never touched.
# No-drift branch: git status --short (everything) is clean — pure verification.
# Drift-found branch: ONLY the one authored doc edited in place appears.
```

### Level 4: Drift-found triage (ONLY if a gate found a real ref)

```text
If gate (a) or (c) flagged a mise/asdf channel-advertising ref or a dead asdf-qmkonnect link in
a file NOT owned by S1/S2 (e.g. docs/usage.md, docs/examples.md, README.md):
1. Re-read the surrounding context — is it a genuine channel ad / dead link, or a benign mention
   (e.g. "mise/asdf are not supported" decision prose, like the spec/ intentional refs)? Benign
   decision-prose => classify + leave (not drift).
2. If it IS a genuine stale channel ad / dead link: edit the authored doc IN PLACE (Mode B) —
   delete the stale line/block (or replace with a "NOT a channel" note mirroring spec/PACKAGING.md
   §6.4 if context requires). NEVER edit src/, spec/, docs/vendor/, docs/llms_full.txt, or plan/.
3. Re-run the gate that flagged it -> confirm 0.
4. Record the before→after in verdict.md.
(Research baseline: this level is NOT reached — no stray refs exist outside S1/S2's files.)
```

## Final Validation Checklist

### Technical Validation

- [ ] Gate (a): `grep -rin 'mise\|asdf' README.md docs/*.md` → 0 hits.
- [ ] Gate (b): `docs/llms_full.txt` → 0 mise|asdf AND 0 asdf-qmkonnect (post-S2).
- [ ] Gate (c): repo-wide `asdf-qmkonnect` (excl noise + plan + vendor) → 0 hits.
- [ ] Gate (d): `grep -in 'mise\|asdf' README.md` → 0; Package-Managers table has no mise/asdf row.
- [ ] `packaging/asdf/` absent (removed).
- [ ] Level 2: spec/ hits classified (PRD+PACKAGING intentional; DEVICE_DISCOVERY "promise" FP).
- [ ] Level 3: `git status --short src/ spec/ docs/vendor/ plan/` clean.

### Feature Validation

- [ ] Verdict stated explicitly: **`VERIFIED: no documentation drift`** (expected) OR the
      specific file/line edited (drift-found).
- [ ] (No-drift) `git status --short` clean — nothing modified (pure verification).
- [ ] (Drift-found) only the one authored doc edited; re-gate clean.
- [ ] The intentional spec/ "NOT a channel" references are explicitly noted as correct.

### Code Quality Validation

- [ ] No `src/`, `spec/`, `docs/vendor/`, `docs/llms_full.txt`, or `plan/` file modified.
- [ ] The verdict + evidence captured under `plan/.../P1M1T2S1/research/verdict.md`.

### Documentation & Deployment

- [ ] DOCS = Mode B. No-drift ⇒ no doc changes (the expected result). Drift-found ⇒ in-place
      fix to the one affected authored doc only.
- [ ] The mise/asdf removal changeset is verified-complete across code/spec/user-docs.

---

## Anti-Patterns to Avoid

- ❌ Don't run `grep -rin 'mise\|asdf' docs/` without excluding `docs/vendor/` — the Ruby Jekyll
  bundle has **60 false-positive files** ("pro**mise**", "asdf" in gem test fixtures). Scope to
  `README.md docs/*.md` (the glob doesn't recurse into vendor/) or add `| grep -v docs/vendor/`.
  A naive grep makes a clean tree look dirty.
- ❌ Don't flag the `spec/PRD.md` + `spec/PACKAGING.md` mise/asdf hits as drift — they are
  INTENTIONAL. They DOCUMENT the "mise/asdf are NOT a channel" decision (§6.4, F15, §2.1, §5).
  Removing them would delete the decision record. Classify them as correct; never edit spec/.
- ❌ Don't be fooled by "promise" — it contains "mise". `spec/DEVICE_DISCOVERY.md:272` +
  `src/*.rs` "zero-config promise" are English/prose false positives, not channel refs. src/ is
  out of scope anyway (docs verification).
- ❌ Don't hand-edit `docs/llms_full.txt` — it's GENERATED (`generate_llms_full.sh`). If it's
  stale, S2 regenerates it; this task only VERIFIES the post-S2 state. Hand-editing is
  overwritten on the next regen and risks merge entropy.
- ❌ Don't treat a failing gate (b) as a finding if S2 hasn't landed — `docs/llms_full.txt` shows
  14 mise|asdf + 4 asdf-qmkonnect hits in the PRE-S2 state. That's S2-pending, not drift. Re-run
  gate (b) after S2; don't hand-fix llms_full.txt to force a pass.
- ❌ Don't exclude `plan/` from your mental model but INCLUDE it in gate (c) — the plan/008
  research + plan/007 asdf work legitimately reference `asdf-qmkonnect` extensively (it's the
  audit's subject). Those are gitignored planning artifacts; gate (c) excludes `plan/` (same
  category as `.pi-subagents/`). A hit there is expected, not drift.
- ❌ Don't edit `src/`, `spec/`, `docs/vendor/`, or `plan/` under any branch — this is a docs
  verification. `git status --short src/ spec/` must stay clean. Even a "real" drift is fixed in
  the authored doc only.
- ❌ Don't conflate this with S1/S2 — they own `docs/installation.md` and `docs/llms_full.txt`.
  This task is the repo-wide belt-and-suspenders sweep + the spec/ classification. If gate (a)'s
  installation.md check fails, that's an S1 regression (flag it, don't re-do S1 here).
- ❌ Don't declare drift without triage — a `mise|asdf` hit is usually a false positive
  ("promise") or an intentional spec decision-prose. Read the context; only a genuine channel
  AD or dead `asdf-qmkonnect` link in an authored doc is a Mode-B fix.
- ❌ Don't commit the verdict artifact into src/spec/docs — it lives under
  `plan/.../P1M1T2S1/research/verdict.md` (plan research area).

---

**Confidence Score: 10/10** for one-pass execution success. This is a docs-side verification whose
four gate greps were run directly during research (authored docs = 0 mise|asdf; llms_full.txt clean
post-S2; repo-wide `asdf-qmkonnect` = 0 excl plan/vendor; README = 0), reproducing
`stale_content_audit.md` §3's "All Other Files — ALREADY SYNCED" assertion. The deliverable is a
one-line verdict ("VERIFIED: no documentation drift") + evidence; no build, no tests, no code/spec
edits. The three residual risks are all pre-empted: (1) the `docs/vendor/` 60-file false-positive
mine is excluded by the `docs/*.md` glob (called out as the #1 gotcha); (2) the intentional
`spec/PRD.md`+`spec/PACKAGING.md` "NOT a channel" references are explicitly classified correct (not
drift); (3) the S2 dependency is stated — a failing gate (b) pre-S2 is S2-pending, not a finding,
and `docs/llms_full.txt` is never hand-edited. The drift-found branch is documented for completeness
but research confirms it is not reached: no mise/asdf channel ad or dead `asdf-qmkonnect` link
survives outside the two files S1/S2 own.