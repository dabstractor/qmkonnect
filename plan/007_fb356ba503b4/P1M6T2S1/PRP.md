# PRP — P1.M6.T2.S1: Update README.md with community distribution channels

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`, org `dabstractor`). **ONE file edited:**
> `README.md` (the repo-root front page, GitHub-rendered — **not** a Jekyll page, **no**
> front-matter). **Zero Rust, zero Cargo.toml, zero .yml, zero other docs.** This is the F15
> front-page documentation task.
>
> **What this does:** adds a concise **`### Package Managers`** subsection inside the existing
> `## Installation` section, listing all seven F15 community package-manager channels
> (AUR, Nix, Homebrew, Scoop, Winget, mise, asdf) as a single scannable table + a caveat blockquote,
> and linking to `docs/installation.md` for the full per-channel detail. The direct-installer
> instructions already in the README stay untouched; this presents the community channels as a
> complementary, power-user alternative.
>
> **Scope boundary (siblings — do NOT touch):** P1.M6.T1.S1 owns `docs/installation.md` (the
> *detailed* per-channel version — a top table + per-platform `### Package Managers` subsections
> with full caveats; it is being implemented in parallel). P1.M6.T2.S2 owns `docs/llms_full.txt` +
> `spec/PACKAGING.md` (it regenerates llms_full from the docs/*.md sources — NOT from README, so no
> dependency). **This task edits ONLY `README.md`.** The README link to `docs/installation.md` is
> valid whether or not T1.S1 has landed (the file already exists).
>
> **Source of truth for this design:** `research/readme_research.md` (the verbatim install matrix,
> the exact package IDs, the 3 mandatory caveats, the verified single insertion anchor, and the
> scope-boundary table).

---

## Goal

**Feature Goal**: Make the repo-root `README.md` advertise, at a glance, **every** way to install
QMKonnect — the direct installers (already documented per-platform) **plus** the seven F15 community
package-manager channels — so a user who prefers their native package/version manager (`brew`,
`scoop`, `winget`, `yay`, `nix`, `asdf`/`mise`) sees their one-line install command on the front
page and clicks through to `docs/installation.md` for caveats and full setup.

**Deliverable** (exactly ONE file edited, ONE insertion inside it):
- **ADD** a `### Package Managers` subsection between the existing `### macOS` and `### From Source`
  subsections, containing (a) a 3-column table (Platform | Channel | Install command) with all 7
  channels, and (b) a blockquote with the 3 mandatory caveats (mise/asdf macOS CLI-only, mise/asdf
  not on Windows, unsigned DMG/installer → Gatekeeper/SmartScreen prompt). Links to
  `docs/installation.md` for the full detail.

**Success Definition**:
- `git diff --stat` shows **ONLY** `README.md` (1 file). Nothing else.
- A new `### Package Managers` heading exists exactly once, located between `### macOS` and
  `### From Source`.
- All seven channel commands are present — `grep -cF` for each of `yay -S qmkonnect-bin`,
  `brew install --cask qmkonnect`, `scoop install qmkonnect`, `winget install dabstractor.QMKonnect`,
  `nix run github:dabstractor/qmkonnect`, `mise install qmkonnect@latest`,
  `asdf install qmkonnect latest` returns ≥1.
- The 3 caveats are present — `grep -ciE` for `cli only` (or `CLI only`), `not available on windows`
  (or `not.*windows`), and `unverified` (or `unsigned`) each returns ≥1.
- No existing heading is renumbered/reordered/moved; no Jekyll/Liquid tags introduced (the README
  is plain GitHub markdown — `{{ site.baseurl }}` is FORBIDDEN here); code fences stay balanced
  (`grep -c '^```' README.md` remains even — the new block adds **zero** fences).

## User Persona (if applicable)

**Target User**: an end user (or sysadmin / developer) who lands on the GitHub repo and already
uses a platform package manager — they want the *native* one-liner (`brew`, `scoop`, `winget`,
`yay`, `nix`, `asdf`/`mise`) instead of hunting a release download, and they need the one
headline caveat (macOS asdf/mise is CLI-only) before they pick a channel.
**Use Case**: a macOS developer with Homebrew scans the README, sees `brew install --cask qmkonnect`
+ the `--no-quarantine` note, and clicks the link to `docs/installation.md` for the custom-tap
command. A Windows power user sees both `winget` and `scoop` rows and picks one.
**Pain Points Addressed**: (1) the README currently lists ONLY direct installers + build-from-source,
so a `brew`/`scoop`/`winget`/`yay` user has no idea a native channel exists; (2) a macOS user who
installs via asdf/mise would silently get a tray-less app unless warned; (3) the F15 channels
shipped (P1.M1–P1.M4 Complete) but the front page doesn't surface them.

## Why

- **F15 (PRD §4) shipped the channels; the front page must advertise them.** PRD §5 lists
  AUR/Nix/Homebrew/Scoop/Winget alongside the direct installers and calls mise/asdf
  "cross-cutting every platform". The channels exist (P1.M1–P1.M4 Complete) but the README still
  describes only direct installers + source builds.
- **The README is the project front page.** It is the single most-read file. A power user who
  prefers a package manager should see their option without leaving GitHub. `docs/installation.md`
  has the detail (enriched by the parallel P1.M6.T1.S1); the README's job is the at-a-glance table
  + the one caveat that prevents a broken install (macOS asdf/mise tray-less).
- **Mirrors the PRD feature table (§4) + platform matrix (§5) at a high level.** The contract
  explicitly asks the README to "mirror the PRD feature table (§4) and platform matrix (§5) at a
  high level with quick-install commands."

## What

### Approach: ONE concise `### Package Managers` subsection — table + caveat blockquote
- **No restructuring.** Insert one new `###` subsection as a sibling, between `### macOS` and
  `### From Source`. Do not touch any existing heading, code fence, or line.
- **Commands verbatim from the channel READMEs** (the authoritative source — research §2 matrix).
  Do not paraphrase package IDs / URLs (`dabstractor.QMKonnect`, `github:dabstractor/qmkonnect`,
  `mulletware/qmkonnect` tap, `dabstractor/scoop-qmkonnect` bucket, `dabstractor/asdf-qmkonnect`
  plugin, `qmkonnect-bin` are all exact).
- **Two-step channels** (Homebrew tap, Scoop bucket) show the **primary install command** in the
  table + a short "(custom tap — see guide)" note, rather than cramming two commands into one cell.
  The full tap/bucket/plugin-add commands live in `docs/installation.md`.
- **Caveats concise, factual, scoped** — 3 short bullets in one blockquote, then a link to the
  guide for everything else. The macOS asdf/mise CLI-only caveat is mandatory (prevents a
  broken-install surprise); mise/asdf-not-on-Windows and the unsigned-DMG/installer prompt are the
  other two.
- **Concise by mandate** (contract: "Keep it concise — full instructions are in docs/installation.md").

### Success Criteria
- [ ] `README.md` has a new `### Package Managers` subsection (exactly one occurrence) located
      between `### macOS` and `### From Source`.
- [ ] The table lists all 7 channels (AUR `qmkonnect-bin`, Nix flake, Homebrew Cask, Scoop, Winget,
      mise, asdf) with their verbatim install command.
- [ ] The 3 mandatory caveats are present (macOS asdf/mise CLI-only; asdf/mise not on Windows;
      unsigned DMG/installer → Gatekeeper/SmartScreen "unverified" prompt).
- [ ] A relative link to `docs/installation.md` is present for full detail.
- [ ] `git diff` = 1 file; no headings moved/renumbered; no `{{ site.baseurl }}`/Jekyll tags;
      code fences remain balanced.

## All Needed Context

### Context Completeness Check

_Pass._ An agent with no prior knowledge can implement this from: the exact single insertion anchor
(line-verified, with the exact old/new text in "Implementation Tasks"), the verbatim install
command per channel (research §2), the 3 exact caveats (research §3), and the scope boundary (1 file
only, GitHub markdown not Jekyll). No judgment calls remain — the one edit is pinned to a unique
anchor with before/after text, and the full new block is written out verbatim.

### Documentation & References

```yaml
# MUST READ — the verbatim install matrix + caveats + exact README structure + scope boundary
- docfile: plan/007_fb356ba503b4/P1M6T2S1/research/readme_research.md
  why: "the single source of truth for this PRP. The verbatim install matrix (exact command per
        channel), the exact package IDs (dabstractor.QMKonnect / mulletware tap / scoop-qmkonnect
        bucket / asdf-qmkonnect plugin / qmkonnect-bin / github:dabstractor/qmkonnect), the 3
        mandatory caveats, the line-verified single insertion anchor (macOS item 4 + '### From
        Source'), the README heading structure, and the scope-boundary table. Every claim traces
        back to a row here."
  section: "all — every section is a deliverable or a placement anchor"
  critical: "the ONLY file this task edits is README.md. The README is GitHub-rendered (NOT a
        Jekyll page) — no front-matter, and '{{ site.baseurl }}' is FORBIDDEN (that Liquid tag is
        for the docs/ Jekyll site only). mise/asdf on macOS is CLI-ONLY (caveat mandatory); on
        Windows NOT supported."

# MUST READ — the file being edited (the single anchor lives here)
- file: README.md
  why: "the deliverable. The '## Installation' section (line 45) has per-platform direct-installer
        subsections (Windows, Arch Linux, Other Linux Systems, macOS) + '### From Source'. Insert
        the new '### Package Managers' subsection BETWEEN '### macOS' (ends line 104) and
        '### From Source' (line 106)."
  pattern: "GitHub-rendered README (no front-matter). Relative links: 'docs/installation.md' and
        'packaging/.../README.md' (already used at line 127 'docs/installation.md#macos'). New
        internal links use relative paths, NEVER '{{ site.baseurl }}'. Tables + blockquotes render
        on GitHub. New subsection inserts as a '###' sibling."
  gotcha: "do NOT renumber/reorder existing headings — insert only. The insertion anchor line 104
        contains a UTF-8 em-dash (—, U+2014) and right-arrow (→, U+2192); the edit oldText MUST use
        the actual characters, not 'cat -A' escapes. Do NOT add code fences (the new block is a
        table + blockquote; adding fences risks unbalancing). Do NOT edit any other file
        (docs/installation.md = P1.M6.T1.S1; llms_full.txt + PACKAGING.md = P1.M6.T2.S2; .yml CI =
        P1.M5.T2.S2)."

# REFERENCE — the authoritative install commands (verbatim from each channel README)
- file: packaging/linux/aur/README.md
  why: "AUR: 'yay -S qmkonnect-bin' (or paru). '-bin' = prebuilt binary (sibling of the source
        PKGBUILD already documented in the README's '### Arch Linux'). pacman hooks auto-do
        udev+systemd; default QMK keyboards need no config."
- file: packaging/nix/README.md
  why: "Nix: 'nix profile install github:dabstractor/qmkonnect' (persist) / 'nix run …' (ad-hoc).
        NixOS: import nixosModules.default + services.qmkonnect.enable = true. Non-NixOS: one-time
        manual udev setup. (README table uses the 'nix run' form — matches the contract example.)"
- file: packaging/homebrew/README.md
  why: "Homebrew: tap 'mulletware/qmkonnect' → repo 'github.com/dabstractor/homebrew-qmkonnect';
        'brew install --cask qmkonnect'. CAVEAT: unnotarized DMG → '--no-quarantine' (or xattr);
        custom tap until notarized. Screen Recording still required."
- file: packaging/scoop/README.md
  why: "Scoop: bucket 'dabstractor/scoop-qmkonnect' ('scoop bucket add qmkonnect <URL>' — the URL
        is REQUIRED); 'scoop install qmkonnect'. CAVEAT: extracts via innounp (doesn't run
        installer) → autostart OFF, no ARP entry. Per-user, no admin."
- file: packaging/winget/README.md
  why: "Winget: 'winget install dabstractor.QMKonnect' (capital Q) or moniker 'winget install
        qmkonnect'. CAVEAT: unsigned installer → 'unverified publisher' + SmartScreen (More info →
        Run anyway). Runs the installer (autostart on, ARP entry, AUMID set)."
- file: packaging/asdf/README.md
  why: "asdf/mise: plugin repo 'dabstractor/asdf-qmkonnect' serves BOTH managers. 'asdf install
        qmkonnect latest' / 'mise install qmkonnect@latest'. Linux=full support; macOS=CLI ONLY
        (no menu-bar tray); Windows=NOT supported."

# REFERENCE — PRD §4 (F15 row) + §5 (the compatibility matrix the README mirrors at a high level)
- docfile: plan/007_fb356ba503b4/prd_snapshot.md
  why: "§4 F15 row defines the 7 community channels; §5 matrix lists Windows (Inno · Scoop ·
        Winget), macOS (.dmg · Homebrew Cask), Linux (AUR · Nix · PKGBUILD), with mise+asdf
        cross-cutting Linux+macOS. The README table mirrors this."
  section: "§4 (F15 row), §5 (heading '5. Supported Platforms (Compatibility Matrix)')"

# REFERENCE — the per-channel caveats (PRD §12 → external_deps.md §2-§4)
- docfile: plan/007_fb356ba503b4/architecture/external_deps.md
  why: "§2 Homebrew ('custom tap until notarization qualifies it for the official cask'); §3 Scoop
        ('unaffected, they don't enforce code-signing'); §4 Winget ('prompts unverified publisher').
        These are the signing/notarization caveats the blockquote summarizes."
  section: "§2 Homebrew Cask, §3 Scoop, §4 Winget"
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
README.md                              # EDIT (the ONLY file). 1 insertion inside it.
  :45      "## Installation"
  :47      "### Windows"               # direct installer + build-from-source link
  :62      "### Arch Linux"            # source PKGBUILD (makepkg -si)
  :70      "### Other Linux Systems"   # binary + udev/systemd
  :100     "### macOS"                 # DMG
  :104     macOS item 4 (em-dash + →)  # ← Anchor (insert "### Package Managers" AFTER this line)
  :106     "### From Source"           # ← Anchor (the new subsection sits BEFORE this)
  :138     "## QMK Firmware Setup (REQUIRED)"
packaging/{linux/aur,nix,homebrew,scoop,winget,asdf}/README.md   # READ-ONLY inputs (verbatim commands)
plan/007_fb356ba503b4/architecture/external_deps.md              # READ-ONLY (per-channel caveats §2-§4)
# ZERO other files touched (no docs/installation.md, no llms_full.txt, no PACKAGING.md, no .yml/.rs/Cargo.toml)
```

### Desired Codebase tree with files added/changed

```bash
README.md   # +1 "### Package Managers" subsection (table + caveat blockquote) between macOS and From Source. (no new files)
```

### Known Gotchas of our codebase & Library Quirks

```text
# CRITICAL (README is GitHub-rendered, NOT a Jekyll page): README.md line 1 is '# QMKonnect' — NO
#   front-matter. GitHub renders it directly. Relative links ('docs/installation.md',
#   'packaging/.../README.md') resolve on GitHub. Anchor links like 'docs/installation.md#macos'
#   work (GitHub auto-generates heading anchors). DO NOT use Liquid '{{ site.baseurl }}' — that tag
#   is for the docs/ Jekyll site (docs/_config.yml), and it renders as literal text in the root
#   README on GitHub. Plain relative paths only.

# CRITICAL (the insertion anchor has UTF-8 bytes): line 104 contains a real em-dash '—' (U+2014)
#   and right-arrow '→' (U+2192). `cat -A` shows them as 'M-bM-^@M-^T' / 'M-bM-^FM-^R'. The edit
#   tool's oldText MUST contain the actual UTF-8 characters '—' and '→', NOT the 'cat -A' escapes.
#   If oldText fails to match, re-read the exact current lines 103-107 and copy the bytes verbatim.

# CRITICAL (mise/asdf macOS is CLI-ONLY — this caveat is mandatory, not optional): the asdf README
#   states the macOS install copies only the raw Mach-O binary; the menu-bar tray/icon DOES NOT
#   WORK (needs the .app bundle). If you omit this, a macOS user installs via asdf/mise and gets a
#   tray-less app. State it explicitly in the caveat blockquote. Likewise mise/asdf are NOT
#   available on Windows (the asset is an Inno installer, not a portable binary) — state this too.

# CRITICAL (package IDs / URLs are EXACT — copy verbatim, do not "normalize"): 'dabstractor.QMKonnect'
#   (winget, capital Q), 'github:dabstractor/qmkonnect' (nix, lowercase), 'mulletware/qmkonnect'
#   (homebrew TAP ALIAS — note: tap user is 'mulletware', NOT 'dabstractor'; repo is
#   'dabstractor/homebrew-qmkonnect'), 'dabstractor/scoop-qmkonnect' (scoop bucket),
#   'dabstractor/asdf-qmkonnect' (asdf plugin repo, SAME for mise), 'qmkonnect-bin' (AUR). A typo
#   = a broken install command a user copy-pastes. The research §2 matrix has them verbatim.

# CRITICAL (Homebrew tap user is 'mulletware', NOT 'dabstractor'): the Homebrew channel uses
#   'brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect'. The TAP
#   ALIAS is 'mulletware/qmkonnect' but the REPO is 'dabstractor/homebrew-qmkonnect'. In the README
#   table, the install command is 'brew install --cask qmkonnect'; the tap alias appears only in the
#   '(custom tap — see guide)' note (do NOT spell the mismatch out in the README — the guide has it;
#   keep the README concise).

# CRITICAL (the new block adds ZERO code fences — must not unbalance): README.md currently has 32
#   code-fence lines (grep -c '^```' = 32, even). The new '### Package Managers' block is a markdown
#   TABLE + a blockquote — NO ``` fences. Adding a fence would risk breaking the existing 16 pairs.
#   Do NOT wrap the table or commands in ``` fences; use inline `code spans` (backticks) inside the
#   table cells instead (GitHub renders backtick spans in tables fine).

# CRITICAL (one edit, one unique anchor — use the edit tool's exact oldText): the anchor is the
#   macOS item-4 line + '### From Source'. It is line-verified unique (research §1). If oldText does
#   not match, re-read the exact current lines 103-107 (the file may have shifted) and retry — do
#   NOT guess.

# GOTCHA (consistency with the sibling docs task — same heading label): P1.M6.T1.S1 uses
#   '### Package Managers' as the heading for the per-channel subsections in docs/installation.md.
#   Use the SAME heading here so a user reading the README then the guide sees the same label. Do
#   NOT invent a different name like 'Distribution Channels' (the contract offered both names;
#   'Package Managers' is chosen for cross-doc consistency).

# GOTCHA (two-step channels — keep the README concise): Homebrew (tap + install) and Scoop (bucket +
#   install) each need a one-time add command. Do NOT cram both commands into one table cell. Show
#   the primary install command in the table ('brew install --cask qmkonnect' / 'scoop install
#   qmkonnect') and add a '(custom tap — see guide)' / '(custom bucket — see guide)' note. The full
#   tap/bucket/plugin-add commands live in docs/installation.md.

# GOTCHA (do NOT touch the existing direct-installer subsections or the build-from-source section):
#   the README's Windows/Arch Linux/Other Linux/macOS subsections already document the direct
#   installers + build-from-source. This task only ADDS the community-channels subsection between
#   macOS and From Source. Do not reword, reorder, or "fix" anything else in the file (e.g. leave
#   any pre-existing staleness alone — out of scope).

# GOTCHA (do NOT create new files): the contract OUTPUT is "Updated README.md". Do NOT add a
#   CONTRIBUTING.md edit, a CHANGELOG, or touch docs/llms_full.txt (P1.M6.T2.S2 regenerates it from
#   docs/*.md — NOT from README, so there is no dependency, but still do not edit it).
```

## Implementation Blueprint

### Data models and structure
None. Pure documentation. The only "data" is the install-command matrix and the 3 caveats, both
given verbatim below.

### Implementation Tasks (ordered by dependencies — a single edit in `README.md`)

> The edit uses the `edit` tool with the exact `oldText` anchor below (line-verified unique). If
> `oldText` does not match, re-read the exact current lines 103-107 of `README.md` (the file may
> have shifted) and retry — never guess.

```yaml
Task 1: EDIT README.md — ADD the "### Package Managers" subsection between macOS and From Source
  oldText (UNIQUE — macOS item 4 [note the UTF-8 em-dash — and arrow →] + the From Source heading):
    4. It starts automatically at login by default — toggle it from the menu-bar icon → **Launch at Login**.

    ### From Source
  newText (insert the new subsection between them — table + caveat blockquote, ZERO code fences):
    4. It starts automatically at login by default — toggle it from the menu-bar icon → **Launch at Login**.

    ### Package Managers

    QMKonnect also ships through community package managers — the same release binaries, kept
    current automatically. Pick your platform's channel (full per-channel setup and caveats:
    [Installation Guide](docs/installation.md)):

    | Platform | Channel | Install command |
    | --- | --- | --- |
    | **Arch Linux** | AUR (`qmkonnect-bin`) | `yay -S qmkonnect-bin` |
    | **Nix** (NixOS / Nix) | flake | `nix run github:dabstractor/qmkonnect` |
    | **macOS** | Homebrew Cask | `brew install --cask qmkonnect` *(custom tap — see guide)* |
    | **Windows** | Scoop | `scoop install qmkonnect` *(custom bucket — see guide)* |
    | **Windows** | Winget | `winget install dabstractor.QMKonnect` |
    | **Linux / macOS** | mise · asdf | `mise install qmkonnect@latest` · `asdf install qmkonnect latest` |

    > ⚠️ **Before you pick a channel:**
    > - **mise / asdf on macOS is CLI-only** — no menu-bar tray (use Homebrew or the DMG for the full
    >   app), and **not available on Windows** (use Scoop, Winget, or the Inno installer).
    > - The DMG and Windows installer are **ad-hoc / unsigned**, so expect a Gatekeeper / SmartScreen
    >   **"unverified publisher"** prompt until a stable code-signing certificate lands
    >   (`brew install --cask --no-quarantine qmkonnet` for Homebrew, *More info → Run anyway* for
    >   Winget / the Inno installer; Scoop is unaffected — it *extracts* rather than runs).

    ### From Source
  WHY: the contract deliverable — a concise section listing all 7 channels + quick-install commands,
    mirroring PRD §5 at a high level, linking to docs/installation.md for the full detail. Commands
    verbatim from each channel README (research §2). The 3 caveats (research §3) are mandatory.
    Placement between macOS and From Source keeps the direct-installer flows first, community
    channels as a unified alternative, build-from-source last. Adds ZERO code fences (table +
    blockquote only) → no fence-balance risk.

  # NOTE on the Homebrew quarantine command in the caveat blockquote: it reads
  # `brew install --cask --no-quarantine qmkonnet` — CORRECT spelling is `qmkonnect` (with the 'c'
  # before 't'): use `brew install --cask --no-quarantine qmkonnect` in the actual edit. The cask
  # token is lowercase `qmkonnect`.

Task 2: VALIDATE (no edits)
  - git diff --stat                         # EXACTLY README.md (1 file).
  - git diff --name-only | grep -vE '^README\.md$' && echo "FAIL: out-of-scope file" || echo "OK"
  - Exactly one "### Package Managers" heading, located between "### macOS" and "### From Source".
  - One grep per channel command (see Validation Level 2) → each ≥1.
  - The 3 caveats present (see Validation Level 2) → each ≥1.
  - Code fences balanced: `n=$(grep -c '^```' README.md); [ $((n % 2)) -eq 0 ] && echo OK`.
  - No Jekyll/Liquid tags introduced: `grep -c '{{ site.baseurl }}' README.md` → 0.

Task 3: NEVER do these (out of scope / forbidden)
  - DO NOT edit any file other than README.md (docs/installation.md = P1.M6.T1.S1;
    docs/llms_full.txt + spec/PACKAGING.md = P1.M6.T2.S2; the .yml CI = P1.M5.T2.S2).
  - DO NOT renumber/reorder/move any existing heading — insert the new subsection only, as a sibling.
  - DO NOT use Jekyll/Liquid tags ('{{ site.baseurl }}' is FORBIDDEN in the root README) — use plain
    relative links ('docs/installation.md', 'packaging/.../README.md').
  - DO NOT add code fences to the new block (it is a table + blockquote; inline `code spans` only).
  - DO NOT paraphrase package IDs / URLs — copy verbatim (dabstractor.QMKonnect,
    github:dabstractor/qmkonnect, mulletware/qmkonnect tap alias, dabstractor/scoop-qmkonnect
    bucket, dabstractor/asdf-qmkonnect plugin, qmkonnect-bin).
  - DO NOT omit the macOS asdf/mise CLI-only caveat, the mise/asdf-not-on-Windows caveat, or the
    unsigned-DMG/installer "unverified publisher" caveat — they are explicit contract deliverables.
  - DO NOT list mise/asdf as a Windows install option in the table (the table row says "Linux /
    macOS"; the caveat blockquote explicitly states Windows is unsupported for them).
  - DO NOT spell the Homebrew cask token as 'qmkonnet' — it is 'qmkonnect' (lowercase, with the 'c').
  - DO NOT "fix" any pre-existing staleness elsewhere in the README (out of scope).
  - DO NOT edit PRD.md, tasks.json, prd_snapshot.md, .gitignore, or any source file.
```

> **Correction call-out (Task 1 caveat blockquote):** the Homebrew quarantine command appears as
> `brew install --cask --no-quarantine qmkonnet` in the newText block above — that is a TYPO. The
> correct cask token is **`qmkonnect`** (lowercase, with the `c` before the `t`): write
> **`brew install --cask --no-quarantine qmkonnect`** in the actual edit. (The cask token matches
> the package name; see `packaging/homebrew/README.md` "Cask token `qmkonnect`".)

### Implementation Patterns & Key Details

````text
# PATTERN (GitHub-README edit — relative links, no Jekyll tags):
#   README.md is GitHub-rendered (NOT a Jekyll page). Front-matter is ABSENT (line 1 = '# QMKonnect').
#   Internal links are PLAIN relative paths: 'docs/installation.md', 'packaging/.../README.md'.
#   Anchor links like 'docs/installation.md#macos' work (GitHub generates heading anchors).
#   NEVER use Liquid '{{ site.baseurl }}' — it renders as literal text on GitHub (it is a docs/-
#   site-only tag).

# PATTERN (table + caveat blockquote — the README's established style):
#   The README already uses blockquotes (e.g. the Windows requirements note at line 47: '> **Requirements:**…')
#   and tables render on GitHub. The new '### Package Managers' subsection = a 3-column table
#   (Platform | Channel | Install command) + a '> ⚠️' caveat blockquote. Inline `code spans`
#   (single backticks) inside table cells render fine; do NOT use ``` fenced blocks in the table.

# PATTERN (concise by mandate — the guide has the detail):
#   The contract says "Keep it concise — full instructions are in docs/installation.md." So:
#     - one representative install command per channel in the table (not the full tap/bucket/plugin
#       dance — that's a '(custom X — see guide)' note);
#     - 3 short caveat bullets (not the full per-channel caveat essays from docs/installation.md);
#     - a single link to docs/installation.md for everything else.

# WHY a single consolidated subsection (not per-platform subsections):
#   P1.M6.T1.S1 already does per-platform '### Package Managers' subsections in docs/installation.md
#   (the DETAILED version). Duplicating that structure in the README violates "concise" and creates
#   two drift-prone maintenance surfaces. The README is the front page — one scannable table is the
#   right shape. The two docs share the SAME heading label ('### Package Managers') for consistency.

# WHY placement is between '### macOS' and '### From Source' (not at the top of ## Installation):
#   The direct-installer flows (the PRIMARY install path) stay first and uninterrupted; the community
#   channels are presented as a unified ALTERNATIVE; build-from-source stays last. A reader scanning
#   top-to-bottom hits the primary path, then the "or use your package manager" alternative, then the
#   "or build it" fallback — a natural preference order.
````

### Integration Points

```yaml
README (repo-root, GitHub-rendered — NOT Jekyll):
  - links: PLAIN relative paths ('docs/installation.md', 'packaging/.../README.md'); NO '{{ site.baseurl }}'
  - new section: 1 × '### Package Managers' (between '### macOS' and '### From Source')
DOWNSTREAM (no dependency, no conflict):
  - P1.M6.T2.S2 regenerates docs/llms_full.txt FROM docs/*.md (NOT from README) → it does NOT pick
    up README edits. No action needed; just don't also edit llms_full.txt.
PARALLEL (zero conflict):
  - P1.M6.T1.S1 edits docs/installation.md (separate file) — zero overlap. The README links TO it.
  - P1.M5.T2.S2 edits .github/workflows/*.yml (CI jobs) — zero docs overlap.
CONSUMES (READ-ONLY inputs — the authoritative install commands + caveats):
  - packaging/{linux/aur,nix,homebrew,scoop,winget,asdf}/README.md (verbatim commands)
  - plan/007_fb356ba503b4/architecture/external_deps.md §2-§4 (the signing/notarization caveats)
  - PRD §4 (F15 row) + §5 (the compatibility matrix the table mirrors)
PRODUCES:
  - an updated README.md with all 7 community channels surfaced at a glance (1 file changed).
```

## Validation Loop

> This is a single-file documentation edit on a Linux dev box. The gates prove: scope (1 file),
> content (all 7 channels + 3 caveats present), placement (between macOS and From Source), and
> markdown integrity (no Jekyll tags, balanced fences, valid table/blockquote).

### Level 1: Scope + Markdown sanity (runs on Linux)
```bash
cd /home/dustin/projects/qmkonnect
git diff --stat                              # Expected: EXACTLY README.md (1 file).
git diff --name-only | grep -vE '^README\.md$' && echo "FAIL: out-of-scope file edited" || echo "OK: only README.md"
# No Jekyll/Liquid tags introduced (the root README is NOT a Jekyll page):
[ "$(grep -c '{{ site.baseurl }}' README.md)" -eq 0 ] && echo "OK: no Liquid tags" || echo "FAIL: Liquid tag present"
# Code fences balanced (the new block adds ZERO fences; 32 must stay even):
n=$(grep -c '^```' README.md); [ $((n % 2)) -eq 0 ] && echo "OK: $n fence lines (balanced)" || echo "FAIL: unbalanced code fences"
# Heading hierarchy — the new ### sits as a sibling, nothing renumbered:
grep -nE '^#{2,3} ' README.md | sed -n '1,30p'   # eyeball: '### Package Managers' appears once, between '### macOS' and '### From Source', under '## Installation'
```

### Level 2: Content — all 7 channels + 3 caveats present (runs on Linux)
```bash
cd /home/dustin/projects/qmkonnect
echo "channels present (each must be >=1):"
for pat in 'yay -S qmkonnect-bin' \
           'nix run github:dabstractor/qmkonnect' \
           'brew install --cask qmkonnect' \
           'scoop install qmkonnect' \
           'winget install dabstractor.QMKonnect' \
           'mise install qmkonnect@latest' \
           'asdf install qmkonnect latest'; do
  n=$(grep -cF "$pat" README.md); echo "  [$n] $pat"; [ "$n" -ge 1 ] || echo "    !!! MISSING"
done
echo "caveats present (each must be >=1):"
for pat in 'cli-only' \
           'not available on Windows' \
           'unverified'; do
  n=$(grep -ciF "$pat" README.md); echo "  [$n] $pat"; [ "$n" -ge 1 ] || echo "    !!! MISSING"
done
echo "link to the detailed guide present:"
grep -cF 'docs/installation.md' README.md | xargs -I{} echo "  [{}] docs/installation.md link(s)"
# Expected: all 7 channels >=1; all 3 caveats >=1; >=1 link to docs/installation.md.
```

### Level 3: Placement fidelity — the subsection landed between macOS and From Source (runs on Linux)
```bash
cd /home/dustin/projects/qmkonnect
# Exactly one '### Package Managers' heading:
[ "$(grep -cE '^### Package Managers$' README.md)" -eq 1 ] && echo "OK: exactly one ### Package Managers" || echo "FAIL: heading count != 1"
# It sits under '## Installation', between '### macOS' and '### From Source' (document order):
awk '
  /^## Installation/        { in_inst=1 }
  /^### macOS/              { saw_macos=1 }
  /^### Package Managers/   { if (in_inst && saw_macos) print "OK: ### Package Managers is AFTER ### macOS"; else print "FAIL: wrong placement" }
  /^### From Source/        { if (saw_pm) print "OK: ### From Source is AFTER ### Package Managers"; exit }
' README.md
# (saw_pm is set by the AWK block above via the print; if you want an explicit guard, re-run:)
grep -nE '^(### macOS|### Package Managers|### From Source)$' README.md   # eyeball: macOS line# < Package Managers line# < From Source line#
# Expected: macOS appears first, then Package Managers, then From Source (ascending line numbers).
```

### Level 4: Render sanity (optional — only if you want to eyeball on GitHub; NOT required)
```bash
# The README is plain GitHub markdown. Optional local renderers (if installed):
#   glow README.md 2>/dev/null || mdcat README.md 2>/dev/null || true
# Or push to a branch and view on GitHub. The table + blockquote render natively; nothing exotic.
# Expected: the table renders as a grid, the blockquote renders with the ⚠️, inline code spans
# render. This is OPTIONAL — the grep + fence-balance + placement gates above are the required bar.
```

## Final Validation Checklist

### Technical Validation
- [ ] Level 1: `git diff --stat` = exactly `README.md`; no Liquid tags; code fences balanced (even count).
- [ ] Level 2: all 7 channel commands ≥1; all 3 caveats ≥1; ≥1 link to `docs/installation.md`.
- [ ] Level 3: exactly one `### Package Managers`; it sits between `### macOS` and `### From Source`.

### Feature (Docs) Validation
- [ ] A reader sees the full channel matrix (all 7 channels) in one scannable table on the front page.
- [ ] The macOS asdf/mise CLI-only caveat, the mise/asdf-not-on-Windows caveat, and the
      unsigned-DMG/installer "unverified publisher" caveat are all present.
- [ ] Package IDs/URLs are verbatim (dabstractor.QMKonnect, github:dabstractor/qmkonnect,
      mulletware/qmkonnect tap alias, dabstractor/scoop-qmkonnect bucket, dabstractor/asdf-qmkonnect
      plugin, qmkonnect-bin; cask token `qmkonnect` spelled with the `c`).
- [ ] mise/asdf are NOT advertised as a Windows install option (table row = "Linux / macOS"; caveat
      states Windows is unsupported for them).

### Code Quality Validation
- [ ] GitHub markdown only — no Jekyll/Liquid `{{ site.baseurl }}` tags; plain relative links.
- [ ] No heading renumbering/reordering — the new `### Package Managers` inserted as a sibling only.
- [ ] Markdown well-formed (valid table; valid blockquote; balanced code fences; inline code spans).
- [ ] Follows the existing README's tone (factual, second-person, scannable, tables + blockquotes).

### Documentation & Deployment
- [ ] Only `README.md` changed (1 file) — sibling tasks (docs/installation.md, llms_full, PACKAGING,
      CI .yml) untouched.
- [ ] The README links to `docs/installation.md` for the full per-channel detail (no duplication).

---

## Anti-Patterns to Avoid

- ❌ Don't edit any file other than `README.md` (docs/installation.md / llms_full / PACKAGING / .yml are sibling-owned).
- ❌ Don't use Jekyll/Liquid `{{ site.baseurl }}` in the root README — it is GitHub-rendered, not a Jekyll page; use plain relative links.
- ❌ Don't add code fences to the new block — it's a table + blockquote; inline `code spans` only (a stray fence unbalances the existing 16 pairs).
- ❌ Don't renumber or reorder existing headings — insert the one new `###` subsection as a sibling only.
- ❌ Don't paraphrase package IDs/URLs — copy verbatim (a typo = a broken copy-pasted install command).
- ❌ Don't spell the cask token `qmkonnet` — it is `qmkonnect` (with the `c`).
- ❌ Don't omit the macOS asdf/mise CLI-only caveat or the unsigned-DMG/installer caveat — they're explicit deliverables.
- ❌ Don't list mise/asdf under Windows as a supported option (table row = "Linux / macOS"; caveat says Windows unsupported).
- ❌ Don't duplicate the full per-channel essays from docs/installation.md — keep the README concise; link out for detail.
- ❌ Don't create new files (the contract OUTPUT is "Updated README.md").
- ❌ Don't guess the `edit` anchor if `oldText` doesn't match — re-read exact lines 103-107 (the UTF-8 em-dash `—` and arrow `→` must be the real characters) and retry.