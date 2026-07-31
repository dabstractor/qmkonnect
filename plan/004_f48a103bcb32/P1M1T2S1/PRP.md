# PRP — P1.M1.T2.S1: Rewrite the `[[rule]]` schema in `docs/{configuration,examples,qmk-integration,troubleshooting}.md`

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`).
> **Scope:** The **4 user-facing doc files** whose TOML schema still shows the
> OLD split `[[layer_rules]]` + `[[callback_rules]]` arrays:
> `docs/configuration.md`, `docs/examples.md`, `docs/qmk-integration.md`,
> `docs/troubleshooting.md`. This is the **docs counterpart** of S3 (which owns
> the `src/` code callers). **No `src/` edits; no `llms_full.txt`** (regenerated
> in S2). **No `spec/` edits** (it is the READ-ONLY source of truth).
> **What changes:** every `[[layer_rules]]`/`[[callback_rules]]` TOML literal →
> the unified `[[rule]]` (SINGULAR), the schema table collapses from 11 split rows
> to 7 unified rows, `layer` is documented as **optional** (raw QMK index, `!=255`,
> `< layer_state_t`), and the "a rule must set ≥1 of layer/enable/disable"
> validity note replaces the old "requires match+layer".
> **Why it's docs-only:** S1 landed the unified `Rule`/`RuleSet` model in code;
> S3 rewired the callers + `render_rules_body` template; this task makes the
> user-facing `docs/*.md` agree byte-for-byte with `spec/HOST_RULES.md` §9.

---

## Goal

**Feature Goal**: Eliminate every stale `[[layer_rules]]` / `[[callback_rules]]`
TOML literal and every stale structural claim ("two table-arrays", "requires
match and layer") from the 4 user-facing doc files, rewriting them to the unified
`[[rule]]` schema that `spec/HOST_RULES.md` §9 already specifies — so a user
reading any of the 4 files sees ONE `[[rule]]` table-array with `layer` optional,
the ≥1-of-(layer/enable/disable) validity rule, and `layer` as a raw QMK index.

**Deliverable**: Edited `docs/configuration.md` (4 sites), `docs/examples.md`
(1 site), `docs/qmk-integration.md` (2 sites), `docs/troubleshooting.md` (1 site).
No new files.

**Success Definition**:
- `grep -rnE '\[\[(layer|callback)_rules\]\]' docs/ | grep -v llms_full.txt` →
  **ZERO hits** (the primary gate).
- `grep -rnE '\[\[rules\]\]' docs/` → **ZERO hits** (no plural mistake).
- The `docs/configuration.md` schema intro says "one table-array" (not "two").
- Every new TOML example uses `[[rule]]` (SINGULAR), with `layer` shown as
  optional and as a raw QMK index (no `224`/`225` floor guidance).
- `docs/troubleshooting.md` "Fix" says a rule must set "≥1 of layer/enable/disable",
  and `layer` is optional.
- No file other than the 4 listed is modified.

## User Persona (if applicable)

**Target User**: A power user editing `rules.toml` from the published docs. They
copy a TOML block from `docs/examples.md` or `docs/configuration.md`; if it still
shows `[[layer_rules]]`, `qmkonnect --validate-rules` silently parses an empty
ruleset (the unknown field is dropped under `#[serde(default)]`) and their rules
do nothing — a confusing, silent failure.

**Use Case**: User opens the Configuration Guide, copies the annotated `[[rule]]`
example, edits `match`/`layer`, validates with `--validate-rules`, and the rules
fire. Today the docs show the split schema; after this task they match the code.

**Pain Points Addressed**: Doc/code schema drift. S1 unified the model in code;
S3 fixed the `render_rules_body` template (what `qmkonnect -c` seeds); this task
fixes the hand-written prose docs so all four surfaces (code, seeded template,
spec, published docs) agree.

## Why

- **Closes the doc half of the schema unification (C8).** S1 + S3 unified the
  code + seeded template; the published `docs/*.md` are the last surface still
  showing `[[layer_rules]]`/`[[callback_rules]]`. Leaving them stale means the
  docs actively mislead users into writing a schema the parser no longer accepts.
- **Mirrors the canonical spec.** `spec/HOST_RULES.md` §9 is already correct and
  singular; this task propagates that wording into the 4 derived doc files.
- **Behavior-preserving.** Pure prose/TOML edits — no code, no config, no build.
  The unified schema is semantically equivalent to the split one (a
  `[[callback_rules]]` rule ⇔ a `[[rule]]` with `layer` unset + enable/disable; a
  `[[layer_rules]]` rule ⇔ a `[[rule]]` with `layer` set + empty enable/disable).

## What

A set of mechanical prose + TOML-block edits across 4 files. Every site is
enumerated below with its exact current text (verified against the tree) and the
verbatim replacement. The implementer applies each via the `edit` tool
(oldText → newText); whitespace in the `oldText` MUST match the file exactly.

### Success Criteria

- [ ] `grep -rnE '\[\[(layer|callback)_rules\]\]' docs/ | grep -v llms_full.txt`
      → 0 hits.
- [ ] `grep -rnE '\[\[rules\]\]' docs/` → 0 hits (singular everywhere).
- [ ] `docs/configuration.md` schema intro (line ~265) says "one table-array".
- [ ] `docs/configuration.md` schema table has a single `[[rule]]` row-set
      (match/layer/enable/disable/case_sensitive/disable_firmware_config) with the
      ≥1-of-(layer/enable/disable) validity note and `layer` documented optional.
- [ ] `docs/configuration.md` annotated TOML example uses 4 `[[rule]]` entries
      with one unified comment divider (mirrors spec §9).
- [ ] `docs/examples.md` Example-4 TOML uses `[[rule]]` (the two `steam_app*`
      rules merged into one demonstrating layer+enable in a single rule).
- [ ] `docs/qmk-integration.md` migration steps 2 & 3 say "add a `[[rule]]` entry
      with a `layer` field" / "with `enable`/`disable`"; example uses `[[rule]]`.
- [ ] `docs/troubleshooting.md` "Fix" says a rule requires match + ≥1 of
      layer/enable/disable; `layer` is optional.
- [ ] Only the 4 listed files are modified.

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything
> needed to implement this successfully?"_ — **Yes.** The canonical source
> (`spec/HOST_RULES.md` §9/§10, quoted), the exact current text of every site
> (quoted, with line numbers), the verbatim replacement for every site, the 5
> semantic deltas every edit must capture, the grep-gate validation, and the
> scope boundaries (no `src/`, no `llms_full.txt`, no `spec/`) are all below.

> **PARALLEL-SIBLING NOTE.** S3 (`src/` code callers) is being implemented IN
> PARALLEL. This task edits ONLY `docs/*.md` — there is NO file-level collision
> with S3 (which owns `src/`). Do NOT touch `src/` or `llms_full.txt`.

### Documentation & References

```yaml
# MUST READ — the canonical source of truth (copy FROM it; never edit it)
- file: /home/dustin/projects/qmkonnect/spec/HOST_RULES.md
  why: "§9 (lines 437-528) is the already-correct singular [[rule]] schema: the
        annotated TOML block (449-481) is the 4-[[rule]] example to mirror in
        configuration.md/examples.md; the divider prose (449-455) is the unified
        'one [[rule]] per (app × behavior)… MUST set ≥1 of layer/enable/disable…
        layer is a RAW QMK layer index (no fixed floor)' text. §10 (530-542) is
        the migration-step wording to mirror in qmk-integration.md."
  section: "9. rules.toml Schema Reference (437-528), 10. Migration from DEFINE_* (530-542)"
  critical: "[[rule]] is SINGULAR (serde rename = \"rule\"). layer is Option<u8>
             (OPTIONAL). A rule with none of layer/enable/disable is a parse error.
             No 224/225 layer floor (that guidance is withdrawn — examples.md says so)."

# MUST READ — the S1 contract (the unified model the docs must match)
- file: /home/dustin/projects/qmkonnect/plan/004_f48a103bcb32/P1M1T1S1/PRP.md
  why: "Defines the exact unified structs the docs describe: RuleSet.rules:
        Vec<Rule> (rename=\"rule\" SINGULAR); Rule.layer: Option<u8> (OPTIONAL, not
        required u8); enable/disable/case_sensitive/disable_firmware_config fields.
        Every doc edit must agree with THIS surface."
  section: "What (a) structs"
  critical: "layer is Option<u8> -> docs must say 'optional'. The split arrays are
             GONE -> docs must show ONE [[rule]] array."

# MUST READ — the S3 contract (the code/template counterpart; confirms the boundary)
- file: /home/dustin/projects/qmkonnect/plan/004_f48a103bcb32/P1M1T1S3/PRP.md
  why: "S3 owns src/ callers + the render_rules_body template. It explicitly defers
        docs/*.md to 'P1.M1.T2' (this task). Confirms NO overlap: this task = docs
        only; S3 = src only. S3's verbatim render_rules_body template (its
        Implementation Patterns) is the SAME 4-[[rule]] shape docs must show — use
        it as a cross-check that prose matches the seeded template."
  section: "Scope" + "Integration Points (DOWNSTREAM: P1.M1.T2)"
  critical: "Do NOT edit src/ or llms_full.txt. The render_rules_body template S3
             writes is what qmkonnect -c seeds; the docs/*.md are the hand-written
             published versions — both must show the same [[rule]] shape."

# MUST READ — the 4 files being edited (read current text before editing)
- file: /home/dustin/projects/qmkonnect/docs/configuration.md
  why: "Sites: ~265 intro ('two table-arrays'), 271-281 schema table (11 split
        rows), 309-332 annotated TOML example (2 dividers + 4 split headers), 398
        stack-vs-replace bullet. Read 260-340 + 390-400."
  gotcha: "Preserve the configuration.md-specific intro comment ('On no host
           match: the host layer is cleared…') and the [host] block — only the
           Layer-rules/Callback-rules dividers + headers change."

- file: /home/dustin/projects/qmkonnect/docs/examples.md
  why: "Site: 294-318 Example-4 TOML block (3 [[layer_rules]] + 2 [[callback_rules]]).
        Read 285-320."
  gotcha: "Merge the two steam_app* rules (a layer rule + a callback rule for the
           same match) into ONE [[rule]] to demonstrate layer+enable in one rule.
           Keep the surrounding prose (255 sentinel, layer_state width) unchanged."

- file: /home/dustin/projects/qmkonnect/docs/qmk-integration.md
  why: "Sites: 211-217 migration steps 2 & 3, 230-232 migration example. Read 205-235."
  gotcha: "Mirror spec §10 verbatim for the step wording ('add a [[rule]] entry
           with a layer field' / 'with enable/disable')."

- file: /home/dustin/projects/qmkonnect/docs/troubleshooting.md
  why: "Site: 519-522 'Fix' sentence ('every [[layer_rules]] entry requires match
        and layer'). Read 515-525."
  gotcha: "layer becomes OPTIONAL; the error is now 'sets none of layer/enable/
           disable'. Keep the trailing 'See the Configuration Guide' sentence."

# REFERENCE — research notes for this subtask (per-site plan + token scan)
- docfile: /home/dustin/projects/qmkonnect/plan/004_f48a103bcb32/P1M1T2S1/research/notes.md
  why: "The ground-truth grep scan (25 TOML token hits + 1 prose hit, with line
        numbers — configuration.md 16, examples.md 5, qmk-integration.md 3,
        troubleshooting.md 1), the 5 semantic deltas, and the verbatim
        before→after for every site."
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/
├── docs/
│   ├── configuration.md     # <-- EDIT (4 sites)
│   ├── examples.md          # <-- EDIT (1 site)
│   ├── qmk-integration.md   # <-- EDIT (2 sites)
│   ├── troubleshooting.md   # <-- EDIT (1 site)
│   ├── llms_full.txt        # DO NOT EDIT (regenerated in S2 by generate_llms_full.sh)
│   └── (other docs untouched)
├── spec/HOST_RULES.md       # READ-ONLY source of truth (§9/§10)
└── src/                     # DO NOT EDIT (S3's scope)
```

### Desired Codebase tree with files to be modified

```bash
docs/
├── configuration.md     # MODIFIED — intro, schema table, TOML example, stack-vs-replace bullet
├── examples.md          # MODIFIED — Example-4 TOML block
├── qmk-integration.md   # MODIFIED — migration steps 2&3, migration example
└── troubleshooting.md   # MODIFIED — "Fix" sentence
# (no new files; no other file touched)
```

### Known Gotchas of our codebase & Library Quirks

```markdown
<!-- CRITICAL: [[rule]] is SINGULAR (serde rename = "rule"), NOT [[rules]].
     A plural [[rules]] key silently parses to an EMPTY rules vec (unknown field
     dropped under #[serde(default)]) — the user's rules would do nothing and
     --validate-rules would pass for the wrong reason. Every TOML literal must be
     [[rule]]. -->

<!-- CRITICAL: layer is OPTIONAL (Option<u8>), not required.
     The old [[layer_rules]] REQUIRED layer; the unified [[rule]] does not. The
     schema table Required column for `layer` flips yes -> no (default `None`),
     and the troubleshooting "Fix" must say "≥1 of layer/enable/disable", NOT
     "requires match and layer". -->

<!-- CRITICAL: layer is a RAW QMK layer index (C11). No 224/225 floor.
     The withdrawn "_GAMING = 1 -> layer = 224" guidance is GONE (examples.md
     already says so). Keep only: != 255 (clear sentinel) and <
     layer_state_t width (≤15 default, ≤31 with LAYER_STATE_32BIT). -->

<!-- CRITICAL: a rule MUST set ≥1 of layer/enable/disable (else parse error).
     This replaces the old "layer_rules requires match+layer". The validity note
     goes in the schema-table [[rule]] row + the unified TOML divider + the
     troubleshooting Fix. -->

<!-- GOTCHA: preserve configuration.md-specific intro prose.
     The "On no host match: the host layer is cleared + host callbacks disabled,
     but the BOARD still runs" comment and the [host] block are configuration.md
     value-adds (spec §9 has a shorter version). Keep them; only the two
     Layer-rules/Callback-rules dividers + the 4 headers change. -->

<!-- GOTCHA: do NOT hand-edit llms_full.txt.
     It's a concatenation of all docs, regenerated by docs/generate_llms_full.sh
     in S2. Editing it now would (a) be overwritten and (b) risk merge entropy.
     The grep gate EXCLUDES it. -->

<!-- GOTCHA: whitespace in edit-tool oldText MUST match the file exactly.
     The TOML code fences use trailing alignment spaces (e.g. after `match =
     "alacritty"`). Copy the exact bytes; a mismatched oldText fails to apply. -->

<!-- NOTE: prose "layer rule"/"callback rule" as CONCEPTS may stay.
     The unified schema still has the CONCEPT of a rule-that-sets-a-layer vs a
     rule-that-sets-callbacks (the code's empty_pattern_warnings keeps
     "layer rule #N"/"callback rule #N" text per S3). Only the TOML KEYS and the
     structural "two table-arrays"/"requires layer" CLAIMS must change. -->
```

## Implementation Blueprint

### Data models and structure

No data models — this is a docs task. The "structure" is the 4 files' edit sites,
each a before→after text replacement derived from `spec/HOST_RULES.md` §9/§10.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: READ the source of truth + the 4 files (anchor every edit)
  - READ: spec/HOST_RULES.md §9 (437-528) + §10 (530-542) — copy FROM these.
  - READ: the 4 doc files at the line ranges in Documentation & References.
  - RUN: grep -rnE '\[\[(layer|callback)_rules\]\]' docs/ | grep -v llms_full.txt
          to confirm the exact current hit list (25 hits: configuration.md 16,
          examples.md 5, qmk-integration.md 3, troubleshooting.md 1).
  - GOAL: know every site's exact current text + the verbatim replacement.

Task 2: EDIT docs/configuration.md (4 sites — A, B, C, D below)
  - Site A (line ~265 intro): "two table-arrays" -> "one table-array".
  - Site B (lines 271-281 schema table): replace the 11 split rows with the
          7 unified [[rule]] rows (see Implementation Patterns → cfg-Site-B).
  - Site C (lines 309-332 annotated TOML): replace the 2 dividers + 4 split
          headers with 1 unified divider + 4 [[rule]] entries (see cfg-Site-C).
  - Site D (line 398 bullet): "Each [[layer_rules]] / [[callback_rules]]" ->
          "Each [[rule]]".

Task 3: EDIT docs/examples.md (1 site — ex-Site below)
  - Rewrite the Example-4 TOML block (lines 294-318): merge the two steam_app*
          rules into one [[rule]]; 4 [[rule]] entries total; unify dividers
          (see Implementation Patterns → ex-Site).

Task 4: EDIT docs/qmk-integration.md (2 sites)
  - Site A (lines 211-217 migration steps 2 & 3): mirror spec §10 — step 2
          "add a [[rule]] entry with a layer field", step 3 "add a [[rule]] entry
          with enable/disable" (see Implementation Patterns → qmk-Site-A).
  - Site B (lines 230-232 example): [[callback_rules]] -> [[rule]] (header swap).

Task 5: EDIT docs/troubleshooting.md (1 site — ts-Site below)
  - Lines 519-522 "Fix": "requires match and layer" -> "requires match and at
          least one of layer/enable/disable"; layer -> optional (see
          Implementation Patterns → ts-Site).

Task 6: VALIDATE (the grep gate — do not skip)
  - RUN: grep -rnE '\[\[(layer|callback)_rules\]\]' docs/ | grep -v llms_full.txt
          -> ZERO output (exit 1).
  - RUN: grep -rnE '\[\[rules\]\]' docs/ -> ZERO output (no plural mistake).
  - OPTIONAL: pipe each new TOML fence through tomllib to confirm well-formedness.
  - VISUAL: open each edited file, confirm the tables render + TOML fences intact.
```

### Implementation Patterns & Key Details

The blocks below are **verbatim before → after**. Apply each with the `edit` tool;
the `oldText` whitespace MUST match the file exactly (copy from the file, not from
memory). Line numbers are anchors, not strict (re-verify with grep before editing).

---

#### cfg-Site-A — `docs/configuration.md` intro (line ~265)

**OLD:**
```
`rules.toml` has one optional table and two table-arrays:
```
**NEW:**
```
`rules.toml` has one optional table and one table-array:
```

---

#### cfg-Site-B — `docs/configuration.md` schema table (lines 271–281)

Replace the entire 11-row split block (from the `[[layer_rules]]` table-array row
through the `[[callback_rules]] disable_firmware_config` row) with these 7 unified
rows. The `layer` row reuses the existing C11 wording verbatim (raw index,
`layer_state_t` width, `!=255`), flipping Required `yes`→`no` and noting it's
optional:

**NEW (the replacement for the 11 split rows):**
```
| `[[rule]]` table-array | no | `[]` | Host rules — one entry per (app × behavior). For each matching rule, `layer` is **first-match-wins** (one host layer active at a time); `enable`/`disable` accumulate across **all** matches (all-match). A rule MUST set at least one of `layer` / `enable` / `disable` (one that sets none is a parse error); it may set layer only, callbacks only, or both. Names come from your keyboard's callback registry (run `qmkonnect --list-callbacks` to see them). |
| `[[rule]] match` | **yes** | — | Window pattern. A bare string (`"alacritty"`) matches the **window class only**; a two-element array (`["*chrome*", "*youtube*"]`) matches **class and title** (equivalent to the firmware `WT(class, title)`). Supports `*`, `^`, `$`, `+`, character classes (`\d \w \s …`), and `.` — full parity with the firmware matcher. |
| `[[rule]] layer` | no | `None` | Optional — the host layer number to activate. A **raw QMK layer index**, not a reserved range. When set it must be `<` your firmware's `layer_state_t` width (≤15 by default, ≤31 with `LAYER_STATE_32BIT`; larger indices are undefined behavior in `layer_on`), and `!= 255` (`0xFF` is the wire "clear layer" sentinel — writing it would silently *clear* the host layer, so `parse_rules`/`--validate-rules` reject it). To make the host layer win in **stack** mode, pick an index above your highest board layer; in **replace** mode any valid index wins. A rule may set `enable`/`disable` only and leave this unset. |
| `[[rule]] enable` | no | `[]` | Callback names to enable on focus-in. |
| `[[rule]] disable` | no | `[]` | Callback names to force off (**explicit exclusion**, order-independent: a `disable` in any matching rule always wins over an `enable` in any other matching rule). Focus-out `on_disable` also fires automatically when a callback leaves the active set. |
| `[[rule]] case_sensitive` | no | `false` | Whether `match` is case-sensitive. |
| `[[rule]] disable_firmware_config` | no | inherits `[host]` | Per-rule stack/replace override. Absent ⇒ uses the `[host]` default. |
```

> The `[host]` rows above this block (the `[host]` table + `disable_firmware_config`
> row) are UNCHANGED — only the 11 split rows become these 7.

---

#### cfg-Site-C — `docs/configuration.md` annotated TOML example (lines 309–332)

Keep the file-header comment + `[host]` block + the "On no host match" comment
UNTOUCHED. Replace ONLY the two dividers + four split headers (from
`# Layer rules: FIRST match wins.` through the final
`disable_firmware_config = true  # for this window...` line) with this unified
divider + four `[[rule]]` entries:

**OLD (the block being replaced — from `# Layer rules:` to the last rule):**
```toml
# Layer rules: FIRST match wins. One host layer active at a time.
# `layer` is a raw QMK layer index (no reserved range): pick one defined in your
# keymap, < your layer_state width (<=15 default, <=31 with LAYER_STATE_32BIT),
# above your highest board layer (so it wins in stack mode), and != 255.
[[layer_rules]]
match = "alacritty"                       # class-only pattern
layer = 10
disable_firmware_config = true           # optional override (default inherits [host])

[[layer_rules]]
match = ["*chrome*", "*youtube*"]         # [class_pattern, title_pattern] (== WT())
layer = 11
case_sensitive = false                    # optional, default false

# Callback rules: ALL matches fire. Names come from the keyboard's registry
# (run `qmkonnect --list-callbacks` to see them). The disable list is an
# explicit-exclusion override; focus-out on_disable fires automatically via the
# desired-set diff.
[[callback_rules]]
match = "neovide"
enable = ["vim_lazy", "disable_vim"]      # run on focus-in
disable = ["vim_lazy"]                    # optional: force-off override

[[callback_rules]]
match = ["*chrome*", "*claude*"]
enable = ["vim_lazy", "disable_vim"]
disable_firmware_config = true           # for this window, skip the string -> board can't match
```

**NEW (the unified replacement):**
```toml
# Rules: one [[rule]] per (app × behavior). For each matching rule, `layer` is
# first-match-wins (one host layer active — exclusive); `enable`/`disable`
# accumulate across ALL matches (all-match). A rule MUST set at least one of
# `layer` / `enable` / `disable` — one that sets none is a parse error (it may set
# layer only, callbacks only, or both). `layer` is a RAW QMK layer index (no fixed
# floor): pick one defined in your keymap, < your layer_state width (<=15 default,
# <=31 with LAYER_STATE_32BIT), above your highest board layer (so it wins in
# stack mode), and != 255 (the "clear" sentinel).
[[rule]]
match = "alacritty"                       # class-only pattern
layer = 10
disable_firmware_config = true           # optional override (default inherits [host])

[[rule]]
match = ["*chrome*", "*youtube*"]         # [class_pattern, title_pattern] (== WT())
layer = 11
case_sensitive = false                    # optional, default false

[[rule]]
match = "neovide"
enable = ["vim_lazy", "disable_vim"]      # run on focus-in
disable = ["vim_lazy"]                    # optional: force-off override

[[rule]]
match = ["*chrome*", "*claude*"]
enable = ["vim_lazy", "disable_vim"]
disable_firmware_config = true           # for this window, skip the string -> board can't match
```

---

#### cfg-Site-D — `docs/configuration.md` stack-vs-replace bullet (line ~398)

**OLD:**
```
- Each `[[layer_rules]]` / `[[callback_rules]]` may set an optional
  `disable_firmware_config` to override it. **Absent ⇒ inherits the `[host]`
  default.**
```
**NEW:**
```
- Each `[[rule]]` may set an optional `disable_firmware_config` to override it.
  **Absent ⇒ inherits the `[host]` default.**
```

---

#### ex-Site — `docs/examples.md` Example-4 TOML (lines ~295–318)

The two `steam_app*` rules (a layer rule + a callback rule for the same `match`)
are merged into ONE `[[rule]]` (layer + enable together) — a faithful equivalence
and a demonstration of the unified schema ("it may set layer only, callbacks only,
or both"). The other three stay 1:1. The two dividers unify into one.

**OLD (the rule section of the block — from `# Layer rules` to the last rule):**
```toml
# Layer rules — FIRST match wins. `layer` = a real QMK layer index from your keymap.
[[layer_rules]]
match = "steam_app*"                       # class-only pattern (board rules also run)
layer = 10

[[layer_rules]]
match = ["cs2", "Counter-Strike 2"]        # [class, title]  == WT(class, title)
layer = 10

# Replace: for this window the host takes over and the board is skipped.
[[layer_rules]]
match = ["*chrome*", "*youtube*"]
layer = 11
disable_firmware_config = true

# Callback rules — ALL matches fire. Names come from DEFINE_HOST_CALLBACKS.
[[callback_rules]]
match = "steam_app*"
enable = ["enable_gaming"]

[[callback_rules]]
match = "*word*"
enable = ["disable_gaming"]
```

**NEW (4 unified [[rule]] entries; the steam_app* layer+callback merge):**
```toml
# Rules — `layer` is first-match-wins (one host layer active); `enable`/`disable`
# accumulate across ALL matches. One [[rule]] can set layer AND callbacks for the
# same app at once. `layer` = a real QMK layer index from your keymap.
[[rule]]
match = "steam_app*"                       # class-only pattern (board rules also run)
layer = 10
enable = ["enable_gaming"]                 # layer + callbacks for one app, in one rule

[[rule]]
match = ["cs2", "Counter-Strike 2"]        # [class, title]  == WT(class, title)
layer = 10

# Replace: for this window the host takes over and the board is skipped.
[[rule]]
match = ["*chrome*", "*youtube*"]
layer = 11
disable_firmware_config = true

[[rule]]
match = "*word*"
enable = ["disable_gaming"]
```

> The `[host]` block above and the "remove from DEFINE_*" prose below are
> UNCHANGED. (The intro prose's "255 clear sentinel" / "layer_state ≤15/≤31"
> wording stays — it's still accurate for the raw-index `layer`.)

---

#### qmk-Site-A — `docs/qmk-integration.md` migration steps 2 & 3 (lines ~211–217)

Mirror `spec/HOST_RULES.md` §10 verbatim. **OLD:**
```
2. **Move a layer rule to the host** — add a `[[layer_rules]]` entry to
   `rules.toml`, then **remove** the matching row from `DEFINE_SERIAL_LAYERS`.
   (Keeping it in both isn't harmful, but it means the same layer is driven by
   two trackers at once, which is confusing.) No reflash needed for this or any
   later edit.
3. **Move a callback rule to the host** — add a `[[callback_rules]]` entry,
   then **remove** the matching row from `DEFINE_SERIAL_COMMANDS`. Here removal
   matters: callbacks are additive, so if a rule stays in both, the same
   `on_enable` would fire twice.
```
**NEW:**
```
2. **Move a layer rule to the host** — add a `[[rule]]` entry with a `layer`
   field to `rules.toml`, then **remove** the matching row from
   `DEFINE_SERIAL_LAYERS`. (Keeping it in both isn't harmful, but it means the
   same layer is driven by two trackers at once, which is confusing.) No reflash
   needed for this or any later edit.
3. **Move a callback rule to the host** — add a `[[rule]]` entry with
   `enable`/`disable`, then **remove** the matching row from
   `DEFINE_SERIAL_COMMANDS`. Here removal matters: callbacks are additive, so if
   a rule stays in both, the same `on_enable` would fire twice.
```

---

#### qmk-Site-B — `docs/qmk-integration.md` migration example (lines ~230–232)

Header swap only; `match`/`enable` rows unchanged. **OLD:**
```toml
[[callback_rules]]
match = ["steam_app*", "*"]        # [class, title]  == WT(class, title)
enable = ["disable_vim"]
```
**NEW:**
```toml
[[rule]]
match = ["steam_app*", "*"]        # [class, title]  == WT(class, title)
enable = ["disable_vim"]
```

---

#### ts-Site — `docs/troubleshooting.md` "Fix" sentence (lines ~519–522)

`layer` becomes OPTIONAL; the validity rule becomes "≥1 of layer/enable/disable".
Keep the trailing "See the Configuration Guide" sentence UNCHANGED.

**OLD:**
```
**Fix**: every `[[layer_rules]]` entry **requires** `match` and `layer` (an entry
missing either is an error); `match` is either a bare string (`"steam_app*"`,
class-only) or a **2-element** array (`["*chrome*", "*youtube*"]` — class and
title; 1- or 3-element arrays are errors); `layer` is a **raw QMK layer index**
(no reserved range) — it must be `<` your `layer_state` width (≤31 with
`LAYER_STATE_32BIT`) and `!= 255` (the wire "clear layer" sentinel, which would
silently *clear* the host layer and is rejected). To win in **stack** mode it
must be above your highest board layer; in **replace** mode any valid index
wins. See the
```

**NEW:**
```
**Fix**: every `[[rule]]` entry **requires** `match` and at least one of `layer`
/ `enable` / `disable` (an entry setting none of those is an error); `match` is
either a bare string (`"steam_app*"`, class-only) or a **2-element** array
(`["*chrome*", "*youtube*"]` — class and title; 1- or 3-element arrays are
errors); `layer` is **optional** — a **raw QMK layer index** (no reserved range)
when set, and must then be `<` your `layer_state` width (≤31 with
`LAYER_STATE_32BIT`) and `!= 255` (the wire "clear layer" sentinel, which would
silently *clear* the host layer and is rejected). To win in **stack** mode it
must be above your highest board layer; in **replace** mode any valid index
wins. See the
```

> The oldText ends at "wins. See the" so the trailing
> "[Configuration Guide]({{ site.baseurl }}/configuration) for the full field table."
> sentence is preserved automatically.

### Integration Points

```yaml
SOURCE FILES:
  - modify: "docs/configuration.md, docs/examples.md, docs/qmk-integration.md,
             docs/troubleshooting.md"
  - do NOT modify: "src/ (S3), docs/llms_full.txt (S2 regenerates),
                    spec/HOST_RULES.md (read-only source of truth), any other doc"

PUBLIC API SURFACE:
  - none. Pure docs.

UPSTREAM CONTRACT (S1 — LANDED; the model the docs describe):
  - "RuleSet.rules: Vec<Rule> (rename=\"rule\" SINGULAR); Rule.layer: Option<u8>
     (OPTIONAL); enable/disable/case_sensitive/disable_firmware_config fields."

PARALLEL SIBLING (S3 — being implemented):
  - S3 owns src/ callers + render_rules_body template. This task (docs/*.md) has
    NO file overlap with S3. Cross-check: the docs' [[rule]] shape should match
    S3's render_rules_body template (both derive from spec §9).

DOWNSTREAM (do NOT implement — listed for awareness):
  - P1.M1.T2.S2: "regenerate docs/llms_full.txt via generate_llms_full.sh (the
    concatenation picks up these doc edits automatically)."
  - P1.M1.T3.S1: "audit README.md + top-level docs for the unified [[rule]] schema
    (separate final sweep — NOT this task)."

VALIDATION CONSUMERS:
  - The grep gate (0 [[layer_rules]]/[[callback_rules]] in docs/ excl. llms_full.txt)
    is the completeness proof for THIS task.
```

## Validation Loop

> All commands run from the repo root: `/home/dustin/projects/qmkonnect`.
> This is a docs task — **no compilation, no tests**. The gate is the grep.

### Level 1: Token hygiene (the primary gate)

```bash
cd /home/dustin/projects/qmkonnect

# PRIMARY GATE — zero stale split-schema TOML literals (excluding llms_full.txt,
# which S2 regenerates from these files).
grep -rnE '\[\[(layer|callback)_rules\]\]' docs/ | grep -v llms_full.txt
# Expected: NO output (grep exits 1). Any hit is a missed site — fix it.

# No PLURAL mistake anywhere in docs/ ([[rules]] would silently parse to empty).
grep -rnE '\[\[rules\]\]' docs/
# Expected: NO output. Every literal must be [[rule]] (singular).

# The structural prose claim is fixed (configuration.md intro):
grep -n 'two table-arrays' docs/configuration.md
# Expected: NO output (it now says "one table-array").
grep -n 'one table-array' docs/configuration.md
# Expected: 1 hit (the fixed intro line).
```

### Level 2: Schema-shape spot checks

```bash
cd /home/dustin/projects/qmkonnect

# configuration.md schema table is now a single [[rule]] row-set (7 rows, not 11).
grep -cE '^\| `\[\[rule\]\]' docs/configuration.md
# Expected: 7  (match / layer / enable / disable / case_sensitive / disable_firmware_config
#            + the [[rule]] table-array row). Was 11 split rows before.

# The unified [[rule]] divider exists in the annotated example.
grep -c '\[\[rule\]\]' docs/configuration.md
# Expected: >= 5  (4 entries + at least 1 in the divider prose / table).

# examples.md demonstrates layer+enable in ONE rule (the steam_app* merge).
grep -A4 'match = "steam_app\*"' docs/examples.md | grep -E 'layer = 10|enable = \["enable_gaming"\]'
# Expected: both lines present (layer + enable in the same [[rule]]).

# qmk-integration.md migration steps mirror spec §10 wording.
grep -n 'add a .\[\[rule\]\]. entry with a .layer. field' docs/qmk-integration.md
grep -n 'add a .\[\[rule\]\]. entry with' docs/qmk-integration.md
# Expected: 1 hit each (steps 2 & 3).

# troubleshooting.md "Fix" reflects the new validity rule + optional layer.
grep -n 'at least one of .layer.' docs/troubleshooting.md   # the ≥1-of validity rule
grep -n 'layer. is ..optional' docs/troubleshooting.md       # layer is optional
# Expected: 1 hit each.
```

### Level 3: TOML well-formedness (cheap insurance — optional but recommended)

```bash
cd /home/dustin/projects/qmkonnect

# Extract each new TOML code-fence and confirm it parses (array-of-tables is valid
# TOML). python3.11+ has tomllib stdlib; else `pip install tomli` or skip.
python3 - <<'EOF'
import re, pathlib, sys
try:
    import tomllib
except ModuleNotFoundError:
    print("tomllib unavailable (python<3.11) — skipping TOML parse check")
    sys.exit(0)
ok = True
for f in ["docs/configuration.md", "docs/examples.md", "docs/qmk-integration.md"]:
    txt = pathlib.Path(f).read_text()
    # grab every ```toml ... ``` fence
    for i, m in enumerate(re.finditer(r"```toml\n(.*?)```", txt, re.S), 1):
        body = m.group(1)
        # strip the all-commented seed template in configuration.md (it's inert;
        # tomllib parses it fine, but it's not meant to be "active" TOML)
        try:
            tomllib.loads(body)
        except Exception as e:
            ok = False
            print(f"{f} fence #{i}: INVALID TOML -> {e}")
print("TOML fences parse OK" if ok else "TOML parse errors (see above)")
EOF
# Expected: "TOML fences parse OK" (or the tomllib-unavailable skip message).
# This catches a malformed fence (e.g. a stray backtick, bad array) that grep can't.
```

### Level 4: Visual / rendering check (manual)

```text
Open each of the 4 edited files in a markdown renderer (or GitHub preview) and
confirm:
- docs/configuration.md: the schema table renders as a clean 7-row [[rule]] table;
  the annotated TOML example shows 4 [[rule]] blocks under one divider; the
  stack-vs-replace bullet says "Each [[rule]]".
- docs/examples.md: the Example-4 block shows 4 [[rule]] entries, with the
  steam_app* rule carrying BOTH layer=10 and enable=["enable_gaming"].
- docs/qmk-integration.md: migration steps 2/3 say "[[rule]] entry with a layer
  field" / "with enable/disable"; the example header is [[rule]].
- docs/troubleshooting.md: the "Fix" says "at least one of layer/enable/disable"
  and "layer is optional".
No broken tables, no stray backticks, no double-blank-line gaps inside fences.
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1: `grep -rnE '\[\[(layer|callback)_rules\]\]' docs/ | grep -v llms_full.txt` → 0 hits.
- [ ] Level 1: `grep -rnE '\[\[rules\]\]' docs/` → 0 hits (singular everywhere).
- [ ] Level 1: `docs/configuration.md` says "one table-array" (not "two").
- [ ] Level 2: configuration.md schema table has 7 `[[rule]]` rows (was 11 split).
- [ ] Level 2: examples.md steam_app* rule has both `layer = 10` and `enable`.
- [ ] Level 2: qmk-integration.md steps 2/3 + example use `[[rule]]`.
- [ ] Level 2: troubleshooting.md "Fix" says "≥1 of layer/enable/disable" + "optional".
- [ ] Level 3 (optional): all new TOML fences parse.
- [ ] Level 4: visual render is clean (no broken tables/fences).

### Feature Validation

- [ ] All 5 semantic deltas captured: (a) one `[[rule]]` array; (b) `layer`
      optional; (c) ≥1-of-(layer/enable/disable) validity; (d) `layer` raw index,
      no 224/225 floor; (e) `[[rule]]` SINGULAR.
- [ ] `layer` documented as a raw QMK index (`!=255`, `< layer_state_t`) everywhere
      it appears — no `224`/`225` floor guidance reintroduced.
- [ ] configuration.md preserves its "On no host match" comment + `[host]` block.
- [ ] examples.md preserves its "255 sentinel" / "layer_state width" intro prose.
- [ ] troubleshooting.md preserves its "See the Configuration Guide" trailing sentence.

### Code Quality Validation

- [ ] `[[rule]]` is SINGULAR everywhere (no `[[rules]]`).
- [ ] Edits follow the existing doc style (markdown tables, `{{ site.baseurl }}`
      links, TOML fence alignment).
- [ ] Only the 4 listed files modified; `src/`, `llms_full.txt`, `spec/` untouched.

### Documentation & Deployment

- [ ] The 4 published docs now agree with `spec/HOST_RULES.md` §9 + the S3-seeded
      `render_rules_body` template (all four surfaces: code, template, spec, docs).
- [ ] `llms_full.txt` left for S2 to regenerate (it concatenates these files).
- [ ] No environment variables, config, or Cargo.toml changes.

---

## Anti-Patterns to Avoid

- ❌ Don't use `[[rules]]` (PLURAL) — it's SINGULAR `[[rule]]` (serde
  `rename = "rule"`). A plural key silently parses to an empty rules vec; the
  user's rules would do nothing. Every TOML literal must be `[[rule]]`.
- ❌ Don't document `layer` as REQUIRED — it's `Option<u8>` (OPTIONAL). The schema
  table Required column for `layer` is `no`; the troubleshooting "Fix" must say
  "≥1 of layer/enable/disable", not "requires match and layer".
- ❌ Don't reintroduce a `224`/`225` layer floor — that guidance is WITHDRAWN
  (examples.md already says "`layer_state` cannot hold bit 224"). `layer` is a raw
  QMK index: only `!= 255` and `< layer_state_t` width.
- ❌ Don't hand-edit `docs/llms_full.txt` — it's regenerated by
  `generate_llms_full.sh` in S2 from these doc files. Editing it now is wasted and
  risks merge entropy. The grep gate EXCLUDES it.
- ❌ Don't edit `src/` or `spec/` — S3 owns `src/`; `spec/HOST_RULES.md` is the
  read-only source of truth (copy FROM it). This task is `docs/*.md` only.
- ❌ Don't lose the configuration.md-specific intro comment ("On no host match…")
  when rewriting the annotated TOML example — only the Layer/Callback dividers +
  headers change; the file-header comment + `[host]` block + no-match comment stay.
- ❌ Don't change the `match`/`enable`/`disable` VALUES in the examples — only the
  table-array HEADER (`[[layer_rules]]`/`[[callback_rules]]` → `[[rule]]`) and the
  merged divider change (plus the examples.md steam_app* merge, which is a true
  equivalence). The rule contents stay byte-identical.
- ❌ Don't collapse the examples.md steam_app* merge into something semantically
  different — layer=10 + enable=["enable_gaming"] in ONE `[[rule]]` is IDENTICAL
  to the old split (first-match-wins layer + all-match callbacks). It's a faithful
  rewrite, not a behavior change.
- ❌ Don't break the markdown table alignment — copy the exact pipe/spacing of the
  existing rows when building the 7-row `[[rule]]` table (the `edit` oldText must
  match the file's bytes exactly, including the column separators).
- ❌ Don't leave a stale "two table-arrays" claim — the configuration.md intro must
  say "one table-array" (the structural claim is part of the gate).
- ❌ Don't edit README.md or other top-level docs — that's P1.M1.T3.S1 (a separate
  final sweep). This task is exactly the 4 files listed.

---

**Confidence Score: 10/10** for one-pass implementation success. The deliverable
is a deterministic, mechanical rewrite of 8 enumerated doc sites across 4 files,
each with its exact current text quoted and a verbatim replacement derived from
the already-correct `spec/HOST_RULES.md` §9/§10. The source of truth is singular
and quoted; the 5 semantic deltas are explicit; the validation is a single grep
(zero stale tokens, zero plural) plus optional TOML-parse insurance; and the scope
is cleanly separated from the parallel S3 (`src/`) and downstream S2
(`llms_full.txt`) tasks. The one residual risk — `edit`-tool oldText whitespace
mismatch on the TOML fences — is mitigated by instructing the implementer to copy
exact bytes from the file (re-verified by grep before editing). No code, no build,
no tests: the gate is purely textual and self-verifying.