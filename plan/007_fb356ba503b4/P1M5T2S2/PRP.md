# PRP — P1.M5.T2.S2: Add Nix flake check + asdf plugin test + asdf publish CI steps

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`, org `dabstractor`). **TWO files edited:**
> `.github/workflows/ci.yml` (+ ONE job: `nix-check`) and `.github/workflows/release.yml` (+ ONE job:
> `asdf-plugin`). **No new files. No Rust. No Cargo.toml. No flake.nix. No packaging/. No docs/*.**
> The `ASDF_PLUGIN_DEPLOY_KEY` secret is documented as an inline comment block (Mode A ride-along),
> mirroring the sibling `homebrew-tap`/`scoop-bucket`/`winget` jobs.
>
> **What this does:** (1) **ci.yml** gets a `nix-check` job that installs Nix via
> `cachix/install-nix-action@v31` and runs **`nix flake check --no-build`** (eval-only) on every push
> to main — validating the in-repo `flake.nix` without publishing (the flake is consumed by users via
> `nix run github:dabstractor/qmkonnect`, so there is no publish step). (2) **release.yml** gets an
> `asdf-plugin` job that, on each `v*` tag push, (a) **TESTS** the plugin scripts with `shellcheck` +
> a `publish.sh --dry-run` against a local mock git remote (no secrets), then (b) **PUBLISHES** them
> to `dabstractor/asdf-qmkonnect` over an SSH deploy key (the same per-repo deploy-key model as
> Homebrew/Scoop; serves both asdf AND mise).
>
> **LOAD-BEARING CORRECTION (Nix):** the contract says "runs `nix flake check`". `flake.nix` ships
> with `cargoHash = pkgs.lib.fakeHash;` — a deliberate placeholder from P1.M1.T2.S2. A BUILDING
> `nix flake check` would FAIL with a cargo-vendor hash mismatch on every push to main. The job uses
> **`--no-build`** (eval-only), which passes today AND still catches real flake breakage. Resolving
> the placeholder (a `flake.nix` edit) is a separate, out-of-scope follow-up. See research/notes.md §1
> for the verbatim evidence.

---

## Goal

**Feature Goal**: complete the F15 community-distribution CI pipeline (PRD §4 F15) by (1) validating
the in-repo Nix flake on every push to main (`nix-check`), and (2) automatically publishing the
asdf plugin to `dabstractor/asdf-qmkonnect` on every release (`asdf-plugin`) — so users can
`nix run github:dabstractor/qmkonnect` and `asdf plugin add qmkonnect …` / `mise plugin add
qmkonnect …` against freshly-released versions, with no maintainer intervention beyond the one-time
deploy-key setup.

**Deliverable** (exactly TWO files edited, TWO jobs added — one per file):
- `.github/workflows/ci.yml` — a `nix-check` job (`runs-on: ubuntu-latest`) that installs Nix via
  `cachix/install-nix-action@v31` (flakes+nix-command enabled by default; `access-tokens` to avoid
  GitHub API rate limits when locking the absent `flake.lock`), then runs **`nix flake check --no-build`**.
- `.github/workflows/release.yml` — an `asdf-plugin` job (`needs: [publish]`,
  `if: github.event_name == 'push'`, `runs-on: ubuntu-latest`) that: determines the bare version
  (`${GITHUB_REF_NAME#v}`); **tests** the plugin (`shellcheck` + `publish.sh --dry-run` against a
  local `file://` mock remote); loads the `ASDF_PLUGIN_DEPLOY_KEY` deploy key via
  `webfactory/ssh-agent@v0.9.0`; then runs `packaging/asdf/publish.sh` to publish. One inline comment
  block documents `ASDF_PLUGIN_DEPLOY_KEY` (Mode A ride-along).

**Success Definition**:
- Both `ci.yml` and `release.yml` parse as valid YAML; `actionlint` (if installed) is clean;
  `git diff --stat` shows **ONLY** those two files.
- `ci.yml` has a new `nix-check` job appended after `build-and-test`, running `nix flake check --no-build`
  with **NO** `flake.nix` edit (the fakeHash placeholder is left untouched).
- `release.yml` has a new `asdf-plugin` job appended after `winget` (the last job), with
  `needs: [publish]` + `if: github.event_name == 'push'`, that runs BOTH the test steps AND the publish
  step (publish.sh), using `webfactory/ssh-agent@v0.9.0` + `ASDF_PLUGIN_DEPLOY_KEY`.
- The `asdf-plugin` job's `Determine version` step outputs a **bare** version (publish.sh rejects a
  leading `v`); the deploy key is configured BEFORE `publish.sh` (its `git clone` uses SSH).
- The `ASDF_PLUGIN_DEPLOY_KEY` inline comment block documents the per-repo deploy-key setup
  (ed25519, public half on `dabstractor/asdf-qmkonnect`, write access, private half = the secret, the
  repo must pre-exist) and states it is the SAME model as Homebrew/Scoop (NOT AUR, NOT Winget).
- (Deferred) On a real tag push with `ASDF_PLUGIN_DEPLOY_KEY` set AND `dabstractor/asdf-qmkonnect`
  pre-created, `publish.sh` pushes the plugin; `nix-check` passes green on the next push to main.

## User Persona (if applicable)

**Target User**: (1) the Nix/NixOS user who runs `nix run github:dabstractor/qmkonnect` or imports the
`nixosModules.default`; (2) the asdf/mise user who runs `asdf plugin add qmkonnect
https://github.com/dabstractor/asdf-qmkonnect` / `mise plugin add qmkonnect …`. Before this task, a
broken `flake.nix` could ship unvalidated, and the asdf plugin lagged until a maintainer manually ran
`publish.sh`. After this task, the flake is validated on every push, and the plugin auto-publishes on
every release.

**Use Case**: maintainer cuts `v0.2.9`; CI builds all platforms, publishes the GitHub Release, then the
`asdf-plugin` job publishes `asdf-qmkonnect` (synced scripts + stamped `0.2.9` examples). Separately,
every push to `main` runs `nix-check`, so a syntax/type regression in `flake.nix` fails CI before merge.
The Nix channel needs no publish (users pull the flake from the repo).

**Pain Points Addressed**: closes the last two gaps in the F15 pipeline — the Nix validation gate
(external_deps.md §5: "Flake lives in the repo; validation in CI (`nix flake check`)") and the asdf/mise
publish automation (external_deps.md §6/§7: asdf plugin repo + the "CI approach: push to plugin repo on
tag" model). AUR (P1.M5.T1.S1), Homebrew+Scoop (P1.M5.T1.S2), Winget (P1.M5.T2.S1) are the sibling jobs.

## Why

- **F15 (PRD §4) requires automated Nix validation + asdf/mise publishing.** external_deps.md §5
  ("CI runs `nix flake check`"; "No external publishing needed" for Nix) and §6/§7 ("CI approach: push
  to plugin repo on tag" for asdf) mandate exactly these two jobs. The flake (P1.M1.T2.S2) and
  `packaging/asdf/publish.sh` (P1.M4.T1.S2) already exist — this task only wires the CI.
- **The artifacts already exist; this task wires the pipeline.** `flake.nix` (P1.M1.T2.S2, Complete)
  is the in-repo flake (consumed read-only). `packaging/asdf/{publish.sh,bin/,lib/,README.md,.tool-versions,
  mise.toml,CHANGELOG.md}` (P1.M4.T1.S1/S2, Complete) is the plugin + its publisher. This task adds NO
  new packaging artifacts — only the two workflow jobs that drive them.
- **asdf publish is AFTER `publish` and tag-only** for ordering/semantics: `publish.sh` stamps the
  version into the plugin's example `.tool-versions`/`mise.toml` and references it in the commit message,
  so the plugin should reflect a RELEASED version. (Unlike AUR/Homebrew/Scoop, asdf resolves versions at
  runtime via `bin/list-all`'s GitHub Releases API call — `publish.sh` downloads NO release asset — so the
  release dependency is only the version string; `needs:[publish]` keeps the ordering honest and matches
  every sibling job.)
- **The asdf auth model = Homebrew/Scoop deploy key.** The target `dabstractor/asdf-qmkonnect` is an
  external repo WE OWN. `publish.sh` clones + pushes over SSH (`git@github.com:dabstractor/asdf-qmkonnect.git`).
  A per-repo GitHub deploy key (write access) + `webfactory/ssh-agent@v0.9.0` is the exact pattern the
  `homebrew-tap` and `scoop-bucket` jobs already use. This is NOT the AUR per-account SSH-key model and
  NOT the Winget classic-PAT-to-an-unowned-repo model.

## What

### Approach: two jobs, ubuntu runners, install-nix-action + webfactory/ssh-agent + publish.sh

**`nix-check` (ci.yml):**
1. Appended after `build-and-test`. Runs on `ubuntu-latest`.
2. `actions/checkout@v4`.
3. `cachix/install-nix-action@v31` with `extra_nix_config: access-tokens = github.com=${{ secrets.GITHUB_TOKEN }}`
   (flakes+nix-command are enabled by the action by default — NO `experimental-features` config needed;
   the token lifts the GitHub API rate limit when nix re-resolves the flake inputs, since `flake.lock`
   is absent). Uses the default `GITHUB_TOKEN` — no new secret.
4. **`nix flake check --no-build`** (eval-only — REQUIRED because the shipped `cargoHash` is `fakeHash`;
   see the load-bearing correction in the job's banner comment).

**`asdf-plugin` (release.yml):**
1. Appended after `winget`. `needs: [publish]`, `if: github.event_name == 'push'`, `runs-on: ubuntu-latest`.
2. `actions/checkout@v4`.
3. `Determine version` (id: `ver`) — `echo "version=${GITHUB_REF_NAME#v}" >> "$GITHUB_OUTPUT"` (bare;
   publish.sh rejects a leading `v`).
4. **TEST (no secrets):**
   - `shellcheck packaging/asdf/bin/* packaging/asdf/lib/*.bash packaging/asdf/publish.sh`
     (preinstalled on ubuntu-latest; verified clean on the dev box).
   - `publish.sh --dry-run` against a local `git init --bare /tmp/asdf-mock.git` mock remote via
     `ASDF_QMKONNECT_REMOTE=file:///tmp/asdf-mock.git` — exercises the full clone→sync→chmod→sed→commit
     flow, skipping only the push.
5. `webfactory/ssh-agent@v0.9.0` with `ssh-private-key: ${{ secrets.ASDF_PLUGIN_DEPLOY_KEY }}` (BEFORE
   publish.sh — its `git clone` uses SSH).
6. `./publish.sh "$VERSION"` (`working-directory: packaging/asdf`) — the real publish to
   `dabstractor/asdf-qmkonnect`. publish.sh sets its OWN git identity, so the job does NOT.

### Success Criteria

- [ ] `.github/workflows/ci.yml` has a new `nix-check` job appended AFTER `build-and-test` (3 jobs total),
      `runs-on: ubuntu-latest`, using `cachix/install-nix-action@v31` and running `nix flake check --no-build`.
- [ ] The nix-check job uses **`--no-build`** (grep for it); it makes NO edit to `flake.nix` (the
      `cargoHash = pkgs.lib.fakeHash;` placeholder is untouched — `git diff` shows no `flake.nix` change).
- [ ] `.github/workflows/release.yml` has a new `asdf-plugin` job appended AFTER `winget` (the last job),
      with `needs: [publish]`, `if: github.event_name == 'push'`, `runs-on: ubuntu-latest`.
- [ ] The asdf-plugin job runs BOTH a `shellcheck` step AND a `publish.sh --dry-run` step (against a
      `file://` mock remote) BEFORE the deploy-key/publish steps.
- [ ] The asdf-plugin job loads `ASDF_PLUGIN_DEPLOY_KEY` via `webfactory/ssh-agent@v0.9.0` BEFORE running
      `./packaging/asdf/publish.sh "$VERSION"`.
- [ ] The `Determine version` step outputs the BARE version (`${GITHUB_REF_NAME#v}`); the publish step
      passes it as `./publish.sh "$VERSION"` with `working-directory: packaging/asdf`.
- [ ] An inline comment block documents `ASDF_PLUGIN_DEPLOY_KEY`: ed25519 deploy key; public half on
      `dabstractor/asdf-qmkonnect` (write access); private half = the Actions secret; the plugin repo must
      pre-exist; SAME model as Homebrew/Scoop (NOT AUR, NOT Winget); expected first-run "Permission denied
      (publickey)" until the key is set.
- [ ] `git diff --stat` shows ONLY `.github/workflows/ci.yml` and `.github/workflows/release.yml`.

## All Needed Context

### Context Completeness Check

_Pass._ An agent with no prior Nix/asdf-CI knowledge can implement this from: the verbatim job YAML for
BOTH jobs (Implementation Patterns); the load-bearing `--no-build` correction (with evidence + the
follow-up path); the EXACT install-nix-action facts (flakes auto-enabled, `@v31`, `access-tokens`
rationale); the asdf deploy-key model (= homebrew-tap/scoop-bucket, NOT AUR/Winget); the publish.sh
contract (`--dry-run` exists, rejects a leading `v`, sets its own git identity, `ASDF_QMKONNECT_REMOTE`
override); the verified-clean shellcheck gate; and the precise gotchas (no flake.nix edit; deploy key
before publish.sh; append after winget; deferred real gates). The Linux dev box validates via YAML parse
+ actionlint + grep gates (Nix is not installed locally → its real eval is a deferred CI gate; shellcheck
IS installed → the asdf test's shellcheck half can be validated locally).

### Documentation & References

```yaml
# MUST READ — the two files being edited (mirror their idioms; APPEND each new job at the END)
- file: .github/workflows/ci.yml
  why: "the existing CI jobs. MIRROR: (a) the top-level `env:` + `concurrency:` (do NOT touch); (b) the
        `fmt` + `build-and-test` jobs' `actions/checkout@v4` + `runs-on: ubuntu-22.04/ubuntu-latest`
        idiom; (c) the absence of a top-level `permissions:` block (the default GITHUB_TOKEN has read
        access — fine for nix-check). APPEND the `nix-check` job at the very END (after build-and-test).
        Do NOT change fmt/build-and-test or the triggers."
  pattern: "2-space indent job keys at column 2; `runs-on: ubuntu-latest`; `actions/checkout@v4` first step;
            a `# ─── banner` comment block above the job documenting the --no-build decision."

- file: .github/workflows/release.yml
  why: "the existing release jobs. MIRROR: (a) the `publish` + `aur`/`homebrew-tap`/`scoop-bucket`/`winget`
        jobs' tag-only gate `needs: [publish]` + `if: github.event_name == 'push'` (my job copies it);
        (b) the homebrew-tap (line ~439) + scoop-bucket (line ~518) `webfactory/ssh-agent@v0.9.0` deploy-key
        steps (my job copies the SAME pin + the SAME secret-loading idiom); (c) the inline HOMEBREW_TAP_DEPLOY_KEY
        + SCOOP_BUCKET_DEPLOY_KEY doc blocks (Mode A ride-along) for the ASDF_PLUGIN_DEPLOY_KEY block; (d) the
        sibling jobs' `${GITHUB_REF_NAME#v}` `Determine version` step (build-less jobs avoid the cargo
        toolchain). APPEND the `asdf-plugin` job at the very END (after `winget`, the current last job)."
  pattern: "needs: [publish]; if: github.event_name == 'push'; runs-on: ubuntu-latest; a `# ─── banner`
            comment block above the job documenting the secret; a `Determine version` (id: ver) step; a
            `webfactory/ssh-agent@v0.9.0` step before the publish step."

# MUST READ — the in-repo flake (consumed READ-ONLY; DO NOT EDIT)
- file: flake.nix
  why: "the flake the nix-check job validates. CRITICAL: it ships `cargoHash = pkgs.lib.fakeHash;` — a
        deliberate placeholder (its own comment explains the 2-step iteration to capture the real hash).
        This is WHY the job uses `nix flake check --no-build` (a building check would fail with a hash
        mismatch). The job makes NO edit to this file."
  pattern: "the flake's outputs: packages.default (buildRustPackage, fakeHash), devShells.default (mkShell),
            nixosModules.default (NixOS module). `--no-build` evaluates all three without instantiating the build."
  gotcha: "fakeHash → MUST use --no-build. Resolving cargoHash (a flake.nix edit) is a separate, OUT-OF-SCOPE
           follow-up. Documented in the job's banner comment."

# MUST READ — the publisher (the asdf-plugin job RUNS this; understand its contract)
- file: packaging/asdf/publish.sh
  why: "the script the asdf-plugin job runs (twice: --dry-run for the test, then real). CONTRACT:
        (1) it has a `--dry-run` flag that clones+syncs+stages+commits but SKIPS the push (the test step);
        (2) it rejects a leading 'v' (`case \"$VERSION\" in v*)` guard) → pass the BARE version;
        (3) `REMOTE` defaults to `git@github.com:dabstractor/asdf-qmkonnect.git` and is overridable via
        `ASDF_QMKONNECT_REMOTE` (the dry-run sets it to a `file://` mock); (4) it sets its OWN git identity
        (step 5) — the job does NOT set git config (UNLIKE homebrew-tap/scoop-bucket); (5) it requires the
        plugin files bin/{list-all,download,install} + lib/utils.bash + README.md + .tool-versions + mise.toml
        + CHANGELOG.md to exist (it fails fast if any is missing); (6) it verifies bin/* land as 100755 in the
        git index (the dry-run validates this)."
  gotcha: "the `git clone` uses SSH → the deploy key (webfactory/ssh-agent) MUST be configured BEFORE
           running publish.sh (the dry-run uses a file:// mock, so no key for it)."

# MUST READ — the asdf plugin doc (platform support, version resolution — confirms what the job publishes)
- file: packaging/asdf/README.md
  why: "documents the plugin the job publishes: asdf + mise compatibility, Linux/macOS/Windows support,
        version resolution via the GitHub Releases API at runtime (so publish.sh downloads NO release asset).
        The job does NOT touch this file."

# MUST READ — the architecture decisions this implements
- docfile: plan/007_fb356ba503b4/architecture/external_deps.md
  why: "§5 Nix ('Flake lives in the repo; validation in CI (`nix flake check`)' + 'No external publishing
        needed'); §6 mise + §7 asdf ('CI approach: push to plugin repo on tag', 'this plugin also serves
        mise users'); 'CI Publishing Strategy' ('For channels requiring repo pushes: deploy keys/tokens as
        GitHub Actions secrets; on tag push, git clone → update → commit → push'); 'Version Source of Truth'
        (derive from Cargo.toml — here transitively via the git tag)."
  section: "5. Nix Flake (Linux)" + "6. mise" + "7. asdf" + "CI Publishing Strategy" + "Version Source of Truth"

# MUST READ — the verbatim decisions + evidence (fakeHash→--no-build, install-nix-action facts, deploy-key model)
- docfile: plan/007_fb356ba503b4/P1M5T2S2/research/notes.md
  why: "(1) the LOAD-BEARING fakeHash→--no-build correction with verbatim flake.nix evidence + the Nix
        manual semantics; (2) install-nix-action flakes-auto-enabled + the @v31 pin + the access-tokens
        rationale (absent flake.lock); (3) the asdf deploy-key model = homebrew-tap/scoop-bucket (webfactory/
        ssh-agent@v0.9.0, secret ASDF_PLUGIN_DEPLOY_KEY, publish.sh sets its own git identity); (4) the
        verified-clean shellcheck gate + the publish.sh --dry-run test (file:// mock, no secret); (5) job
        placement (ci.yml: append after build-and-test; release.yml: append after winget)."

# EXTERNAL — confirm the install-nix-action major + that flakes are auto-enabled
- url: https://github.com/cachix/install-nix-action
  why: "the action README: 'The experimental flakes and nix-command features are enabled' (disable by
        overriding experimental-features in extra_nix_config). Confirms NO experimental-features config is
        needed for `nix flake check`. `git ls-remote --tags` → latest major is v31 (v31.11.0 newest patch)."
  critical: "pin `@v31` (the current major; repo convention is major pins like @v4/@v2). Do NOT pin a moving
             `@latest` (not a valid ref). Flakes are enabled by the action — do NOT add
             `experimental-features = nix-command flakes` (redundant)."

# EXTERNAL — the eval-only flag semantics
- url: https://nix.dev/manual/nix/2.22/command-ref/new-cli/nix3-flake-check.html
  why: "the `nix flake check` reference: '--no-build: Do not build checks' + 'verifies the flake can be
        evaluated'. Confirms `--no-build` EVALUATES every output without building → passes with fakeHash."
  critical: "this is the factual basis for the --no-build correction. A building check (no flag) would FAIL
             on the fakeHash cargo vendor fetch."

# REFERENCE — the sibling deploy-key jobs (mirror their webfactory/ssh-agent + banner-comment idioms)
- file: .github/workflows/release.yml
  why: "the `homebrew-tap` job (~line 397-464) and `scoop-bucket` job (~line 483-538) are the EXACT model
        for the asdf-plugin job's deploy-key + banner-comment + Determine-version idioms. The `winget` job
        (the LAST job, ~line 529-end) is where the asdf-plugin job appends after."

# REFERENCE — the sibling PRP (parallel P1.M5.T2.S1) — coordination
- docfile: plan/007_fb356ba503b4/P1M5T2S1/PRP.md
  why: "P1.M5.T2.S1 (Winget) appends a `winget` job after scoop-bucket. This task appends `asdf-plugin`
        after `winget`. Zero overlap: different jobs, different secrets (WINGET_GITHUB_TOKEN vs
        ASDF_PLUGIN_DEPLOY_KEY), different external repos, different auth models (PAT vs deploy key)."
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
.github/workflows/
  ci.yml                 # EDIT: append the `nix-check` job at the END (after build-and-test)
      jobs.fmt
      jobs.build-and-test
  release.yml            # EDIT: append the `asdf-plugin` job at the END (after winget)
      jobs.macos / windows / linux-binary / arch
      jobs.publish       # the dependency: needs:[publish] (the asdf plugin reflects a released version)
      jobs.aur           # P1.M5.T1.S1 — sibling (per-account AUR SSH key)
      jobs.homebrew-tap  # P1.M5.T1.S2 — sibling (per-repo deploy key — the model to mirror)
      jobs.scoop-bucket  # P1.M5.T1.S2 — sibling (per-repo deploy key — the model to mirror)
      jobs.winget        # P1.M5.T2.S1 (landed) — LAST job; append `asdf-plugin` AFTER it
flake.nix                # P1.M1.T2.S2 (Complete) — INPUT (consumed read-only; cargoHash = fakeHash → --no-build)
packaging/asdf/          # P1.M4.T1.S1/S2 (Complete) — INPUT (consumed by the job/publish.sh; never edited)
  publish.sh             # the publisher the job runs (has --dry-run; rejects leading 'v'; sets own git identity)
  bin/{list-all,download,install}
  lib/utils.bash
  README.md, .tool-versions, mise.toml, CHANGELOG.md
packaging/nix/README.md  # P1.M1.T2.S2 (Complete) — the flake user doc (NOT modified)
Cargo.toml               # version = "0.2.8" (the tag v0.2.8 is cut from this; ref_name#v derives it)
```

### Desired Codebase tree with files added/changed

```bash
.github/workflows/ci.yml       # +1 job (`nix-check`) appended after build-and-test
.github/workflows/release.yml  # +1 job (`asdf-plugin`) appended after winget
# (no Rust, no Cargo.toml, no flake.nix, no packaging/, no docs/*)
```

### Known Gotchas of our codebase & Library Quirks

```yaml
# CRITICAL (fakeHash → nix flake check MUST be --no-build): flake.nix ships `cargoHash = pkgs.lib.fakeHash;`
#   (a deliberate P1.M1.T2.S2 placeholder). A BUILDING `nix flake check` (no flag) fetches the cargo vendor
#   tarball and fails with a hash mismatch on EVERY push to main → a RED gate that trains people to ignore
#   CI. `nix flake check --no-build` EVALUATES all outputs (packages, devShells, nixosModules) WITHOUT
#   building, so fakeHash (a valid string) is never verified → eval passes AND still catches real flake
#   breakage (syntax/type errors, missing inputs, a broken packages.default derivation object). This is the
#   LOAD-BEARING correction to the contract's literal "nix flake check". Document it in the job's banner.

# CRITICAL (do NOT edit flake.nix): resolving the cargoHash placeholder is a SEPARATE follow-up (a flake.nix
#   edit, out of scope — contract OUTPUT = only ci.yml + release.yml). Leave fakeHash untouched. The follow-up
#   path (documented in the banner): run `nix build .#qmkonnect`, read the "got: sha256-…" from the failure,
#   paste it into flake.nix, then flip the job to a full `nix flake check` + `nix build .#qmkonnect --no-link`.

# CRITICAL (deploy key BEFORE publish.sh): publish.sh's first repo action is `git clone "$REMOTE"` over SSH.
#   The webfactory/ssh-agent step MUST run BEFORE `./publish.sh` (else the clone fails "Permission denied
#   (publickey)"). The dry-run test uses a `file://` mock remote, so it needs NO key — run it before ssh-agent.

# CRITICAL (asdf auth = per-repo deploy key, NOT AUR, NOT Winget): the target dabstractor/asdf-qmkonnect is
#   an external repo WE OWN. publish.sh pushes over SSH. A per-repo GitHub deploy key (write access) + the
#   ASDF_PLUGIN_DEPLOY_KEY secret + webfactory/ssh-agent@v0.9.0 is the EXACT homebrew-tap/scoop-bucket model.
#   NOT the AUR per-account SSH-key model (that key registers with the AUR account) and NOT the Winget
#   classic-PAT model (that PAT forks an unowned repo). The secret name ASDF_PLUGIN_DEPLOY_KEY is fixed by
#   the contract + publish.sh's header.

# CRITICAL (bare version — publish.sh rejects a leading 'v'): publish.sh has a `case "$VERSION" in v*)` guard
#   that exits with an error if the version starts with 'v'. The Determine version step MUST yield the bare
#   `0.2.8` via `${GITHUB_REF_NAME#v}`. Never pass `${{ github.ref_name }}` (`v0.2.8`).

# CRITICAL (publish.sh sets its OWN git identity): unlike the homebrew-tap/scoop-bucket jobs (where the JOB
#   sets `git config user.email/name`), publish.sh step 5 sets a git identity in its clone if none exists
#   ("qmkonnect-bot@users.noreply.github.com" / "QMKonnect release automation"). Do NOT add a git-config step
#   to the asdf-plugin job — it is redundant.

# GOTCHA (flake.lock is ABSENT): there is no flake.lock in the repo (verified). Each `nix flake check` run
#   re-resolves the inputs (nixpkgs-unstable + flake-utils) and locks them ephemerally (in the runner sandbox).
#   Without a GitHub token this can hit the GitHub API rate limit → intermittent 403s. The job passes
#   `extra_nix_config: access-tokens = github.com=${{ secrets.GITHUB_TOKEN }}` to prevent that. Uses the
#   default GITHUB_TOKEN (no new secret); ci.yml has no top-level permissions block (default read token suffices).

# GOTCHA (flakes are enabled by the action — do NOT add experimental-features): cachix/install-nix-action
#   enables `nix-command` + `flakes` by default (its README confirms this; disable by overriding
#   experimental-features in extra_nix_config). Adding `experimental-features = nix-command flakes` is
#   redundant. The `access-tokens` line is ADDITIVE to the action's defaults (not a replacement).

# GOTCHA (--no-build does NOT fully eval the NixOS module): `nix flake check --no-build` evaluates
#   nixosModules.default only enough to type-check it is a module-shaped value; it does NOT instantiate it
#   against a full NixOS config (that needs a `checks.x86_64-linux.*` output added to the flake — out of
#   scope). It DOES evaluate packages.default + devShells.default as derivation objects. Documented honestly.

# GOTCHA (shellcheck is preinstalled on ubuntu-latest): the asdf scripts already carry
#   `# shellcheck disable=SC1091` for the sourced utils.bash (intentional; NOT a real failure). Verified CLEAN
#   on the dev box (shellcheck 0.11.0). If a future runner lacks shellcheck, fall back to
#   `sudo apt-get install -y shellcheck` or `ludeeus/action-shellcheck@v2`. Keep the direct `shellcheck` call.

# GOTCHA (publish.sh --dry-run needs a bare mock remote): `git clone file:///tmp/asdf-mock.git` of a freshly
#   `git init --bare` repo yields an empty clone (warning, not error). publish.sh then config/add/commit on
#   it. Create the mock with `git init --bare /tmp/asdf-mock.git` in the SAME step (not a separate job).

# GOTCHA (do NOT use asdf-vm/actions/plugin-test): that action installs a real version from GitHub Releases,
#   which needs the just-published release + udev/systemd for the Linux binary (won't work in CI). shellcheck
#   + publish.sh --dry-run is the pragmatic, dependency-free test that actually validates the shipped plugin.

# GOTCHA (append, never insert): append nix-check at the END of ci.yml (after build-and-test) and asdf-plugin
#   at the END of release.yml (after winget). Do NOT reorder or insert between jobs.
```

## Implementation Blueprint

### Data models and structure

No data models. The deliverables are two YAML jobs. The only "data": one secret name
(`ASDF_PLUGIN_DEPLOY_KEY`), the bare version (`GITHUB_REF_NAME#v`), the nix command
(`nix flake check --no-build`), the action pins (`cachix/install-nix-action@v31`,
`webfactory/ssh-agent@v0.9.0`), and the publish.sh invocation (`./publish.sh "$VERSION"`).

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: EDIT .github/workflows/ci.yml — append the `nix-check` job
  - PLACEMENT: at the very END of the file (after `build-and-test`, the current last job), at the same
    2-space indent as `fmt:` / `build-and-test:`. Append-only — do NOT insert between jobs.
  - IMPLEMENT: the verbatim job YAML from "Implementation Patterns → The `nix-check` job" below.
  - STRUCTURE: (1) a `# ─── banner` comment block documenting the --no-build decision + the fakeHash
    follow-up; (2) the job header (`nix-check:` / `name:` / `runs-on: ubuntu-latest`); (3) `actions/checkout@v4`;
    (4) `cachix/install-nix-action@v31` with `extra_nix_config: access-tokens`; (5) the `nix flake check
    --no-build` step.
  - NAMING: job key `nix-check`, `name: nix flake check (eval)`.
  - PRESERVE: `fmt` + `build-and-test` UNCHANGED, plus the top-level `env:` / `concurrency:` / triggers.

Task 2: EDIT .github/workflows/release.yml — append the `asdf-plugin` job
  - PLACEMENT: at the very END of the file (after `winget`, the current last job), at the same 2-space
    indent as `publish:` / `winget:`. Append-only. (If a re-plan shows winget is NOT yet present, append at
    the very END regardless — never insert between jobs.)
  - IMPLEMENT: the verbatim job YAML from "Implementation Patterns → The `asdf-plugin` job" below.
  - STRUCTURE: (1) a `# ─── banner` comment block documenting ASDF_PLUGIN_DEPLOY_KEY (Mode A ride-along) +
    the test-then-publish flow; (2) the job header (`asdf-plugin:` / `name:` / `needs: [publish]` /
    `if: github.event_name == 'push'` / `runs-on: ubuntu-latest`); (3) `actions/checkout@v4`; (4) the
    `Determine version` step; (5) the shellcheck + publish.sh --dry-run TEST steps; (6) the webfactory/
    ssh-agent step; (7) the `./publish.sh "$VERSION"` PUBLISH step.
  - NAMING: job key `asdf-plugin`, `name: Publish to asdf plugin (asdf-qmkonnect)`.
  - PRESERVE: every existing job UNCHANGED. Do NOT reorder.

Task 3: VALIDATE (no edits)
  - YAML parse: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"` and the same
    for release.yml.
  - actionlint (if installed): `actionlint .github/workflows/*.yml`.
  - Local shellcheck (installed on the dev box): `shellcheck packaging/asdf/bin/* packaging/asdf/lib/*.bash
    packaging/asdf/publish.sh` (must be CLEAN).
  - Local publish.sh dry-run (no secret): `git init --bare /tmp/asdf-mock.git && ASDF_QMKONNECT_REMOTE=file:///tmp/asdf-mock.git
    ./packaging/asdf/publish.sh --dry-run 0.2.8` (must succeed, printing the staged tree).
  - grep gates (Validation Level 3): nix-check job key + `--no-build` + install-nix-action@v31 + access-tokens
    + NO `flake.nix` diff; asdf-plugin job key + needs:[publish] + if push + shellcheck + dry-run + webfactory/
    ssh-agent@v0.9.0 + ASDF_PLUGIN_DEPLOY_KEY + `${GITHUB_REF_NAME#v}` + `./publish.sh "$VERSION"`.
  - (DEFERRED) Real nix flake eval on push to main (Nix not installed locally); real asdf publish on a tag
    push with the deploy key set + the plugin repo pre-created — see Validation Level 4.

Task 4: NEVER do these (out of scope / forbidden)
  - DO NOT drop `--no-build` from `nix flake check` (the shipped cargoHash is fakeHash → a building check
      fails on every push). Use `nix flake check --no-build`.
  - DO NOT edit `flake.nix` (resolving cargoHash is a separate, out-of-scope follow-up). Leave fakeHash.
  - DO NOT add `experimental-features = nix-command flakes` to install-nix-action (it is enabled by default;
      redundant). The access-tokens line is ADDITIVE.
  - DO NOT pass a v-prefixed version to publish.sh (it rejects a leading 'v'). Strip the 'v' in steps.ver.
  - DO NOT set a git identity in the asdf-plugin job (publish.sh sets its own — unlike homebrew-tap/scoop-bucket).
  - DO NOT run publish.sh BEFORE webfactory/ssh-agent (the clone uses SSH; the dry-run uses file:// so it
      runs first, key-free).
  - DO NOT use `asdf-vm/actions/plugin-test` (needs a real release install + udev/systemd). Use shellcheck +
      publish.sh --dry-run.
  - DO NOT modify flake.nix / packaging/* / Cargo.toml / docs/* / PRD.md / tasks.json / prd_snapshot.md.
  - DO NOT reorder jobs or insert between them — append both new jobs at the END of their files.
```

### Implementation Patterns & Key Details

```yaml
# ===== The `nix-check` job (VERBATIM — append at the END of ci.yml, after build-and-test) =====

  # ─────────────────────────────────────────────────────────────────────────
  # Nix flake check — validate flake.nix evaluates cleanly on every push to
  # main (PRD §4 F15 "Nix flake" + architecture/external_deps.md §5 "CI runs
  # `nix flake check`"). The flake lives in-repo, so there is NO publish step
  # — users `nix run github:dabstractor/qmkonnect`; this job only validates it.
  #
  # WHY --no-build (load-bearing): flake.nix ships with
  #   `cargoHash = pkgs.lib.fakeHash;` — a deliberate placeholder (P1.M1.T2.S2)
  #   awaiting a one-time `nix build .#qmkonnect` iteration to capture the real
  #   cargo vendor hash. A BUILDING `nix flake check` would FAIL with a hash
  #   mismatch until that placeholder is resolved (a separate flake.nix follow-up
  #   — NOT in scope for this task). `nix flake check --no-build` EVALUATES every
  #   flake output (packages, devShells, nixosModules) WITHOUT instantiating the
  #   build, so it passes today AND still catches real flake breakage (syntax /
  #   type errors, missing inputs, a malformed packages.default / devShell).
  #
  #   FOLLOW-UP (out of scope): once cargoHash is resolved, run the real
  #   `nix build .#qmkonnect`, paste the "got: sha256-…" into flake.nix, then
  #   replace --no-build with a full `nix flake check` + `nix build .#qmkonnect
  #   --no-link`. (Full nixosModules eval would also need a `checks.*` output
  #   added to the flake.)
  #
  # install-nix-action enables `nix-command` + `flakes` by default (its README
  # confirms this), so no experimental-features config is needed. The
  # access-tokens line lifts the GitHub API rate limit when nix re-resolves the
  # flake inputs (flake.lock is absent, so each run re-resolves nixpkgs +
  # flake-utils); it uses the default GITHUB_TOKEN — no new secret.
  # ─────────────────────────────────────────────────────────────────────────
  nix-check:
    name: nix flake check (eval)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      # Flakes + nix-command are enabled by the action by default. access-tokens
      # (additive, not a replacement) prevents GitHub API rate limits when nix
      # locks the absent flake.lock on each run. Uses the default GITHUB_TOKEN.
      - name: Install Nix
        uses: cachix/install-nix-action@v31
        with:
          extra_nix_config: |
            access-tokens = github.com=${{ secrets.GITHUB_TOKEN }}

      # --no-build: evaluate all outputs WITHOUT building. REQUIRED because
      # flake.nix's cargoHash is a deliberate fakeHash placeholder (see banner).
      # Catches flake syntax/type errors + a broken packages.default / devShell.
      - name: nix flake check (--no-build)
        run: nix flake check --no-build


# ===== The `asdf-plugin` job (VERBATIM — append at the END of release.yml, after winget) =====

  # ─────────────────────────────────────────────────────────────────────────
  # asdf plugin — publish packaging/asdf/ to dabstractor/asdf-qmkonnect
  # (the cross-platform version-manager channel, PRD §4 F15; serves BOTH asdf
  # AND mise, which is asdf-compatible).
  #
  # Runs AFTER `publish` (so the plugin's stamped example versions + commit
  # message reference a RELEASED version) and only on real tag pushes (skipped
  # for workflow_dispatch dry-runs). publish.sh downloads NO release asset —
  # asdf resolves versions at runtime via bin/list-all's GitHub Releases API —
  # so the release dependency is only the version string.
  #
  # FLOW: (1) TEST the plugin with NO secrets — shellcheck + publish.sh
  #   --dry-run against a LOCAL file:// mock remote (validates the full
  #   clone→sync→chmod→sed→commit flow, skipping only the push); (2) load the
  #   SSH deploy key; (3) PUBLISH via publish.sh (clones the real repo, syncs
  #   the plugin scripts + metadata, stamps the version, commits, pushes).
  #   publish.sh sets its OWN git identity — this job does NOT set git config.
  #
  # SECRET — ASDF_PLUGIN_DEPLOY_KEY (REQUIRED for the publish step):
  #   The plugin repo is pushed via a GitHub deploy key (SSH, WRITE access).
  #
  #   One-time setup:
  #     1. Generate a dedicated ed25519 key pair (do NOT reuse a personal key):
  #          ssh-keygen -t ed25519 -C "qmkonnect-asdf-ci" -f qmkonnect-asdf
  #     2. Add the PUBLIC half (qmkonnect-asdf.pub) to the plugin repo
  #        dabstractor/asdf-qmkonnect: Settings → Deploy keys → "Add new
  #        deploy key". CHECK "Allow write access" (or the push is denied).
  #     3. Store the PRIVATE half (qmkonnect-asdf, the full PEM incl. the
  #        BEGIN/END lines) as the ASDF_PLUGIN_DEPLOY_KEY Actions secret in
  #        dabstractor/qmkonnect (Settings → Secrets and variables → Actions).
  #     4. Ensure dabstractor/asdf-qmkonnect PRE-EXISTS on GitHub (empty is
  #        fine — create it once at github.com/new; do NOT add a README so the
  #        first push is clean). publish.sh clones it; a missing repo 404s.
  #
  #   Per-REPO GitHub deploy key (public half on the plugin repo) — the SAME
  #   model as HOMEBREW_TAP_DEPLOY_KEY + SCOOP_BUCKET_DEPLOY_KEY (NOT the AUR
  #   per-account SSH-key model, NOT the Winget classic-PAT model). Until this
  #   secret is set, the publish step fails with "Permission denied (publickey)"
  #   — expected on the very first run. The TEST steps run with NO secret.
  # ─────────────────────────────────────────────────────────────────────────
  asdf-plugin:
    name: Publish to asdf plugin (asdf-qmkonnect)
    needs: [publish]
    if: github.event_name == 'push'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      # Build-less: GITHUB_REF_NAME is the tag (v0.2.8); strip the 'v' for the
      # BARE version publish.sh requires (it REJECTS a leading 'v').
      - name: Determine version
        id: ver
        run: echo "version=${GITHUB_REF_NAME#v}" >> "$GITHUB_OUTPUT"

      # ── TEST (no secrets) ─────────────────────────────────────────────
      # shellcheck is preinstalled on ubuntu-latest. The plugin scripts carry
      # `# shellcheck disable=SC1091` for the sourced utils.bash (intentional;
      # not a real failure). FAIL on real issues. Verified clean at author time.
      - name: shellcheck plugin scripts
        run: shellcheck packaging/asdf/bin/* packaging/asdf/lib/*.bash packaging/asdf/publish.sh

      # Dry-run publish.sh against a LOCAL bare mock remote (file://, no SSH, no
      # secret). Exercises the full clone→sync→chmod→sed-stamp→git-add→commit
      # flow and verifies the bin/* exec bit lands as 100755 in the index; only
      # the push is skipped. publish.sh rejects a leading 'v' → bare VERSION.
      # ASDF_QMKONNECT_REMOTE overrides its default git@ remote.
      - name: Test publish.sh (dry-run against a local mock remote)
        env:
          VERSION: ${{ steps.ver.outputs.version }}
        run: |
          set -euo pipefail
          git init --bare /tmp/asdf-mock.git
          ASDF_QMKONNECT_REMOTE=file:///tmp/asdf-mock.git \
            ./packaging/asdf/publish.sh --dry-run "$VERSION"

      # ── PUBLISH ───────────────────────────────────────────────────────
      # Loads the deploy key into ssh-agent + pre-trusts github.com (built-in
      # host-key list). MUST run before publish.sh — its `git clone` uses SSH.
      - name: Configure SSH deploy key for the plugin repo
        uses: webfactory/ssh-agent@v0.9.0
        with:
          ssh-private-key: ${{ secrets.ASDF_PLUGIN_DEPLOY_KEY }}

      # publish.sh clones dabstractor/asdf-qmkonnect over the deploy key, syncs
      # packaging/asdf/{bin,lib,README.md,.tool-versions,mise.toml,CHANGELOG.md},
      # stamps the version into .tool-versions + mise.toml, verifies the bin/*
      # exec bit (100755), commits, and pushes. It sets its OWN git identity —
      # do NOT set git config here.
      - name: Publish to asdf-qmkonnect (publish.sh)
        working-directory: packaging/asdf
        env:
          VERSION: ${{ steps.ver.outputs.version }}
        run: ./publish.sh "$VERSION"
```

```yaml
# PATTERN (tag-only gate — identical to publish + aur + homebrew-tap + scoop-bucket + winget):
needs: [publish]
if: github.event_name == 'push'

# PATTERN (version from the git tag — build-less jobs avoid the cargo toolchain):
echo "version=${GITHUB_REF_NAME#v}" >> "$GITHUB_OUTPUT"

# PATTERN (the Nix eval gate — eval-only, with the fakeHash rationale):
- uses: cachix/install-nix-action@v31
  with:
    extra_nix_config: |
      access-tokens = github.com=${{ secrets.GITHUB_TOKEN }}
- run: nix flake check --no-build     # --no-build REQUIRED (cargoHash = fakeHash placeholder)

# PATTERN (the asdf deploy-key step — identical pin to homebrew-tap/scoop-bucket):
- uses: webfactory/ssh-agent@v0.9.0
  with:
    ssh-private-key: ${{ secrets.ASDF_PLUGIN_DEPLOY_KEY }}

# PATTERN (publish via the existing publisher — working-directory mirrors the aur job):
- working-directory: packaging/asdf
  env: { VERSION: ${{ steps.ver.outputs.version }} }
  run: ./publish.sh "$VERSION"
# (publish.sh sets its own git identity; publish.sh rejects a leading 'v'; the dry-run
#  uses ASDF_QMKONNECT_REMOTE=file://... so it runs key-free before ssh-agent.)
```

### Integration Points

```yaml
GITHUB WORKFLOWS:
  - add to: .github/workflows/ci.yml (append `nix-check` after build-and-test)
  - add to: .github/workflows/release.yml (append `asdf-plugin` after winget)
  - ci.yml nix-check: runs-on: ubuntu-latest (runs on every push to main + workflow_dispatch)
  - release.yml asdf-plugin: needs:[publish]; if: github.event_name == 'push'; runs-on: ubuntu-latest
SECRET (one-time, documented inline — Mode A):
  - ASDF_PLUGIN_DEPLOY_KEY: per-REPO GitHub deploy key (ed25519). Public half on
    dabstractor/asdf-qmkonnect (Settings → Deploy keys, WRITE access); private half = the Actions secret.
    SAME model as HOMEBREW_TAP_DEPLOY_KEY + SCOOP_BUCKET_DEPLOY_KEY. NOT the AUR per-account SSH key, NOT
    the Winget classic PAT.
EXTERNAL DEPENDENCIES (unchanged by this task):
  - cachix/install-nix-action@v31   (installs Nix; flakes+nix-command auto-enabled; ref confirmed via
    git ls-remote — latest major v31, v31.11.0 newest patch).
  - webfactory/ssh-agent@v0.9.0     (loads the deploy key; the SAME pin both homebrew-tap + scoop-bucket use).
  - dabstractor/asdf-qmkonnect      (the publish target — must pre-exist; empty is fine).
CONSUMES:
  - flake.nix (P1.M1.T2.S2, Complete) — READ-ONLY (the nix-check job validates it; cargoHash = fakeHash → --no-build).
  - packaging/asdf/* (P1.M4.T1.S1/S2, Complete) — RUN by the asdf-plugin job's test + publish steps (never edited).
PRODUCES:
  - the `nix-check` job in ci.yml + the `asdf-plugin` job in release.yml + the inline ASDF_PLUGIN_DEPLOY_KEY
    doc (TWO files changed).
PARALLEL / SIBLING (zero conflicts):
  - P1.M5.T1.S1 (aur) + P1.M5.T1.S2 (homebrew-tap + scoop-bucket) + P1.M5.T2.S1 (winget): independent siblings;
    append `asdf-plugin` after `winget`. Each needs:[publish]; auth models differ (aur=per-account SSH key,
    homebrew/scoop/asdf=per-repo deploy keys, winget=classic PAT) but the jobs are independent.
PLATFORM VALIDATION:
  - Linux dev box: YAML parse + actionlint + grep gates + LOCAL shellcheck + LOCAL publish.sh dry-run (no
    secret). Nix eval is deferred to CI (Nix not installed locally).
  - Real asdf publish: deferred to CI on a tag push with ASDF_PLUGIN_DEPLOY_KEY set + the plugin repo
    pre-created (Validation Level 4).
```

## Validation Loop

> The implementing agent runs on a **Linux dev box** with NO `ASDF_PLUGIN_DEPLOY_KEY` deploy key and NO Nix
> install. The local gates prove both files are well-formed, both jobs structurally correct, the asdf test
> steps actually pass (shellcheck + publish.sh dry-run run locally), and the load-bearing `--no-build`
> correction is applied. The real Nix eval + the real asdf publish are deferred to CI.

### Level 1: YAML well-formedness (runs on Linux)
```bash
cd /home/dustin/projects/qmkonnect
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml')); print('ci.yml: valid YAML')"
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml')); print('release.yml: valid YAML')"
# Expected: both "valid YAML". If either errors → fix indentation (the new jobs must align at the same
#   2-space column as their siblings — fmt:/build-and-test: in ci.yml, publish:/winget: in release.yml).
actionlint .github/workflows/ci.yml .github/workflows/release.yml 2>/dev/null \
  || echo "(actionlint not installed — YAML parse is the gate)"
# Expected: clean (or "not installed"). Address any actionlint errors (e.g. an unknown job in needs, a
#   malformed `uses:` reference, an invalid `if:`).
git diff --stat    # Expected: ONLY .github/workflows/ci.yml + .github/workflows/release.yml (2 files). Nothing else.
git diff --name-only | grep -E 'flake\.nix|Cargo\.toml|packaging/|docs/' && echo "FAIL: out-of-scope file edited" || echo "OK: only the two workflows touched"
```

### Level 2: local sanity — the action references resolve + the test steps actually pass (runs on Linux)
```bash
cd /home/dustin/projects/qmkonnect
# Confirm the install-nix-action major resolves (so `uses:` will resolve in CI):
git ls-remote --tags https://github.com/cachix/install-nix-action.git | grep -E 'refs/tags/v31$' \
  && echo "OK: cachix/install-nix-action@v31 resolves (major)"
# Expected: "refs/tags/v31" (or a v31.x tag) + "OK: … @v31 resolves".
# Confirm webfactory/ssh-agent@v0.9.0 (the pin both sibling jobs use):
git ls-remote --tags https://github.com/webfactory/ssh-agent.git | grep -E 'refs/tags/v0\.9\.0$' \
  && echo "OK: webfactory/ssh-agent@v0.9.0 resolves"
# Confirm the asdf plugin files publish.sh requires are intact (publish.sh fails fast if any is missing):
for f in bin/list-all bin/download bin/install lib/utils.bash README.md .tool-versions mise.toml CHANGELOG.md; do
  test -f "packaging/asdf/$f" || { echo "MISSING: packaging/asdf/$f"; exit 1; }
done && echo "OK: all asdf plugin files present (publish.sh prerequisites)"
# Run the ACTUAL shellcheck the job runs (shellcheck is installed on this dev box — must be CLEAN):
shellcheck packaging/asdf/bin/* packaging/asdf/lib/*.bash packaging/asdf/publish.sh && echo "OK: shellcheck clean"
# Run the ACTUAL publish.sh dry-run the job runs (file:// mock remote — no secret needed):
git init -q --bare /tmp/asdf-mock.git
ASDF_QMKONNECT_REMOTE=file:///tmp/asdf-mock.git ./packaging/asdf/publish.sh --dry-run 0.2.8
echo "OK: publish.sh --dry-run succeeded (clone→sync→chmod→sed→commit flow validated)"
rm -rf /tmp/asdf-mock.git
# Expected: shellcheck clean; publish.sh --dry-run prints "==> Publishing asdf-qmkonnect v0.2.8 (dry-run)"
#   + the staged tree (ls-files -s showing bin/* as 100755) + "==> Dry-run: skipping push."
```

### Level 3: grep invariants — both jobs are structurally correct + the load-bearing corrections applied (runs on Linux)
```bash
cd /home/dustin/projects/qmkonnect
CI=.github/workflows/ci.yml
REL=.github/workflows/release.yml

# ---- ci.yml: nix-check job ----
grep -nE '^\s{2}nix-check:' "$CI"                                 # Expected: 1 (the new job key)
grep -nE 'cachix/install-nix-action@v31' "$CI"                    # Expected: 1 (the action pin). MUST be @v31.
! grep -qE 'cachix/install-nix-action@latest' "$CI" && echo "OK: no @latest (would not resolve)"   # Expected: OK
grep -nE 'nix flake check --no-build' "$CI"                       # Expected: 1 (the eval gate). MUST have --no-build.
! grep -qE 'nix flake check\b' "$CI" | head -1; grep -qE 'nix flake check --no-build' "$CI" && echo "OK: --no-build present (fakeHash safe)"
grep -nE 'access-tokens = github\.com=' "$CI"                     # Expected: 1 (rate-limit guard; uses default GITHUB_TOKEN)
grep -nE 'experimental-features' "$CI" && echo "WARN: redundant experimental-features (action enables flakes by default)" || echo "OK: no redundant experimental-features"
grep -cE '^\s{2}(fmt|build-and-test|nix-check):' "$CI"            # Expected: 3 (2 originals + nix-check)
git diff --name-only -- flake.nix | grep -q . && echo "FAIL: flake.nix was edited (out of scope)" || echo "OK: flake.nix untouched (fakeHash placeholder preserved)"

# ---- release.yml: asdf-plugin job ----
grep -nE '^\s{2}asdf-plugin:' "$REL"                              # Expected: 1 (the new job key)
awk '/^  asdf-plugin:/{j=1} j&&/needs: \[publish\]/{print FILENAME":"NR": "$0; j=0}' "$REL"      # Expected: 1 line
awk '/^  asdf-plugin:/{j=1} j&&/if: github\.event_name == .push./{print FILENAME":"NR": "$0; j=0}' "$REL"  # Expected: 1 line
grep -nE 'webfactory/ssh-agent@v0\.9\.0' "$REL"                   # Expected: 3 (homebrew-tap + scoop-bucket + asdf-plugin)
grep -nE 'ssh-private-key: \$\{\{ secrets\.ASDF_PLUGIN_DEPLOY_KEY \}\}' "$REL"  # Expected: 1 (the deploy-key input)
grep -nE 'ASDF_PLUGIN_DEPLOY_KEY' "$REL"                          # Expected: >=2 (1× ssh-private-key + >=1× comment doc block)
grep -nE 'shellcheck packaging/asdf/' "$REL"                      # Expected: 1 (the TEST step)
grep -nE 'ASDF_QMKONNECT_REMOTE=file:///tmp/asdf-mock\.git' "$REL"# Expected: 1 (the dry-run mock remote)
grep -nE 'publish\.sh --dry-run' "$REL"                           # Expected: 1 (the dry-run test invocation)
awk '/^  asdf-plugin:/{j=1} /^  [a-z]/{if($0 !~ /^  asdf-plugin:/)j=0} j&&/working-directory: packaging\/asdf/{print FILENAME":"NR": "$0}' "$REL"  # Expected: 1 (the publish step's cwd)
grep -nE '\./publish\.sh "\$VERSION"' "$REL"                      # Expected: 1 (the real publish invocation)
grep -nE '\$\{GITHUB_REF_NAME#v\}' "$REL"                         # Expected: >=4 (aur? no — homebrew-tap/scoop-bucket/winget/asdf-plugin bare-version steps)
# Confirm NO existing job was disturbed + the new jobs are appended:
grep -cE '^\s{2}(macos|windows|linux-binary|arch|publish|aur|homebrew-tap|scoop-bucket|winget|asdf-plugin):' "$REL"  # Expected: 10 (9 originals incl. winget + asdf-plugin)
grep -cE '^\s{2}(macos|windows|linux-binary|arch|publish|aur|homebrew-tap|scoop-bucket|winget):' "$REL"            # Expected: 9 (unchanged originals)
# Confirm the asdf-plugin job does NOT set git config (publish.sh sets its own):
awk '/^  asdf-plugin:/{j=1} /^  [a-z]/{if($0 !~ /^  asdf-plugin:/)j=0} j&&/git config user/{print FILENAME":"NR": "$0}' "$REL"  # Expected: NOTHING
# Confirm the deploy-key step comes AFTER the dry-run test and BEFORE ./publish.sh:
awk '/^  asdf-plugin:/{j=1} j{print NR": "$0}' "$REL" | awk '/dry-run/{d=NR} /ssh-agent@v0\.9\.0/{s=NR} /publish\.sh "\$VERSION"/{p=NR} END{print "dry-run line:"d" ssh-agent line:"s" publish line:"p}'
# Expected: dry-run line < ssh-agent line < publish line (test → key → publish order)
```

### Level 4: Real CI execution (DEFERRED — needs the deploy key / a push to main)
```bash
# NOT run from the dev box (no deploy key; Nix not installed). Two deferred gates:
#
# (A) nix-check — on the next push to main (or workflow_dispatch of ci.yml):
#     1. The job installs Nix (cachix/install-nix-action@v31), locks the flake inputs (access-tokens
#        prevents rate limits), and runs `nix flake check --no-build`.
#     2. Verify: the step prints the eval output and exits 0 (green). With fakeHash + --no-build it
#        EVALUATES packages.default + devShells.default + (type-checks) nixosModules.default WITHOUT
#        building, so it passes. A RED here means a REAL flake regression (syntax/type error, a broken
#        derivation object) — investigate, do NOT just drop --no-build.
#
# (B) asdf-plugin — on a real tag push with ASDF_PLUGIN_DEPLOY_KEY set + dabstractor/asdf-qmkonnect
#     pre-created:
#     1. git tag v0.2.9 && git push origin v0.2.9  -> triggers the full pipeline.
#     2. After macos/windows/linux-binary/arch + publish go green, `asdf-plugin` runs:
#        - Determine version → 0.2.9.
#        - shellcheck → clean.
#        - publish.sh --dry-run → prints the staged tree (bin/* as 100755).
#        - webfactory/ssh-agent loads the key.
#        - publish.sh → "==> Publishing asdf-qmkonnect v0.2.9", clone, sync, stamp, commit, push to main.
#     3. Verify post-publish: https://github.com/dabstractor/asdf-qmkonnect shows the new commit
#        ("asdf-qmkonnect v0.2.9"); `asdf plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect
#        && asdf install qmkonnect 0.2.9` works on a Linux box (resolves 0.2.9 via the Releases API).
```

## Final Validation Checklist

### Technical Validation
- [ ] Level 1 (YAML parse) + actionlint clean; `git diff --stat` shows ONLY ci.yml + release.yml; NO
      flake.nix/Cargo.toml/packaging/docs change.
- [ ] Level 2: `git ls-remote` confirms `cachix/install-nix-action@v31` + `webfactory/ssh-agent@v0.9.0`
      resolve; all asdf plugin files present; LOCAL `shellcheck` clean; LOCAL `publish.sh --dry-run` succeeds.
- [ ] Level 3: all grep invariants pass (nix-check job key + `--no-build` + @v31 + access-tokens + no
      redundant experimental-features + flake.nix untouched; asdf-plugin job key + needs:[publish]+if push +
      shellcheck + dry-run + webfactory/ssh-agent@v0.9.0 + ASDF_PLUGIN_DEPLOY_KEY + bare version +
      `./publish.sh "$VERSION"` + no git-config step + test→key→publish order; 3 ci.yml jobs / 10 release.yml
      jobs).
- [ ] Level 4 (deferred to CI): nix-check passes green on push to main; asdf-plugin publishes on a tag push
      with the deploy key set + the plugin repo pre-created.

### Feature Validation
- [ ] All success criteria from "What" met.
- [ ] nix-check appended after build-and-test (ci.yml); asdf-plugin appended after winget (release.yml);
      no existing job reordered/changed.
- [ ] The `--no-build` load-bearing correction applied + documented in the nix-check banner (fakeHash rationale
      + the follow-up path).
- [ ] The asdf-plugin job runs BOTH test steps (shellcheck + dry-run) AND the publish step; deploy key loaded
      before publish.sh.
- [ ] The inline ASDF_PLUGIN_DEPLOY_KEY comment block documents the per-repo deploy-key setup (ed25519, write
      access, public-on-plugin-repo, private-as-secret, repo-must-pre-exist) + the Homebrew/Scoop model note.
- [ ] (Deferred) `nix run github:dabstractor/qmkonnect` consumes the validated flake; `asdf install qmkonnect
      <ver>` resolves the just-published version.

### Code Quality Validation
- [ ] Mirrors the sibling jobs' idioms (banner comment, tag-only gate, ref_name#v version step,
      webfactory/ssh-agent@v0.9.0 deploy-key step).
- [ ] The load-bearing `--no-build` correction applied + the asdf deploy-key model correctly identified as
      Homebrew/Scoop (NOT AUR/Winget).
- [ ] Anti-patterns avoided (see below).

### Documentation & Deployment
- [ ] The nix-check banner is self-contained (a maintainer understands WHY --no-build + the follow-up path).
- [ ] The asdf-plugin banner is self-contained (a maintainer can set up the deploy key + understand the
      pre-exist prerequisite from it alone).
- [ ] No new env vars beyond the documented secret.

---

## Anti-Patterns to Avoid

- ❌ Don't run a building `nix flake check` (no `--no-build`) — the shipped `cargoHash = fakeHash` makes it
      fail on every push. Use `nix flake check --no-build`.
- ❌ Don't edit `flake.nix` to resolve cargoHash — it is a separate, out-of-scope follow-up. Leave fakeHash.
- ❌ Don't add `experimental-features = nix-command flakes` — install-nix-action enables them by default.
      The `access-tokens` line is additive.
- ❌ Don't pass a v-prefixed version to publish.sh — it rejects a leading `v`. Strip it in `steps.ver`.
- ❌ Don't set a git identity in the asdf-plugin job — publish.sh sets its own (unlike homebrew-tap/scoop-bucket).
- ❌ Don't run `./publish.sh` before `webfactory/ssh-agent` — the clone uses SSH. The dry-run uses `file://`,
      so it runs first, key-free.
- ❌ Don't use `asdf-vm/actions/plugin-test` — it needs a real release install + udev/systemd. Use shellcheck
      + `publish.sh --dry-run`.
- ❌ Don't conflate the auth models — asdf = per-repo deploy key (Homebrew/Scoop), NOT the AUR per-account
      SSH key, NOT the Winget classic PAT.
- ❌ Don't modify flake.nix / packaging/* / Cargo.toml / docs/* — they are INPUT (Complete); this task only
      consumes them.
- ❌ Don't reorder jobs — append both new jobs at the END of their files.

---

## Confidence Score

**9/10** for one-pass implementation success. Both deliverables are single, verbatim-specified YAML jobs
appended to one file each; the load-bearing external facts are confirmed this session (install-nix-action
flakes-auto-enabled + @v31; `nix flake check --no-build` eval-only semantics; the shipped fakeHash — read
verbatim from flake.nix; shellcheck clean on all asdf scripts — run locally; publish.sh's `--dry-run` +
leading-`v` guard + git-identity behavior — read from the script). The asdf-plugin job mirrors the idioms of
two already-landed deploy-key sibling jobs (homebrew-tap + scoop-bucket) exactly. The `--no-build` correction
and the asdf deploy-key model are each grep-gateable on the Linux dev box, and BOTH test steps (shellcheck +
publish.sh dry-run) can be run locally with no secret. The two deferred risks (the real Nix eval on push to
main; the real asdf publish on a tag push) are honestly gated to CI and depend on the one-time deploy-key
setup + the plugin repo pre-existing — neither available locally, and neither this task can or should
pre-provision.