# Research Notes — P6.M1.T1.S4 (Update README.md + regenerate docs/llms_full.txt)

> Documentation-only task. Two files: repo-root `README.md` and
> `docs/llms_full.txt` (73,615 bytes / 2,205 lines). No Rust, no Cargo, no spec.
> (Re-created after a session reset wiped the prior artifacts; ground-truth
> re-verified against the current tree on the date below.)

---

## §0 — Item CONTRACT (verbatim, from the work-item description)

1. **RESEARCH NOTE:** README.md (project root) is the top-level project overview.
   `docs/llms_full.txt` (73.6 KB) is a concatenated LLM reference of all docs.
   Both need to surface the host-rules feature.
2. **INPUT:** All doc updates from P6.M1.T1.S1-S3, feature completion from P5.M2.
3. **LOGIC:**
   - In `README.md`: add a feature blurb for host-side window rules (F11/F12),
     mention `rules.toml`, link to `docs/configuration.md`. Update the feature
     list to include **"Host-side window rules (no reflash)"**.
   - In `docs/llms_full.txt`: **regenerate** the concatenation to include the
     updated `docs/configuration.md`, `qmk-integration.md`, `examples.md`, and
     `troubleshooting.md` sections.
4. **OUTPUT:** Updated `README.md` and `docs/llms_full.txt`.
5. **DOCS:** This IS the documentation subtask.

### §0.1 — spec confirmation (HOST_RULES.md §1 "Deliverables")

> "Docs: updates to `docs/qmk-integration.md`, `docs/configuration.md`,
> `docs/examples.md`, `docs/troubleshooting.md`, **`Readme.md`**, and a
> **regenerated `docs/llms_full.txt`**, plus the migration subsection (§10)."

→ S4 owns **only** README.md + llms_full.txt. The four docs/*.md edits are owned
by S1/S2/S3 (treat as landed contracts; S4 consumes their final on-disk state).

---

## §1 — `docs/llms_full.txt`: ground-truth format (deliverable #2)

### §1.1 — It is HAND-MAINTAINED (no generator script exists)

- `grep -rn llms_full` across the repo (excl. target/_site/.git) finds **NO
  `.sh/.py/.rs/.ps1` generator** — only references *to* the file. No `scripts/`
  or `tools/` dir exists.
- `git log --oneline -- docs/llms_full.txt` = exactly **2 commits**, both manual
  (`5c0b37f updated docs & added llms_full.txt`, `d458f86 restructure
  documentation for clarity and accuracy`).
- It IS tracked in git (`git ls-files docs/llms_full.txt` → listed); NOT in
  `docs/.gitignore` (that ignores only `_site/`, `.jekyll-cache/`, `.bundle/`,
  `vendor/`).

→ "Regenerate" means: **rebuild the concatenation from the current source
Markdown** (there is no tool to run; either hand-rebuild or write a generator).

### §1.2 — It is STALE (this is the core reason a full regen is needed)

Re-verified this session: `grep -c 'build-installer.ps1\|releases/download/v0.1.0'
docs/llms_full.txt` = **4** (stale markers present); `grep -ci
'rules.toml\|host-side\|no reflash'` = **0** (zero host-rules content). Its §1
embeds an **OLD** README:

| aspect | stale `llms_full.txt` §1 | current `README.md` |
|---|---|---|
| Windows build cmd | `…/packaging/windows && ./build-installer.ps1` | `cargo build --release` + `packaging/windows/inno/build.ps1` (Inno) |
| release URLs | `…/releases/download/v0.1.0/…` | `…/releases/latest` |
| Windows pkg model | "A Session-0 *service* build exists via the WiX MSI" | "It is **not** a Windows service" (Inno tray app) |
| `qmk_notifier` desc | "Desktop application that sends commands" | "The Rust transport library that QMKonnect links to" |

→ A **full regeneration** is required (the CONTRACT verb is "regenerate"); do NOT
surgically patch host-rules text into the stale file.

### §1.3 — Exact byte-level format (re-verified this session)

**Header block (lines 1–16)** — verbatim, preserved:

```
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
```

(Then 2 blank lines, then the first divider.)

**Each section** = exactly:

```
<blank>
================================================================================
 N. <path> (<Label>)
================================================================================
<blank>
<file body — Jekyll front matter STRIPPED>
```

- Divider line = **exactly 80 `=` chars** (verified: `awk '/^={70,}$/{if(length($0)!=80)print "BAD:"NR}'`
  → no BAD lines; all 16 dividers are 80).
- Header line has a **single leading space**: ` 1. README.md`.
- `grep -cE '^={70,}'` = **16** = 8 sections × 2 dividers each.
- After the closing divider there is **one blank line**, then the file body.

**Section order + EXACT labels** (re-verified line numbers this session):

| # | line | path | label (parenthetical) |
|---|------|------|------------------------|
| 1 | 20 | `README.md` | *(none)* |
| 2 | 334 | `docs/index.md` | `Home` |
| 3 | 406 | `docs/installation.md` | `Installation` |
| 4 | 646 | `docs/qmk-integration.md` | `QMK Integration - REQUIRED firmware setup` |
| 5 | 838 | `docs/configuration.md` | `Desktop-side Configuration` |
| 6 | 1179 | `docs/usage.md` | `Usage` |
| 7 | 1306 | `docs/examples.md` | `Firmware examples` |
| 8 | 1585 | `docs/troubleshooting.md` | `Troubleshooting` |

### §1.4 — Front-matter handling (re-verified)

- All 7 `docs/*.md` begin with Jekyll front matter (`head -1` = `---` for each).
- The repo-root `README.md` has **NO** front matter (starts `# QMKonnect`).
- In the current `llms_full.txt`: `grep -c 'permalink:\|layout: default\|^title:'`
  = **0** → front matter is **STRIPPED** from each docs/*.md before embedding.
- **Gotcha:** many docs legitimately contain `---` as Markdown horizontal rules,
  so a strip routine must only remove the **leading** `---…---` block (lines 1
  through the second `---`), not all `---` lines. The PRP's `awk` does this.

### §1.5 — Recommended regeneration method: a small generator script

Because the file is hand-maintained and 8 files must be concatenated with
front-matter stripping, the safest one-pass path is a **deterministic generator
script** (`docs/generate_llms_full.sh`) that the implementer writes, runs, and
commits alongside the regenerated output. Benefits: reproducible, re-runnable
(next doc edit = re-run + commit), trivially verifiable (re-run → `git diff`
empty). The exact script body is specified in the PRP's Implementation Blueprint
(Task 4). A pure-manual rebuild is the fallback (same procedure, hand-assembled).

---

## §2 — `README.md`: ground-truth (deliverable #1)

### §2.1 — Current `## Features` section (re-verified; lines 16–32)

```
## Features

- **Cross-Platform Support**:
  - Windows
  - macOS
  - Linux: Arch/Hyprland only

- **Core Functionality**:
  - Detects window changes in real-time
  - Sends app name and window title to your QMK keyboard
  - Low resource usage
  - Debug logging when you need it

- **Configuration**:
  - Easy to configure
  - Reloads settings automatically

## Installation
```

→ Insert a **new fourth bullet group** `- **Host-Side Window Rules**:` between the
`Configuration` group (ends line 31) and `## Installation` (line 33). One lead
bullet + sub-bullets naming `rules.toml`, the "no reflash" promise, the
"Reload rules" tray item, and a link to `docs/configuration.md`. (Adding it as
its own group matches the CONTRACT's "update the feature list to include
'Host-side window rules (no reflash)'".)

### §2.2 — Link conventions in README.md (GitHub rendering, NOT Jekyll)

README.md is rendered by **GitHub directly** (repo root), so internal links are
**relative file paths**, NOT `{{ site.baseurl }}`:

- existing: `[macOS install guide](docs/installation.md#macos)` (line ~115);
  `[Complete documentation](docs/llms_full.txt)` (line 5).
- → host-rules link must be `docs/configuration.md` (with `.md`), optionally
  also `docs/qmk-integration.md`. **Do NOT** use `{{ site.baseurl }}/configuration`
  (that Liquid syntax only works inside the Jekyll-rendered docs/*.md pages).

### §2.3 — What NOT to do in README.md

- Do **not** paste the `rules.toml` schema / example (S1 owns it in
  `docs/configuration.md`); README gets a **blurb + link** only.
- Do **not** reproduce the migration procedure (S2 owns it in
  `docs/qmk-integration.md`); link out.
- Do **not** touch the Overview / Installation / QMK Firmware Setup / Usage /
  Technical Requirements sections (they are accurate). The only edit is the
  additive `## Features` group.
- Do **not** change the self-referential line 5 (`[Complete documentation]
  (docs/llms_full.txt)`) — after regen it still resolves correctly.

---

## §3 — Boundary with sibling tasks + current landed state (verified this session)

| Sibling | Owns | Status / on-disk state | S4's relationship |
|---|---|---|---|
| **S1** | `docs/configuration.md` (rules.toml schema, CLI flags table, §9 example, file location) | **Complete + COMMITTED** (commit `aafe5b1 "Document rules schema…"`; 44 host/rules.toml refs on disk) | README + llms_full LINK to it. Do not duplicate schema. |
| **S2** | `docs/qmk-integration.md` (4-step migration, `DEFINE_HOST_CALLBACKS`) | **Complete + COMMITTED** (commit `1b29dc0 "Add host-rules migration guide…"`; 13 refs on disk) | README + llms_full LINK to it. Do not duplicate procedure. |
| **S3** | `docs/examples.md` (Example 4 recipe) + `docs/troubleshooting.md` (Host Rules Issues) | **Implementing** — NOT yet on disk (examples.md has 0 host-rules content; both files clean in `git status`) | S4 regenerates llms_full **after** S3 lands. Do not edit those two files. |
| **P5.M2** | Tray "Reload rules" menu item | **Complete on ALL THREE platforms** (macOS/Windows via `src/tray.rs` L315-318/L389-391; Linux via `src/linux_tray.rs` L190-199) — commits `1aa6efa`, `bc44060` | README blurb MAY reference "Reload rules" tray item unconditionally (no "where available" hedge). |
| **P4** | Pipeline integration + handshake | **Complete** | No code blockers for docs. |

**Ordering note:** S4 is the LAST doc subtask. Its `llms_full.txt` regeneration
must run against the FINAL on-disk state of all `docs/*.md` (i.e. **after S3
finishes**). If S3 is still editing when S4 starts, S4's regen step should be the
very last action so it captures S3's output. Re-run the generator once more after
any late S3 edit.

---

## §4 — Design decisions (D1–D5)

- **D1 — Full regeneration, not a patch.** `llms_full.txt` is stale across ALL
  sections (not just missing host-rules). The CONTRACT verb is "regenerate".
  Rebuild from current sources; do not surgically insert host-rules text.
- **D2 — Preserve the exact section order, labels, divider width (80 `=`), and
  header block verbatim.** These are the file's stable contract; downstream
  consumers (agents, `docs/index.md` link, README line 5) rely on them.
- **D3 — Strip front matter, embed bodies.** Matches the existing file's
  behavior (verified: 0 `permalink:`/`layout:`/`^title:` hits). The README body
  (section 1) has no front matter.
- **D4 — Generator script is the primary method.** `docs/generate_llms_full.sh`
  (committed) makes the regen deterministic + re-verifiable. The PRP gives the
  exact script body. Manual rebuild = identical procedure, acceptable fallback.
- **D5 — README blurb is additive + minimal.** One new `## Features` bullet group
  (+ 2–3 sub-bullets); link out to `docs/configuration.md` (+ optionally
  `docs/qmk-integration.md`). Use GitHub relative-path links, not Liquid.

## §5 — Gotchas (G1–G5)

- **G1 (stale file) — don't be fooled by partial freshness.** Section 1 looks
  plausibly like a README but is an OLD one. The CONTRACT verb is "regenerate" —
  always rebuild from current sources.
- **G2 (front-matter strip must be leading-only).** A naïve `grep -v '^---$'`
  would also delete legit `---` horizontal rules inside doc bodies. Strip only
  lines 1 → the second `---`. (PRP's `awk` one-liner does this correctly.)
- **G3 (README link syntax differs from docs/).** README.md = GitHub-relative
  `docs/foo.md`; docs/*.md = Jekyll `{{ site.baseurl }}/foo`. Don't mix them.
- **G4 (self-reference is fine).** README line 5 links to `docs/llms_full.txt`,
  and llms_full §1 embeds README verbatim → the regenerated §1 will contain that
  same line. This is expected and correct (it's how the file has always been).
- **G5 (S3 may still be editing).** Run the llms_full regen LAST, after
  confirming `docs/examples.md` + `docs/troubleshooting.md` are in their final
  state (check `git status`). Otherwise the regen captures a half-written file.

> (Note: a prior version of these notes had a gotcha "don't over-promise the tray
> Reload rules item — Linux-only". That is **obsolete**: P5.M2.T1.S1 landed the
> macOS/Windows item too, so "Reload rules" is available on all three platforms.
> The README blurb may name it unconditionally.)

## §6 — Validation commands (verified working)

```bash
cd /home/dustin/projects/qmkonnect

# README host-rules blurb landed.
grep -n 'Host-Side Window Rules\|no reflash\|rules\.toml' README.md
grep -n 'docs/configuration.md' README.md          # the link (GitHub-relative)
grep -c '{{ site.baseurl }}' README.md             # EXPECT 0 (no Liquid in README)

# llms_full regenerated + host-rules now present.
grep -c 'rules.toml' docs/llms_full.txt            # > 0 (from configuration/qmk-integration/examples/troubleshooting sections)
grep -nE '^ [0-9]+\. ' docs/llms_full.txt          # exactly 8 section headers, correct order + labels
test "$(grep -cE '^={70,}' docs/llms_full.txt)" -eq 16 && echo "dividers OK (8×2)" || echo "BAD dividers"
grep -c 'permalink:\|layout: default\|^title:' docs/llms_full.txt   # 0 — front matter stripped
grep -c 'build-installer.ps1\|releases/download/v0.1.0' docs/llms_full.txt   # 0 — staleness gone

# Scope gate.
git diff --name-only                              # README.md + docs/llms_full.txt (+ optionally docs/generate_llms_full.sh)
```