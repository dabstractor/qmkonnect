# PRP — P1.M1.T1.S1: Remove mise/asdf sections from docs/installation.md

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). ONE file edited:
> `docs/installation.md`. **Doc-only deletion — no code change.**
> **Scope:** delete exactly three self-contained mise/asdf blocks. Do NOT touch the
> platform overview table, Nix, .deb, .rpm, Homebrew, Scoop, Winget, or AUR sections.
> `docs/llms_full.txt` regeneration is the **next sibling** (P1.M1.T1.S2) — out of
> scope here, and it MUST run AFTER this edit (its lines mirror installation.md).

---

## Goal

**Feature Goal**: Remove the three stale mise/asdf blocks from `docs/installation.md`
so the user-facing install docs match the authoritative decision that mise/asdf are
**NOT a distribution channel** (category mismatch for an always-on tray daemon —
`spec/PACKAGING.md` §6.4, `spec/PRD.md` F15). After the edit, `grep -in 'mise\|asdf'
docs/installation.md` and `grep -in 'asdf-qmkonnect' docs/installation.md` both
return **zero** hits, and the surrounding sections (platform table → `## Windows`;
Nix → `.deb`; Homebrew → `### Launch at login`) are intact with exactly one blank
line separating each.

**Deliverable**: `docs/installation.md` with the three blocks deleted (BLOCK 1 lines
29–32, BLOCK 2 lines 289–301, BLOCK 3 lines 367–376 — content + trailing blank line
each). No other change.

**Success Definition**: both grep gates return zero hits; `git diff` shows only
deletions in `docs/installation.md`; the three neighboring junctions each have
exactly one blank-line separator (no double blanks, no orphaned headings); markdown
fence/heading balance is unchanged (the two ```bash fences removed are the only fence
count change — net −2 fence lines, balanced).

## User Persona (if applicable)

**Target User**: A user reading the install docs who would otherwise be misled into
trying `asdf`/`mise` (which the project has decided is a category mismatch — no
autostart, version-switching meaningless for a single-instance daemon, updates
re-wire autostart).

**Use Case**: User opens `docs/installation.md` to pick an install method; the
remaining channels (Inno/.dmg/AUR/Nix/.deb/.rpm/Homebrew/Scoop/Winget) are the
truthful set.

**Pain Points Addressed**: Removes 4 broken `github.com/dabstractor/asdf-qmkonnect`
links (the repo is gone — `packaging/asdf/` was removed) and stops advertising a
non-channel.

## Why

- **Spec/code are already synced; only the user-facing doc lags.** The audit
  (`plan/008_2cb9c1ec28a2/architecture/stale_content_audit.md`) verified
  `spec/PACKAGING.md` §6.4, `spec/PRD.md` F15/§2.1/§5, `README.md`, `release.yml`,
  and all `src/*.rs` are already clean of mise/asdf-as-a-channel. The 3 blocks in
  `docs/installation.md` (12 grep hits + 4 broken links) are the sole residual drift.
- **It is the prerequisite for the llms_full.txt regen (S2).** The audit §2 notes the
  generated `docs/llms_full.txt` mirrors installation.md at lines 490, 750–760,
  828–835; regenerating BEFORE fixing the source would leave those stale. This
  subtask fixes the source first.
- **It is a clean, bounded deletion.** Three self-contained blocks; the grep gate is
  deterministic.

## What

Delete exactly three blocks. Each block = its content lines + its **trailing blank
line**, leaving the **preceding** blank line as the single separator between the
neighboring sections. (Verbatim text below — match it exactly, including em-dashes
`—`, the ellipsis `…`, and the middot `·` if present.)

### BLOCK 1 — intro paragraph (lines 29–32)

**Preceded by** the platform overview table's last row (line 27) + a blank line (28).
**Followed by** `## Windows` (line 33).

Delete these 3 content lines + the trailing blank line (32):
```
**mise / asdf** are cross-platform version managers that install the prebuilt release binary:
**Linux** (full app) and **macOS** (**CLI only — no menu-bar tray**); not available on Windows.
See the per-platform sections.

```
**After deletion:** the table's trailing blank (28) directly precedes `## Windows`
(one blank line between them). The platform overview table (lines 23–27) is
**untouched**.

### BLOCK 2 — Linux "mise / asdf" subsection (lines 289–301)

**Preceded by** the Nix flake README paragraph (ends line 287) + a blank line (288).
**Followed by** `**.deb (Debian / Ubuntu)**` (line 302).

Delete this subsection (text + ```bash code block + trailing blank line 301):
```
**mise / asdf** — cross-platform version managers. The same `asdf-qmkonnect` plugin serves both
(mise runs asdf plugin scripts unchanged). **Linux is fully supported** — install the binary, then
run the one-time udev/systemd setup the plugin documents:

```bash
# asdf:
asdf plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect
asdf install qmkonnect latest
# mise:
mise plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect
mise install qmkonnect@latest
```

```
> Note: the block contains a nested ```bash … ``` fence. When deleting, remove the
> opening ` ```bash ` line, all 6 command lines, and the closing ` ``` ` line — the
> whole fenced region plus the trailing blank. **After deletion:** the Nix paragraph's
> trailing blank (288) directly precedes `**.deb (Debian / Ubuntu)**` (one blank line).
> This removes 3 of the 4 broken `asdf-qmkonnect` links.

### BLOCK 3 — macOS "mise / asdf — CLI only" subsection (lines 367–376)

**Preceded by** the Homebrew uninstall paragraph (ends line 365) + a blank line (366).
**Followed by** `### Launch at login` (line 377).

Delete this subsection (text + ```bash code block + trailing blank line 376):
```
**mise / asdf — CLI only (no menu-bar tray).** These install the raw Mach-O binary from the DMG,
which runs CLI flags (`--help`, `--list-callbacks`, `-r`, …) but **not** the menu-bar tray/icon —
that needs the full `.app` bundle. For the complete macOS app, use the **Homebrew cask** above or
the **direct DMG** instead:

```bash
asdf plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect
asdf install qmkonnect latest        # CLI only — no menu-bar app
```

```
> Same fence-removal care as BLOCK 2. **After deletion:** the Homebrew paragraph's
> trailing blank (366) directly precedes `### Launch at login` (one blank line). This
> removes the 4th broken `asdf-qmkonnect` link.

### Success Criteria

- [ ] `grep -in 'mise\|asdf' docs/installation.md` → **zero** hits.
- [ ] `grep -in 'asdf-qmkonnect' docs/installation.md` → **zero** hits.
- [ ] The platform overview table (lines 23–27) is byte-for-byte unchanged.
- [ ] `## Windows`, `**.deb (Debian / Ubuntu)**`, and `### Launch at login` are each
      preceded by exactly one blank line (no double blanks, no zero-blank joins).
- [ ] Nix, .deb, .rpm, Homebrew, Scoop, Winget, AUR sections are untouched.
- [ ] `git diff -- docs/installation.md` shows only deletions (no additions/edits).
- [ ] No file other than `docs/installation.md` is modified (llms_full.txt = S2).

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed
> to implement this successfully?"_ — **Yes.** The verbatim text of all three blocks,
> their exact line ranges, their preceding/following anchors, the one-blank-line
> spacing rule, the "do not touch" list, and the deterministic grep gates are all below.

### Documentation & References

```yaml
# MUST READ — the audit that enumerated the three blocks (verbatim text + line ranges + neighbors)
- file: /home/dustin/projects/qmkonnect/plan/008_2cb9c1ec28a2/architecture/stale_content_audit.md
  why: "§1 lists the exact 3 blocks (lines 29-32, 289-301, 367-376) with verbatim content, the preceding/
        following anchors, and the post-edit spacing. §2 documents that llms_full.txt mirrors these lines
        (490, 750-760, 828-835) and MUST be regenerated AFTER this edit (S2). §3 confirms every other file
        (PACKAGING.md, PRD.md, README.md, release.yml, src/*.rs) is already clean."
  section: "1. docs/installation.md — STALE (3 blocks...)", "2. docs/llms_full.txt", "3. All Other Files"
  critical: "All 12 mise/asdf grep hits + all 4 broken asdf-qmkonnect links are CONFINED to these 3 blocks.
             No other mise/asdf content exists in the file. Delete the blocks and the file is clean."

# MUST READ — the file being edited (confirm exact current text + line numbers before editing)
- file: /home/dustin/projects/qmkonnect/docs/installation.md
  why: "Contains the 3 blocks at the cited lines. Line numbers may have drifted since the audit — MATCH ON
        THE VERBATIM TEXT (the What section quotes it), not just line numbers. 498 lines total."
  pattern: "Jekyll/Liquid markdown ({{ site.baseurl }} links elsewhere). The 3 target blocks are plain
            markdown prose + two ```bash fences. Neighbors: platform table (23-27), ## Windows (33),
            Nix paragraph (~286-287), **.deb...** (302), Homebrew uninstall (~364-365), ### Launch at login (377)."
  gotcha: "Each block's em-dash (—), ellipsis (…), and backtick-fenced commands must be matched EXACTLY in the
           deletion (an edit tool's oldText is whitespace/punctuation sensitive). Preserve the ONE blank line
           between each block's neighbors after deletion."

# REFERENCE — the authoritative decision (WHY the docs must change; already synced in spec)
- file: /home/dustin/projects/qmkonnect/spec/PACKAGING.md
  why: "§6.4 'mise / asdf — NOT a channel (category mismatch)' is the rationale: no autostart, version-switching
        meaningless for a single-instance daemon, updates re-wire autostart. Cited in the doc work's justification."
  section: "6.4 mise / asdf — NOT a channel (category mismatch)"
- file: /home/dustin/projects/qmkonnect/spec/PRD.md
  why: "F15 row enumerates the real channels (AUR, .deb/.rpm, Homebrew, Scoop, Winget, Nix) and explicitly
        excludes mise/asdf. Already synced — confirms the user-facing doc is the only stale surface."
  section: "4. Top-Level Feature Set (F15)", "2.1 Goals (item 6)"
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/
├── docs/
│   ├── installation.md        # <-- FILE TO EDIT (delete 3 blocks). 498 lines.
│   ├── llms_full.txt          # GENERATED — regenerated in S2 AFTER this edit (mirrors installation.md)
│   └── generate_llms_full.sh  # the generator (S2 runs it; NOT this subtask)
├── spec/PACKAGING.md          # §6.4 = authoritative "mise/asdf NOT a channel" (already synced; read-only ref)
└── spec/PRD.md                # F15 = channel list excluding mise/asdf (already synced; read-only ref)
```

### Desired Codebase tree with files to be modified

```bash
docs/
└── installation.md   # MODIFIED ONLY — 3 blocks deleted (content + trailing blank line each).
```

> No new files. `docs/llms_full.txt` is NOT regenerated here (P1.M1.T1.S2). No spec file is edited
> (PACKAGING.md/PRD.md are already correct — they are the read-only justification).

### Known Gotchas of our codebase & Library Quirks

```text
# CRITICAL: match the verbatim text, not just line numbers.
#   The audit's line ranges (29-32, 289-301, 367-376) were accurate at audit time but may have drifted.
#   The What section quotes each block verbatim — use that as the deletion key (edit-tool oldText).
#   Em-dash (—), ellipsis (…), and the middot (·) in the platform table must be handled exactly.

# CRITICAL: preserve exactly ONE blank line at each of the 3 junctions.
#   Delete each block's content + its TRAILING blank line, keeping the PRECEDING blank line as the separator.
#   Junctions after edit: table-blank → ## Windows; Nix-blank → **.deb...**; Homebrew-blank → ### Launch at login.
#   Avoid leaving zero blanks (heading sticks to paragraph) OR two blanks (markdown extra-spacing).

# CRITICAL: do NOT touch the platform overview table (lines 23–27).
#   It lists the real channels (Scoop·Winget / Homebrew Cask / AUR·Nix·.deb/.rpm·PKGBUILD/binary) and is correct.
#   BLOCK 1 is the paragraph AFTER the table (line 29+), not the table itself.

# CRITICAL: do NOT delete the ```bash fence lines incompletely.
#   BLOCK 2 and BLOCK 3 each contain a ```bash … ``` fence. Remove the opening fence line, all command lines,
#   AND the closing fence line together. Leaving a dangling ``` creates a malformed doc.

# NOTE: llms_full.txt mirrors installation.md — regenerate AFTER this edit (S2), not before.
#   The audit §2: llms_full.txt lines 490, 750-760, 828-835 are derived from these exact blocks. Regenerating
#   before the source fix would leave them stale. This subtask = source fix only.

# NOTE: this is a markdown doc — there is no build/test gate. The grep gates ARE the verification. A markdown
#   linter is not configured for this repo; do not invent one.
```

## Implementation Blueprint

### Data models and structure

No data models. The "structure" is the markdown section flow: each deletion must
leave the preceding and following sections joined by exactly one blank line, with no
orphaned fence markers.

### Implementation Tasks (ordered: the three blocks, then verify)

```yaml
Task 1: CONFIRM current text of the 3 blocks (line numbers may have drifted)
  - RUN: grep -in 'mise\|asdf' docs/installation.md
  - EXPECT: exactly 12 hits, clustered in 3 regions (BLOCK 1 ~line 29; BLOCK 2 ~289-299; BLOCK 3 ~367-374).
  - READ: the verbatim text of each block + 2 lines before/after, to anchor the exact deletion boundaries.
  - GOAL: ensure the deletion keys (oldText) match the file exactly, incl. em-dash/ellipsis/fence lines.

Task 2: DELETE BLOCK 1 (intro paragraph, ~lines 29-32)
  - DELETE: the 3 content lines ("**mise / asdf** are cross-platform..." / "**Linux** (full app)..." /
          "See the per-platform sections.") + the trailing blank line.
  - PRESERVE: the platform overview table (23-27) and the blank line (28) that precedes the block.
  - RESULT: table-row (27) → blank (28) → "## Windows". One blank separator.

Task 3: DELETE BLOCK 2 (Linux subsection, ~lines 289-301)
  - DELETE: the 3-line intro ("**mise / asdf** — cross-platform..." / "(mise runs..." / "run the one-time...")
          + its blank + the ```bash fence + 6 command lines + closing ``` fence + trailing blank line.
  - PRESERVE: the Nix paragraph (ends ~287) and its trailing blank (288).
  - RESULT: Nix-paragraph → blank (288) → "**.deb (Debian / Ubuntu)**". One blank separator.

Task 4: DELETE BLOCK 3 (macOS subsection, ~lines 367-376)
  - DELETE: the 4-line intro ("**mise / asdf — CLI only...**" / "which runs CLI flags..." / "that needs the
          full `.app` bundle..." / "the **direct DMG** instead:") + its blank + the ```bash fence + 2 command
          lines + closing ``` fence + trailing blank line.
  - PRESERVE: the Homebrew uninstall paragraph (ends ~365) and its trailing blank (366).
  - RESULT: Homebrew-paragraph → blank (366) → "### Launch at login". One blank separator.

Task 5: VALIDATE (the grep gates + structure sanity)
  - RUN: grep -in 'mise\|asdf' docs/installation.md          → EXPECT zero hits
  - RUN: grep -in 'asdf-qmkonnect' docs/installation.md      → EXPECT zero hits
  - RUN: git diff --stat -- docs/installation.md             → EXPECT 1 file, deletions only
  - RUN: the junction-spacing + fence-balance checks (Validation Loop Level 3).
```

### Implementation Patterns & Key Details

```text
# === THE DELETION RULE (apply to all 3 blocks) ===
# Delete: block content lines + the block's TRAILING blank line.
# Keep:   the PRECEDING blank line (it becomes the single separator to the next section).
# Why trailing (not leading): matches the audit's line ranges (29-32, 289-301, 367-376) and yields exactly
# one blank at each junction deterministically.

# === EDIT-Tool oldText (exact, unique per block) ===
# Block 1 oldText starts: "**mise / asdf** are cross-platform version managers that install the prebuilt release binary:"
# Block 2 oldText starts: "**mise / asdf** — cross-platform version managers. The same `asdf-qmkonnect` plugin serves both"
# Block 3 oldText starts: "**mise / asdf — CLI only (no menu-bar tray).** These install the raw Mach-O binary from the DMG,"
# Each is unique in the file → safe exact-match deletion. Include the trailing blank line in oldText; replace with "".

# === DO NOT TOUCH ===
# Platform overview table (23-27); Nix, .deb, .rpm, Homebrew, Scoop, Winget, AUR sections; ## Windows,
# ### Launch at login headings (these are the NEIGHBORS, preserved).
```

### Integration Points

```yaml
SOURCE FILES:
  - modify: "docs/installation.md ONLY (3 block deletions)"

DOWNSTREAM CONSUMER (do NOT run here — next sibling):
  - P1.M1.T1.S2: "Regenerate docs/llms_full.txt via `bash docs/generate_llms_full.sh` AFTER this edit, then
                  verify zero mise/asdf hits in llms_full.txt too. The audit §2 maps the stale llms_full lines
                  (490, 750-760, 828-835) to these blocks."

RELATED (do NOT implement now):
  - P1.M1.T2.S1: "Verify README.md + spec docs remain correctly synced (audit §3 says they already are)."

AUTHORITATIVE REFS (read-only — already synced; cite as the WHY):
  - spec/PACKAGING.md §6.4: "mise / asdf — NOT a channel (category mismatch)."
  - spec/PRD.md F15: "channel list excluding mise/asdf."
```

## Validation Loop

> All commands run from the repo root: `/home/dustin/projects/qmkonnect`.

### Level 1: The grep gates (the deterministic verification)

```bash
cd /home/dustin/projects/qmkonnect

# (a) No mise/asdf references remain.
grep -in 'mise\|asdf' docs/installation.md
# Expected: NO output (exit 1). Any hit = a block was missed or partially deleted.

# (b) No broken asdf-qmkonnect links remain.
grep -in 'asdf-qmkonnect' docs/installation.md
# Expected: NO output (exit 1).

# (c) Confirm the count dropped from 12 → 0.
grep -ic 'mise\|asdf' docs/installation.md
# Expected: 0
```

### Level 2: Diff is deletions-only

```bash
cd /home/dustin/projects/qmkonnect

git diff -- docs/installation.md
# Expected: ONE file; only `-` (red) lines; the only `+` lines, if any, are incidental whitespace — ideally
# pure deletion. The diff should show the 3 blocks removed (3 prose lines + a 7-line fence block + a 6-line
# fence block, plus their trailing blanks). No edits to the platform table, Nix, .deb, Homebrew, etc.

git diff --stat -- docs/installation.md
# Expected: "1 file changed, <N> deletions(-)" (no insertions).
```

### Level 3: Junction spacing + fence/heading balance (markdown sanity)

```bash
cd /home/dustin/projects/qmkonnect

# (a) The 3 neighboring junctions each have exactly ONE blank line (no double-blanks, no zero-blank joins).
#     Check there are no runs of 3+ consecutive blank lines anywhere in the file (a common deletion artifact).
awk 'BEGIN{n=0} /^$/{n++; if(n>=2) print NR": DOUBLE-BLANK"; next} {n=0}' docs/installation.md
# Expected: NO "DOUBLE-BLANK" output. (If any prints, a deletion left adjacent blank lines — collapse to one.)

# (b) The neighbors are present and in order.
grep -n '^## Windows\|^\*\*\.deb (Debian / Ubuntu)\*\*\|^### Launch at login' docs/installation.md
# Expected: 3 lines, in that order, each present exactly once.

# (c) Fence balance: ``` count is EVEN (every opener has a closer). The 2 deleted ```bash blocks removed
#     2 openers + 2 closers = net -4 fence lines, still balanced.
grep -c '```' docs/installation.md
# Expected: an EVEN number (was even before; removing 2 balanced fences keeps it even). If ODD, a fence
# was half-deleted — restore the block and delete the whole fence.

# (d) The platform overview table is intact (3 data rows: Windows / macOS / Linux).
sed -n '23,27p' docs/installation.md
# Expected: the header + separator + 3 platform rows, unchanged.
```

### Level 4: Scope preservation (no collateral edits)

```bash
cd /home/dustin/projects/qmkonnect

# Only docs/installation.md changed.
git status --short
# Expected: " M docs/installation.md" ONLY. NOT docs/llms_full.txt (that's S2), NOT spec/*, NOT README.md,
# NOT any src/ file.

# Spot-check the preserved sections still mention their channels (deletion didn't nick them).
grep -c 'Homebrew\|Scoop\|Winget\|\.deb\|\.rpm\|\bAUR\b\|Nix' docs/installation.md
# Expected: a non-zero count (the real channels remain). A sharp drop would indicate over-deletion.
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1 (a): `grep -in 'mise\|asdf' docs/installation.md` → zero hits.
- [ ] Level 1 (b): `grep -in 'asdf-qmkonnect' docs/installation.md` → zero hits.
- [ ] Level 2: `git diff --stat` → 1 file, deletions only (no insertions).
- [ ] Level 3 (a): no double-blank lines anywhere in the file.
- [ ] Level 3 (b): `## Windows`, `**.deb (Debian / Ubuntu)**`, `### Launch at login` all present, in order.
- [ ] Level 3 (c): ``` fence count is even.
- [ ] Level 3 (d): platform overview table (23–27) byte-identical to before.
- [ ] Level 4: `git status --short` → only `M docs/installation.md`.

### Feature Validation

- [ ] BLOCK 1 (intro paragraph) removed; table → `## Windows` separated by one blank.
- [ ] BLOCK 2 (Linux subsection + ```bash fence) removed; Nix → `.deb` separated by one blank.
- [ ] BLOCK 3 (macOS subsection + ```bash fence) removed; Homebrew → `### Launch at login` separated by one blank.
- [ ] All 4 broken `asdf-qmkonnect` links gone (3 in BLOCK 2, 1 in BLOCK 3).
- [ ] Nix / .deb / .rpm / Homebrew / Scoop / Winget / AUR sections untouched.

### Code Quality Validation

- [ ] Deletion follows the audit's verbatim block text exactly (no partial lines left).
- [ ] One-blank-line separator at each of the 3 junctions (markdown convention).
- [ ] No orphaned/dangling ``` fence markers.

### Documentation & Deployment

- [ ] Mode A: this edit IS the doc work (no code to document).
- [ ] `docs/llms_full.txt` is NOT regenerated here (P1.M1.T1.S2 runs the generator AFTER this edit).
- [ ] No spec file edited (PACKAGING.md/PRD.md already synced — read-only justification).

---

## Anti-Patterns to Avoid

- ❌ Don't match on line numbers alone — they may have drifted. Match the verbatim block text (the What section
  quotes it) as the deletion key.
- ❌ Don't delete a block's LEADING blank line and keep the trailing one "by feel" — follow the rule
  (delete content + TRAILING blank, keep PRECEDING blank) so each junction has exactly one blank deterministically.
- ❌ Don't leave zero blanks at a junction (heading sticks to the preceding paragraph) OR two blanks (extra
  markdown spacing). The Level-3 double-blank awk catches the latter.
- ❌ Don't half-delete a ```bash fence — remove the opener, all command lines, AND the closer together. A
  dangling ``` unbalances the doc (Level-3 fence-count check catches it).
- ❌ Don't touch the platform overview table (lines 23–27) — BLOCK 1 is the paragraph AFTER it, not the table.
- ❌ Don't edit Nix, .deb, .rpm, Homebrew, Scoop, Winget, or AUR sections — they are correct and out of scope.
- ❌ Don't regenerate `docs/llms_full.txt` here — that's S2, and it MUST run after this source fix (the audit §2
  maps its stale lines to these blocks).
- ❌ Don't run a regex like `sed 's/.*mise.*/x/'` blindly across the file — the 3 blocks are bounded and
  adjacent to content that must be preserved; use exact-match deletion per block.
- ❌ Don't edit any spec file (`spec/PACKAGING.md`, `spec/PRD.md`) — they are the read-only authoritative
  justification and are already synced.
- ❌ Don't claim success without running BOTH grep gates — they are the deterministic definition of "done".

---

**Confidence Score: 10/10** for one-pass execution success. This is three exact,
self-contained block deletions in one markdown file, with the verbatim text of each
block quoted, the precise neighbors and one-blank-line spacing rule given, the
"do-not-touch" list explicit, the downstream llms_full.txt regen correctly deferred
to S2, and two deterministic grep gates (zero `mise|asdf`, zero `asdf-qmkonnect`)
plus structural sanity checks (double-blank awk, fence-balance, neighbor presence)
that make verification unambiguous.