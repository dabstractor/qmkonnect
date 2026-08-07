# PRP — P1.M5.T1.S1: Add AUR publication CI job

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`, org `dabstractor`). **ONE file edited:**
> `.github/workflows/release.yml` (+ one new `aur` job). **No new files. No Rust. No packaging/.
> No docs/*.** The secret requirement is documented as an inline comment block in the job (Mode A ride-along).
>
> **What this does:** adds a new `aur` job that runs **after the `publish` job** (so the GitHub
> Release + its `linux-binary` tarball are already live — `publish.sh`'s `makepkg -g` downloads that
> tarball to compute its SHA256), configures an SSH AUR deploy key from the `AUR_SSH_PRIVATE_KEY`
> secret, and invokes `packaging/linux/aur/publish.sh <version>` in an `archlinux:latest` container
> (makepkg is Arch-only). `publish.sh` does all the AUR-side work: patch `pkgver`, refresh
> `sha256sums`, regenerate `.SRCINFO`, clone `aur@aur.archlinux.org:qmkonnect-bin.git`, copy files,
> commit, and push.
>
> **Source of truth:** `architecture/external_deps.md` §1 + "CI Publishing Strategy"
> ("store deploy keys as GitHub Actions secrets; on tag push, git clone → update file → commit → push")
> and `research/codebase_findings.md` (verbatim release.yml idioms + the publish.sh contract).

---

## Goal

**Feature Goal**: After every `v*` tag push, the `qmkonnect-bin` AUR package is automatically
updated to the just-released version — its `PKGBUILD` carries the new `pkgver` + a fresh `sha256sums`
for the release tarball, its `.SRCINFO` is regenerated, and both are pushed to
`aur.archlinux.org/qmkonnect-bin.git`. An Arch user can then `yay -S qmkonnect-bin` (or
`paru -S qmkonnect-bin`) and get the new release with no maintainer intervention.

**Deliverable** (exactly ONE file edited, ONE job added):
- `.github/workflows/release.yml` — a new `aur` job (`needs: [publish]`, `if: github.event_name == 'push'`,
  `container: archlinux:latest`) that installs makepkg deps + openssh, checks out, extracts the
  version, sets up an unprivileged `builder` user with the AUR SSH key + known_hosts + git identity
  + a `~/.ssh/config` Host block, and runs `./packaging/linux/aur/publish.sh <version>` as builder.
  An inline comment block documents the `AUR_SSH_PRIVATE_KEY` secret (how to generate the key pair,
  where to register the public half, how to store the private half).

**Success Definition**:
- `actionlint .github/workflows/release.yml` (if installed) is clean; the YAML parses
  (`python3 -c 'import yaml; yaml.safe_load(...)'`); `git diff --stat` shows ONLY
  `.github/workflows/release.yml`.
- The `aur` job is structurally correct: `needs: [publish]`, `if: github.event_name == 'push'`,
  `container: archlinux:latest`, installs `base-devel cargo git jq openssh`, runs makepkg as a
  non-root `builder` user (mirroring the existing `arch` job), configures the AUR SSH key under the
  builder's `~/.ssh`, sets a git identity (publish.sh doesn't), and invokes `publish.sh <version>`.
- (Deferred) On a real tag push with `AUR_SSH_PRIVATE_KEY` configured, the AUR package updates;
  `yay -Si qmkonnect-bin` shows the new `pkgver`. (The Linux dev box has no AUR deploy key, so this
  gate runs in CI, not locally — see Validation Level 4.)

## User Persona (if applicable)

**Target User**: the Arch Linux end user who installs QMKonnect via an AUR helper (`yay`/`paru`).
Before this task, the AUR package lags each release until a maintainer manually runs `publish.sh`.
After this task, it updates automatically within minutes of the tag push.

**Use Case**: maintainer cuts `v0.2.9`; CI builds all platforms, publishes the GitHub Release, then
this `aur` job runs `publish.sh 0.2.9` → the AUR `qmkonnect-bin` is now at 0.2.9. The Arch user runs
`yay -Syu qmkonnect-bin` and gets 0.2.9.

**Pain Points Addressed**: closes the "manual AUR step" gap in the F15 community-distribution pipeline
(PRD §4 F15 — "publish every release to AUR, Homebrew, Scoop, Winget, Nix, mise/asdf"). This is the
AUR instance; Homebrew/Scoop are sibling jobs (P1.M5.T1.S2).

## Why

- **F15 (PRD §4) requires automated AUR publishing.** `architecture/external_deps.md` §1 mandates an
  AUR `-bin` channel and its "CI approach: Separate workflow job that pushes PKGBUILD + .SRCINFO to
  AUR on tag." This task IS that job.
- **`publish.sh` already exists and is tested** (P1.M1.T1.S2, marked Complete). The CI job is a thin
  environment-provider — it does NOT reimplement the clone→patch→push; it runs the proven script. This
  minimizes risk and matches the architecture brief's "git clone → update file → commit → push" pattern
  (which publish.sh embodies).
- **The job must run AFTER `publish`.** publish.sh's `makepkg -g` downloads the release tarball
  (`https://github.com/dabstractor/qmkonnect/releases/download/v${pkgver}/qmkonnect-*.tar.gz`) to
  compute its SHA256. If the job ran in parallel with `publish`, the tarball wouldn't be live yet →
  `makepkg -g` would fail. Hence `needs: [publish]`.
- **The AUR is SSH-key-only.** Unlike Homebrew/Scoop (GitHub deploy keys), the AUR authenticates via an
  SSH key registered to the AUR *account* (not a per-repo deploy key). The job loads the private half
  from the `AUR_SSH_PRIVATE_KEY` secret into the builder's `~/.ssh`. This is the documented AUR CI model.

## What

### Approach: one job, archlinux container, run publish.sh as builder

Add a single `aur` job that mirrors the existing `arch` job's makepkg-in-container pattern and invokes
`publish.sh`. The job's responsibilities are purely environmental:

1. **`needs: [publish]`, `if: github.event_name == 'push'`** — release must be live; tag-only (skips
   `workflow_dispatch` dry-runs, where the release isn't published and `makepkg -g` would fail).
2. **`container: archlinux:latest`** — makepkg is Arch-only.
3. **Install deps:** `pacman -Sy --noconfirm --needed base-devel cargo git jq openssh` (cargo+jq for
   version extraction per the repo-wide idiom; openssh for the AUR git-over-SSH — NOT in the base image).
4. **Checkout + chown to builder** (makepkg refuses root).
5. **Configure builder's SSH + git identity** — write the key to `~/.ssh/aur_key` (600, owned by
   builder), a `~/.ssh/config` Host block (`Host aur.archlinux.org; IdentityFile ~/.ssh/aur_key;
   IdentitiesOnly yes`), populate `~/.ssh/known_hosts` via `ssh-keyscan`, and set `git config --global
   user.email/name` (publish.sh commits to the AUR but does NOT set an identity — see Gotchas).
6. **Extract version** (`cargo metadata | jq`, id `ver`) and **run `./packaging/linux/aur/publish.sh
   <version>` as builder.**

publish.sh then does: patch `pkgver` → `makepkg -g` (refresh sha256) → `makepkg --printsrcinfo` →
clone `aur@aur.archlinux.org:qmkonnect-bin.git` → copy PKGBUILD + .SRCINFO + qmkonnect.install →
`git commit -m "qmkonnect-bin v<version>"` → `git push`.

### Success Criteria

- [ ] `.github/workflows/release.yml` has a new `aur` job with `name:`, `needs: [publish]`,
      `if: github.event_name == 'push'`, `runs-on: ubuntu-latest`, `container: archlinux:latest`.
- [ ] The job installs `base-devel cargo git jq openssh` (openssh is the non-obvious one — base image
      lacks it; without it `ssh-keyscan`/`ssh` are absent and the git clone fails).
- [ ] The job creates an unprivileged `builder` user and runs `publish.sh` via `su builder -c`
      (makepkg refuses root — mirrors the existing `arch` job).
- [ ] The job writes `$AUR_SSH_PRIVATE_KEY` to the builder's `~/.ssh/aur_key` (600, builder-owned),
      adds a `~/.ssh/config` Host block for `aur.archlinux.org`, and populates `known_hosts`.
- [ ] The job sets `git config --global user.email/user.name` for builder (publish.sh does NOT).
- [ ] The job passes the version from `steps.ver.outputs.version` to `publish.sh`.
- [ ] An inline comment block documents the `AUR_SSH_PRIVATE_KEY` secret (generate ed25519, register
      public half on the AUR account, store private half as the Actions secret).
- [ ] `git diff --stat` shows ONLY `.github/workflows/release.yml`.

## All Needed Context

### Context Completeness Check

_Pass._ An agent with no prior AUR-CI knowledge can implement this from: the verbatim `aur` job YAML
(in Implementation Patterns); the exact idioms to mirror from the existing `arch` job (quoted in
References); the complete publish.sh contract (its 4 steps + the fact it does NOT set a git identity);
the AUR SSH-key model (from publish.sh's header + external_deps.md §1); and the precise gotchas
(openssh not in the base image; makepkg refuses root; key must live in builder's ~/.ssh; git identity
required). The Linux dev box validates via YAML parse + actionlint + a local mock (no AUR key needed);
the real push is deferred to CI.

### Documentation & References

```yaml
# MUST READ — the file being edited (mirror its idioms verbatim)
- file: .github/workflows/release.yml
  why: "the existing 5 jobs. MIRROR: (a) version extraction 'v=$(cargo metadata --no-deps --format-version 1
        | jq -r '.packages[] | select(.name==\"qmkonnect\") | .version'); echo version=$v >> $GITHUB_OUTPUT';
        (b) the arch job's container+makepkg pattern (pacman -Sy --needed base-devel rust cargo git ...;
        useradd -m -G wheel builder; echo 'builder ALL=(ALL) NOPASSWD: ALL' >> /etc/sudoers; chown -R builder
        $GITHUB_WORKSPACE; su builder -c '...'); (c) the publish job's 'needs:[...] if: github.event_name==push'
        tag-only gate; (d) the inline APPLE_* secret documentation style for AUR_SSH_PRIVATE_KEY."
  pattern: "container: archlinux:latest + pacman install + su builder -c (the arch job is the direct template)"
  gotcha: "the arch job does NOT install openssh (it never SSHes). The aur job MUST add openssh for ssh-keyscan/ssh.
           Do NOT change any existing job — APPEND the aur job only. Keep permissions: contents: read at top;
           the aur job needs NO extra permissions (it pushes to the AUR, not to this repo)."

# MUST READ — the script the job invokes (the AUR-side logic lives here, not in the workflow)
- file: packaging/linux/aur/publish.sh
  why: "the contract. Steps: (1) sed pkgver; (2) makepkg -g -> refresh sha256 (DOWNLOADS the release tarball,
        so the job MUST needs:[publish]); (3) makepkg --printsrcinfo > .SRCINFO; (4) git clone
        aur@aur.archlinux.org:qmkonnect-bin.git -> cp PKGBUILD .SRCINFO qmkonnect.install -> git commit
        -m 'qmkonnect-bin v<version>' -> git push. The CI job ONLY provides makepkg + the SSH key + git
        identity + the version; publish.sh does the rest."
  pattern: "set -euo pipefail; ./publish.sh <version> (no --dry-run in CI; --dry-run is for local testing)"
  critical: "publish.sh does NOT set git config user.email/user.name (grep-confirmed). The CI MUST configure a
        git identity for builder, or the AUR 'git commit' fails with 'Author identity unknown'. makepkg refuses
        root -> publish.sh MUST run as builder (su builder -c). publish.sh hardcodes the AUR remote
        (aur@aur.archlinux.org:qmkonnect-bin.git) — the CI does NOT clone the AUR itself."

# MUST READ — the architecture decision this implements + the AUR SSH-key model
- docfile: plan/007_fb356ba503b4/architecture/external_deps.md
  why: "§1 (AUR): '-bin package; key files PKGBUILD/.SRCINFO/.install; Publication: git push to
        aur.archlinux.org/qmkonnect-bin.git; AUR git repo needs SSH key for CI publishing; CI approach:
        Separate workflow job that pushes PKGBUILD + .SRCINFO to AUR on tag'. 'CI Publishing Strategy':
        'store deploy keys as GitHub Actions secrets; on tag push (after GitHub Release publish), git clone
        -> update file -> commit -> push'. 'Version Source of Truth': 'All channels must derive version from
        Cargo.toml (via cargo metadata)'."
  section: "1. AUR" + "CI Publishing Strategy" + "Version Source of Truth"
  critical: "the AUR uses SSH-key auth ONLY (no token). The PUBLIC key registers on the AUR ACCOUNT
        (My Account -> SSH Public Key), not on a repo. The PRIVATE key is the AUR_SSH_PRIVATE_KEY secret."

# MUST READ — verbatim release.yml idioms + the publish.sh contract + the container gotchas
- docfile: plan/007_fb356ba503b4/P1M5T1S1/research/codebase_findings.md
  why: "the grep-verified release.yml job inventory; the EXACT makepkg-as-builder pattern to copy from the
        arch job; the CRITICAL fact that publish.sh sets no git identity (CI must); the container gotchas
        (openssh absent from base image; key must live in builder's ~/.ssh; ~.ssh/config Host block for
        key-type-agnostic auth; known_hosts via ssh-keyscan); and the scope boundary (this task adds ONE
        job, edits ONE file; source-repo commit-back is out of scope)."
  section: "all; especially §2 (publish.sh contract), §4 (AUR SSH model), §5 (container gotchas)"

# REFERENCE — the PKGBUILD publish.sh patches (source URL -> the tarball publish.sh -g downloads)
- file: packaging/linux/aur/PKGBUILD
  why: "confirms source=(\"https://github.com/dabstractor/qmkonnect/releases/download/v${pkgver}/qmkonnect-${pkgver}-linux-x86_64.tar.gz\")
        — i.e. makepkg -g downloads the linux-binary job's tarball, NOT the arch .pkg.tar.zst. Hence aur needs
        [publish] (which uploads the linux-binary asset), not [arch]. It's a -bin package (no build() compile),
        so makepkg -g is fast (download + checksum only)."
  pattern: "pkgver placeholder 0.2.8 (publish.sh patches); sha256sums refreshed by makepkg -g"

# EXTERNAL — AUR SSH-key setup for CI (verify before shipping user-facing prose)
- url: https://wiki.archlinux.org/title/Arch_User_Repository#Rules
  why: "AUR submission rules: SSH key auth, the scp-like git remote (aur@aur.archlinux.org:<pkg>.git),
        and that .SRCINFO MUST be present and in sync. Confirms the publish.sh model is correct."
- url: https://docs.github.com/en/actions/security-guides/using-secrets-in-github-actions
  why: "storing the private SSH key as a repository secret (AUR_SSH_PRIVATE_KEY) and writing it to a file
        in a step (printf '%s\\n' \"$SECRET\" > key; chmod 600). Multi-line PEM secrets work as-is."
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
.github/workflows/
  release.yml          # EDIT: append ONE `aur` job after the `publish` job
      jobs.macos       # (mirror its version step + inline APPLE_* secret doc idiom)
      jobs.windows     # (mirror its version step)
      jobs.linux-binary# (stages the tarball publish.sh -g downloads)
      jobs.arch        # <<< DIRECT TEMPLATE: container: archlinux:latest + pacman install + su builder -c makepkg >>>
      jobs.publish     # the dependency: needs:[publish] (release must be live before makepkg -g)
packaging/linux/aur/
  publish.sh           # INPUT (P1.M1.T1.S2, Complete) — the script the aur job invokes (NOT modified)
  PKGBUILD             # INPUT — publish.sh patches pkgver + sha256sums
  .SRCINFO             # INPUT — publish.sh regenerates it
  qmkonnect.install    # INPUT — copied into the AUR repo by publish.sh
Cargo.toml             # version = "0.2.8" (cargo metadata reads this)
```

### Desired Codebase tree with files added/changed

```bash
.github/workflows/release.yml   # +1 job (`aur`) appended after `publish`. NO new files.
# (no Rust, no Cargo.toml, no packaging/ changes, no docs/*)
```

### Known Gotchas of our codebase & Library Quirks

```yaml
# CRITICAL (publish.sh sets NO git identity — CI must): grep-confirmed, packaging/linux/aur/publish.sh has no
#   `git config user.email/user.name`. The asdf publish.sh (P1.M4.T1.S2) DOES set a fallback; the AUR one does
#   NOT. Without it, publish.sh's `git -C "$WORK/aur" commit -m "qmkonnect-bin v<version>"` fails:
#   "Author identity unknown". The aur job MUST run `git config --global user.email/name` for builder BEFORE
#   invoking publish.sh.

# CRITICAL (makepkg refuses root): publish.sh's header states "Do NOT run as root (makepkg refuses root)". The
#   container runs as root by default. The job MUST create an unprivileged `builder` user (useradd -m builder)
#   and run publish.sh via `su builder -c`. Mirror the existing `arch` job's builder setup verbatim.

# CRITICAL (openssh is NOT in the archlinux base image and NOT installed by the arch job): the arch job never
#   SSHes, so it omits openssh. The aur job's `git clone aur@aur.archlinux.org:...` needs ssh + ssh-keyscan
#   (from the `openssh` package). Add `openssh` to the `pacman -Sy --needed` line or the clone hangs/errors.

# CRITICAL (the SSH key must live in BUILDER's ~/.ssh, not root's): publish.sh's git clone runs as builder (via
#   su builder -c). ssh reads ~/.ssh relative to the EFFECTIVE user's $HOME (/home/builder). Write the key to
#   /home/builder/.ssh/aur_key, chown builder, chmod 600; add /home/builder/.ssh/config + known_hosts likewise.
#   A root-level ~/.ssh setup is invisible to the su'd builder.

# CRITICAL (known_hosts must be populated — no interactive host-key prompt in CI): the container has no prior
#   knowledge of aur.archlinux.org's host key. Run `ssh-keyscan aur.archlinux.org >> ~/.ssh/known_hosts` (as
#   builder, or root then chown builder), OR set `StrictHostKeyChecking accept-new` in ~/.ssh/config. Without
#   this, the first git clone hangs on the "Are you sure you want to continue connecting (yes/no)?" prompt.

# GOTCHA (the aur job needs NO extra permissions): it pushes to the AUR (external), not to this repo. The
#   top-level `permissions: contents: read` suffices. Do NOT add `permissions: contents: write` to the aur job
#   (that's only for the publish job, which creates the GitHub Release). Adding it would be an over-grant.

# GOTCHA (aur needs [publish], NOT [arch]): publish.sh's makepkg -g downloads the linux-binary tarball from
#   the GitHub Release (PKGBUILD source URL), not the arch .pkg.tar.zst. The release is live only after the
#   publish job. So `needs: [publish]`. Do NOT add [arch] (the .pkg.tar.zst is a release artifact, unrelated
#   to the AUR PKGBUILD which sources the tarball).

# GOTCHA (tag-only via github.event_name == 'push'): on workflow_dispatch dry-runs, the GitHub Release is NOT
#   published, so makepkg -g's tarball download would 404. The `if: github.event_name == 'push'` gate skips the
#   aur job on workflow_dispatch — same gate the publish job uses. Do NOT drop this gate.

# GOTCHA (version is BARE in publish.sh's arg; the git TAG is v-prefixed): the cargo-metadata version is bare
#   (0.2.8); publish.sh patches pkgver=0.2.8 (no 'v'). Pass steps.ver.outputs.version (bare) to publish.sh.
#   Do NOT strip/add a 'v' — cargo metadata already yields the bare version.

# GOTCHA (pass the version as an ENV var through `su builder -c`): su without --login preserves env vars set
#   via `env:`. Set VERSION via env: and reference "$VERSION" inside the su -c script. (publish.sh also cd's
#   to its own SCRIPT_DIR, so cwd is robust either way.)

# GOTCHA (source-repo commit-back of patched PKGBUILD/.SRCINFO is OUT OF SCOPE): publish.sh warns "Commit those
#   to the qmkonnect SOURCE repo too so it stays in sync with the AUR." This needs a PAT + a push to `main`
#   (not the tag), adding complexity. AUR publishing works WITHOUT it (publish.sh re-patches pkgver from the
#   version arg each run). Leave source-repo sync as a documented follow-up; do NOT add it to this job.

# GOTCHA (the secret is a PRIVATE OpenSSH key; the PUBLIC half is on the AUR account): unlike GitHub deploy
#   keys (public half on the repo), the AUR registers the public key on the USER ACCOUNT (My Account -> SSH
#   Public Key). Document both halves in the inline comment block (Mode A). AUR key auth is the ONLY option.

# GOTCHA (do NOT modify publish.sh): it is INPUT from P1.M1.T1.S2 (Complete). If you find it lacks a feature
#   (e.g. git identity), compensate in the CI JOB (set git config before invoking), not by editing publish.sh.
#   Editing publish.sh is out of scope for this task and would conflict with P1.M1.T1.S2's contract.

# GOTCHA (one file, one job): `git diff --stat` must show ONLY .github/workflows/release.yml. No Cargo.toml,
#   no packaging/, no docs/*. The secret doc is an INLINE COMMENT in the job (Mode A ride-along).
```

## Implementation Blueprint

### Data models and structure

No data models. The deliverable is a YAML job. The only "data" is the secret name
(`AUR_SSH_PRIVATE_KEY`), the AUR remote (`aur@aur.archlinux.org:qmkonnect-bin.git`, hardcoded inside
publish.sh), and the version (from `cargo metadata`).

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: EDIT .github/workflows/release.yml — append the `aur` job after the `publish` job
  - PLACEMENT: AFTER the entire `publish:` job block (the file's last job), at the same 2-space
    indent as `publish:`. Do NOT insert it between existing jobs.
  - IMPLEMENT: the verbatim job YAML from "Implementation Patterns → The `aur` job (verbatim)" below.
  - STRUCTURE (the 6 steps):
      1. `name: Install build dependencies` — `pacman -Sy --noconfirm --needed base-devel cargo git jq openssh`
         (NOTE: openssh is the non-obvious dep the arch job lacks).
      2. `uses: actions/checkout@v4`.
      3. `name: Determine version` (id: ver) — the cargo metadata | jq idiom (bare version, no 'v').
      4. `name: Configure builder, SSH key, and git identity for AUR publish` — env: AUR_SSH_PRIVATE_KEY;
         useradd builder; chown workspace; install builder ~/.ssh + aur_key (600) + config Host block +
         known_hosts (ssh-keyscan); `su builder -c 'git config --global user.email/name'`.
      5. `name: Publish qmkonnect-bin to the AUR` — env: VERSION; working-directory: packaging/linux/aur;
         `su builder -c 'set -euo pipefail; ./publish.sh "$VERSION"'`.
  - NAMING: job key `aur`, `name: Publish to AUR (qmkonnect-bin)`.
  - PRESERVE: every existing job (macos/windows/linux-binary/arch/publish) UNCHANGED. Do NOT reorder.

Task 2: ADD the inline AUR_SSH_PRIVATE_KEY documentation comment block (Mode A ride-along)
  - PLACEMENT: immediately ABOVE the `aur:` job key (a `# ───` banner + a comment block), mirroring how
    the `macos` job documents APPLE_* secrets inline.
  - CONTENT: how to generate the key pair (`ssh-keygen -t ed25519 -C "qmkonnect-aur" -f aur_key`), where
    to register the PUBLIC half (https://aur.archlinux.org -> My Account -> SSH Public Key), and where to
    store the PRIVATE half (repo Settings -> Secrets -> Actions -> AUR_SSH_PRIVATE_KEY). State the AUR is
    SSH-key-only. (Verbatim text in Implementation Patterns.)

Task 3: VALIDATE (no edits)
  - YAML parse: `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/release.yml'))"` (or yq).
  - actionlint (if installed): `actionlint .github/workflows/release.yml`.
  - grep gates (Validation Level 3): needs:[publish], if push, container archlinux, openssh installed,
    su builder, ssh-keyscan, git config, publish.sh invoked, AUR_SSH_PRIVATE_KEY documented.
  - git diff --stat: ONLY .github/workflows/release.yml.
  - (DEFERRED) Real AUR publish on a tag push with the secret set — see Validation Level 4.

Task 4: NEVER do these (out of scope / forbidden)
  - DO NOT modify publish.sh, PKGBUILD, .SRCINFO, or any packaging/ file (INPUT from P1.M1.T1.S2).
  - DO NOT add the Homebrew/Scoop/Winget/Nix/asdf CI jobs (P1.M5.T1.S2 / P1.M5.T2.S1 / P1.M5.T2.S2).
  - DO NOT add source-repo commit-back of the patched PKGBUILD/.SRCINFO (out of scope; needs PAT + main push).
  - DO NOT add `permissions: contents: write` to the aur job (it pushes to the AUR, not this repo).
  - DO NOT drop the `if: github.event_name == 'push'` gate (makepkg -g 404s on workflow_dispatch dry-runs).
  - DO NOT run makepkg/publish.sh as root (makepkg refuses root — use the builder user).
  - DO NOT omit openssh from the pacman install (the arch job omits it because it never SSHes; the aur job must).
  - DO NOT put the SSH key in root's ~/.ssh (it's invisible to the su'd builder — put it in builder's).
  - DO NOT strip/add a 'v' to the version (cargo metadata yields bare 0.2.8; publish.sh wants bare).
  - DO NOT change any Rust/Cargo/docs file or edit PRD.md/tasks.json/prd_snapshot.md.
```

### Implementation Patterns & Key Details

```yaml
# ===== The `aur` job (VERBATIM — author exactly this, appended after the `publish:` job) =====

  # ─────────────────────────────────────────────────────────────────────────
  # AUR — publish qmkonnect-bin to the Arch User Repository.
  #
  # Runs AFTER `publish` (makepkg -g in publish.sh downloads the release
  # tarball, so the GitHub Release + its linux-binary asset must be live) and
  # only on real tag pushes (skipped for workflow_dispatch dry-runs, where the
  # release isn't published and makepkg -g would 404).
  #
  # SECRET — AUR_SSH_PRIVATE_KEY (REQUIRED for this job to do anything):
  #   The AUR authenticates via SSH-key ONLY (no token/password). The job loads
  #   the private half from this secret into the builder's ~/.ssh and publish.sh
  #   uses it to clone + push the AUR git repo.
  #
  #   One-time setup:
  #     1. Generate a dedicated key pair (do NOT reuse a personal key):
  #          ssh-keygen -t ed25519 -C "qmkonnect-aur-ci" -f qmkonnect-aur
  #     2. Register the PUBLIC half (qmkonnect-aur.pub) with the AUR account
  #        that owns qmkonnect-bin: https://aur.archlinux.org -> My Account ->
  #        Account Type -> SSH Public Key. (AUR keys are per-ACCOUNT, not per-repo.)
  #     3. Store the PRIVATE half (qmkonnect-aur, the full PEM including the
  #        BEGIN/END lines) as the AUR_SSH_PRIVATE_KEY Actions secret in
  #        dabstractor/qmkonnect (Settings -> Secrets and variables -> Actions
  #        -> New repository secret). Paste the file contents verbatim.
  #     4. Ensure the qmkonnect-bin AUR git repo exists (it does for a published
  #        package); publish.sh clones it.
  #
  #   Until this secret is set, the job is wired but the publish.sh step will
  #   fail at the `git clone aur@aur.archlinux.org:qmkonnect-bin.git` with
  #   "Permission denied (publickey)" — expected on the very first run.
  # ─────────────────────────────────────────────────────────────────────────
  aur:
    name: Publish to AUR (qmkonnect-bin)
    needs: [publish]
    if: github.event_name == 'push'
    runs-on: ubuntu-latest
    container: archlinux:latest
    steps:
      # Container ships no build toolchain; install everything before checkout.
      # openssh is REQUIRED (the arch job omits it because it never SSHes; this
      # job clones/pushes the AUR over SSH and needs ssh + ssh-keyscan).
      - name: Install build dependencies
        run: |
          pacman -Sy --noconfirm --needed \
            base-devel cargo git jq openssh

      - uses: actions/checkout@v4

      - name: Determine version
        id: ver
        run: |
          v=$(cargo metadata --no-deps --format-version 1 \
              | jq -r '.packages[] | select(.name=="qmkonnect") | .version')
          echo "version=$v" >> "$GITHUB_OUTPUT"

      # makepkg refuses root, so create an unprivileged builder (mirrors the
      # `arch` job). The same builder runs publish.sh and holds the AUR SSH key.
      - name: Configure builder, SSH key, and git identity for AUR publish
        env:
          AUR_SSH_PRIVATE_KEY: ${{ secrets.AUR_SSH_PRIVATE_KEY }}
        run: |
          set -euo pipefail
          useradd -m -G wheel builder
          echo 'builder ALL=(ALL) NOPASSWD: ALL' >> /etc/sudoers
          chown -R builder "$GITHUB_WORKSPACE"

          # The SSH key + known_hosts + an explicit Host block live in BUILDER's
          # ~/.ssh (publish.sh's git clone runs as builder via `su builder -c`).
          SSH_DIR=/home/builder/.ssh
          install -d -m700 -o builder -g builder "$SSH_DIR"

          # Private key (full PEM, multi-line secret pasted verbatim).
          printf '%s\n' "$AUR_SSH_PRIVATE_KEY" > "$SSH_DIR/aur_key"
          chmod 600 "$SSH_DIR/aur_key"
          chown builder "$SSH_DIR/aur_key"

          # Key-type-agnostic identity: pin the key to the AUR host so ssh finds
          # it regardless of filename (ed25519 or rsa). accept-new guards a
          # missed keyscan key type.
          cat > "$SSH_DIR/config" <<'EOF'
          Host aur.archlinux.org
              IdentityFile ~/.ssh/aur_key
              IdentitiesOnly yes
              StrictHostKeyChecking accept-new
          EOF
          chmod 600 "$SSH_DIR/config"
          chown builder "$SSH_DIR/config"

          # Pre-seed the host key so the first git clone is non-interactive.
          ssh-keyscan aur.archlinux.org >> "$SSH_DIR/known_hosts" || true
          chown builder "$SSH_DIR/known_hosts"

          # publish.sh commits to the AUR but sets NO git identity itself; give
          # the builder one or `git commit` fails ("Author identity unknown").
          su builder -c 'git config --global user.email "qmkonnect-bot@users.noreply.github.com"'
          su builder -c 'git config --global user.name  "QMKonnect release automation"'

      # publish.sh does ALL the AUR-side work: patch pkgver, refresh sha256sums
      # (makepkg -g downloads the release tarball), regenerate .SRCINFO, clone
      # the AUR repo, copy PKGBUILD + .SRCINFO + qmkonnect.install, commit, push.
      - name: Publish qmkonnect-bin to the AUR
        working-directory: packaging/linux/aur
        env:
          VERSION: ${{ steps.ver.outputs.version }}
        run: |
          su builder -c 'set -euo pipefail; ./publish.sh "$VERSION"'
```

```yaml
# PATTERN (makepkg-in-container as non-root — copied verbatim from the existing `arch` job):
container: archlinux:latest
- run: pacman -Sy --noconfirm --needed base-devel ... openssh   # +openssh for SSH
- run: useradd -m -G wheel builder
       echo 'builder ALL=(ALL) NOPASSWD: ALL' >> /etc/sudoers
       chown -R builder "$GITHUB_WORKSPACE"
       su builder -c '...'

# PATTERN (version extraction — identical to the macos/linux-binary/arch jobs):
v=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name=="qmkonnect") | .version')
echo "version=$v" >> "$GITHUB_OUTPUT"

# PATTERN (tag-only gate — identical to the publish job):
needs: [publish]
if: github.event_name == 'push'

# PATTERN (inline secret doc — the macos job documents APPLE_* this way; mirror it for AUR_SSH_PRIVATE_KEY):
#   a `# ─── banner` + a SECRET comment block above the job key, stating generate/register/store steps.

# WHY a ~/.ssh/config Host block (not ssh-agent): key-type-agnostic (ed25519 or rsa), no reliance on the
#   default ~/.ssh/id_* filename, no need to export SSH_AUTH_SOCK across the `su builder -c` boundary
#   (ssh-agent env doesn't survive a new login shell). The Host block makes `git clone aur@aur.archlinux.org:...`
#   pick up ~/.ssh/aur_key automatically.

# WHY accept-new (not StrictHostKeyChecking yes): ssh-keyscan fetches the host key for the common case;
#   accept-new is a belt-and-suspenders fallback if keyscan missed the offered key type, so the first
#   connection auto-trusts instead of hanging on an interactive prompt.
```

### Integration Points

```yaml
GITHUB WORKFLOW:
  - add to: .github/workflows/release.yml (append after the `publish:` job)
  - job key: `aur`; needs: [publish]; if: github.event_name == 'push'; container: archlinux:latest
AUR SSH SECRET (AUR_SSH_PRIVATE_KEY):
  - one-time: ssh-keygen -t ed25519 -C "qmkonnect-aur-ci"; register PUBLIC half on the AUR account
    (https://aur.archlinux.org -> My Account -> SSH Public Key); store PRIVATE half as the repo Actions secret.
AUR REPO (external, unchanged):
  - aur.archlinux.org/qmkonnect-bin.git (publish.sh clones + pushes; pre-exists for a published package)
CONSUMES:
  - the GitHub Release's linux-binary tarball (makepkg -g downloads it — hence needs:[publish])
  - packaging/linux/aur/publish.sh (P1.M1.T1.S2, Complete) — invoked verbatim, NOT modified
  - PKGBUILD / .SRCINFO / qmkonnect.install — patched by publish.sh, NOT by this job
PRODUCES:
  - the `aur` job in release.yml + its inline secret doc (ONE file changed)
PARALLEL / SIBLING (zero conflicts):
  - P1.M4.T1.S2 (parallel): packaging/asdf/ — no overlap (this task touches only .github/workflows/release.yml).
  - P1.M5.T1.S2 (downstream sibling): will APPEND homebrew + scoop jobs to the SAME release.yml — coordinate
    so both jobs land cleanly (they append after `publish`; if both append, order them aur/homebrew/scoop).
    No logical dependency between the jobs (each needs:[publish] independently).
PLATFORM VALIDATION:
  - Linux dev box: YAML parse + actionlint + grep gates + a local publish.sh --dry-run mock (no AUR key).
  - Real AUR publish: deferred to CI on a tag push with AUR_SSH_PRIVATE_KEY set (Validation Level 4).
```

## Validation Loop

> The implementing agent runs on a **Linux dev box** with NO AUR deploy key. The local gates prove the
> YAML is well-formed and the job is structurally correct. `publish.sh` itself is already validated by
> P1.M1.T1.S2 (Complete). The real AUR push is deferred to CI.

### Level 1: YAML well-formedness (runs on Linux)
```bash
cd /home/dustin/projects/qmkonnect
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml')); print('release.yml: valid YAML')"
# Expected: "release.yml: valid YAML". If it errors → fix indentation (GH Actions YAML is indent-sensitive;
#   the `aur:` job must align at the same 2-space column as `publish:`).
actionlint .github/workflows/release.yml 2>/dev/null || echo "(actionlint not installed — YAML parse is the gate)"
# Expected: clean (or "not installed"). Address any actionlint errors (e.g. needs on an unknown job,
#   an invalid `if:`, a missing `uses:` version).
git diff --stat    # Expected: ONLY .github/workflows/release.yml (1 file). Nothing else.
```

### Level 2: Local publish.sh dry-run mock (runs on Linux — proves the script path; no AUR key)
```bash
cd /home/dustin/projects/qmkonnect
# publish.sh needs makepkg (Arch only). Skip the makepkg steps on a non-Arch box — just confirm the
# script is intact + the version plumbing is sane:
test -x packaging/linux/aur/publish.sh && echo "publish.sh present + executable"
grep -q 'aur@aur.archlinux.org:qmkonnect-bin.git' packaging/linux/aur/publish.sh && echo "AUR remote wired"
grep -q 'makepkg --printsrcinfo' packaging/linux/aur/publish.sh && echo ".SRCINFO regen step present"
grep -q 'git commit -m "qmkonnect-bin v' packaging/linux/aur/publish.sh && echo "commit step present"
# (If you ARE on Arch and have an AUR key, run: ./packaging/linux/aur/publish.sh --dry-run 0.2.8 — it
#  patches pkgver/sha256/.SRCINFO locally and skips the AUR push. The CI job runs it WITHOUT --dry-run.)
```

### Level 3: grep invariants — the job is structurally correct (runs on Linux)
```bash
cd /home/dustin/projects/qmkonnect
F=.github/workflows/release.yml
grep -nE '^\s+aur:' "$F"                                    # Expected: 1 (the job key)
grep -nE 'needs: \[publish\]' "$F" | tail -1                # Expected: >=1 (the aur job's dependency)
grep -nE 'if: github\.event_name == .push.' "$F" | tail -1  # Expected: >=1 (tag-only gate; publish has one too)
grep -nE 'container: archlinux:latest' "$F"                 # Expected: >=1 (arch + aur jobs)
grep -nE 'pacman -Sy.*openssh' "$F"                         # Expected: 1 in the aur job (CRITICAL — arch job lacks openssh)
grep -nE 'su builder -c' "$F"                               # Expected: >=1 in aur (makepkg refuses root)
grep -nE 'ssh-keyscan aur.archlinux.org' "$F"               # Expected: 1 (non-interactive host-key trust)
grep -nE 'git config --global user' "$F"                    # Expected: 1 (publish.sh doesn't set identity — CI must)
grep -nE 'AUR_SSH_PRIVATE_KEY' "$F"                         # Expected: >=2 (env: + the comment doc block)
grep -nE '\./publish.sh "\$VERSION"' "$F"                   # Expected: 1 (the invocation)
grep -niE 'permissions:' "$F" | tail -3                     # aur job should have NO permissions: line (inherits read)
# Confirm NO existing job was disturbed:
grep -cE '^\s+(macos|windows|linux-binary|arch|publish|aur):' "$F"   # Expected: 6 (the 5 originals + aur)
```

### Level 4: Real AUR publish on a tag push (DEFERRED — CI, needs the secret)
```bash
# NOT run from the dev box (no AUR key). On a real release:
#   1. (once) generate the ed25519 key; register PUBLIC half on the AUR account; store PRIVATE half as
#      AUR_SSH_PRIVATE_KEY in the repo Actions secrets.
#   2. git tag v0.2.9 && git push origin v0.2.9  -> triggers the full pipeline.
#   3. After macos/windows/linux-binary/arch + publish go green, the `aur` job runs publish.sh.
# Verify post-publish:
#   yay -Si qmkonnect-bin | grep -E '^Version'    # Expected: 0.2.9
#   (or) curl -s "https://aur.archlinux.org/rpc/?v=5&type=info&arg=qmkonnect-bin" | jq '.results[0].Version'
# If publish.sh fails at `git clone` with "Permission denied (publickey)" -> the secret is missing/wrong
#   or the public half isn't registered on the AUR account. If makepkg -g 404s -> the release asset isn't
#   live yet (check the publish job succeeded + the linux-binary tarball is attached to the release).
```

## Final Validation Checklist

### Technical Validation
- [ ] Level 1: `release.yml` parses as valid YAML; `actionlint` clean (or "not installed"); `git diff --stat`
      = ONLY `.github/workflows/release.yml`.
- [ ] Level 3 grep gates pass (needs:[publish], if push, container archlinux, openssh installed, su builder,
      ssh-keyscan, git config identity, AUR_SSH_PRIVATE_KEY referenced + documented, publish.sh invoked).

### Feature Validation
- [ ] The `aur` job has `needs: [publish]` (release live before makepkg -g) and `if: github.event_name == 'push'`
      (tag-only; skipped on workflow_dispatch dry-runs).
- [ ] makepkg/publish.sh runs as the non-root `builder` user (makepkg refuses root).
- [ ] The AUR SSH key is in BUILDER's `~/.ssh/aur_key` (600, builder-owned) with a Host config block +
      populated known_hosts (no interactive host-key prompt in CI).
- [ ] A git identity is configured for builder (publish.sh commits to the AUR but sets none).
- [ ] The version passed to publish.sh is the bare cargo-metadata version (no leading 'v').
- [ ] The inline comment block documents AUR_SSH_PRIVATE_KEY (generate ed25519, register public half on the
      AUR account, store private half as the Actions secret).

### Code Quality Validation
- [ ] Mirrors the existing `arch` job's container + makepkg-as-builder idiom (lowest cognitive load).
- [ ] Mirrors the existing version-extraction idiom (cargo metadata | jq).
- [ ] Mirrors the existing inline-secret-documentation idiom (the macos job's APPLE_* comments).
- [ ] The `aur` job has NO `permissions:` escalation (it pushes to the AUR, not this repo).

### Documentation & Deployment
- [ ] The Mode-A secret doc rides WITH the job (inline comment block), per the contract.
- [ ] No Rust/Cargo/packaging/docs changes; no PRD/tasks.json/prd_snapshot edits.

---

## Anti-Patterns to Avoid

- ❌ Don't reimplement the AUR clone/patch/push in the workflow — `publish.sh` (P1.M1.T1.S2) already does it
  and is tested. The job PROVIDES THE ENVIRONMENT (makepkg + SSH key + git identity + version) and invokes it.
- ❌ Don't run makepkg/publish.sh as root — makepkg refuses root; use the `builder` user (mirror the `arch` job).
- ❌ Don't omit `openssh` from the pacman install — the archlinux base image lacks it; the AUR git clone needs ssh.
- ❌ Don't put the SSH key in root's `~/.ssh` — publish.sh's `git clone` runs as builder; put it in builder's.
- ❌ Don't skip configuring a git identity — publish.sh commits to the AUR but sets no `user.email/name`.
- ❌ Don't drop the `if: github.event_name == 'push'` gate — on workflow_dispatch the release isn't live and
  makepkg -g 404s.
- ❌ Don't add `needs: [arch]` — the AUR PKGBUILD sources the linux-binary tarball (uploaded by `publish`), not
  the arch .pkg.tar.zst. `needs: [publish]` is correct.
- ❌ Don't add `permissions: contents: write` to the aur job — it pushes to the AUR (external), not this repo.
- ❌ Don't strip/add a `v` to the version — cargo metadata yields the bare version publish.sh wants.
- ❌ Don't add the source-repo commit-back of the patched PKGBUILD/.SRCINFO (out of scope; needs PAT + main push;
  AUR publishing works without it). Leave it as a documented follow-up.
- ❌ Don't modify publish.sh / PKGBUILD / .SRCINFO (INPUT from P1.M1.T1.S2 — compensate in the job, not the script).
- ❌ Don't add the Homebrew/Scoop/Winget/Nix/asdf jobs (separate work items P1.M5.T1.S2 / P1.M5.T2.S1 / P1.M5.T2.S2).
- ❌ Don't edit PRD.md, any tasks.json, or prd_snapshot.md.

---

## Confidence Score: 9/10

The task is small (one job in one file) and every fact is verified this session: the exact makepkg-in-
container + builder-user pattern to copy from the existing `arch` job; the verbatim version-extraction
idiom; publish.sh's complete contract (4 steps, hardcoded AUR remote, NO git identity); the AUR SSH-key
model (per-account, SSH-only) from external_deps.md §1 + publish.sh's header; and the three non-obvious
gotchas (openssh absent from the base image; key must live in builder's ~/.ssh; git identity required).
The local gates (YAML parse + actionlint + grep) run on any Linux box; the real AUR push is a clean
deferred CI gate. The −1 reserves for: (a) the parallel P1.M5.T1.S2 appending sibling jobs to the same
release.yml — coordinate append order (no logical dependency, just file-level ordering); (b) the AUR
host-key fingerprint could change (the `accept-new` + keyscan belt-and-suspenders handles it).