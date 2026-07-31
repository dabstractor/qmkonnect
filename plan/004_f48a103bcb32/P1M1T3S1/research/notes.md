# Research Notes — P1.M1.T3.S1: Audit README.md + top-level docs for unified `[[rule]]`

## TL;DR — this is a VERIFICATION task, almost certainly a NO-OP

The delta PRD §4 (Documentation Impact Summary) concluded **Mode B = "none"**:
README.md mentions `rules.toml` only as a version-agnostic one-line feature blurb,
and no top-level/architecture doc describes the split schema. This task's job (per
the SOW) is to **VERIFY that claim against the actual repo state** rather than
assume it — and if anything is stale, fix it. The verification (below) confirms
**everything is already clean**: zero split-schema tokens anywhere in the docs,
`[[rule]]` present only where it should be. So the deliverable is a **verification
report** (grep evidence); no doc edits are expected.

## Verification evidence (run against the live repo, 2026-07-31)

### (1) Split-schema tokens — ZERO hits across ALL docs/spec/README
```bash
grep -rnE 'layer_rules|callback_rules|LayerRule|CallbackRule' \
  README.md AGENTS.md REMAINING_ISSUES.md docs/*.md spec/*.md
# → NO output (exit 1). CLEAN.
```
Repo-wide `*.md` (excluding `target/ plan/ .pi-subagents/ docs/vendor/`) → also
zero. The unified `[[rule]]` schema is propagated everywhere.

### (2) WHERE does `[[rule]]` (the unified schema) appear?
```bash
grep -rlnE '\[\[rule[s]?\]\]' --include='*.md' . | grep -vE '/(target|plan|\.pi-subagents|docs/vendor)/'
```
→ EXACTLY 5 files, the expected set:
- `spec/HOST_RULES.md` — the spec source of truth (§9 schema; already correct).
- `docs/configuration.md`, `docs/examples.md`, `docs/qmk-integration.md`,
  `docs/troubleshooting.md` — the 4 docs S1 (P1.M1.T2.S1) unified.

No README/index/usage/spec-inline schema. The schema is described ONLY where it
should be; everything else points to it.

### (3) README.md's `rules.toml` mention (lines 34–41) — version-agnostic blurb, LEAVE IT
```
- **Change layers & callbacks without reflashing** — edit a `rules.toml` file
  on your computer (the **Edit rules** tray item opens it; changes hot-reload
  on the next window change); no firmware rebuild needed
- Host rules **stack on top of** your board's existing `DEFINE_SERIAL_*` rules …
- Requires firmware that advertises the typed-command capability (`proto_ver == 2`) …
- Full schema, CLI flags (`--list-callbacks`, `--validate-rules`), and per-OS
  file location: see the [Configuration Guide](docs/configuration.md) …
```
This is a **feature blurb** — it does NOT show `[[rule]]` OR `[[layer_rules]]`,
names no fields, and **points to `docs/configuration.md` for the full schema**.
It is version-agnostic and already accurate. Per the item: "if it's the
version-agnostic one-liner, leave it." → **NO EDIT.** ✓

### (4) Spec sections the item calls out — all BEHAVIOR/POINTERS, no schema
- **spec/ARCHITECTURE.md §5.7** (Host-side-rules extension, lines 268–285):
  "the full design is in `HOST_RULES.md` … evaluates `rules.toml` … sends
  `APPLY_HOST_CONTEXT` … see `HOST_RULES.md` §4". Pure behavior + pointer. ✓
- **spec/PROTOCOL.md §8** (Typed-Command Namespace, line 281; rules mentions at
  286/326/332): "(handshake, per-window send logic, `rules.toml`) is in
  `HOST_RULES.md`" + `disable_firmware_config` mention + handshake. Pointers. ✓
- **spec/UI.md §1.1** (menu layout, line 31): "Edit rules… ← seed rules.toml if
  absent, then open in system editor". Tray-menu behavior. ✓
- **spec/UI.md §1.2** (Linux SNI menu, line 54): "Edit rules ← seed rules.toml if
  absent, then xdg-open". Behavior. ✓
- **spec/UI.md §2.3** (line 137): "fires an automatic 'rules.toml invalid'
  notification when `rules.toml` fails to parse". Behavior. ✓
- **spec/CONFIG.md** (lines 111–115, 138–139): path info + "Schema:
  `HOST_RULES.md` §9" + `--validate-rules`/`--rules-path` flags. Pointers. ✓
- **spec/PRD.md** (lines 7–8, 132, 296–297, 330–355): feature blurb (F11/F12) +
  glossary entries, all pointing to `HOST_RULES.md`. No schema. ✓

None of these describe the split (or any) schema inline. They all defer to
`HOST_RULES.md` §9. So even before unification they would have been correct
(they never showed `[[layer_rules]]`); after unification they're trivially still
correct. **NO EDIT needed.**

### (5) BONUS — withdrawn `≥ 224` floor guidance in top-level docs
```bash
grep -rnE '≥ 224|>= 224|must be.*224' README.md AGENTS.md REMAINING_ISSUES.md \
  docs/index.md docs/usage.md docs/installation.md docs/README.md spec/*.md
```
→ ONE hit: `spec/HOST_RULES.md:134` — and it's the **intentional withdrawal note**
("*(The earlier '≥ 224' reservation is withdrawn: …)*"). That is CORRECT
documentation (it explains the floor was withdrawn), NOT stale guidance. **NO
EDIT.** (Same finding as T2.S2's examples.md withdrawal note.)

## Out-of-scope observation (do NOT fix here — S3's domain, Complete)
`src/core/rules.rs:347` has a comment referencing the old `LayerRule`/`CallbackRule`
types ("Consumes `RuleSet`/`HostDefaults`/`LayerRule`/`CallbackRule` (S1)"). This is
**src/, not a doc** — it's P1.M1.T1.S3's responsibility (marked Complete). This
task's scope is README.md + top-level DOCS only. Flag it as an observation if
desired, but do NOT edit `src/` from this task (scope violation + S3 owns it).

## Coordination with the parallel T2.S2 (llms_full regeneration)
T2.S2 regenerates `docs/llms_full.txt`, which **concatenates README.md + the 7
`docs/*.md`** (via `docs/generate_llms_full.sh`). T2.S2's PRP explicitly notes:
"if T3.S1 edits README, llms_full must be re-regenerated again." Since this audit
finds **README needs NO edit**, **no re-regeneration is required**. But the PRP
must state the conditional: IF (unexpectedly) an edit IS made to README or any
concatenated source, re-run `bash docs/generate_llms_full.sh` afterward so the
single-file dump stays in sync.

## Why the audit is still valuable even though it's a no-op
- It's the **final coherence gate** for the unified-`[[rule]]` changeset (C8): it
  confirms that README + the top-level overview + the spec docs don't contradict
  the unified schema a user/agent would copy from `docs/configuration.md` or
  `HOST_RULES.md` §9.
- It produces **grep evidence** that the Mode B "none" claim holds against the
  real repo (not just the delta PRD's assertion) — the SOW explicitly requires
  this verification, not assumption.
- It runs **last** (depends on T1 code + T2 docs), so it catches any drift
  introduced by the implementing subtasks.

## Deliverable shape
- If clean (expected): a **verification report** — the grep commands + their
  clean output + the README-blurb / spec-section analysis — captured in the
  commit message (if any incidental edit) or the task-completion record. No doc
  file changes required.
- If a stale reference IS found (unexpected): make the targeted edit per the
  item's conditional logic (see PRP "What"), then (if a concatenated source was
  touched) re-run the llms_full generator.