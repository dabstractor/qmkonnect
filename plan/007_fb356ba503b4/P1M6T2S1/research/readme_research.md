# Research — P1.M6.T2.S1: Update README.md with community distribution channels

> **Deliverable:** ONE file edited — `README.md` (repo root, GitHub-rendered — **not** a Jekyll
> page, **no** front-matter). Add a concise "Package Managers" subsection inside the existing
> `## Installation` section listing all 7 F15 community channels + the direct-installers reference,
> with the key caveats inline and a link to `docs/installation.md` for full detail.
> **Concise is the mandate** (contract: "Keep it concise — full instructions are in
> docs/installation.md"). The detailed per-channel caveats live in the sibling task P1.M6.T1.S1
> (docs/installation.md); the README only mirrors + links.

---

## 1. The file being edited — `README.md` (329 lines, 13 KB)

- **NOT a Jekyll page.** First line is `# QMKonnect` — no `---layout:…---` front-matter. GitHub
  renders it directly from the repo root. Relative links (`docs/installation.md`,
  `packaging/.../README.md`) resolve on GitHub. Anchor links like `docs/installation.md#macos`
  work (GitHub auto-generates heading anchors).
- **Established link convention:** relative paths. Already used at line 127:
  `[macOS install guide](docs/installation.md#macos)` and line 42
  `[Configuration Guide](docs/configuration.md)`. New links MUST follow this (no `{{ site.baseurl }}`
  — that Liquid tag is for the docs/ Jekyll site, NOT the root README).
- **Code fences are balanced:** `grep -c '^```' README.md` → 32 (even). The new block adds a
  markdown table + a blockquote, **zero** code fences — cannot unbalance anything.
- **No existing "Package Managers" / "Distribution Channels" section.** The `## Installation`
  section (line 45) currently covers ONLY direct installers + build-from-source. Community
  channels are entirely absent. (Confirmed: `grep -iE 'scoop|winget|homebrew|brew |yay|nix |asdf|mise' README.md` → 0 hits.)

### Heading structure (grep -nE '^#{1,6} ')
```
1:# QMKonnect
7:## Overview
16:## Features
45:## Installation
47:### Windows
62:### Arch Linux
70:### Other Linux Systems
100:### macOS
107:### From Source
138:## QMK Firmware Setup (REQUIRED)
188:## Configuration
250:## Usage
272:## Technical Requirements
296:## Integration with QMK
305:## Default Configuration
316:## Example Use Cases
323:## Contributing
327:## License
```

### Insertion anchor (line-verified, UNIQUE)
The macOS subsection ends and `### From Source` begins. Insert `### Package Managers` between
them (so the platform direct-installer flows stay intact, community channels are presented as a
unified alternative, then From Source last):

```
4. It starts automatically at login by default — toggle it from the menu-bar icon → **Launch at Login**.    ← line 104 (UNIQUE)

### From Source                                                                                                ← line 106
```
- The em-dash `—` (U+2014) and right-arrow `→` (U+2192) are real UTF-8 bytes in the file
  (`cat -A` shows `M-bM-^@M-^T` and `M-bM-^FM-^R`). The `edit` tool's `oldText` MUST contain the
  actual characters, not the `cat -A` escapes.

---

## 2. The verbatim install matrix (from each channel README — copy these EXACTLY)

| Channel | Platform | Representative install command (verbatim) | One-time tap/bucket/plugin add (full detail in guide) |
|---|---|---|---|
| **AUR** (`qmkonnect-bin`) | Arch Linux | `yay -S qmkonnect-bin` | none (`-bin` downloads the release tarball) |
| **Nix** (flake) | NixOS / Nix | `nix run github:dabstractor/qmkonnect` | none (`nix profile install …` to persist) |
| **Homebrew** (Cask) | macOS | `brew install --cask qmkonnect` | `brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect` |
| **Scoop** (manifest) | Windows | `scoop install qmkonnect` | `scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnect` |
| **Winget** (manifest) | Windows | `winget install dabstractor.QMKonnect` | none (moniker `qmkonnect` also works) |
| **mise** (asdf backend) | Linux (full) · macOS (CLI only) | `mise install qmkonnect@latest` | `mise plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect` |
| **asdf** | Linux (full) · macOS (CLI only) | `asdf install qmkonnect latest` | `asdf plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect` |

### Exact package IDs / URLs — DO NOT "normalize" (a typo = a broken copy-paste)
- **Winget** id = `dabstractor.QMKonnect` (capital **Q**). Moniker = `qmkonnect`.
- **Homebrew** TAP ALIAS = `mulletware/qmkonnect` (user is `mulletware`, **NOT** `dabstractor`),
  but the TAP REPO = `github.com/dabstractor/homebrew-qmkonnect`. Both parts verbatim — the
  mismatch is intentional. Cask token = `qmkonnect` (lowercase).
- **Scoop** bucket = `dabstractor/scoop-qmkonnect`. The `scoop bucket add qmkonnect <URL>` alias
  REQUIRES the explicit URL (bare `scoop bucket add qmkonnect` resolves to the wrong implicit user).
- **Nix** flake ref = `github:dabstractor/qmkonnect` (lowercase org/repo, colon prefix).
- **asdf/mise** plugin repo = `dabstractor/asdf-qmkonnect` (SAME repo serves both managers — mise
  runs asdf plugin scripts unchanged).
- **AUR** package = `qmkonnect-bin` (the `-bin` suffix = prebuilt binary; distinct from the
  source `qmkonnect` PKGBUILD already documented in the README's `### Arch Linux` subsection).

---

## 3. Caveats that MUST appear (concise — one-liners; full detail in docs/installation.md)

From `architecture/external_deps.md` §2-§4 + the channel READMEs:

1. **mise/asdf on macOS = CLI only.** The plugin copies the raw Mach-O binary from the DMG; the
   menu-bar tray/icon DOES NOT WORK (needs the full `.app` bundle). → use Homebrew Cask or the
   direct DMG for the full macOS app. (asdf README "macOS caveat — CLI only".)
2. **mise/asdf NOT available on Windows.** The Windows asset is an Inno installer (`.exe`), not a
   portable binary. → use Scoop / Winget / the Inno installer on Windows.
3. **Homebrew DMG + Windows installer are ad-hoc / unsigned.** Expect a Gatekeeper / SmartScreen
   "unverified publisher" prompt until a stable code-signing certificate lands:
   - Homebrew: `brew install --cask --no-quarantine qmkonnect` (or `xattr -dr com.apple.quarantine`).
   - Winget / Inno: *More info → Run anyway*.
   - Scoop is unaffected (it *extracts* via innounp, doesn't run the installer → no signing check).

---

## 4. Scope boundary — what each sibling owns (do NOT touch)

| File | Owner | This task? |
|---|---|---|
| `README.md` | **P1.M6.T2.S1 (THIS)** | ✅ EDIT (only file) |
| `docs/installation.md` | P1.M6.T1.S1 (parallel — Implementing) | ❌ read-only (link target) |
| `docs/llms_full.txt`, `spec/PACKAGING.md` | P1.M6.T2.S2 (Planned) | ❌ |
| `.github/workflows/*.yml` | P1.M5.T2.S2 | ❌ |
| `packaging/**/README.md` | (already Complete) | ❌ read-only inputs |

**Relationship to P1.M6.T1.S1 (docs/installation.md):** that task adds the *detailed* per-channel
sections (a top `## Installation Methods` table + `### Package Managers` subsections under each
platform, with full caveats + the tap/bucket/plugin commands). The README mirrors it at a glance
and LINKS to `docs/installation.md` for the full detail. **Naming consistency:** both use the
heading `### Package Managers` (so a user reading the README then the guide sees the same label).
**Zero file overlap** (README.md vs docs/installation.md) → safe to run in parallel; the README's
link to `docs/installation.md` is valid regardless of whether T1.S1 has landed (the file already
exists; T1.S1 only enriches it).

---

## 5. Design decision — ONE concise `### Package Managers` subsection

**Why a single consolidated table + caveat blockquote (not per-platform subsections):**
- The contract example lists all 7 commands in ONE list and says "keep it concise."
- Per-platform subsections are what T1.S1 already does in docs/installation.md (the detailed
  version) — duplicating that structure in the README violates "concise" and creates two
  maintenance surfaces that can drift.
- The README's existing `## Installation` already has per-platform direct-installer subsections;
  the community channels are a *complementary alternative* best presented as one scannable block.

**Placement:** AFTER `### macOS` (the last direct-installer platform subsection) and BEFORE
`### From Source`. Rationale: primary direct-install flows first, community channels as a unified
alternative, then build-from-source last. Minimal, surgical, single-anchor edit.

**Format:** a 3-column GitHub markdown table (Platform | Channel | Install command) + a `>`
blockquote with the 3 mandatory caveats. Code spans (`` `…` ``) inside table cells render on
GitHub. Adds **zero** code fences (no fence-balance risk). Two-step channels (Homebrew tap,
Scoop bucket) show the primary install command in the table + a "(custom tap — see guide)"
note rather than cramming two commands into one cell.

**Tone match:** the README uses factual, second-person prose + code fences + tables; the new
block matches. Bold platform/channel names; backtick commands; a blockquote for caveats (the
README already uses blockquotes, e.g. the Windows requirements note at line 47).