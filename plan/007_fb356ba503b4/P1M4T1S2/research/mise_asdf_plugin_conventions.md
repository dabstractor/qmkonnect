# Research: asdf plugin-repo metadata conventions + mise interaction

> **Scope:** conventions for the *metadata* files of an asdf plugin REPOSITORY (the `asdf-qmkonnect`
> GitHub repo) — `.tool-versions`, `mise.toml`, `CHANGELOG.md` — plus `publish.sh` concerns
> (executable-bit), and a reference-only note on the `asdf plugin test` CI action (sibling P1.M5.T2.S2).
> The plugin SCRIPTS (`bin/list-all`, `bin/download`, `bin/install`, `lib/utils.bash`, `README.md`) are
> S1's deliverable and are **already landed** under `packaging/asdf/`.
>
> **Sourcing note:** this environment has no live web access; findings are from the researcher's training
> knowledge of the asdf and mise docs. Canonical doc URLs are cited from memory and clearly flagged
> `[from knowledge]`. Each factual claim is marked with a confidence level. Live-verify the exact URL
> anchors before quoting them in shipped prose, but the *mechanisms* described are stable and high-confidence.

---

## Summary

asdf's `.tool-versions` is a simple `<tool> <version>`-per-line file (whitespace-separated) that
supports full-line `#` comments, but shipping one inside a plugin repo is **illustrative, not required**
and not part of asdf's documented plugin structure. **mise runs an asdf plugin's `bin/*` scripts
unchanged** and reads both `.tool-versions` and `mise.toml` as *user-project* config — but a
`mise.toml`/`.tool-versions` placed *inside the plugin repo* is **purely documentation**; mise does NOT
load it as "backend config" (no such mechanism exists for asdf plugins). The correct `[tools]` pin for an
asdf-compat plugin is `qmkonnect = "0.2.8"`. For `publish.sh`, `cp` to an *existing* destination does NOT
apply the source's mode, so an explicit `chmod +x` after copying (then `git add`) is the robust way to
guarantee the executable bit; `git add` then captures that bit from the working tree.

---

## The one-table TL;DR — FUNCTIONAL vs DOCUMENTATION

This is the single most important distinction for this task (Q3's crux). Every metadata file in the
plugin repo falls into exactly one column:

| File (in plugin repo root) | FUNCTIONAL — does the version manager read & act on it? | DOCUMENTATION — illustrative example users copy |
|---|---|---|
| `.tool-versions` | ❌ Not read by asdf/mise *as plugin config*. (mise/asdf only read `.tool-versions` from the **user's project cwd tree**, not from an installed plugin's dir.) | ✅ Illustrative example. |
| `mise.toml` | ❌ Not read by mise as backend/plugin config. No such mechanism exists for asdf-compat plugins. | ✅ Illustrative example showing the `[tools]` compat pin. |
| `CHANGELOG.md` | ❌ Never read by any tool. | ✅ Human-facing release notes. |
| `bin/*`, `lib/*`, `README.md` | ✅ asdf + mise both run `bin/*` directly. | — (these are the actual plugin) |

**Key correction to the inherited task framing:** the S1 PRP references a "mise native backend stub /
`mise.toml` backend" owned by S2. There is **no mise mechanism** in which a `mise.toml` inside an
asdf-plugin repo provides backend configuration that mise consumes. The only thing close to a "mise
native backend" is mise's **`ubi` backend** (`qmkonnect = "ubi:dabstractor/qmkonnect"`), which installs
release binaries **without any plugin repo at all** and is therefore an *alternative* to this plugin, not
something to stub into it. So: the S2 `mise.toml` is **documentation only**; do not expect mise to read
it from the repo. (See Q3 for full detail.)

---

## Q1 — asdf `.tool-versions` file format

### 1a. Exact format
One `<tool> <version>` entry per line, whitespace-separated (any run of spaces/tabs). Example:
```
nodejs 18.17.0
ruby 3.2.2
qmkonnect 0.2.8
```
- Multiple versions on one line are allowed (`nodejs 18.17.0 20.5.0`); asdf uses the *last* non-comment
  token as the version. A `path:<dir>` or `ref:<sha>` prefix selects a non-release install. `[from knowledge, high confidence]`
- Source doc (from memory): **https://asdf-vm.com/manage/configuration.html** — "The `.tool-versions`
  file" section documents the `<toolname> <version>` format. `[from knowledge — verify exact anchor]`

### 1b. Are `#` comments supported?
**Yes.** asdf's `.tool-versions` parser supports `#` comment lines (a `#` starts a comment). Full-line
comments and blank lines are ignored. `[from knowledge, high confidence]`
```
# Pin QMKonnect for this project (example — copy into your own project root)
qmkonnect 0.2.8
```
- Inline trailing comments (e.g. `qmkonnet 0.2.8  # latest`) are *also* tolerated by asdf's tokenizer in
  practice (it splits on whitespace and drops the rest), but the **safe, documented usage is full-line
  `#` comments**. Prefer full-line comments in any example we ship. `[from knowledge, medium-high confidence]`

### 1c. Is it conventional/expected for a PLUGIN REPO to ship `.tool-versions` as an example?
**No — it is NOT part of asdf's documented plugin-repository structure, and it is NOT required.** `[from knowledge, high confidence]`
- The official plugin-creation doc (**https://asdf-vm.com/plugins/create.html**) defines a plugin repo's
  structure as `bin/*` (required scripts) + `lib/` (optional helpers) + a `README`. It does **not** list
  `.tool-versions` as part of a plugin repo.
- Popular reference plugins (`asdf-vm/asdf-nodejs`, `asdf-vm/asdf-ruby`, `asdf-community/asdf-golang`)
  ship a `README.md` and the `bin/*` scripts; a root-level `.tool-versions` is **not a standard fixture**
  in those repos. `[from knowledge — I am not aware of a canonical asdf plugin that ships a root .tool-versions as a documented convention; treat "many plugins do it" in the task as an overstatement]`
- **Conclusion for this task:** shipping a `.tool-versions` in `asdf-qmkonnect` is acceptable as an
  *illustrative example*, but (a) it must be clearly labeled as an example (a comment helps), and (b)
  expect zero functional effect. It is a "nice-to-have documentation artifact," not an asdf expectation.

### 1d. What version should the example pin?
Pin a **concrete released version — `0.2.8`** (the current release, matching `Cargo.toml` and the
GitHub Releases). Concrete is better than `latest` for an *example* because it's reproducible and
unambiguous (a reader copying it gets a known-good install). `latest` is also valid (it resolves from
`bin/list-all` output at install time), but for an *example file* the concrete pin is clearer. `[recommendation]`

**Concrete `.tool-versions` to ship (exact contents):**
```
# Example .tool-versions — copy this into YOUR project root, not the plugin repo.
# (asdf/mise read .tool-versions from your project's working directory, not from an
#  installed plugin directory. This file here is documentation only.)
qmkonnect 0.2.8
```

---

## Q2 — mise + asdf plugins: the "compat path"

### 2a. Does mise run an asdf plugin's `bin/*` scripts UNCHANGED?
**Yes — confirmed.** mise has a first-class **asdf backend**: it detects asdf plugins and executes their
`bin/list-all`, `bin/download`, `bin/install`, etc. **verbatim**, with no mise-specific code required. `[from knowledge, high confidence]`
- Doc (from memory): **https://mise.jdx.dev/dev-tools/backends/asdf.html** — documents that mise can use
  any asdf plugin; the plugin's standard scripts are invoked as-is. mise sets the same `ASDF_*`
  environment variables (`ASDF_INSTALL_VERSION`, `ASDF_INSTALL_PATH`, `ASDF_DOWNLOAD_PATH`, …) that
  classic asdf sets, so the plugin scripts work identically. `[from knowledge — verify exact section title; mise groups this under "Dev tools › Backends › asdf"]`
- **This is exactly why S1's single implementation serves both managers** (also stated in
  `architecture/external_deps.md` §6: "Simplest path is to make the asdf plugin work with mise"). No
  `bin/*` changes are needed for mise.

### 2b. Does mise read `.tool-versions` too?
**Yes.** mise reads BOTH `.tool-versions` AND `mise.toml` / `.mise.toml` / `mise.local.toml` as
project-local config (walked up from the current directory). `.tool-versions` support exists explicitly
for asdf compatibility. `[from knowledge, high confidence]`
- Doc (from memory): **https://mise.jdx.dev/configuration.html** — lists the config files mise reads;
  `.tool-versions` is among them (the "global config files" / "project config" sections). `[from knowledge — verify anchor]`
- Precedence (from memory): `mise.toml` and `.tool-versions` both apply; mise merges them. When both
  define the same tool, the more specific/inner file wins. For a *single-tool* example this doesn't
  matter. `[from knowledge, medium confidence on the exact merge precedence — irrelevant for the qmkonnect-only example]`

---

## Q3 — CRITICAL: is a plugin-repo `mise.toml` FUNCTIONAL or DOCUMENTATION?

### 3a. The answer
**DOCUMENTATION ONLY.** mise does **not** read a `mise.toml` (or `.tool-versions`) located inside an
installed asdf-plugin's repository directory as "backend configuration." There is no such mechanism for
asdf-compat plugins. `[from knowledge, high confidence]`

How mise config actually works (and why a plugin-repo `mise.toml` is inert there):
- mise discovers **tool-version requirements** by reading config files **upward from the user's current
  working directory** (`mise.toml` / `.tool-versions` in the user's project, then parent dirs, then
  global `~/.config/mise/config.toml`). Source: **https://mise.jdx.dev/configuration.html**. `[from knowledge]`
- mise discovers a **plugin's install behavior** by running that plugin's `bin/*` scripts (the asdf
  backend). The plugin's identity is `[tools] qmkonnect = "<version>"` in the *user's* config; the plugin
  *code* is the `bin/*` scripts. There is **no third artifact** — no `mise.toml` inside the plugin repo
  that mise reads to configure the backend. `[from knowledge, high confidence]`
- An installed asdf plugin lives at `~/.local/share/mise/plugins/qmkonnect/` (or `~/.asdf/plugins/...`).
  Users never `cd` there, so even mise's cwd-walk config discovery would never pick up a `mise.toml`
  placed there. It is inert. `[from knowledge]`

**Practical consequence:** placing `mise.toml` in `asdf-qmkonnect`'s repo root is harmless and serves
only as a copy-paste example. Label it clearly. Do NOT rely on mise reading it.

### 3b. The correct `[tools]` syntax for an asdf-compat-pinned tool
In the **user's project** `mise.toml` (what the example must show), pinning a tool that is served by an
installed asdf plugin:
```toml
[tools]
qmkonnect = "0.2.8"
```
This is the canonical, simplest form. mise resolves `qmkonnect` to the installed plugin of that name and
runs its `bin/*` to install `0.2.8`. The equivalent table form also works: `[from knowledge, high confidence]`
```toml
[tools]
qmkonnect = { version = "0.2.8" }
```
Notes:
- The plugin must be **added first**: `mise plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect`.
  Then `mise install` / `mise use` honor the `[tools]` pin. The example `mise.toml` should be paired with
  that `plugin add` command (it already is, in S1's README "mise setup" section). `[from knowledge]`
- mise also accepts explicit-backend prefixes in `[tools]` (e.g. the `ubi:` backend). For an asdf plugin
  an explicit `asdf:`-prefix form exists in some mise versions, but the **simple un-prefixed
  `qmkonnect = "0.2.8"` is correct and preferred** — mise auto-resolves the installed plugin. `[from knowledge, medium confidence on the exact `asdf:` prefix syntax — avoid it; use the simple form]`

### 3c. Is there a "native backend" (`ubi` / `[backend]`/`[config]`) relevant here?
**Not for this plugin.** mise has a separate, *native* **`ubi` backend** (`https://mise.jdx.dev/dev-tools/backends/ubi.html`, from memory) that can install a tool **without any plugin repo** by
downloading GitHub release assets directly:
```toml
[tools]
qmkonnect = "ubi:dabstractor/qmkonnect"
```
- This is a genuine *alternative* to the asdf plugin (no `asdf-qmkonnect` repo needed) — NOT something to
  embed in the asdf plugin repo. There is **no `[backend]`/`[config]`/redirection table** that mise reads
  from inside an asdf plugin. `[from knowledge, high confidence]`
- **Recommendation:** keep S2's `mise.toml` as a `[tools]`-only *documentation example* for the asdf-compat
  path. Do not add a ubi backend to the asdf plugin repo (that would conflate two distribution channels).
  If a "mise-native" channel is ever wanted, it would be a separate `mise.toml`-in-user-project or a
  separate ubi-based repo — out of scope here. `[recommendation]`

**Concrete `mise.toml` to ship (exact contents) — DOCUMENTATION ONLY:**
```toml
# Example mise.toml — copy the [tools] block into YOUR project's mise.toml (or
# .mise.toml). mise does NOT read this file from inside an installed asdf plugin;
# this is a documentation example for the asdf-compat path.
#
# Prerequisite: add the plugin once:
#   mise plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect

[tools]
qmkonnect = "0.2.8"
```

---

## Q4 — asdf plugin `CHANGELOG.md` convention

### 4a. Is a `CHANGELOG.md` conventional/expected in an asdf plugin repo root?
**Common and good practice, but not mandatory.** A `CHANGELOG.md` is not part of asdf's *required*
plugin structure (`bin/*` + README), but many well-maintained asdf plugins ship one (e.g. asdf-vm's own
plugins and asdf-community plugins frequently include a `CHANGELOG.md`). It is human-facing release
notes — no tool reads it. `[from knowledge, medium-high confidence — "common" is accurate; treat "expected/required" as an overstatement]`
- **Conclusion:** shipping a `CHANGELOG.md` in `asdf-qmkonnect` is a reasonable, low-risk best practice.
  It pairs naturally with `publish.sh` (each publish = a new changelog entry).

### 4b. Format
The de-facto standard is **Keep a Changelog** (https://keepachangelog.com/en/1.1.0/), optionally with
**Semantic Versioning**. Keep-a-Changelog uses `## [version] - YYYY-MM-DD` headers and `### Added /
### Changed / ### Fixed` subsections. `[from knowledge, high confidence]`

### 4c. Plugin-versioning nuance
asdf plugins are *independent* of the tool they install and are usually versioned on their own track.
However, for `asdf-qmkonnect` — whose entire content is "wrap the QMKonnect GitHub releases" — tying the
plugin's changelog to the tool release (starting at `0.2.8`) is pragmatic and matches the contract.
If the plugin later gains plugin-specific fixes unrelated to a QMKonnect release, introduce a separate
plugin version (e.g. `plugin-1.0.0`) or just keep appending entries per publish. `[recommendation]`

**Concrete `CHANGELOG.md` to ship (exact contents) — minimal first release for 0.2.8:**
```markdown
# Changelog

All notable changes to the `asdf-qmkonnect` plugin are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and versions track the [QMKonnect](https://github.com/dabstractor/qmkonnect/releases)
release they install.

## [Unreleased]

## [0.2.8] - 2025-01-01

### Added
- Initial release of the `asdf-qmkonnect` plugin.
- `bin/list-all` — lists QMKonnect versions from the GitHub Releases API (ascending,
  newest last; `latest` resolves correctly without a `bin/latest-stable` callback).
- `bin/download` — maps `uname -s` / `uname -m` to the release asset and fetches it
  into `$ASDF_DOWNLOAD_PATH` (verifies SHA256 if a `<asset>.sha256` sidecar exists).
- `bin/install` — installs to `$ASDF_INSTALL_PATH/bin/`:
  Linux x86_64 (full: `qmkonnect` + `qmkonnect-hid-id`, udev/systemd staged under
  `share/qmkonnect/`), macOS (raw DMG binary, CLI-only), Windows (redirects to
  Scoop/Winget/Inno).
- `mise` compatibility — mise runs this plugin's `bin/*` scripts unchanged.

[Unreleased]: https://github.com/dabstractor/asdf-qmkonnect/compare/v0.2.8...HEAD
[0.2.8]: https://github.com/dabstractor/asdf-qmkonnect/releases/tag/v0.2.8
```
*(Adjust the date to the actual release date. The `[Unreleased]`/`[0.2.8]` link references at the bottom
are Keep-a-Changelog's optional comparison-link convention; drop them if you don't tag the plugin repo.)*

---

## Q5 — `asdf plugin test` GitHub Action (REFERENCE ONLY — sibling P1.M5.T2.S2 builds this; S2 does NOT)

> This section exists so S2's `publish.sh` + repo metadata are *compatible* with the eventual CI, and so
> S2 does not accidentally build the CI workflow. **Do not author `.github/workflows/*` in S2.**

### 5a. The official action
The asdf ecosystem publishes GitHub Actions in the **`asdf-vm/actions`** repo (https://github.com/asdf-vm/actions, from memory). The relevant actions: `[from knowledge, high confidence]`
- **`asdf-vm/actions/plugin-test`** — the canonical "test my plugin in CI" action. It sets up asdf, adds
  the plugin from a git URL, installs a version, and runs a verification command.
- `asdf-vm/actions/setup` — installs asdf itself.
- `asdf-vm/actions/install` — installs a tool from a `.tool-versions`.

### 5b. `plugin-test` usage snippet (reference shape)
```yaml
jobs:
  plugin_test:
    name: asdf plugin test
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - name: asdf plugin test
        uses: asdf-vm/actions/plugin-test@v3
        with:
          command: "qmkonnect --help"          # REQUIRED: command that verifies the install (exit 0)
          plugin: qmkonnect                     # REQUIRED: plugin name
          giturl: https://github.com/dabstractor/asdf-qmkonnect  # REQUIRED: plugin repo URL
          tool: qmkonnect                       # tool name (usually == plugin)
          # version: latest                    # optional; defaults to "latest"
```
`[from knowledge — exact input NAMES (`command`, `plugin`, `giturl`, `tool`, `version`) are my best
recollection of the action's action.yml; verify against github.com/asdf-vm/actions/plugin-test/action.yml before the CI task finalizes it. The overall shape (setup → plugin add → install → run command) is high-confidence.]`

### 5c. Does `plugin test` need a `.tool-versions` in the repo, or run `bin/*` directly?
**It runs the plugin's `bin/*` directly — it does NOT require a `.tool-versions` in the plugin repo.** `[from knowledge, high confidence]`
- `plugin-test` uses its *own* inputs (`plugin`, `giturl`, `version`, `command`). It internally does the
  equivalent of: `asdf plugin add <plugin> <giturl>` → `asdf install <plugin> <version>` → run `<command>`.
  That exercises `bin/list-all` (to resolve `latest`), `bin/download`, and `bin/install`.
- **Implication for S2's metadata:** the `.tool-versions`/`mise.toml` we ship are irrelevant to
  `plugin-test`. The only things the CI needs are (a) the plugin repo to exist at the `giturl` with valid
  `bin/*` scripts, and (b) a working `command` (`qmkonnect --help`). So S2's `publish.sh` just needs to
  make sure `bin/list-all`, `bin/download`, `bin/install` land in the repo **with the executable bit** —
  which is exactly the Q6 concern.

---

## Q6 — Executable-bit preservation when syncing bash scripts into git

### 6a. The precise mechanics
- **Git stores a per-file mode** of either `100644` (non-executable) or `100755` (executable) in the
  index/object tree. It decides which based on the **working-tree file's mode at `git add` time**: if the
  file has the *owner-execute* bit set (`st_mode & 0100`), git records `100755`; otherwise `100644`. `[from knowledge, high confidence]`
- Therefore: **whatever the working-tree mode is when you `git add`, that's what gets committed.**

### 6b. Does `cp` preserve the executable bit? (the subtlety)
This depends on whether the *destination already exists*:
- **`cp` to a NON-EXISTENT destination** (first publish / new file): the new file is created with the
  source's mode bits, modified by the process `umask`. Under a normal umask (`022`), the owner-execute bit
  survives → `git add` records `100755`. ✅ Usually works.
- **`cp` to an EXISTING destination** (re-publish over an already-cloned file): GNU/BSD `cp` **overwrites
  the contents but preserves the EXISTING destination file's mode** — it does NOT apply the source's mode
  to an existing file. So if the previously-committed file landed as `100644` (non-executable), a plain
  `cp` will NOT fix it. ❌ This is the real trap. `[from knowledge, high confidence — "cp preserves existing destination mode" is standard POSIX/GNU behavior]`

### 6c. Robust recipe for `publish.sh` (the recommendation)
Do NOT rely on `cp` mode semantics. **Explicitly `chmod +x` after copying, then `git add`:**
```bash
cp -r packaging/asdf/bin "$WORK/plugin/"
chmod +x "$WORK/plugin/bin/list-all" "$WORK/plugin/bin/download" "$WORK/plugin/bin/install"
git -C "$WORK/plugin" add -A
```
- `chmod +x` is correct whether the destination is new or pre-existing, so it removes the 6b trap entirely.
- `git add` then captures `100755` from the working tree. `[from knowledge, high confidence]`
- **Fallback / belt-and-suspenders** (only if you cannot chmod the working tree, e.g. some odd CI
  filesystem): stage first, then force the index mode:
  ```bash
  git -C "$WORK/plugin" add -A
  git -C "$WORK/plugin" update-index --chmod=+x bin/list-all bin/download bin/install
  ```
  Note `update-index --chmod=+x` operates on the **index** (the file must already be staged), and it is
  *redundant* if the working-tree file is already executable. Prefer the `chmod +x` approach. `[from knowledge, high confidence]`
- **`.gitattributes` cannot set the executable bit** — git has no `.gitattributes` directive for mode.
  Line-ending normalization (`* text=auto eol=lf`) does not affect the exec bit. So there is no
  repo-internal workaround; the mode must be set in the working tree at publish time. `[from knowledge, high confidence]`
- **Verification** (good for the publish script's dry-run / log):
  ```bash
  git -C "$WORK/plugin" ls-files -s bin/list-all bin/download bin/install
  # expect lines starting with "100755" (NOT "100644")
  ```

### 6d. Note on the existing in-repo scripts
The S1 scripts under `packaging/asdf/bin/` are already `chmod +x` in the *source* repo (confirmed —
`bin/install` reads as an executable script). But the *destination* (`asdf-qmkonnect` GitHub repo) is a
separate clone; the trap in 6b applies there, so `publish.sh` must `chmod +x` regardless.

---

## Concrete deliverable checklist for S2's metadata (synthesizing Q1–Q4, Q6)

These are the files S2 ships into the `asdf-qmkonnect` repo (via `publish.sh`), with their FUNCTIONAL/DOC
status. Exact contents are given inline above (Q1d, Q3c, Q4c):

| File | Status | Notes |
|---|---|---|
| `.tool-versions` | DOC | Comment-labeled example; pins `qmkonnet 0.2.8`. (Q1d) |
| `mise.toml` | DOC | `[tools] qmkonnet = "0.2.8"` + prerequisite comment. (Q3c) |
| `CHANGELOG.md` | DOC | Keep-a-Changelog; first entry `[0.2.8]`. (Q4c) |
| `publish.sh` | TOOL | Mirrors `packaging/linux/aur/publish.sh` + `homebrew/update-cask.sh` idiom: `--dry-run`, clone `asdf-qmkonnect` via SSH deploy key, `cp` in `bin/`+`lib/`+`README.md`+metadata, **`chmod +x bin/*`**, `git add -A`, commit, push. (Q6; AUR/Homebrew twins) |
| `bin/*` (already from S1) | FUNC | Copied in; the `chmod +x` in publish.sh guarantees exec bit. |

`publish.sh` skeleton mirroring the AUR twin (clone→copy→chmod→commit→push + `--dry-run` + SSH deploy key):
```bash
#!/usr/bin/env bash
# Sync packaging/asdf/ (the asdf-qmkonnect plugin) into the asdf-qmkonnect GitHub repo.
# Mirrors packaging/linux/aur/publish.sh (clone→copy→commit→push, SSH deploy key, --dry-run).
set -euo pipefail

DRY_RUN=0; VERSION=""
for a in "$@"; do case "$a" in
    --dry-run|-n) DRY_RUN=1 ;; -h|--help) sed -n '2,20p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) VERSION="$a" ;; esac
done
[ -n "$VERSION" ] || { echo "Usage: $0 [--dry-run] <version>"; exit 1; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC="$(cd "$SCRIPT_DIR/.." && pwd)/packaging/asdf"   # …/packaging/asdf
REMOTE="git@github.com:dabstractor/asdf-qmkonnect.git"

echo "==> Publishing asdf-qmkonnect v${VERSION}${DRY_RUN:+ (dry-run)}"

WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
git clone "$REMOTE" "$WORK/plugin" 2>/dev/null || { mkdir -p "$WORK/plugin"; git -C "$WORK/plugin" init; git -C "$WORK/plugin" remote add origin "$REMOTE"; }

# Sync plugin scripts + metadata.
mkdir -p "$WORK/plugin/bin" "$WORK/plugin/lib"
cp "$SRC/bin/list-all" "$SRC/bin/download" "$SRC/bin/install" "$WORK/plugin/bin/"
cp "$SRC/lib/utils.bash" "$WORK/plugin/lib/"
cp "$SRC/README.md" "$WORK/plugin/README.md"
cp "$SCRIPT_DIR/.tool-versions" "$SCRIPT_DIR/mise.toml" "$SCRIPT_DIR/CHANGELOG.md" "$WORK/plugin/" 2>/dev/null || true

# CRITICAL (Q6): guarantee the executable bit — cp to an existing file keeps the old mode.
chmod +x "$WORK/plugin/bin/list-all" "$WORK/plugin/bin/download" "$WORK/plugin/bin/install"

# Stamp the version into the example files.
sed -i "s/^qmkonnect .*/qmkonnect ${VERSION}/" "$WORK/plugin/.tool-versions"
sed -i "s/^qmkonnect = \".*\"/qmkonnect = \"${VERSION}\"/" "$WORK/plugin/mise.toml"

git -C "$WORK/plugin" add -A
git -C "$WORK/plugin" ls-files -s bin/list-all bin/download bin/install | grep -q '^100755' \
    || { echo "ERROR: bin/* not executable in index"; exit 1; }

if git -C "$WORK/plugin" diff --cached --quiet; then
    echo "==> asdf-qmkonnect already at v${VERSION}; nothing to push."
else
    git -C "$WORK/plugin" commit -m "asdf-qmkonnect v${VERSION}"
    [ "$DRY_RUN" -eq 1 ] && { echo "==> Dry-run: skipping push."; exit 0; }
    git -C "$WORK/plugin" push origin HEAD:main
    echo "==> Published asdf-qmkonnect v${VERSION}."
fi
```
*(This skeleton is illustrative for the S2 implementer; the version-stamp `sed` patterns assume the exact
file contents given in Q1d/Q3c. Adjust branch name `main` to the repo's default branch.)*

---

## Sources

**Kept (cited, from knowledge — verify URLs before shipping in user-facing prose):**
- asdf `.tool-versions` format + comments — https://asdf-vm.com/manage/configuration.html — the canonical format reference.
- asdf plugin-repository structure (what a plugin repo must/should contain) — https://asdf-vm.com/plugins/create.html.
- mise asdf backend (mise runs `bin/*` unchanged, sets `ASDF_*`) — https://mise.jdx.dev/dev-tools/backends/asdf.html.
- mise configuration (reads `.tool-versions` + `mise.toml`, cwd-walk discovery) — https://mise.jdx.dev/configuration.html.
- mise `ubi` backend (native alternative, no plugin repo) — https://mise.jdx.dev/dev-tools/backends/ubi.html.
- asdf-vm/actions (the `plugin-test` / `setup` / `install` GitHub Actions) — https://github.com/asdf-vm/actions.
- Keep a Changelog format — https://keepachangelog.com/en/1.1.0/.
- Reference plugin repos (README + bin/*, no required root `.tool-versions`) — https://github.com/asdf-vm/asdf-nodejs, https://github.com/asdf-vm/asdf-ruby, https://github.com/asdf-community/asdf-golang.
- In-repo publish twins (clone→copy→commit→push + SSH deploy key + `--dry-run`) — `packaging/linux/aur/publish.sh`, `packaging/homebrew/update-cask.sh`.
- In-repo architecture contract (mise reuses asdf plugin unchanged) — `plan/007_fb356ba503b4/architecture/external_deps.md` §6–7.

**Dropped:**
- General "what is asdf/mise" blog posts — redundant; the official docs above are authoritative.
- Stack Overflow / forum threads on git exec bits — the POSIX/GNU `cp` and git-mode facts are primary knowledge and cited inline.

## Gaps / verification steps (low-risk; do not block implementation)

1. **Exact URL anchors** for the asdf/mise doc sections (configuration.html `.tool-versions` section;
   asdf backend page title) were cited from memory. Live-browse to confirm the exact headings before
   quoting them in shipped README/prose. The *mechanisms* are high-confidence regardless.
2. **`asdf-vm/actions/plugin-test` exact input names** (`command`/`plugin`/`giturl`/`tool`/`version`):
   verify against `github.com/asdf-vm/actions/plugin-test/action.yml` before the sibling CI task
   (P1.M5.T2.S2) finalizes the workflow. This is out of S2's scope regardless.
3. **Inline trailing comments** in `.tool-versions`: asdf tolerates them in practice, but the documented
   form is full-line `#`. The shipped example uses full-line comments (safe).
4. **mise `[tools]` explicit `asdf:`-prefix syntax**: I recommend the *un-prefixed* simple form
   (`qmkonnect = "0.2.8"`); I did not over-commit on the prefixed variant. If the user wants an explicit
   backend tag, confirm the current mise syntax against mise.jdx.dev.

## Supervisor coordination

None required — the brief is complete from training knowledge + repo files, per supervisor guidance.
Two non-blocking notes surfaced for the parent:
- The inherited "mise native backend stub" framing (S1 PRP) is **misleading**: there is no mise mechanism
  that reads a `mise.toml` from inside an asdf-plugin repo. S2's `mise.toml` is documentation only (Q3).
- `publish.sh` **must `chmod +x` the `bin/*` scripts** after copying into the clone, because `cp` to an
  *existing* file preserves the old (possibly non-executable) mode (Q6).