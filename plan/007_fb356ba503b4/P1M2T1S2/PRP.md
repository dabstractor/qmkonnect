# PRP — P1.M2.T1.S2: Homebrew tap repo structure + update automation

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). **Pure packaging — no Rust/source/CI change.**
> **Two new files:** `packaging/homebrew/tap-README.md` (the README for the `homebrew-qmkonnect` tap REPO) +
> `packaging/homebrew/update-cask.sh` (download DMG → hash → patch version/sha256 → `brew audit`).
> **Scope:** the tap-repo scaffolding + update script ONLY. The cask formula itself is **sibling P1.M2.T1.S1**
> (consuming its `Casks/qmkonnect.rb` output — do NOT edit it); the CI push job is **P1.M5.T1.S2** (this task
> only DOCUMENTS the deploy key).
> **Parallel context:** P1.M2.T1.S1 (parallel) authors `Casks/qmkonnect.rb` + `packaging/homebrew/README.md`.
> This task treats S1's cask as a CONTRACT input and never duplicates it.

---

## Goal

**Feature Goal**: Stand up the **tap-repo scaffolding** (its README) and the **cask-update automation
script** so that (a) the `dabstractor/homebrew-qmkonnect` tap repo has correct user-facing install docs +
maintainer cask-audit steps, and (b) every tagged release can be mechanically rolled into the cask
(download DMG, `shasum -a 256`, patch `version`+`sha256`, `brew audit`) — ready for the CI publish job
(P1.M5.T1.S2) to clone the tap, run this script, and push.

**Deliverable** (2 new files):
1. `packaging/homebrew/tap-README.md` — the README.md content for the `homebrew-qmkonnect` tap repo: what
   the tap is, the install command (with the explicit `brew tap mulletware/qmkonnect <url>` form), what it
   installs, the cask audit steps, and the deploy-key CI-publish note.
2. `packaging/homebrew/update-cask.sh` — a portable (`set -euo pipefail`) bash script: args `<version>
   [sha256]`; downloads `QMKonnect-<version>-macos.dmg` from the GitHub release (or accepts a pre-computed
   sha256), computes SHA256 via `shasum -a 256`/`sha256sum`, BSD+GNU-sed-portable patches the cask's
   `version`+`sha256` lines, best-effort `brew audit --cask`. Header documents the deploy key for CI.

**Success Definition**:
- `bash -n packaging/homebrew/update-cask.sh` → syntax OK; `shellcheck` (if available) → no errors.
- `./update-cask.sh` with no args → prints usage and exits non-zero (clean arg validation).
- `./update-cask.sh --help` → prints the documented header.
- `tap-README.md` contains the exact tap command (`brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect`), `brew install --cask qmkonnect`, and the `brew audit --cask` maintainer steps.
- The script's sed patches the S1 cask's exact lines (`  version "…"` / `  sha256 :no_check`) without
  disturbing any other stanza (verified by a dry-run patch + `ruby -c` + `git diff`).
- No edit to S1's `Casks/qmkonnect.rb` or `packaging/homebrew/README.md`; no CI/Rust/Cargo change.

## User Persona (if applicable)

**Target User**: (1) A macOS end-user installing QMKonnect via the tap (reads the tap-README on GitHub).
(2) A maintainer/releasing CI rolling a new version into the cask (runs update-cask.sh).

**Use Case**: On a tag push, the release workflow (P1.M5.T1.S2) runs `update-cask.sh <version>` to patch
the cask, copies it into a clone of `dabstractor/homebrew-qmkonnect` using a deploy key, commits, pushes.
End users then `brew upgrade --cask qmkonnect` (the cask's livecheck already detected the new version).

**User Journey**: tag `v0.2.9` → GitHub release publishes `QMKonnect-0.2.9-macos.dmg` → CI calls
`update-cask.sh 0.2.9` (downloads DMG, hashes it, patches the cask, audits) → CI pushes the patched cask
to the tap → `brew upgrade` picks it up.

**Pain Points Addressed**: removes manual cask editing per release (the script is mechanical + idempotent);
gives the tap repo proper install/audit docs so users and contributors aren't guessing.

## Why

- **F15 (PRD §4) requires a Homebrew channel**; the cask (S1) is the formula, but a tap needs (a) a
  discoverable README with the correct `brew tap` command and (b) automation to keep `version`/`sha256`
  current. This task delivers both.
- **PRD §12 / external_deps.md §2**: "Homebrew ships via a custom tap until notarization qualifies it for
  the official cask." The tap-README documents that path; the update script is the mechanical bridge from
  a tagged GitHub release to a tap-repo cask commit.
- **Mirrors the proven AUR pattern** (`packaging/linux/aur/publish.sh`, P1.M1.T1.S2 Complete): same
  "patch manifest fields from the release artifact, validate, document the CI push key" shape, adapted to
  Homebrew (cask `version`/`sha256` + `brew audit` instead of PKGBUILD `pkgver`/`sha256sums` + `makepkg`).
- **Deliberate scope split:** per the contract, `update-cask.sh` does the **local file update + validate**
  (steps a–d) and **documents** the deploy key; the actual tap-repo git push is the CI job P1.M5.T1.S2.
  This keeps the script testable anywhere (no tap-repo access needed) and the push a separate concern.

## What

### File 1 — `packaging/homebrew/tap-README.md` (content to author)

> This file is the **README.md of the `dabstractor/homebrew-qmkonnect` tap repo** (it will be copied there
> verbatim by CI / a maintainer). It is DISTINCT from `packaging/homebrew/README.md` (S1's source-repo
> packaging doc). Author it section-by-section:

1. **Title + one-line**: `# homebrew-qmkonnect` — Custom Homebrew tap for [QMKonnect](https://github.com/dabstractor/qmkonnect),
   distributing the macOS Cask until the DMG is notarized and graduates to the official `homebrew-cask` repo (PRD §12).
2. **What this is**: a Homebrew **tap** (a repo named `homebrew-<name>`) holding `Casks/qmkonnect.rb`. The
   QMKonnect `.app` ships as a `.dmg`; this tap is the macOS community channel alongside the primary DMG
   download (PRD §4 F15, §5). Per-user (Homebrew is per-user). Universal binary → one cask for Apple Silicon + Intel.
3. **Install** (the EXACT command — the explicit URL resolves the `mulletware` alias → `dabstractor` repo):
   ```bash
   brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect
   brew install --cask qmkonnect
   # If the build is unnotarized and Gatekeeper blocks first launch:
   brew install --cask --no-quarantine qmkonnect
   ```
   Mention `brew upgrade --cask qmkonnect` (the cask's livecheck auto-detects new versions).
4. **What it installs**: `QMKonnect.app` → `/Applications`; per-user config at
   `~/Library/Application Support/QMKonnect/{config.toml,rules.toml}`. Auto-starts at login (SMAppService, default on).
5. **Uninstall**: `brew uninstall --cask qmkonnect` (+ `--zap` to also remove `~/Library/Application Support/QMKonnect/`).
6. **For maintainers — updating the cask** (the cask audit steps):
   ```bash
   # From a clone of THIS tap repo:
   ruby -c Casks/qmkonnect.rb                        # syntax check (any host with ruby)
   brew audit --cask --new-cask Casks/qmkonnect.rb   # DSL / stanza-order (macOS or Linuxbrew)
   # --strict + the real sha256 are only provable here AFTER CI fills the hash from a published release.
   ```
   Point to the source-repo update script: `packaging/homebrew/update-cask.sh <version>` downloads the
   release DMG, computes its SHA256, and patches `version`+`sha256` into the cask.
7. **CI publishing (deploy key)**: new releases are pushed to this tap automatically by the QMKonnect
   release workflow. It uses a GitHub **deploy key** (SSH, write access) for this repo, stored as the
   `HOMEBREW_TAP_DEPLOY_KEY` secret in `dabstractor/qmkonnect`. See `update-cask.sh` header + the source
   repo's `architecture/external_deps.md` §"CI Publishing Strategy". (Wired in P1.M5.T1.S2.)
8. **Path to the official cask**: once the DMG is Developer-ID-signed + notarized (PRD §12), this cask
   graduates to `Homebrew/homebrew-cask`; the `version`/`url`/`sha256`/`livecheck` carry over unchanged.
9. Cross-links: the source repo (`https://github.com/dabstractor/qmkonnect`), its `docs/installation.md`,
   and `packaging/macos/` build scripts.

### File 2 — `packaging/homebrew/update-cask.sh` (exact content)

```bash
#!/usr/bin/env bash
# Update the QMKonnect Homebrew cask (Casks/qmkonnect.rb) for a new release.
#
# Usage:
#   ./update-cask.sh <version>            # download the release DMG, compute SHA256, patch + audit
#   ./update-cask.sh <version> <sha256>   # use a pre-computed SHA256 (skip the download)
#   ./update-cask.sh --help
#
# Example:
#   ./update-cask.sh 0.2.8
#   ./update-cask.sh 0.2.8 86dcaa57254f4b7dd23595ef8e473432c3997df3b6c12621913d215b9ab1b216
#
# Steps:
#   1. (unless a sha256 arg is given) download QMKonnect-<version>-macos.dmg from the GitHub
#      release and compute its SHA256 (`shasum -a 256`, falling back to `sha256sum`).
#   2. Patch the `version` and `sha256` lines in Casks/qmkonnect.rb (2-space-indented stanzas).
#   3. Validate with `brew audit --cask --new-cask` if `brew` is available (best-effort).
#
# This script is a PURE local file update — it does NOT push to the tap repo. The actual tap
# publication is the CI job P1.M5.T1.S2, which runs this script against the source checkout,
# copies the patched cask into a clone of dabstractor/homebrew-qmkonnect, and pushes.
#
# DEPLOY KEY (CI publishing — wired in P1.M5.T1.S2):
#   The tap repo dabstractor/homebrew-qmkonnect is pushed via a GitHub deploy key (SSH, write
#   access). Generate a key pair, add the PUBLIC half to the tap repo (Settings → Deploy keys),
#   and store the PRIVATE half as the HOMEBREW_TAP_DEPLOY_KEY Actions secret in dabstractor/qmkonnect.
#   CI loads it into ssh-agent, then: git clone git@github.com:dabstractor/homebrew-qmkonnect.git,
#   run this script, cp Casks/qmkonnect.rb into the clone, commit, push. (Mirrors the AUR SSH-key
#   model — see packaging/linux/aur/publish.sh + architecture/external_deps.md §"CI Publishing Strategy".)
#
# Prerequisites:
#   * The GitHub release for <version> must already be published (step 1 downloads the DMG).
#   * `curl`, `awk`, and (`shasum` | `sha256sum`) must be on PATH. `brew` is optional (audit is
#     best-effort). `ruby` is needed only if you additionally run `ruby -c` yourself.
#   * Portable across macOS (BSD sed; ships shasum, NOT sha256sum) and Linux (GNU sed; sha256sum).
set -euo pipefail

CASK_REPO="dabstractor/qmkonnect"
TAP_REPO="dabstractor/homebrew-qmkonnect"

usage() { sed -n '2,40p' "${BASH_SOURCE[0]:-$0}"; }

VERSION=""
SHA256=""
for a in "$@"; do
    case "$a" in
        -h|--help) usage; exit 0 ;;
        -*) echo "Unknown option: $a" >&2; exit 1 ;;
        *)
            if [ -z "$VERSION" ]; then VERSION="$a"
            elif [ -z "$SHA256" ]; then SHA256="$a"
            else echo "Unexpected extra argument: $a" >&2; exit 1
            fi
            ;;
    esac
done

if [ -z "$VERSION" ]; then
    echo "Usage: $0 <version> [sha256]   (e.g. 0.2.8)" >&2
    exit 1
fi

# Reject version strings with a leading 'v' (release TAGS are v-prefixed; cask versions are not).
case "$VERSION" in
    v*) echo "ERROR: version must not have a leading 'v' (got '$VERSION'). Use '$(echo "$VERSION" | sed 's/^v//')'." >&2; exit 1 ;;
esac

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CASK="$SCRIPT_DIR/Casks/qmkonnect.rb"
[ -f "$CASK" ] || { echo "ERROR: cask not found at $CASK" >&2; exit 1; }

DMG_URL="https://github.com/${CASK_REPO}/releases/download/v${VERSION}/QMKonnect-${VERSION}-macos.dmg"

# --- Step 1: obtain the SHA256 (download+hash, or use the provided arg) ---
if [ -n "$SHA256" ]; then
    echo "==> Using provided SHA256 for v${VERSION} (skipping download)"
else
    echo "==> Downloading ${DMG_URL}"
    WORK="$(mktemp -d)"
    trap 'rm -rf "$WORK"' EXIT
    DMG="$WORK/QMKonnect-${VERSION}-macos.dmg"
    curl -fL "$DMG_URL" -o "$DMG"
    if command -v shasum >/dev/null 2>&1; then
        SHA256="$(shasum -a 256 "$DMG" | awk '{print $1}')"
    elif command -v sha256sum >/dev/null 2>&1; then
        SHA256="$(sha256sum "$DMG" | awk '{print $1}')"
    else
        echo "ERROR: need 'shasum' or 'sha256sum' on PATH" >&2; exit 1
    fi
fi

# Sanity: a SHA256 is 64 lowercase hex chars.
case "$SHA256" in
    [0-9a-f]{64}) ;;
    *) echo "ERROR: sha256 looks malformed: '$SHA256'" >&2; exit 1 ;;
esac
echo "    version -> ${VERSION}"
echo "    sha256  -> ${SHA256}"

# --- Step 2: patch version + sha256 in the cask (BSD+GNU sed portable) ---
# S1's cask stanzas are 2-space-indented inside `cask "qmkonnect" do … end`:
#     version "0.2.8"
#     sha256 :no_check
patch_line() {  # <regex> <replacement> <file>
    sed -i.bak -E -e "s|${1}|${2}|" "$3" && rm -f "$3.bak"
}
patch_line '^  version .*$'  "  version \"${VERSION}\""  "$CASK"
patch_line '^  sha256 .*$'   "  sha256 \"${SHA256}\""    "$CASK"

# Confirm both stanzas landed (guards against a future stanza reorder silently no-oping the sed).
grep -q "^  version \"${VERSION}\"" "$CASK" || { echo "ERROR: version stanza not patched" >&2; exit 1; }
grep -q "^  sha256 \"${SHA256}\""   "$CASK" || { echo "ERROR: sha256 stanza not patched" >&2; exit 1; }
echo "    patched $CASK"

# --- Step 3: best-effort `brew audit` (macOS/Linuxbrew only) ---
if command -v brew >/dev/null 2>&1; then
    echo "==> brew audit --cask --new-cask $CASK"
    brew audit --cask --new-cask "$CASK"
else
    echo "==> (brew not found; skipping audit — run on a macOS/Linuxbrew host)"
fi

echo "==> Done. Next (CI, P1.M5.T1.S2): copy $CASK into a clone of ${TAP_REPO} and push."
```

### Success Criteria
- [ ] `packaging/homebrew/tap-README.md` exists with sections 1–9 above; contains the exact
      `brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect` command.
- [ ] `packaging/homebrew/update-cask.sh` exists, is `chmod +x`, and matches the script above.
- [ ] `bash -n update-cask.sh` → OK; `update-cask.sh` (no args) → usage + non-zero; `--help` → header.
- [ ] The sed targets the S1 cask's exact 2-space-indented `version`/`sha256` lines; post-patch `ruby -c` OK.
- [ ] No edit to `Casks/qmkonnect.rb` content semantics (the script patches it at RUNTIME only; do not
      hand-edit S1's file), `packaging/homebrew/README.md`, any CI workflow, or any Rust/Cargo/docs file.

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge can create both files verbatim from the "What" section (tap-README
is specced section-by-section; update-cask.sh is given in full), run `bash -n` + `--help`, and dry-run the
sed against S1's cask — all using only this PRP + the codebase.

### Documentation & References

```yaml
# MUST READ — authoritative Homebrew tap + cask docs (external)
- url: https://docs.brew.sh/Taps
  why: a tap is a repo named `homebrew-<name>`; `brew tap <user>/<name> <url>` explicit-URL form; Casks/ layout
  critical: "brew tap mulletware/qmkonnect" alone would look for github.com/mulletware/homebrew-qmkonnect —
            the explicit <url> arg (dabstractor/homebrew-qmkonnect) is REQUIRED because org != alias
- url: https://docs.brew.sh/Cask-Cookbook
  why: cask stanza order (the audit gate the script's `brew audit --cask --new-cask` exercises), `sha256` hex form
- url: https://docs.brew.sh/Brew-Livecheck
  why: documents the livecheck the cask (S1) already uses; the tap-README references it for `brew upgrade`

# MUST READ — the architecture decision this implements
- docfile: plan/007_fb356ba503b4/architecture/external_deps.md
  why: §2 Homebrew (Cask DSL, livecheck strategy :header, "custom tap until notarized for official cask",
       "push cask file to homebrew tap repo on tag"); §"CI Publishing Strategy" (deploy keys/tokens as GH secrets,
       clone → update → commit → push pattern)
  section: "2. Homebrew Cask (macOS)" + "CI Publishing Strategy" + "Hashing"

# MUST READ — the proven precedent to mirror (AUR publish automation, P1.M1.T1.S2 Complete)
- file: packaging/linux/aur/publish.sh
  why: the established shape for "patch a manifest from the release artifact, validate, document the CI push key"
  pattern: "set -euo pipefail; usage via header comment; sed-patch the version field; refresh the hash from the
           release download; document the SSH/deploy key in the header; CI (separate job) does the git push"
  gotcha: AUR's publish.sh bundles the AUR git push; THIS task deliberately does NOT bundle the tap push (the
          contract scopes the script to download/hash/patch/audit + DOCUMENT the deploy key; the push is P1.M5.T1.S2)

# MUST READ — the cask this script patches (the S1 CONTRACT input)
- docfile: plan/007_fb356ba503b4/P1M2T1S1/PRP.md
  why: S1's cask has `  version "0.2.8"` + `  sha256 :no_check` (2-space indent) — the EXACT sed targets; the
       `url …/v#{version}/QMKonnect-#{version}-macos.dmg` stanza this script's version feeds
  section: "What → File 1" (the full cask source)

# MUST READ — release-artifact facts (verified in release.yml)
- file: .github/workflows/release.yml
  why: DMG asset name `QMKonnect-<version>-macos.dmg` (L84); version via `cargo metadata`+`jq`, no `v` prefix (L44-46);
       tag is `v<version>`; universal build `MACOS_UNIVERSAL=1` (L65); notarization gated on secrets (L49/69)
  gotcha: "version has NO leading v; the release URL path uses v<version>; the script rejects a v-prefixed arg"

# REFERENCE — naming facts (the mulletware/dabstractor nuance)
- file: packaging/macos/build.sh
  why: bundle id `io.mulletware.qmkonnect` (L43) — confirms the `mulletware` brand vs `dabstractor` GitHub org
- url: https://github.com/dabstractor/qmkonnect  (source repo; release URLs; the tap alias maps here)
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
packaging/
  homebrew/                       # S1 (parallel) authors Casks/qmkonnect.rb + README.md here
    Casks/qmkonnect.rb            # S1 INPUT — this task's script patches its version/sha256 lines
    README.md                     # S1 — source-repo packaging doc (do NOT touch)
    Icon*.png, Icon.ico           # existing assets (do NOT touch)
  linux/aur/publish.sh            # P1.M1.T1.S2 — the proven publish-script precedent (READ-ONLY model)
  macos/{build.sh,...}            # the DMG producer (bundle id, universal flag)
.github/workflows/release.yml     # CI: builds + renames QMKonnect-<v>-macos.dmg; no Homebrew job yet (P1.M5.T1.S2)
# NEW (this task):
packaging/homebrew/
  tap-README.md                   # README for the dabstractor/homebrew-qmkonnect tap repo
  update-cask.sh                  # download DMG → hash → patch version/sha256 → brew audit
```

### Desired Codebase tree (files this task ADDS)

```bash
packaging/homebrew/
├── tap-README.md     # the tap repo's README.md (install/audit/deploy-key docs)
└── update-cask.sh    # version+sha256 patcher + brew audit (chmod +x)
```
(No other files. The cask itself = S1; the CI push job = P1.M5.T1.S2.)

### Known Gotchas of our codebase & Library Quirks
```bash
# CRITICAL (naming): the GitHub org is `dabstractor` (source repo dabstractor/qmkonnect; tap repo
# dabstractor/homebrew-qmkonnect). The tap ALIAS is `mulletware/qmkonnect` (bundle id io.mulletware.qmkonnect).
# `brew tap mulletware/qmkonnect` alone resolves to github.com/mulletware/homebrew-qmkonnect (WRONG). The
# explicit-URL form is REQUIRED: `brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect`.
# Use this exact command in tap-README.md (S1's README already does).

# CRITICAL (version v-prefix): cask `version` has NO leading `v` (e.g. "0.2.8"); the release TAG is `v0.2.8`;
# the download URL path is `.../v0.2.8/QMKonnect-0.2.8-macos.dmg`. The script rejects a v-prefixed <version> arg
# to prevent a double-`v` URL. The DMG filename uses the bare version.

# CRITICAL (sed portability): macOS ships BSD sed (`sed -i` needs a suffix arg); Linux ships GNU sed (`sed -i` alone).
# The portable pattern is `sed -i.bak -E -e '...' file && rm -f file.bak` — used in the script. Do NOT use bare `sed -i`.

# CRITICAL (hash tool): macOS ships `shasum` but NOT `sha256sum` (the latter needs `brew install coreutils`).
# Linux has both. The script prefers `shasum -a 256` and falls back to `sha256sum`. Do NOT hardcode one.

# GOTCHA (sed targets): S1's cask stanzas are 2-space-indented INSIDE `cask "qmkonnect" do … end`:
#   `  version "0.2.8"` and `  sha256 :no_check`. The regexes `^  version .*$` / `^  sha256 .*$` match them.
# The script post-checks `grep -q "^  version \"${VERSION}\""` to fail loud if a future stanza reorder
# silently no-ops the patch. Keep that guard.

# GOTCHA (sha256 placeholder): S1's template uses `sha256 :no_check` (the Cask-Cookbook special value). The script
# replaces it with `sha256 "<hex>"` (a quoted 64-char hex string). `brew audit --cask` (non-strict) accepts `:no_check`;
# `--strict` is only provable in the tap repo AFTER CI fills the hash — the script's audit is `--new-cask` (non-strict).

# GOTCHA (scope): the script is a PURE local update — it does NOT clone/push the tap repo. The contract scopes it to
# download/hash/patch/audit (steps a-d) + DOCUMENTING the deploy key. The git push is the CI job P1.M5.T1.S2. Do NOT
# bundle a tap push here (unlike AUR's publish.sh) — that would cross into P1.M5.T1.S2 scope.

# GOTCHA (brew optional): `brew` is not on every authoring box. The script's `brew audit` is best-effort
# (`if command -v brew`). `bash -n` + `--help` + the sed dry-run work anywhere; the audit defers to a macOS/Linuxbrew host.
```

## Implementation Blueprint

### Data models and structure
No data models. Two static files: a Markdown README and a bash script. The script patches two scalar lines
(`version`, `sha256`) in S1's cask at runtime; it declares no new types/structs.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: CREATE packaging/homebrew/update-cask.sh
  - IMPLEMENT: the exact script from "What → File 2" (copy verbatim). `#!/usr/bin/env bash`, `set -euo pipefail`.
  - BEHAVIOR: arg parse (<version> [sha256], --help); reject v-prefixed version; download DMG (curl -fL) OR use the
    sha256 arg; compute SHA256 (shasum -a 256 | sha256sum fallback); 64-hex sanity check; BSD/GNU-sed-portable patch
    of the 2 cask lines; post-patch grep guards; best-effort `brew audit --cask --new-cask`.
  - NAMING: file `update-cask.sh`; `chmod +x`.
  - PLACEMENT: packaging/homebrew/update-cask.sh.
  - DEPENDENCIES: S1's Casks/qmkonnect.rb (the sed target). Do NOT create/edit the cask here.

Task 2: VALIDATE update-cask.sh (no network needed for the cheap gates)
  - RUN: bash -n packaging/homebrew/update-cask.sh                                    → syntax OK
  - RUN: shellcheck packaging/homebrew/update-cask.sh                                 → (if installed) no errors
  - RUN: packaging/homebrew/update-cask.sh                                            → usage + exit 1
  - RUN: packaging/homebrew/update-cask.sh --help                                     → prints the header
  - RUN: packaging/homebrew/update-cask.sh v0.2.8                                     → ERROR (rejects v-prefix)
  - DRY-RUN the sed against a COPY of S1's cask (do not modify the real one):
        cp packaging/homebrew/Casks/qmkonnect.rb /tmp/q.rb
        sed -i.bak -E -e 's|^  version .*$|  version "9.9.9"|' -e 's|^  sha256 .*$|  sha256 "deadbeef"|' /tmp/q.rb && rm -f /tmp/q.rb.bak
        ruby -c /tmp/q.rb   # if ruby present → "Syntax OK"; confirm ONLY the 2 lines changed (git diff --no-index the copy)
  - (Optional, needs a published release + network): packaging/homebrew/update-cask.sh 0.2.8 → downloads, hashes, patches, audits.

Task 3: CREATE packaging/homebrew/tap-README.md
  - IMPLEMENT: sections 1–9 from "What → File 1". Title `# homebrew-qmkonnect`.
  - MUST INCLUDE verbatim: the tap command `brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect`;
    `brew install --cask qmkonnect` (+ `--no-quarantine`); the `ruby -c` + `brew audit --cask --new-cask` audit steps;
    a pointer to `update-cask.sh`; the deploy-key/HOMEBREW_TAP_DEPLOY_KEY note (P1.M5.T1.S2); the notarization →
    official-cask path (PRD §12).
  - FOLLOW pattern: packaging/linux/aur/README.md structure/tone (title+oneliner → what → install → what-it-installs →
    maintainers → CI → cross-links).
  - PLACEMENT: packaging/homebrew/tap-README.md.

Task 4: NEVER do these (out of scope / forbidden)
  - DO NOT edit S1's Casks/qmkonnect.rb or packaging/homebrew/README.md (S1 owns them).
  - DO NOT write the CI push job / edit .github/workflows/* (that's P1.M5.T1.S2; this task only DOCUMENTS the deploy key).
  - DO NOT bundle a tap-repo git push into update-cask.sh (scope split — the contract scopes the script to steps a-d).
  - DO NOT change any Rust source / Cargo.toml / docs outside packaging/homebrew/ (top-level README + docs/installation.md
    are P1.M6).
  - DO NOT invent a tap name/URL that drops the explicit-URL form, or swap mulletware↔dabstractor.
  - DO NOT use bare `sed -i` (BSD sed) or hardcode `sha256sum` (absent on macOS).
  - DO NOT edit PRD.md, any tasks.json, or prd_snapshot.md.
```

### Implementation Patterns & Key Details
```bash
# Portable SHA256 (macOS ships shasum, not sha256sum):
#   if command -v shasum >/dev/null 2>&1; then H=$(shasum -a 256 "$f" | awk '{print $1}')
#   elif command -v sha256sum >/dev/null 2>&1; then H=$(sha256sum "$f" | awk '{print $1}')
#   else echo "need shasum/sha256sum" >&2; exit 1; fi

# Portable sed (BSD+GNU): sed -i.bak -E -e '...' file && rm -f file.bak

# The cask sed targets (2-space indent, inside `cask "qmkonnect" do … end`):
#   ^  version .*$  →  "  version \"${VERSION}\""
#   ^  sha256 .*$   →  "  sha256 \"${SHA256}\""
# Post-patch grep guard catches a silent no-op if a future stanza reorder moves the lines.

# Tap command (alias != org → explicit URL required):
#   brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect
```

### Integration Points
```yaml
INPUT (S1):     packaging/homebrew/Casks/qmkonnect.rb (the version/sha256 lines the script patches)
OUTPUT:         packaging/homebrew/{tap-README.md, update-cask.sh}
RELEASE ARTIFACT: QMKonnect-<version>-macos.dmg (release.yml:84), universal (MACOS_UNIVERSAL=1)
TAP REPO:       dabstractor/homebrew-qmkonnect (git remote; CI pushes here via deploy key)
CI (P1.M5.T1.S2): loads HOMEBREW_TAP_DEPLOY_KEY → ssh-agent → clones tap → runs update-cask.sh
                  → cp Casks/qmkonnect.rb into the clone → commit → push
DOCS SYNC (P1.M6): docs/installation.md + top-level README will link to the tap (NOT this task)
PARALLEL (S1): authors the cask + packaging/homebrew/README.md — no overlap with these two files
```

## Validation Loop

### Level 1: Script syntax + arg handling (any host)
```bash
cd /home/dustin/projects/qmkonnect
bash -n packaging/homebrew/update-cask.sh                       # → no output (syntax OK)
[ -x packaging/homebrew/update-cask.sh ] || echo "chmod +x it"  # → executable
packaging/homebrew/update-cask.sh                               # → usage, exit 1
packaging/homebrew/update-cask.sh --help | head -5             # → prints the header block
packaging/homebrew/update-cask.sh v0.2.8                        # → ERROR: rejects v-prefix
# (optional) shellcheck packaging/homebrew/update-cask.sh       # → no errors if shellcheck installed
```

### Level 2: Sed patch correctness against S1's cask (no network)
```bash
cd /home/dustin/projects/qmkonnect
# Dry-run the script's sed against a COPY so the real cask is untouched:
cp packaging/homebrew/Casks/qmkonnect.rb /tmp/q_test.rb
sed -i.bak -E \
    -e 's|^  version .*$|  version "9.9.9"|' \
    -e 's|^  sha256 .*$|  sha256 "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"|' \
    /tmp/q_test.rb && rm -f /tmp/q_test.rb.bak
diff <(grep -E '^  (version|sha256)' packaging/homebrew/Casks/qmkonnect.rb) \
     <(grep -E '^  (version|sha256)' /tmp/q_test.rb)   # → ONLY the 2 lines differ
command -v ruby >/dev/null && ruby -c /tmp/q_test.rb   # → "Syntax OK" (if ruby present)
rm -f /tmp/q_test.rb
```

### Level 3: End-to-end (needs a published release + network; macOS/Linuxbrew for the audit)
```bash
cd /home/dustin/projects/qmkonnect
# Use the real S1 cask; this downloads the DMG and patches in place:
./packaging/homebrew/update-cask.sh 0.2.8
# Expected: downloads QMKonnect-0.2.8-macos.dmg, prints the sha256, patches the cask, runs `brew audit`
# (if brew present). Then `git diff packaging/homebrew/Casks/qmkonnect.rb` shows ONLY version+sha256 changed.
# SKIP this level if no v0.2.8 release DMG is published or you don't want to mutate the cask — Levels 1-2 suffice.
```

### Level 4: tap-README content review
```bash
cd /home/dustin/projects/qmkonnect
grep -nE 'brew tap mulletware/qmkonnect|dabstractor/homebrew-qmkonnect|brew install --cask|--no-quarantine|brew audit --cask|update-cask.sh|HOMEBREW_TAP_DEPLOY_KEY|notari' \
  packaging/homebrew/tap-README.md
# Expected: at least one hit for each — the tap command, the install, the audit steps, the script pointer,
# the deploy-key note, and the notarization→official-cask path.
```

## Final Validation Checklist

### Technical Validation
- [ ] `bash -n update-cask.sh` OK; `chmod +x`; no-arg → usage+exit1; `--help` → header; v-prefix → rejected.
- [ ] (if shellcheck) `shellcheck update-cask.sh` → no errors.
- [ ] Level-2 sed dry-run mutates ONLY the 2 version/sha256 lines; `ruby -c` on the result → Syntax OK.
- [ ] `git status` shows ONLY `packaging/homebrew/tap-README.md` + `packaging/homebrew/update-cask.sh` (new).

### Feature Validation
- [ ] update-cask.sh: downloads DMG (or uses provided sha256), computes SHA256 portably, patches both lines,
      post-checks with grep, best-effort `brew audit`.
- [ ] tap-README has the exact explicit-URL tap command + `brew install --cask` + audit steps + deploy-key note.
- [ ] Naming is consistent: org `dabstractor`, alias `mulletware/qmkonnect`, never swapped.

### Code Quality Validation
- [ ] Script mirrors AUR `publish.sh` conventions (`set -euo pipefail`, header usage, documented key) adapted to cask.
- [ ] Portable: BSD+GNU sed (`-i.bak`+rm), shasum/sha256sum fallback, optional `brew`/`ruby`.
- [ ] tap-README mirrors `packaging/linux/aur/README.md` structure/tone.
- [ ] No bundled tap push (scope split respected); no invented DMG/URL/naming facts.

### Documentation & Deployment
- [ ] tap-README documents install, audit, the update-cask.sh pointer, the deploy key, and the official-cask path.
- [ ] update-cask.sh header documents the deploy key + the CI push flow (P1.M5.T1.S2) for future maintainers.

---

## Anti-Patterns to Avoid
- ❌ Don't drop the explicit `<url>` from `brew tap mulletware/qmkonnect <url>` — the alias ≠ org, so without it
  `brew` looks for `mulletware/homebrew-qmkonnect` (wrong repo). Don't swap mulletware↔dabstractor anywhere.
- ❌ Don't accept a v-prefixed `<version>` — the cask version has no `v`; the URL path already adds `v`. Reject it.
- ❌ Don't use bare `sed -i` (breaks on BSD/macOS) or hardcode `sha256sum` (absent on macOS). Use the portable forms.
- ❌ Don't bundle a tap-repo `git clone`/`push` into update-cask.sh — the contract scopes it to download/hash/patch/audit
  + documenting the deploy key; the push is P1.M5.T1.S2. (This is the deliberate difference from AUR's publish.sh.)
- ❌ Don't edit S1's `Casks/qmkonnect.rb` or `packaging/homebrew/README.md`, any CI workflow, or any Rust/Cargo/docs
  outside packaging/homebrew/.
- ❌ Don't invent a fake 64-hex placeholder in the cask — S1's `:no_check` is the template; the script fills the real hash.
- ❌ Don't edit PRD.md, tasks.json, prd_snapshot.md, or any file outside the two deliverables.

---

## Confidence Score: 9/10

Both files are fully specified (update-cask.sh verbatim; tap-README section-by-section), the sed targets,
naming nuance (`mulletware` alias ↔ `dabstractor` org), version `v`-prefix rule, and portability gotchas
(BSD/GNU sed, shasum/sha256sum) are all verified against the codebase + S1's cask + release.yml. The script's
cheap gates (`bash -n`, `--help`, v-prefix reject, sed dry-run against a cask copy) run on any host without
network or `brew`; only the end-to-end download+audit (Level 3) needs a published release + macOS/Linuxbrew.
The 1-point reservation is for the Level-3 end-to-end run being host/release-dependent (explicitly skippable).