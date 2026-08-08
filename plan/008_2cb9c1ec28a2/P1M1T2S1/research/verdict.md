# Verdict — P1.M1.T2.S1 (repo-wide mise/asdf removal Mode B sweep)

## VERIFIED: no documentation drift

The mise/asdf channel-removal changeset (plan/008) is **complete across every
user-facing doc**, and **no dead `asdf-qmkonnect` plugin-repo link survives**
anywhere outside gitignored planning artifacts. S1 (installation.md) and S2
(llms_full.txt regeneration) have both landed; this repo-wide belt-and-suspenders
sweep independently re-confirms `stale_content_audit.md` §3's "All Other Files —
ALREADY SYNCED" assertion. No file was modified.

## Evidence

### Prerequisites confirmed

- **S1 (Complete):** `docs/installation.md` → 0 mise|asdf, 0 asdf-qmkonnect.
- **S2 (landed):** `docs/llms_full.txt` → 0 mise|asdf, 0 asdf-qmkonnect (the
  PRE-S2 state of 14 + 4 is gone; the regeneration is current).

### Level 1 — the four gate greps

| gate | command | expected | result |
| --- | --- | ---: | ---: |
| (a) authored user docs | `grep -rin 'mise\|asdf' README.md docs/*.md` | 0 | **0** |
| (b-i) llms_full.txt mise\|asdf | `grep -in 'mise\|asdf' docs/llms_full.txt` | 0 (post-S2) | **0** |
| (b-ii) llms_full.txt dead links | `grep -in 'asdf-qmkonnect' docs/llms_full.txt` | 0 (post-S2) | **0** |
| (c) repo-wide dead links | `grep -rn 'asdf-qmkonnect' . \| grep -vE '\.git/\|/target/\|node_modules/\|\.pi-subagents/\|/plan/\|docs/vendor/'` | 0 | **0** |
| (d) README | `grep -in 'mise\|asdf' README.md` | 0 | **0** |

`packaging/asdf/` → **absent (correct)** — the removed plugin dir does not exist.

The `docs/*.md` glob does not descend into `docs/vendor/` (sidestepping the ~60
Ruby-gem false positives), and `docs/llms_full.txt` is not `.md` so gate (a)
naturally excludes it.

### Level 2 — spec/ classification (correct vs false positive)

Every `spec/` mise|asdf hit classified — **none are drift**:

| file:line | text (gist) | classification |
| --- | --- | --- |
| `spec/PRD.md:97` | "Runtime version managers like mise/asdf are a [category mismatch…]" (§2.1 Goals) | **INTENTIONAL** — "NOT a channel" decision |
| `spec/PRD.md:152` | F15 row: "mise/asdf are a category mismatch and are NOT a channel" | **INTENTIONAL** |
| `spec/PRD.md:169` | "mise/asdf are not channels" (§5) | **INTENTIONAL** |
| `spec/PACKAGING.md:393,395,402,405,406` | §6.4 "mise / asdf — NOT a channel (category mismatch)" | **INTENTIONAL** — authoritative exclusion decision (even notes "`packaging/asdf/` has been removed") |
| `spec/DEVICE_DISCOVERY.md:272` | "promise." | **FALSE POSITIVE** — English word ⊃ "mise" |

These are the project's "we decided NOT to" record + a substring accident.
Removing the intentional ones would delete the decision record. `spec/` is never
edited by this task.

### Level 3 — no-edit invariant

- `git status --short src/ spec/` → **clean** (0). No code/spec touched.
- `git status --short docs/vendor/` → **clean** (0). Third-party gems untouched.
- `plan/` shows only orchestrator-owned `tasks.json` + this task's `P1M1T2S1/`
  research dir (gitignored planning artifacts, correctly excluded from gate (c)).

### README spot-read

The `### Package Managers` table (README.md:121–133) lists exactly the 7 real
channels — AUR, Nix, .deb, .rpm, Homebrew, Scoop, Winget. **No mise/asdf row.**
Accurate (gate (d) already confirmed 0 hits).

## Branch taken

**No-drift branch** (the expected, research-confirmed result). The drift-found
branch (Mode-B in-place fix to a sibling authored doc) was not reached: no
mise/asdf channel-advertising ref or dead `asdf-qmkonnect` link survives outside
the two files S1/S2 own. Agreement with `stale_content_audit.md` §3 is reproduced
independently.

## Conclusion

The mise/asdf doc-removal changeset is verified-complete across code / spec /
user-docs: authored docs (gate a), the generated LLM context (gate b, post-S2),
the whole repo's dead-link surface (gate c), and the README (gate d) are all
clean; the intentional `spec/` "NOT a channel" decision record is correctly
preserved; `packaging/asdf/` is gone. Combined with S1 (Complete) and S2 (landed),
this closes the plan/008 mise/asdf changeset.