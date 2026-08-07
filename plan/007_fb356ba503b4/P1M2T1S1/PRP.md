# PRP — P1.M2.T1.S1: Homebrew Cask formula for QMKonnect.app DMG

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). **Pure packaging — no Rust/source change.**
> **Two new files:** `packaging/homebrew/Casks/qmkonnect.rb` (the cask) + `packaging/homebrew/README.md` (Mode A docs).
> **Scope:** the **cask formula + its docs ONLY**. The tap-repo scaffolding + CI publication job are the **sibling
> P1.M2.T1.S2** — do NOT create tap `.git`/structure or CI workflow edits here.
> **Parallel context:** P1.M1.T2.S2 (in parallel) edits `flake.nix` + `packaging/nix/README.md` (Linux/Nix) — no overlap.

---

## Goal

**Feature Goal**: Provide a valid Homebrew **Cask** (`qmkonnect.rb`) that installs the macOS `QMKonnect.app` DMG
from GitHub releases via a custom tap, with a `livecheck` that auto-detects new versions, a `caveats` block that
warns about Screen Recording permission + the discovery CLI + the unnotarized-build Gatekeeper bypass, and a
`zap` that cleans the per-user config. Plus a Mode-A `README.md` documenting tap setup, install, and the
notarization → official-cask path.

**Deliverable** (2 new files):
1. `packaging/homebrew/Casks/qmkonnect.rb` — a `cask "qmkonnect" do … end` block (CI patches `version` + `sha256`
   on each tagged release and pushes the result to the tap repo `dabstractor/homebrew-qmkonnect`).
2. `packaging/homebrew/README.md` — tap setup (`brew tap …`), install (`brew install --cask [--no-quarantine]
   qmkonnect`), what-it-installs, and the notarization path to the official `homebrew-cask` repo.

**Success Definition**:
- `ruby -c packaging/homebrew/Casks/qmkonnect.rb` → "Syntax OK" (verifiable anywhere Ruby exists).
- On a macOS/Linuxbrew host: `brew audit --cask --new-cask ./packaging/homebrew/Casks/qmkonnect.rb` passes (no
  DSL/token/stanza-order errors). (`--strict` + the real `sha256` are provable only in the tap repo post-CI.)
- The cask `livecheck` resolves `https://github.com/dabstractor/qmkonnect/releases/latest` → `0.2.8` (strips the
  `v`), matching the cask `version`.
- Stanza order conforms to rubocop-cask `StanzaOrder` (version→sha256→url→name→desc→homepage→livecheck→app→zap→caveats).
- The README mirrors the structure of `packaging/linux/aur/README.md` (what-it-is / install / what-it-installs / path-forward).

## User Persona (if applicable)

**Target User**: a macOS user who installs software via Homebrew and wants QMKonnect through their native
package manager instead of the direct DMG download (PRD §5 / F15).

**Use Case**: `brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect` then
`brew install --cask qmkonnect` → QMKonnect.app lands in `/Applications`, autostarts at login (SMAppService),
and `brew upgrade` pulls future releases (livecheck detects them).

**User Journey**: tap → install (see caveat re: Gatekeeper if unnotarized) → launch → grant Screen Recording →
discover app strings with `qmkonnect --show-window-info` → write rules.toml. Upgrade: `brew upgrade --cask qmkonnect`.

**Pain Points Addressed**: native package-manager install + auto-updates on macOS (F15), instead of manual
DMG download/replace. Closes the macOS half of the F15 community-distribution matrix.

## Why

- **F15 (PRD §4) requires a macOS Homebrew channel** alongside the direct DMG (PRD §5: "macOS: `.dmg` (primary)
  · Homebrew Cask"). This cask is that channel's formula.
- **PRD §12 / architecture/external_deps.md §2**: "Homebrew ships via a custom tap (`brew tap mulletware/qmkonnect`)
  until notarization qualifies it for the official cask." This task builds the tap-ready cask + documents that path.
- **Mirrors the proven AUR channel pattern.** The Linux AUR channel (P1.M1.T1, Complete) ships a
  `packaging/linux/aur/` dir with a `PKGBUILD` + `README.md`; this task is the macOS analog (`Casks/qmkonnect.rb` +
  `README.md`). The CI-publishes-to-external-repo model is identical (external_deps.md §"Automation").
- **`livecheck` removes manual version bumps.** A `strategy :header` livecheck against `/releases/latest` means
  `brew livecheck`/`brew bump-cask-pr` can detect new releases automatically (CI still patches the cask file in
  the tap, but livecheck is the discovery mechanism).

## What

### File 1 — `packaging/homebrew/Casks/qmkonnect.rb` (exact content)

```ruby
# frozen_string_literal: true
#
# Homebrew Cask for QMKonnect — distributed via a custom tap
#   brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect
#   brew install --cask qmkonnect
# until notarization qualifies it for the official homebrew-cask repo (PRD §12).
#
# CI (.github/workflows/release.yml) patches `version` and `sha256` on each tagged
# release and pushes this file to the tap repo (architecture/external_deps.md §"Automation").
# The `sha256 :no_check` below is the template placeholder CI overwrites with the
# real `shasum -a 256` of QMKonnect-<version>-macos.dmg.
#
# Validate locally:  brew audit --cask --new-cask ./qmkonnect.rb   (DSL/token/order only)
#                    ruby -c qmkonnect.rb                            (syntax, any host)

cask "qmkonnect" do
  # CI replaces both fields on each tagged release.
  version "0.2.8"
  sha256 :no_check   # template placeholder — CI overwrites with the release DMG's real hash

  url "https://github.com/dabstractor/qmkonnect/releases/download/v#{version}/QMKonnect-#{version}-macos.dmg",
      verified: "github.com/dabstractor/qmkonnect/"

  name "QMKonnect"
  desc "Cross-platform window activity notifier for QMK keyboards"
  homepage "https://github.com/dabstractor/qmkonnect"

  livecheck do
    url "https://github.com/dabstractor/qmkonnect/releases/latest"
    regex(/^v?(\d+(?:\.\d+)+)$/i)
    strategy :header
  end

  app "QMKonnect.app"

  zap trash: [
    "~/Library/Application Support/QMKonnect/",
  ]

  caveats <<~EOS
    QMKonnect needs Screen Recording permission to read window titles. On first
    launch, grant it at System Settings → Privacy & Security → Screen Recording
    (the app runs without it, but sends only app names, not window titles).

    Discover the exact class/title strings for your rules.toml with:
        qmkonnect --show-window-info

    The released DMG is ad-hoc signed (not yet notarized). If macOS Gatekeeper
    blocks the first launch, either right-click the app → "Open", clear the
    quarantine attribute:
        xattr -dr com.apple.quarantine /Applications/QMKonnect.app
    or install with:
        brew install --cask --no-quarantine qmkonnect
  EOS
end
```

### File 2 — `packaging/homebrew/README.md` (content to author)

Structure (mirror `packaging/linux/aur/README.md`):
1. **Title + one-line**: `# qmkonnect — Homebrew Cask (macOS)` — this dir holds the Homebrew Cask for QMKonnect,
   distributed via a custom tap (`dabstractor/homebrew-qmkonnect`).
2. **What this is**: a Cask (GUI `.app` from a `.dmg`); the macOS community channel alongside the primary DMG
   (PRD §5). Per-user install (Homebrew is per-user). Universal binary → one cask for Apple Silicon + Intel.
3. **Install**:
   ```bash
   brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect
   brew install --cask qmkonnect
   # If the build is unnotarized and Gatekeeper blocks first launch:
   brew install --cask --no-quarantine qmkonnect
   ```
   (mention `brew upgrade --cask qmkonnect` for updates; livecheck auto-detects new versions.)
4. **Post-install**: grant Screen Recording (System Settings → Privacy & Security); discover rule strings with
   `qmkonnect --show-window-info`; QMKonnect auto-starts at login (SMAppService, default on — toggle in the app).
5. **What it installs**: `QMKonnect.app` → `/Applications`; per-user config at
   `~/Library/Application Support/QMKonnect/{config.toml,rules.toml}`.
6. **Uninstall**: `brew uninstall --cask qmkonnect` (+ `--zap` to also remove `~/Library/Application Support/QMKonnect/`).
7. **Path to the official cask**: PRD §12 — once the DMG is Developer-ID-signed + notarized, this cask graduates
   from the custom tap to the official `Homebrew/homebrew-cask` repo (the `sha256`/`url`/`livecheck` here carry
   over unchanged; only the distribution channel changes). Link the relevant docs.
8. Cross-links: `docs/installation.md`, the source `packaging/macos/` build scripts, and the CI release workflow.

### Success Criteria
- [ ] `packaging/homebrew/Casks/qmkonnect.rb` exists with the exact stanzas above (order: version→sha256→url→
      name→desc→homepage→livecheck→app→zap→caveats).
- [ ] `ruby -c` on the cask → "Syntax OK".
- [ ] `packaging/homebrew/README.md` documents tap setup, install (+ `--no-quarantine`), post-install, uninstall,
      and the notarization → official-cask path.
- [ ] No Rust/source/Cargo change; no CI workflow edit; no tap-repo scaffolding (those are the sibling S2 / P1.M5).

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge can create both files verbatim from the "What" section (the full cask
source is given, and the README is specced section-by-section), plus run `ruby -c` and the documented `brew audit`.

### Documentation & References

```yaml
# MUST READ — authoritative Homebrew DSL (external)
- url: https://docs.brew.sh/Cask-Cookbook
  why: stanza order (rubocop-cask StanzaOrder — the `brew audit --cask --strict` gate), required stanzas,
       `sha256 :no_check` special value, `zap trash:`/`rmdir:`, `caveats` heredoc, `verified:` for GitHub URLs
  critical: stanza order is version→sha256→url→name→desc→homepage→livecheck→…→app→zap→caveats (zap BEFORE caveats)
- url: https://docs.brew.sh/Brew-Livecheck
  why: `strategy :header` follows the redirect and matches the Location header against `regex`; GitHub
       `/releases/latest` → 302 → `/releases/tag/vX.Y.Z` → the explicit regex strips the `v`
- url: https://docs.brew.sh/Adding-Software-to-Homebrew
  why: cask token rules + required stanzas for a new cask

# MUST READ — the release artifact this cask consumes (verified)
- file: .github/workflows/release.yml
  why: confirms DMG asset name `QMKonnect-${version}-macos.dmg` (L84), universal build `MACOS_UNIVERSAL=1` (L65),
       and that notarization (notarytool+stapler, L76-80) runs ONLY when secrets are set (default = ad-hoc signed)
  gotcha: version comes from `cargo metadata` with NO `v` prefix; the release TAG is `v{version}` (URL uses v{version})

# MUST READ — the architecture decision this implements
- docfile: plan/007_fb356ba503b4/architecture/external_deps.md
  why: §2 Homebrew — Cask DSL requirements, livecheck `strategy :header`, "custom tap until notarized for official
       cask", "push cask file to homebrew tap repo on tag" (CI); §"Automation" describes the version/sha256 CI patch
  section: "2. Homebrew Cask (macOS)" + "Automation (version/hash patching)" + "For channels requiring repo pushes"

# MUST READ — repo packaging-doc conventions to mirror
- file: packaging/linux/aur/README.md
  why: the structure to follow for the Homebrew README (what-it-is / install / what-it-installs / conventions / path-forward)
  pattern: "title + one-line → What this is → Install (bash) → What it installs → cross-links"
- file: packaging/macos/build.sh
  why: confirms bundle id `io.mulletware.qmkonnect` (L42-44), ad-hoc codesign default (L68), DMG UDZO + /Applications symlink
- file: packaging/macos/uninstall.sh
  why: confirms the per-user path `~/Library/Application Support/QMKonnect/` (L26) → the exact `zap trash` target
- file: src/platforms/macos.rs
  why: confirms config dir `~/Library/Application Support/QMKonnect/` (L469/492) — the zap target is accurate

# REFERENCE — PRD sections the cask embodies
- url: spec/PRD.md (heading h2.4 "Supported Platforms" → macOS row: ".dmg (primary) · Homebrew Cask"; Screen Recording for titles)
- url: spec/PRD.md (heading h2.3 / F15 "Community package-manager distribution" → macOS: Homebrew cask)
- url: spec/PRD.md (heading h2.76 "macOS Packaging" → bundle id, codesign ad-hoc vs Developer ID, DMG build)
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
packaging/                     # existing channels (read-only precedent)
  linux/aur/                   # AUR: PKGBUILD + README.md + .SRCINFO + publish.sh  (P1.M1.T1, Complete)
  nix/                         # Nix: flake.nix + README.md                          (P1.M1.T2)
  macos/{build.sh,clean.sh,install.sh,uninstall.sh,Icon.icns}   # the DMG producer this cask consumes
  windows/inno/                # Inno installer
.github/workflows/release.yml  # CI: builds + renames QMKonnect-<v>-macos.dmg; optional notarization
# NEW (this task):
packaging/homebrew/
  Casks/qmkonnect.rb           # the cask (CI patches version/sha256 → pushes to tap repo)
  README.md                    # tap setup + install + notarization path (Mode A docs)
```

### Desired Codebase tree (files this task ADDS)

```bash
packaging/homebrew/
├── Casks/
│   └── qmkonnect.rb           # Homebrew cask formula (source of truth; CI → tap repo)
└── README.md                  # tap/install/uninstall docs + notarization → official cask path
```
(No other files. The tap repo `dabstractor/homebrew-qmkonnect` itself + CI publication are the sibling S2 / P1.M5.T1.S2.)

### Known Gotchas of our codebase & Library Quirks
```ruby
# CRITICAL (stanza order): `brew audit --cask --strict` enforces rubocop-cask StanzaOrder. The required order is
# version → sha256 → url → name → desc → homepage → livecheck → (depends_on/conflicts_with — omit) → app (artifact)
# → uninstall (omit) → zap → caveats. The FINAL group is [zap, caveats]: zap BEFORE caveats. Getting this wrong = audit fail.

# CRITICAL (livecheck v-prefix): GitHub /releases/latest 302→/releases/tag/v0.2.8. The bare `strategy :header`
# default extraction is NOT guaranteed to strip the leading `v`, which would make livecheck report `v0.2.8` ≠ the
# cask `version "0.2.8"`. Add the explicit `regex(/^v?(\d+(?:\.\d+)+)$/i)` (capture group 1 = "0.2.8"). This is a
# hardening of the contract's `strategy :header` — keep BOTH the regex and the strategy.

# CRITICAL (unnotarized DMG): the released DMG is ad-hoc signed UNLESS CI notarization secrets are set (release.yml:76-80
# gated). An ad-hoc-signed app is quarantined by Homebrew and Gatekeeper blocks first launch. The caveats + README MUST
# tell the user: `brew install --cask --no-quarantine qmkonnect` OR `xattr -dr com.apple.quarantine /Applications/QMKonnect.app`
# (or right-click → Open). Omitting this = "it won't launch" support tickets.

# GOTCHA (sha256 placeholder): `sha256 :no_check` is the Cask-Cookbook-documented special value. It's the cleanest
# CI-replaceable target (sed `^sha256 :no_check$` → `sha256 "<hash>"`). `brew audit --cask` (non-strict) accepts it;
# `--strict` flags it — but `--strict` + the real hash run in the TAP repo after CI substitutes the hash, not here.

# GOTCHA (universal binary): MACOS_UNIVERSAL=1 (release.yml:65) → one DMG for aarch64 + x86_64. Do NOT add
# `depends_on arch:` — a single cask serves both Apple Silicon and Intel.

# GOTCHA (zap accuracy): the ONLY confirmed per-user path is `~/Library/Application Support/QMKonnect/`
# (config.toml + rules.toml; macos.rs:469/492, uninstall.sh:26). LSUIElement menu-bar app, no UserDefaults/plist.
# Keep `zap trash:` to that path — do NOT invent bundle-id plist/caches paths that don't exist.

# GOTCHA (verified: stanza): GitHub release URLs need `verified: "github.com/dabstractor/qmkonnect/"` so `brew audit`
# doesn't flag the download as unverified. (Homebrew auto-trusts github.com, but the explicit stanza is belt-and-suspenders
# and matches the Cask Cookbook guidance for per-repo release URLs.)

# GOTCHA (env-gated validation): `brew` may not be installed on the authoring box. `ruby -c` (syntax) works anywhere;
# the `brew audit --cask --new-cask` DSL gate needs a macOS/Linuxbrew host. The real checksum + `--strict` are only
# provable in the tap repo after CI fills sha256. (Mirrors the Nix PRP's env-gated pattern.)
```

## Implementation Blueprint

### Data models and structure
No data models — this is a Ruby cask file + a Markdown doc. The cask is a declarative DSL; CI patches two scalar
fields (`version`, `sha256`) per release.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: CREATE packaging/homebrew/Casks/qmkonnect.rb
  - IMPLEMENT: the exact `cask "qmkonnect" do … end` block from the "What → File 1" section (copy verbatim).
  - STANZA ORDER (load-bearing): version, sha256 (:no_check), url (+verified:), name, desc, homepage,
    livecheck (url + regex + strategy :header), app "QMKonnect.app", zap trash: [...], caveats heredoc.
  - FOLLOW pattern: Homebrew Cask Cookbook stanza order (docs.brew.sh/Cask-Cookbook) — do NOT reorder.
  - NAMING: file `qmkonnect.rb`, cask token `qmkonnect` (lowercase, matches repo/app name; no special chars).
  - PLACEMENT: packaging/homebrew/Casks/qmkonnect.rb (the Casks/ subdir is the Homebrew tap convention).
  - DEPENDENCIES: none (pure declarative file).

Task 2: VALIDATE the cask (env-gated)
  - RUN (any host with ruby):  ruby -c packaging/homebrew/Casks/qmkonnect.rb   → "Syntax OK"
  - RUN (macOS/Linuxbrew host): brew audit --cask --new-cask ./packaging/homebrew/Casks/qmkonnect.rb
    Expected: no DSL/token/stanza-order errors. (--strict + real sha256 are the TAP repo's gate post-CI.)
  - IF brew is unavailable on the authoring box: rely on `ruby -c` + the line-by-line stanza-order check in the
    Gotchas; defer the `brew audit` to a macOS host (document this in the session report).

Task 3: CREATE packaging/homebrew/README.md
  - IMPLEMENT: the 8 sections listed in "What → File 2" (title → what-this-is → install [incl. --no-quarantine]
    → post-install → what-it-installs → uninstall → path-to-official-cask → cross-links).
  - FOLLOW pattern: packaging/linux/aur/README.md (structure + tone + cross-link style to ../../.github/workflows/release.yml).
  - PLACEMENT: packaging/homebrew/README.md.

Task 4: NEVER do these (out of scope / forbidden)
  - DO NOT create the tap repo scaffolding (homebrew-qmkonnect .git/, Casks/ in a separate repo, README there) —
    that's sibling P1.M2.T1.S2.
  - DO NOT edit .github/workflows/release.yml or any CI (Homebrew publication CI is P1.M5.T1.S2).
  - DO NOT change any Rust source / Cargo.toml / docs/* (the docs/installation.md + README.md top-level updates
    are P1.M6.T1.S1 / P1.M6.T2.S1 — this task's docs live ONLY in packaging/homebrew/).
  - DO NOT add `depends_on arch:` (universal binary) or invent zap paths not in the codebase.
  - DO NOT drop the `regex(...)` from livecheck (it's the v-prefix fix) or the Gatekeeper caveat.
  - DO NOT edit PRD.md, any tasks.json, or prd_snapshot.md.
```

### Implementation Patterns & Key Details
```ruby
# The two CI-patched fields are intentionally trivially-regexable:
#   version "0.2.8"          →  ^version ".*"$         (CI: new version from cargo metadata)
#   sha256 :no_check         →  ^sha256 :no_check$      (CI: → sha256 "<shasum -a 256 of the DMG>")
# CI pushes the patched file to dabstractor/homebrew-qmkonnect (external_deps.md §"Automation").

# livecheck idiom (GitHub /releases/latest with v-stripping):
#   livecheck do
#     url "https://github.com/dabstractor/qmkonnect/releases/latest"
#     regex(/^v?(\d+(?:\.\d+)+)$/i)   # 302 → /releases/tag/v0.2.8 → captures "0.2.8"
#     strategy :header
#   end

# caveats heredoc form (multi-line, indented via <<~EOS … EOS) — required for the 3 heads-up messages.
```

### Integration Points
```yaml
RELEASE ARTIFACT: QMKonnect-<version>-macos.dmg (release.yml:84), universal (MACOS_UNIVERSAL=1), ad-hoc OR notarized.
TAP REPO:         dabstractor/homebrew-qmkonnect (created/maintained by sibling S2; this task just authors the cask).
CI PATCH:         release.yml (P1.M5.T1.S2) will sed version+sha256 → git push to the tap on tag.
DOCS SYNC:        packaging/homebrew/README.md (this task) → referenced later by docs/installation.md (P1.M6.T1.S1).
PARALLEL:         P1.M1.T2.S2 edits flake.nix + packaging/nix/README.md — no overlap.
```

## Validation Loop

### Level 1: Syntax (Immediate — any host with Ruby)
```bash
cd /home/dustin/projects/qmkonnect
ruby -c packaging/homebrew/Casks/qmkonnect.rb
# Expected: "Syntax OK". (If ruby is absent, install via system pkg manager or skip to Level 2's host.)
```

### Level 2: Cask DSL Audit (macOS / Linuxbrew host)
```bash
cd /home/dustin/projects/qmkonnect
brew audit --cask --new-cask ./packaging/homebrew/Casks/qmkonnect.rb
# Expected: no errors (DSL structure, token validity, stanza order, required stanzas all pass).
# NOTE: `--strict` is NOT expected to pass here — it requires the real sha256 + a downloadable DMG, which exist
# only in the TAP repo after CI substitutes the hash. The non-strict `--new-cask` audit is this template's gate.
# If `brew` is unavailable on the authoring box, defer this to a macOS host and note it in the session report.
```

### Level 3: livecheck resolution (macOS / Linuxbrew host, optional)
```bash
# Only meaningful once a v0.2.8 release + DMG exist; otherwise livecheck has nothing to follow.
brew livecheck --cask ./packaging/homebrew/Casks/qmkonnect.rb
# Expected (post-release): reports "qmkonnect: 0.2.8" (the regex strips the `v` from the /releases/latest redirect).
```

### Level 4: Documentation review
```bash
# Verify the README covers: tap setup, install (+--no-quarantine), post-install (Screen Recording + --show-window-info),
# what-it-installs, uninstall, and the notarization → official-cask path. Cross-links resolve (../../.github/workflows/release.yml).
grep -nE 'brew tap|brew install --cask|--no-quarantine|Screen Recording|--show-window-info|notariz|homebrew-cask' \
  packaging/homebrew/README.md
# Expected: at least one hit for each term (the 7 documented heads-ups).
```

## Final Validation Checklist

### Technical Validation
- [ ] `ruby -c packaging/homebrew/Casks/qmkonnect.rb` → "Syntax OK".
- [ ] (macOS/Linuxbrew) `brew audit --cask --new-cask …/qmkonnect.rb` → no DSL/order errors.
- [ ] Stanza order: version→sha256→url→name→desc→homepage→livecheck→app→zap→caveats (zap before caveats).
- [ ] livecheck has BOTH `regex(/^v?(\d+(?:\.\d+)+)$/i)` AND `strategy :header`.
- [ ] `git status` shows ONLY the two new files under `packaging/homebrew/`.

### Feature Validation
- [ ] `url` points at `releases/download/v#{version}/QMKonnect-#{version}-macos.dmg` with `verified:`.
- [ ] `app "QMKonnect.app"` matches the DMG's bundle (build.sh assembles QMKonnect.app at the DMG root).
- [ ] `zap trash:` includes `~/Library/Application Support/QMKonnect/` (the confirmed per-user path).
- [ ] `caveats` covers Screen Recording + `--show-window-info` + the unnotarized Gatekeeper bypass.
- [ ] README documents tap setup, install (+`--no-quarantine`), uninstall, and the official-cask path.

### Code Quality Validation
- [ ] Cask mirrors the Cask Cookbook conventions (token, stanza order, heredoc caveats, `verified:`).
- [ ] README mirrors `packaging/linux/aur/README.md` structure/tone.
- [ ] No invented zap paths; no `depends_on arch:` (universal binary).
- [ ] `# frozen_string_literal: true` magic comment at the top of the cask (Homebrew convention).

### Documentation & Deployment
- [ ] README cross-links to `docs/installation.md`, `packaging/macos/`, and `release.yml`.
- [ ] Inline comments in the cask explain the `:no_check` placeholder + the CI patch for future maintainers.

---

## Anti-Patterns to Avoid
- ❌ Don't reorder stanzas — `brew audit --cask --strict` enforces the fixed order (zap before caveats; artifacts
  after homepage/livecheck; `depends_on` between livecheck and artifacts).
- ❌ Don't drop the `regex(...)` from livecheck — the bare `strategy :header` may report `v0.2.8` (with the `v`),
  breaking version match. Keep both.
- ❌ Don't omit the Gatekeeper/no-quarantine caveat for an unnotarized DMG — users hit "app is damaged / can't be opened".
- ❌ Don't add `depends_on arch:` — the DMG is universal (aarch64 + x86_64).
- ❌ Don't invent `zap trash:` paths (no bundle-id plist/caches exist for this LSUIElement app) — use only the
  confirmed `~/Library/Application Support/QMKonnect/`.
- ❌ Don't create the tap repo / CI publication here — that's sibling S2 / P1.M5.T1.S2.
- ❌ Don't use a fake-looking 64-hex `sha256` placeholder — use the documented `:no_check` (honest + CI-replaceable).
- ❌ Don't edit PRD.md, tasks.json, prd_snapshot.md, any Rust source, Cargo.toml, or docs/* outside packaging/homebrew/.

---

## Confidence Score: 8/10

The cask is fully specified verbatim (copy-paste ready), the release-artifact facts are verified in `release.yml`,
and the zap target is confirmed in the source. Score is 8 (not 9-10) for two reasons: (1) the definitive gate
(`brew audit --cask --strict` + real `sha256` + `brew livecheck`) requires a macOS/Linuxbrew host AND a published
release DMG, neither guaranteed on the authoring box — so one-pass success leans on `ruby -c` + careful textual
review until the tap/CI lands (sibling S2); and (2) two contract hardenings were applied (the livecheck `regex`
for `v`-stripping, and the Gatekeeper caveat for unnotarized builds) that the implementer must keep rather than
"revert to the literal contract." Both risks are explicitly mitigated above.