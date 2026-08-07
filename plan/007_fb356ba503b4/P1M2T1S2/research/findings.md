# Research Findings — P1.M2.T1.S2: Homebrew tap repo structure + update automation

## Task
Create two files under `packaging/homebrew/`:
1. `tap-README.md` — the README.md content for the **tap REPO** (`dabstractor/homebrew-qmkonnect`),
   NOT the same as `packaging/homebrew/README.md` (the source-repo packaging doc that S1 owns).
2. `update-cask.sh` — downloads the DMG, computes SHA256, patches `version`+`sha256` in
   `Casks/qmkonnect.rb`, validates with `brew audit --cask` (if available). Args: `<version> [sha256]`.

Inputs: `packaging/homebrew/Casks/qmkonnect.rb` (authored by S1). Outputs: the two files above,
ready for the CI publish job (P1.M5.T1.S2). DOCS: tap-README.md IS the documentation (Mode A).

## The naming nuance (CRITICAL — getting this wrong breaks the whole tap)

There are TWO different identifiers in play; do not conflate them:

| What | Value | Source |
|---|---|---|
| GitHub **org** that owns the source + tap repos | `dabstractor` | release.yml URLs, source repo `dabstractor/qmkonnect` |
| **Tap repo** full name | `dabstractor/homebrew-qmkonnect` | contract RESEARCH NOTE; external_deps.md §2 |
| **Tap alias** (what users type after `brew tap`) | `mulletware/qmkonnect` | contract RESEARCH NOTE; bundle id `io.mulletware.qmkonnect` (build.sh:43) |
| **Bundle id** | `io.mulletware.qmkonnect` | build.sh:43 |

Because `brew tap mulletware/qmkonnect` would by default look for `github.com/mulletware/homebrew-qmkonnect`
(NOT `dabstractor/...`), the tap command MUST use the **explicit-URL form** with the third argument:

```bash
brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect
```

S1's cask + README already use this exact form. The tap-README.md and update-cask.sh must match it
verbatim. The tap repo's git remote (for CI) is `https://github.com/dabstractor/homebrew-qmkonnect`.

## The cask lines update-cask.sh patches (from S1's cask, verified)

S1's `Casks/qmkonnect.rb` (source of truth — it will exist when this task starts) has, inside the
`cask "qmkonnect" do … end` block (2-space indent):

```ruby
  version "0.2.8"
  sha256 :no_check   # template placeholder — CI overwrites with the release DMG's real hash
```

The url stanza (unchanged by the script) interpolates `#{version}`:
```ruby
  url "https://github.com/dabstractor/qmkonnect/releases/download/v#{version}/QMKonnect-#{version}-macos.dmg",
      verified: "github.com/dabstractor/qmkonnect/"
```

So the sed targets are:
- `^  version .*` → `  version "<NEW>"`
- `^  sha256 .*`  → `  sha256 "<HEX>"`  (the `:no_check` placeholder is replaced by a quoted hex string)

**sed portability:** macOS ships BSD `sed` (which needs `-i ''` or `-i.bak`); Linux ships GNU `sed`
(`-i` alone). The portable pattern is `sed -i.bak -E '...' file && rm -f file.bak`. Use that.

## DMG release artifact facts (verified in release.yml)

- DMG asset name: `QMKonnect-<version>-macos.dmg` (release.yml:84 `mv QMKonnect.dmg "QMKonnect-${version}-macos.dmg"`).
- Version source: `cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name=="qmkonnect") | .version'` (release.yml:44-46). NO `v` prefix; the release TAG is `v<version>`.
- Universal binary: `MACOS_UNIVERSAL=1` (release.yml:65) → one DMG for aarch64 + x86_64.
- Notarization is GATED (`vars.ENABLE_MACOS_NOTARIZE == 'true'` + APPLE_* secrets, release.yml:49/69) → default release is ad-hoc signed. The cask README/caveats (S1) already cover the `--no-quarantine` workaround.
- Release download URL pattern: `https://github.com/dabstractor/qmkonnect/releases/download/v<VERSION>/QMKonnect-<VERSION>-macos.dmg`.

## Hashing tool (portability — verified on this box)

This box has BOTH `/usr/bin/core_perl/shasum` AND `/usr/bin/sha256sum`. **macOS ships `shasum` natively
but NOT `sha256sum`** (the latter needs `brew install coreutils`). The script must prefer `shasum -a 256`
and fall back to `sha256sum`:

```bash
if command -v shasum >/dev/null 2>&1; then
    HASH="$(shasum -a 256 "$file" | awk '{print $1}')"
elif command -v sha256sum >/dev/null 2>&1; then
    HASH="$(sha256sum "$file" | awk '{print $1}')"
else
    echo "ERROR: need shasum or sha256sum" >&2; exit 1
fi
```

## The proven precedent: AUR `publish.sh` (P1.M1.T1.S2, Complete)

`packaging/linux/aur/publish.sh` is the established pattern for "CI patches a manifest file then pushes
to an external repo." Its shape:
1. Parse args (`<version>`, `--dry-run`, `--help`); header comment documents prerequisites + the SSH key.
2. `sed -i` patch `pkgver` in PKGBUILD.
3. Refresh sha256sums (`makepkg -g` downloads the release tarball).
4. Generate `.SRCINFO`.
5. (if not dry-run) clone the AUR repo, copy files, commit, push.

**Scope difference for Homebrew (deliberate, per contract):** the contract scopes `update-cask.sh` to
steps (a)–(d) = download DMG → compute SHA256 → patch version+sha256 → `brew audit`. It does NOT bundle
the tap-repo git push; instead it **documents** the deploy key for the CI job (P1.M5.T1.S2), which will
orchestrate: clone tap → run update-cask.sh → copy cask → commit → push. So `update-cask.sh` is a PURE
local file-update+validate script (testable anywhere), and the push is a separate CI concern. This is a
cleaner separation than AUR's bundled push and is exactly what the contract specifies.

## The deploy key (documented, not implemented here)

Per external_deps.md §"CI Publishing Strategy": "For channels requiring repo pushes (AUR, Homebrew tap,
Scoop bucket): 1. Store deploy keys / tokens as GitHub Actions secrets; 2. On tag push (after GitHub
Release publish), push updated manifests; 3. Use `git clone` → update file → commit → push pattern."

For Homebrew specifically:
- Generate an SSH **deploy key** pair for the `dabstractor/homebrew-qmkonnect` repo (Settings → Deploy
  keys → Add → paste PUBLIC half with write access).
- Store the PRIVATE half as a GitHub Actions secret (e.g. `HOMEBREW_TAP_DEPLOY_KEY`) in the SOURCE repo
  `dabstractor/qmkonnect`.
- CI (P1.M5.T1.S2) loads it into `ssh-agent`, clones the tap via SSH
  (`git@github.com:dabstractor/homebrew-qmkonnect.git`), runs update-cask.sh against the source checkout,
  copies the patched cask into the tap clone, commits, pushes.

This is the Homebrew analog of AUR's SSH key (external_deps.md §1: "AUR git repo needs SSH key or token
for CI publishing"). The script header + tap-README document this; the CI job that USES it is P1.M5.T1.S2.

## Cask audit steps (documented in tap-README)

Maintainers validate the cask with (per external_deps.md §2 + Homebrew Cask-Cookbook):
```bash
ruby -c Casks/qmkonnect.rb                                  # syntax (any host with ruby)
brew audit --cask --new-cask Casks/qmkonnect.rb             # DSL/order (macOS/Linuxbrew)
# --strict + real sha256 run in the tap repo after CI fills the hash (not locally on the template)
```

## Scope boundaries (sibling tasks)
- **S1** (parallel, implementing): authors `Casks/qmkonnect.rb` + `packaging/homebrew/README.md` (source-repo
  packaging doc). This task CONSUMES the cask file S1 produces; do NOT edit it or create it.
- **P1.M5.T1.S2** (planned): the CI workflow job that calls update-cask.sh and pushes to the tap. This task
  documents the deploy key; it does NOT write the CI job.
- **P1.M6** (planned): docs/installation.md + top-level README.md community-channel sections. This task's docs
  live ONLY in `packaging/homebrew/tap-README.md` (the tap repo's README).

## Files this task adds (desired tree)
```bash
packaging/homebrew/
├── Casks/qmkonnect.rb        # S1 (input — do not touch)
├── README.md                 # S1 (source-repo packaging doc — do not touch)
├── tap-README.md             # THIS TASK — README.md for the dabstractor/homebrew-qmkonnect tap repo
└── update-cask.sh            # THIS TASK — download DMG, hash, patch version+sha256, brew audit
```
No other files. No Rust/Cargo/CI/docs-outside-packaging-homebrew changes.