# PRP — P1.M6.T1.S1: Add community channel sections to docs/installation.md

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`, org `dabstractor`). **ONE file edited:**
> `docs/installation.md` (a Jekyll page — `layout: default`, `permalink: /installation/`). **Zero
> Rust, zero Cargo.toml, zero .yml, zero other docs.** This is the F15 documentation task.
>
> **What this does:** documents the seven F15 community package-manager channels
> (AUR, Nix, Homebrew, Scoop, Winget, mise, asdf) as installable alternatives to the direct
> installers, with the **exact install commands** (verbatim from each channel README) and the
> **per-channel signing/notarization caveats** (PRD §12 / external_deps.md §2-§4). Removes the now-false
> "(there is no AUR package)" note. Adds a top summary table cross-referencing PRD §5.
>
> **Scope boundary (siblings — do NOT touch):** P1.M6.T2.S1 owns `README.md`; P1.M6.T2.S2 owns
> `docs/llms_full.txt` (+ PACKAGING refs) — it REGENERATES llms_full from these source docs, so it
> will pick up THIS task's edits as an input (no conflict). P1.M5.T2.S2 (parallel) owns the CI
> `.yml` files. **This task edits ONLY `docs/installation.md`.**
>
> **Source of truth for this design:** `research/install_doc_research.md` (the verbatim install
> matrix, the per-channel caveats, the exact current heading structure + line-number anchors, and
> the scope-boundary table).

---

## Goal

**Feature Goal**: Make `docs/installation.md` the one accurate, navigable source for *every* way to
install QMKonnect — the direct installers (already documented) **plus** the seven F15 community
package-manager channels. A reader who lands on the page can (a) see the full channel matrix at a
glance, and (b) copy the exact install command + read the caveats for their platform's package manager.

**Deliverable** (exactly ONE file edited, FIVE edits inside it):
1. **REMOVE** the stale "(there is no AUR package)" parenthetical (line 57) and reword the Arch
   line to point at the new AUR channel.
2. **ADD** a `## Installation Methods` summary table (cross-refs PRD §5) right after the intro,
   before `## Windows`.
3. **ADD** a `### Package Managers` subsection under `## Windows` (Scoop + Winget + unverified-publisher caveat).
4. **ADD** a `### Package Managers` subsection under `## Linux` (AUR + Nix + mise/asdf; non-NixOS udev note).
5. **ADD** a `### Package Managers` subsection under `## macOS` (Homebrew + custom-tap/`--no-quarantine` caveat + mise/asdf CLI-only caveat).

**Success Definition**:
- `git diff --stat` shows **ONLY** `docs/installation.md` (1 file). Nothing else.
- The string `(there is no AUR package)` is **gone** (`grep -c "no AUR package" docs/installation.md` → 0).
- All seven channels' install commands are present: `grep -c` for `yay -S qmkonnect-bin`,
  `nix profile install github:dabstractor/qmkonnect`, `brew install --cask`, `scoop bucket add`,
  `winget install dabstractor.QMKonnect`, `asdf plugin add qmkonnect`, `mise plugin add qmkonnect`
  each returns ≥1.
- The per-channel caveats are present: `grep` for `unverified publisher`, `--no-quarantine`,
  `custom tap`, `CLI only`.
- Jekyll front-matter (lines 1-4) is **byte-identical**; the `{{ site.baseurl }}` link convention
  is preserved; no heading-level renumbering (existing `#`/`##`/`###` structure intact — new sections
  are inserted as siblings, not reshuffled).

## User Persona (if applicable)

**Target User**: an end user (or sysadmin) who already uses a platform package manager and wants to
install QMKonnect the *native* way (`brew`, `scoop`, `winget`, `yay`, `nix`, `asdf`/`mise`) instead
of hunting for a download link — and who needs to know the signing/tray caveats up front.
**Use Case**: a macOS Homebrew user runs `brew install --cask qmkonnect`, gets "app is damaged" from
Gatekeeper, and finds the `--no-quarantine` fix on the install page. A Windows `winget` user sees
"unverified publisher" and learns it's expected + how to proceed.
**Pain Points Addressed**: (1) the page currently claims "no AUR package" when `qmkonnect-bin` exists;
(2) no channel install commands exist at all; (3) the signing/tray caveats (Winget unverified
publisher, Homebrew quarantine, Scoop autostart-off, macOS asdf CLI-only) are buried in packaging
READMEs the end user will never read.

## Why

- **F15 (PRD §4) is shipping the channels; the install page must advertise them.** PRD §5's
  compatibility matrix lists AUR/Nix/Homebrew/Scoop/Winget alongside the direct installers, and the
  §5 note calls mise/asdf "cross-cutting every platform". The channels exist (P1.M1–P1.M4 Complete)
  but `docs/installation.md` still describes only the direct installers + source builds.
- **The page is actively misleading about AUR.** Line 57 says "there is no AUR package"; `qmkonnect-bin`
  now exists (P1.M1.T1). A user reading the page is told to build from source when a one-line
  `yay -S qmkonnect-bin` exists.
- **Caveats belong on the install page, not in packaging READMEs.** Winget's "unverified publisher",
  Homebrew's `--no-quarantine` / custom tap, Scoop's autostart-off, and macOS-asdf's CLI-only
  limitation are real install-time surprises. They're documented in the packaging READMEs (which
  maintainers read) but not where end users look first.

## What

### Approach: one summary table + three per-platform "Package Managers" subsections + one line reword

- **No restructuring.** Insert each new block as a sibling at the correct heading level (`## ` for the
  top table, `### ` under each platform). Do not reorder or renumber existing headings.
- **Commands verbatim from the channel READMEs** (the authoritative source — `research` table). Do not
  paraphrase package IDs / URLs (`dabstractor.QMKonnect`, `github:dabstractor/qmkonnect`,
  `mulletware/qmkonnect` tap, `dabstractor/scoop-qmkonnect` bucket are all exact).
- **Caveats factual and scoped** — each channel's caveat is one short paragraph pulled from
  external_deps.md §2-§4 + the channel README.
- **mise/asdf cross-cut** — Linux fully supported, macOS CLI-only (no menu-bar tray), Windows
  unsupported. State this explicitly so a macOS user doesn't install a broken tray via asdf.
- **Preserve** the Jekyll front-matter, the `{{ site.baseurl }}` link convention, and all existing
  primary-install / build-from-source / Verification / Next-Steps content.

### Success Criteria
- [ ] `docs/installation.md` has a new `## Installation Methods` section (summary table) before `## Windows`.
- [ ] A `### Package Managers` subsection exists under each of `## Windows`, `## Linux`, `## macOS`.
- [ ] The Windows subsection covers Scoop + Winget with the "unverified publisher" caveat.
- [ ] The Linux subsection covers AUR (`yay -S qmkonnect-bin`) + Nix + mise/asdf (non-NixOS udev note).
- [ ] The macOS subsection covers Homebrew (custom tap + `--no-quarantine`) + mise/asdf (CLI-only caveat).
- [ ] The "(there is no AUR package)" parenthetical is removed; the Arch line points to the AUR channel.
- [ ] Jekyll front-matter + the `{{ site.baseurl }}` convention are unchanged; `git diff` = 1 file.

## All Needed Context

### Context Completeness Check

_Pass._ An agent with no prior knowledge can implement this from: the exact 5 edits (verbatim old/new
text in "Implementation Tasks"), the exact install commands per channel (the research table), the exact
per-channel caveats (external_deps.md §2-§4, quoted), the precise insertion anchors (line-verified in
research §"The file being edited"), and the scope boundary (1 file only). No judgment calls remain —
every edit is pinned to a unique anchor string with before/after text.

### Documentation & References

```yaml
# MUST READ — the verbatim install matrix + caveats + exact current doc structure + scope boundary
- docfile: plan/007_fb356ba503b4/P1M6T1S1/research/install_doc_research.md
  why: "the single source of truth for this PRP. The install matrix (verbatim commands per channel),
        the per-channel caveats table (Winget unverified publisher / Homebrew custom tap + quarantine /
        Scoop autostart-off / macOS asdf CLI-only / non-NixOS udev), the exact current heading structure
        + line numbers, the 5 line-verified insertion anchors, and the scope-boundary table (which
        siblings own which files). Every claim in this PRP traces back to a row here."
  section: "all — every section is a deliverable or a placement anchor"
  critical: "the ONLY file this task edits is docs/installation.md. The 5 edits' oldText anchors are
        line-verified unique. mise/asdf on macOS is CLI-ONLY (caveat mandatory); on Windows NOT supported."

# MUST READ — the per-channel caveats (PRD §12 → external_deps.md §2-§4)
- docfile: plan/007_fb356ba503b4/architecture/external_deps.md
  why: "§2 Homebrew ('custom tap until notarization qualifies it for the official cask'); §3 Scoop
        ('unaffected, they don't enforce code-signing'); §4 Winget ('prompts unverified publisher').
        These three lines ARE the signing/notarization caveats the contract requires in the doc."
  section: "§2 Homebrew Cask, §3 Scoop, §4 Winget"
  critical: "the caveats are factual and short — do NOT editorialize. Winget=unverified publisher;
        Homebrew=custom tap (until notarized) + ad-hoc/unnotarized DMG needs --no-quarantine; Scoop=extracts
        (no signing check) so autostart is off."

# MUST READ — the file being edited (the 5 anchors live here)
- file: docs/installation.md
  why: "the deliverable. Current heading structure (grep-verified in research): # Installation Guide →
        ## Windows (Installer / Build from Source) → ## Linux (Hyprland / Arch / Other) → ## macOS
        (release / Launch at login / Build from source) → ## Build from Source (Linux Only) → ## Verification
        → ## Next Steps. The 'no AUR package' line is at line 57. Jekyll front-matter lines 1-4 MUST stay."
  pattern: "Jekyll page: front-matter `---\nlayout: default\ntitle: Installation\npermalink: /installation/\n---`,
        then `# Installation Guide`. Internal links use `{{ site.baseurl }}/foo`. New sections insert as
        siblings at the matching heading level (## for the top table, ### under each platform)."
  gotcha: "do NOT renumber or reorder existing headings — insert only. Do NOT 'fix' the pre-existing
        `v0.1.0` binary-download URL in the Linux section (separate, out-of-scope staleness). Do NOT edit
        any other file (README.md, llms_full.txt, PACKAGING.md are sibling tasks)."

# REFERENCE — the authoritative install commands (verbatim from each channel README)
- file: packaging/linux/aur/README.md
  why: "AUR install: `yay -S qmkonnect-bin` (or paru). pacman hooks auto-do udev+systemd; default QMK
        keyboards need no config. `-bin` = prebuilt binary (sibling of the source PKGBUILD)."
- file: packaging/nix/README.md
  why: "Nix install: `nix profile install github:dabstractor/qmkonnect` / `nix run …`. NixOS: import
        nixosModules.default + `services.qmkonnect.enable = true`. Non-NixOS: one-time manual udev setup."
- file: packaging/homebrew/README.md
  why: "Homebrew install: `brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect`
        + `brew install --cask qmkonnect`. CAVEAT: unnotarized DMG → `--no-quarantine` (or xattr) or
        Gatekeeper blocks launch; custom tap until notarized. Screen Recording still required."
- file: packaging/scoop/README.md
  why: "Scoop install: `scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnect` +
        `scoop install qmkonnect`. CAVEAT: extracts via innounp (doesn't run installer) → autostart OFF
        (toggle 'Open at Login' in tray), no ARP entry. Per-user, no admin."
- file: packaging/winget/README.md
  why: "Winget install: `winget install dabstractor.QMKonnect` (or moniker `winget install qmkonnect`).
        CAVEAT: unsigned installer → 'unverified publisher' + SmartScreen (More info → Run anyway). Runs
        the installer (autostart on, ARP entry, AUMID set)."
- file: packaging/asdf/README.md
  why: "asdf/mise install: `asdf plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect` +
        `asdf install qmkonnect latest` / `mise plugin add qmkonnect …` + `mise install qmkonnect@latest`.
        SAME plugin serves both managers. Linux=full support; macOS=CLI ONLY (no menu-bar tray); Windows
        =NOT supported (use Scoop/Winget/Inno)."

# REFERENCE — PRD §5 (the compatibility matrix the summary table cross-references)
- docfile: plan/007_fb356ba503b4/prd_snapshot.md
  why: "§5 Supported Platforms matrix: Windows 10/11 x64 (Inno · Scoop · Winget), macOS 13+ (.dmg ·
        Homebrew Cask), Linux/Hyprland (AUR · Nix · PKGBUILD/binary); mise+asdf cross-cut Linux+macOS.
        The summary table in the doc mirrors this."
  section: "§5 (heading '5. Supported Platforms (Compatibility Matrix)')"
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
docs/installation.md              # EDIT (the ONLY file). 5 edits inside it.
  :1-4     Jekyll front-matter                       # PRESERVE verbatim
  :14      "QMKonnect has different installation methods for each platform."   # Anchor A (insert table after)
  :30-34   Windows Installer bullets → "### Build from Source"                  # Anchor C (insert Package Managers before Build from Source)
  :57      "…(there is no AUR package):"                                         # Anchor B (reword — remove parenthetical)
  :110-117 "sudo qmkonnect -r" code fence → "---" → "## macOS"                  # Anchor D (insert Linux Package Managers before ---)
  :126-130 macOS release step 5 → "### Launch at login"                         # Anchor E (insert macOS Package Managers before Launch at login)
packaging/{linux/aur,nix,homebrew,scoop,winget,asdf}/README.md   # READ-ONLY inputs (verbatim commands)
plan/007_fb356ba503b4/architecture/external_deps.md              # READ-ONLY (per-channel caveats §2-§4)
# ZERO other files touched (no README.md, no llms_full.txt, no PACKAGING.md, no .yml/.rs/Cargo.toml)
```

### Desired Codebase tree with files added/changed

```bash
docs/installation.md   # +1 summary-table section, +3 "Package Managers" subsections, 1 line reword. (no new files)
```

### Known Gotchas of our codebase & Library Quirks

```text
# CRITICAL (Jekyll front-matter MUST be preserved verbatim): docs/installation.md lines 1-4 are
#   `---\nlayout: default\ntitle: Installation\npermalink: /installation/\n---`. This is what makes
#   the page render at /installation/ on the GitHub Pages site. Do NOT add/remove/alter it. The 5 edits
#   all occur BELOW line 4.

# CRITICAL (this is a Jekyll site — preserve the link convention): internal links use Liquid
#   `{{ site.baseurl }}/foo` (e.g. the existing `[Troubleshooting → Screen Recording]({{ site.baseurl }}/troubleshooting/)`).
#   New internal links MUST follow this. External links (AUR, GitHub, the channel repos) are plain
#   `https://…` URLs (already used in the file). Do NOT use relative `./foo.md` links.

# CRITICAL (mise/asdf macOS is CLI-ONLY — this caveat is mandatory, not optional): the asdf README
#   states the macOS install copies only the raw Mach-O binary; the menu-bar tray/icon DOES NOT WORK
#   (needs the .app bundle). If you omit this, a macOS user installs via asdf/mise and gets a tray-less
#   app. State it explicitly in the macOS Package Managers subsection. Likewise mise/asdf are NOT
#   available on Windows (the asset is an Inno installer, not a portable binary) — do NOT list them
#   under Windows; if mentioned, note Windows is unsupported.

# CRITICAL (package IDs / URLs are EXACT — copy verbatim, do not "normalize"): `dabstractor.QMKonnet`
#   (winget, capital Q), `github:dabstractor/qmkonnect` (nix, lowercase), `mulletware/qmkonnect`
#   (homebrew TAP name — note: tap user is `mulletware`, NOT `dabstractor`), `dabstractor/scoop-qmkonnect`
#   (scoop bucket), `dabstractor/asdf-qmkonnect` (asdf plugin repo), `qmkonnect-bin` (AUR). A typo here
#   = a broken install command a user copy-pastes. The research table has them verbatim.

# CRITICAL (Homebrew tap user is `mulletware`, NOT `dabstractor`): the Homebrew channel README uses
#   `brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect`. The TAP ALIAS is
#   `mulletware/qmkonnect` but the REPO is `dabstractor/homebrew-qmkonnect`. Copy both verbatim. Do not
#   "fix" the mismatch — it's intentional.

# CRITICAL (5 edits, 5 unique anchors — use the edit tool's exact oldText): each insertion anchor is
#   line-verified unique in research §"The file being edited". If an `edit` oldText fails to match,
#   re-read the exact current lines around the anchor (the file may have shifted) and retry — do NOT
#   guess. The anchors: (A) the intro sentence + `## Windows`; (B) line 57 parenthetical; (C) the
#   Windows "Uninstalls cleanly…" bullet + `### Build from Source`; (D) `sudo qmkonnect -r` fence +
#   `---` + `## macOS`; (E) macOS step 5 + `### Launch at login`.

# GOTCHA (the `---` before `## macOS` is a horizontal rule): insert the Linux Package Managers block
#   BEFORE that `---` (so it's the last ### under ## Linux), keeping the `---` as the separator before
#   ## macOS. See Anchor D's exact oldText/newText in Task 4.

# GOTCHA (do NOT touch the pre-existing `v0.1.0` binary URL): the Linux "Other Distributions" section
#   has a `releases/download/v0.1.0/qmkonnect` link that is stale (current version is 0.2.8). That is a
#   SEPARATE pre-existing issue, out of scope for this task. Leave it. (Fixing it = scope creep + a
#   different review surface.)

# GOTCHA (Scoop autostart is OFF — distinct from Winget/Inno which are ON): Scoop EXTRACTS the installer
#   via innounp (doesn't run it), so the HKCU Run autostart value is never written. Tell the user to
#   enable "Open at Login" in the tray. Winget and the direct Inno installer RUN the installer, so
#   autostart is on. This asymmetry is the key Scoop-vs-Winget difference worth one line.

# GOTCHA (do NOT create new top-level files): the contract OUTPUT is "Updated docs/installation.md".
#   Do NOT add a new docs/channels.md or touch llms_full.txt (P1.M6.T2.S2 regenerates it from this file).
```

## Implementation Blueprint

### Data models and structure
None. Pure documentation. The only "data" is the install-command matrix and the caveat text, both
given verbatim below.

### Implementation Tasks (ordered by dependencies — all 5 edits in `docs/installation.md`)

> All edits use the `edit` tool with the exact `oldText` anchors below (line-verified unique). If any
> `oldText` does not match, re-read the surrounding lines (file may have shifted) and retry — never
> guess. Make the edits in document order (top→bottom) so earlier edits don't shift later anchors.

```yaml
Task 1: EDIT docs/installation.md — ADD the top "## Installation Methods" summary table (Anchor A)
  oldText (UNIQUE — the intro sentence + the Windows heading):
    QMKonnect has different installation methods for each platform.

    ## Windows
  newText (insert the table section between the intro and ## Windows):
    QMKonnect has different installation methods for each platform.

    ## Installation Methods

    QMKonnect ships through a **direct installer** (recommended) and, on each platform, one or more
    **community package-manager channels** that keep it updated automatically. Pick one per platform —
    the exact commands and caveats are in each platform's **Package Managers** section below.
    (Full compatibility matrix: PRD §5.)

    | Platform | Direct installer (recommended) | Community channels |
    | --- | --- | --- |
    | **Windows** 10/11 (x64) | Inno `.exe` (per-user, no admin) | Scoop · Winget |
    | **macOS** 13+ | `.dmg` (universal) | Homebrew Cask |
    | **Linux** (Hyprland) | binary / Arch PKGBUILD | AUR · Nix |

    **mise / asdf** are cross-platform version managers that install the prebuilt release binary:
    **Linux** (full app) and **macOS** (**CLI only — no menu-bar tray**); not available on Windows.
    See the per-platform sections.

    ## Windows
  WHY: the contract deliverable (c) — a summary table cross-referencing PRD §5. Placed at the top so a
    reader sees the whole channel matrix before diving into a platform. Plain-text channel names (no
    in-page anchors) to avoid broken Jekyll anchor links.

Task 2: EDIT docs/installation.md — REMOVE the "no AUR package" note (Anchor B)
  oldText (UNIQUE — line 57):
    Build the package from the local `PKGBUILD` (there is no AUR package):
  newText:
    Build from the **source** `PKGBUILD` (or install the prebuilt binary from the AUR — see
    **Package Managers** below):
  WHY: the contract deliverable (a) — `qmkonnect-bin` now exists (P1.M1.T1), so "no AUR package" is
    false. Reword keeps the source-build instructions but points at the new AUR subsection.

Task 3: EDIT docs/installation.md — ADD the Windows "### Package Managers" subsection (Anchor C)
  oldText (UNIQUE — the last Windows-Installer bullet + the Build-from-Source heading):
    - Uninstalls cleanly via Add/Remove Programs

    ### Build from Source
  newText:
    - Uninstalls cleanly via Add/Remove Programs

    ### Package Managers

    Community package managers on Windows fetch the same Inno installer and keep it updated
    automatically. Both are **per-user — no Administrator** needed.

    **Scoop** (extracts the installer; no publisher prompt):

    ```bash
    scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnect
    scoop install qmkonnect
    scoop update qmkonnect      # pull a later release
    ```

    Because Scoop *extracts* the installer via `innounp` instead of running it, **autostart is off
    by default** — enable **"Open at Login"** in QMKonnect's tray menu (the app writes the same HKCU
    `Run` value itself). There is no Add/Remove-Programs entry; manage the app with `scoop update` /
    `scoop uninstall qmkonnect`.

    **Winget** (runs the installer; same result as the direct `.exe`):

    ```powershell
    winget install dabstractor.QMKonnect      # or: winget install qmkonnect
    winget upgrade dabstractor.QMKonnect      # keep current
    ```

    The installer is **not code-signed**, so the first `winget install` (and Windows SmartScreen)
    shows an **"unverified publisher"** prompt — choose *More info → Run anyway*. This is the expected
    beta state, identical to running the unsigned direct installer, and goes away once QMKonnect has a
    stable code-signing certificate. (Scoop is unaffected — it extracts rather than runs, so it never
    trips the publisher check.)

    ### Build from Source
  WHY: contract deliverable (b) Windows + (d) Winget caveat. Commands verbatim from scoop/winget READMEs.

Task 4: EDIT docs/installation.md — ADD the Linux "### Package Managers" subsection (Anchor D)
  oldText (UNIQUE — the qmkonnect -r fence + the --- separator + the macOS heading):
    qmkonnect -c          # writes a commented-out default config (edit as needed)
    sudo qmkonnect -r
    ```

    ---

    ## macOS
  newText:
    qmkonnect -c          # writes a commented-out default config (edit as needed)
    sudo qmkonnect -r
    ```

    ### Package Managers

    **AUR (Arch)** — `qmkonnect-bin` is the prebuilt-binary package: it downloads the GitHub release
    tarball (no Rust toolchain or build dependencies). It is the `-bin` sibling of the source `PKGBUILD`
    above — both install to the same paths and reuse the same pacman hooks (udev reload, systemd-template
    instantiation, global enable).

    ```bash
    yay -S qmkonnect-bin          # or: paru -S qmkonnect-bin
    ```

    The pacman hooks run automatically on install/upgrade, so default QMK keyboards then need **no
    configuration** — QMKonnect auto-discovers them via the Raw HID usage page (`0xFF60` / `0x61`) and
    the shipped static udev rule already grants permissions.

    **Nix** (NixOS, or Nix on another distro) — the flake builds from source against pinned Nixpkgs:

    ```bash
    nix profile install github:dabstractor/qmkonnect   # add to your profile
    # …or run ad-hoc without installing:
    nix run github:dabstractor/qmkonnect
    ```

    On **NixOS**, prefer the flake's module — add `qmkonnet.nixosModules.default` to your config and:

    ```nix
    services.qmkonnect.enable = true;   # udev rule + systemd user service + PATH
    ```

    On **non-NixOS** (Nix on Arch/Ubuntu/Fedora/…), Nix can't install the udev rule system-wide, so do
    the one-time HID-permissions setup (install the static rule, symlink the `qmkonnect-hid-id` helper
    the package ships, reload udev) — see the
    [Nix flake README](https://github.com/dabstractor/qmkonnect/blob/main/packaging/nix/README.md).

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

    ---

    ## macOS
  WHY: contract deliverable (b) Linux. Commands verbatim from aur/nix/asdf READMEs. NOTE the `---`
    separator stays BELOW the new subsection (it precedes ## macOS). NixOS module line uses
    `qmkonnet.nixosModules.default` — wait, CORRECTION: the module output is `qmkonnect.nixosModules.default`
    (lowercase, full name). Use `qmkonnect.nixosModules.default`. (Self-correction: in newText above,
    write `qmkonnect.nixosModules.default`, NOT `qmkonnet.…`.)

Task 5: EDIT docs/installation.md — ADD the macOS "### Package Managers" subsection (Anchor E)
  oldText (UNIQUE — macOS release step 5 + the Launch-at-login heading):
    5. Grant the **Screen Recording** prompt when it appears — this is required to read window titles (see [Troubleshooting → Screen Recording]({{ site.baseurl }}/troubleshooting/) if it keeps reappearing).

    ### Launch at login
  newText:
    5. Grant the **Screen Recording** prompt when it appears — this is required to read window titles (see [Troubleshooting → Screen Recording]({{ site.baseurl }}/troubleshooting/) if it keeps reappearing).

    ### Package Managers

    **Homebrew Cask** — installs the universal `QMKonnect.app` into `/Applications` and keeps it updated
    with `brew upgrade`. It ships through a **custom tap** (`mulletware/qmkonnect`), not the official
    `homebrew-cask`, until the DMG is Developer-ID-signed + notarized:

    ```bash
    brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect
    brew install --cask qmkonnect
    ```

    > **Quarantine caveat (ad-hoc / unnotarized DMG):** the released DMG is **ad-hoc signed and not
    > notarized**, so Homebrew quarantines it and Gatekeeper blocks the first launch ("'QMKonnect' is
    > damaged / can't be opened"). Bypass quarantine for now:
    > ```bash
    > brew install --cask --no-quarantine qmkonnect
    > # …or, after a normal install:
    > xattr -dr com.apple.quarantine /Applications/QMKonnect.app
    > ```
    > Once the DMG is notarized this flag is unnecessary and the cask can graduate to the official
    > `homebrew-cask` repo. The **Screen Recording** prompt (for window titles) is still required either
    > way — see [Troubleshooting]({{ site.baseurl }}/troubleshooting/).

    Uninstall with `brew uninstall --cask qmkonnect` (add `--zap` to also remove the per-user config
    under `~/Library/Application Support/QMKonnect/`).

    **mise / asdf — CLI only (no menu-bar tray).** These install the raw Mach-O binary from the DMG,
    which runs CLI flags (`--help`, `--list-callbacks`, `-r`, …) but **not** the menu-bar tray/icon —
    that needs the full `.app` bundle. For the complete macOS app, use the **Homebrew cask** above or
    the **direct DMG** instead:

    ```bash
    asdf plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect
    asdf install qmkonnect latest        # CLI only — no menu-bar app
    ```

    ### Launch at login
  WHY: contract deliverable (b) macOS + (d) Homebrew custom-tap caveat. Commands verbatim from the
    homebrew/asdf READMEs. The `--no-quarantine` + `xattr` workarounds come straight from the cask README.

Task 6: VALIDATE (no edits)
  - git diff --stat                         # EXACTLY docs/installation.md (1 file).
  - git diff --name-only | grep -vE 'docs/installation\.md' && echo "FAIL: out-of-scope file" || echo "OK"
  - grep -c "no AUR package" docs/installation.md                 # → 0 (removed).
  - One grep per channel command (see Validation Level 2) → each ≥1.
  - Jekyll front-matter intact: `sed -n '1,4p' docs/installation.md` shows the unchanged 4-line block.
  - Markdown sanity (see Validation Level 1).
  - (Optional) preview the rendered table locally if a Jekyll env is set up — NOT required.

Task 7: NEVER do these (out of scope / forbidden)
  - DO NOT edit any file other than docs/installation.md (README.md = P1.M6.T2.S1; llms_full.txt +
    PACKAGING.md = P1.M6.T2.S2; the .yml CI files = P1.M5.T2.S2).
  - DO NOT renumber/reorder existing headings — insert only, as siblings.
  - DO NOT alter the Jekyll front-matter (lines 1-4) or the `{{ site.baseurl }}` link convention.
  - DO NOT "fix" the pre-existing `v0.1.0` binary-download URL in the Linux section (separate, OOS).
  - DO NOT paraphrase package IDs / URLs — copy them verbatim (dabstractor.QMKonnect,
    github:dabstractor/qmkonnect, mulletware/qmkonnect tap, dabstractor/scoop-qmkonnect,
    dabstractor/asdf-qmkonnect, qmkonnect-bin).
  - DO NOT omit the macOS asdf/mise CLI-only caveat, the Homebrew --no-quarantine caveat, or the
    Winget unverified-publisher caveat — they are explicit contract deliverables.
  - DO NOT list mise/asdf as a Windows option (unsupported there).
  - DO NOT edit PRD.md, tasks.json, prd_snapshot.md, .gitignore, or any source file.
```

> **Correction call-out (Task 4):** the NixOS module is referenced as `qmkonnect.nixosModules.default`
> in newText. The Task-4 body contains a self-correction note because an earlier draft had a typo
> (`qmkonnet…`). When writing the actual edit, use **`qmkonnect.nixosModules.default`** (full, lowercase
> crate name + the flake's `nixosModules.default` output — confirmed by `packaging/nix/README.md`).

### Implementation Patterns & Key Details

````text
# PATTERN (Jekyll page edit — preserve front-matter + Liquid links):
#   docs/installation.md is a GitHub Pages page. Front-matter `---\nlayout/title/permalink\n---` is
#   sacrosanct. Internal links are `{{ site.baseurl }}/foo`. New external links are plain https URLs.
#   Never use relative `./foo.md` links (they break on the rendered site).

# PATTERN (per-platform "Package Managers" subsection — consistent shape across the 3 platforms):
### Package Managers
**<Channel>** (one-line what-it-is) — <exact install command block> — <one-paragraph caveat>.
# Each channel = a bold lead + a fenced command + (if needed) a caveat blockquote/paragraph.

# PATTERN (caveat framing — factual, scoped, links to troubleshooting where relevant):
#   Winget: "not code-signed → 'unverified publisher' + SmartScreen → More info → Run anyway (expected
#            beta state, identical to the unsigned direct installer)."
#   Homebrew: "custom tap (not homebrew-cask) until notarized; ad-hoc/unnotarized DMG → --no-quarantine
#              or xattr; Screen Recording still required."
#   Scoop: "extracts via innounp (doesn't run installer) → autostart OFF, no ARP entry."
#   asdf/mise macOS: "CLI only — raw binary, no menu-bar tray; use Homebrew cask or DMG for the full app."

# WHY plain-text channel names in the top table (no anchors): Jekyll/GitHub auto-generates heading
#   anchors as lowercase-hyphenated; multi-word headings with symbols (e.g. "Package Managers (Scoop ·
#   Winget)") yield fragile anchors. Plain text avoids broken in-page links; readers scroll to the
#   platform section. (The existing doc already uses no in-page anchors.)

# WHY the --- stays BELOW the Linux Package Managers block (Anchor D): the horizontal rule separates
#   ## Linux from ## macOS. Putting the new ### block ABOVE the --- keeps it inside ## Linux while the
#   --- still visually precedes ## macOS.
````

### Integration Points

```yaml
DOCS SITE (docs/installation.md — Jekyll page):
  - front-matter: PRESERVE (layout/title/permalink, lines 1-4)
  - link convention: `{{ site.baseurl }}` internal; plain https external (both already used)
  - new sections: 1 × `## Installation Methods` (top), 3 × `### Package Managers` (under each platform)
DOWNSTREAM (consumes this file — no conflict):
  - P1.M6.T2.S2 regenerates docs/llms_full.txt FROM docs/installation.md (+ other docs/*.md) → it will
    automatically pick up this task's edits. No action needed here; just don't also edit llms_full.txt.
  - P1.M6.T2.S1 edits README.md (separate file) — zero overlap.
PARALLEL (zero conflict):
  - P1.M5.T2.S2 edits .github/workflows/*.yml (CI jobs) — zero docs overlap.
CONSUMES (READ-ONLY inputs — the authoritative install commands):
  - packaging/{linux/aur,nix,homebrew,scoop,winget,asdf}/README.md (verbatim commands + caveats)
  - plan/007_fb356ba503b4/architecture/external_deps.md §2-§4 (the signing/notarization caveats)
  - PRD §5 (the compatibility matrix the summary table mirrors)
PRODUCES:
  - an updated docs/installation.md with all 7 community channels documented (1 file changed).
```

## Validation Loop

> This is a single-file documentation edit on a Linux dev box. The gates prove: scope (1 file), content
> (all 7 channels + caveats present, the stale note gone), and Jekyll integrity (front-matter + links).

### Level 1: Scope + Markdown sanity (runs on Linux)
```bash
cd /home/dustin/projects/qmkonnect
git diff --stat                              # Expected: EXACTLY docs/installation.md (1 file).
git diff --name-only | grep -vE '^docs/installation\.md$' && echo "FAIL: out-of-scope file edited" || echo "OK: only docs/installation.md"
# Markdown heading hierarchy — no orphans / no level jumps:
grep -nE '^#{1,6} ' docs/installation.md | sed -n '1,60p'   # eyeball: ## Installation Methods (new, top), three ### Package Managers (new), no renumbered old headings
# Jekyll front-matter intact (byte-for-byte):
diff <(printf -- '---\nlayout: default\ntitle: Installation\npermalink: /installation/\n---\n') <(sed -n '1,5p' docs/installation.md) && echo "OK: front-matter intact"
# Code fences balanced (every opening ``` has a closing ```):
opens=$(grep -c '^```' docs/installation.md); [ $((opens % 2)) -eq 0 ] && echo "OK: $opens fence lines (balanced)" || echo "FAIL: unbalanced code fences"
```

### Level 2: Content — all 7 channels + caveats present, stale note gone (runs on Linux)
```bash
cd /home/dustin/projects/qmkonnect
echo "stale note removed:";  [ "$(grep -c 'no AUR package' docs/installation.md)" -eq 0 ] && echo "  OK (0 hits)" || echo "  FAIL: still present"
echo "channels present (each must be >=1):"
for pat in 'yay -S qmkonnect-bin' \
           'nix profile install github:dabstractor/qmkonnect' \
           'brew install --cask' \
           'scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnect' \
           'winget install dabstractor.QMKonnect' \
           'asdf plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect' \
           'mise plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect'; do
  n=$(grep -cF "$pat" docs/installation.md); echo "  [$n] $pat"; [ "$n" -ge 1 ] || echo "    !!! MISSING"
done
echo "caveats present (each must be >=1):"
for pat in 'unverified publisher' '--no-quarantine' 'custom tap' 'CLI only'; do
  n=$(grep -ciF "$pat" docs/installation.md); echo "  [$n] $pat"; [ "$n" -ge 1 ] || echo "    !!! MISSING"
done
echo "mise/asdf NOT offered on Windows (the Windows Package Managers block must not list asdf/mise):"
awk '/^### Package Managers/{p=1} /^### Build from Source/{if(p){print "WINDOWS_PM_END"; p=0}} p' docs/installation.md \
  | grep -iE 'asdf|mise' && echo "  WARN: asdf/mise mentioned in Windows PM block (should not be)" || echo "  OK: no asdf/mise in Windows PM block"
# Expected: stale note = 0; all 7 channels >=1; all 4 caveats >=1; no asdf/mise in the Windows block.
```

### Level 3: Anchor fidelity — the 5 edits landed where intended (runs on Linux)
```bash
cd /home/dustin/projects/qmkonnect
grep -nE '^## Installation Methods$' docs/installation.md          # 1 hit, ABOVE `## Windows`
awk '/^## Installation Methods/{a=1} /^## Windows$/{if(a){print "OK: Installation Methods precedes Windows"; a=0}}' docs/installation.md
grep -nE '^### Package Managers$' docs/installation.md             # 3 hits (Windows, Linux, macOS)
# Each ### Package Managers sits under its platform (the nearest preceding ## is the platform):
awk '/^## (Windows|Linux|macOS)/{plat=$2} /^### Package Managers/{print "  ### Package Managers under: " plat}' docs/installation.md
# Expected: under Windows, under Linux, under macOS (in that order).
grep -nE 'Build from the \*\*source\*\* `PKGBUILD`' docs/installation.md   # 1 hit (the reworded Arch line)
! grep -qE 'there is no AUR package' docs/installation.md && echo "OK: stale note removed" || echo "FAIL"
```

### Level 4: Render sanity (optional — only if a Jekyll env is set up; NOT required)
```bash
# The site uses Jekyll (docs/_config.yml, Gemfile). If bundler is available:
# ( cd docs && bundle exec jekyll build -d /tmp/qmkonnect-site 2>&1 | tail -5 ) || echo "(Jekyll not run — optional)"
# Expected: no "Liquid syntax error" / "Markdown" errors referencing installation.md. The new table +
# subsections render. This is OPTIONAL — the grep + fence-balance gates above are the required bar.
```

## Final Validation Checklist

### Technical Validation
- [ ] Level 1: `git diff --stat` = exactly `docs/installation.md`; front-matter intact; code fences balanced.
- [ ] Level 2: stale note = 0; all 7 channel commands ≥1; all 4 caveats ≥1; no asdf/mise in Windows block.
- [ ] Level 3: `## Installation Methods` above `## Windows`; 3 `### Package Managers` (under Windows/Linux/macOS); Arch line reworded.

### Feature (Docs) Validation
- [ ] A reader sees the full channel matrix in the top table, then each platform's exact commands below.
- [ ] The "(there is no AUR package)" claim is gone; the Arch line points at the AUR channel.
- [ ] Every per-channel caveat is present and factual (Winget unverified publisher; Homebrew custom tap +
      `--no-quarantine`; Scoop autostart-off; macOS asdf/mise CLI-only).
- [ ] mise/asdf are NOT advertised as a Windows option.
- [ ] Package IDs/URLs are verbatim (dabstractor.QMKonnect, github:dabstractor/qmkonnect, mulletware/qmkonnect
      tap, dabstractor/scoop-qmkonnect, dabstractor/asdf-qmkonnect, qmkonnect-bin).

### Code Quality Validation
- [ ] Jekyll front-matter (lines 1-4) byte-identical; `{{ site.baseurl }}` link convention preserved.
- [ ] No heading renumbering/reordering — new sections inserted as siblings.
- [ ] Markdown well-formed (balanced fences; valid table; valid nested code/blockquote).
- [ ] Follows the existing doc's tone (factual, second-person, code-block-driven).

### Documentation & Deployment
- [ ] Only `docs/installation.md` changed (1 file) — sibling tasks (README, llms_full, PACKAGING) untouched.
- [ ] Downstream P1.M6.T2.S2 (llms_full regen) will pick up these edits automatically — no hand-sync needed.

---

## Anti-Patterns to Avoid

- ❌ Don't edit any file other than `docs/installation.md` (README/llms_full/PACKAGING/.yml are sibling-owned).
- ❌ Don't renumber or reorder existing headings — insert the new sections as siblings only.
- ❌ Don't alter the Jekyll front-matter or drop the `{{ site.baseurl }}` link convention.
- ❌ Don't paraphrase package IDs/URLs — copy verbatim (a typo = a broken copy-pasted install command).
- ❌ Don't omit the macOS asdf/mise CLI-only caveat or the Homebrew/Winget signing caveats — they're explicit deliverables.
- ❌ Don't list mise/asdf under Windows (unsupported there).
- ❌ Don't "fix" the pre-existing `v0.1.0` binary URL (separate, out-of-scope staleness).
- ❌ Don't add in-page anchor links to the top table (Jekyll anchors from symbol-laden headings are fragile; plain text is safer).
- ❌ Don't create new files (the contract OUTPUT is "Updated docs/installation.md").
- ❌ Don't guess an `edit` anchor if `oldText` doesn't match — re-read the exact current lines and retry.