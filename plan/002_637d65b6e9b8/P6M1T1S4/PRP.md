# PRP — P6.M1.T1.S4: Update `README.md` + regenerate `docs/llms_full.txt`

> **Repo under change:** the **qmkonnect** desktop app at
> `/home/dustin/projects/qmkonnect`. This is a **documentation-only** task (item
> CONTRACT: "DOCS: This IS the documentation subtask"). It edits **TWO** files —
> the repo-root **`README.md`** (~312 lines) and **`docs/llms_full.txt`** (73,615
> bytes / 2,205 lines, a single-file LLM reference concatenation of all docs) — to
> surface the host-side window-rules feature (PRD F11/F12).
>
> **Files touched (2 mandatory + 1 optional helper):** `README.md`,
> `docs/llms_full.txt`, and (recommended) a new `docs/generate_llms_full.sh`
> generator script that produces the second file deterministically. **Nothing
> else** — no Rust, no Cargo, no spec, no other docs.
>
> **This is NOT a code task.** All code dependencies are **complete** (P1–P5 all
> green; the tray "Reload rules" item landed on **all three platforms** — see
> `src/tray.rs` L315-318/L389-391 for macOS/Windows and `src/linux_tray.rs`
> L190-199 for Linux SNI). Its inputs are: the item CONTRACT (below), the sibling
> doc PRPs (S1/S2 are committed; S3 is in flight — treat both as landed contracts
> whose on-disk output is the final state), and the ground-truth `llms_full.txt`
> format captured in `research/notes.md`.
>
> **This task is the top-level surface of the host-rules docs.** README.md is the
> repo's front door (GitHub-rendered); `docs/llms_full.txt` is the canonical
> single-file reference that agents/LLMs read. Both currently have **ZERO**
> host-rules content, and `llms_full.txt` is additionally **stale across all 8
> sections** (its embedded README predates the Inno installer, the
> `releases/latest` URLs, and the "not a service" wording). So this task does
> **two** things: (1) add a concise host-rules feature blurb + `rules.toml` mention
> + "Reload rules" + link to README; (2) **fully regenerate** `llms_full.txt` from
> the current source Markdown (which picks up README's new blurb AND the
> host-rules sections that S1/S2/S3 added to `docs/configuration.md`,
> `qmk-integration.md`, `examples.md`, and `troubleshooting.md`).

---

## Goal

**Feature Goal**: Make the host-side window-rules feature (F11/F12) discoverable
from the repo's two top-level entry points: (a) a visitor opening `README.md` on
GitHub sees a **"Host-Side Window Rules (no reflash)"** feature bullet that names
`rules.toml`, mentions the tray "Reload rules" item, and links into the docs;
(b) an agent/LLM reading `docs/llms_full.txt` gets an **up-to-date,
fully-regenerated** concatenation that contains the new README blurb **and** the
host-rules sections landed by S1–S3 (schema, migration, recipe, FAQ), with all 8
sections in sync with their source files (front matter stripped, exact format
preserved).

**Deliverable** (two files edited, one optional helper added):
1. **`README.md`** — a new `- **Host-Side Window Rules**:` bullet group inserted
   into the `## Features` section (between the existing `Configuration` group and
   `## Installation`). It states the "no reflash" promise, names `rules.toml`,
   notes the capability-gated fallback to string-only mode, mentions the tray
   **"Reload rules"** item (available on all platforms), and links to
   `docs/configuration.md` (schema/CLI) — and optionally `docs/qmk-integration.md`
   (firmware migration). Uses **GitHub-relative** link syntax (`docs/foo.md`),
   matching the file's existing `docs/installation.md#macos` convention.
2. **`docs/llms_full.txt`** — a **full regeneration** rebuilt from the current
   repo-root `README.md` + the 7 `docs/*.md` files, preserving the file's exact
   header block, the 8 section headers (order + labels), the 80-`=` dividers, and
   front-matter stripping. The regenerated §1 embeds the *updated* README (with
   the new blurb); the regenerated §4/§5/§7/§8 embed S1/S2/S3's host-rules
   content.
3. **(Recommended)** **`docs/generate_llms_full.sh`** — a small, committed,
   deterministic generator that emits `llms_full.txt` by concatenating the 8
   sources (stripping Jekyll front matter, emitting dividers/labels/header). Makes
   the regen reproducible + trivially verifiable (`bash docs/generate_llms_full.sh
   && git diff -- docs/llms_full.txt` must be empty after the first run).

**Success Definition**:
- `README.md` contains the literal phrase **"Host-side window rules (no
  reflash)"** (or close casing), mentions **`rules.toml`**, mentions the tray
  **"Reload rules"** item, and links to **`docs/configuration.md`** — all inside
  `## Features`.
- `docs/llms_full.txt` is regenerated so that, for every section N, the embedded
  body equals the current source file with its leading `---` front matter removed
  (README.md section has no front matter to strip). Verified by re-running the
  generator and observing an empty diff.
- `docs/llms_full.txt` retains: the verbatim header block (lines 1–16), exactly
  **8** section headers in the canonical order with the **exact** labels, and
  **16** divider lines (80 `=` each, 2 per section). `grep -c
  'permalink:\|layout: default\|^title:'` = **0** (front matter stripped).
- `docs/llms_full.txt` now contains host-rules content (`grep -c 'rules.toml'` >
  0, sourced from the embedded configuration/qmk-integration/examples/
  troubleshooting sections) and the updated README (no `build-installer.ps1`, no
  `v0.1.0` URLs, no "WiX MSI service").
- `git diff --name-only` = `README.md` + `docs/llms_full.txt` (+ optionally
  `docs/generate_llms_full.sh`). No other file changes.

## User Persona (if applicable)

**Target User**: (a) a **new visitor** browsing the repo on GitHub who scans
`README.md` to learn what QMKonnect does and whether rules can be changed without
reflashing; (b) an **AI agent / LLM** (or a power user) that reads
`docs/llms_full.txt` as the single-file canonical reference and must see the
host-rules feature documented alongside everything else.

**Use Case** (README): "I land on the GitHub repo, read the Features list, and
learn that QMKonnect supports **host-side window rules** — I can edit a
`rules.toml` file to change app→layer / app→callback behavior **without
reflashing my keyboard**, then click the tray's **Reload rules**. I click the
`docs/configuration.md` link for the schema."

**Use Case** (llms_full.txt): "I'm an agent ingesting the one-file docs bundle. I
expect it to be a faithful concatenation of the current docs — including the new
host-rules schema, migration guide, recipe, and FAQ — and to reflect the current
README, not a stale snapshot."

**Pain Points Addressed**: today (a) README's Features list is purely descriptive
of the string-only world — there is no hint that rules can move to the host; and
(b) `llms_full.txt` is a **stale snapshot** (its embedded README predates the Inno
installer and still describes a Windows *service*), so any agent reading it gets
wrong information about the current product — and zero information about host
rules.

## Why

- **Item CONTRACT point 3 (LOGIC)** names exactly what to add: README feature
  blurb + `rules.toml` mention + link to `docs/configuration.md`; and a
  **regeneration** of `llms_full.txt` to include the updated
  configuration/qmk-integration/examples/troubleshooting sections.
- **spec/HOST_RULES.md §1 "Deliverables"** confirms: "Docs: updates to
  `docs/qmk-integration.md`, `docs/configuration.md`, `docs/examples.md`,
  `docs/troubleshooting.md`, **`Readme.md`**, and a **regenerated
  `docs/llms_full.txt`**." S4 owns exactly the last two.
- **README is the front door.** It is the only doc a GitHub visitor is guaranteed
  to see; burying the flagship "edit rules without reflashing" capability only in
  `docs/` means most users never discover it. A one-line blurb + link fixes that.
- **`llms_full.txt` is the canonical agent reference** and it is *wrong* today
  (stale README, no host-rules content). Agents that read it produce inaccurate
  answers. A regeneration fixes both problems in one step.
- **Completes the P6 doc set:** S1 (schema/CLI), S2 (migration), S3 (recipe/FAQ),
  S4 (top-level surface + reference regen). Each links to the others; S4 is the
  last to land so its regen captures everyone's final output.

## What

**Two edits + one optional script.**

1. **`README.md`** — INSERT a new `- **Host-Side Window Rules**:` bullet group at
   the end of the `## Features` section (after the `Configuration` group, before
   `## Installation`). Additive only; no existing line is modified.
2. **`docs/llms_full.txt`** — **regenerate** (full rebuild from current sources;
   see Implementation Blueprint Task 4). Either via the recommended
   `docs/generate_llms_full.sh` or an equivalent manual concatenation following
   the exact format in research §1.3.
3. **(Recommended) `docs/generate_llms_full.sh`** — NEW committed generator script
   (the means by which #2 is produced and made reproducible).

### Success Criteria
- [ ] `README.md` `## Features` contains a new bullet group whose lead line
      includes **"Host-Side Window Rules"** and the phrase **"no reflash"** (or
      "without reflashing").
- [ ] The new README bullet **names `rules.toml`**, **mentions the tray "Reload
      rules" item** (available on all platforms — do NOT hedge with "where
      available"), and **links to `docs/configuration.md`** using GitHub-relative
      syntax (`](docs/configuration.md)`), matching the file's existing
      `docs/installation.md#macos` link style.
- [ ] The README blurb notes host rules are **capability-gated** (require
      `proto_ver == 2` firmware; legacy firmware falls back to string-only).
- [ ] `README.md` diff is **additive only** (the `## Features` insertion); no
      other README section is changed.
- [ ] `docs/llms_full.txt` is regenerated and its §1 body equals the **updated**
      `README.md` verbatim (incl. the new blurb); §2–§8 equal their current
      `docs/*.md` sources with front matter stripped.
- [ ] `docs/llms_full.txt` keeps the verbatim header block, the 8 section headers
      in canonical order with exact labels, and 16 dividers (80 `=` each).
- [ ] `grep -c 'permalink:\|layout: default\|^title:' docs/llms_full.txt` == 0
      (front matter stripped) and `grep -cE '^={70,}' docs/llms_full.txt` == 16.
- [ ] `grep -c 'rules.toml' docs/llms_full.txt` > 0 (host-rules content now
      present via the embedded configuration/qmk-integration/examples/
      troubleshooting sections).
- [ ] `grep -c 'build-installer.ps1\|releases/download/v0.1.0' docs/llms_full.txt`
      == 0 (stale README content gone).
- [ ] `git diff --name-only` == `README.md` + `docs/llms_full.txt` (+ optionally
      `docs/generate_llms_full.sh`).

## All Needed Context

### Context Completeness Check

_Pass_: an agent with no prior knowledge of this codebase can implement this
using only this PRP + `research/notes.md`, because: (a) the exact current
`## Features` section of README.md (verbatim, lines 16–32) + its link conventions
(GitHub-relative `docs/foo.md`) are in research §2.1/§2.2; (b) the verbatim item
CONTRACT + the spec §1 deliverables confirmation are in research §0/§0.1; (c) the
exact byte-level format of `llms_full.txt` (header block, 8 section order+labels,
80-`=` dividers, front-matter stripping — all re-verified) is in research §1.3/§1.4;
(d) the proof that the file is hand-maintained (no generator exists) AND stale
(full-regen justification) is in research §1.1/§1.2; (e) the recommended generator
script body is given verbatim in the Implementation Blueprint (Task 4); (f) the
current landed state of siblings (S1/S2 committed; S3 in flight; P5.M2 tray item
complete on all platforms) is in research §3; (g) design decisions D1–D5 + gotchas
G1–G5 are in research §4/§5; (h) the validation commands (verified working) are in
research §6. No code/build dependency.

### Documentation & References

```yaml
# MUST READ — THIS task's full contract (verbatim item CONTRACT, the llms_full
# format ground truth, README skeleton, sibling landed state, validation, the
# exact generator procedure). Everything needed is here + the two files edited.
- file: plan/002_637d65b6e9b8/P6M1T1S4/research/notes.md
  why: "§0 = verbatim item CONTRACT + spec §1 confirmation. §1 = llms_full.txt
        ground truth: (1.1) hand-maintained, no generator; (1.2) STALE (full
        regen justified, with a diff table); (1.3) EXACT byte format — header
        block verbatim, 8 section order+labels, 80-= dividers (all re-verified);
        (1.4) front-matter stripped (verified: 0 permalink/layout/title hits);
        (1.5) recommended generator. §2 = README ground truth: (2.1) verbatim
        Features section (L16-32); (2.2) GitHub-relative link syntax (NOT Liquid);
        (2.3) what NOT to do. §3 = sibling boundary + CURRENT landed state (S1/S2
        committed; S3 in flight; P5.M2 tray item COMPLETE on all platforms).
        §4/§5 = D1-D5 + G1-G5. §6 = validation."

# MUST READ — the file THIS task regenerates (study its EXACT format before
# rebuilding). Compare section-by-section against the current sources to confirm
# staleness and to reproduce the format byte-for-byte.
- file: docs/llms_full.txt
  why: "The deliverable #2. Header block (L1-16) is preserved verbatim. The 8
        section headers (lines 20/334/406/646/838/1179/1306/1585) define the
        canonical order + labels to reproduce. The 80-= dividers (16 of them)
        and the 'leading-space + N. path (Label)' header shape must be matched.
        Section bodies are source-file content with front matter stripped."
  pattern: "header block; then repeat( blank; 80x=; ' N. path (Label)'; 80x=;
            blank; <body-with-FM-stripped> )."
  gotcha: "the file is STALE (§1 embedded README predates Inno; references
           build-installer.ps1, v0.1.0 URLs, 'WiX MSI service'). Do NOT patch it
           in place — REBUILD from current sources. Front matter is already
           stripped in the existing file; keep that behavior."

# MUST READ — the file THIS task edits (deliverable #1).
- file: README.md
  why: "Repo-root README, GitHub-rendered. '## Features' (L16-32) has 3 bullet
        groups (Cross-Platform / Core Functionality / Configuration). INSERT a
        4th group '- **Host-Side Window Rules**:' after Configuration, before
        '## Installation' (L33). Internal links are GitHub-relative
        'docs/foo.md' (see L5 docs/llms_full.txt, ~L115 docs/installation.md#macos)
        — NOT Jekyll '{{ site.baseurl }}'. Line 5 self-links to llms_full.txt;
        leave it (regenerated §1 will still embed it correctly)."
  pattern: "'## Features' H2; '- **Group**:' bullets with indented sub-bullets;
            GitHub-relative doc links."
  gotcha: "do NOT edit any section other than the additive Features group. do NOT
           use Liquid '{{ site.baseurl }}' here (README is not Jekyll-rendered).
           do NOT paste the rules.toml schema/example (S1 owns it in
           docs/configuration.md) — blurb + link only."

# MUST READ — the spec (feature scope + the doc deliverables list).
- file: spec/HOST_RULES.md
  why: "§1 'Deliverables' confirms S4 owns README.md + regenerated llms_full.txt
        (and that the four docs/*.md belong to S1/S2/S3). §1 'Success definition'
        is the source for the README blurb's capability-gating + graceful-fallback
        wording (proto_ver 2; legacy = string-only). Use it to phrase the blurb
        accurately."
  section: "§1 (Goal & Deliverables, incl. the Docs bullet + Success definition)"

# REFERENCE — the sibling PRPs (treat as landed contracts; LINK to their pages,
# do not duplicate their content in README).
- file: plan/002_637d65b6e9b8/P6M1T1S1/PRP.md
  why: "S1's docs/configuration.md owns the rules.toml schema field table, the
        verbatim §9 example, the 3 CLI flags table, the per-OS file location
        (COMMITTED, commit aafe5b1; 44 host/rules.toml refs on disk). The README
        blurb LINKS to docs/configuration.md for all of that."
  section: "Goal + What (configuration.md section shape) + the link target slug"
- file: plan/002_637d65b6e9b8/P6M1T1S2/PRP.md
  why: "S2's docs/qmk-integration.md owns the host-rules migration guide
        (4-step procedure + DEFINE_HOST_CALLBACKS) (COMMITTED, commit 1b29dc0;
        13 refs on disk). The README blurb may OPTIONALLY link to
        docs/qmk-integration.md for the firmware-side change."
  section: "Goal + What (qmk-integration.md migration section)"
- file: plan/002_637d65b6e9b8/P6M1T1S3/PRP.md
  why: "S3's docs/examples.md (Example 4 recipe) + docs/troubleshooting.md (Host
        Rules Issues) are the LAST docs to land before S4 (currently IN FLIGHT —
        not yet on disk). S4's llms_full regen must run AFTER S3 finishes so it
        captures Example 4 + the Host Rules Issues section. Do NOT edit those two
        files."
  section: "Goal + What (examples.md Example 4; troubleshooting.md Host Rules Issues)"

# REFERENCE — the tray code that backs the README blurb's "Reload rules" claim
# (P5.M2 COMPLETE on all platforms). Read-only; confirms the menu item exists.
- file: src/tray.rs
  why: "Confirms the macOS/Windows tray 'Reload rules' MenuItem exists (L315-318
        builds it; L389-391 places it in the prefs group) and that a background
        reload thread + RulesReloadd event exist (L36-74). Backs the README blurb's
        unconditional 'Reload rules' mention (no 'where available' hedge)."
  section: "the reload_rules_i MenuItem + RulesReloaded(ReloadResult) event"
- file: src/linux_tray.rs
  why: "Confirms the Linux SNI tray 'Reload rules' item exists (L190-199) with a
        detached reload thread (do_reload_rules, L456-512). Completes the
        all-platforms picture."

# REFERENCE — docs that cite llms_full.txt (so the regenerated file's downstream
# consumers are understood; DO NOT edit these).
- file: docs/index.md
  why: "Links to llms_full.txt in 3 places (the docs home page). Confirms the
        regenerated file must keep the same path/name and remain a faithful
        concatenation. READ-ONLY context."
```

### Current Codebase tree (relevant subset)

```bash
README.md              # ← THIS TASK EDITS (deliverable #1): + "## Features" host-rules
                       #   bullet group (blurb + rules.toml + Reload rules + link to
                       #   docs/configuration.md). GitHub-rendered (NOT Jekyll).
docs/
  llms_full.txt        # ← THIS TASK REGENERATES (deliverable #2): full rebuild from
                       #   current sources. Hand-maintained, currently STALE, no generator.
  generate_llms_full.sh # ← (RECOMMENDED NEW) the deterministic generator for llms_full.txt.
  configuration.md     # S1 (rules.toml schema + CLI flags). COMMITTED. UNCHANGED (embedded by regen).
  qmk-integration.md   # S2 (host-rules migration). COMMITTED. UNCHANGED (embedded by regen).
  examples.md          # S3 (Example 4 recipe). IN FLIGHT. UNCHANGED by S4 (embedded by regen).
  troubleshooting.md   # S3 (Host Rules Issues). IN FLIGHT. UNCHANGED by S4 (embedded by regen).
  index.md             # docs home (links to llms_full.txt). UNCHANGED.
  installation.md      # UNCHANGED (embedded by regen).
  usage.md             # UNCHANGED (embedded by regen).
  _config.yml          # Jekyll: baseurl "/qmkonnect". UNCHANGED.
src/tray.rs            # macOS/Windows "Reload rules" tray item (P5.M2 COMPLETE). READ-ONLY.
src/linux_tray.rs      # Linux SNI "Reload rules" tray item (P5.M2 COMPLETE). READ-ONLY.
spec/HOST_RULES.md     # §1 deliverables + success definition. READ-ONLY.
```

### Desired Codebase tree with files to be changed

```bash
README.md                   # MODIFIED: + "## Features" → "- **Host-Side Window Rules**:"
                            #   bullet group (no-reflash promise + rules.toml mention +
                            #   Reload rules tray item + capability-gating note +
                            #   GitHub-relative link to docs/configuration.md [and
                            #   optionally docs/qmk-integration.md]). Additive only.
docs/llms_full.txt          # REGENERATED: full rebuild from current README.md + 7 docs/*.md.
                            #   Header block verbatim; 8 sections in canonical order+labels;
                            #   80-= dividers; front matter stripped. Picks up the new README
                            #   blurb (§1) AND S1/S2/S3 host-rules sections (§4/§5/§7/§8).
docs/generate_llms_full.sh  # NEW (recommended): deterministic generator that emits
                            #   docs/llms_full.txt. Committed for reproducibility.
# EVERYTHING ELSE UNCHANGED. No Rust, no Cargo.toml, no other docs, no spec.
```

### Known Gotchas of our codebase & Library Quirks

```markdown
<!-- CRITICAL (G1 — llms_full.txt is STALE, do not patch in place). Its §1 embeds
     an OLD README (build-installer.ps1, v0.1.0 URLs, "WiX MSI service"). The
     CONTRACT verb is "regenerate". REBUILD from current sources; do not
     surgically insert host-rules text into the stale file. -->

<!-- CRITICAL (G2 — front-matter strip must be LEADING-only). docs/*.md start with
     a Jekyll '---\nlayout...\n---' block; the existing llms_full STRIPS it
     (verified: 0 permalink/layout:/^title: hits). But many docs ALSO contain
     '---' as Markdown horizontal rules mid-body. A naïve 'grep -v ^---$' would
     delete those. Strip ONLY lines 1 → the second '---'. The PRP's awk one-liner
     does this correctly. README.md (§1) has NO front matter — embed it whole. -->

<!-- CRITICAL (G3 — README link syntax differs from docs/). README.md is rendered
     by GitHub directly, so internal links are RELATIVE FILE PATHS
     'docs/configuration.md' (see existing docs/installation.md#macos, docs/llms_full.txt).
     docs/*.md use Jekyll '{{ site.baseurl }}/slug'. Do NOT put
     '{{ site.baseurl }}' in README.md — it won't render on GitHub. -->

<!-- GOTCHA (G4 — preserve the file's stable contract). The header block (L1-16),
     the 8 section order+labels, the 80-'=' dividers, and the ' N. path (Label)'
     header shape are the file's contract that downstream readers rely on.
     Reproduce them EXACTLY. Section 1 (README.md) is the ONLY one with no
     parenthetical label and no front matter. -->

<!-- GOTCHA (G5 — self-reference is expected; S3 timing). README L5 links to
     docs/llms_full.txt, and llms_full §1 embeds README verbatim, so the
     regenerated §1 will contain that same self-link line. This is correct — do
     not "fix" it. AND: run the llms_full regen LAST, after confirming
     docs/examples.md + docs/troubleshooting.md are in their final S3 state
     (check git status), so the regen does not capture a half-written file. -->
```

## Implementation Blueprint

### Document structure (the deliverables)

The new README content + the regeneration procedure. Exact wording is in the
Implementation Tasks; this is the shape.

#### A. The new `## Features` bullet group in `README.md`

Inserted after the `- **Configuration**:` group (which ends with "Reloads
settings automatically") and before `## Installation`. Additive only.

```markdown
- **Host-Side Window Rules**:
  - **Change layers & callbacks without reflashing** — edit a `rules.toml` file
    on your computer, then click **Reload rules** in the tray/menu bar; no
    firmware rebuild needed
  - Host rules **stack on top of** your board's existing `DEFINE_SERIAL_*` rules
    (the board's rules run first, then host rules apply on top)
  - Requires firmware that advertises the typed-command capability
    (`proto_ver == 2`); legacy firmware keeps working in today's string-only mode
  - Full schema, CLI flags (`--list-callbacks`, `--validate-rules`), and
    per-OS file location: see the [Configuration Guide](docs/configuration.md)
    (firmware-side setup: [QMK Integration Guide](docs/qmk-integration.md))
```

> *(Phrasing is intentionally a blurb — link out to docs/configuration.md for the
> schema/CLI and docs/qmk-integration.md for the migration. Do NOT paste the
> rules.toml example here. "Reload rules" is available on Windows, macOS, and
> Linux — name it unconditionally, no "where available" hedge.)*

#### B. The `docs/llms_full.txt` regeneration (full rebuild)

The file is a concatenation with this exact shape (verified — see research §1.3):

```
<HEADER BLOCK — lines 1-16, verbatim>

<blank>
================================================================================
 1. README.md
================================================================================
<blank>
<repo README.md body — NO front matter to strip>

<blank>
================================================================================
 2. docs/index.md (Home)
================================================================================
<blank>
<docs/index.md body — leading Jekyll front matter (--- … ---) STRIPPED>

… repeat for sections 3–8, in this EXACT order with these EXACT labels:
  3. docs/installation.md (Installation)
  4. docs/qmk-integration.md (QMK Integration - REQUIRED firmware setup)
  5. docs/configuration.md (Desktop-side Configuration)
  6. docs/usage.md (Usage)
  7. docs/examples.md (Firmware examples)
  8. docs/troubleshooting.md (Troubleshooting)
```

- Divider = exactly **80 `=`** characters.
- Section header line = **single leading space** + `N. ` + path + ` (Label)`
  (section 1 has no `(Label)`).
- One blank line after the closing divider, then the body.
- Front matter = the leading `---\n…\n---\n` block of each docs/*.md; **strip it**
  (README.md has none). The PRP's `awk` strips **only** the leading block.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: READ the sources + research/notes.md
  - READ README.md in full; confirm the verbatim '## Features' section (L16-32)
    and the insertion point: after the '- **Configuration**:' group (ends
    'Reloads settings automatically', ~L31) and before '## Installation' (~L33).
    Confirm link style is GitHub-relative (L5 docs/llms_full.txt; ~L115
    docs/installation.md#macos) — NOT Liquid.
  - READ docs/llms_full.txt: the header block (L1-16), the 8 section headers
    (L20/334/406/646/838/1179/1306/1585) with their EXACT labels, and the 80-'='
    dividers. Spot-confirm staleness (§1 has 'build-installer.ps1' / 'v0.1.0').
  - READ plan/002_637d65b6e9b8/P6M1T1S4/research/notes.md §1 (format ground truth),
    §2 (README), §3 (sibling landed state), §4/§5 (decisions/gotchas).
  - READ spec/HOST_RULES.md §1 (Deliverables + Success definition) for the
    capability-gating / graceful-fallback wording to use in the README blurb.
  - CONFIRM S3 has landed its docs (git status: docs/examples.md and
    docs/troubleshooting.md final) BEFORE running the regen in Task 4 — the regen
    must capture S3's Example 4 + Host Rules Issues. GOTCHA G5.
  - NOTE: the tray "Reload rules" item is COMPLETE on all 3 platforms (src/tray.rs
    L315-318/L389-391; src/linux_tray.rs L190-199). The README blurb may name it
    unconditionally.

Task 2: EDIT README.md — INSERT the host-rules bullet group (blueprint block A)
  - INSERT the new '- **Host-Side Window Rules**:' group at the end of '## Features'
    (after the Configuration group, before '## Installation').
  - LEAD with the literal phrase 'Host-side window rules' and the 'no reflash' /
    'without reflashing' promise (Success Criterion).
  - NAME 'rules.toml' explicitly, and MENTION the tray 'Reload rules' item
    (unconditional — it ships on Windows/macOS/Linux).
  - STATE the stack-on-top model + the capability gate (proto_ver 2; legacy =
    string-only fallback) — phrased from spec §1 Success definition.
  - LINK to docs/configuration.md (GitHub-relative ']docs/configuration.md)'),
    and OPTIONALLY docs/qmk-integration.md for the firmware-side change.
  - KEEP every other README section VERBATIM (Overview, Installation, QMK Firmware
    Setup, Configuration, Usage, Technical Requirements, etc.). The ONLY change
    is the additive Features group. Do NOT touch L5 (the llms_full.txt self-link).
  - GOTCHA G3: GitHub-relative links here, NOT '{{ site.baseurl }}'.
  - VERIFY: grep -n 'Host-Side Window Rules\|no reflash\|rules\.toml' README.md;
    grep -n 'docs/configuration.md' README.md.

Task 3: (RECOMMENDED) CREATE docs/generate_llms_full.sh — the deterministic generator
  - CREATE a bash script at docs/generate_llms_full.sh (chmod +x) that emits
    docs/llms_full.txt by concatenating the 8 sources in canonical order, with the
    verbatim header block, the 80-'=' dividers, the ' N. path (Label)' headers,
    and front-matter stripping. Use the EXACT body below (copy verbatim):
    ----------------------------------------------------------------
    #!/usr/bin/env bash
    # Regenerate docs/llms_full.txt — a single-file concatenation of QMKonnect's
    # documentation, for agents/LLMs. Run after editing README.md or any docs/*.md:
    #   bash docs/generate_llms_full.sh && git diff --stat docs/llms_full.txt
    set -euo pipefail
    DOCS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    ROOT="$(cd "$DOCS_DIR/.." && pwd)"
    OUT="$DOCS_DIR/llms_full.txt"
    DIV="$(printf '%0.s=' $(seq 1 80))"
    # Strip a LEADING Jekyll front-matter block (--- ... ---) if line 1 is '---'.
    # Files without front matter (e.g. repo README.md) are passed through whole.
    strip_fm() {
      awk 'NR==1 && /^---[[:space:]]*$/ {fm=1; next} fm && /^---[[:space:]]*$/ {fm=0; next} fm {next} {print}' "$1"
    }
    emit() {  # <number-and-path>  [<label>]
      printf '\n%s\n %s%s\n%s\n\n' "$DIV" "$1" "${2:+ ($2)}" "$DIV"
    }
    {
      cat <<'HDR'
    # QMKonnect - Complete Documentation (for Agents / LLMs)

    This is a single-file concatenation of QMKonnect's documentation, generated from
    the source Markdown in this repository. It is the canonical reference for agents
    and LLMs.

    IMPORTANT REALITY CHECK
    -----------------------
    QMKonnect is only ONE HALF of a two-part system. It detects the active window
    and SENDS that information to your keyboard over Raw HID. Your keyboard cannot
    react to it unless the companion **qmk-notifier** module is built into your QMK
    firmware. That firmware setup is REQUIRED, not optional.

    When the docs say "no configuration" or "zero-config", they refer ONLY to the
    desktop app's vendor/product-ID selection (a single standard QMK keyboard is
    auto-discovered). The firmware configuration is mandatory.
    HDR
      emit "1. README.md";            strip_fm "$ROOT/README.md"
      emit "2. docs/index.md"            "Home"
      strip_fm "$DOCS_DIR/index.md"
      emit "3. docs/installation.md"     "Installation"
      strip_fm "$DOCS_DIR/installation.md"
      emit "4. docs/qmk-integration.md"  "QMK Integration - REQUIRED firmware setup"
      strip_fm "$DOCS_DIR/qmk-integration.md"
      emit "5. docs/configuration.md"    "Desktop-side Configuration"
      strip_fm "$DOCS_DIR/configuration.md"
      emit "6. docs/usage.md"            "Usage"
      strip_fm "$DOCS_DIR/usage.md"
      emit "7. docs/examples.md"         "Firmware examples"
      strip_fm "$DOCS_DIR/examples.md"
      emit "8. docs/troubleshooting.md"  "Troubleshooting"
      strip_fm "$DOCS_DIR/troubleshooting.md"
    } > "$OUT"
    echo "wrote $OUT ($(wc -l < "$OUT") lines, $(wc -c < "$OUT") bytes)"
    ----------------------------------------------------------------
  - NOTE: the heredoc body above is the EXACT current header block (research §1.3).
    Do not alter it. The emit() order+labels are the canonical 8 (research §1.3).
  - GOTCHA G2: strip_fm removes ONLY the leading '---…---' block (awk flips fm
    off at the second '---'); mid-body '---' horizontal rules are preserved.
  - (FALLBACK: if not creating the script, perform the identical concatenation
    manually — same header, same order/labels, same 80-'=' dividers, same
    front-matter strip — and write the result to docs/llms_full.txt.)

Task 4: RUN the regeneration + VERIFY it captured every source
  - RUN: bash docs/generate_llms_full.sh   (or the manual concatenation).
  - VERIFY the regenerated section bodies equal the current sources:
      # Simplest overall check: re-run the generator a second time; git diff on
      # llms_full.txt must then be EMPTY — proving determinism.
    (Per-section spot checks: for each docs/*.md, strip its front matter and diff
     against the embedded body; §1 README has no strip. All diffs empty.)
  - VERIFY the README change propagated: grep -n 'Host-Side Window Rules' docs/llms_full.txt
    (the regenerated §1 embeds the updated README).
  - VERIFY staleness is gone: grep -c 'build-installer.ps1\|releases/download/v0.1.0' docs/llms_full.txt  # == 0
  - VERIFY host-rules present: grep -c 'rules.toml' docs/llms_full.txt  # > 0
  - GOTCHA G5: confirm docs/examples.md + docs/troubleshooting.md are final (S3 done)
    before running — re-run the generator once more after any late S3 edit.

Task 5: VALIDATE (Markdown + concatenation invariants — NO cargo)
  - RUN the §6 validation commands from research/notes.md:
      git diff --name-only                         # README.md + docs/llms_full.txt (+ generate_llms_full.sh)
      grep -n 'Host-Side Window Rules\|no reflash\|rules\.toml' README.md
      grep -n 'docs/configuration.md' README.md
      grep -nE '^ [0-9]+\. ' docs/llms_full.txt     # exactly 8 headers, correct order+labels
      test "$(grep -cE '^={70,}' docs/llms_full.txt)" -eq 16 && echo "dividers OK" || echo "BAD"
      grep -c 'permalink:\|layout: default\|^title:' docs/llms_full.txt   # == 0 (FM stripped)
      grep -c 'rules.toml' docs/llms_full.txt                            # > 0
      grep -c 'build-installer.ps1\|releases/download/v0.1.0' docs/llms_full.txt  # == 0
      test $(( $(grep -c '^```' README.md) % 2 )) -eq 0 && echo "README fences OK" || echo UNBALANCED
  - EYEBALL: the README Features group reads cleanly on GitHub (relative link
    resolves to docs/configuration.md); llms_full.txt §1 shows the NEW README
    (Inno installer, 'releases/latest', 'not a service') + the host-rules blurb.
```

### Implementation Patterns & Key Details

```markdown
<!-- PATTERN: README '## Features' uses '- **Group**:' lead bullets with indented
     sub-bullets. The new host-rules group matches that shape exactly. See the
     existing Cross-Platform / Core Functionality / Configuration groups. -->

<!-- PATTERN: README internal links are GitHub-relative 'docs/foo.md' (L5, ~L115).
     The docs/*.md pages use Jekyll '{{ site.baseurl }}/slug'. Never mix: README
     = relative paths; docs/ = Liquid. -->

<!-- KEY DETAIL: llms_full.txt is a concatenation with a STABLE shape — header
     block (verbatim), 8 sections (canonical order+labels), 80-'=' dividers (16
     total), front-matter-stripped bodies. Reproduce it EXACTLY; the generator
     script encodes this so it is reproducible and re-verifiable. -->

<!-- KEY DETAIL: the README blurb is capability-honest. From spec §1 Success
     definition: host rules need proto_ver==2 firmware; legacy firmware keeps
     working in string-only mode (no host commands sent). Say so — don't imply
     host rules work with the current public firmware release unconditionally. -->

<!-- KEY DETAIL: the tray 'Reload rules' item is COMPLETE on all three platforms
     (src/tray.rs macOS/Windows; src/linux_tray.rs Linux). Name it in the README
     blurb unconditionally — do NOT hedge with "where available". -->
```

### Integration Points

```yaml
README.md (deliverable #1):
  - INSERT a new '- **Host-Side Window Rules**:' group at the end of '## Features'
    (after Configuration, before Installation). Additive only; no other section
    touched. GitHub-relative links to docs/configuration.md (+ optional
    docs/qmk-integration.md).

docs/llms_full.txt (deliverable #2):
  - FULL REGENERATION from current README.md + 7 docs/*.md. Preserves: header
    block (verbatim), 8 section order+labels, 80-'=' dividers, front-matter
    stripping. Picks up the new README blurb (§1) and S1/S2/S3 host-rules content
    (§4 qmk-integration / §5 configuration / §7 examples / §8 troubleshooting).

docs/generate_llms_full.sh (recommended NEW):
  - Committed generator that emits docs/llms_full.txt. Makes future doc-sync a
    one-command re-run. Not required by the CONTRACT but strongly recommended for
    reproducibility + verifiability.

DOWNSTREAM CONSUMERS (read-only; confirm they still resolve):
  - README.md L5: '[Complete documentation](docs/llms_full.txt)' — still resolves
    (the regenerated file keeps the same path).
  - docs/index.md (3 links to llms_full.txt), docs/installation.md — still resolve
    (same path/name, faithful concatenation).

ORDERING: S4 is the LAST doc subtask. Run the llms_full regen AFTER S3's
  docs/examples.md + docs/troubleshooting.md are final, so the concatenation
  captures Example 4 + the Host Rules Issues section.

BUILD: none. README is GitHub-rendered; docs/*.md are Jekyll-rendered on GitHub
  Pages (docs/_config.yml). llms_full.txt is plain text consumed directly.
```

## Validation Loop

### Level 1: Markdown + scope (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect

# Scope gate: ONLY the expected files changed.
git diff --name-only
# EXPECT: README.md + docs/llms_full.txt (+ optionally docs/generate_llms_full.sh).

# README: the host-rules blurb landed inside ## Features.
grep -n 'Host-Side Window Rules' README.md
grep -ni 'no reflash\|without reflashing' README.md
grep -n 'rules\.toml' README.md
grep -ni 'Reload rules' README.md                      # tray item named (all platforms)
grep -n '](docs/configuration.md)' README.md          # GitHub-relative link

# README: no Liquid syntax leaked in (GitHub wouldn't render it).
grep -c '{{ site.baseurl }}' README.md               # EXPECT 0

# README: code fences still balanced (the edit is additive prose, but check).
test $(( $(grep -c '^```' README.md) % 2 )) -eq 0 && echo "README fences OK" || echo "README UNBALANCED"
```

### Level 2: llms_full.txt invariants (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect

# Structure: exactly 8 section headers, canonical order + labels.
grep -nE '^ [0-9]+\. ' docs/llms_full.txt
# EXPECT (in order):
#   1. README.md
#   2. docs/index.md (Home)
#   3. docs/installation.md (Installation)
#   4. docs/qmk-integration.md (QMK Integration - REQUIRED firmware setup)
#   5. docs/configuration.md (Desktop-side Configuration)
#   6. docs/usage.md (Usage)
#   7. docs/examples.md (Firmware examples)
#   8. docs/troubleshooting.md (Troubleshooting)

# Dividers: 16 (8 sections × 2), each exactly 80 '='.
test "$(grep -cE '^={70,}' docs/llms_full.txt)" -eq 16 && echo "dividers OK (8x2)" || echo "BAD divider count"
awk '/^={70,}$/{if(length($0)!=80) print "BAD width at "NR": "length($0)}' docs/llms_full.txt  # no output = all 80

# Front matter stripped (no Jekyll YAML leaked into the concatenation).
grep -c 'permalink:\|layout: default\|^title:' docs/llms_full.txt   # EXPECT 0

# Header block preserved verbatim.
sed -n '1,16p' docs/llms_full.txt | head -3          # '# QMKonnect - Complete Documentation (for Agents / LLMs)'

# Staleness gone + host-rules present.
grep -c 'build-installer.ps1\|releases/download/v0.1.0' docs/llms_full.txt   # EXPECT 0
grep -c 'rules.toml' docs/llms_full.txt                                     # EXPECT > 0
grep -n 'Host-Side Window Rules' docs/llms_full.txt                         # the regenerated §1 README blurb
```

### Level 3: Body-fidelity cross-check (System Validation)

```bash
cd /home/dustin/projects/qmkonnect

# Strongest check: if the generator script was used, re-run it and confirm a
# clean diff (determinism). The output must be byte-identical to itself.
cp docs/llms_full.txt /tmp/llms_before.txt
bash docs/generate_llms_full.sh
diff /tmp/llms_before.txt docs/llms_full.txt && echo "DETERMINISTIC — regen matches itself" || echo "NON-DETERMINISTIC — investigate"

# Per-section fidelity: each embedded body == its source (front-matter stripped).
# Helper (front-matter strip identical to the generator's strip_fm):
strip_fm() { awk 'NR==1&&/^---[[:space:]]*$/{f=1;next}f&&/^---[[:space:]]*$/{f=0;next}f{next}{print}' "$1"; }
body_of() { awk -v n="$1" '$0~"^ "n"\\. "{f=1;next} f&&/^={70,}/{f=0} f' docs/llms_full.txt | sed '1{/^=/d}'; }

# Section 1 (README, no FM): bodies must match.
diff <(cat README.md) <(body_of 1) && echo "§1 README OK" || echo "§1 MISMATCH"
# Section 5 (configuration, FM stripped):
diff <(strip_fm docs/configuration.md) <(body_of 5) && echo "§5 configuration OK" || echo "§5 MISMATCH"
# Section 7 (examples, FM stripped) — captures S3's Example 4:
diff <(strip_fm docs/examples.md) <(body_of 7) && echo "§7 examples OK" || echo "§7 MISMATCH"
# Section 8 (troubleshooting, FM stripped) — captures S3's Host Rules Issues:
diff <(strip_fm docs/troubleshooting.md) <(body_of 8) && echo "§8 troubleshooting OK" || echo "§8 MISMATCH"
```

### Level 4: Render + downstream check (Domain-Specific)

```bash
cd /home/dustin/projects/qmkonnect

# README renders on GitHub: eyeball that the new Features group is well-formed
# and the docs/configuration.md link points at the right file.
grep -n -A6 'Host-Side Window Rules' README.md

# Downstream links still resolve (read-only confirm — do NOT edit these files).
grep -n 'llms_full.txt' README.md docs/index.md docs/installation.md   # all still point at docs/llms_full.txt
test -f docs/llms_full.txt && echo "llms_full.txt present" || echo "MISSING"

# (Optional) If ruby + bundler are present, build the Jekyll site to confirm the
# docs pages (which embed no change here) still render and their llms_full links
# resolve. Not required — GitHub Pages renders on push.
( cd docs && bundle exec jekyll build 2>&1 | grep -iE 'error' ) || echo "jekyll clean (or unavailable)"
```

## Final Validation Checklist

### Technical Validation
- [ ] All 4 validation levels completed successfully.
- [ ] `git diff --name-only` == `README.md` + `docs/llms_full.txt` (+ optional
      `docs/generate_llms_full.sh`); nothing else changed.
- [ ] `grep -c 'permalink:\|layout: default\|^title:' docs/llms_full.txt` == 0.
- [ ] `grep -cE '^={70,}' docs/llms_full.txt` == 16 (and each divider == 80 `=`).
- [ ] `grep -nE '^ [0-9]+\. ' docs/llms_full.txt` shows exactly the 8 canonical
      section headers in order with exact labels.
- [ ] Regeneration is deterministic (re-run → empty diff) OR each embedded body
      diffs clean against its front-matter-stripped source.

### Feature Validation
- [ ] README `## Features` contains "Host-side window rules" + "no reflash" +
      `rules.toml` + the "Reload rules" tray item + a GitHub-relative link to
      `docs/configuration.md`.
- [ ] README blurb is capability-honest (proto_ver 2 / legacy string-only).
- [ ] `docs/llms_full.txt` now contains `rules.toml` (>0) and the updated README
      (no `build-installer.ps1` / `v0.1.0` / "WiX MSI service").
- [ ] README diff is additive only (no existing section modified).

### Code Quality Validation
- [ ] Follows the existing `## Features` bullet-group pattern in README.md.
- [ ] `docs/llms_full.txt` reproduces the file's stable format exactly (header
      block, order, labels, dividers, front-matter stripping).
- [ ] No sibling content duplicated in README (blurb + link only — schema is in
      docs/configuration.md/S1, migration in docs/qmk-integration.md/S2).
- [ ] README uses GitHub-relative links (no Liquid); docs/*.md conventions
      untouched.

### Documentation & Deployment
- [ ] The regenerated `llms_full.txt` is self-consistent (header says
      "generated from the source Markdown" and it is).
- [ ] If `docs/generate_llms_full.sh` was added, it is executable and its header
      comment documents how to re-run it.
- [ ] No environment variables or config introduced (pure docs).

---

## Anti-Patterns to Avoid

- ❌ Don't **patch `llms_full.txt` in place** — it is stale across all sections;
  REGENERATE from current sources (CONTRACT verb: "regenerate").
- ❌ Don't **strip all `---` lines** when regenerating — that deletes legit
  Markdown horizontal rules. Strip ONLY the leading Jekyll front-matter block.
- ❌ Don't **use Liquid `{{ site.baseurl }}` in README.md** — README is
  GitHub-rendered; use relative `docs/foo.md` paths (see L5/~L115).
- ❌ Don't **paste the rules.toml schema/example into README** — S1 owns it in
  `docs/configuration.md`; README gets a blurb + link.
- ❌ Don't **edit any docs/*.md source file** — S1/S2/S3 own those; S4 only
  consumes their final state via the regeneration.
- ❌ Don't **run the regen before S3 lands** its `docs/examples.md` +
  `docs/troubleshooting.md` — the concatenation would capture a half-written
  file. Re-run once more after any late S3 edit.
- ❌ Don't **hedge the tray "Reload rules" item** ("where available") — it is
  COMPLETE on Windows/macOS/Linux; name it unconditionally.
- ❌ Don't **alter the `llms_full.txt` header block, section order, labels, or
  divider format** — they are the file's stable contract.