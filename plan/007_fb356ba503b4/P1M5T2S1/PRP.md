# PRP — P1.M5.T2.S1: Add Winget publishing CI job

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`, org `dabstractor`). **ONE file edited:**
> `.github/workflows/release.yml` (+ ONE new job: `winget`). **No new files. No Rust. No Cargo.toml. No
> packaging/. No docs/*.** The `WINGET_GITHUB_TOKEN` secret is documented as an inline comment block (Mode A
> ride-along).
>
> **What this does:** appends ONE `winget` job that runs **after `publish`** (the action reads the GitHub
> Release by tag to find the `.exe` asset, so the release must be live) and only on real tag pushes. It uses
> `vedantmgoyal9/winget-releaser@v2` (Komac under the hood, cross-platform → `ubuntu-latest`), which
> auto-finds the installer via `installers-regex`, computes its SHA256, syncs a fork of `microsoft/winget-pkgs`
> under `dabstractor/winget-pkgs`, and opens an update PR `fork → microsoft:main`. The job first computes the
> **bare** version (strips the tag's leading `v`) and passes it as the action's `version` input.
>
> **Source of truth:** `architecture/external_deps.md` §4 (Winget) + "CI Publishing Strategy" ("for Winget:
> automated PR to microsoft/winget-pkgs using wingetcreate or the official bot") + "Version Source of Truth"
> (Cargo.toml). `packaging/winget/README.md` "Publishing to microsoft/winget-pkgs" (P1.M3.T2.S2) documents
> the two publishing paths + the PAT. `research/notes.md` holds the **verbatim `winget-releaser` action.yml
> analysis** with FOUR load-bearing corrections to the work-item contract.

---

## Goal

**Feature Goal**: After every `v*` tag push, the Windows Winget community channel updates **automatically**
with no maintainer intervention: `vedantmgoyal9/winget-releaser@v2` finds the released
`QMKonnet-<ver>-windows-x64.exe`, hashes it, and opens a per-release PR to `microsoft/winget-pkgs`; once the
winget-pkgs maintainers merge it, `winget upgrade dabstractor.QMKonnect` serves the new version to users.

**Deliverable** (exactly ONE file edited, ONE job added):
- `.github/workflows/release.yml` — a `winget` job (`needs: [publish]`, `if: github.event_name == 'push'`,
  `runs-on: ubuntu-latest`) with: a `Determine version` step (`${GITHUB_REF_NAME#v}` → bare version), then the
  `vedantmgoyal9/winget-releaser@v2` action with `identifier: dabstractor.QMKonnect`,
  `version: ${{ steps.ver.outputs.version }}`, `installers-regex: 'QMKonnect-.*-windows-x64\.exe$'`,
  `token: ${{ secrets.WINGET_GITHUB_TOKEN }}` (+ `release-repository`/`release-tag`/`fork-user` intentionally
  OMITTED — they default correctly). One inline comment block documents `WINGET_GITHUB_TOKEN` (classic PAT,
  `public_repo` scope; why the default GITHUB_TOKEN can't fork winget-pkgs; the one-time manual
  `wingetcreate new` prerequisite).

**Success Definition**:
- `release.yml` parses as valid YAML; `actionlint` (if installed) is clean; `git diff --stat` shows
  **ONLY** `.github/workflows/release.yml`.
- The `winget` job has `needs: [publish]` + `if: github.event_name == 'push'` + `runs-on: ubuntu-latest`.
- The job uses `vedantmgoyal9/winget-releaser@v2` (**NOT `@latest`** — `@latest` is not a valid ref for this
  action and would fail to resolve; verified via `git ls-remote`).
- The `version` input receives a **bare** version (no leading `v`) from a `steps.ver` step; the
  `installers-regex` matches `QMKonnect-…` (NOT the contract's typo `QMKonnet-…`).
- `release-repository` is OMITTED (default `qmkonnect` is correct — the action prepends the owner; setting
  `dabstractor/qmkonnect` would double the owner → 404).
- The job adds NO `permissions:` escalation (the PAT handles the winget-pkgs fork/PR; top-level
  `contents: read` suffices).
- (Deferred) On a real tag push with `WINGET_GITHUB_TOKEN` set AND the one-time manual `wingetcreate new`
  merged, the action opens a PR to `microsoft/winget-pkgs`. (No PAT on the dev box + no first submission →
  the live PR is a CI gate — see Validation Level 4.)

## User Persona (if applicable)

**Target User**: the Windows user who installs via Winget (`winget install dabstractor.QMKonnect`). Before
this task, each release's winget-pkgs entry lagged until a maintainer manually ran `wingetcreate update` +
opened a PR. After this task, the PR opens automatically within minutes of the tag push (subject to the
one-time manual first submission).

**Use Case**: maintainer cuts `v0.2.9`; CI builds all platforms, publishes the GitHub Release, then the
`winget` job opens a PR bumping `dabstractor.QMKonnect` to `0.2.9` (new `InstallerUrl` + `InstallerSha256`).
The winget-pkgs maintainers merge it; `winget upgrade dabstractor.QMKonnect` then serves 0.2.9.

**Pain Points Addressed**: closes the "manual Winget step" gap in the F15 community-distribution pipeline
(PRD §4 F15 — "publish every release to AUR, Homebrew, Scoop, **Winget**, Nix, mise/asdf"). AUR (P1.M5.T1.S1),
Homebrew + Scoop (P1.M5.T1.S2) are the sibling jobs; this task is the **Winget** instance.

## Why

- **F15 (PRD §4) requires an automated Winget channel.** `architecture/external_deps.md` §4 + "CI Publishing
  Strategy" mandate an automated PR to `microsoft/winget-pkgs` (via wingetcreate or the official bot). The
  `vedantmgoyal9/winget-releaser@v2` action IS that bot — it wraps Komac, which is the maintained successor
  to the older winget-pkgs-automation. This task IS that job.
- **The manifest + the publish automation already exist.** `packaging/winget/*.yaml` (P1.M3.T2.S1, Complete)
  is the manifest template (for the one-time manual `wingetcreate new`). `packaging/winget/submit.ps1`
  (P1.M3.T2.S2, Complete) is the wingetcreate-CLI alternative (Option A). This task wires the contract's
  PRIMARY path (Option B, the `winget-releaser` action) into CI — it adds NO new packaging artifacts; it only
  adds the workflow job that drives the action.
- **The job MUST run AFTER `publish`.** The `winget-releaser` action reads the GitHub Release by tag
  (`releases/tags/<tag>`) to find the `.exe` asset. That release exists only after the `publish` job attaches
  the `windows` job's artifact to it. Hence `needs: [publish]` + `if: github.event_name == 'push'`
  (workflow_dispatch dry-runs don't publish → the release-tags API call 404s).
- **Auth model = a classic PAT, NOT the GITHUB_TOKEN and NOT a deploy key.** Unlike the AUR (per-account SSH
  key) / Homebrew-tap + Scoop-bucket (per-repo deploy keys) jobs, Winget's target is an EXTERNAL repo we do
  NOT own (`microsoft/winget-pkgs`). The `winget-releaser` action auto-forks it under the PAT's owner
  (`dabstractor/winget-pkgs`), pushes a branch, and opens `fork → microsoft:main`. The default `GITHUB_TOKEN`
  is scoped to `dabstractor/qmkonnect` only and **cannot** fork an external repo → a separate classic PAT
  (`public_repo` scope) is mandatory.

## What

### Approach: one job, ubuntu runner, the winget-releaser action

The job runs on `ubuntu-latest` (Komac, which the action uses, is cross-platform — no `windows-latest` +
`wingetcreate` needed). The job's responsibilities:

1. **`needs: [publish]`, `if: github.event_name == 'push'`** — release assets live; tag-only.
2. **`name: Determine version` (id: ver)** — `echo "version=${GITHUB_REF_NAME#v}" >> "$GITHUB_OUTPUT"`. This
   job builds NO Rust, so the cargo-metadata idiom would needlessly install the toolchain. The tag is
   `v<ver>`; stripping the `v` yields the bare version the action's `version` input requires (used verbatim).
3. **`uses: vedantmgoyal9/winget-releaser@v2`** with `identifier`, `version`, `installers-regex`, `token`.
   The action finds the installer in the release by tag (defaulted), hashes it, syncs the fork, and opens the
   PR. **No checkout step** (the action reads the release via the API, not the repo checkout).

### Success Criteria

- [ ] `.github/workflows/release.yml` has a new `winget` job with `needs: [publish]`,
      `if: github.event_name == 'push'`, `runs-on: ubuntu-latest`.
- [ ] The job has a `Determine version` step (id: `ver`) that outputs the **bare** version via
      `${GITHUB_REF_NAME#v}`.
- [ ] The job uses `vedantmgoyal9/winget-releaser@v2` (**NOT `@latest`** — verify with grep).
- [ ] The action's `with:` sets `identifier: dabstractor.QMKonnect`,
      `version: ${{ steps.ver.outputs.version }}`,
      `installers-regex: 'QMKonnect-.*-windows-x64\.exe$'` (NOTE: `QMKonnect`, two c's — NOT the contract's
      `QMKonnet` typo), and `token: ${{ secrets.WINGET_GITHUB_TOKEN }}`.
- [ ] `release-repository`, `release-tag`, `fork-user`, `max-versions-to-keep`, `release-notes-url` are all
      **OMITTED** (they default correctly: `release-repository` = `qmkonnect` (name only — the action prepends
      the owner); `release-tag` = `github.ref_name` on a push event; `fork-user` = `dabstractor`).
- [ ] An inline comment block documents `WINGET_GITHUB_TOKEN`: classic PAT with `public_repo` scope; create at
      https://github.com/settings/tokens (classic) → check `public_repo`; store as the repo Actions secret in
      `dabstractor/qmkonnect`; WHY a separate PAT (default GITHUB_TOKEN can't fork winget-pkgs); the one-time
      manual `wingetcreate new` prerequisite; never log the token.
- [ ] The job adds NO `permissions:` line (the PAT handles the external fork/PR; top-level `contents: read`
      suffices).
- [ ] `git diff --stat` shows ONLY `.github/workflows/release.yml`.

## All Needed Context

### Context Completeness Check

_Pass._ An agent with no prior Winget-CI knowledge can implement this from: the verbatim job YAML
(Implementation Patterns); the EXACT `winget-releaser` action.yml contract (4 inputs set, 5 omitted with
their defaults — quoted in References); the FOUR load-bearing contract corrections (`@v2` not `@latest`,
`QMKonnect` not `QMKonnet`, bare `version`, OMIT `release-repository`); the auth model (classic PAT, not
GITHUB_TOKEN, not deploy key); and the precise gotchas (tag-only gate; `needs:[publish]`; no checkout; no
permissions escalation; the first-run "package does not exist" pre-flight failure until the manual
`wingetcreate new` is merged). The Linux dev box validates via YAML parse + actionlint + grep gates (no PAT
needed); the real PR is a deferred CI gate.

### Documentation & References

```yaml
# MUST READ — the file being edited (mirror its idioms verbatim; APPEND the winget job at the END)
- file: .github/workflows/release.yml
  why: "the existing jobs. MIRROR: (a) the `publish` + `aur`/`homebrew-tap`/`scoop-bucket` jobs' tag-only
        gate `needs: [publish]` + `if: github.event_name == 'push'` (my job copies it — the release must be
        live); (b) the top-level `permissions: contents: read` (my job adds NO `permissions:` line — the PAT
        handles the external fork/PR, not the GITHUB_TOKEN); (c) the inline AUR_SSH_PRIVATE_KEY /
        APPLE_* secret documentation style (Mode A ride-along) for the WINGET_GITHUB_TOKEN block;
        (d) the sibling jobs' `${GITHUB_REF_NAME#v}` `Determine version` step (build-less jobs avoid the
        cargo toolchain — my job reuses the identical idiom for consistency). APPEND the `winget` job at the
        very END of the file, AFTER `scoop-bucket` (the current last job). Do NOT change any existing job."
  pattern: "needs: [publish]; if: github.event_name == 'push'; runs-on: ubuntu-latest; a `# ─── banner`
            comment block above the job documenting the secret; a `Determine version` (id: ver) step before
            the action."
  gotcha: "current job order: macos, windows, linux-binary, arch, publish, aur, homebrew-tap, scoop-bucket.
           P1.M5.T1.S2 (homebrew-tap + scoop-bucket) has LANDED — scoop-bucket is the last job. Append winget
           after it. If a re-plan shows a different tail, append at the very END regardless."

# MUST READ — the load-bearing external action (read its action.yml — quoted verbatim in research/notes.md)
- url: https://github.com/vedantmgoyal9/winget-releaser/blob/master/action.yml
  why: "the EXACT `inputs:` contract. REQUIRED inputs: identifier, installers-regex, release-repository (has
        a default), release-tag (has a default), max-versions-to-keep (has a default), token, fork-user (has a
        default). OPTIONAL: version, release-notes-url. CONFIRMS: (1) release-tag default =
        `${{ github.event.release.tag_name || github.ref_name }}` → falls back to `github.ref_name` on a PUSH
        event, so NO explicit release-tag is needed; (2) release-repository default =
        `${{ github.event.repository.name }}` (= `qmkonnect`, NAME ONLY) and the action PREPENDS
        `${{ github.repository_owner }}/` → `dabstractor/qmkonnect`, so OMIT it (setting `dabstractor/qmkonnect`
        DOUBLES the owner → 404); (3) version handling: empty → `$tag -replace '^v'`; provided → VERBATIM;
        (4) it uses Komac (cross-platform → ubuntu-latest), NOT wingetcreate; (5) pre-flight check: errors if
        the package does not already exist in winget-pkgs (the manual-first prerequisite)."
  critical: "FOUR contract corrections flow from this: (a) reference `@v2` NOT `@latest` (no `latest` ref
             exists — `git ls-remote` shows only `refs/tags/v2`); (b) installers-regex must be `QMKonnect`
             (two c's), not the contract's `QMKonnet` typo; (c) `version` must be BARE (strip the tag's v in
             steps.ver — the action uses a provided version verbatim); (d) OMIT release-repository (default
             `qmkonnect` is correct; the action owner-prepends)."

# MUST READ — the publisher-side doc (PAT setup, the two paths, the manual-first prerequisite)
- file: packaging/winget/README.md
  why: "the `## Publishing to microsoft/winget-pkgs (for maintainers)` section (P1.M3.T2.S2) documents BOTH
        paths: Option A (submit.ps1 on windows-latest) and Option B (winget-releaser on ubuntu-latest — what
        THIS job implements), the classic-PAT/`public_repo`/WINGET_GITHUB_TOKEN requirement, the one-time
        manual `wingetcreate new`, and the leading-`v` version gotcha. ⚠️ NOTE: its Option B snippet sets
        `release-repository: dabstractor/qmkonnect` — that is the OWNER-DOUBLING bug (see gotcha above); the
        raw action.yml is authoritative, not that snippet."
  pattern: "the inline comment block for WINGET_GITHUB_TOKEN should mirror this section's PAT guidance
            (classic PAT, public_repo scope, why not GITHUB_TOKEN, manual-first)."

# MUST READ — the architecture decision + CI strategy this implements
- docfile: plan/007_fb356ba503b4/architecture/external_deps.md
  why: "§4 Winget — publication = PR to microsoft/winget-pkgs; required fields (PackageIdentifier,
        PackageVersion, InstallerType inno, InstallerSha256). §'CI Publishing Strategy' → 'For Winget:
        Automated PR to microsoft/winget-pkgs using wingetcreate or the official bot' (the winget-releaser
        action IS that bot). §'Version Source of Truth' (derive from Cargo.toml — here, transitively via the
        git tag the cargo-release cut from Cargo.toml). §'Hashing' (Winget: InstallerSha256, computed by the
        action from the downloaded .exe)."
  section: "4. Winget (Windows)" + "CI Publishing Strategy" + "Version Source of Truth" + "Hashing"

# MUST READ — the verbatim action.yml analysis + the 4 contract corrections + the coordination facts
- docfile: plan/007_fb356ba503b4/P1M5T2S1/research/notes.md
  why: "(1) the verbatim action.yml (inputs table + the release-by-tag fallback + the verbatim version
        handling + the pre-flight existence check); (2) the FOUR load-bearing contract corrections (@v2 not
        @latest; QMKonnect not QMKonnet; bare version; OMIT release-repository); (3) why Option B
        (winget-releaser) and not Option A (submit.ps1) for this job; (4) the current release.yml job
        inventory + that scoop-bucket is the last job (append after it); (5) zero-conflict coordination with
        the parallel P1.M5.T1.S2."

# REFERENCE — the Option A alternative (NOT used by this job; documented for completeness)
- file: packaging/winget/submit.ps1
  why: "the wingetcreate-CLI alternative (P1.M3.T2.S2, Complete). THIS job uses Option B (winget-releaser)
        instead — the contract's primary. If a future task switches to Option A, the job becomes:
        runs-on: windows-latest; actions/checkout@v4; `winget install Microsoft.WingetCreate`; then
        `pwsh ./packaging/winget/submit.ps1 -Version <bare> -Submit -Token $env:WINGET_GITHUB_TOKEN`. Do NOT
        implement that here — the contract specifies the winget-releaser action."
- file: packaging/winget/dabstractor.QMKonnect.installer.yaml   # P1.M3.T2.S1 (Complete)
  why: "confirms PackageIdentifier `dabstractor.QMKonnect`, InstallerType inno, the exact asset URL
        `.../v0.2.8/QMKonnect-0.2.8-windows-x64.exe` (→ the installers-regex), the 64-zero InstallerSha256
        PLACEHOLDER that the action overwrites on the winget-pkgs copy. The job does NOT touch this file."

# EXTERNAL — confirm the action's @v2 tag resolves (do NOT use @latest)
- url: https://github.com/vedantmgoyal9/winget-releaser
  why: "the action repo. `git ls-remote --tags https://github.com/vedantmgoyal9/winget-releaser.git` shows
        `refs/tags/v2` (commit 4ffc7888). There is NO `latest` ref → `@latest` fails to resolve. Pin to `@v2`."
  critical: "the work-item contract says `@latest`; that is WRONG — use `@v2`. GitHub Actions does not treat
             `@latest` as a magic moving ref; it must match an existing tag/branch."

# EXTERNAL — PAT scope + the winget-pkgs fork/PR model
- url: https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens
  why: "confirms the CLASSIC PAT `public_repo` scope covers forking a public repo (`microsoft/winget-pkgs`) +
        opening a PR. Use a CLASSIC token (Tokens (classic)), not a fine-grained one."
  critical: "the default `${{ secrets.GITHUB_TOKEN }}` is scoped to `dabstractor/qmkonnect` ONLY and CANNOT
             fork `microsoft/winget-pkgs` → a separate classic PAT is mandatory."
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
.github/workflows/
  release.yml            # EDIT: append the `winget` job at the END (after scoop-bucket)
      jobs.macos         # produces the DMG (unrelated to winget)
      jobs.windows       # produces the Inno exe → renamed QMKonnect-<ver>-windows-x64.exe (the asset the action finds)
      jobs.linux-binary
      jobs.arch
      jobs.publish       # the dependency: needs:[publish] (release must be live; the action reads it by tag)
      jobs.aur           # P1.M5.T1.S1 (Complete) — sibling, different auth model (per-account SSH key)
      jobs.homebrew-tap  # P1.M5.T1.S2 (landed) — sibling, deploy-key model
      jobs.scoop-bucket  # P1.M5.T1.S2 (landed) — LAST job; append `winget` AFTER it
packaging/winget/
  *.yaml                 # P1.M3.T2.S1 (Complete) — manifest template (NOT modified by this job)
  submit.ps1             # P1.M3.T2.S2 (Complete) — Option A alternative (NOT used by this job)
  README.md              # P1.M3.T2.S2 (Complete) — publisher doc (the PAT model + the two paths)
Cargo.toml               # version = "0.2.8" (the tag v0.2.8 is cut from this; ref_name#v derives it)
```

### Desired Codebase tree with files added/changed

```bash
.github/workflows/release.yml   # +1 job (`winget`) appended after scoop-bucket. NO new files.
# (no Rust, no Cargo.toml, no packaging/ changes, no docs/*)
```

### Known Gotchas of our codebase & Library Quirks

```yaml
# CRITICAL (reference @v2, NOT @latest): the work-item contract says `vedantmgoyal9/winget-releaser@latest`.
#   There is NO `latest` tag/branch on that repo (`git ls-remote` → only `refs/tags/v2`). `@latest` fails to
#   resolve ("Unable to resolve action … @latest"). Use `vedantmgoyal9/winget-releaser@v2`. Pinning a major
#   is also GitHub Actions best practice (reproducibility/supply-chain).

# CRITICAL (installers-regex: QMKonnect, two c's): the contract's regex `QMKonnet-.*-windows-x64\.exe$`
#   (one c) matches NOTHING — the asset is `QMKonnect-<ver>-windows-x64.exe` (two c's). Use
#   `QMKonnect-.*-windows-x64\.exe$` (matches the README + the installer.yaml asset URL). A wrong regex →
#   the action resolves zero installer URLs and errors.

# CRITICAL (version must be BARE): the action's `version` input is used VERBATIM when provided (it strips a
#   leading 'v' ONLY when version is OMITTED — see action.yml's If/Else). The contract passes
#   `${{ steps.ver.outputs.version }}`, so steps.ver MUST yield the bare `0.2.8` (strip the tag's v via
#   `${GITHUB_REF_NAME#v}`). NEVER pass `${{ github.event.release.tag_name }}` (null on a push event) or the
#   raw `github.ref_name` (`v0.2.8` — winget rejects a leading v).

# CRITICAL (OMIT release-repository — default `qmkonnect` is correct): the action builds the release-API URL
#   as `repos/${{ github.repository_owner }}/${{ inputs.release-repository }}/...` — it PREPENDS the owner.
#   release-repository defaults to `${{ github.event.repository.name }}` = `qmkonnect` (NAME ONLY) → final
#   `dabstractor/qmkonnect`. The README's Option B snippet sets `release-repository: dabstractor/qmkonnect`,
#   which DOUBLES the owner (`dabstractor/dabstractor/qmkonnect`) → 404. OMIT release-repository entirely.
#   (The action.yml is authoritative, not the README snippet.) Same logic: OMIT release-tag (default
#   `github.event.release.tag_name || github.ref_name` → `github.ref_name` on push = the tag — correct) and
#   fork-user (default `dabstractor` — correct).

# CRITICAL (needs:[publish], NOT [windows]): the winget-releaser action reads the GitHub Release by tag
#   (`releases/tags/<tag>`) to find the .exe asset. That release exists only AFTER `publish` attaches the
#   `windows` job's artifact to it. The `windows` job uploads a workflow ARTIFACT, not a release asset. So
#   needs:[publish]. workflow_dispatch dry-runs don't publish → the `if: github.event_name == 'push'` gate
#   skips the job there (else the release-tags API call 404s).

# CRITICAL (no checkout step needed): unlike the AUR/Homebrew/Scoop jobs, the winget-releaser action reads
#   the release via the GitHub API — it does NOT need a repo checkout. Add NO `actions/checkout` step. (The
#   action's only repo interaction is reading OUR release; it has no use for our source tree.)

# CRITICAL (the FIRST run fails until the manual `wingetcreate new` is merged): the action's first step is a
#   pre-flight HEAD request to `winget-pkgs/tree/master/manifests/d/da/dabstractor/QMKonnect`; if the package
#   does not exist, it errors "Package dabstractor.QMKonnect does not exist in the winget-pkgs repository.
#   Please add atleast one version of the package before using this action." This is EXPECTED on the very
#   first release — the one-time manual `wingetcreate new` (see packaging/winget/README.md) must be merged
#   first. Document this in the inline comment block so a red first-run isn't mistaken for a bug.

# CRITICAL (auth = classic PAT, NOT GITHUB_TOKEN, NOT deploy key): the target is microsoft/winget-pkgs
#   (EXTERNAL). The action auto-forks it under the PAT's owner (dabstractor/winget-pkgs), pushes, and PRs to
#   microsoft:main. The default GITHUB_TOKEN is scoped to dabstractor/qmkonnect and CANNOT fork an external
#   repo. A CLASSIC PAT (Tokens (classic)) with `public_repo` scope is mandatory → WINGET_GITHUB_TOKEN secret.
#   (This differs from the AUR per-account SSH key and the Homebrew/Scoop per-repo deploy keys.)

# GOTCHA (no permissions: escalation needed): the job pushes to microsoft/winget-pkgs over the PAT
#   (inputs.token), NOT via the GITHUB_TOKEN. The action uses github.token (the default) only to READ our own
#   release, which the top-level `permissions: contents: read` permits. Do NOT add `permissions: contents:
#   write` (that's for the publish job, which creates the GitHub Release).

# GOTCHA (version from the git tag, not cargo metadata): this job builds NO Rust. Installing
#   dtolnay/rust-toolchain@stable purely to read a version is waste. On a `v*` tag push, GITHUB_REF_NAME is
#   the tag (v0.2.8); `${GITHUB_REF_NAME#v}` is the bare version (0.2.8) the action's `version` input needs.
#   The trigger `on: push: tags: - 'v*'` guarantees ref_name is a v-tag. cargo-release cuts the tag from
#   Cargo.toml, so this is transitively the Cargo.toml version (external_deps.md "Version Source of Truth").

# GOTCHA (the action internally pins cargo-binstall to @main): the winget-releaser action's composite steps
#   use `cargo-bins/cargo-binstall@main` (a moving target) + `cargo binstall komac -y`. This is the action's
#   INTERNAL choice — not something our workflow controls. If that internal dep ever breaks, the action
#   breaks for everyone (not just us). Mitigation (out of scope here): pin the action to a specific commit
#   SHA instead of `@v2`. For now `@v2` matches the contract's intent + the README.

# GOTCHA (do NOT implement Option A / submit.ps1 here): the contract names the winget-releaser action as the
#   PRIMARY mechanism. submit.ps1 (windows-latest + wingetcreate) is the documented ALTERNATIVE only. If you
#   find a gap, compensate in the JOB's inputs, not by switching to submit.ps1 or editing packaging/.

# GOTCHA (one file, one job): `git diff --stat` must show ONLY .github/workflows/release.yml. No Cargo.toml,
#   no packaging/, no docs/*. The secret doc is an INLINE COMMENT BLOCK in the job (Mode A ride-along).
```

## Implementation Blueprint

### Data models and structure

No data models. The deliverable is one YAML job. The only "data": one secret name
(`WINGET_GITHUB_TOKEN`), one PackageIdentifier (`dabstractor.QMKonnect`), one installers-regex
(`QMKonnect-.*-windows-x64\.exe$`), and the bare version (from `GITHUB_REF_NAME#v`).

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: EDIT .github/workflows/release.yml — append the `winget` job
  - PLACEMENT: at the very END of the file (after `scoop-bucket`, the current last job), at the same 2-space
    indent as `publish:` / `scoop-bucket:`. Append-only — do NOT insert between jobs.
  - IMPLEMENT: the verbatim job YAML from "Implementation Patterns → The `winget` job" below.
  - STRUCTURE (3 logical pieces, 2 steps):
      1. a `# ─── banner` comment block (the SECRET doc — Mode A ride-along).
      2. the job header (`winget:` / `name:` / `needs: [publish]` / `if: github.event_name == 'push'` /
         `runs-on: ubuntu-latest`).
      3. the `Determine version` step (id: ver) — `echo "version=${GITHUB_REF_NAME#v}" >> "$GITHUB_OUTPUT"`.
      4. the `vedantmgoyal9/winget-releaser@v2` action step with the 4 `with:` inputs.
  - NAMING: job key `winget`, `name: Publish to Winget (winget-pkgs PR)`.
  - PRESERVE: every existing job UNCHANGED. Do NOT reorder.

Task 2: ADD the inline WINGET_GITHUB_TOKEN documentation comment block (Mode A ride-along)
  - PLACEMENT: a `# ─── banner` + SECRET comment block immediately ABOVE the `winget:` job key (mirror the
    aur job's AUR_SSH_PRIVATE_KEY doc and the homebrew-tap/scoop-bucket deploy-key docs).
  - CONTENT: (a) classic PAT, `public_repo` scope; create at https://github.com/settings/tokens (classic) →
    check `public_repo`; (b) store as WINGET_GITHUB_TOKEN Actions secret in dabstractor/qmkonnect; (c) WHY a
    separate PAT (default GITHUB_TOKEN is scoped to dabstractor/qmkonnect and CANNOT fork
    microsoft/winget-pkgs); (d) the action auto-forks winget-pkgs under dabstractor/winget-pkgs + PRs to
    microsoft:main; (e) the one-time manual `wingetcreate new` prerequisite (the job's first run fails with
    "Package dabstractor.QMKonnect does not exist…" until that's merged — expected); (f) never log the token.
    (Verbatim text in Implementation Patterns.) State this is a classic PAT (not the AUR per-account SSH-key
    model, not the Homebrew/Scoop per-repo deploy-key model).

Task 3: VALIDATE (no edits)
  - YAML parse: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"`.
  - actionlint (if installed): `actionlint .github/workflows/release.yml`.
  - grep gates (Validation Level 3): the new `winget:` job key; `needs: [publish]` + `if: github.event_name
    == 'push'`; `vedantmgoyal9/winget-releaser@v2` (NOT @latest); the 4 `with:` inputs (identifier/version/
    installers-regex/token); the corrected regex (`QMKonnect`, not `QMKonnet`); `${GITHUB_REF_NAME#v}`;
    WINGET_GITHUB_TOKEN referenced + documented; NO `release-repository: dabstractor` (the owner-doubling
    bug); no extra `permissions:` line; `git diff --stat` shows ONLY release.yml.
  - (DEFERRED) Real winget-pkgs PR on a tag push with the PAT set + the manual `wingetcreate new` merged —
    see Validation Level 4.

Task 4: NEVER do these (out of scope / forbidden)
  - DO NOT use `vedantmgoyal9/winget-releaser@latest` (does not resolve — no `latest` ref). Use `@v2`.
  - DO NOT use the contract's `QMKonnet-…` regex (typo). Use `QMKonnect-.*-windows-x64\.exe$`.
  - DO NOT pass a v-prefixed version. Strip the `v` in `steps.ver` (the action uses a provided version
      verbatim).
  - DO NOT set `release-repository: dabstractor/qmkonnect` (owner-doubling → 404). OMIT release-repository
      (default `qmkonnect` is correct — the action prepends the owner).
  - DO NOT add `release-tag` / `fork-user` / `max-versions-to-keep` / `release-notes-url` — they all default
      correctly; adding them is unnecessary and risks a typo.
  - DO NOT add an `actions/checkout` step (the action reads the release via the API, not the repo).
  - DO NOT add a `permissions:` line to the job (the PAT handles the external fork/PR; top-level
      `contents: read` suffices).
  - DO NOT depend on `[windows]` (it uploads a workflow artifact, not a release asset) — use `[publish]`.
  - DO NOT drop the `if: github.event_name == 'push'` gate (the release 404s on workflow_dispatch dry-runs).
  - DO NOT implement Option A (submit.ps1 / wingetcreate) — the contract specifies the winget-releaser action.
  - DO NOT modify packaging/winget/* (manifest + submit.ps1 + README are INPUT, Complete).
  - DO NOT change any Rust/Cargo/docs file or edit PRD.md/tasks.json/prd_snapshot.md.
```

### Implementation Patterns & Key Details

```yaml
# ===== The `winget` job (VERBATIM — author exactly this, appended at the END of release.yml) =====

  # ─────────────────────────────────────────────────────────────────────────
  # Winget — open a per-release PR to microsoft/winget-pkgs for
  # dabstractor.QMKonnect (the Windows community channel, PRD §4 F15).
  #
  # Uses vedantmgoyal9/winget-releaser@v2 (Komac under the hood), which finds
  # the release's .exe via installers-regex, computes its SHA256, syncs a fork of
  # microsoft/winget-pkgs under dabstractor/winget-pkgs, and opens an update PR
  # (fork -> microsoft:main). Runs on ubuntu-latest (Komac is cross-platform; no
  # windows-latest + wingetcreate install needed).
  #
  # Runs AFTER `publish` (the action reads the GitHub Release by tag to find the
  # .exe asset — release-repository/release-tag default to this repo + the pushed
  # tag) and only on real tag pushes (workflow_dispatch dry-runs don't publish a
  # release, so the releases/tags/<tag> API call would 404).
  #
  # PREREQUISITE: dabstractor.QMKonnect must ALREADY exist in winget-pkgs (the
  #   one-time manual `wingetcreate new` — see packaging/winget/README.md
  #   "Publishing to microsoft/winget-pkgs"). Until that first PR is merged, this
  #   job's first run errors with "Package dabstractor.QMKonnect does not exist
  #   in the winget-pkgs repository. Please add atleast one version of the
  #   package before using this action." — EXPECTED on the very first release.
  #
  # SECRET — WINGET_GITHUB_TOKEN (REQUIRED for this job to do anything):
  #   A CLASSIC GitHub PAT with the `public_repo` scope. The default GITHUB_TOKEN
  #   is scoped to dabstractor/qmkonnect only and CANNOT fork
  #   microsoft/winget-pkgs. winget-releaser/Komac auto-creates the fork
  #   <owner>/winget-pkgs (fork-user defaults to dabstractor) under the token's
  #   account, pushes a branch, and PRs to microsoft/winget-pkgs:main.
  #
  #   One-time setup:
  #     1. Create a CLASSIC PAT: https://github.com/settings/tokens ->
  #        Tokens (classic) -> "Generate new token (classic)" -> check
  #        `public_repo` (the only scope needed to fork a public repo + open a
  #        PR). Use a CLASSIC token, not fine-grained.
  #     2. Store it as the WINGET_GITHUB_TOKEN Actions secret in
  #        dabstractor/qmkonnect (Settings -> Secrets and variables -> Actions ->
  #        New repository secret).
  #     3. (One-time, manual, on a Windows host) submit the first version with
  #        `wingetcreate new <installerURL> --token <PAT> --submit` (fill
  #        metadata from packaging/winget/*.yaml) so dabstractor.QMKonnect exists
  #        in winget-pkgs — see packaging/winget/README.md.
  #
  #   This is a CLASSIC PAT (not the AUR per-account SSH-key model, not the
  #   Homebrew/Scoop per-repo deploy-key model). Until this secret is set, the
  #   job runs but Komac fails to fork/push with a 401/403 — expected until the
  #   PAT is configured. The token is never logged (it travels only inside the
  #   action's inputs.token).
  # ─────────────────────────────────────────────────────────────────────────
  winget:
    name: Publish to Winget (winget-pkgs PR)
    needs: [publish]
    if: github.event_name == 'push'
    runs-on: ubuntu-latest
    steps:
      # This job builds NO Rust. The workflow only triggers on `v*` tags, so
      # GITHUB_REF_NAME is the tag (v0.2.8); stripping the leading 'v' yields
      # the bare version (0.2.8) the winget-releaser `version` input needs.
      # The action uses a PROVIDED version VERBATIM (it strips a leading 'v'
      # ONLY when version is OMITTED — see the action's action.yml), so we MUST
      # pass a bare value here — never the raw v-prefixed tag.
      - name: Determine version
        id: ver
        run: echo "version=${GITHUB_REF_NAME#v}" >> "$GITHUB_OUTPUT"

      # winget-releaser finds the installer in the GitHub Release (release-tag
      # defaults to github.ref_name on a push event; release-repository defaults
      # to the repo name `qmkonnect` and the action prepends the owner ->
      # dabstractor/qmkonnect), hashes it, and opens the PR. fork-user defaults
      # to dabstractor (-> dabstractor/winget-pkgs). OMIT release-repository:
      # setting `dabstractor/qmkonnect` would DOUBLE the owner (action.yml
      # prepends github.repository_owner itself) -> 404.
      - name: Submit winget-pkgs PR (winget-releaser)
        uses: vedantmgoyal9/winget-releaser@v2
        with:
          identifier: dabstractor.QMKonnect
          # Bare version (no leading 'v'); see the ver step above.
          version: ${{ steps.ver.outputs.version }}
          installers-regex: 'QMKonnect-.*-windows-x64\.exe$'
          token: ${{ secrets.WINGET_GITHUB_TOKEN }}
```

```yaml
# PATTERN (tag-only gate — identical to the publish + aur + homebrew-tap + scoop-bucket jobs):
needs: [publish]
if: github.event_name == 'push'

# PATTERN (version from the git tag — build-less jobs avoid the cargo toolchain):
echo "version=${GITHUB_REF_NAME#v}" >> "$GITHUB_OUTPUT"
# (consistency alternative: v=$(cargo metadata --no-deps --format-version 1 | jq -r '...') — but that needs
#  a dtolnay/rust-toolchain@stable step BEFORE it; ref_name does not. This job uses ref_name for consistency
#  with the sibling homebrew-tap/scoop-bucket jobs.)

# PATTERN (the action invocation — 4 inputs, the rest defaulted):
- uses: vedantmgoyal9/winget-releaser@v2
  with:
    identifier: dabstractor.QMKonnect
    version: ${{ steps.ver.outputs.version }}
    installers-regex: 'QMKonnect-.*-windows-x64\.exe$'
    token: ${{ secrets.WINGET_GITHUB_TOKEN }}
# DEFAULTS we rely on (do NOT set them): release-repository = github.event.repository.name (`qmkonnect`,
#   name only — action prepends owner); release-tag = github.event.release.tag_name || github.ref_name (=
#   ref_name on push = the tag); fork-user = github.repository_owner (`dabstractor`); max-versions-to-keep
#   = `0` (keep all).

# WHY no checkout: the action reads the release via the GitHub API (releases/tags/<tag>); it has no use for
#   the source tree. (The AUR/Homebrew/Scoop jobs check out to RUN a script; this job runs an action that
#   needs no source.)

# WHY no permissions: escalation: the external fork/PR is done over inputs.token (the PAT), not the
#   GITHUB_TOKEN. The action uses github.token only to READ our own release, which top-level contents:read
#   permits.
```

### Integration Points

```yaml
GITHUB WORKFLOW:
  - add to: .github/workflows/release.yml (append the `winget` job at the END, after scoop-bucket)
  - job key: `winget`; needs: [publish]; if: github.event_name == 'push'; runs-on: ubuntu-latest
SECRET (one-time, documented inline — Mode A):
  - WINGET_GITHUB_TOKEN: CLASSIC PAT (Tokens (classic)) with `public_repo` scope. Create at
    https://github.com/settings/tokens → check `public_repo` → store as the Actions secret in
    dabstractor/qmkonnect. NOT the default GITHUB_TOKEN (can't fork winget-pkgs). NOT a deploy key (those
    are for repos WE own; winget-pkgs is external).
EXTERNAL DEPENDENCY (unchanged by this task):
  - vedantmgoyal9/winget-releaser@v2  (composite action; uses Komac; ref `refs/tags/v2` confirmed via
    git ls-remote). The action internally depends on cargo-bins/cargo-binstall@main + Komac.
  - microsoft/winget-pkgs  (the PR target — package must pre-exist via the manual `wingetcreate new`)
CONSUMES:
  - the GitHub Release's `QMKonnect-<ver>-windows-x64.exe` asset (the action finds it by tag + regex) — hence needs:[publish]
  - packaging/winget/*.yaml (P1.M3.T2.S1, Complete) — the manifest template (NOT modified; only used by the manual `wingetcreate new`)
PRODUCES:
  - the `winget` job in release.yml + its inline secret doc (ONE file changed)
PARALLEL / SIBLING (zero conflicts):
  - P1.M5.T1.S1 (aur, Complete) + P1.M5.T1.S2 (homebrew-tap + scoop-bucket, landed): independent siblings;
    append the `winget` job AFTER them. Each needs:[publish]; auth models differ (aur=SSH key,
    homebrew/scoop=deploy keys, winget=classic PAT) but the jobs are independent.
  - P1.M5.T2.S2 (Nix flake check + asdf plugin CI, downstream): separate job(s); no overlap with this task.
PLATFORM VALIDATION:
  - Linux dev box: YAML parse + actionlint + grep gates (no PAT needed locally).
  - Real winget-pkgs PR: deferred to CI on a tag push with WINGET_GITHUB_TOKEN set AND the one-time manual
    `wingetcreate new` merged (Validation Level 4).
```

## Validation Loop

> The implementing agent runs on a **Linux dev box** with NO `WINGET_GITHUB_TOKEN` PAT and no winget-pkgs
> write access. The local gates prove the YAML is well-formed and the job is structurally correct (including
> the FOUR contract corrections). The real winget-pkgs PR is deferred to CI.

### Level 1: YAML well-formedness (runs on Linux)
```bash
cd /home/dustin/projects/qmkonnect
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml')); print('release.yml: valid YAML')"
# Expected: "release.yml: valid YAML". If it errors → fix indentation (GH Actions YAML is indent-sensitive;
#   the new job must align at the same 2-space column as `publish:` / `scoop-bucket:`).
actionlint .github/workflows/release.yml 2>/dev/null || echo "(actionlint not installed — YAML parse is the gate)"
# Expected: clean (or "not installed"). Address any actionlint errors (e.g. an unknown job in needs, a
#   malformed `uses:` reference, an invalid `if:`).
git diff --stat    # Expected: ONLY .github/workflows/release.yml (1 file). Nothing else.
```

### Level 2: local sanity — the action reference resolves + the inputs are sound (runs on Linux)
```bash
cd /home/dustin/projects/qmkonnect
# Confirm the action's @v2 tag exists (so `uses:` will resolve in CI):
git ls-remote --tags https://github.com/vedantmgoyal9/winget-releaser.git | grep -E 'refs/tags/v2$' \
  && echo "OK: vedantmgoyal9/winget-releaser@v2 resolves"
# Expected: "refs/tags/v2" + "OK: … @v2 resolves". (Confirms @v2; proves @latest would NOT resolve.)
# Confirm the winget packaging artifacts the job's docs reference are intact (the job does NOT touch them):
test -f packaging/winget/submit.ps1 && echo "submit.ps1 present (Option A alt, not used by this job)"
test -f packaging/winget/README.md && grep -q 'Publishing to microsoft/winget-pkgs' packaging/winget/README.md \
  && echo "winget README publishing section present (the PAT/prereq doc this job's comment points at)"
# Confirm the asset name the regex targets matches the windows job's rename:
grep -q 'QMKonnect-.*-windows-x64.exe' packaging/winget/dabstractor.QMKonnect.installer.yaml \
  && echo "installer.yaml asset name matches the installers-regex"
```

### Level 3: grep invariants — the job is structurally correct AND the 4 contract corrections are applied (runs on Linux)
```bash
cd /home/dustin/projects/qmkonnect
F=.github/workflows/release.yml
grep -nE '^\s{2}winget:' "$F"                              # Expected: 1 (the new job key)
grep -nE 'vedantmgoyal9/winget-releaser@v2' "$F"           # Expected: 1 (the action reference). MUST be @v2.
! grep -qE 'vedantmgoyal9/winget-releaser@latest' "$F" && echo "OK: no @latest (would not resolve)"   # Expected: OK
awk '/^  winget:/{j=1} j&&/needs: \[publish\]/{print FILENAME":"NR": "$0; j=0}' "$F"   # Expected: 1 line
awk '/^  winget:/{j=1} j&&/if: github\.event_name == .push./{print FILENAME":"NR": "$0; j=0}' "$F"  # Expected: 1 line
grep -nE 'identifier: dabstractor\.QMKonnect' "$F"         # Expected: 1 (the action input)
grep -nE 'version: \$\{\{ steps\.ver\.outputs\.version \}\}' "$F"  # Expected: 1 (bare version, verbatim-passed)
grep -nE "installers-regex: 'QMKonnect-\.\*-windows-x64\\\\\.exe\$'" "$F"  # Expected: 1 (QMKonnect, TWO c's)
! grep -qiE "installers-regex: 'QMKonnet-" "$F" && echo "OK: no QMKonnet typo (one c)"   # Expected: OK
grep -nE 'token: \$\{\{ secrets\.WINGET_GITHUB_TOKEN \}\}' "$F"     # Expected: 1 (the PAT input)
grep -nE 'WINGET_GITHUB_TOKEN' "$F"                         # Expected: >=2 (1× token input + >=1× comment doc block)
grep -nE '\$\{GITHUB_REF_NAME#v\}' "$F"                     # Expected: >=1 under the winget job (the bare-version step)
! grep -qE 'release-repository: dabstractor/qmkonnect' "$F" && echo "OK: no owner-doubling release-repository"  # Expected: OK
! grep -qE 'release-repository:' "$F" && echo "OK: release-repository omitted (defaults to repo name)"           # Expected: OK
# Confirm NO existing job was disturbed + the new job is appended (not inserted):
grep -cE '^\s{2}(macos|windows|linux-binary|arch|publish|aur|homebrew-tap|scoop-bucket):' "$F"   # Expected: 8 (unchanged originals)
# Confirm the winget job has NO permissions: escalation of its own:
awk '/^  winget:/{j=1} /^  [a-z]/{if($0 !~ /^  winget:/)j=0} j&&/permissions:/{print FILENAME":"NR": "$0}' "$F"  # Expected: NOTHING
```

### Level 4: Real winget-pkgs PR on a tag push (DEFERRED — CI, needs the PAT + the manual first submission)
```bash
# NOT run from the dev box (no PAT, and dabstractor.QMKonnect does not yet exist in winget-pkgs). On a real
# release:
#   1. (once) create the classic PAT (public_repo) -> store as WINGET_GITHUB_TOKEN.
#   2. (once, manual, Windows host) run `wingetcreate new <url> --token <PAT> --submit` for the FIRST
#      version, fill metadata from packaging/winget/*.yaml, get the initial PR merged (so the package
#      exists in winget-pkgs). Until this is done, the `winget` job's pre-flight errors "Package
#      dabstractor.QMKonnect does not exist…" — EXPECTED.
#   3. git tag v0.2.9 && git push origin v0.2.9  -> triggers the full pipeline.
#   4. After macos/windows/linux-binary/arch + publish go green, `winget` runs.
# Verify post-publish:
#   - The job log shows: the `Determine version` step (version=0.2.9), then winget-releaser's Komac output
#     (sync-fork, komac update --version 0.2.9 --submit, cleanup).
#   - A PR appears in https://github.com/microsoft/winget-pkgs (or the action reports its URL).
#   - Once merged: `winget upgrade dabstractor.QMKonnect` serves 0.2.9 on a Windows box.
```

## Final Validation Checklist

### Technical Validation
- [ ] Level 1 (YAML parse) + actionlint clean; `git diff --stat` shows ONLY `.github/workflows/release.yml`.
- [ ] Level 2: `git ls-remote` confirms `vedantmgoyal9/winget-releaser@v2` resolves; the winget packaging
      artifacts the comment references exist.
- [ ] Level 3: all grep invariants pass (job key; @v2 not @latest; needs:[publish]+if push; the 4 `with:`
      inputs; `QMKonnect` regex not `QMKonnet`; bare `${GITHUB_REF_NAME#v}` version; WINGET_GITHUB_TOKEN
      referenced+documented; NO owner-doubling `release-repository`; no extra `permissions:`; 8 original
      jobs unchanged).
- [ ] Level 4 (deferred to CI): on a tag push with the PAT set + the manual `wingetcreate new` merged, the
      action opens a PR to microsoft/winget-pkgs.

### Feature Validation
- [ ] All success criteria from "What" met.
- [ ] The `winget` job is appended at the END (after scoop-bucket); no existing job reordered/changed.
- [ ] The inline comment block documents WINGET_GITHUB_TOKEN (classic PAT, public_repo, why not
      GITHUB_TOKEN, the manual-first prerequisite, never-log).
- [ ] (Deferred) `winget upgrade dabstractor.QMKonnect` serves the new version once the CI PR is merged.

### Code Quality Validation
- [ ] Mirrors the sibling jobs' idioms (banner comment, tag-only gate, ref_name#v version step).
- [ ] The 4 contract corrections applied (@v2; QMKonnect; bare version; OMIT release-repository).
- [ ] Anti-patterns avoided (see below).

### Documentation & Deployment
- [ ] The inline comment block is self-contained (a maintainer can set up the PAT + understand the
      manual-first prerequisite from it alone, cross-referencing packaging/winget/README.md).
- [ ] No new env vars beyond the documented secret.

---

## Anti-Patterns to Avoid

- ❌ Don't use `@latest` — pin to `@v2` (the action's only moving major tag; `@latest` doesn't resolve).
- ❌ Don't pass a v-prefixed version — strip the `v` in `steps.ver` (the action uses a provided version verbatim).
- ❌ Don't set `release-repository: dabstractor/qmkonnect` — the action prepends the owner → doubles it → 404. OMIT it.
- ❌ Don't use the contract's `QMKonnet` typo — the asset is `QMKonnect`.
- ❌ Don't add a checkout step (the action reads the release via the API, not the repo).
- ❌ Don't add a `permissions:` escalation (the PAT handles the external fork/PR; contents:read suffices).
- ❌ Don't depend on `[windows]` instead of `[publish]` (the action needs the published release asset, not the workflow artifact).
- ❌ Don't implement Option A (submit.ps1) — the contract specifies the winget-releaser action.
- ❌ Don't modify packaging/winget/* — they are INPUT (Complete); this job only consumes the action.
- ❌ Don't mistake the expected "Package dabstractor.QMKonnect does not exist…" first-run failure for a bug
      — it's the documented manual-`wingetcreate new` prerequisite.

---

## Confidence Score

**9/10** for one-pass implementation success. The deliverable is a single, verbatim-specified YAML job
appended to one file; the load-bearing external action's `action.yml` was read in full this session
(inputs, the push-event release-tag fallback, the verbatim version handling, the pre-flight existence check
all confirmed); the FOUR load-bearing contract corrections (`@v2`, `QMKonnect`, bare `version`, OMIT
`release-repository`) are each grep-gateable on the Linux dev box; and the job mirrors the idioms of four
already-landed sibling jobs. The one deferred risk (the real winget-pkgs PR) is honestly gated to CI and
depends on the one-time manual `wingetcreate new` + the PAT — neither of which is available locally, and
neither of which this task can or should pre-provision.