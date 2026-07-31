# PRP — P1.M1.T2.S2: Regenerate `docs/llms_full.txt` and verify the full tree

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`).
> **Scope:** Run the committed generator `docs/generate_llms_full.sh` to refresh
> the single-file doc concatenation after S1 unified the 4 source docs to
> `[[rule]]`, then verify the regenerated file (and the whole `docs/`+`src/`
> tree) carries zero stale split-schema tokens and zero withdrawn `≥ 224` floor
> guidance. **One deliverable artifact:** a regenerated `docs/llms_full.txt`,
> committed alongside S1's doc edits. **No `src/`, no `docs/*.md`, no script
> edits** — the generator is the only sanctioned writer of llms_full.
> **Why a dedicated step:** llms_full.txt was last regenerated 2026-07-20; S1
> edited the 4 source docs on 2026-07-31, so the concat is stale (still 25
> `[[layer_rules]]`/`[[callback_rules]]` + 0 `[[rule]]` + the withdrawn `≥ 224`
> floor). Regenerating is deterministic and closes both gaps at once.

---

## Goal

**Feature Goal**: Make `docs/llms_full.txt` — the canonical single-file doc dump
for agents/LLMs — mirror the current (S1-unified) source docs byte-for-byte, so
that an LLM reading llms_full sees the unified `[[rule]]` schema (SINGULAR,
`layer` optional, ≥1-of validity) and the withdrawn-`224`-floor correction,
identical to what a human reading `docs/*.md` sees.

**Deliverable**:
1. A regenerated `docs/llms_full.txt` (produced by `bash docs/generate_llms_full.sh`).
2. A short verification report: the four grep counts + the cargo test/check pass
   status (captured in the commit message or the Final Validation Checklist).
3. `docs/llms_full.txt` committed together with S1's 4 doc edits (one coherent
   "unified `[[rule]]` schema in docs + regenerated llms_full" commit).

**Success Definition** (the four gates from the contract — all must hold):
- (a) `grep -rnE '\[\[(layer|callback)_rules\]\]' docs/ src/` → **0 hits** (was 25,
      all in llms_full before regen; src/ already 0 via S3).
- (b) `grep -cE '\[\[rule\]\]' docs/llms_full.txt` → **≥ 20** (~22: 13+5+3+1 from
      the 4 source docs; was 0 before regen).
- (c) `cargo test --bin qmkonnect -- --test-threads=1` AND
      `cargo check --bin qmkonnect --offline` → both **clean** (sanity; S2 touches
      no Rust, no llms_full sync test exists).
- (d) `grep -nE '≥ 224|>= 224' docs/llms_full.txt` → **0 hits** (the withdrawn
      floor GUIDANCE; was several before regen. The intentional withdrawal NOTE
      in examples.md still mentions "224" — that is correct, not a failure.)

## User Persona (if applicable)

**Target User**: An AI agent / LLM (or a human skimmer) that consumes
`docs/llms_full.txt` as the single-source QMKonnect reference. Today it would
copy a stale `[[layer_rules]]` example, write `rules.toml`, and the parser would
silently drop the unknown field (empty ruleset) — or pick the withdrawn `224`
floor and pick an undefined layer.

**Use Case**: An agent is told "configure QMKonnect host rules"; it reads
llms_full.txt, finds the unified `[[rule]]` schema with `layer` optional, copies
the annotated 4-`[[rule]]` example, and the rules fire. After this task llms_full
agrees with `docs/*.md` and `spec/HOST_RULES.md` §9.

**Pain Points Addressed**: The published single-file reference drifting from the
hand-edited per-page docs. llms_full is the MOST-likely-to-be-stale artifact
(generated, easy to forget to re-run), and the one agents read first.

## Why

- **Closes the docs-unification task (C8) end-to-end.** S1 unified the 4 source
  `docs/*.md`; S3 unified `src/`; this task propagates those changes into the
  generated concat so all four surfaces (code, per-page docs, spec, llms_full)
  agree. Without it, llms_full actively contradicts the per-page docs.
- **Captures a bonus fix for free.** The frozen llms_full still carries the
  WITHDRAWN `≥ 224` layer-floor guidance (the sources dropped it earlier but the
  concat was never re-run). Regenerating purges it in the same pass — no extra
  edit needed.
- **Deterministic + zero-risk.** The script is a pure concatenation with
  front-matter stripping; running it cannot break anything. The only artifact
  changed is `docs/llms_full.txt` (git-tracked). No code, no config, no build.

## What

Run the generator, then run four verification greps + two cargo sanity commands.
Record the results. Commit `docs/llms_full.txt` with S1's doc edits.

### Success Criteria

- [ ] (a) `grep -rnE '\[\[(layer|callback)_rules\]\]' docs/ src/` → 0 hits.
- [ ] (b) `grep -cE '\[\[rule\]\]' docs/llms_full.txt` → ≥ 20.
- [ ] (c) `cargo test --bin qmkonnect -- --test-threads=1` → pass; `cargo check
      --bin qmkonnect --offline` → clean.
- [ ] (d) `grep -nE '≥ 224|>= 224' docs/llms_full.txt` → 0 hits.
- [ ] `docs/llms_full.txt` mtime is newer than the 4 source docs' mtimes.
- [ ] `git diff --stat docs/llms_full.txt` shows ONLY llms_full changed by this
      task (the 4 `docs/*.md` + `src/` are S1/S3's, already done).
- [ ] llms_full.txt committed alongside S1's doc edits.

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything
> needed to implement this successfully?"_ — **Yes.** The exact generator command,
> the script's mechanics (8 files, fixed order, `strip_fm`, overwrites llms_full),
> the verified current-state snapshot (S1 landed, src/ clean, llms_full stale with
> exact stale counts), the four verification commands with their BEFORE→AFTER
> expected values, the cargo sanity rationale, the scope/exclusion gotchas
> (`plan/`, `.pi-subagents/`, `target/`, `docs/vendor/` all carry noise), and the
> commit instruction are all below. No source-file reading is required beyond the
> script and the greps.

### Documentation & References

```yaml
# MUST READ — the generator (the ONLY thing S2 runs; read it, don't edit it)
- file: /home/dustin/projects/qmkonnect/docs/generate_llms_full.sh
  why: "The script S2 executes. set -euo pipefail; resolves DOCS_DIR/ROOT from its
        own location (cwd-independent); overwrites docs/llms_full.txt; concatenates
        8 files in fixed order (README.md, docs/index.md, docs/installation.md,
        docs/qmk-integration.md, docs/configuration.md, docs/usage.md,
        docs/examples.md, docs/troubleshooting.md); strip_fm() awk drops a LEADING
        Jekyll --- front-matter block when line 1 is --- (README has none; the 7
        docs/*.md all do); emit() writes an 80-'=' divider + ' N. path (label)'
        header per section; final echo prints 'wrote ... (N lines, M bytes)'."
  pattern: "Run it: `bash docs/generate_llms_full.sh` from the repo root. It prints
            the new line/byte count and exits 0 on success."
  gotcha: "It OVERWRITES docs/llms_full.txt — there is no --dry-run. The change is
           visible via `git diff docs/llms_full.txt`. Do NOT hand-edit the file;
           the script is the only sanctioned writer."

# MUST READ — the previous subtask's PRP (the CONTRACT for what the sources now say)
- file: /home/dustin/projects/qmkonnect/plan/004_f48a103bcb32/P1M1T2S1/PRP.md
  why: "S1 rewrote the 4 source docs to unified [[rule]] (SINGULAR), layer optional,
        >=1-of-(layer/enable/disable) validity, no 224 floor. S1 explicitly defers
        llms_full.txt to 'S2 regenerates (it concatenates these files)'. This task
        IS that regeneration. S1's grep gate EXCLUDES llms_full.txt; S2's gate
        INCLUDES it (after regen it must be clean)."
  section: "Goal, Anti-Patterns (don't hand-edit llms_full), Integration Points
            (DOWNSTREAM: P1.M1.T2.S2)"
  critical: "PRECONDITION: S1 must have LANDED before S2 runs. Verify first
             (Task 1): the 4 source docs must have 0 split tokens + [[rule]]
             present. If S1 hasn't landed, STOP — regenerating now would bake the
             STALE split schema into a fresh llms_full and S1's gate would still
             exclude it (false pass)."

# REFERENCE — current-state verification (research notes for this subtask)
- docfile: /home/dustin/projects/qmkonnect/plan/004_f48a103bcb32/P1M1T2S2/research/notes.md
  why: "The verified snapshot: S1 landed (4 docs unified, 22 [[rule]] total), src/
        clean (S3), llms_full frozen 2026-07-20 (25 split tokens, 0 [[rule]], 13
        '224' refs incl. the withdrawn >=224 floor). Documents the two fixes regen
        captures, the exact verification grep forms, and the scope/exclusion rules
        (plan/, .pi-subagents/, target/, docs/vendor/ all carry token noise)."

# REFERENCE — the dev-loop commands (cargo test single-threaded, why --offline)
- file: /home/dustin/projects/qmkonnect/AGENTS.md
  why: "The cargo test loop: `cargo test --bin qmkonnect -- --test-threads=1`
        (single-threaded because of shared debouncer state). The --offline flag on
        cargo check assumes deps are cached (Cargo.lock is present). If --offline
        errors on a fresh clone, drop it — the regen itself needs no Rust."
  section: "macOS/Windows/Linux dev test loop"
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/
├── docs/
│   ├── generate_llms_full.sh   # <-- RUN THIS (committed, executable, set -euo pipefail)
│   ├── llms_full.txt           # <-- REGENERATED (the ONE artifact S2 changes; git-tracked)
│   ├── index.md                # source 2 (clean — never had split schema)
│   ├── installation.md         # source 3 (clean)
│   ├── qmk-integration.md      # source 4 (S1-unified: [[rule]])
│   ├── configuration.md        # source 5 (S1-unified: [[rule]] x13)
│   ├── usage.md                # source 6 (clean)
│   ├── examples.md             # source 7 (S1-unified: [[rule]] x5; has the 224 WITHDRAWAL note)
│   ├── troubleshooting.md      # source 8 (S1-unified: [[rule]] x1)
│   ├── README.md (../)         # source 1 (no front matter; clean)
│   └── vendor/                 # Ruby gems / Jekyll fonts — NOT concatenated; grep NOISE for "224"
└── src/                        # clean (S3 / P1.M1.T1.S3 complete) — DO NOT EDIT
```

### Desired Codebase tree with files to be modified

```bash
docs/
└── llms_full.txt   # REGENERATED ONLY (by the script). No other file touched by S2.
# (the 4 docs/*.md are S1's; src/ is S3's; the script is unchanged)
```

### Known Gotchas of our codebase & Library Quirks

```bash
# CRITICAL PRECONDITION: S1 must have LANDED before you regenerate. Verify first
#   (Task 1): the 4 source docs must have 0 [[layer_rules]]/[[callback_rules]] and
#   [[rule]] present. If you regenerate while the sources still show the SPLIT
#   schema, you bake the stale schema into a FRESH llms_full — and S1's grep gate
#   (which excludes llms_full) would still pass, hiding the failure. Only regen
#   AFTER Task 1 confirms the sources are unified.

# CRITICAL: the script OVERWRITES docs/llms_full.txt (no --dry-run). The change is
#   a git diff, fully reviewable. NEVER hand-edit llms_full.txt — it's a pure
#   concatenation; manual edits are overwritten on the next run and risk merge
#   entropy. The script is the only sanctioned writer.

# CRITICAL: scope your greps. The split-schema tokens and "224" appear as NOISE in
#   several NON-doc places — do NOT repo-wide grep:
#     - plan/            : 28 files — the PRPs' OLD-text blocks contain [[layer_rules]].
#     - .pi-subagents/   : transcripts (may contain the tokens).
#     - target/          : build artifacts.
#     - docs/vendor/     : vendored Ruby gems / Jekyll font SVGs — thousands of
#                           false-positive "224" / ">=" substrings in glyph paths.
#   Token grep scope: `docs/ src/`. The "224" grep scope: `docs/llms_full.txt`
#   ONLY (the script never concatenates docs/vendor/).

# GOTCHA: verification (d) greps the FLOOR-GUIDANCE forms `≥ 224` (unicode) and
#   `>= 224` (ASCII) — both → 0 after regen. A bare `224` grep will STILL find 2
#   lines (examples.md's withdrawal note: "layer = 224" / "bit 224"). That is
#   INTENTIONAL documentation (it tells users the 224 floor was withdrawn), NOT a
#   failure. Do not "fix" the withdrawal note.

# GOTCHA: README.md is the ONLY source without Jekyll front matter (line 1 =
#   "# QMKonnect"). The 7 docs/*.md all start with "---". strip_fm() handles both:
#   README passes through whole; the 7 have their leading --- ... --- block
#   dropped. You don't need to do anything — the script is correct as-is.

# GOTCHA: cargo test/check are PURE SANITY. There is no test/code referencing
#   llms_full (verified: grep -rn llms_full src/ tests/ → empty). S2 changes no
#   Rust, so these must pass unchanged. --test-threads=1 is required by the shared
#   debouncer (AGENTS.md). --offline needs cached deps (Cargo.lock is present); if
#   it fails, drop --offline — the regen needs no Rust.

# NOTE: docs/llms_full.txt is git-TRACKED (git ls-files lists it), unlike build
#   outputs (target/, *.dmg, *.msi) which are gitignored. So `git add
#   docs/llms_full.txt` works and the regen is a normal committed change.

# NOTE: the script is cwd-independent (it resolves DOCS_DIR from BASH_SOURCE), so
#   `bash docs/generate_llms_full.sh` works from the repo root OR any subdir. The
#   contract says "from the repo root" for clarity.
```

## Implementation Blueprint

### Data models and structure

No data models. This is a "run a script + verify" task. The "structure" is the
generator's fixed 8-file concatenation order + the four verification gates.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: VERIFY THE PRECONDITION (S1 landed) — do this BEFORE regenerating
  - RUN: for f in docs/configuration.md docs/examples.md docs/qmk-integration.md
          docs/troubleshooting.md; do printf "%-30s split=%s rule=%s\n" "$f" \
          "$(grep -cE '\[\[(layer|callback)_rules\]\]' "$f")" \
          "$(grep -c '\[\[rule\]\]' "$f")"; done
  - EXPECT: each row split=0 and rule=>0 (S1 unified the 4 docs). If ANY shows
          split>0, S1 has NOT landed — STOP and re-check plan status; do not
          regenerate yet (you'd bake the stale split schema into a fresh concat).
  - ALSO confirm src/ is clean: grep -rnE '\[\[(layer|callback)_rules\]\]' src/
          → empty (S3 complete). And llms_full is the ONLY stale file:
          grep -cE '\[\[(layer|callback)_rules\]\]' docs/llms_full.txt → 25 (stale).

Task 2: REGENERATE docs/llms_full.txt
  - RUN (from repo root): bash docs/generate_llms_full.sh
  - EXPECT: the script prints "wrote /.../docs/llms_full.txt (<N> lines, <M> bytes)"
          and exits 0 (set -euo pipefail aborts on any error). N will be close to
          the prior 2718 (the schema rewrite is roughly line-neutral; the 224
          removal trims a little).
  - CONFIRM: stat -c '%y' docs/llms_full.txt is now newer than the 4 source docs.

Task 3: VERIFY gate (a) — zero stale split-schema tokens
  - RUN: grep -rnE '\[\[(layer|callback)_rules\]\]' docs/ src/
  - EXPECT: NO output (exit 1). Before regen this printed 25 (all in llms_full);
          after regen 0. src/ was already 0 (S3). RECORD "0 hits" for the report.
  - NOTE: this grep is scoped to docs/ src/ — do NOT widen it (plan/,
          .pi-subagents/, target/ contain the tokens as PRP/transcript noise).

Task 4: VERIFY gate (b) — regenerated [[rule]] hits present
  - RUN: grep -cE '\[\[rule\]\]' docs/llms_full.txt
  - EXPECT: ~22 (13+5+3+1 from the 4 source docs). Gate: ≥ 20. Before regen this
          was 0. RECORD the count for the report.
  - ALSO (no plural mistake): grep -cE '\[\[rules\]\]' docs/llms_full.txt → 0
          ([[rules]] plural would silently parse to an empty ruleset).

Task 5: VERIFY gate (c) — Rust sanity (no behavior change)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1
  - EXPECT: all tests pass, 0 failed (single-threaded for the shared debouncer).
  - RUN: cargo check --bin qmkonnect --offline
  - EXPECT: clean (deps cached; Cargo.lock present). If --offline errors on a
          fresh clone, drop it and run `cargo check --bin qmkonnect` — the regen
          needs no Rust; this is pure "nothing broke" insurance.
  - RECORD: "cargo test: PASS; cargo check: clean" for the report.

Task 6: VERIFY gate (d) — no withdrawn ≥224/>=224 floor guidance
  - RUN: grep -nE '≥ 224|>= 224' docs/llms_full.txt
  - EXPECT: NO output. Before regen this printed several (889, 1247, 1271, 1278,
          1868, 2498, 2526, …); after regen 0. RECORD "0 hits" for the report.
  - SANITY (expected non-zero, NOT a failure): grep -nE '\b224\b' docs/llms_full.txt
          → ~2 hits (the examples.md withdrawal note: "layer = 224 is withdrawn …
          layer_state cannot hold bit 224"). That note is intentional; leave it.

Task 7: REVIEW the diff + COMMIT
  - RUN: git diff --stat docs/llms_full.txt   # confirm ONLY llms_full changed by S2
  - RUN: git diff docs/llms_full.txt | head -60   # eyeball: split→[[rule]], 224 gone
  - COMMIT: git add docs/llms_full.txt (and S1's 4 docs/*.md if not yet committed)
          → one commit: "docs: unify [[rule]] schema + regenerate llms_full.txt".
          Per the contract OUTPUT, llms_full goes in the SAME commit as S1's doc
          edits.
  - REPORT: capture the four grep counts + the cargo pass status (the "short
          verification report" deliverable) — in the commit message or a summary.
```

### Implementation Patterns & Key Details

```bash
# === WHY a dedicated regen step (vs. S1 doing it) ===
#   Separation of concerns: S1 owns the doc PROSE/TOML edits (docs/*.md); S2 owns
#   the GENERATED artifact (llms_full.txt). Keeping them separate means S1's grep
#   gate can EXCLUDE llms_full (so a stale concat doesn't mask a missed prose
#   site), and S2's gate INCLUDES it (proving the concat was refreshed). If S1
#   also regenerated, a failure mid-S1 would leave a half-regenerated concat.

# === WHY regenerating also fixes the 224 floor (a freebie) ===
#   The sources dropped the "≥ 224" floor earlier (configuration.md line 273 now
#   says "raw QMK layer index, not a reserved range"; examples use layer=10/11).
#   But llms_full was never re-run, so it still shows the OLD "Must be ≥ 224"
#   guidance + layer=224 examples. One regen pass propagates BOTH the S1 schema
#   unification AND the earlier 224-drop into the concat. No extra edit needed.

# === WHY cargo test/check are included (S2 touches no Rust) ===
#   Pure sanity / defense-in-depth. There is no llms_full sync test (verified), so
#   these cannot fail due to S2. They confirm the broader tree still builds/tests
#   green after the docs task — a cheap "did anything regress?" check that matches
#   the AGENTS.md dev loop. If they fail, it's a PRE-EXISTING breakage unrelated
#   to S2 (flag it, don't try to fix src/ — that's out of scope).

# === WHY scope greps narrowly ===
#   plan/ holds 28 PRP files whose OLD-text blocks literally contain
#   [[layer_rules]] (they document the before→after). .pi-subagents/ holds
#   transcripts. docs/vendor/ holds Ruby gem SVGs with thousands of "224" glyph
#   coords. A repo-wide grep floods with these. The contract's "excluding target/,
#   plan/, .pi-subagents/" note exists BECAUSE those dirs carry the tokens. Scope
#   token greps to docs/ src/; scope the 224 grep to docs/llms_full.txt only.
```

### Integration Points

```yaml
SOURCE FILES:
  - run (only): "docs/generate_llms_full.sh (unchanged — the generator)"
  - regenerate (the one artifact): "docs/llms_full.txt (git-tracked, overwritten
    by the script)"
  - do NOT modify: "src/ (S3), docs/*.md (S1), docs/generate_llms_full.sh,
    spec/HOST_RULES.md (read-only), docs/vendor/ (gems)"

UPSTREAM CONTRACT (must be LANDED before S2 runs):
  - P1.M1.T2.S1 (S1): "the 4 source docs unified to [[rule]] (layer optional,
    >=1-of validity, no 224 floor). VERIFY in Task 1 before regenerating."
  - P1.M1.T1.S3 (S3, Complete): "src/ callers + render_rules_body unified; src/
    has 0 split tokens."

PARALLEL / SIBLING:
  - None in flight at this layer. S1 is the immediate predecessor; S3 is done.

DOWNSTREAM (do NOT implement — listed for awareness):
  - P1.M1.T3.S1: "Audit README.md + top-level docs for the unified [[rule]]
    schema (separate final sweep). NOTE: README is concatenated into llms_full,
    so if T3.S1 edits README, llms_full must be re-regenerated again (re-run this
    script). Today README is clean (no split tokens), so this regen is correct."

PUBLIC API SURFACE:
  - none. Pure docs artifact.

GIT:
  - "docs/llms_full.txt is git-tracked (git ls-files lists it). git add + commit.
    Commit it WITH S1's 4 docs/*.md edits (one coherent commit per the contract)."
```

## Validation Loop

> All commands run from the repo root: `/home/dustin/projects/qmkonnect`.

### Level 1: Precondition + regenerate

```bash
cd /home/dustin/projects/qmkonnect

# (0) PRECONDITION — S1 must have landed. Each row: split=0, rule=>0.
for f in docs/configuration.md docs/examples.md docs/qmk-integration.md docs/troubleshooting.md; do
  printf "%-30s split=%s rule=%s\n" "$f" \
    "$(grep -cE '\[\[(layer|callback)_rules\]\]' "$f")" \
    "$(grep -c '\[\[rule\]\]' "$f")"
done
# Expected: configuration.md split=0 rule=13; examples.md split=0 rule=5;
#   qmk-integration.md split=0 rule=3; troubleshooting.md split=0 rule=1.
#   If any split>0: S1 not landed — STOP, do not regenerate.

# Confirm llms_full is the ONLY stale file (it should be: 25 split tokens).
grep -cE '\[\[(layer|callback)_rules\]\]' docs/llms_full.txt   # Expected: 25 (stale, pre-regen)

# (1) REGENERATE.
bash docs/generate_llms_full.sh
# Expected: "wrote /home/dustin/projects/qmkonnect/docs/llms_full.txt (<N> lines, <M> bytes)"
#   and exit 0. set -euo pipefail aborts on any error.

# Confirm the file was actually rewritten (mtime newer than the source docs).
stat -c '%y %n' docs/llms_full.txt docs/configuration.md   # llms_full mtime must be newest
```

### Level 2: The four verification gates

```bash
cd /home/dustin/projects/qmkonnect

# (a) Zero stale split-schema tokens in docs/ + src/ (the primary gate).
grep -rnE '\[\[(layer|callback)_rules\]\]' docs/ src/
# Expected: NO output (grep exits 1). Was 25 (all in llms_full) before regen.

# (b) Regenerated [[rule]] hits present in llms_full.
grep -cE '\[\[rule\]\]' docs/llms_full.txt
# Expected: ~22 (13+5+3+1). Gate: >= 20.
grep -cE '\[\[rules\]\]' docs/llms_full.txt
# Expected: 0 (no plural mistake — [[rules]] would silently parse to empty).

# (c) Rust sanity (S2 touches no Rust; expected unchanged/passing).
cargo test --bin qmkonnect -- --test-threads=1
# Expected: "test result: ok. <N> passed; 0 failed". Single-threaded (shared debouncer).
cargo check --bin qmkonnect --offline
# Expected: "Finished" with no errors. (If --offline errors, drop it — fresh clone.)

# (d) No withdrawn >=224 floor GUIDANCE in llms_full.
grep -nE '≥ 224|>= 224' docs/llms_full.txt
# Expected: NO output. Was several (889, 1247, 1271, 1278, 1868, 2498, 2526, ...) before.
# Sanity (expected NON-zero — the intentional withdrawal note, NOT a failure):
grep -nE '\b224\b' docs/llms_full.txt
# Expected: ~2 hits — examples.md's note: "layer = 224 is withdrawn ... bit 224".
```

### Level 3: Diff review (manual sanity)

```bash
cd /home/dustin/projects/qmkonnect

# Confirm ONLY llms_full changed by THIS task (the 4 docs/*.md are S1's).
git diff --stat docs/llms_full.txt
# Expected: 1 file changed (llms_full.txt). Net delta is moderate (schema rewrite
#   is ~line-neutral; the 224 removal trims a little).

# Eyeball the transformation: split headers -> [[rule]], 224 examples -> 10/11.
git diff docs/llms_full.txt | grep -E '^\-.*\[\[(layer|callback)_rules\]\]|^\+.*\[\[rule\]\]' | head -40
# Expected: many `-[[layer_rules]]`/`-[[callback_rules]]` removals and
#   `+[[rule]]` additions. Confirms the schema propagated.

git diff docs/llms_full.txt | grep -E '^\-.*(≥ 224|>= 224|layer = 224)' | head
# Expected: the withdrawn floor lines removed. (examples.md's withdrawal NOTE
#   mentioning "224" is NOT removed — it's a `+`/unchanged line, correct.)
```

### Level 4: Structural integrity of the regenerated file

```bash
cd /home/dustin/projects/qmkonnect

# All 8 source sections present, in order, with their dividers.
grep -nE '^ [0-9]\. ' docs/llms_full.txt
# Expected: exactly 8 hits — "1. README.md", "2. docs/index.md (Home)",
#   "3. docs/installation.md (Installation)", "4. docs/qmk-integration.md (...)",
#   "5. docs/configuration.md (...)", "6. docs/usage.md (Usage)",
#   "7. docs/examples.md (Firmware examples)", "8. docs/troubleshooting.md (...)".

# The header preamble is intact (the "REALITY CHECK" block the script hardcodes).
grep -c 'QMKonnect - Complete Documentation' docs/llms_full.txt   # Expected: 1
grep -c 'IMPORTANT REALITY CHECK' docs/llms_full.txt              # Expected: 1

# No front-matter leakage (strip_fm dropped the leading --- blocks).
grep -cE '^---[[:space:]]*$' docs/llms_full.txt
# Expected: 0 (the 7 docs/*.md front-matter blocks were stripped; README had none).
```

## Final Validation Checklist

### Technical Validation

- [ ] Task 1 precondition: the 4 source docs show split=0, rule=>0 (S1 landed).
- [ ] Task 2: `bash docs/generate_llms_full.sh` ran, printed "wrote …", exit 0.
- [ ] Gate (a): `grep -rnE '\[\[(layer|callback)_rules\]\]' docs/ src/` → 0 hits.
- [ ] Gate (b): `grep -cE '\[\[rule\]\]' docs/llms_full.txt` → ≥ 20 (~22).
- [ ] Gate (b): `grep -cE '\[\[rules\]\]' docs/llms_full.txt` → 0 (no plural).
- [ ] Gate (c): `cargo test --bin qmkonnect -- --test-threads=1` → pass.
- [ ] Gate (c): `cargo check --bin qmkonnect --offline` → clean.
- [ ] Gate (d): `grep -nE '≥ 224|>= 224' docs/llms_full.txt` → 0 hits.
- [ ] Level 4: all 8 section dividers present in order; header preamble intact;
      0 `^---` front-matter lines leaked.

### Feature Validation

- [ ] `docs/llms_full.txt` mtime is newer than the 4 source docs (it was refreshed).
- [ ] `git diff --stat docs/llms_full.txt` shows ONLY llms_full changed by S2.
- [ ] The diff shows `[[layer_rules]]`/`[[callback_rules]]` → `[[rule]]` and the
      `≥ 224`/`>= 224` floor lines removed (Level 3 spot check).
- [ ] The examples.md withdrawal note ("layer = 224 is withdrawn … bit 224") is
      PRESERVED (it's intentional; `224` grep still finds ~2 lines — correct).
- [ ] Verification report captured: the 4 grep counts + cargo pass status.

### Code Quality Validation

- [ ] No hand-edit to `docs/llms_full.txt` — it was produced solely by the script.
- [ ] No `src/`, `docs/*.md`, `spec/`, or script edits (scope respected).
- [ ] Greps were scoped (not repo-wide) — no confusion with plan/ / .pi-subagents/
      target/ / docs/vendor/ noise.

### Documentation & Deployment

- [ ] `docs/llms_full.txt` committed WITH S1's 4 `docs/*.md` edits (one commit).
- [ ] Commit message notes: regenerated llms_full.txt; unified `[[rule]]` schema
      propagated; withdrawn `≥ 224` floor purged; cargo test/check green.
- [ ] No environment variables, Cargo.toml, or config changes.

---

## Anti-Patterns to Avoid

- ❌ Don't regenerate BEFORE confirming S1 landed (Task 1). If the sources still
  show the SPLIT schema, a fresh concat bakes the stale schema in — and S1's gate
  (which excludes llms_full) would falsely pass. Verify split=0 in the 4 sources
  first.
- ❌ Don't hand-edit `docs/llms_full.txt` — it's a pure concatenation produced by
  `generate_llms_full.sh`. Manual edits are overwritten on the next run and risk
  merge entropy. The script is the ONLY sanctioned writer.
- ❌ Don't edit `docs/generate_llms_full.sh`, `docs/*.md` (S1), `src/` (S3), or
  `spec/HOST_RULES.md` (read-only). S2's entire output is the regenerated
  llms_full.txt.
- ❌ Don't repo-wide grep for the split tokens or "224" — `plan/` (28 PRP files
  with OLD-text blocks), `.pi-subagents/` (transcripts), `target/` (build
  artifacts), and `docs/vendor/` (Ruby gem SVGs with thousands of false-positive
  "224"/">=" glyph coords) all flood the results. Scope token greps to `docs/ src/`;
  scope the 224 grep to `docs/llms_full.txt` only.
- ❌ Don't treat the examples.md withdrawal note's "224" mention as a failure —
  verification (d) greps the FLOOR-GUIDANCE forms `≥ 224` / `>= 224` (→ 0). A bare
  `224` still matches ~2 lines (the note explaining the floor was WITHDRAWN). That
  note is correct and must stay.
- ❌ Don't skip the cargo test/check just because S2 touches no Rust — the contract
  asks for them as a sanity gate. They're cheap and confirm the tree is green. (If
  they fail, it's a pre-existing breakage — flag it, don't fix src/.)
- ❌ Don't drop `--test-threads=1` from cargo test — the shared debouncer state
  requires single-threaded runs (AGENTS.md); parallel tests can flake/fail.
- ❌ Don't commit llms_full.txt SEPARATELY from S1's doc edits — the contract says
  "commit llms_full.txt alongside the doc changes" (one coherent commit).
- ❌ Don't forget `--offline` may fail on a fresh clone (uncached deps) — drop it
  then; the regeneration itself needs no Rust, so `cargo check --bin qmkonnect`
  (online) is an acceptable fallback for the sanity gate.
- ❌ Don't assume the regenerated line count must match the old 2718 exactly — the
  schema rewrite is roughly line-neutral but the `≥ 224` removal trims some lines.
  The gate is the four greps, not the line count.
- ❌ Don't widen Task 1's precondition check into editing anything — it's READ-ONLY
  verification that S1 landed. If S1 hasn't landed, STOP and re-check the plan
  status; do not "help" by editing the docs (that's S1's job).

---

**Confidence Score: 10/10** for one-pass implementation success. The deliverable
is a single deterministic script invocation (`bash docs/generate_llms_full.sh`)
plus four mechanical grep/cargo verifications, each with its exact BEFORE→AFTER
expected value quoted from a verified current-state snapshot (S1 landed: 4 docs
unified to 22 `[[rule]]`, src/ clean; llms_full frozen with 25 split tokens + the
withdrawn `≥ 224` floor). The generator is committed, syntax-clean, executable,
cwd-independent, and overwrites exactly one git-tracked file. The only residual
risk — regenerating before S1 lands (baking the stale schema into a fresh concat)
— is gated by an explicit Task-1 precondition check. The two grep-scoping traps
(plan/.pi-subagents/target carry split-token noise; docs/vendor/ carries "224"
noise) and the withdrawal-note-is-intentional distinction are all called out with
exact scoped commands. No code, no build beyond a sanity check: the gate is purely
textual and self-verifying.