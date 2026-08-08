# Research Notes — P1.M1.T2.S1: verify README.md + spec docs remain correctly synced (mise/asdf)

Repo: **`/home/dustin/projects/qmkonnect`**. This is the **final changeset-level
documentation verification** (Mode B) for the mise/asdf channel removal. It runs
LAST, after S1 (docs/installation.md cleanup — Complete) and S2 (regenerate
docs/llms_full.txt — parallel). Expected outcome: **VERIFIED — no drift**.

## 0. What changed in this changeset (the delta under verification)

The delta (plan/008) removes mise/asdf as an F15 community-distribution channel.
mise/asdf are a "category mismatch" for an always-on single-instance tray daemon
(no autostart; the "switch versions" workflow is meaningless; updates re-wire
autostart). The spec source-of-truth already documents this decision
(`spec/PACKAGING.md` §6.4, `spec/PRD.md` F15/§2.1/§5). The packaging/asdf/ dir is
removed; the dead `github.com/dabstractor/asdf-qmkonnect` plugin repo link must
not survive anywhere. S1+S2 fix the two stale USER-FACING artifacts
(docs/installation.md, docs/llms_full.txt). This task verifies the WHOLE tree is
clean.

## 1. Ground-truth state (greps run this session)

### 1a. Authored user docs — CLEAN (the pass)
`grep -rin 'mise\|asdf' README.md docs/*.md` (excl `llms_full.txt` + `vendor/`)
→ **ZERO hits**. docs/installation.md alone = 0 (S1 landed). README.md = 0.
All 8 llms_full source files clean.

### 1b. docs/llms_full.txt — STALE NOW, CLEAN AFTER S2 (the dependency)
Currently 14 mise|asdf hits (lines 157, 160, 490, 750–760, 828–835) + 4
`asdf-qmkonnect` dead links (lines 750, 756, 759, 834). This is the PRE-S2 state.
S2 (parallel, in progress) regenerates it via `bash docs/generate_llms_full.sh` →
both greps return 0. **My task runs AFTER S2**, so at my runtime llms_full.txt is
clean. The gate must INCLUDE llms_full.txt (expect 0 post-S2).

### 1c. spec/ docs — INTENTIONAL "NOT a channel" references (NOT drift)
- `spec/PRD.md`: 3 hits — L97 (§2.1 Goals: "mise/asdf … explicitly NOT a channel"),
  L152 (F15 row: "mise/asdf are a category mismatch and are NOT a channel"),
  L169 (§5 dist channels: "mise/asdf are not channels"). All document the
  EXCLUSION decision. Correct.
- `spec/PACKAGING.md`: 5 hits — all in §6.4 "mise / asdf — NOT a channel
  (category mismatch)" (the PRD-provided content confirms). Correct.
- `spec/DEVICE_DISCOVERY.md`: **1 hit = FALSE POSITIVE** — L272 "promise." (the
  English word contains "mise"). NOT a channel reference. (Not in the §3 audit
  table because it's a substring accident.)

### 1d. asdf-qmkonnect dead links — 4 in llms_full.txt (S2 clears), 0 elsewhere
`grep -rn 'asdf-qmkonnect' .` (excl `.git/`, `target/`, `node_modules/`,
`.pi-subagents/`, `plan/`, `docs/vendor/`) → 4 hits, ALL in docs/llms_full.txt
(S2's scope). After S2: 0. The `plan/` hits are research artifacts (gitignored
plan area, correctly excluded). `packaging/asdf/` does not exist (removed).

### 1e. src/*.rs — FALSE POSITIVES (out of scope anyway)
`src/linux_tray.rs` + `src/tray.rs` matched on "zero-config **promise**"
("promise" ⊃ "mise"). These are Rust doc-comments about the zero-config design —
not channel refs. src/ is out of scope for a docs verification regardless.

## 2. The #1 gotcha: `docs/vendor/` is a false-positive mine

`docs/vendor/bundle/ruby/3.4.0/gems/...` — the Jekyll site's vendored Ruby gems —
contains **60 files** matching `mise|asdf` ("pro**mise**", "com**promise**",
"asdf" in gem test fixtures like kramdown/nokogiri/sass). A naive
`grep -rin 'mise\|asdf' docs/` returns ~60+ false-positive FILES and makes a
clean tree look dirty. **The gate MUST scope to authored docs only**
(`README.md` + `docs/*.md`, which by shell-glob excludes `docs/vendor/`'s nested
dirs) and explicitly exclude `docs/vendor/`. The stale_content_audit.md §3 and
the S1/S2 PRPs all operate on the authored docs only; the generator
(`generate_llms_full.sh`) also never reads vendor/.

## 3. The verification gate (exact commands + expected results, post-S1+S2)

```bash
# (a) Authored user docs — channel advertising must be ZERO.
grep -rin 'mise\|asdf' README.md docs/*.md      # excl vendor/ (glob) + llms_full (not .md)
# Expected: 0 hits. (docs/*.md glob does NOT descend into docs/vendor/.)

# (b) Generated artifact — must mirror the clean sources (post-S2).
grep -in 'mise\|asdf' docs/llms_full.txt         # Expected: 0.
grep -in 'asdf-qmkonnect' docs/llms_full.txt     # Expected: 0.

# (c) Dead asdf-qmkonnect links anywhere in the repo (excl noise + plan research).
grep -rn 'asdf-qmkonnect' . | grep -vE '\.git/|/target/|node_modules/|\.pi-subagents/|/plan/|docs/vendor/'
# Expected: 0. (plan/ research artifacts are correctly excluded — they're gitignored
#  planning notes, not shipped content; packaging/asdf/ is removed.)

# (d) README Package-Managers table — no mise/asdf row (spot-read).
grep -in 'mise\|asdf' README.md                   # Expected: 0 (already clean).
```

## 4. The intentional references (note as CORRECT, not drift)

The verification's OUTPUT must explicitly call out that the spec/ mise/asdf hits
are intentional and correct — they document WHY mise/asdf is excluded (the
"NOT a channel" decision), they do NOT advertise the channel:
- `spec/PRD.md` §2.1 (L97), F15 (L152), §5 (L169).
- `spec/PACKAGING.md` §6.4 (the "mise / asdf — NOT a channel" subsection).

This matches the item contract: "Any remaining hits in spec/PRD.md or
spec/PACKAGING.md §6.4 are intentional and correct." (DEVICE_DISCOVERY.md's
"promise." is a false positive, not intentional — note it as such.)

## 5. Verdict (the pre-verified result the runbook reproduces)

**VERIFIED: no documentation drift.** Post S1+S2:
- All USER-FACING docs (README.md, docs/*.md authored, docs/llms_full.txt) carry
  ZERO mise/asdf channel-advertising content and ZERO dead asdf-qmkonnect links.
- The spec/ mise/asdf references are the INTENTIONAL "NOT a channel" decision
  documentation (PRD §2.1/F15/§5, PACKAGING §6.4) — correct, not drift.
- The spec/DEVICE_DISCOVERY.md + src/*.rs hits are false positives ("promise").
- packaging/asdf/ is removed; no dead plugin-repo link survives outside plan/
  research artifacts.

This agrees with stale_content_audit.md §3 ("All Other Files — ALREADY SYNCED").

## 6. Scope boundaries (what NOT to do)

- **No src/ or spec/ edits** — this is a docs verification; src/ false positives
  ("promise") and spec/ intentional refs are out of scope. `git status` for
  src/+spec/ must stay clean.
- **No docs/vendor/ edits** — third-party Ruby gems; the false positives there
  are not QMKonnect content. Exclude from the gate; never edit.
- **No plan/ edits** — research artifacts (correctly excluded from the gate).
- **Depends on S1 (done) + S2 (parallel)** — if S2 has NOT landed when this task
  runs, docs/llms_full.txt still shows the 14+4 hits; that's an S2-pending state,
  NOT a finding for THIS task. The gate's llms_full.txt check is the S2 success
  criterion re-confirmed here.
- **Drift-found branch (unexpected):** if a mise/asdf channel-advertising ref or
  a dead asdf-qmkonnect link appears in a file NOT covered by S1/S2 (e.g. a
  stray ref in docs/usage.md or docs/examples.md), remove it in place (Mode B)
  and document it. Research confirms this branch is NOT reached.