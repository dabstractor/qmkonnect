# PRP — P1.M4.T1.S2: Create asdf plugin repo metadata + mise native backend stub

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). **Pure packaging — no Rust/source change.**
> **Four new files, all under `packaging/asdf/`:** `.tool-versions`, `mise.toml`, `CHANGELOG.md`,
> `publish.sh`. These are the **asdf plugin-repo metadata** + the **CI publish helper** that sync
> `packaging/asdf/` into the separate `asdf-qmkonnect` GitHub repo. The plugin **scripts** themselves
> (`bin/list-all`, `bin/download`, `bin/install`, `lib/utils.bash`, `README.md`) are **already landed by
> P1.M4.T1.S1** (commit `6a1bc8b`, confirmed: the 5 files exist and `bin/*` are `chmod +x`).
> **Scope:** the 3 metadata files + the publish script ONLY. The CI workflow (`asdf plugin test` + the
> release-tag invocation of `publish.sh`) is **P1.M5.T2.S2**. The `docs/installation.md` mise/asdf row is
> **P1.M6.T1.S1**.
> **Pattern:** this task is the asdf analogue of the COMPLETED AUR S2 + Homebrew S2 + Scoop S2 —
> "create the external repo's metadata + a `publish.sh` that clones the repo, copies files in, commits,
> and pushes via an SSH deploy key" (see `packaging/linux/aur/publish.sh` — the direct clone→copy→commit→
> push twin — and `packaging/homebrew/update-cask.sh` for the deploy-key documentation idiom).

---

## ⚠️ CRITICAL DESIGN REALITY (read first — it shapes the whole task)

**A plugin-repo `mise.toml` and `.tool-versions` are DOCUMENTATION, not functional config.** mise and asdf
discover tool-version requirements by reading `.tool-versions`/`mise.toml` from the **user's project
working directory** (cwd-walk), NEVER from inside an installed plugin's directory. A `mise.toml` placed
in `asdf-qmkonnect`'s repo root is **inert to mise** — it exists only as a copy-paste example. (Verified
in `research/mise_asdf_plugin_conventions.md` §Q3.) This is why the contract's phrasing — *"mise.toml
with `[tools]` section documenting the asdf-compat path"* — is exactly right: it DOCUMENTS the path, it
does not drive it.

**The inherited "mise native backend stub" title is aspirational/loose.** There is **no mise mechanism**
in which a `mise.toml` inside an asdf-plugin repo supplies backend config. mise's only genuine "native"
alternative is its **`ubi` backend** (`qmkonnect = "ubi:dabstractor/qmkonnect"`), which installs release
binaries **with no plugin repo at all** — an *alternative* channel, out of scope here, and one that would
**conflate two distribution channels** if shoe-horned into this plugin. So this task ships a
**documentation `mise.toml`** (faithful to the contract's LOGIC) and does NOT attempt a real "native
backend." Ship the example; label it honestly.

**`publish.sh` MUST `chmod +x` the `bin/*` scripts after copying.** `cp` to an *existing* destination
preserves the destination's (possibly non-executable) mode; only `cp` to a *new* path inherits the source
mode. On a re-publish over an already-cloned repo, a plain `cp` can leave `bin/*` as `100644`
(non-executable), which breaks `asdf install`. Explicit `chmod +x` after copy is mandatory.
(`.gitattributes` cannot set the exec bit.) Verified in the research brief §Q6.

| File (this task ships) | FUNCTIONAL (a tool reads it) | DOCUMENTATION (illustrative) |
|---|---|---|
| `.tool-versions` | ❌ asdf/mise read it from the *user's project*, not the plugin repo | ✅ example users copy |
| `mise.toml` | ❌ mise does NOT read a plugin-repo mise.toml | ✅ example users copy |
| `CHANGELOG.md` | ❌ never read by any tool | ✅ human release notes |
| `publish.sh` | ✅ the CI/maintainer RUNS it to sync files into `asdf-qmkonnect` | — |

---

## Goal

**Feature Goal**: Add the `asdf-qmkonnect` **plugin-repo metadata** — a `.tool-versions` example, a
`mise.toml` example (documenting the asdf-compat pin), and a `CHANGELOG.md` — plus a `publish.sh` that
syncs `packaging/asdf/` (the 5 S1 plugin files + the 3 metadata files) into the separate
`dabstractor/asdf-qmkonnect` GitHub repo on each release. Together with S1's scripts, this completes the
**cross-platform-version-manager channel of F15** (PRD §4; §5 platform row "mise/asdf"; §12
"mise/asdf are unaffected [by signing]").

**Deliverable** (4 new files under `packaging/asdf/`):
1. `.tool-versions` — example pin `qmkonnect 0.2.8` (full-line `#` comments labeling it an example).
2. `mise.toml` — `[tools] qmkonnect = "0.2.8"` example + prerequisite comment (documents the asdf-compat
   path; mise auto-runs the plugin's `bin/*` unchanged).
3. `CHANGELOG.md` — Keep a Changelog; first entry `## [0.2.8] - 2026-07-16` describing the initial plugin.
4. `publish.sh` — mirrors `packaging/linux/aur/publish.sh`: `--dry-run` flag; clones
   `git@github.com:dabstractor/asdf-qmkonnect.git` via an SSH deploy key; copies `bin/`+`lib/`+`README.md`
   + the 3 metadata files in; **`chmod +x bin/*`**; stamps the version into the cloned `.tool-versions`/
   `mise.toml`; idempotent commit (`asdf-qmkonnect v<version>`); pushes to `main`.

**Success Definition**:
- All 4 files exist under `packaging/asdf/`; `publish.sh` is executable (`chmod +x`) with a
  `#!/usr/bin/env bash` shebang; `bash -n` passes; `shellcheck` (if installed) is clean.
- The `mise.toml` parses as valid TOML (a `[tools]` table with `qmkonnect = "0.2.8"`).
- **Mock end-to-end on the Linux dev box (no GitHub/key needed):** create a local bare git repo, point
  `publish.sh` at it via `ASDF_QMKONNECT_REMOTE`, run `./publish.sh --dry-run 0.2.8`, and confirm the
  staged tree contains `bin/{list-all,download,install}` + `lib/utils.bash` + `README.md` +
  `.tool-versions` + `mise.toml` + `CHANGELOG.md`, with `bin/*` recorded as `100755` (executable) in the
  index and the version `0.2.8` stamped into both `.tool-versions` and `mise.toml`.
- `git diff --stat` shows ONLY the 4 new files under `packaging/asdf/` (no Cargo/source/.github/
  other-packaging changes); the 5 S1 files are untouched.
- (Deferred) The real push to `dabstractor/asdf-qmkonnect` is wired in CI (P1.M5.T2.S2), which stores
  the SSH deploy key as a GitHub Actions secret and runs `publish.sh <version>` on each release tag.

## User Persona (if applicable)

**Target User (two audiences)**:
1. **The QMKonnect end user** (Linux/macOS) who manages tools with asdf or mise — they consume the
   `.tool-versions`/`mise.toml` examples as copy-paste templates for pinning `qmkonnect` in their project.
2. **The maintainer / CI** — they run `publish.sh` (or CI runs it) to push the plugin into the
   `asdf-qmkonnect` repo so `asdf plugin add qmkonnect …` works.

**Use Case (maintainer/CI)**: on each QMKonnect release tag, CI (P1.M5.T2.S2) runs
`./packaging/asdf/publish.sh <version>`, which clones `asdf-qmkonnect`, syncs the current `packaging/asdf/`
contents, commits `asdf-qmkonnect v<version>`, and pushes — so `asdf list all qmkonnect` / `mise install
qmkonnect@latest` reflect the new release immediately (the scripts scrape the GitHub Releases API at
runtime; the repo just needs the latest scripts).

**Pain Points Addressed**: closes the "metadata + automation" half of the mise/asdf channel (S1 closed the
"scripts" half). Without `publish.sh`, the `asdf-qmkonnect` repo can't stay in sync with the source
`packaging/asdf/`; without `.tool-versions`/`mise.toml`, users lack a copy-paste version-pin example.

## Why

- **F15 (PRD §4) requires a mise/asdf channel.** S1 shipped the plugin scripts; **S2 ships the plugin-repo
  metadata + the publish automation** so the external `asdf-qmkonnect` repo is real and stays current.
  Per `architecture/external_deps.md` §6-7 + §"CI Publishing Strategy" ("For channels requiring repo
  pushes: store deploy keys as GitHub Actions secrets; on tag push, clone → update file → commit → push"),
  this is the asdf instance of the proven AUR/Homebrew/Scoop publish pattern.
- **Mirrors the COMPLETED sibling publish scripts.** `packaging/linux/aur/publish.sh` (clone→copy→commit→
  push, SSH key, `--dry-run`, idempotent diff check) is the direct twin. `publish.sh` follows the same
  shape, only simpler — asdf plugins have **no version/hash to patch** (versions resolve from the GitHub
  Releases API at runtime), so publishing is a pure file-sync, not a manifest edit.
- **No new toolchain.** `publish.sh` is portable bash depending only on `git`, `cp`, `chmod`, `sed`. The
  metadata files are static text/TOML. No Rust, no Node, no jq.

## What

### Naming Truth (GROUND TRUTH — verified this session)

- `git remote get-url origin` → `git@github.com:dabstractor/qmkonnect.git` ⇒ **GitHub org = `dabstractor`**.
- Source repo = **`dabstractor/qmkonnect`**. **Tool/plugin name = `qmkonnect`** (the S1 contract's
  `mise plugin add qmkonnet …` is a TYPO — use **`qmkonnect`** everywhere).
- **Plugin repo = `asdf-qmkonnect`** (asdf convention `asdf-{toolname}`), URL
  `https://github.com/dabstractor/asdf-qmkonnect`, SSH `git@github.com:dabstractor/asdf-qmkonnect.git`.
  **The repo must pre-exist (empty is fine) on GitHub** — `publish.sh` clones it; GitHub rejects a push to
  a non-existent repo. (Create it once: github.com/new → name `asdf-qmkonnect` → Public → no README.)
- **Current release = `0.2.8`** (`Cargo.toml`); tag `v0.2.8` dated **2026-07-16** (`git log -1 --format=%ci v0.2.8`).
- **Origin default branch = `main`** (`git symbolic-ref refs/remotes/origin/HEAD`) → `publish.sh` pushes
  to `HEAD:main`.

### S1 outputs this task CONSUMES (already landed, commit `6a1bc8b` — DO NOT modify)

`packaging/asdf/{lib/utils.bash, bin/list-all, bin/download, bin/install, README.md}` — the 5 plugin
files. `bin/*` are already `chmod +x` in the source tree. `publish.sh` copies these 5 + the 3 metadata
files (this task) into the `asdf-qmkonnect` clone. `publish.sh` itself is NOT copied (it's a source-repo
helper, not part of the published plugin).

### Success Criteria
- [ ] `packaging/asdf/{.tool-versions, mise.toml, CHANGELOG.md, publish.sh}` all exist; `publish.sh` is
      `chmod +x` with `#!/usr/bin/env bash`.
- [ ] `.tool-versions` pins `qmkonnect 0.2.8` (with `#` comment labels).
- [ ] `mise.toml` has a `[tools]` table with `qmkonnect = "0.2.8"` (valid TOML; documents the asdf-compat
      path + the `mise plugin add` prerequisite).
- [ ] `CHANGELOG.md` is Keep-a-Changelog with a `## [0.2.8] - 2026-07-16` first entry.
- [ ] `publish.sh`: `bash -n` + `shellcheck` clean; the mock end-to-end (Level 2) stages the 8 files with
      `bin/*` at `100755` and the version stamped; `--dry-run` skips the push.
- [ ] `git diff --stat` = exactly the 4 new files under `packaging/asdf/` (S1's 5 untouched).

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior asdf-publish knowledge can create the 3 static files verbatim from "What →
Files 1-3" and `publish.sh` verbatim from "What → File 4", then validate on the Linux dev box via
`bash -n` + `shellcheck` + the mock end-to-end (a local bare repo + `ASDF_QMKONNECT_REMOTE` override —
needs NO GitHub access or deploy key) + grep invariants. The functional-vs-documentation distinction, the
`chmod +x` requirement, the SSH-deploy-key model, and the exact sibling idiom to mirror are all verified
and documented.

### Documentation & References

```yaml
# MUST READ — the S1 PRP (the contract whose outputs this task consumes + publishes)
- docfile: plan/007_fb356ba503b4/P1M4T1S1/PRP.md
  why: defines the 5 plugin files publish.sh must sync (bin/list-all, bin/download, bin/install,
       lib/utils.bash, README.md) and the 'Naming Truth' (org dabstractor; tool qmkonnect; plugin repo
       asdf-qmkonnect; version bare; URL v-prefixed path). Confirms S1 explicitly DEFERRED .tool-versions,
       mise.toml, and plugin-repo metadata to S2.
  section: "Naming Truth" + "Success Criteria" + Anti-Patterns ("Don't add bin/latest-stable, a mise.toml
           backend, or plugin-repo metadata — that's P1.M4.T1.S2")
  critical: "S1 is IMPLEMENTED (commit 6a1bc8b). The 5 files exist; bin/* are executable. publish.sh
           references them verbatim — do NOT modify S1's files."

# MUST READ — the verified conventions brief (exact file contents + the functional-vs-doc distinction)
- docfile: plan/007_fb356ba503b4/P1M4T1S2/research/mise_asdf_plugin_conventions.md
  why: "§Q1 = .tool-versions exact format + # comments + 'illustrative, not required' verdict. §Q3 (CRITICAL)
        = a plugin-repo mise.toml is DOCUMENTATION ONLY (mise does not load it as backend config); correct
        [tools] syntax = 'qmkonnect = \"0.2.8\"'. §Q4 = CHANGELOG.md Keep-a-Changelog minimal first entry.
        §Q5 = asdf-vm/actions/plugin-test@v3 reference (sibling CI, NOT this task). §Q6 (CRITICAL) = publish.sh
        MUST chmod +x bin/* after copy (cp to an existing file preserves the old/non-exec mode)."
  section: "all; especially the FUNCTIONAL-vs-DOCUMENTATION table + Q3 + Q6"
  critical: "the title's 'mise native backend stub' is loose — there is NO mise mechanism that reads a
        plugin-repo mise.toml. Ship the documentation example; do NOT build a ubi backend (out of scope,
        conflates channels). The chmod +x in publish.sh is MANDATORY, not optional."

# MUST READ — the architecture decision this implements
- docfile: plan/007_fb356ba503b4/architecture/external_deps.md
  why: "§6 (mise — 'downloads GitHub release binary; mise is asdf-compatible; simplest path is to make the
        asdf plugin work with mise') + §7 (asdf — 'bin/list-all scrape GitHub releases; bin/download fetch;
        bin/install extract to ASDF_INSTALL_PATH') + §'CI Publishing Strategy' ('store deploy keys as GitHub
        Actions secrets; on tag push, git clone → update file → commit → push')."
  section: "6. mise" + "7. asdf" + "CI Publishing Strategy"

# MUST READ — the DIRECT clone→copy→commit→push twin (publish.sh mirrors its structure exactly)
- file: packaging/linux/aur/publish.sh
  why: "the established idiom: set -euo pipefail; usage/help header; --dry-run flag; version arg; temp
        clone with 'trap rm -rf EXIT'; git clone <ssh-remote>; cp files in; git add; idempotent
        'git diff --cached --quiet' check; commit -m '<name> v<version>'; push. The asdf publish.sh is
        this shape, simpler (no makepkg/sha/srcinfo — asdf has no version/hash to patch)."
  pattern: "WORK=$(mktemp -d); trap 'rm -rf \"$WORK\"' EXIT; git clone \"$REMOTE\" \"$WORK/repo\"; cp …;
           git -C \"$WORK/repo\" add -A; if git -C diff --cached --quiet; then nothing-to-push else
           commit; push; fi"

# MUST READ — the SSH-deploy-key documentation idiom (mirror publish.sh's header comment on the key)
- file: packaging/homebrew/update-cask.sh
  why: "the header comment block documents the deploy-key model verbatim: generate a key pair, add the
        PUBLIC half to the external repo (Settings → Deploy keys), store the PRIVATE half as a GitHub
        Actions secret, CI loads it into ssh-agent. publish.sh's header copies this explanation for
        ASDF_PLUGIN_DEPLOY_KEY → dabstractor/asdf-qmkonnect."
  pattern: "the 'DEPLOY KEY (CI publishing …)' comment block — adapt CASK/TAP → asdf-qmkonnect."

# REFERENCE — the plugin scripts publish.sh syncs (confirm they exist + are the right names)
- file: packaging/asdf/bin/list-all            # exists (S1); copied verbatim by publish.sh
- file: packaging/asdf/bin/download            # exists (S1)
- file: packaging/asdf/bin/install             # exists (S1)
- file: packaging/asdf/lib/utils.bash          # exists (S1)
- file: packaging/asdf/README.md               # exists (S1)

# REFERENCE — the CI task that will INVOKE publish.sh (do NOT build the workflow here)
- docfile: plan/007_fb356ba503b4/P1M5T2S2/PRP.md   # (if it exists when you implement) — the CI job that
  why: "stores ASDF_PLUGIN_DEPLOY_KEY + runs 'asdf plugin test' (asdf-vm/actions/plugin-test@v3) AND
        invokes ./packaging/asdf/publish.sh <version> on each release tag. This PRP makes publish.sh
        CI-callable (--dry-run, env-overridable remote, non-interactive git identity); it does NOT add
        .github/workflows/*."

# REFERENCE — PRD context
- url: spec/PRD.md
  why: §4 F15 (community package-manager distribution incl. mise/asdf); §5 platform row (mise/asdf);
       §12 ("mise/asdf are unaffected [by signing]").
  section: "h2.3 (4. F15)" + "h2.11 (12. Beta Status)"

# EXTERNAL (from-knowledge, verify anchors before shipping in user-facing prose) — asdf/mise docs
- url: https://asdf-vm.com/plugins/create.html
  why: "the plugin-repo structure (bin/* + lib/ + README). Confirms .tool-versions/mise.toml are NOT part
       of the required plugin structure (they're optional/illustrative) — see research §Q1c."
- url: https://mise.jdx.dev/dev-tools/backends/asdf.html
  why: "mise runs an asdf plugin's bin/* UNCHANGED (sets the same ASDF_* env vars). Confirms the
       asdf-compat path the mise.toml documents (research §Q2a)."
- url: https://github.com/asdf-vm/actions
  why: "the asdf-vm/actions/plugin-test@v3 action (reference for sibling P1.M5.T2.S2). publish.sh need
       only ensure bin/* land with the executable bit — plugin-test runs them directly, no .tool-versions
       needed (research §Q5c)."
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
packaging/
  linux/aur/publish.sh           # <<< DIRECT twin: clone→copy→commit→push, SSH key, --dry-run >>>
  homebrew/update-cask.sh        # <<< the deploy-key doc idiom publish.sh's header mirrors >>>
  asdf/                          # S1 landed (6a1bc8b):
    lib/utils.bash               #   shared bash helpers (S1)
    bin/{list-all,download,install}  #   the 3 plugin scripts (S1; already chmod +x)
    README.md                    #   the plugin README (S1)
    # NEW (this task): .tool-versions, mise.toml, CHANGELOG.md, publish.sh
.github/workflows/release.yml    # asset naming + version-from-cargo (CI invocation of publish.sh = P1.M5.T2.S2)
Cargo.toml                       # version=0.2.8
# git: org = dabstractor; v0.2.8 tag dated 2026-07-16; origin default branch = main
```

### Desired Codebase tree (files this task ADDS)

```bash
packaging/asdf/
├── .tool-versions     # NEW — example pin (qmkonnect 0.2.8) + # comment labels (DOCUMENTATION)
├── mise.toml          # NEW — [tools] qmkonnect = "0.2.8" + asdf-compat prerequisite (DOCUMENTATION)
├── CHANGELOG.md       # NEW — Keep a Changelog; [0.2.8] - 2026-07-16 first entry (DOCUMENTATION)
└── publish.sh         # NEW — sync packaging/asdf/ into dabstractor/asdf-qmkonnect (TOOL)
```
(No other files. The CI workflow = P1.M5.T2.S2; docs/installation.md mise/asdf row = P1.M6.T1.S1;
`bin/latest-stable` is NOT added — asdf resolves `latest` from list-all's ascending output, so it is
unnecessary, and adding it would overlap nothing but adds maintenance for zero gain.)

### Known Gotchas of our codebase & Library Quirks

```bash
# CRITICAL (plugin-repo mise.toml/.tool-versions are DOCUMENTATION ONLY): mise/asdf read .tool-versions +
#   mise.toml from the USER's project cwd, NEVER from an installed plugin dir. The files we ship are
#   copy-paste EXAMPLES — label them clearly. Do NOT expect mise to read them from the repo. (research §Q3)

# CRITICAL (publish.sh MUST chmod +x bin/* after copy): cp to an EXISTING destination preserves the
#   destination's mode (possibly 100644/non-exec). On a re-publish over a prior clone, plain cp can leave
#   bin/* non-executable → asdf install fails. Always chmod +x after copy; .gitattributes CANNOT set the
#   exec bit. Verify with 'git ls-files -s bin/list-all' → expect '100755'. (research §Q6)

# CRITICAL (the asdf-qmkonnect repo must PRE-EXIST): git push to a non-existent GitHub repo fails
#   ("Repository not found"). Create the empty repo ONCE (github.com/new, name asdf-qmkonnect, Public, no
#   README). publish.sh clones it; the first --dry-run/push works against an empty repo (clone succeeds,
#   first commit on the unborn branch, 'git push origin HEAD:main' creates main).

# CRITICAL (org != local user; tool name has both c's): org = dabstractor (git remote). Tool/plugin name
#   = qmkonnect (NOT 'qmkonnet' — the S1 contract had a typo). Plugin repo = asdf-qmkonnect. SSH remote =
#   git@github.com:dabstractor/asdf-qmkonnect.git.

# CRITICAL (version is BARE in files; tag is v-prefixed): .tool-versions/mise.toml use BARE '0.2.8'
#   (asdf/mise never want the v). publish.sh's commit message + the git tag use 'v0.2.8'. Users type
#   'asdf install qmkonnect 0.2.8'.

# GOTCHA (publish.sh must NOT copy itself): the plugin repo = packaging/asdf MINUS publish.sh. publish.sh
#   copies bin/ + lib/ + README.md + .tool-versions + mise.toml + CHANGELOG.md explicitly (never a blanket
#   cp of packaging/asdf/).

# GOTCHA (no version/hash to patch — asdf differs from AUR/Homebrew/Scoop): asdf versions resolve from the
#   GitHub Releases API at runtime (bin/list-all scrapes tags). So publish.sh is a pure FILE-SYNC, not a
#   manifest edit. The version arg only (a) goes in the commit message and (b) is STAMPED into the cloned
#   .tool-versions/mise.toml examples (so the published examples reflect the just-released version).

# GOTCHA (CI git identity): a fresh GitHub Actions clone has no git user.name/email → 'git commit' fails
#   with "Author identity unknown". publish.sh sets a local fallback identity in the clone (only if none is
#   configured) so the commit succeeds unattended.

# GOTCHA (remote override for local testing): publish.sh reads the remote from
#   ${ASDF_QMKONNECT_REMOTE:-git@github.com:dabstractor/asdf-qmkonnect.git} so the mock end-to-end can
#   point at a local bare repo WITHOUT GitHub or a deploy key.

# GOTCHA (scope): do NOT add .github/workflows/* (CI = P1.M5.T2.S2) or docs/installation.md (P1.M6.T1.S1);
#   do NOT add bin/latest-stable (unnecessary — list-all's ascending sort resolves 'latest'); do NOT build a
#   ubi/mise-native backend (out of scope, conflates channels); do NOT touch S1's 5 files.

# GOTCHA (set -euo pipefail in publish.sh): the 3 optional/defensive steps (version-stamp sed, the
#   nothing-to-push diff check, the identity fallback) must be guarded so a no-op does not abort the script.
```

## Implementation Blueprint

### Data models and structure
No code data models. The metadata files are static text/TOML; `publish.sh` is a thin git-sync script. The
only "data" is the release identity (org `dabstractor`, tool `qmkonnect`, version `0.2.8`) embedded as
static text + the git remote.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: CREATE packaging/asdf/.tool-versions (author verbatim from "What → File 1")
  - IMPLEMENT: the verbatim .tool-versions from "What → File 1" (full-line # comments labeling it an
    EXAMPLE; the pin line 'qmkonnect 0.2.8').
  - FORMAT: '<tool> <version>' one per line, whitespace-separated. (asdf tolerates full-line # comments.)
  - PLACEMENT: packaging/asdf/.tool-versions.

Task 2: CREATE packaging/asdf/mise.toml (author verbatim from "What → File 2")
  - IMPLEMENT: the verbatim mise.toml from "What → File 2" (# header comments documenting the asdf-compat
    path + the 'mise plugin add' prerequisite; a [tools] table with qmkonnect = "0.2.8").
  - VALID TOML: [tools] is a table; qmkonnect = "0.2.8" is a string value. (A TOML linter, if available,
    must accept it; the mock test confirms parseability indirectly via the grep invariants.)
  - PLACEMENT: packaging/asdf/mise.toml.

Task 3: CREATE packaging/asdf/CHANGELOG.md (author verbatim from "What → File 3")
  - IMPLEMENT: the verbatim CHANGELOG.md from "What → File 3" (Keep a Changelog header; '## [Unreleased]';
    '## [0.2.8] - 2026-07-16' first entry describing the 4 scripts + mise compat; optional comparison links).
  - PLACEMENT: packaging/asdf/CHANGELOG.md.

Task 4: CREATE packaging/asdf/publish.sh (author verbatim from "What → File 4")
  - IMPLEMENT: the verbatim publish.sh from "What → File 4" (set -euo pipefail; usage/help header;
    --dry-run + <version> arg parse; ASDF_QMKONNECT_REMOTE override; temp clone 'trap rm -rf EXIT';
    git clone; copy bin/ + lib/ + README.md + .tool-versions + mise.toml + CHANGELOG.md into the clone;
    chmod +x bin/*; stamp version into the cloned .tool-versions + mise.toml; git identity fallback;
    git add -A; verify bin/* are 100755; idempotent 'git diff --cached --quiet'; commit 'asdf-qmkonnect
    v<version>'; push origin HEAD:main unless --dry-run).
  - DEPENDS: Tasks 1-3 (the metadata files) + S1's 5 files (already present).
  - PLACEMENT: packaging/asdf/publish.sh; chmod +x.

Task 5: VALIDATE (no edits)
  - chmod +x publish.sh; bash -n publish.sh; shellcheck (if present).
  - Mock end-to-end (Validation Level 2): local bare repo + ASDF_QMKONNECT_REMOTE + --dry-run; verify
    staged tree (8 files) + bin/* at 100755 + version stamped.
  - grep invariants (Validation Level 3); git diff --stat (exactly 4 new files).

Task 6: NEVER do these (out of scope / forbidden)
  - DO NOT add .github/workflows/* (the 'asdf plugin test' + publish invocation CI = P1.M5.T2.S2).
  - DO NOT edit docs/installation.md (the mise/asdf install row = P1.M6.T1.S1).
  - DO NOT add bin/latest-stable (unnecessary — list-all ascending sort resolves 'latest'; S1 designed it so).
  - DO NOT build a 'mise native / ubi' backend (out of scope; conflates two distribution channels; the
    contract's mise.toml is documentation of the asdf-compat path, not a native backend).
  - DO NOT modify S1's 5 files (bin/, lib/, README.md).
  - DO NOT use the typo 'qmkonnet' — the tool name is 'qmkonnect'.
  - DO NOT put a leading 'v' in .tool-versions/mise.toml (users + asdf/mise want bare 0.2.8).
  - DO NOT rely on cp preserving the executable bit in publish.sh (always chmod +x).
  - DO NOT blanket-copy packaging/asdf/ into the clone (that would publish publish.sh itself).
  - DO NOT run publish.sh against the real GitHub repo from this task (no deploy key on the dev box; the
    mock end-to-end + the deferred real push via CI is the validation path).
  - DO NOT change any Rust source / Cargo.toml / other packaging dir.
  - DO NOT edit PRD.md, any tasks.json, or prd_snapshot.md.
```

### File 1 — `packaging/asdf/.tool-versions` (reference — author verbatim)

```
# Example .tool-versions — copy the pin line into YOUR project root, not the plugin repo.
# asdf/mise read .tool-versions from your project's working directory (walking up from cwd),
# NEVER from an installed plugin directory. This file here is a documentation example only.
#
#   asdf global qmkonnect 0.2.8      # user-wide default
#   # or, per-project: copy the line below into your project's .tool-versions
qmkonnect 0.2.8
```

### File 2 — `packaging/asdf/mise.toml` (reference — author verbatim)

```toml
# Example mise.toml — copy the [tools] block into YOUR project's mise.toml (or .mise.toml).
#
# mise does NOT read this file from inside an installed asdf plugin; it is a documentation
# example for the asdf-compat path. mise runs an asdf plugin's bin/* scripts UNCHANGED, so
# the SAME plugin (https://github.com/dabstractor/asdf-qmkonnect) serves both managers.
#
# Prerequisite — add the plugin once (mise fetches + runs its bin/* to install a version):
#   mise plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect
#   mise install qmkonnect@0.2.8
#   mise use -g qmkonnect@0.2.8        # set the global default (or drop this block in a project)

[tools]
# Pin a specific release (recommended for reproducibility):
qmkonnect = "0.2.8"
# Or track the newest published release at install time:
# qmkonnect = "latest"
```

### File 3 — `packaging/asdf/CHANGELOG.md` (reference — author verbatim)

```markdown
# Changelog

All notable changes to the `asdf-qmkonnect` plugin are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This plugin wraps [QMKonnect](https://github.com/dabstractor/qmkonnect/releases) GitHub
releases, so its versions track the QMKonnect release each entry ships support for. The
same plugin serves [mise](https://mise.jdx.dev/) unchanged (mise runs an asdf plugin's
`bin/*` scripts verbatim).

## [Unreleased]

## [0.2.8] - 2026-07-16

### Added
- Initial release of the `asdf-qmkonnect` plugin.
- `bin/list-all` — lists QMKonnect versions from the GitHub Releases API, ascending
  (newest last) so `asdf install qmkonnect latest` resolves correctly without a
  `bin/latest-stable` callback.
- `bin/download` — maps `uname -s` / `uname -m` to the release asset and fetches it into
  `$ASDF_DOWNLOAD_PATH` (verifies SHA256 only if a `<asset>.sha256` sidecar exists; none
  is published today).
- `bin/install` — installs into `$ASDF_INSTALL_PATH/bin/`:
  - Linux x86_64 (primary): `qmkonnect` + `qmkonnect-hid-id`, with the udev rule + systemd
    template staged under `share/qmkonnect/` for a one-time manual setup.
  - macOS: the raw Mach-O binary from the DMG (CLI flags only — the menu-bar tray needs
    the full `.app` bundle; use the Homebrew cask or direct DMG for that).
  - Windows: redirects to Scoop / Winget / the Inno installer (the `.exe` is an installer,
    not a portable binary).
- `mise` compatibility — mise runs this plugin's `bin/*` scripts unchanged; see `mise.toml`
  for the `[tools]` pin example.

[Unreleased]: https://github.com/dabstractor/asdf-qmkonnect/compare/v0.2.8...HEAD
[0.2.8]: https://github.com/dabstractor/asdf-qmkonnect/releases/tag/v0.2.8
```

### File 4 — `packaging/asdf/publish.sh` (reference — author verbatim)

```bash
#!/usr/bin/env bash
# Sync packaging/asdf/ (the asdf-qmkonnect plugin) into the dabstractor/asdf-qmkonnect
# GitHub repo so `asdf plugin add qmkonnect …` and `mise plugin add qmkonnect …` work.
#
# Usage:
#   ./publish.sh <version>            # sync into asdf-qmkonnect, commit, push
#   ./publish.sh --dry-run <version>  # clone + stage + (local) commit, but SKIP the push
#   ./publish.sh 0.2.8
#   ASDF_QMKONNECT_REMOTE=file:///tmp/fake.git ./publish.sh --dry-run 0.2.8   # local mock test
#
# What it syncs (the plugin repo = packaging/asdf MINUS this publish.sh):
#   bin/list-all, bin/download, bin/install   (P1.M4.T1.S1 plugin scripts)
#   lib/utils.bash                            (P1.M4.T1.S1 shared helpers)
#   README.md                                 (P1.M4.T1.S1)
#   .tool-versions, mise.toml, CHANGELOG.md   (P1.M4.T1.S2 — this task's metadata)
#
# The <version> arg (a) goes in the commit message and (b) is STAMPED into the cloned
# .tool-versions / mise.toml examples so the published examples reflect this release. asdf
# itself resolves versions from the GitHub Releases API at runtime (bin/list-all), so there
# is no version/hash to patch in the scripts — this is a pure file-sync.
#
# DEPLOY KEY (CI publishing — wired in P1.M5.T2.S2):
#   The plugin repo dabstractor/asdf-qmkonnect is pushed via a GitHub deploy key (SSH, write
#   access). Generate a key pair, add the PUBLIC half to the plugin repo (Settings → Deploy
#   keys), and store the PRIVATE half as the ASDF_PLUGIN_DEPLOY_KEY Actions secret in
#   dabstractor/qmkonnect. CI loads it into ssh-agent, then runs this script on each release
#   tag. (Mirrors the AUR SSH-key model — packaging/linux/aur/publish.sh — and the Homebrew
#   tap model — packaging/homebrew/update-cask.sh.)
#
# Prerequisites:
#   * The dabstractor/asdf-qmkonnect repo must PRE-EXIST on GitHub (empty is fine — create it
#     once at github.com/new; do NOT add a README so the first push is clean). git push to a
#     non-existent repo fails with "Repository not found".
#   * `git`, `cp`, `chmod`, `sed` on PATH (all standard). An SSH key in ssh-agent for the
#     real remote (the local mock uses a file:// remote, so no key needed).
#   * P1.M4.T1.S1's plugin files (bin/, lib/, README.md) must be present under packaging/asdf/.
#
# This script does NOT modify any source file; it only writes inside a temp clone. CI
# integration (secret loading + the release-tag trigger + the 'asdf plugin test' job) is
# P1.M5.T2.S2.
set -euo pipefail

DRY_RUN=0
VERSION=""
for a in "$@"; do
    case "$a" in
        --dry-run|-n) DRY_RUN=1 ;;
        -h|--help)    sed -n '2,40p' "${BASH_SOURCE[0]}"; exit 0 ;;
        -*)           echo "Unknown option: $a" >&2; exit 1 ;;
        *)            VERSION="$a" ;;
    esac
done

if [ -z "$VERSION" ]; then
    echo "Usage: $0 [--dry-run] <version>   (e.g. 0.2.8)" >&2
    exit 1
fi
# Versions are bare (no leading 'v'); only the git TAG is v-prefixed.
case "$VERSION" in
    v*) echo "ERROR: version must not have a leading 'v' (got '$VERSION'). Use '${VERSION#v}'." >&2; exit 1 ;;
esac

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC="$SCRIPT_DIR"   # packaging/asdf (this script lives here too, but is never copied)
REMOTE="${ASDF_QMKONNECT_REMOTE:-git@github.com:dabstractor/asdf-qmkonnect.git}"

# The plugin scripts + README must exist (P1.M4.T1.S1). Fail fast with a clear message.
for f in bin/list-all bin/download bin/install lib/utils.bash README.md; do
    [ -f "$SRC/$f" ] || { echo "ERROR: required plugin file missing: $SRC/$f (is P1.M4.T1.S1 landed?)" >&2; exit 1; }
done
for f in .tool-versions mise.toml CHANGELOG.md; do
    [ -f "$SRC/$f" ] || { echo "ERROR: required metadata file missing: $SRC/$f (P1.M4.T1.S2)" >&2; exit 1; }
done

echo "==> Publishing asdf-qmkonnect v${VERSION}${DRY_RUN:+ (dry-run)}"
echo "    remote: $REMOTE"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# 1. Clone the plugin repo (it must pre-exist; an empty repo clones to an empty working tree).
git clone "$REMOTE" "$WORK/plugin"

# 2. Sync the plugin files + metadata INTO the clone (never copy publish.sh itself).
mkdir -p "$WORK/plugin/bin" "$WORK/plugin/lib"
cp "$SRC/bin/list-all" "$SRC/bin/download" "$SRC/bin/install" "$WORK/plugin/bin/"
cp "$SRC/lib/utils.bash" "$WORK/plugin/lib/"
cp "$SRC/README.md" "$SRC/.tool-versions" "$SRC/mise.toml" "$SRC/CHANGELOG.md" "$WORK/plugin/"

# 3. CRITICAL: guarantee the executable bit. cp to an EXISTING file preserves the old (possibly
#    non-executable) mode, so a re-publish could leave bin/* as 100644 → 'asdf install' fails.
#    .gitattributes cannot set the exec bit; this chmod is mandatory.
chmod +x "$WORK/plugin/bin/list-all" "$WORK/plugin/bin/download" "$WORK/plugin/bin/install"

# 4. Stamp the version into the published example files (the source copies keep their default).
#    Full-line patterns; comment lines start with '#'/whitespace so they never match.
sed -i.bak -E "s/^qmkonnect .*/qmkonnect ${VERSION}/" "$WORK/plugin/.tool-versions" && rm -f "$WORK/plugin/.tool-versions.bak"
sed -i.bak -E "s/^qmkonnect = \".*\"/qmkonnect = \"${VERSION}\"/" "$WORK/plugin/mise.toml" && rm -f "$WORK/plugin/mise.toml.bak"

# 5. Ensure a git identity exists in the clone (CI runners often lack one → 'git commit' fails).
if ! git -C "$WORK/plugin" config user.email >/dev/null 2>&1; then
    git -C "$WORK/plugin" config user.email "qmkonnect-bot@users.noreply.github.com"
    git -C "$WORK/plugin" config user.name  "QMKonnect release automation"
fi

# 6. Stage + verify the executable bit landed in the index (100755, NOT 100644).
git -C "$WORK/plugin" add -A
for b in bin/list-all bin/download bin/install; do
    mode="$(git -C "$WORK/plugin" ls-files -s "$b" | awk '{print $1}')"
    [ "$mode" = "100755" ] || { echo "ERROR: $b is $mode in the index (expected 100755); chmod +x did not take." >&2; exit 1; }
done

# 7. Idempotent commit + push (skip push under --dry-run).
if git -C "$WORK/plugin" diff --cached --quiet; then
    echo "==> asdf-qmkonnect already at v${VERSION}; nothing to commit."
else
    git -C "$WORK/plugin" commit -m "asdf-qmkonnect v${VERSION}" -m "Sync plugin scripts + metadata from dabstractor/qmkonnect@v${VERSION}."
fi

if [ "$DRY_RUN" -eq 1 ]; then
    echo "==> Dry-run: skipping push. Staged tree:"
    git -C "$WORK/plugin" ls-files -s | sed 's/^/    /'
    exit 0
fi

# Push to the repo's default branch (origin HEAD = main for dabstractor/asdf-qmkonnect).
# HEAD:main works for both the first publish (empty repo) and subsequent updates.
DEFAULT_BRANCH="$(git -C "$WORK/plugin" symbolic-ref --short HEAD 2>/dev/null || echo main)"
git -C "$WORK/plugin" push origin "HEAD:${DEFAULT_BRANCH}"
echo "==> Published asdf-qmkonnect v${VERSION} to ${DEFAULT_BRANCH}."
```

> **Note on the `git symbolic-ref --short HEAD`** in the final push: for a freshly-cloned repo with
> history, this resolves to the checked-out default branch (`main`), and `push origin HEAD:<branch>` is a
> no-op-safe fast-forward. For an empty-repo clone (first publish), `symbolic-ref HEAD` may fail (unborn
> branch) → the `|| echo main` fallback pushes `HEAD:main`, creating the branch. Both paths work.

### Implementation Patterns & Key Details

```bash
# PATTERN (clone → copy → chmod → commit → push — the AUR/Homebrew twin, applied to asdf):
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
git clone "$REMOTE" "$WORK/plugin"
cp … "$WORK/plugin/"            # bin/, lib/, README.md, .tool-versions, mise.toml, CHANGELOG.md
chmod +x "$WORK/plugin/bin/"*   # MANDATORY (cp to an existing file keeps the old mode — research §Q6)
git -C "$WORK/plugin" add -A
git -C "$WORK/plugin" diff --cached --quiet || git -C "$WORK/plugin" commit -m "asdf-qmkonnect v${VERSION}"
[ "$DRY_RUN" -eq 1 ] || git -C "$WORK/plugin" push origin "HEAD:${DEFAULT_BRANCH}"

# PATTERN (idempotent publish): the 'git diff --cached --quiet' gate means re-running publish.sh with no
#   changes prints "nothing to commit" and exits 0 — CI re-runs are safe.

# PATTERN (CI-friendly, non-interactive): the git-identity fallback (config user.email/name if unset) lets
#   'git commit' succeed on a fresh GitHub Actions clone that has no global identity.

# PATTERN (testable without GitHub): ASDF_QMKONNECT_REMOTE override → the mock end-to-end points at a local
#   bare repo (file://), exercising clone+copy+chmod+stamp+commit with no deploy key.

# PATTERN (documentation files that are inert to the tool): .tool-versions + mise.toml are clearly labeled
#   '# Example … copy into YOUR project root'. mise/asdf never read them from the plugin repo (research §Q3).

# GOTCHA (no version/hash patching): unlike AUR/Homebrew/Scoop, asdf resolves versions at runtime from the
#   GitHub Releases API, so publish.sh is a FILE-SYNC. The <version> arg only stamps the EXAMPLE files +
#   the commit message — it does not edit the plugin scripts.
```

### Integration Points

```yaml
GITHUB:
  - plugin repo: https://github.com/dabstractor/asdf-qmkonnect (SSH: git@github.com:dabstractor/asdf-qmkonnect.git)
  - must PRE-EXIST (empty OK). origin default branch = main.
DEPLOY KEY (CI — P1.M5.T2.S2):
  - PUBLIC half → dabstractor/asdf-qmkonnect Settings → Deploy keys (write)
  - PRIVATE half → ASDF_PLUGIN_DEPLOY_KEY secret in dabstractor/qmkonnect; CI loads into ssh-agent
CONSUMES (from S1, landed 6a1bc8b):
  - packaging/asdf/bin/{list-all,download,install}, lib/utils.bash, README.md (copied verbatim; chmod +x enforced)
PRODUCES (this task):
  - packaging/asdf/.tool-versions, mise.toml, CHANGELOG.md (static metadata, DOCUMENTATION)
  - packaging/asdf/publish.sh (the sync TOOL; CI-invokeable via --dry-run + env override)
RUNTIME DEPS (publish.sh): git, cp, chmod, sed (all standard). NO jq, NO makepkg, NO ruby.
PARALLEL / SIBLING (no conflicts):
  - P1.M4.T1.S1 (preceding, DONE): owns bin/ + lib/ + README.md. S2 adds .tool-versions/mise.toml/CHANGELOG.md/publish.sh
    to the SAME dir but DIFFERENT files → ZERO overlap. Merge clean.
  - P1.M5.T2.S2 (downstream): adds the .github/workflows/* job (asdf plugin test + invoke publish.sh on tag).
  - P1.M6.T1.S1 (downstream): adds the mise/asdf row to docs/installation.md.
PLATFORM VALIDATION:
  - Linux dev box: bash -n + shellcheck + the MOCK end-to-end (local bare repo, no GitHub/key needed).
  - Real push to asdf-qmkonnect: deferred to CI (P1.M5.T2.S2) with the deploy key.
```

## Validation Loop

> The implementing agent runs on a **Linux dev box**. The metadata files are static text/TOML (no runtime
> to exercise). `publish.sh` is validated by a **mock end-to-end** — a local bare git repo + the
> `ASDF_QMKONNECT_REMOTE` env override + `--dry-run` — which exercises clone+copy+chmod+stamp+commit
> WITHOUT GitHub access or a deploy key. The real push is deferred to CI (P1.M5.T2.S2).

### Level 1: Syntax & Style (runs on Linux)
```bash
cd /home/dustin/projects/qmkonnect
chmod +x packaging/asdf/publish.sh
bash -n packaging/asdf/publish.sh
# Expected: no output (parses). If a syntax error → fix before proceeding.
shellcheck packaging/asdf/publish.sh 2>/dev/null || echo "(shellcheck not installed — skipped; bash -n is the gate)"
# Expected: clean (or "not installed"). Address any SC warnings (esp. SC2086 on the chmod/ls-files loops
#   — the paths are fixed/known, but quote them per shellcheck if it flags).
git diff --stat    # Expected: exactly 4 NEW files under packaging/asdf/ (.tool-versions, mise.toml, CHANGELOG.md, publish.sh).
```

### Level 2: Mock end-to-end (runs on Linux — the headline gate; no GitHub/key needed)
```bash
cd /home/dustin/projects/qmkonnect
# Build a LOCAL bare repo to stand in for dabstractor/asdf-qmkonnect (no deploy key, no network push).
FAKE="$(mktemp -d)/asdf-qmkonnect.git"
git init --bare -b main "$FAKE" >/dev/null

# Run publish.sh in dry-run against the local fake remote:
ASDF_QMKONNECT_REMOTE="file://$FAKE" ./packaging/asdf/publish.sh --dry-run 0.2.8

# Expected: the script clones the (empty) fake repo, copies the 8 files in, chmods bin/*, stamps 0.2.8,
#   commits LOCALLY (dry-run skips the push), and prints the staged tree under "Staged tree:".
#   The staged lines MUST show:
#     100755 ... bin/download
#     100755 ... bin/install
#     100755 ... bin/list-all
#     100644 ... .tool-versions
#     100644 ... CHANGELOG.md
#     100644 ... lib/utils.bash
#     100644 ... mise.toml
#     100644 ... README.md
#   (i.e. bin/* = 100755 EXECUTABLE; everything else 100644; publish.sh is ABSENT from the tree.)
#
# If bin/* show 100644 → the chmod +x step is missing/broken (research §Q6) — fix before proceeding.
# If publish.sh appears in the tree → you did a blanket cp of packaging/asdf/ — fix to copy the explicit set.

# Verify the version was STAMPED into the (cloned, committed) example files:
WORK_TREE="$(mktemp -d)"; git clone -q "$FAKE" "$WORK_TREE" 2>/dev/null || true
# (the fake repo is empty pre-push under --dry-run, so the clone may be empty; instead inspect via the
#  commit publish.sh made — re-run WITHOUT --dry-run against a SECOND throwaway bare repo to commit+push:)
FAKE2="$(mktemp -d)/asdf2.git"; git init --bare -b main "$FAKE2" >/dev/null
ASDF_QMKONNECT_REMOTE="file://$FAKE2" ./packaging/asdf/publish.sh 0.2.8   # real push to the local fake
PROBE="$(mktemp -d)"; git clone -q "$FAKE2" "$PROBE"
grep -x 'qmkonnect 0.2.8' "$PROBE/.tool-versions"          # Expected: a match (the pin line, version-stamped)
grep -x 'qmkonnect = "0.2.8"' "$PROBE/mise.toml"           # Expected: a match (the [tools] pin, version-stamped)
test -x "$PROBE/bin/list-all" && test -x "$PROBE/bin/install" && test -x "$PROBE/bin/download"  # Expected: all exec
test ! -f "$PROBE/publish.sh"                              # Expected: publish.sh NOT published into the plugin repo
rm -rf "$FAKE" "$FAKE2" "$WORK_TREE" "$PROBE"
# If any assertion fails → read the script's stderr and fix (most likely: a wrong sed pattern, a missing
#   chmod, or a blanket copy that included publish.sh).
```

### Level 3: grep invariants (runs on Linux)
```bash
cd /home/dustin/projects/qmkonnect
# Metadata file contents:
grep -x 'qmkonnect 0.2.8' packaging/asdf/.tool-versions          # Expected: 1 match (the pin line)
grep -A1 '^\[tools\]' packaging/asdf/mise.toml | grep -x 'qmkonnect = "0.2.8"'   # Expected: 1 match
grep -n '## \[0.2.8\] - 2026-07-16' packaging/asdf/CHANGELOG.md  # Expected: 1 match
# publish.sh structure + the mandatory safeguards:
grep -n 'chmod +x' packaging/asdf/publish.sh                     # Expected: >=1 (the mandatory exec-bit fix)
grep -n 'ASDF_QMKONNECT_REMOTE' packaging/asdf/publish.sh        # Expected: >=1 (the test/CI remote override)
grep -n 'diff --cached --quiet' packaging/asdf/publish.sh        # Expected: 1 (idempotent nothing-to-push)
grep -n 'HEAD:${DEFAULT_BRANCH}\|HEAD:main' packaging/asdf/publish.sh   # Expected: 1 (first-publish-safe push)
grep -n 'config user.email' packaging/asdf/publish.sh            # Expected: >=1 (the CI identity fallback)
grep -n 'asdf-qmkonnect.git' packaging/asdf/publish.sh           # Expected: 1 (the default SSH remote)
# No typos / no wrong scope anywhere in the 4 new files:
grep -rniE 'qmkonnet[^c]' packaging/asdf/.tool-versions packaging/asdf/mise.toml packaging/asdf/CHANGELOG.md packaging/asdf/publish.sh || echo "(no 'qmkonnet' typo — good)"
grep -rnE 'v0\.2\.8' packaging/asdf/.tool-versions packaging/asdf/mise.toml || echo "(no leading-v in the example pins — good; only the git tag/comparison links use v)"
grep -n 'publish.sh' packaging/asdf/publish.sh | grep -q 'cp.*publish.sh' && echo "WARNING: publish.sh copies itself" || echo "(publish.sh is not self-published — good)"
# S1's files untouched:
git diff --stat -- packaging/asdf/bin packaging/asdf/lib packaging/asdf/README.md   # Expected: EMPTY
```

### Level 4: mise.toml TOML validity (runs on Linux if a TOML tool is present)
```bash
cd /home/dustin/projects/qmkonnect
# Best-effort TOML parse (python3's tomllib is stdlib on 3.11+):
python3 -c 'import tomllib,sys; tomllib.load(open("packaging/asdf/mise.toml","rb")); print("mise.toml: valid TOML")' 2>/dev/null \
  || echo "(tomllib unavailable — the [tools] table + 'qmkonnect = \"0.2.8\"' line are visibly valid; skip)"
# Expected: "mise.toml: valid TOML". If it errors → fix the TOML (a stray quote / bad table header).
```

### Level 5: Real publish to dabstractor/asdf-qmkonnect (DEFERRED — CI, P1.M5.T2.S2)
```bash
# NOT run from this task (no deploy key on the dev box). CI does, on each release tag:
#   1. (once) create the empty dabstractor/asdf-qmkonnect repo (github.com/new, Public, no README).
#   2. (once) add the deploy-key PUBLIC half to the repo; store the PRIVATE half as ASDF_PLUGIN_DEPLOY_KEY.
#   3. on tag: ssh-agent add key → ./packaging/asdf/publish.sh <version> → 'asdf plugin test' (asdf-vm/actions/plugin-test@v3).
# Verify post-publish: `asdf plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect && asdf install
#   qmkonnect 0.2.8 && qmkonnect --help` (the Level-2 mock already proved the file tree + exec bits).
```

## Final Validation Checklist

### Technical Validation
- [ ] `bash -n packaging/asdf/publish.sh` passes; `shellcheck` (if installed) clean.
- [ ] Mock end-to-end (Level 2): the 8 files stage with `bin/*` at `100755`; version `0.2.8` stamped into
      `.tool-versions` + `mise.toml`; `publish.sh` is NOT in the staged tree.
- [ ] `git diff --stat` = exactly the 4 new files under `packaging/asdf/` (S1's 5 untouched).

### Feature Validation
- [ ] `.tool-versions` pins `qmkonnect 0.2.8` with `#` comment labels (documentation example).
- [ ] `mise.toml` has `[tools]` with `qmkonnect = "0.2.8"` + the asdf-compat prerequisite comment; valid TOML.
- [ ] `CHANGELOG.md` is Keep-a-Changelog with `## [0.2.8] - 2026-07-16` describing the 4 scripts + mise compat.
- [ ] `publish.sh`: clones `asdf-qmkonnect`, copies the explicit 8-file set, `chmod +x bin/*`, stamps the
      version, idempotent commit, `--dry-run` skips push, `ASDF_QMKONNECT_REMOTE` override for local tests,
      CI git-identity fallback, first-publish-safe `HEAD:main` push.

### Code Quality Validation
- [ ] Portable bash (`#!/usr/bin/env bash`, `set -euo pipefail`); no jq/makepkg/ruby dependency.
- [ ] No "qmkonnet" typo; no leading `v` in the example pins; no self-publish of `publish.sh`.
- [ ] Comments explain the WHY (functional-vs-doc; chmod +x; pre-existing repo; deploy-key model; no
      version/hash patching; the asdf-compat path).

### Documentation & Deployment
- [ ] The 3 metadata files are self-documenting (labeled as examples; prerequisites stated).
- [ ] `publish.sh` header documents the deploy-key model + prerequisites + the CI hand-off (P1.M5.T2.S2).
- [ ] No Rust/Cargo/.github/other-packaging changes; no PRD/tasks.json/prd_snapshot edits.

---

## Anti-Patterns to Avoid
- ❌ Don't treat the plugin-repo `mise.toml`/`.tool-versions` as FUNCTIONAL config — they are DOCUMENTATION
  examples (mise/asdf read these from the user's project, not the plugin repo). Label them clearly.
- ❌ Don't build a "mise native / ubi backend" — out of scope; conflates two distribution channels. Ship the
  documentation `mise.toml` (the contract's actual ask).
- ❌ Don't skip `chmod +x bin/*` in `publish.sh` — `cp` to an existing file preserves the old (non-exec)
  mode; a re-publish can silently break `asdf install`. `.gitattributes` cannot set the exec bit.
- ❌ Don't blanket-`cp` `packaging/asdf/` into the clone — that would publish `publish.sh` itself. Copy the
  explicit 8-file set.
- ❌ Don't require the real GitHub repo / deploy key to validate — use the `ASDF_QMKONNECT_REMOTE=file://…`
  mock + `--dry-run` (the Level-2 gate runs on any Linux box).
- ❌ Don't add `bin/latest-stable` — S1's ascending `list-all` already resolves `latest`; it's redundant.
- ❌ Don't add `.github/workflows/*` (CI = P1.M5.T2.S2) or `docs/installation.md` rows (P1.M6.T1.S1).
- ❌ Don't modify S1's 5 files (`bin/`, `lib/`, `README.md`).
- ❌ Don't use the "qmkonnet" typo, a leading `v` in example pins, or `git push` without a branch target
  (use `HEAD:main` so the first publish of an empty repo works).
- ❌ Don't patch a version/hash into the plugin scripts (asdf resolves versions from the GitHub Releases API
  at runtime — publish.sh is a pure file-sync, unlike AUR/Homebrew/Scoop).
- ❌ Don't edit PRD.md, any tasks.json, or prd_snapshot.md.

---

## Confidence Score: 9/10

The task is small and well-bounded (4 new files: 3 static text/TOML + 1 bash sync script), and every
ground-truth fact is verified this session: S1 has **landed and committed** (`6a1bc8b` — the 5 plugin files
exist, `bin/*` executable); the org is `dabstractor`; the version is `0.2.8` (tag dated 2026-07-16); the
origin default branch is `main`. The `publish.sh` design mirrors the COMPLETED, in-repo `packaging/linux/
aur/publish.sh` twin (clone→copy→commit→push + SSH deploy key + `--dry-run` + idempotent diff check) and
the `packaging/homebrew/update-cask.sh` deploy-key documentation idiom — proven patterns, not invention.
The functional-vs-documentation distinction (research §Q3) and the mandatory `chmod +x` (§Q6) — the two
non-obvious pitfalls — are explicitly called out and designed around. The headline gate is a **mock
end-to-end** that runs on the Linux dev box with NO GitHub access or deploy key (a local bare repo +
`ASDF_QMKONNECT_REMOTE` + `--dry-run`), deterministically validating the 8-file staged tree, the `100755`
exec bits, and the version stamping. The static files are given verbatim, so one-pass authoring risk is
low. The 1-point reservation is the **deferred real push** (validated only by the mock locally; the actual
push to `dabstractor/asdf-qmkonnect` + the `asdf plugin test` CI run land in P1.M5.T2.S2 with the deploy
key) and the minor uncertainty in the `asdf-vm/actions/plugin-test@v3` action's exact input names (research
§Q5 — reference-only for the sibling; does not affect this task's deliverables). The from-knowledge doc
URLs (asdf-vm.com, mise.jdx.dev) need anchor verification before being quoted in shipped user-facing prose,
but the mechanisms they document are stable and high-confidence.