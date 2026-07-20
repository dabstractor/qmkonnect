# PRP — P1.M1.T1.S3: Full-tree verification grep and cargo check

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`) — THIS repo.
> **Scope:** A read-only VERIFICATION pass. Run 5 repo-wide greps + one
> `cargo check`; confirm the v0.2.4→v0.2.8 naming-drift remediation (S1 source
> fix + S2 generated-mirror regen) has cleared the entire *product* tree; write a
> verification report. **No source, doc, or build change is the intended
> outcome** — if every check passes, this task touches NOTHING in the product
> tree. The only artifact produced is a report under `plan/`.
> **Task type:** Mode C verification gate (the gate that closes P1.M1.T1).

---

## Goal

**Feature Goal**: Prove — by deterministic, re-runnable commands — that the
v0.2.8 naming-convention drift (`qmk-notifier_notify`, `package = "qmk_notifier"`,
`tag = "v0.2.1"`, `build-installer.ps1` in CI, `config/qmk-notifier` config path)
has been eliminated from the **shipped product tree** (everything except the
planning artifacts in `plan/`, the build cache in `target/`, vendored Jekyll in
`docs/vendor/`, git metadata in `.git/`, and subagent scratch in
`.pi-subagents/`). AND that the crate still compiles cleanly
(`cargo check --bin qmkonnect --offline`, no errors/warnings).

**Deliverable**: A verification report at
`plan/003_7059790d6c5b/P1M1T1S3/research/verification_report.md` recording, for
each of the 6 checks (5 greps + cargo check): the exact command run, the expected
result, the observed result, and pass/fail. The report must also record the two
**known, accepted observations** (see Context): (1) `installer.wxs` does not
exist (removed in `cb9a165`; the contract's "explicitly-retained" clause is
stale), (2) `spec/PACKAGING.md:88,232` reference the removed WiX tooling but are
**out of scope** of contract grep (d) (which is `.github/`-scoped).

**Success Definition**:
- All 5 contract greps return **zero** product-tree hits (grep exit code 1).
- `cargo check --bin qmkonnect --offline` exits **0** with **no warnings**.
- The verification report exists, names all 6 checks, and marks each pass/fail
  with observed output.
- **If any product-tree stale reference is found** (a genuine, unexpected hit),
  it is FIXED before this task is marked complete (per the contract OUTPUT
  clause: "If any stale reference is found, it must be fixed before this subtask
  is complete"). Documenting a real hit without fixing it = failure.

## User Persona (if applicable)

**Target User**: The release/quality engineer (human or orchestrator agent)
who needs a signed-off guarantee that the v0.2.8 delta is internally consistent
before bumping/tagging. Also future agents that re-derive the tree state.

**Use Case**: After S1 (source fix) + S2 (generated mirror regen) land, run a
single deterministic verification pass that either confirms the whole tree is
clean or surfaces any remaining drift — so P1.M1.T1 ("Fix residual doc drift and
verify clean tree") can be closed with evidence.

**Pain Points Addressed**: Without a scoped, exclusion-aware grep pass, the
remediation looks unfinished: a naive `grep -rn 'qmk-notifier_notify' .` returns
~30 hits, all in `plan/` (the PRPs/architecture docs that *describe* the fix) and
`.pi-subagents/artifacts/` (cached research transcripts) — none in the product
tree. The scoping makes the verification meaningful rather than noisy.

## Why

- **Closes the verification gate for P1.M1.T1.** S1 fixed the source
  (`docs/troubleshooting.md:647`); S2 regenerated the mirror
  (`docs/llms_full.txt:2622`). S3 is the independent, full-tree confirmation that
  no stale token survived anywhere a user/build/CI would see it.
- **The greps are scoped for a reason.** The `plan/` directory legitimately
  contains `qmk-notifier_notify` (the PRPs quote the old form in before/after
  diffs; `delta_verification.md` documents the drift it found). The exclusions
  isolate the greps to the *shipped product tree* — where a stale token would
  actually mislead. Without exclusions the verification is meaningless noise.
- **`cargo check` guards the build.** The naming drift was cosmetic (docs only),
  so the build is expected to be unaffected — but the contract requires
  confirming it compiles cleanly (`--offline` ⇒ uses the cached `qmk-notifier`
  v0.3.0 git dep; no network). This is the cheap, deterministic proof the
  remediation didn't accidentally touch anything that breaks compilation.
- **Produces an auditable artifact.** The report is the evidence P1.M1.T1 is
  done; future re-verifications can re-run the exact commands and diff.

## What

Run 6 read-only commands from the repo root, capture each one's output/exit code,
and write a report. **Do not modify any product-tree file** unless a check
genuinely FAILS (an unexpected stale hit in src/, docs/, spec/, Cargo.toml,
.github/, packaging/, README, etc.) — in which case fix it and re-run.

### The 6 checks (exact commands, exact scopes)

> All run from `/home/dustin/projects/qmkonnect`. The exclusion filter
> `grep -vE` is applied to greps that scan the whole repo (`a`, `b`, `c`, `e`);
> grep (d) is scoped to `.github/` and needs no filter.

```bash
# Common exclusion filter (product tree only): drop planning/build/vendor/git/subagent scratch.
# Define once, reuse for the repo-wide greps.
EXCL='\.pi-subagents/|/target/|docs/vendor/|/\.git/|/plan/'

# (a) stale firmware-callback token — the S1/S2 fix target. ZERO expected.
grep -rn 'qmk-notifier_notify' . | grep -vE "$EXCL"

# (b) old crate self-declaration (pre-v0.2.8 the crate was named qmk_notifier). ZERO expected.
grep -rn 'package = "qmk_notifier"' --include='*.toml' . | grep -vE "$EXCL"

# (c) old crate git tag (v0.2.1 → v0.3.0 bump). ZERO expected.
grep -rn 'tag = "v0.2.1"' --include='*.toml' . | grep -vE "$EXCL"

# (d) legacy WiX build script referenced by OLD CI. .github/-scoped. ZERO expected.
grep -rn 'build-installer.ps1' .github/

# (e) old config directory path (qmk-notifier/ → qmkonnect/). ZERO expected.
grep -rn 'config/qmk-notifier' --include='*.rs' --include='*.md' . | grep -vE "$EXCL"

# (f) the build gate. Exit 0, NO warnings.
cargo check --bin qmkonnect --offline
```

### Success Criteria

- [ ] Check (a): `grep -rn 'qmk-notifier_notify' . | grep -vE "$EXCL"` → no output.
- [ ] Check (b): `grep -rn 'package = "qmk_notifier"' --include='*.toml' . | grep -vE "$EXCL"` → no output.
- [ ] Check (c): `grep -rn 'tag = "v0.2.1"' --include='*.toml' . | grep -vE "$EXCL"` → no output.
- [ ] Check (d): `grep -rn 'build-installer.ps1' .github/` → no output.
- [ ] Check (e): `grep -rn 'config/qmk-notifier' --include='*.rs' --include='*.md' . | grep -vE "$EXCL"` → no output.
- [ ] Check (f): `cargo check --bin qmkonnect --offline` → exit 0, zero warnings.
- [ ] Report written to `plan/003_7059790d6c5b/P1M1T1S3/research/verification_report.md`
      documenting all 6 checks (command, expected, observed, pass/fail) + the two
      known observations.
- [ ] IF any check fails with a genuine product-tree hit: the stale reference is
      fixed and ALL checks are re-run green before completion.

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed
> to implement this successfully?"_ — **Yes.** The exact 6 commands (with exact
> scopes and the exact exclusion regex), the expected result for each, the
> baseline-confirmed state (all pass today), the two known discrepancies and how
> to treat them, the report path and required contents, and the fix-or-fail
> policy are all below. No codebase exploration is required beyond running the 6
> commands and reading their output.

> **BASELINE ALERT — read this first.** As of this PRP's authoring, the working
> tree is **already fully clean**: S1 (`docs/troubleshooting.md:647`) and S2
> (`docs/llms_full.txt:2622`) have BOTH landed (both read `qmk_notifier_notify`).
> All 6 checks were run during research and **PASS** (every grep returns exit 1;
> `cargo check --offline` exits 0 in 0.13s with no warnings). So the expected
> outcome of this task is: run the 6 commands, confirm they're still green,
> write the report, touch nothing. The fix-or-fail branch is unlikely to trigger
> — but the agent must still run every check and record evidence, not skip on
> assumption.

### Documentation & References

```yaml
# MUST READ — the verification doc that defines the "clean grep" contract
- file: /home/dustin/projects/qmkonnect/plan/003_7059790d6c5b/architecture/delta_verification.md
  why: "§'Clean Grep Results (confirmed zero hits)' is the origin of greps (b)-(e):
        package = \"qmk_notifier\" (none), tag = \"v0.2.1\" (none), build-installer.ps1
        in .github/ (none), qmk-notifier/ as config path (only legitimate tree label in
        HOST_RULES.md:563). §'Residual Drift' items #1/#2 are the S1/S2 targets that
        grep (a) confirms cleared."
  section: "Clean Grep Results" and "Residual Drift (Actionable)"
  critical: "This is the authoritative contract for WHICH greps, WHICH scopes, and
             WHAT 'clean' means. Re-run exactly these greps (plus the S1/S2 token grep)
             with the exclusions, not ad-hoc variants."

# MUST READ — the upstream PRP whose output S3 consumes (S1: the source fix)
- file: /home/dustin/projects/qmkonnect/plan/003_7059790d6c5b/P1M1T1S1/PRP.md
  why: "Defines the S1 deliverable: docs/troubleshooting.md:647 qmk-notifier_notify →
        qmk_notifier_notify. S3's grep (a) confirms this fix (and S2's mirror) left zero
        stale hits in the product tree."
  section: "What" (exact line edit) and "Integration Points" (names S3 as downstream verifier)
  critical: "If S1 somehow regressed (source reverted to hyphen), grep (a) would hit
             docs/troubleshooting.md — that is a real failure: re-apply the S1 fix."

# MUST READ — the upstream PRP whose output S3 consumes (S2: the generated mirror)
- file: /home/dustin/projects/qmkonnect/plan/003_7059790d6c5b/P1M1T1S2/PRP.md
  why: "Defines the S2 deliverable: docs/llms_full.txt regenerated so line ~2622 mirrors
        the corrected qmk_notifier_notify. S3's grep (a) confirms the .txt mirror is
        also clean."
  section: "Goal" + "Integration Points" (names S3's repo-wide grep as the gate S2 unblocks)
  critical: "If S2 didn't land (llms_full.txt still hyphen at ~2622), grep (a) would hit
             docs/llms_full.txt — re-run docs/generate_llms_full.sh (per S2 PRP)."

# REFERENCE — the naming convention the whole verification enforces
- file: /home/dustin/projects/qmkonnect/PRD.md
  why: "§1.1 'The broader ecosystem' table + 'Naming hazard' callout + §13 Glossary define
        the convention every grep encodes: qmk_notifier (underscore) = firmware C module;
        qmk-notifier (hyphen) = Rust transport crate (v0.3.0). A firmware callback token
        is underscore; the crate package/repo/tag is hyphen; config dir is qmkonnect/."
  section: "1.1 The broader ecosystem" and "13. Glossary"

# REFERENCE — legitimate tree labels that must NOT trip a "fix"
- file: /home/dustin/projects/qmkonnect/spec/HOST_RULES.md
  why: "Line ~563 has a file-tree diagram labeling the two external repos:
        'qmk-notifier/  (external crate)' and 'qmk_notifier/  (external firmware)'.
        These are CORRECT labels (hyphen = crate repo, underscore = firmware repo),
        not config paths or package declarations. None of the 5 greps match them
        (grep e is 'config/qmk-notifier' — no 'config/' prefix here; grep b is
        'package = \"qmk_notifier\"' — no 'package =' here). Do NOT 'fix' them."
  section: "file-tree diagram (~line 563)"

# REFERENCE — known out-of-scope legacy WiX doc refs (do NOT treat as failures)
- file: /home/dustin/projects/qmkonnect/spec/PACKAGING.md
  why: "Lines 88 and 232 reference the REMOVED legacy WiX tooling
        ('packaging/windows/installer.wxs + build-installer.ps1 (needs WiX v3)' and
        'build-installer.ps1 is not invoked by CI'). These are stale doc refs to
        tooling removed in commit cb9a165. BUT they are OUT OF SCOPE of contract grep
        (d), which is scoped to .github/ ONLY. A repo-wide grep would hit them; the
        contract scope deliberately excludes them. Record as a known observation, do
        NOT fix here (separate doc-drift work item)."
  section: "lines 88, 232"
  critical: "Do NOT run an UNSCOPED `grep -rn 'build-installer.ps1' .` and then fail
             on spec/PACKAGING.md — that is NOT the contract. Run grep (d) scoped to
             .github/ exactly as specified."

# REFERENCE — proof installer.wxs was removed (explains its absence)
- url: (local git) git show cb9a165 --stat
  why: "Commit 'cb9a165 ci(windows): remove legacy WiX tooling' deleted installer.wxs
        and build-installer.ps1. This is why `find . -name '*.wxs'` returns nothing.
        The S2 PRP / item-description clause 'excluding ... the explicitly-retained
        packaging/windows/installer.wxs legacy file' is STALE — the file no longer
        exists, so excluding it is a no-op. Do NOT fail because it's missing; do NOT
        restore it."
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/                      # THIS repo — root of all greps
├── .github/workflows/          # grep (d) scope: ci.yml, pages.yml, release.yml
├── .pi-subagents/artifacts/    # EXCLUDED — cached subagent transcripts (contain the stale token legitimately)
├── Cargo.toml                  # grep (b)(c) target: line 18 = qmk-notifier v0.3.0 (correct)
├── Cargo.lock                  # confirms qmk-notifier 0.3.0 git source cached (enables --offline)
├── docs/
│   ├── troubleshooting.md      # S1 target (line 647 = qmk_notifier_notify, fixed)
│   ├── llms_full.txt           # S2 target (line ~2622 mirror, regenerated)
│   └── vendor/bundle           # EXCLUDED — vendored Jekyll (build artifact, not source)
├── spec/                       # IN product tree — greps a/e scan it; PACKAGING.md has out-of-scope legacy refs
│   ├── HOST_RULES.md           # :563 legitimate qmk-notifier/ + qmk_notifier/ labels
│   ├── PACKAGING.md            # :88,232 legacy WiX refs (out of grep-d scope — known observation)
│   └── ...                     # ARCHITECTURE/CONFIG/FIRMWARE/LINUX/PACKAGING/PLATFORMS/PRD/PROTOCOL/UI.md
├── src/                        # grep (e) target: config paths use qmkonnect/ (correct)
├── packaging/                  # grep a/e scan it; NO installer.wxs (removed cb9a165)
├── target/                     # EXCLUDED — cargo build cache
├── .git/                       # EXCLUDED — git metadata
├── README.md, release.toml, AGENTS.md, REMAINING_ISSUES.md   # in product tree (scanned)
└── plan/003_7059790d6c5b/      # EXCLUDED — planning artifacts (PRPs/delta docs legitimately contain the stale token)
    ├── architecture/delta_verification.md   # the contract source (mentions qmk-notifier_notify in its evidence)
    ├── delta_prd.md, tasks.json              # mention the stale token in task descriptions
    ├── P1M1T1S1/PRP.md, P1M1T1S2/PRP.md     # quote the old form in before/after diffs
    └── P1M1T1S3/                             # <-- THIS task
        ├── PRP.md                            # <-- this file
        └── research/
            ├── notes.md                      # research notes
            └── verification_report.md        # <-- THE DELIVERABLE (write this)
```

### Desired Codebase tree with files to be added/modified

```bash
plan/003_7059790d6c5b/P1M1T1S3/research/
└── verification_report.md   # NEW — the verification report (ONLY artifact if all checks pass)
```

> If all 6 checks pass, **NO product-tree file is modified** — the only new file
> is the report under `plan/`. (A product-tree file is modified ONLY in the
> fix-or-fail branch, if a genuine stale hit is found.)

### Known Gotchas of our codebase & Library Quirks

```text
# CRITICAL: run the greps WITH the exclusions, or they scream noise.
#   A bare `grep -rn 'qmk-notifier_notify' .` returns ~30 hits — ALL legitimate, in
#   plan/ (the PRPs/architecture docs that QUOTE the old form) and .pi-subagents/
#   (cached research transcripts). None is product drift. The exclusion regex
#   EXCL='\.pi-subagents/|/target/|docs/vendor/|/\.git/|/plan/' isolates the product
#   tree. Forgetting it makes the verification meaningless (everything "fails").

# CRITICAL: grep (d) is .github/-scoped — do NOT widen it.
#   The contract is `grep -rn 'build-installer.ps1' .github/`. An UNSCOPED
#   `grep -rn 'build-installer.ps1' .` would hit spec/PACKAGING.md:88,232 (legacy
#   WiX doc refs). Those are OUT OF SCOPE and are a known/accepted observation, not
#   a failure. Run the command exactly as scoped.

# CRITICAL: installer.wxs does NOT exist — do not treat its absence as an error.
#   The item description and S2 PRP context say to exclude "the explicitly-retained
#   packaging/windows/installer.wxs legacy file." But commit cb9a165 REMOVED the WiX
#   tooling. There is no installer.wxs to exclude (find returns nothing). Excluding a
#   nonexistent path is a harmless no-op. Do NOT fail, do NOT restore/recreate it.
#   This stale clause should be noted in the report under "known observations."

# CRITICAL: this is a VERIFICATION task — default to touching NOTHING.
#   If all 6 checks pass, the deliverable is the REPORT only. Do not "tidy," reword,
#   or refactor anything. Only if a check FAILS (a genuine product-tree stale hit)
#   do you fix that specific reference, then re-run ALL checks green.

# NOTE: cargo check --offline requires the git dep to be cached.
#   Verified during research: `cargo check --bin qmkonnect --offline` exits 0 in
#   0.13s, no warnings — the qmk-notifier v0.3.0 git dep IS in the local cargo
#   cache (Cargo.lock pins it). So --offline works here. IF the environment's cache
#   were wiped (--offline errors with "can't find qmk-notifier in registry" / a git
#   fetch attempt), fall back to plain `cargo check --bin qmkonnect` (online) and
#   document the deviation in the report. Do NOT silently drop --offline without
#   noting it.

# NOTE: 'clean' for cargo check means no WARNINGS either, not just no errors.
#   The contract says "expect clean (no errors/warnings)." A successful build that
#   prints warnings (e.g. unused-import) is a PARTIAL pass — investigate; if a
#   warning was introduced by the remediation, fix it. (Baseline: zero warnings.)

# NOTE: grep exit codes — exit 1 means "no matches" (the SUCCESS case here).
#   For greps, exit 0 = found matches (a FAILURE for checks a-e), exit 1 = no
#   matches (PASS), exit 2 = error (bad regex/path). When piping through
#   grep -vE "$EXCL", the pipe's exit code is the LAST grep's — so check the
#   FILTERED output is empty AND (ideally) confirm with `set -o pipefail` or by
#   capturing both stages, so a hit that the exclusion accidentally kept isn't
#   masked. Simplest robust form:
#       grep -rn 'PATTERN' . > /tmp/raw.txt 2>/dev/null || true
#       grep -vE "$EXCL" /tmp/raw.txt        # this should print nothing
#   (Inspecting the raw vs filtered lists makes the evidence auditable.)

# NOTE: legitimate labels in spec/HOST_RULES.md:563 must be left alone.
#   The file-tree diagram labels 'qmk-notifier/  (external crate)' and
#   'qmk_notifier/  (external firmware)'. Hyphen = Rust crate repo (correct);
#   underscore = firmware repo (correct). None of the 5 greps match them. Do not
#   "fix" them — they already follow the convention.

# NOTE: do NOT fix spec/PACKAGING.md legacy WiX refs in this task.
#   Lines 88, 232 reference removed WiX tooling. They ARE stale, but they are OUT
#   of the contract's grep scope (grep d is .github/-only) and OUT of this task's
#   scope ("DOCS: none — verification pass"). Record them as a known observation;
#   fixing spec/ doc drift is a separate work item. Scope creep here risks
#   confusing the verification gate's meaning.
```

## Implementation Blueprint

### Data models and structure

Not applicable — no data models, types, or code. The "data" is the captured
stdout/stderr + exit code of each of the 6 commands, recorded in the report.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: PRE-FLIGHT — confirm S1 and S2 outputs are present BEFORE verifying
  - RUN: grep -n 'qmk_notifier_notify' docs/troubleshooting.md
    EXPECT: exactly one hit, line 647. (Confirms S1 landed.)
  - RUN: grep -n 'qmk_notifier_notify' docs/llms_full.txt
    EXPECT: exactly one hit (≈ line 2622). (Confirms S2 landed.)
  - RUN: grep -n 'qmk-notifier_notify' docs/troubleshooting.md docs/llms_full.txt
    EXPECT: no output (exit 1). (Both files clean of the stale form.)
  - IF either file still shows the stale hyphen form: that upstream sibling regressed.
          Fix per its own PRP (S1: edit line 647; S2: re-run generate_llms_full.sh),
          then proceed. This is the fix-or-fail branch triggering early.
  - GOAL: guarantee the inputs to S3 (fixed source + regenerated mirror) exist
          before S3 claims the tree is clean.

Task 2: RUN the 5 contract greps (capture raw + filtered output for each)
  - FROM: /home/dustin/projects/qmkonnect
  - DEFINE: EXCL='\.pi-subagents/|/target/|docs/vendor/|/\.git/|/plan/'
  - RUN (a): grep -rn 'qmk-notifier_notify' . | grep -vE "$EXCL"
             → EXPECT empty. (Robust form: grep -rn ... . > /tmp/a_raw; grep -vE "$EXCL" /tmp/a_raw)
  - RUN (b): grep -rn 'package = "qmk_notifier"' --include='*.toml' . | grep -vE "$EXCL"
             → EXPECT empty.
  - RUN (c): grep -rn 'tag = "v0.2.1"' --include='*.toml' . | grep -vE "$EXCL"
             → EXPECT empty.
  - RUN (d): grep -rn 'build-installer.ps1' .github/
             → EXPECT empty. (NOTE the .github/ scope — do NOT widen.)
  - RUN (e): grep -rn 'config/qmk-notifier' --include='*.rs' --include='*.md' . | grep -vE "$EXCL"
             → EXPECT empty.
  - FOR EACH: record command, expected, observed (empty or the offending hits), exit code.
  - IF ANY prints a hit: that is a GENUINE product-tree stale reference. Go to Task 5
          (fix-or-fail). Do NOT mark the task complete with an open hit.

Task 3: RUN the build gate — cargo check --offline
  - FROM: /home/dustin/projects/qmkonnect
  - RUN (f): cargo check --bin qmkonnect --offline
  - EXPECT: exit 0, final line "Finished `dev` profile ... target(s) in <N>s", NO warnings.
  - IF --offline fails with a registry/git-fetch error (cache miss): fall back to
          `cargo check --bin qmkonnect` (online) and DOCUMENT the deviation in the report.
  - IF it prints WARNINGS: investigate each; if the remediation introduced it, fix it;
          otherwise note as pre-existing. Baseline is zero warnings.
  - IF it ERRORS (compile failure): that is a regression — investigate root cause,
          fix, re-run. (Highly unlikely: the drift was docs-only.)

Task 4: WRITE the verification report (THE deliverable)
  - CREATE: plan/003_7059790d6c5b/P1M1T1S3/research/verification_report.md
  - CONTENT (sections):
      1. Header: task id, date, repo path, command CWD.
      2. Summary: "All N/6 checks PASS" (or list failures).
      3. Per-check table: # | command | scope/exclusions | expected | observed | exit | pass/fail.
         Include the raw-vs-filtered note for repo-wide greps.
      4. Known observations (NON-failures, documented for completeness):
         (i)  installer.wxs ABSENT — removed in cb9a165; the contract's
              "explicitly-retained" exclusion clause is stale (no-op). `find . -name '*.wxs'`
              → empty.
         (ii) spec/PACKAGING.md:88,232 reference removed WiX tooling (installer.wxs,
              build-installer.ps1). OUT of contract grep (d) scope (.github/-only). Stale
              doc; NOT actioned here (separate work item).
      5. Inputs confirmed (Task 1): S1 + S2 outputs present and clean.
      6. Conclusion: P1.M1.T1 verification gate PASSES (or FAILS with remediation taken).
  - GOAL: a future agent can re-run the exact 6 commands and reproduce the verdict.

Task 5: FIX-OR-FAIL (only if a check in Task 2/3 prints an unexpected hit)
  - SCOPE: fix ONLY the genuine stale reference(s) found. Minimal, surgical edits.
  - FOR a docs/spec stale token: exact-text replacement (see P1M1T1S1 PRP for the
          qmk_notifier vs qmk-notifier rule — underscore = firmware symbol, hyphen = crate).
  - FOR a Cargo.toml stale package/tag: correct to qmk-notifier v0.3.0 (see Cargo.toml:18).
  - FOR a CI stale ref (.github/): ensure release.yml uses Inno build.ps1, not WiX.
  - FOR a config-path stale ref (src/): change config/qmk-notifier → config/qmkonnect.
  - AFTER fixing: RE-RUN all 6 checks (Task 2 + Task 3) until green. Record the fix
          and the re-run in the report.
  - GOAL: the contract clause "If any stale reference is found, it must be fixed
          before this subtask is complete" is honored — never leave a real hit open.
```

### Implementation Patterns & Key Details

```text
# Robust grep pattern (auditable: keeps raw + filtered lists).
#   The naive `grep ... | grep -vE "$EXCL"` hides the raw hit list. For a
#   verification REPORT you want evidence. Capture both:
#
#     EXCL='\.pi-subagents/|/target/|docs/vendor/|/\.git/|/plan/'
#     grep -rn 'qmk-notifier_notify' . > /tmp/check_a_raw.txt 2>/dev/null || true
#     grep -vE "$EXCL" /tmp/check_a_raw.txt   # <-- the verdict line (must be empty)
#     wc -l < /tmp/check_a_raw.txt            # <-- raw count (will be ~30; all in excluded dirs)
#
#   The report can then show: "raw: 28 hits (all under plan/ + .pi-subagents/);
#   filtered (product tree): 0 hits." This proves the exclusions are doing their
#   job and that no product-tree hit was quietly dropped.

# Exit-code semantics recap (greps):
#   grep exit 0 = match found  → for checks a-e this is FAILURE (a stale ref exists)
#   grep exit 1 = no match     → for checks a-e this is PASS
#   grep exit 2 = usage error  → treat as a tooling problem, not a verdict
#   When piping `grep PATTERN | grep -vE EXCL`, the pipeline exit code is the LAST
#   grep's — so prefer the capture-then-filter form above for unambiguous evidence.

# The whole task is deterministic and re-runnable. There is no code to write.
#   "Implementation" = run 6 commands, interpret exit codes, write a report.
```

### Integration Points

```yaml
PRODUCT-TREE FILES:
  - modify: "NONE (unless a check fails — then only the specific stale reference)."
  - the report is NOT a product-tree file; it lives under plan/.

DEPENDENCIES / BUILD:
  - cargo check --offline: requires the qmk-notifier v0.3.0 git dep cached locally
    (VERIFIED present — Cargo.lock pins it, baseline check exits 0 in 0.13s).
    Fallback if cache missing: plain `cargo check --bin qmkonnect` (online); document.

UPSTREAM CONTRACTS (consumed):
  - P1.M1.T1.S1 (Complete): docs/troubleshooting.md:647 = qmk_notifier_notify.
    S3 Task 1 verifies this is still true; grep (a) verifies it left no stale hit.
  - P1.M1.T1.S2 (Ready/Implementing): docs/llms_full.txt regenerated (line ~2622 mirror).
    S3 Task 1 verifies this; grep (a) verifies the .txt mirror is also clean.

DOWNSTREAM CONTRACTS (produced):
  - The verification report is the evidence that P1.M1.T1 ("Fix residual doc drift
    and verify clean tree") is COMPLETE. It closes the milestone M1 verification gate.
  - plan/003_7059790d6c5b/architecture/delta_verification.md §"Residual Drift": both
    items (#1 source, #2 mirror) are now closed + independently confirmed.

VERIFICATION-ONLY CONSUMER:
  - The orchestrator reads verification_report.md to flip P1.M1.T1.S3 to Complete
    and P1.M1.T1 to Done.
```

## Validation Loop

> All commands run from the repo root: `/home/dustin/projects/qmkonnect`

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect

# Pre-flight (Task 1) — confirm upstream inputs are present and clean.
grep -n 'qmk_notifier_notify' docs/troubleshooting.md   # expect exactly one hit, line 647
grep -n 'qmk_notifier_notify' docs/llms_full.txt        # expect exactly one hit, ~2622
grep -n 'qmk-notifier_notify' docs/troubleshooting.md docs/llms_full.txt   # expect no output (exit 1)
# If the last command prints anything, an upstream sibling regressed — fix it first.

# Define the exclusion filter (reused by checks a, b, c, e).
EXCL='\.pi-subagents/|/target/|docs/vendor/|/\.git/|/plan/'
```

### Level 2: Unit Tests (Component Validation)

```text
NOT APPLICABLE — there are no unit tests for a grep/cargo-check verification pass.
The 5 greps + 1 cargo check in Level 3 ARE the component verification.
```

### Level 3: Integration Testing (System Validation) — THE 6 CHECKS

```bash
cd /home/dustin/projects/qmkonnect
EXCL='\.pi-subagents/|/target/|docs/vendor/|/\.git/|/plan/'

# Check (a) — stale firmware-callback token. EXPECT: no output.
grep -rn 'qmk-notifier_notify' . | grep -vE "$EXCL"
# (auditable form: grep -rn 'qmk-notifier_notify' . > /tmp/a_raw 2>/dev/null || true; grep -vE "$EXCL" /tmp/a_raw)

# Check (b) — old crate self-declaration. EXPECT: no output.
grep -rn 'package = "qmk_notifier"' --include='*.toml' . | grep -vE "$EXCL"

# Check (c) — old crate git tag. EXPECT: no output.
grep -rn 'tag = "v0.2.1"' --include='*.toml' . | grep -vE "$EXCL"

# Check (d) — legacy WiX build script in CI. EXPECT: no output. (NOTE .github/ scope.)
grep -rn 'build-installer.ps1' .github/

# Check (e) — old config directory path. EXPECT: no output.
grep -rn 'config/qmk-notifier' --include='*.rs' --include='*.md' . | grep -vE "$EXCL"

# Check (f) — the build gate. EXPECT: exit 0, "Finished `dev` profile ...", NO warnings.
cargo check --bin qmkonnect --offline
# If --offline errors on a missing registry/git dep (cache miss), fall back:
#   cargo check --bin qmkonnect   # and document the deviation in the report.

# Each grep MUST print nothing (its exit is 1 = no match = PASS). Any printed line
# from checks a-e = a genuine stale reference → Task 5 (fix-or-fail). A non-zero
# exit from check f = a build regression → investigate + fix.
```

### Level 4: Creative & Domain-Specific Validation

```bash
cd /home/dustin/projects/qmkonnect
EXCL='\.pi-subagents/|/target/|docs/vendor/|/\.git/|/plan/'

# Confidence cross-check 1: the stale token's RAW count should be non-zero (it lives
# in the excluded planning dirs). If raw == 0 too, the exclusions aren't doing anything
# special (still a pass, but worth noting). If raw > 0 AND filtered == 0, the
# exclusions are correctly isolating the product tree.
grep -rn 'qmk-notifier_notify' . > /tmp/a_raw.txt 2>/dev/null || true
echo "raw hits: $(wc -l < /tmp/a_raw.txt)"          # expect ~30 (plan/ + .pi-subagents/)
echo "product-tree hits: $(grep -vcE "$EXCL" /tmp/a_raw.txt)"   # expect 0
grep -vE "$EXCL" /tmp/a_raw.txt | sed 's/:.*//' | sort -u       # list any product dirs that hit (expect none)

# Confidence cross-check 2: installer.wxs is genuinely absent (documents known obs #1).
find . -name '*.wxs' 2>/dev/null | grep -vE "$EXCL"   # expect no output
git log --oneline -1 -- '**/installer.wxs'             # expect cb9a165 (removal commit)

# Confidence cross-check 3: the out-of-scope legacy WiX refs DO exist in spec/PACKAGING.md
# (documents known obs #2) — proving grep (d)'s .github/ scope is load-bearing.
grep -n 'build-installer.ps1' spec/PACKAGING.md        # expect lines 88 and 232
grep -rn 'build-installer.ps1' .github/                # expect no output (the actual check d)

# Confidence cross-check 4: legitimate tree labels in HOST_RULES.md are untouched & correct.
grep -nE 'qmk[-_]notifier/\s' spec/HOST_RULES.md      # expect ~563: both labels present

# Cargo determinism: re-run to confirm stable result.
cargo check --bin qmkonnect --offline 2>&1 | tail -2   # expect "Finished `dev` profile ... in <N>s"
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1 passed: pre-flight confirms S1 (troubleshooting.md:647) + S2 (llms_full.txt ~2622) both read `qmk_notifier_notify`, zero stale in those two files.
- [ ] Level 3 check (a) passed: `grep -rn 'qmk-notifier_notify' . | grep -vE "$EXCL"` → no output.
- [ ] Level 3 check (b) passed: `grep -rn 'package = "qmk_notifier"' --include='*.toml' . | grep -vE "$EXCL"` → no output.
- [ ] Level 3 check (c) passed: `grep -rn 'tag = "v0.2.1"' --include='*.toml' . | grep -vE "$EXCL"` → no output.
- [ ] Level 3 check (d) passed: `grep -rn 'build-installer.ps1' .github/` → no output (`.github/` scope honored).
- [ ] Level 3 check (e) passed: `grep -rn 'config/qmk-notifier' --include='*.rs' --include='*.md' . | grep -vE "$EXCL"` → no output.
- [ ] Level 3 check (f) passed: `cargo check --bin qmkonnect --offline` → exit 0, zero warnings.
- [ ] IF any check failed: the stale reference was fixed and ALL 6 checks re-run green (Task 5).

### Feature Validation

- [ ] Verification report exists at `plan/003_7059790d6c5b/P1M1T1S3/research/verification_report.md`.
- [ ] Report records all 6 checks (command, expected, observed, exit, pass/fail).
- [ ] Report documents known observation #1: `installer.wxs` absent (removed cb9a165; stale exclusion clause).
- [ ] Report documents known observation #2: `spec/PACKAGING.md:88,232` legacy WiX refs are out of grep (d)'s `.github/` scope (not actioned).
- [ ] Report's summary verdict matches the observed check results (no false "PASS" with an open hit).

### Code Quality Validation

- [ ] No product-tree file was modified UNLESS a check genuinely failed (then only the specific stale ref).
- [ ] Grep (d) was run with the EXACT `.github/` scope (not widened to repo root).
- [ ] The exclusion filter was applied to all repo-wide greps (a, b, c, e).
- [ ] No blanket `sed s/qmk-notifier/qmk_notifier/g` sweep was run (would wrongly rewrite legitimate Rust-crate references).

### Documentation & Deployment

- [ ] The verification report IS the only artifact (when all checks pass) — no product doc surface change (per contract "DOCS: none").
- [ ] `--offline` deviation (if any) is documented in the report.
- [ ] No environment variables, config, or build outputs affected.

---

## Anti-Patterns to Avoid

- ❌ Don't run the greps WITHOUT the exclusion filter — `plan/` and `.pi-subagents/`
  legitimately contain `qmk-notifier_notify` (PRPs that quote the old form, cached
  research transcripts). Without exclusions every check "fails" with ~30 false hits.
- ❌ Don't widen grep (d) to the repo root — `spec/PACKAGING.md:88,232` reference the
  removed WiX tooling and are OUT of the contract's `.github/` scope. Run it scoped
  exactly as specified.
- ❌ Don't fail (or try to "restore") `installer.wxs` — it was removed in `cb9a165`.
  The "explicitly-retained" clause is stale; excluding a nonexistent path is a no-op.
- ❌ Don't fix `spec/PACKAGING.md` legacy WiX refs here — they are out of scope
  (grep d is `.github/`-only) and this is a verification pass ("DOCS: none"). Record
  them as a known observation; fixing spec/ doc drift is a separate work item.
- ❌ Don't run a blanket `sed s/qmk-notifier/qmk_notifier/g` to "clean up" — it would
  wrongly rewrite the legitimate Rust-crate name (`qmk-notifier` v0.3.0, hyphen) in
  Cargo.toml, Cargo.lock, and docs. Only firmware C *symbols* use underscore.
- ❌ Don't skip the cargo `--offline` flag without documenting it — if the cache is
  present (it is, per baseline), `--offline` is the contract. If it errors (cache
  miss), fall back to online AND note the deviation in the report.
- ❌ Don't treat a grep exit code 1 as an error — for checks a-e, exit 1 (no match) is
  the PASS condition. Exit 0 (match found) is the failure. (Exit 2 = grep usage error.)
- ❌ Don't mark the task complete with an open hit — the contract is explicit: "If any
  stale reference is found, it must be fixed before this subtask is complete." Any
  printed product-tree line from checks a-e (or a cargo error) MUST be fixed + re-run.
- ❌ Don't "fix" the legitimate labels in `spec/HOST_RULES.md:563`
  (`qmk-notifier/` crate repo + `qmk_notifier/` firmware repo) — hyphen = crate,
  underscore = firmware, both correct. None of the 5 greps match them.
- ❌ Don't re-edit `docs/troubleshooting.md` or `docs/llms_full.txt` unless Task 1's
  pre-flight shows they regressed — those are S1's and S2's deliverables. S3 verifies,
  it doesn't redo them.

---

**Confidence Score: 10/10** for one-pass implementation success. The deliverable is
six deterministic, read-only commands (5 greps + 1 cargo check) plus a report, all
of which were run during research and ALL PASS in the current working tree. The exact
commands, exact scopes, exact exclusion regex, the expected result for each, and the
two known discrepancies (and exactly how to treat them) are fully specified. The only
branches — fix-or-fail (if a genuine stale hit appears) and the `--offline` cache-miss
fallback — are spelled out. There is no code to write, no build to produce, and no
ambiguity in "clean": each grep prints nothing and cargo exits 0 with no warnings.
The task closes the P1.M1.T1 verification gate with auditable evidence.