# PRP — P1.M1.T3.S1: Audit & update README.md + top-level docs for the unified `[[rule]]` schema

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`).
> **Scope:** A read-heavy **verification sweep** (the C8 changeset's final coherence
> gate) confirming `README.md` + every top-level/overview doc + the `spec/*.md`
> files carry **zero stale split-schema references** (`layer_rules`/`callback_rules`/
> `LayerRule`/`CallbackRule`) and don't describe the old two-table schema. This is
> the **Mode B** changeset-level doc sweep; it runs **last** (after T1 code + T2
> docs). **Expected outcome: a no-op** — verified clean — recorded as a grep-evidence
> report. Make a targeted edit ONLY if a stale reference is actually found.
> **Files touched:** at most `README.md` / a `docs/*.md` / a `spec/*.md` (only if the
> audit finds something stale). **Do NOT touch `src/`** (S3's domain) or
> `docs/llms_full.txt` (T2.S2 owns it).

---

## Goal

**Feature Goal**: **Verify**, against the actual repo state (not the delta PRD's
assertion), that `README.md` and every top-level / overview / spec doc are consistent
with the **unified `[[rule]]` schema** (P1.M1.T1/T2) — i.e. they contain **no**
`layer_rules`/`callback_rules`/`LayerRule`/`CallbackRule` references and don't show the
old split two-table schema. If any are found, fix them per the item's conditional
rules. If (expected) none are found, record the clean audit as a verification report.

**Deliverable**:
1. A **grep-evidence verification report** (the four greps below + their output + the
   README-blurb / spec-section analysis) — captured in the commit message if any
   incidental edit is made, or in the task-completion record if a pure no-op.
2. **Zero edits** if the audit is clean (the overwhelmingly likely outcome — see
   Evidence), OR a targeted edit to the single stale doc if one is found.
3. (Conditional) A re-run of `bash docs/generate_llms_full.sh` IF any concatenated
   source (`README.md` or a `docs/*.md`) was edited, so `docs/llms_full.txt` stays in
   sync (see Coordination with T2.S2).

**Success Definition** (all gates — verified-true expected values below):
- (a) `grep -rnE 'layer_rules|callback_rules|LayerRule|CallbackRule' README.md AGENTS.md REMAINING_ISSUES.md docs/*.md spec/*.md` → **0 hits**.
- (b) `[[rule]]` appears **only** in `spec/HOST_RULES.md` + the 4 S1-unified docs
      (`docs/{configuration,examples,qmk-integration,troubleshooting}.md`) — NOT in
      README/index/usage/spec-overview.
- (c) `README.md`'s `rules.toml` mention is the version-agnostic feature blurb
      (points to `docs/configuration.md` for the schema) — **left as-is**.
- (d) The spec sections the item names (ARCHITECTURE §5.7, PROTOCOL §8, UI
      §1.1/§1.2/§2.3) describe behavior/pointers, not an inline schema — **left as-is**.
- (e) No withdrawn `≥ 224` floor **guidance** in top-level docs (only HOST_RULES.md's
      intentional withdrawal note, which is correct).

## User Persona (if applicable)

**Target User**: A user (or AI agent) who skims `README.md` / a spec overview to
understand the host-rules feature, then opens `docs/configuration.md` or
`HOST_RULES.md` §9 for the schema. They must never see a stale two-table example that
contradicts the unified `[[rule]]` form the parser actually accepts.

**Use Case**: After the C8 unification ships, a reviewer/agent runs this audit to
certify the changeset is internally coherent across ALL doc surfaces — not just the
4 docs S1 rewrote, but every README blurb and spec pointer too.

**Pain Points Addressed**: A stale `[[layer_rules]]`/`[[callback_rules]]` example in an
overview doc would silently mislead (the unified parser drops unknown fields ⇒ empty
ruleset). This audit proves no such drift exists.

## Why

- **The SOW requires verification, not assumption.** The delta PRD §4 *asserted*
  Mode B = "none" (README is a version-agnostic blurb; no overview references the split
  tables). This task **verifies that assertion against the live repo** — the documented
  grep evidence is the deliverable, not just the conclusion.
- **It's the final coherence gate for the unified-`[[rule]]` changeset (C8).** T1
  unified the code (S1–S3), T2 unified the 4 user-facing docs + regenerated llms_full.
  This sweep covers the REMAINING doc surfaces (README, top-level `*.md`, the `spec/`
  overview/pointers) so nothing contradicts the unified schema a user copies.
- **It guards the spec pointer pattern.** The spec docs deliberately defer the schema
  to `HOST_RULES.md §9` and describe only behavior. This audit confirms that pattern
  holds (so a future schema change needs only the one edit in HOST_RULES.md + the 4 docs).

## What

Run the verification greps, review the named sections, and apply the item's
**conditional edit logic** only if a stale reference is found. **All commands run from
the repo root `/home/dustin/projects/qmkonnect`.** Scope greps to the doc set
(`README.md AGENTS.md REMAINING_ISSUES.md docs/*.md spec/*.md`) — **do NOT repo-wide
grep** (see Known Gotchas: `plan/`, `.pi-subagents/`, `target/`, `docs/vendor/` carry
the tokens as PRP/transcript/glyph noise).

### Gate (a) — split-schema tokens must be ZERO (primary gate)

```bash
grep -rnE 'layer_rules|callback_rules|LayerRule|CallbackRule' \
  README.md AGENTS.md REMAINING_ISSUES.md docs/*.md spec/*.md
# Expected: NO output (exit 1). Verified-clean 2026-07-31.
# Repo-wide *.md sanity (excluding the known noise dirs):
grep -rnE 'layer_rules|callback_rules|LayerRule|CallbackRule' --include='*.md' . \
  | grep -vE '/(target|plan|\.pi-subagents|docs/vendor)/'
# Expected: NO output. (Confirms no stray .md anywhere references the split schema.)
```

### Gate (b) — `[[rule]]` lives only where the schema is described

```bash
grep -rlnE '\[\[rule[s]?\]\]' --include='*.md' . \
  | grep -vE '/(target|plan|\.pi-subagents|docs/vendor)/' | sort
# Expected EXACTLY these 5 files:
#   docs/configuration.md
#   docs/examples.md
#   docs/qmk-integration.md
#   docs/troubleshooting.md
#   spec/HOST_RULES.md
# (The schema is described ONLY in HOST_RULES.md §9 + the 4 S1-unified docs.
#  README/index/usage/spec-overview must NOT appear here — they point, never inline.)
```

### Gate (c) — README's `rules.toml` blurb is version-agnostic → LEAVE

Read `README.md` lines ~34–41 (the "Host-Side Window Rules" feature bullets). It says:
*"edit a `rules.toml` file … (the **Edit rules** tray item opens it; changes
hot-reload) … Host rules stack on top of your board's `DEFINE_SERIAL_*` rules … Full
schema … see the [Configuration Guide](docs/configuration.md)"*. This names **no**
fields and shows **no** `[[…]]` block — it points to `docs/configuration.md`. Per the
item: *"if it's the version-agnostic one-liner, leave it."* → **NO EDIT.**

### Gate (d) — named spec sections are behavior/pointers → LEAVE

Spot-check that these describe behavior and defer to `HOST_RULES.md`, NOT an inline
schema (verified-clean — they contain zero split tokens):
- `spec/ARCHITECTURE.md` §5.7 "Host-side-rules extension" (~lines 268–285): "the full
  design is in `HOST_RULES.md` … evaluates `rules.toml` … `APPLY_HOST_CONTEXT` …".
- `spec/PROTOCOL.md` §8 "Typed-Command Namespace" (~lines 281–335): "(… `rules.toml`)
  is in `HOST_RULES.md`" + the handshake + `disable_firmware_config`.
- `spec/UI.md` §1.1 (~line 31), §1.2 (~line 54), §2.3 (~line 137): the "Edit rules"
  tray items (seed/open `rules.toml`) + the "rules.toml invalid" notification.
- (Bonus pointers, also clean:) `spec/CONFIG.md` (~111–115 "Schema: HOST_RULES.md §9"),
  `spec/PRD.md` (F11/F12 feature row + glossary entries → `HOST_RULES.md`).

### Gate (e) — no withdrawn `≥ 224` floor GUIDANCE in top-level docs

```bash
grep -rnE '≥ 224|>= 224|must be.*224' \
  README.md AGENTS.md REMAINING_ISSUES.md docs/index.md docs/usage.md \
  docs/installation.md docs/README.md spec/*.md
# Expected: ONE hit — spec/HOST_RULES.md:134, the INTENTIONAL withdrawal note
# ("*(The earlier '≥ 224' reservation is withdrawn: …)*"). That is CORRECT (it
# explains the withdrawal), NOT stale guidance. Do NOT "fix" it.
```

### Conditional edit logic (apply ONLY if a gate FAILS — not expected)

If Gate (a) or (b) finds a stale reference in `README.md` or a top-level/spec doc:
- **README blurb / feature row** that merely *names* `rules.toml` and points elsewhere
  → **leave** (version-agnostic; correct by design).
- **Inline schema** (a `[[layer_rules]]`/`[[callback_rules]]` block or a two-table
  field reference) → **rewrite to the unified `[[rule]]` form**, mirroring
  `spec/HOST_RULES.md` §9 (the source of truth: `match` required; `layer` optional,
  raw QMK index `!= 255`; `enable`/`disable` optional name lists; `case_sensitive`
  default `false`; `disable_firmware_config` optional override; ≥1-of-`(layer|enable|
  disable)` validity). Do NOT re-introduce a `224` floor.
- **Pointer** ("see HOST_RULES.md §9") → **leave** (the pointer is correct regardless
  of schema version).
After ANY edit to `README.md` or a `docs/*.md` (a concatenated llms_full source),
re-run `bash docs/generate_llms_full.sh` (see Coordination).

### Success Criteria

- [ ] Gate (a): zero `layer_rules`/`callback_rules`/`LayerRule`/`CallbackRule` in the doc set.
- [ ] Gate (b): `[[rule]]` only in the 4 S1 docs + `spec/HOST_RULES.md`.
- [ ] Gate (c): README blurb is version-agnostic (no field names / `[[…]]`) — left as-is.
- [ ] Gate (d): the named spec sections defer to `HOST_RULES.md` — left as-is.
- [ ] Gate (e): no `≥ 224` floor guidance (only the intentional HOST_RULES.md withdrawal note).
- [ ] If no edits were needed: the verification report (the 5 greps + their clean output
      + the section analysis) is recorded (commit message if an incidental edit, else
      task-completion record).
- [ ] If an edit WAS made to a concatenated source: `docs/llms_full.txt` re-regenerated
      via the script, and committed together with the edit.
- [ ] `src/` untouched; `docs/llms_full.txt` untouched UNLESS a source was edited.

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed to
> do this audit successfully?"_ — **Yes.** The exact 5 greps with their verified-clean
> expected output, the README-blurb / spec-section analysis (with line numbers), the
> conditional edit logic keyed to `spec/HOST_RULES.md` §9 as the source of truth, the
> grep-scoping trap (noise dirs), the llms_full coordination rule, and the dependency
> preconditions are all below. No source-file reading is required beyond the greps +
> the spot-check reads.

### Documentation & References

```yaml
# MUST READ — the source of truth for the unified schema (mirror this if an edit is needed).
- file: /home/dustin/projects/qmkonnect/spec/HOST_RULES.md
  why: "§9 is the canonical unified [[rule]] schema (TOML + Rust Rule model + the
        '≥1-of-(layer|enable|disable)' Validity paragraph). Verified ALREADY correct
        (references [[rule]], struct Rule, rename = \"rule\"). If a stale reference is
        found in README/top-level, rewrite it to match §9 EXACTLY. Otherwise leave §9."
  section: "§9 rules.toml Schema Reference, §4 (stack vs replace), §6 callback registry"
  critical: "§9 is the ONLY place the schema is described authoritatively. README/index/
             usage/spec-overview POINT to it (behavior blurbs); they must never inline
             a competing schema. The intentional '≥ 224 withdrawn' note at line 134 is
             CORRECT — do not treat it as stale."

# MUST READ — the docs this audit's siblings already unified (do NOT re-edit; just verify).
- file: /home/dustin/projects/qmkonnect/docs/configuration.md
  why: "P1.M1.T2.S1 rewrote its schema table to unified [[rule]] (13 hits). This audit
        CONFIRMS it's clean (Gate a/b) — it is NOT in scope to re-edit unless a gate fails."
- file: /home/dustin/projects/qmkonnect/docs/{examples,qmk-integration,troubleshooting}.md
  why: "The other 3 S1-unified docs. Same: verify-only."

# MUST READ — the PRP for the parallel sibling (llms_full owner + the README-regen rule).
- file: /home/dustin/projects/qmkonnect/plan/004_f48a103bcb32/P1M1T2S2/PRP.md
  why: "T2.S2 regenerates docs/llms_full.txt (concatenates README.md + 7 docs/*.md via
        docs/generate_llms_full.sh). Its PRP explicitly states: 'if T3.S1 edits README,
        llms_full must be re-regenerated.' This task's Gate (c) finds README needs NO
        edit, so normally no regen is needed — but the conditional-edit step MUST
        re-run the script if a concatenated source is touched."
  section: "Integration Points (DOWNSTREAM: P1.M1.T3.S1), Anti-Patterns"

# REFERENCE — the delta PRD §4 (the Mode B 'none' assertion this task verifies).
- file: /home/dustin/projects/qmkonnect/plan/004_f48a103bcb32/delta_prd.md
  why: "§4 Documentation Impact Summary asserts Mode B = 'none' (README is a
        version-agnostic blurb; no overview references the split tables). This audit
        VERIFY that assertion with grep evidence rather than assuming it."
  section: "§4 Documentation Impact Summary"

# REFERENCE — research notes for THIS subtask (the full verified evidence snapshot).
- docfile: /home/dustin/projects/qmkonnect/plan/004_f48a103bcb32/P1M1T3S1/research/notes.md
  why: "The 5 greps with their 2026-07-31 clean output, the README-blurb / spec-section
        line-by-line analysis, the out-of-scope src/ comment note, the llms_full
        coordination rationale, and the no-op deliverable shape."
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/
├── README.md                 # <-- AUDIT (rules blurb lines ~34-41; version-agnostic → leave)
├── AGENTS.md                 # <-- AUDIT (no rules.toml mention; expected clean)
├── REMAINING_ISSUES.md       # <-- AUDIT (expected clean)
├── docs/
│   ├── index.md              # <-- AUDIT (qmk_notifier pointer only; expected clean)
│   ├── installation.md       # <-- AUDIT (expected clean)
│   ├── usage.md              # <-- AUDIT (expected clean)
│   ├── README.md             # <-- AUDIT (expected clean)
│   ├── configuration.md      # S1-unified [[rule]] x13 (verify-only; NOT re-edit)
│   ├── examples.md           # S1-unified [[rule]] x5  (verify-only)
│   ├── qmk-integration.md    # S1-unified [[rule]] x3  (verify-only)
│   ├── troubleshooting.md    # S1-unified [[rule]] x1  (verify-only)
│   ├── llms_full.txt         # T2.S2 owns (regenerated; do NOT touch unless a source edited)
│   └── generate_llms_full.sh # the generator (run ONLY if a concatenated source is edited)
└── spec/
    ├── HOST_RULES.md         # source of truth §9 [[rule]] (verify clean; line 134 withdrawal note is CORRECT)
    ├── ARCHITECTURE.md       # <-- AUDIT §5.7 (behavior+pointer; expected clean)
    ├── PROTOCOL.md           # <-- AUDIT §8 (pointer; expected clean)
    ├── UI.md                 # <-- AUDIT §1.1/§1.2/§2.3 (tray behavior; expected clean)
    ├── CONFIG.md             # <-- AUDIT (path + "Schema: HOST_RULES.md §9"; expected clean)
    ├── PRD.md                # <-- AUDIT (F11/F12 + glossary → HOST_RULES.md; expected clean)
    └── (FIRMWARE/LINUX/PACKAGING/PLATFORMS.md — not rules-schema relevant; clean)
```

### Desired Codebase tree with files to be modified

```bash
# EXPECTED: NO files modified (pure no-op verification). If a gate fails, at most ONE of:
README.md   # OR a docs/*.md OR a spec/*.md  — rewritten to unified [[rule]] per HOST_RULES.md §9
docs/llms_full.txt   # ONLY re-regenerated (via the script) if README.md or a docs/*.md was edited
# (src/ is NEVER touched by this task — it is S3's domain, Complete)
```

### Known Gotchas of our codebase & Library Quirks

```bash
# CRITICAL: scope your greps. The split tokens appear as NOISE outside the doc set:
#     plan/            — 28 PRP files; their OLD-text blocks literally contain [[layer_rules]]
#     .pi-subagents/   — session transcripts
#     target/          — build artifacts
#     docs/vendor/     — Ruby gem / Jekyll font SVGs (thousands of "224"/">=" glyph coords)
#   A repo-wide grep floods with these. Scope to the doc set
#   (README.md AGENTS.md REMAINING_ISSUES.md docs/*.md spec/*.md). The repo-wide *.md
#   sanity grep MUST exclude those 4 dirs (the grep -vE pattern in Gate a/b).

# CRITICAL: do NOT edit src/. The one src/ hit (src/core/rules.rs:347 comment mentions
#   LayerRule/CallbackRule) is CODE, not a doc — it's P1.M1.T1.S3's responsibility
#   (marked Complete). This task's scope is README + top-level DOCS only. Editing src/
#   here is a scope violation + collides with S3.

# CRITICAL: the HOST_RULES.md:134 "≥ 224 withdrawn" note is CORRECT, not stale. Gate (e)
#   greps the GUIDANCE forms (≥ 224 / >= 224 / must be ...224). The single hit is the
#   note EXPLAINING the withdrawal ("The earlier '≥ 224' reservation is withdrawn").
#   Do NOT delete or "fix" it — it is intentional documentation.

# CRITICAL: README.md is a CONCATENATED llms_full source. If you edit it (not expected),
#   you MUST re-run `bash docs/generate_llms_full.sh` so docs/llms_full.txt (which
#   T2.S2 just regenerated) stays in sync. T2.S2's PRP flags this exact dependency.
#   If README is NOT edited (expected), do NOT touch llms_full.

# GOTCHA: the audit is VERIFY, not RE-DESCRIBE. Do not add a `[[rule]]` schema block to
#   README or a spec overview "for completeness" — the design is that those docs POINT
#   to HOST_RULES.md §9 / docs/configuration.md and describe only behavior. Adding an
#   inline schema would create a SECOND source of truth that drifts on the next change.

# GOTCHA: AGENTS.md / REMAINING_ISSUES.md are in the grep scope but are dev-facing; they
#   don't mention rules.toml at all (verified). Expected clean — leave them.

# NOTE: this task depends on T1 (code, Complete) + T2 (docs). Verify T2's 4 docs are
#   unified BEFORE trusting Gate (b)'s "exactly 5 files" (if S1 hasn't landed, the count
#   differs). The plan_status shows T2.S1 Complete, T2.S2 Implementing — S1's 4 docs are
#   done, so Gate (b)'s 5-file expectation holds now.
```

## Implementation Blueprint

### Data models and structure

No data models. This is a "run greps + spot-check + (conditionally) edit one doc +
report" task. The "structure" is the 5-gate verification + the conditional edit logic.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: VERIFY PRECONDITIONS (T1 code + T2 docs landed)
  - RUN: grep -c '\[\[rule\]\]' docs/configuration.md docs/examples.md \
          docs/qmk-integration.md docs/troubleshooting.md
  - EXPECT: each > 0 (S1 unified the 4 docs). If any is 0, S1 hasn't landed — STOP;
          re-check plan status (the audit is meaningless against stale docs).
  - RUN: grep -rnE 'layer_rules|callback_rules' src/   # S3 sanity (should be 0 except the rules.rs:347 comment)
  - NOTE: src/core/rules.rs:347's comment mentions LayerRule/CallbackRule — that is
          OUT OF SCOPE (src/, S3's domain). Do NOT fix it here.

Task 2: RUN Gate (a) — the primary split-token grep (must be 0)
  - RUN: grep -rnE 'layer_rules|callback_rules|LayerRule|CallbackRule' \
          README.md AGENTS.md REMAINING_ISSUES.md docs/*.md spec/*.md
  - EXPECT: NO output (exit 1). RECORD "0 hits" for the report.
  - REPO-WIDE sanity: grep -rnE 'layer_rules|callback_rules|LayerRule|CallbackRule' \
          --include='*.md' . | grep -vE '/(target|plan|\.pi-subagents|docs/vendor)/'
    EXPECT: NO output. (Catches any stray .md outside the doc set.)

Task 3: RUN Gate (b) — [[rule]] lives only in the 5 expected files
  - RUN: grep -rlnE '\[\[rule[s]?\]\]' --include='*.md' . \
          | grep -vE '/(target|plan|\.pi-subagents|docs/vendor)/' | sort
  - EXPECT: EXACTLY docs/{configuration,examples,qmk-integration,troubleshooting}.md +
          spec/HOST_RULES.md (5 files). RECORD the list for the report.
  - IF README/index/usage/spec-overview appears here: a doc inlines the schema
          unexpectedly → review it (Gate c/d logic) and decide leave-vs-rewrite.

Task 4: REVIEW Gate (c) — README's rules.toml blurb (lines ~34-41)
  - READ README.md lines 34-41. CONFIRM: version-agnostic feature bullets, NO field
          names, NO [[...]] block, points to docs/configuration.md for the schema.
  - DECISION: LEAVE (per the item "if version-agnostic one-liner, leave it").
  - RECORD: "README blurb version-agnostic → no edit" for the report.

Task 5: REVIEW Gate (d) — the named spec sections (behavior/pointers)
  - READ spec/ARCHITECTURE.md §5.7 (~268-285), spec/PROTOCOL.md §8 (~281-335),
          spec/UI.md §1.1 (~31), §1.2 (~54), §2.3 (~137). CONFIRM each describes
          behavior + defers to HOST_RULES.md (no inline schema).
  - DECISION: LEAVE (they never showed the split schema; they point to HOST_RULES.md).
  - RECORD: "spec sections are behavior/pointers → no edit" for the report.

Task 6: RUN Gate (e) — no withdrawn ≥224 floor GUIDANCE
  - RUN: grep -rnE '≥ 224|>= 224|must be.*224' README.md AGENTS.md REMAINING_ISSUES.md \
          docs/index.md docs/usage.md docs/installation.md docs/README.md spec/*.md
  - EXPECT: ONE hit — spec/HOST_RULES.md:134 (the intentional withdrawal note).
  - DECISION: LEAVE (correct documentation). RECORD for the report.

Task 7: CONDITIONAL EDIT (only if a gate failed — not expected)
  - IF Gate (a)/(b) found a stale split reference in a doc:
      - README blurb / pointer → leave.
      - inline schema block → rewrite to unified [[rule]] per spec/HOST_RULES.md §9.
  - IF you edited README.md or any docs/*.md (a concatenated llms_full source):
      - RE-RUN: bash docs/generate_llms_full.sh   (so llms_full stays in sync)
      - VERIFY: grep -rnE 'layer_rules|callback_rules' docs/ src/ → 0 (T2.S2's gate a)
  - IF no gate failed: NO edit. Skip to Task 8.

Task 8: PRODUCE THE VERIFICATION REPORT (the deliverable)
  - CAPTURE the 5 greps + their (clean) output + the README-blurb / spec-section
          analysis. This IS the deliverable.
  - IF an edit was made: put the report in the commit message; commit the edit (+ the
          regenerated llms_full if applicable) as one coherent change.
  - IF no edit (expected no-op): record the report in the task-completion record
          (the audit's evidence that Mode B = 'none' holds against the live repo).
```

### Implementation Patterns & Key Details

```bash
# === WHY this is verify-not-edit ===
#   The design INTENT is that README + spec overviews POINT to HOST_RULES.md §9 /
#   docs/configuration.md and describe only behavior. They never inlined the schema
#   (not even the old split form), so unification can't have left them stale. The
#   audit CONFIRMS that intent holds — it doesn't "improve" the docs by adding a schema.

# === WHY scope greps narrowly ===
#   plan/ holds 28 PRPs whose OLD-text blocks contain [[layer_rules]] (they document
#   the before→after). .pi-subagents/ holds transcripts. docs/vendor/ holds Ruby gem
#   SVGs. A repo-wide grep floods with these. The contract's "excluding target/,
#   plan/, .pi-subagents/" note exists BECAUSE those carry the tokens as noise.

# === WHY the HOST_RULES.md:134 "224" note is NOT a failure ===
#   Gate (e) greps GUIDANCE forms (≥ 224 / >= 224). The one hit is the note EXPLAINING
#   the floor was withdrawn ("The earlier '≥ 224' reservation is withdrawn"). Deleting
#   it would remove useful migration guidance. It is correct documentation.

# === WHY re-run the llms_full generator conditionally ===
#   docs/llms_full.txt is a concatenation of README.md + 7 docs/*.md (T2.S2 regenerates
#   it). If THIS task edits a concatenated source, the dump goes stale until re-run.
#   README is expected clean (no edit), so normally no regen — but the conditional step
#   guards the edge case. (T2.S2's PRP flags this exact dependency.)
```

### Integration Points

```yaml
SOURCE FILES (audit targets):
  - audit (read-only, expected clean): "README.md, AGENTS.md, REMAINING_ISSUES.md,
    docs/{index,installation,usage,README}.md, spec/{ARCHITECTURE,PROTOCOL,UI,CONFIG,
    PRD,HOST_RULES,FIRMWARE,LINUX,PACKAGING,PLATFORMS}.md"
  - verify-only (S1 owns; do NOT re-edit): "docs/{configuration,examples,
    qmk-integration,troubleshooting}.md"
  - conditional edit (only if a gate fails): "the single stale doc found"
  - conditional regenerate (only if a concatenated source edited): "docs/llms_full.txt
    via bash docs/generate_llms_full.sh"

UPSTREAM CONTRACT (must be LANDED before this audit is meaningful):
  - P1.M1.T1.S1–S3 (Complete): unified Rule/RuleSet model + evaluator + all src/ callers.
  - P1.M1.T2.S1 (Complete): the 4 docs/*.md unified to [[rule]].
  - P1.M1.T2.S2 (Implementing): docs/llms_full.txt regenerated (not strictly required
    for THIS audit — llms_full is OUT of this task's grep scope — but its regeneration
    closes the docs-unification; this task is the last coherence gate).

PARALLEL / SIBLING:
  - P1.M1.T2.S2 (in flight): owns docs/llms_full.txt. Coordinate: if this task edits
    README or a docs/*.md, re-run its generator so llms_full stays in sync.

DOWNSTREAM:
  - None. This is the final task of P1.M1 (the C8 changeset's last sweep).

OUT-OF-SCOPE (do NOT touch):
  - src/ (S3, Complete — incl. the rules.rs:347 comment that mentions LayerRule/CallbackRule).
  - docs/llms_full.txt (T2.S2 owns; regenerate via script only if a source is edited).
  - docs/generate_llms_full.sh (never edit the generator).
  - PRD.md / tasks.json / prd_snapshot.md (forbidden — orchestrator-owned).

PUBLIC API SURFACE: none. Pure docs audit (+ conditional doc edit).
```

## Validation Loop

> All commands run from the repo root: `/home/dustin/projects/qmkonnect`.

### Level 1: The 5 verification gates (these ARE the implementation)

```bash
cd /home/dustin/projects/qmkonnect

# (a) PRIMARY — split-schema tokens in the doc set (must be 0).
grep -rnE 'layer_rules|callback_rules|LayerRule|CallbackRule' \
  README.md AGENTS.md REMAINING_ISSUES.md docs/*.md spec/*.md
# Expected: NO output (exit 1).
# Repo-wide *.md sanity (excluding noise dirs):
grep -rnE 'layer_rules|callback_rules|LayerRule|CallbackRule' --include='*.md' . \
  | grep -vE '/(target|plan|\.pi-subagents|docs/vendor)/'
# Expected: NO output.

# (b) [[rule]] lives only in the 5 expected files.
grep -rlnE '\[\[rule[s]?\]\]' --include='*.md' . \
  | grep -vE '/(target|plan|\.pi-subagents|docs/vendor)/' | sort
# Expected: docs/configuration.md, docs/examples.md, docs/qmk-integration.md,
#   docs/troubleshooting.md, spec/HOST_RULES.md (exactly 5).

# (c) README blurb (read lines 34-41) — version-agnostic, points to configuration.md.
sed -n '34,41p' README.md
# Expected: feature bullets naming rules.toml + "see the Configuration Guide"; NO fields/[[...]].

# (d) Named spec sections — behavior/pointers (read; confirm no inline schema).
sed -n '268,285p' spec/ARCHITECTURE.md   # §5.7 → "full design is in HOST_RULES.md"
sed -n '281,335p' spec/PROTOCOL.md       # §8  → "(…rules.toml) is in HOST_RULES.md"
sed -n '28,33p;52,56p;135,140p' spec/UI.md  # §1.1/§1.2/§2.3 → tray "Edit rules" + invalid notification

# (e) No withdrawn ≥224 floor GUIDANCE (only the intentional HOST_RULES.md note).
grep -rnE '≥ 224|>= 224|must be.*224' \
  README.md AGENTS.md REMAINING_ISSUES.md docs/index.md docs/usage.md \
  docs/installation.md docs/README.md spec/*.md
# Expected: ONE hit — spec/HOST_RULES.md:134 (the withdrawal note; CORRECT, leave it).
```

### Level 2: Conditional-edit verification (ONLY if Task 7 made an edit)

```bash
cd /home/dustin/projects/qmkonnect
# If README.md or a docs/*.md was edited, re-run the generator + re-verify.
bash docs/generate_llms_full.sh
grep -rnE 'layer_rules|callback_rules|LayerRule|CallbackRule' docs/ src/   # → 0 (T2.S2's gate a)
grep -cE '\[\[rule\]\]' docs/llms_full.txt   # → ≥ 20 (the unified schema propagated)
git diff --stat   # the edited doc + docs/llms_full.txt only
```

### Level 3: Sanity (the tree still builds — only if any edit touched nothing code-side)

```bash
cd /home/dustin/projects/qmkonnect
# Only meaningful if you're unsure an edit had side effects. This audit edits no Rust,
# so these are pure insurance (expected unchanged/passing).
cargo test --bin qmkonnect -- --test-threads=1   # pass (single-threaded: shared debouncer)
cargo check --bin qmkonnect --offline            # clean (if --offline fails on a fresh clone, drop it)
```

### Level 4: Report completeness

```bash
cd /home/dustin/projects/qmkonnect
# The deliverable is the REPORT. Confirm it captures all 5 gates' evidence:
#   - Gate (a) output (0 hits) + repo-wide sanity (0 hits)
#   - Gate (b) the 5-file list
#   - Gate (c) README blurb verbatim + "version-agnostic → leave"
#   - Gate (d) the spec-section excerpts + "behavior/pointer → leave"
#   - Gate (e) the single HOST_RULES.md:134 hit + "intentional withdrawal note → leave"
# If a no-op: record in the task-completion record. If edited: in the commit message.
```

## Final Validation Checklist

### Technical Validation
- [ ] Task 1 precondition: the 4 S1 docs show `[[rule]]` > 0 (T2.S1 landed).
- [ ] Gate (a): split tokens in the doc set → **0 hits**; repo-wide `*.md` sanity → 0.
- [ ] Gate (b): `[[rule]]` in **exactly** the 4 S1 docs + `spec/HOST_RULES.md` (5 files).
- [ ] Gate (c): README blurb version-agnostic (no fields/`[[…]]`, points to configuration.md) → left.
- [ ] Gate (d): ARCHITECTURE §5.7 / PROTOCOL §8 / UI §1.1·§1.2·§2.3 are behavior+pointers → left.
- [ ] Gate (e): only `spec/HOST_RULES.md:134` matches the `≥ 224` grep (the withdrawal note; left).
- [ ] If an edit was made: `cargo test --bin qmkonnect -- --test-threads=1` still passes.

### Feature Validation
- [ ] No stale `layer_rules`/`callback_rules`/`LayerRule`/`CallbackRule` anywhere in the docs.
- [ ] The unified `[[rule]]` schema is described ONLY in `spec/HOST_RULES.md` §9 + the 4 S1 docs.
- [ ] README + spec overviews defer to those sources (no competing inline schema introduced).
- [ ] Verification report captures all 5 gates' evidence (the deliverable).

### Code Quality Validation
- [ ] No doc was edited UNLESS a gate found a stale reference (the audit is verify-first).
- [ ] No NEW inline schema added "for completeness" (single source of truth preserved).
- [ ] `src/` untouched (incl. the out-of-scope `rules.rs:347` comment — S3's domain).
- [ ] `docs/llms_full.txt` untouched UNLESS a concatenated source was edited (then re-generated).
- [ ] Greps were scoped (not repo-wide) — no confusion with `plan/`/`.pi-subagents/`/`target/`/`docs/vendor/` noise.

### Documentation & Deployment
- [ ] The verification report is recorded (commit message if an edit; task-completion if no-op).
- [ ] If a concatenated source was edited: llms_full re-generated + committed together.
- [ ] No PRD.md / tasks.json / prd_snapshot.md / .gitignore changes (forbidden).

---

## Anti-Patterns to Avoid

- ❌ Don't ASSUME the Mode B "none" conclusion — VERIFY it with the greps (the SOW
  explicitly requires verification against the repo, not reliance on the delta PRD's
  assertion). The grep evidence IS the deliverable.
- ❌ Don't edit `src/`. The `src/core/rules.rs:347` comment mentioning
  `LayerRule`/`CallbackRule` is CODE (S3's domain, Complete), not a doc. This task's
  scope is README + top-level DOCS. Editing src/ is a scope violation.
- ❌ Don't repo-wide grep without the exclusions — `plan/` (28 PRPs with OLD-text
  `[[layer_rules]]` blocks), `.pi-subagents/` (transcripts), `target/` (artifacts), and
  `docs/vendor/` (Ruby gem SVGs with thousands of "224"/">=" glyph coords) all flood the
  results. Scope to the doc set; for the repo-wide `*.md` sanity, use the `grep -vE`
  exclusion pattern.
- ❌ Don't treat `spec/HOST_RULES.md:134`'s "≥ 224 withdrawn" note as stale — it's the
  INTENTIONAL migration note explaining the floor was withdrawn. Gate (e) expects exactly
  that one hit. Deleting it removes useful guidance.
- ❌ Don't add a `[[rule]]` schema block to README or a spec overview "for completeness" —
  the design is that those docs POINT to `HOST_RULES.md` §9 / `docs/configuration.md` and
  describe only behavior. Adding a second inline schema creates drift on the next change.
- ❌ Don't edit README's version-agnostic blurb — it names no fields and points to the
  Configuration Guide; it's correct for ANY schema version. Per the item: leave it.
- ❌ Don't touch `docs/llms_full.txt` by hand — if a concatenated source (`README.md` or a
  `docs/*.md`) was edited, re-run `bash docs/generate_llms_full.sh` (T2.S2's generator);
  never hand-edit the concatenation.
- ❌ Don't forget the llms_full coordination with T2.S2 — its PRP states "if T3.S1 edits
  README, llms_full must be re-regenerated." README is expected clean (no edit), so
  normally no regen — but if you DO edit a source, re-run the generator.
- ❌ Don't run this audit before T2.S1 landed — Gate (b)'s "exactly 5 files" expectation
  assumes the 4 docs are already unified. Verify the precondition (Task 1) first.
- ❌ Don't manufacture work — if all 5 gates are clean (expected), the task is a NO-OP and
  the report is the deliverable. Don't invent edits to "justify" the task.

---

**Confidence Score: 10/10** for one-pass success. The task is a deterministic verification
sweep whose expected output is verified-clean greps (zero split tokens in the doc set;
`[[rule]]` in exactly the 4 S1 docs + `spec/HOST_RULES.md`; README + spec sections are
behavior/pointers). Every gate has its exact verified-true expected value quoted from a
2026-07-31 snapshot of the live repo, the conditional-edit logic is keyed to
`spec/HOST_RULES.md` §9 as the single source of truth, the grep-scoping trap
(`plan/`/`.pi-subagents/`/`target/`/`docs/vendor/` noise) and the HOST_RULES.md:134
withdrawal-note distinction are both called out, the out-of-scope `src/` comment is
flagged, and the llms_full coordination with the parallel T2.S2 is documented. The only
"work" is running 5 greps + 4 spot-check reads + recording the report — and the rare
edge case (a gate fails) has explicit, source-of-truth-keyed remediation.