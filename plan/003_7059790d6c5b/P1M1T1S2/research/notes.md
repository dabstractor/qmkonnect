# Research Notes — P1.M1.T1.S2 (Regenerate docs/llms_full.txt)

## Scope (one line)

Run `bash docs/generate_llms_full.sh` from the repo root to regenerate
`docs/llms_full.txt` so its verbatim mirror of `docs/troubleshooting.md` (currently
line 2622) picks up the S1 fix (`qmk-notifier_notify` → `qmk_notifier_notify`).
Then verify with two deterministic greps. No hand-editing of llms_full.txt; no
script modification. This IS the documentation deliverable (Mode A).

## CRITICAL BASELINE STATE (verified in the working tree right now)

- **Source is ALREADY fixed (S1 landed).** `grep -n 'qmk_notifier_notify'
  docs/troubleshooting.md` → `647:   (there is no built-in \`qmk_notifier_notify\`
  callback …)`; `grep -n 'qmk-notifier_notify' docs/troubleshooting.md` → zero
  hits. So the S1 edit is present; S2 just needs to propagate it into the
  generated mirror.
- **Mirror is STILL STALE.** `grep -n 'qmk-notifier_notify' docs/llms_full.txt` →
  `2622:   (there is no built-in \`qmk-notifier_notify\` callback …)`;
  `grep -n 'qmk_notifier_notify' docs/llms_full.txt` → zero hits. So llms_full.txt
  is out of sync with the (now-fixed) source — exactly the residual drift this
  task closes (delta_verification.md §Residual Drift item #2).
- **Conclusion:** S2 is a pure regeneration run. No creative editing. Run the
  committed script, verify the two greps, review the diff.

## The regeneration script — `docs/generate_llms_full.sh` (READ, do NOT modify)

- Shebang `#!/usr/bin/env bash`; `set -euo pipefail` (fails fast on error / unset
  var / pipe failure).
- Computes `DOCS_DIR` (dirname of the script), `ROOT` (parent = repo root),
  `OUT="$DOCS_DIR/llms_full.txt"`, `DIV` = 80 `=` characters.
- `strip_fm()` — an `awk` one-liner that strips a LEADING Jekyll front-matter
  block (`--- … ---`) ONLY when line 1 is `---`. Files without front matter (e.g.
  README.md) pass through whole.
- `emit()` — prints a numbered `===` divider header like
  `\n====...====\n 8. docs/troubleshooting.md (Troubleshooting)\n====...====\n\n`.
- **Explicit ordered source list (NOT a glob):**
  1. `$ROOT/README.md`
  2. `docs/index.md` (Home)
  3. `docs/installation.md` (Installation)
  4. `docs/qmk-integration.md` (QMK Integration)
  5. `docs/configuration.md` (Desktop-side Configuration)
  6. `docs/usage.md` (Usage)
  7. `docs/examples.md` (Firmware examples)
  8. `docs/troubleshooting.md` (Troubleshooting)  ← the file whose fix propagates
- Writes the whole concatenation to `$OUT` (truncating), then echoes
  `wrote $OUT (<lines> lines, <bytes> bytes)`.
- **Item-description caveat:** the item says the script "concatenates README.md +
  docs/*.md", but the script is an **explicit ordered list of 8 files**, not a
  glob. The script is the source of truth — run it verbatim. Do not "improve" it
  to a glob (would change ordering and pull in files like `LICENSE`, `_config.yml`
  side-effects, or future stray `.md`).
- The header block (HDR) in the output already uses the **underscore** firmware
  convention ("companion **qmk_notifier** module") — it is NOT stale; only the
  troubleshooting.md mirror is. So the regen does not need a header edit.

## Why the line number stays at ~2622 after regen

The script is byte-deterministic given fixed source inputs. S1 changed exactly
ONE character in troubleshooting.md:647 (hyphen-minus → underscore, same byte
length). `strip_fm` strips front matter only; troubleshooting.md's body is
emitted whole. Therefore the regenerated llms_full.txt differs from the current
committed one by that single character, and the mirror line stays at **line
2622** (line count is unchanged). The verification grep does not depend on the
exact number — it just needs (a) zero hyphen-form hits and (b) one underscore-form
hit. But a stable line 2622 is the expected outcome and a good sanity signal.

## The two deterministic verification gates (this is the "contract check")

After running the script:
1. `grep -n 'qmk-notifier_notify' docs/llms_full.txt` → **MUST print nothing**
   (exit code 1). The stale hyphen form is gone.
2. `grep -n 'qmk_notifier_notify' docs/llms_full.txt` → **MUST print exactly one
   line** (≈ line 2622): the underscore form, now mirrored.

These two greps are the entire functional success criterion. Everything else is
hygiene (diff review, fence balance, no unintended file changes).

## Expected diff shape

- `git diff -- docs/llms_full.txt` → ideally **one hunk, one line** (line 2622:
  hyphen → underscore). Because the script regenerates the WHOLE file, if any
  OTHER source doc drifted since llms_full.txt was last committed, the diff will
  also carry those changes — that is acceptable (the script's job is full sync),
  but review the diff. Per `delta_verification.md`, the ONLY residual drift is
  this one token, so the minimal one-line diff is the expected outcome.
- `git status --short docs/` → shows `M docs/troubleshooting.md` (from S1) AND
  `M docs/llms_full.txt` (from S2). S2 owns only the llms_full.txt line.

## Dependencies / environment

- Bash + coreutils (`awk`, `seq`, `wc`, `cat`, `printf`, `dirname`) — all
  standard on the Linux dev box (`/home/dustin/projects/qmkonnect`). No Jekyll,
  no Ruby, no cargo needed to RUN the script (it is pure shell text concat).
- Run from the repo root: `bash docs/generate_llms_full.sh` (the script resolves
  paths via `BASH_SOURCE`, so CWD does not strictly matter, but repo root is the
  documented invocation).
- The script is executable (`-rwxr-xr-x`), so `docs/generate_llms_full.sh` also
  works; the canonical invocation in the item is `bash docs/generate_llms_full.sh`.

## Files NOT to touch

- `docs/generate_llms_full.sh` — run as-is; do not modify the file list, ordering,
  or awk.
- `docs/troubleshooting.md` — owned by S1 (already fixed); do not re-edit.
- Any `docs/*.md` source, README.md, `_config.yml`, Gemfile, LICENSE.
- llms_full.txt is REGENERATED, never hand-edited.
- No source code (`src/`), no `Cargo.toml`, no `.github/` workflows.

## Sibling context

- **P1.M1.T1.S1** (sibling, parallel, "Implementing"): fixed
  troubleshooting.md:647 (hyphen → underscore). Treat as a CONTRACT: when S2
  runs, the source line 647 reads `qmk_notifier_notify`. S2 verifies this before
  regen (Task 1) so it does not regenerate a stale source.
- **P1.M1.T1.S3** (next sibling, "Full-tree verification grep and cargo check"):
  will run the cross-tree grep `qmk-notifier_notify` (expect zero hits repo-wide
  after S1+S2) AND a `cargo check` (unrelated to docs; confirms no source break).
  S2 must land the llms_full.txt regen BEFORE S3 so S3's repo-wide grep is clean.

## Risk inventory (all low; all mitigated by the deterministic gates)

1. **Script not run from repo root.** Mitigation: the script uses `BASH_SOURCE`
   path resolution; the documented `bash docs/generate_llms_full.sh` works from
   any CWD. Run it from repo root anyway (matches the item spec).
2. **Diff larger than one line** (another doc drifted). Mitigation: review the
   diff; the two greps still pass. Document any extra hunks; they are legitimate
   syncs, not regressions.
3. **Front-matter stripping edge case** (troubleshooting.md without leading `---`).
   Mitigation: the awk only strips when line 1 is `---`; otherwise pass-through.
   The current committed llms_full.txt was generated by this same script, so
   re-running it reproduces the same structure minus the one-character fix.
4. **Accidental hand-edit of llms_full.txt.** Mitigation: the PRP explicitly
   forbids it and the success gate is "ran the script + greps pass", not "edited
   line 2622 by hand".
5. **Stale source regen** (S1 not actually landed when S2 runs). Mitigation:
   Task 1 greps troubleshooting.md FIRST and HALTS if the source is still hyphen
   (do not regenerate a stale source into the mirror).