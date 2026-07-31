# Research Notes — P1.M1.T2.S2: Regenerate `docs/llms_full.txt` and verify

Repo: **QMKonnect** (`/home/dustin/projects/qmkonnect`). This is the
**regeneration + verification** step that closes the docs-unification task
(`[[layer_rules]]`/`[[callback_rules]]` → unified `[[rule]]`). It runs the
committed generator script over the 8 doc sources and confirms the concat no
longer carries the stale split schema (S1) nor the withdrawn `≥ 224` layer floor.

## 0. Current state snapshot (verified this session)

| Surface | `[[layer_rules]]`/`[[callback_rules]]` | `[[rule]]` | Status |
| --- | --- | --- | --- |
| `docs/configuration.md` | **0** | 13 | ✅ S1 LANDED (unified) |
| `docs/examples.md` | **0** | 5 | ✅ S1 LANDED |
| `docs/qmk-integration.md` | **0** | 3 | ✅ S1 LANDED |
| `docs/troubleshooting.md` | **0** | 1 | ✅ S1 LANDED |
| `README.md`, `docs/index.md`, `docs/installation.md`, `docs/usage.md` | **0** | 0 | clean (never had split schema) |
| `src/` (all) | **0** | — | ✅ S3 LANDED (P1.M1.T1.S3 complete) |
| **`docs/llms_full.txt`** | **25** (stale) | **0** (stale) | ❌ **FROZEN at 2026-07-20** — never re-run after the doc edits (mtimes: docs 2026-07-31 18:25, llms_full 2026-07-20 06:50) |

⇒ **S1's precondition is MET** (all 4 source docs unified; `docs/` excl. llms_full
has 0 split tokens; src/ is clean). The ONLY stale artifact is `llms_full.txt`
itself, which the script will fix deterministically. (Note: S1 landed *during*
this research session — the docs flipped from 25 split tokens → 0 between two
read passes, confirming the parallel implementer finished.)

## 1. How the generator works (`docs/generate_llms_full.sh`)

- **Shebang:** `#!/usr/bin/env bash`, `set -euo pipefail` (fails fast on any error
  / unset var / pipe break). Syntax-checked clean (`bash -n` → OK). Executable bit set.
- **Path resolution:** `DOCS_DIR` = script's own dir; `ROOT` = parent. So it runs
  correctly from **any cwd** (the contract says "from the repo root" for clarity,
  but it's cwd-independent). `OUT="$DOCS_DIR/llms_full.txt"`.
- **Output:** OVERWRITES `docs/llms_full.txt`. Final line:
  `echo "wrote $OUT ($(wc -l < "$OUT") lines, $(wc -c < "$OUT") bytes)"`.
- **Structure:** a fixed header block (`# QMKonnect - Complete Documentation…` +
  the "REALITY CHECK" preamble) then 8 file sections. Each section is emitted by
  `emit()`:
  ```
  <blank>
  <80 '='>
   {N}. {path} {optional (label)}
  <80 '='>
  <blank>
  ```
  (a leading space before `{N}`.) Then the file body via `strip_fm()`.
- **`strip_fm()` awk:** drops a LEADING Jekyll front-matter block (`---`…`---`)
  ONLY when line 1 is `---`. Files without front matter pass through whole.
  - **README.md** has NO front matter (line 1 = `# QMKonnect`) → passed whole.
  - The **7 `docs/*.md`** files all start with `---` → front matter stripped.
  (Verified: each of index/installation/qmk-integration/configuration/usage/
  examples/troubleshooting has line 1 = `---`.)
- **The 8 files, in FIXED order:** (1) README.md, (2) docs/index.md,
  (3) docs/installation.md, (4) docs/qmk-integration.md, (5) docs/configuration.md,
  (6) docs/usage.md, (7) docs/examples.md, (8) docs/troubleshooting.md.
- **S1 touches only 4 of these** (qmk-integration, configuration, examples,
  troubleshooting). The other 4 (README, index, installation, usage) are
  **untouched by S1** — but verified clean (0 split tokens), so regenerating from
  them adds no stale schema. (README/index/installation top-level audit is a
  SEPARATE later task: P1.M1.T3.S1.)

## 2. The TWO fixes regeneration captures (both already in the sources)

### (a) Schema unification — split → `[[rule]]`
The 4 source docs now carry `[[rule]]` (SINGULAR) totaling **22 literals**
(13 + 5 + 3 + 1) and **0** split tokens. The frozen `llms_full.txt` still has
**25** split tokens + **0** `[[rule]]`. Regenerating flips llms_full to match:
0 split, ~22 `[[rule]]`.

### (b) Withdrawn `≥ 224` layer floor → "raw QMK index"
The frozen `llms_full.txt` still carries the WITHDRAWN guidance as ACTIVE
text (13 `224` references, incl. `≥ 224` at lines 889/1247/1868/2498/2526 and
`>= 224` ASCII at 1271/1278/1875, plus `layer = 224` example values at
1281/1878/1882). The source docs have ALREADY dropped all of that:
- `configuration.md` line 273 now says "a **raw QMK layer index**, not a reserved
  range … `<` your firmware's `layer_state_t` width … `!= 255`" (no floor).
- The examples use `layer = 10` / `layer = 11` (not 224).
- The **ONLY remaining `224` mention in the 8 sources** is the intentional
  WITHDRAWAL NOTE in `docs/examples.md:288-289`:
  > "(The earlier guidance to remap `_GAMING = 1` → `layer = 224` is withdrawn —
  > `layer_state` cannot hold bit 224.)"
  This note says `layer = 224` / "bit 224" — it does NOT contain `≥ 224` or
  `>= 224`. So after regeneration, `llms_full.txt` will have **0** `≥ 224`/`>= 224`
  floor GUIDANCE, while still (correctly) carrying the 2-line withdrawal note.

⇒ Verification (d) must grep for the **floor-guidance forms** `≥ 224` (unicode)
and `>= 224` (ASCII) — both → 0 after regen. A bare `224` grep will still find
the withdrawal note (2 lines) — that is EXPECTED and correct, not a failure.

## 3. The four verification steps (exact forms + expected results)

Per the contract. All run from `/home/dustin/projects/qmkonnect`.

**(a) Zero stale split-schema tokens in `docs/` + `src/`:**
```bash
grep -rnE '\[\[(layer|callback)_rules\]\]' docs/ src/
```
- BEFORE regen: 25 hits (all in `docs/llms_full.txt`). AFTER regen: **0**.
- Scope is `docs/ src/` only. NOTE: `plan/` (28 files — the PRPs' OLD-text blocks
  contain these tokens) and `.pi-subagents/` (transcripts) and `target/` (build
  artifacts) ALL contain these tokens but are OUT of scope — do NOT repo-wide grep.
- `src/` is already 0 (S3 complete); this step mainly proves llms_full was fixed.

**(b) Regenerated `[[rule]]` hits present in llms_full:**
```bash
grep -cE '\[\[rule\]\]' docs/llms_full.txt
```
- BEFORE: 0. AFTER: **~22** (13+5+3+1 from the 4 source docs). Gate: ≥ 20 (the
  exact count isn't load-bearing; the point is the unified literals propagated).

**(c) Rust sanity (no behavior change — S2 touches no Rust):**
```bash
cargo test --bin qmkonnect -- --test-threads=1   # single-threaded (shared debouncer)
cargo check --bin qmkonnect --offline            # deps are cached (Cargo.lock present)
```
- There is **NO test/code referencing `llms_full`** (confirmed: `grep -rn llms_full
  src/ tests/` → empty). So these are pure "nothing broke" sanity checks, expected
  to pass unchanged. The `--test-threads=1` and `--offline` match the AGENTS.md
  dev loop. If `--offline` fails (uncached deps / fresh clone), drop `--offline`
  (the regen itself needs no Rust).

**(d) No `≥ 224`/`>= 224` floor GUIDANCE in llms_full:**
```bash
grep -nE '≥ 224|>= 224' docs/llms_full.txt
```
- BEFORE: several (889, 1247, 1271, 1278, 1868, 2498, 2526, …). AFTER: **0**.
- Do NOT scope this to all of `docs/` — `docs/vendor/` (vendored Ruby gems /
  Jekyll font SVGs) contains thousands of false-positive `224` / `>=` substrings
  in glyph path data. Scope to `docs/llms_full.txt` only (the script never
  concatenates `docs/vendor/`).

## 4. Scope boundaries

- **DO** regenerate `docs/llms_full.txt` (the one deliverable artifact) + commit it
  with S1's doc edits.
- **DO NOT hand-edit** `llms_full.txt` — it's a pure concatenation; any manual edit
  is overwritten on the next run and risks merge entropy. The script is the only
  sanctioned writer.
- **DO NOT touch** `src/`, `docs/*.md` (S1 owns those), `spec/HOST_RULES.md`
  (read-only source of truth), or the script itself.
- **DO NOT** repo-wide grep for the split tokens — `plan/` (PRPs),
  `.pi-subagents/` (transcripts), `target/`, and `docs/vendor/` (gems) all carry
  them as noise. Scope greps to `docs/ src/` (token) or `docs/llms_full.txt` (224).

## 5. The commit

Per the contract OUTPUT: "Commit llms_full.txt alongside the doc changes." So the
regenerated `docs/llms_full.txt` goes into the SAME commit as S1's 4 doc edits
(one coherent "unified `[[rule]]` schema in docs + regenerated llms_full" commit).
`docs/llms_full.txt` is git-TRACKED (confirmed: `git ls-files` lists it), so it's
`git add`-able (unlike build outputs, which are gitignored).