# Research Notes — P1.M1.T1.S2 (Regenerate docs/llms_full.txt + verify)

## 0. Task shape

Run the committed generator `docs/generate_llms_full.sh` to regenerate
`docs/llms_full.txt` so its verbatim mirrors of `README.md` + `docs/installation.md`
pick up the mise/asdf removal. Then verify ZERO mise/asdf hits remain. No hand-editing
of llms_full.txt; no script modification; Mode B (changeset-level regen, depends on S1).

## 1. CRITICAL BASELINE — verify BEFORE regenerating (S1 must have landed)

Greps run this session against the CURRENT working tree:

| File | mise/asdf hits | Status |
|---|---|---|
| `README.md` | **0** | ✅ ALREADY CLEAN (independent of S1) |
| `docs/installation.md` | **12** (lines 29, 289–299, 367–374) | ❌ STILL STALE — S1 ("Implementing") has NOT landed yet |
| `docs/llms_full.txt` | **14** (lines 157, 160, 490, 750–760, 828–835) | ❌ STALE (the artifact S2 regenerates) |

**Conclusion / the gate:** S2 MUST NOT regenerate until S1 has landed
(`grep -ic 'mise\|asdf' docs/installation.md` → 0). If S2 regenerates while
installation.md is still stale, the generator re-bakes the stale content into
llms_full.txt: the README-derived lines (157/160) WOULD clear (README is clean), but
the installation-derived lines (490, 750–760, 828–835) would NOT — the regen would be
partial and would mask that S1 is incomplete. Task 1 of the PRP gates on this.

## 2. The 14 stale llms_full.txt hits — source mapping (why regen clears all 14)

| llms_full.txt line(s) | Content | Source doc | Clears on regen because… |
|---|---|---|---|
| 157 | `\| **Linux / macOS** \| mise · asdf \| ...\|` (table row) | README.md | README.md is already clean (0 hits) — the row is gone from the source |
| 160 | `> - **mise / asdf on macOS is CLI-only** — …` (blockquote) | README.md | same — README.md clean |
| 490 | `**mise / asdf** are cross-platform version managers…` (BLOCK 1 intro) | docs/installation.md:29 | S1 deletes BLOCK 1 |
| 750–760 | `**mise / asdf** — cross-platform…` + ```bash fence (BLOCK 2) | docs/installation.md:289–299 | S1 deletes BLOCK 2 |
| 828–835 | `**mise / asdf — CLI only…**` + ```bash fence (BLOCK 3) | docs/installation.md:367–374 | S1 deletes BLOCK 3 |

So once BOTH sources are clean (README already is; installation.md after S1), the
regenerated concatenation contains zero mise/asdf text — the 14 hits vanish entirely
(not relocated). Line numbers in llms_full.txt will SHIFT (installation.md shrinks
~24 lines across the 3 blocks), so post-regen verification uses grep (zero hits), NOT
line-number assertions.

## 3. The generator — `docs/generate_llms_full.sh` (RUN verbatim, do NOT modify)

- Shebang `#!/usr/bin/env bash`; `set -euo pipefail` (fails fast).
- `DOCS_DIR` = script dir; `ROOT` = repo root; `OUT="$DOCS_DIR/llms_full.txt"`; `DIV` = 80 `=`.
- `strip_fm()` — awk that strips a LEADING Jekyll `--- … ---` front-matter block (only when
  line 1 is `---`); files without front matter (e.g. README.md) pass through whole.
- `emit()` — prints a numbered `===` divider header (`N. path (label)`).
- **EXPLICIT ordered 8-file source list** (NOT a glob — the item's "8 hardcoded source files"):
  1. `$ROOT/README.md`
  2. `docs/index.md` (Home)
  3. `docs/installation.md` (Installation)  ← S1's fix propagates through here
  4. `docs/qmk-integration.md`
  5. `docs/configuration.md`
  6. `docs/usage.md`
  7. `docs/examples.md`
  8. `docs/troubleshooting.md`
- Writes the concatenation to `$OUT` (truncate), echoes `wrote $OUT (<lines> lines, <bytes> bytes)`.
- `docs/vendor/` is excluded BY DESIGN (it's not in the explicit list).
- Executable (0755). Canonical invocation (per the script's own header comment):
  `bash docs/generate_llms_full.sh && git diff --stat docs/llms_full.txt`.

**Do NOT "improve" the script to a glob** — it would reorder sections and pull in stray
files (LICENSE, _config.yml, vendor/). Run it verbatim.

## 4. The deterministic verification gates (this IS the contract)

After the generator runs (post-S1):
1. `grep -in 'mise\|asdf' docs/llms_full.txt` → **ZERO** hits (exit 1). [primary]
2. `grep -in 'asdf-qmkonnect' docs/llms_full.txt` → **ZERO** hits (exit 1). [the 4 broken links]
3. The script's stdout line `wrote docs/llms_full.txt (<N> lines, <N> bytes)` is present.

These three are the entire functional success criterion. The 14 stale hits must be GONE
(not relocated) because both source docs are clean.

## 5. Expected diff shape

- `git diff -- docs/llms_full.txt` → ideally: the 14 mise/asdf lines removed (plus the
  line-number shifts ripple through the file). Because the generator regenerates the
  WHOLE file, if ANY other source doc drifted since llms_full.txt was last committed,
  the diff also carries those changes — acceptable (the script's job is full sync), but
  REVIEW the diff. Per `stale_content_audit.md` §3, the only residual mise/asdf drift is
  in installation.md (S1) + the llms_full.txt mirror (S2); other docs are clean. So the
  minimal mise/asdf-removal diff is the expected outcome.
- `git status --short` → `M docs/installation.md` (S1) AND `M docs/llms_full.txt` (S2).
  S2 owns ONLY the llms_full.txt change.

## 6. Dependencies / environment

- Bash + coreutils (`awk`, `seq`, `wc`, `cat`, `printf`, `dirname`) — standard on the
  Linux dev box. No Jekyll/Ruby/cargo needed to RUN the script (pure shell text concat).
- Run from the repo root: `bash docs/generate_llms_full.sh` (the script resolves paths
  via `BASH_SOURCE`, so CWD doesn't strictly matter, but repo root is the documented call).

## 7. Files NOT to touch (boundary discipline)

- `docs/generate_llms_full.sh` — run as-is; do not modify the file list/ordering/awk.
- `docs/installation.md` — owned by S1 (the prerequisite); do not re-edit.
- `README.md` — already clean; do not touch.
- Any other `docs/*.md` source, `spec/*`, `src/`, `Cargo.toml`, `.github/`.
- `llms_full.txt` is REGENERATED, never hand-edited.

## 8. Sibling context

- **P1.M1.T1.S1** (parallel, "Implementing"): removes the 3 mise/asdf blocks from
  `docs/installation.md`. Treat as a CONTRACT: when S2 runs, installation.md has 0
  mise/asdf hits. S2's Task 1 gate verifies this BEFORE regenerating (do not regenerate a
  stale source into the mirror).
- **P1.M1.T2.S1** (next, "Verify README.md + spec docs remain synced"): the
  stale_content_audit §3 says README/PACKAGING/PRD are already clean; T2.S1 is a
  read-only confirmation. S2 (the regen) is independent of T2.S1.

## 9. Risk inventory (all low; all gated)

1. **Regenerating before S1 lands** → re-bakes stale installation.md content; the
   README-derived lines (157/160) clear but the installation-derived lines (490,
   750–760, 828–835) do NOT. Mitigated by Task 1's gate (`grep -ic mise|asdf
   docs/installation.md` → 0; halt if non-zero).
2. **Accidental hand-edit of llms_full.txt** → the PRP forbids it; the gate is "ran the
   script + greps pass", not "edited lines by hand".
3. **Larger-than-expected diff** (another doc drifted) → review; the two greps still
   pass. Document any extra hunks; they are legitimate syncs, not regressions.
4. **Modifying the generator script** → run verbatim; a glob "improvement" would reorder
   sections / pull in stray files.
5. **Line-number-based verification** → use grep (zero hits), not line numbers; the
   regen shifts line numbers.