# PRP — P1.M5.T1.S2: Add Homebrew tap + Scoop bucket publication CI jobs

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`, org `dabstractor`). **ONE file edited:**
> `.github/workflows/release.yml` (+ two new jobs: `homebrew-tap` and `scoop-bucket`). **No new files.
> No Rust. No Cargo.toml. No packaging/. No docs/*.** The two deploy-key secrets are documented as
> inline comment blocks (Mode A ride-along).
>
> **What this does:** adds two jobs that run **after `publish`** (so the GitHub Release + its DMG/exe
> assets are live — both jobs download the asset to compute its SHA256). Each job: checks out the
> source repo, determines the bare version from the git tag, downloads the just-released asset
> (DMG for Homebrew, Inno `.exe` for Scoop), computes its SHA256, runs the existing update script
> against the source checkout (which patches the cask/manifest *in place*), loads a GitHub SSH deploy
> key into `ssh-agent`, clones the external tap/bucket repo, copies the patched file into the clone,
> sets a git identity, commits, and pushes. The update scripts (`update-cask.sh`, `update-manifest.ps1`)
> do the version+hash patching; the jobs own the clone→copy→commit→push.
>
> **Source of truth:** `architecture/external_deps.md` §2 (Homebrew), §3 (Scoop), and §"CI Publishing
> Strategy" ("store deploy keys as GitHub Actions secrets; on tag push (after GitHub Release publish),
> git clone → update file → commit → push") + "Version Source of Truth" (derive from Cargo.toml).
> `packaging/homebrew/update-cask.sh` + `packaging/scoop/update-manifest.ps1` headers document the
> exact CI flow verbatim. `research/codebase_findings.md` holds the verified facts (incl. a CRITICAL
> correction to the SSH-action reference).

---

## Goal

**Feature Goal**: After every `v*` tag push, the two community package-manager channels update
**automatically** with no maintainer intervention:
- the **Homebrew tap** (`dabstractor/homebrew-qmkonnect`) `Casks/qmkonnect.rb` carries the new
  `version` + the real SHA256 of the released DMG; `brew upgrade --cask qmkonnect` gets the new build.
- the **Scoop bucket** (`dabstractor/scoop-qmkonnect`) `bucket/qmkonnect.json` carries the new
  `version` + concrete `url` + SHA256 `hash`; `scoop update qmkonnect` gets the new build.

**Deliverable** (exactly ONE file edited, TWO jobs added):
- `.github/workflows/release.yml` — a `homebrew-tap` job (`needs: [publish]`,
  `if: github.event_name == 'push'`, `runs-on: ubuntu-latest`) and a `scoop-bucket` job (same gate,
  same runner). Each: checkout → version (git tag) → download asset + `sha256sum` → run the update
  script with the precomputed hash → `webfactory/ssh-agent` deploy key → clone external repo →
  `cp` patched file in → git identity → commit → push. Two inline comment blocks document
  `HOMEBREW_TAP_DEPLOY_KEY` and `SCOOP_BUCKET_DEPLOY_KEY` (how to generate the key pair, where to
  register the public half with write access, where to store the private half).

**Success Definition**:
- `release.yml` parses as valid YAML; `actionlint` (if installed) is clean; `git diff --stat` shows
  ONLY `.github/workflows/release.yml`.
- Both jobs have `needs: [publish]` + `if: github.event_name == 'push'` (assets live; tag-only).
- Each job downloads the correct asset, computes its SHA256, and passes it to the update script in
  the script's documented 2-arg / `-Sha256` form (so the script skips its own download → one
  download total).
- Each job uses `webfactory/ssh-agent@v0.9.0` (NOT `webfactory/agents/github-ssh-agent` — that path
  does not exist and would fail CI) with the right secret, clones the right external repo over SSH,
  copies the patched file to the right in-repo path, and pushes with a configured git identity.
- (Deferred) On a real tag push with both secrets configured, `brew upgrade --cask qmkonnect` and
  `scoop update qmkonnect` serve the new version. (No deploy keys on the dev box → real push is a CI
  gate — see Validation Level 4.)

## User Persona (if applicable)

**Target User**: the macOS user who installs via Homebrew (`brew install --cask qmkonnect`) and the
Windows user who installs via Scoop (`scoop install qmkonnect`). Before this task, each release's
cask/manifest lagged until a maintainer manually ran the update script + pushed. After this task,
both update automatically within minutes of the tag push.

**Use Case**: maintainer cuts `v0.2.9`; CI builds all platforms, publishes the GitHub Release, then
the `homebrew-tap` job patches `version 0.2.9` + the DMG's SHA256 into the tap's cask, and the
`scoop-bucket` job patches `version 0.2.9` + the exe's SHA256 into the bucket's manifest. Both users
`upgrade` and get 0.2.9.

**Pain Points Addressed**: closes the "manual Homebrew + Scoop step" gap in the F15
community-distribution pipeline (PRD §4 F15 — "publish every release to AUR, Homebrew, Scoop, Winget,
Nix, mise/asdf"). AUR is the sibling job P1.M5.T1.S1; this task is the Homebrew + Scoop instances.

## Why

- **F15 (PRD §4) requires automated Homebrew + Scoop publishing.** `architecture/external_deps.md`
  §2/§3 + "CI Publishing Strategy" mandate deploy-key-driven, tag-triggered pushes to the external
  tap/bucket repos. This task IS those two jobs.
- **Both update scripts already exist and are tested** (P1.M2.T1.S2, P1.M3.T1.S2 — Complete). The CI
  jobs are thin environment-providers — they do NOT reimplement the patching; they run the proven
  scripts (passing the precomputed SHA256 to skip the script's own download) and own the
  clone→copy→commit→push. Minimal risk; matches the architecture's "git clone → update file →
  commit → push" pattern.
- **Both jobs must run AFTER `publish`.** They download the release DMG/exe from
  `github.com/dabstractor/qmkonnect/releases/download/v<ver>/...`, which exists only after `publish`
  attaches the artifacts to the GitHub Release. Hence `needs: [publish]` + `if: github.event_name == 'push'`
  (workflow_dispatch dry-runs don't publish → the download 404s).
- **GitHub deploy keys are the auth model** (external_deps.md "store deploy keys as GitHub Actions
  secrets"). `webfactory/ssh-agent` is the canonical, battle-tested action for this: it loads the key
  into `ssh-agent` and pre-trusts `github.com` in `known_hosts`, so `git clone git@github.com:…` works
  non-interactively. (The AUR job uses raw `~/.ssh`+`ssh-keyscan` only because it pushes to the
  non-GitHub host `aur.archlinux.org`; for GitHub→GitHub deploy keys, `webfactory/ssh-agent` is cleaner.)

## What

### Approach: two jobs, ubuntu runners, run update scripts + own the push

Both jobs run on `ubuntu-latest`. Each job's responsibilities:

1. **`needs: [publish]`, `if: github.event_name == 'push'`** — release assets live; tag-only.
2. **`actions/checkout@v4`** — the source repo (gives us `packaging/homebrew/update-cask.sh` /
   `packaging/scoop/update-manifest.ps1` + the cask/manifest templates they patch).
3. **Determine version from the git tag** (`${GITHUB_REF_NAME#v}`) — these jobs build NO Rust, so
   the cargo-metadata idiom would needlessly install the toolchain. The tag is `v<ver>`; stripping
   the `v` yields the bare version both scripts require. (cargo-release cuts the tag from Cargo.toml,
   so this is transitively the Cargo.toml source-of-truth.)
4. **Download the released asset + compute SHA256** (`curl -fL` + `sha256sum | awk '{print $1}'`).
5. **Run the update script with the precomputed hash** — `update-cask.sh "<ver>" "<sha>"` (bash) /
   `update-manifest.ps1 -Version "<ver>" -Sha256 "<sha>"` (pwsh). The script skips its own download
   (hash given) and patches the file **in the source checkout** (`packaging/homebrew/Casks/qmkonnect.rb`
   / `packaging/scoop/qmkonnect.json`).
6. **`webfactory/ssh-agent@v0.9.0`** with the deploy-key secret → loads key into `ssh-agent` +
   pre-trusts `github.com`.
7. **Clone the external repo over SSH**, `cp` the patched file to the correct in-repo path, set a git
   identity, `git add` + `commit` + `push`. (The scripts do NOT push; the job owns this.)

### Success Criteria

- [ ] `.github/workflows/release.yml` has new `homebrew-tap` and `scoop-bucket` jobs, each with
      `needs: [publish]`, `if: github.event_name == 'push'`, `runs-on: ubuntu-latest`.
- [ ] Each job derives the version from `${GITHUB_REF_NAME#v}` (bare; no leading `v`), OR via the
      `cargo metadata | jq` idiom (acceptable alternative — note it needs `dtolnay/rust-toolchain`).
- [ ] Each job downloads the correct asset (`QMKonnect-<ver>-macos.dmg` / `QMKonnect-<ver>-windows-x64.exe`)
      and computes SHA256 via `sha256sum | awk '{print $1}'` (lowercase 64-hex, which both scripts validate).
- [ ] `homebrew-tap` runs `./update-cask.sh "$VERSION" "$SHA256"` (working-directory: packaging/homebrew);
      `scoop-bucket` runs `./update-manifest.ps1 -Version "$VERSION" -Sha256 "$SHA256"` (working-directory:
      packaging/scoop, `shell: pwsh`).
- [ ] Each job uses `webfactory/ssh-agent@v0.9.0` with `ssh-private-key: ${{ secrets.HOMEBREW_TAP_DEPLOY_KEY }}`
      (resp. `SCOOP_BUCKET_DEPLOY_KEY`). **NOT** `webfactory/agents/github-ssh-agent` (does not exist).
- [ ] `homebrew-tap` clones `git@github.com:dabstractor/homebrew-qmkonnect.git` and copies
      `packaging/homebrew/Casks/qmkonnect.rb` → `<clone>/Casks/qmkonnect.rb`.
- [ ] `scoop-bucket` clones `git@github.com:dabstractor/scoop-qmkonnect.git` and copies
      `packaging/scoop/qmkonnect.json` → `<clone>/bucket/qmkonnect.json`.
- [ ] Each job sets `git config user.email/user.name` in the clone before committing (the scripts do
      not push; the job owns the commit and must supply an identity).
- [ ] Inline comment blocks document `HOMEBREW_TAP_DEPLOY_KEY` and `SCOOP_BUCKET_DEPLOY_KEY` (generate
      ed25519, add public half to the tap/bucket repo Deploy keys with "Allow write access", store the
      private half as the repo Actions secret).
- [ ] Neither new job adds a `permissions:` escalation (they push to external repos over a deploy key,
      not via the GITHUB_TOKEN → top-level `contents: read` suffices).
- [ ] `git diff --stat` shows ONLY `.github/workflows/release.yml`.

## All Needed Context

### Context Completeness Check

_Pass._ An agent with no prior Homebrew/Scoop-CI knowledge can implement this from: the verbatim YAML
for both jobs (Implementation Patterns); the exact contracts of both update scripts (2-arg / -Sha256
form; patch in source checkout; do NOT push — quoted in References); the external repo layouts (clone
URLs + in-repo file paths — verified table); the deploy-key + ssh-agent model (external_deps.md
§"CI Publishing Strategy" + both READMEs' "CI publishing" sections); the CRITICAL ssh-action
reference correction (`webfactory/ssh-agent`, not the bucket-README's wrong path); and the precise
gotchas (tag-only gate; needs:[publish]; no toolchain needed for version; lowercase SHA256; git
identity required; copy into the right `Casks/` / `bucket/` subdir). The Linux dev box validates via
YAML parse + actionlint + grep gates (no deploy key needed); the real push is a deferred CI gate.

### Documentation & References

```yaml
# MUST READ — the file being edited (mirror its idioms verbatim; APPEND two jobs at the end)
- file: .github/workflows/release.yml
  why: "the existing jobs. MIRROR: (a) the `publish` job's `needs:[...] if: github.event_name == 'push'`
        tag-only gate (my jobs copy it — assets must be live); (b) the top-level `permissions: contents: read`
        (my jobs add NO `permissions:` line — they push over a deploy key, not the GITHUB_TOKEN);
        (c) the inline APPLE_* secret documentation style (Mode A ride-along) for my two deploy-key secrets.
        APPEND both jobs at the very END of the file (after `publish`, or after `aur` if P1.M5.T1.S1
        already landed). Do NOT change any existing job."
  pattern: "needs: [publish]; if: github.event_name == 'push'; runs-on: ubuntu-latest; steps use
            actions/checkout@v4 + inline # ─── banner comment blocks above each job"
  gotcha: "version extraction differs from the build jobs: build jobs use `cargo metadata | jq` because
           they install the toolchain anyway; my jobs build NO Rust, so `${GITHUB_REF_NAME#v}` is cleaner
           (no dtolnay/rust-toolchain step). Both are correct; pick ONE per job and be consistent."

# MUST READ — the Homebrew script the job invokes (patching logic lives here, NOT in the workflow)
- file: packaging/homebrew/update-cask.sh
  why: "the contract. `./update-cask.sh <version> <sha256>` → uses the given SHA256 (skips its own
        download), patches `version` + `sha256` in $SCRIPT_DIR/Casks/qmkonnect.rb (BSD+GNU-sed portable),
        rejects a leading 'v', validates 64-lowercase-hex, best-effort `brew audit` (absent on ubuntu →
        skipped). PURE local update — does NOT push. Its header documents the CI flow verbatim
        (clone tap → run script → cp cask in → commit → push)."
  pattern: "with working-directory: packaging/homebrew → ./update-cask.sh \"\$VERSION\" \"\$SHA256\""
  critical: "the script patches the file IN THE SOURCE CHECKOUT (packaging/homebrew/Casks/qmkonnect.rb).
             The job must then `cp packaging/homebrew/Casks/qmkonnect.rb <tap-clone>/Casks/qmkonnect.rb`.
             Pass BOTH version and sha256 (the 2-arg form) so the script does not re-download the DMG.
             The SHA256 must be lowercase 64-hex (sha256sum yields lowercase; the script validates the same)."

# MUST READ — the Scoop script the job invokes (patching logic lives here, NOT in the workflow)
- file: packaging/scoop/update-manifest.ps1
  why: "the contract. `./update-manifest.ps1 -Version <ver> -Sha256 <sha>` → uses the given hash (skips
        its own download), regex-patches top-level `version` + concrete `architecture.64bit.url` + `.hash`
        in $PSScriptRoot/qmkonnect.json (LEAVING the autoupdate `$version` template), re-parses JSON,
        rejects leading 'v', validates 64-lowercase-hex, best-effort `scoop checkver` (absent on ubuntu →
        skipped). PURE local update — does NOT push. Header documents the CI flow verbatim."
  pattern: "with working-directory: packaging/scoop, shell: pwsh → ./update-manifest.ps1 -Version \"\$VERSION\" -Sha256 \"\$SHA256\""
  critical: "pwsh 7+ is PREINSTALLED on ubuntu-latest → `shell: pwsh` works (the script's author made it
             cross-platform: 'PowerShell 5.1 (Windows) or 7+ (pwsh, cross-platform)'). The script patches
             the file IN THE SOURCE CHECKOUT (packaging/scoop/qmkonnect.json); the job must then
             `cp packaging/scoop/qmkonnect.json <bucket-clone>/bucket/qmkonnect.json` (NOTE the `bucket/`
             subdir in the external repo). Pass -Sha256 to avoid a re-download of the exe."

# MUST READ — the authoritative external-repo layouts + deploy-key model
- docfile: plan/007_fb356ba503b4/architecture/external_deps.md
  why: "§2 (Homebrew): 'Push cask file to homebrew tap repo on tag; key files Casks/qmkonnect.rb; CI
        approach: push cask to homebrew tap repo on tag'. §3 (Scoop): 'Push manifest to bucket repo on
        tag; key files qmkonnect.json; CI approach: push manifest to bucket repo on tag'. §'CI Publishing
        Strategy': 'store deploy keys as GitHub Actions secrets; on tag push (after GitHub Release
        publish), git clone → update file → commit → push'. §'Version Source of Truth': 'derive version
        from Cargo.toml'. §'Hashing': Homebrew sha256 in cask; Scoop hash in manifest."
  section: "2. Homebrew Cask" + "3. Scoop" + "CI Publishing Strategy" + "Version Source of Truth" + "Hashing"
  critical: "GitHub deploy keys are the auth model (public half on the tap/bucket repo with WRITE access;
             private half as the Actions secret). NOT a PAT, NOT a per-account SSH key (that's the AUR model)."

# MUST READ — the tap + bucket repo structures (where the patched file lands in the external repo)
- file: packaging/homebrew/tap-README.md
  why: "authoritative layout of dabstractor/homebrew-qmkonnect: cask lives at `Casks/qmkonnect.rb`.
        'CI publishing (deploy key)' section documents the exact flow: load key into ssh-agent → clone
        git@github.com:dabstractor/homebrew-qmkonnect.git → run update-cask.sh → cp Casks/qmkonnect.rb in
        → commit → push. Public half → tap repo Settings → Deploy keys (Allow write access). Private half
        → HOMEBREW_TAP_DEPLOY_KEY Actions secret."
  pattern: "cp packaging/homebrew/Casks/qmkonnect.rb  <tap-clone>/Casks/qmkonnect.rb"
- file: packaging/scoop/bucket-README.md
  why: "authoritative layout of dabstractor/scoop-qmkonnect: manifest lives at `bucket/qmkonnect.json`.
        'CI publishing (deploy key)' section documents the exact flow and EXPLICITLY names
        `webfactory/agents/github-ssh-agent@v0.9.0` — ⚠️ THAT ACTION PATH IS WRONG (see gotcha). Correct
        repo is `webfactory/ssh-agent` (verified via GitHub API). Everything else in that section is correct."
  pattern: "cp packaging/scoop/qmkonnect.json  <bucket-clone>/bucket/qmkonnect.json  (NOTE the bucket/ subdir)"

# MUST READ — verbatim findings incl. the CRITICAL ssh-action correction + the full job design
- docfile: plan/007_fb356ba503b4/P1M5T1S2/research/codebase_findings.md
  why: "the grep/read-verified facts: (1) the existing release.yml job inventory + why needs:[publish];
        (2) the two update-script contracts (2-arg / -Sha256 form; patch in source checkout; do NOT push;
        lowercase SHA256); (3) the external-repo layout table (clone URLs + in-repo file paths); (4) the
        CRITICAL correction that the ssh-agent action is `webfactory/ssh-agent@v0.9.0`/`v0.10.0`, NOT the
        bucket-README's `webfactory/agents/github-ssh-agent`; (5) why github.ref_name beats cargo metadata
        for build-less jobs; (6) zero-conflict coordination with the parallel aur job."
  section: "all; especially §2 (script contracts), §3 (external layouts), §4 (ssh-action correction)"

# REFERENCE — the cask/manifest templates the scripts patch (confirms stanza shapes + asset URLs)
- file: packaging/homebrew/Casks/qmkonnect.rb
  why: "confirms the `version \"0.2.8\"` + `sha256 :no_check` stanzas update-cask.sh overwrites, and the
        `url …/QMKonnect-#{version}-macos.dmg` (the asset the job downloads to hash)."
- file: packaging/scoop/qmkonnect.json
  why: "confirms the top-level `version`, the concrete `architecture.64bit.url`/`.hash` (64-zero
        placeholder), and the `autoupdate` `$version` template the script leaves untouched. Asset the job
        downloads to hash: `QMKonnect-<ver>-windows-x64.exe`."

# EXTERNAL — the deploy-key ssh-agent action (verify the EXACT reference; getting it wrong fails CI)
- url: https://github.com/marketplace/actions/webfactory-ssh-agent
  why: "the canonical action. Repo = `webfactory/ssh-agent`. Reference `webfactory/ssh-agent@v0.9.0`
        (stable, tens of millions of uses) or `@v0.10.0` (latest — node-24 upgrade; functionally identical).
        Input: `ssh-private-key: ${{ secrets.<KEY> }}`. BEHAVIOUR: loads the key into ssh-agent AND
        pre-trusts `github.com` in ~/.ssh/known_hosts by default → `git clone git@github.com:…` is
        non-interactive. Supports multiple keys (not needed here — one key per job)."
  critical: "the bucket-README's `webfactory/agents/github-ssh-agent@v0.9.0` DOES NOT EXIST. Use
             `webfactory/ssh-agent@v0.9.0`. Verified via api.github.com/repos/webfactory/ssh-agent."
- url: https://docs.github.com/en/authentication/connecting-to-github-with-ssh/managing-deploy-keys
  why: "GitHub deploy keys: per-repo, SSH, optional write access. The public half goes on the TARGET repo
        (homebrew-qmkonnect / scoop-qmkonnect), the private half is the Actions secret in dabstractor/qmkonnect.
        Check 'Allow write access' on the deploy key or the push fails with 'permission denied'."
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
.github/workflows/
  release.yml           # EDIT: append `homebrew-tap` + `scoop-bucket` jobs at the END (after publish/aur)
      jobs.macos        # produces the DMG the homebrew-tap job downloads to hash
      jobs.windows      # produces the Inno exe the scoop-bucket job downloads to hash
      jobs.linux-binary
      jobs.arch
      jobs.publish      # the dependency: needs:[publish] (release assets must be live before the download)
      # (jobs.aur)      # P1.M5.T1.S1, parallel — may or may not be present when this lands (append after it if so)
packaging/homebrew/
  update-cask.sh        # INPUT (P1.M2.T1.S2, Complete) — the script the homebrew-tap job invokes (NOT modified)
  Casks/qmkonnect.rb    # INPUT — update-cask.sh patches version+sha256 here (in the SOURCE checkout)
  tap-README.md         # authoritative layout of the external homebrew-qmkonnect repo
packaging/scoop/
  update-manifest.ps1   # INPUT (P1.M3.T1.S2, Complete) — the script the scoop-bucket job invokes (NOT modified)
  qmkonnect.json        # INPUT — update-manifest.ps1 patches version+url+hash here (in the SOURCE checkout)
  bucket-README.md      # authoritative layout of the external scoop-qmkonnect repo
Cargo.toml              # version = "0.2.8" (the tag v0.2.8 is cut from this; ref_name#v derives it)
```

### Desired Codebase tree with files added/changed

```bash
.github/workflows/release.yml   # +2 jobs (`homebrew-tap`, `scoop-bucket`) appended after publish/aur. NO new files.
# (no Rust, no Cargo.toml, no packaging/ changes, no docs/*)
```

### Known Gotchas of our codebase & Library Quirks

```yaml
# CRITICAL (the ssh-agent action reference in bucket-README.md is WRONG): bucket-README.md cites
#   `webfactory/agents/github-ssh-agent@v0.9.0`. That repo/path does NOT exist — using it fails CI with
#   "unable to resolve action `webfactory/agents/github-ssh-agent`". The correct reference is
#   `webfactory/ssh-agent@v0.9.0` (or @v0.10.0). Verified via api.github.com/repos/webfactory/ssh-agent
#   (latest tag v0.10.0). Do NOT trust the bucket-README's action path; trust the GitHub Marketplace URL.

# CRITICAL (needs:[publish], NOT [macos]/[windows]): the download step fetches the DMG/exe from the
#   GITHUB RELEASE URL (github.com/dabstractor/qmkonnect/releases/download/v<ver>/...), which exists
#   only AFTER `publish` attaches the artifacts. The build jobs upload workflow ARTIFACTS, not release
#   assets. So needs:[publish]. workflow_dispatch dry-runs don't publish → the `if: github.event_name == 'push'`
#   gate skips both jobs there (else the download 404s).

# CRITICAL (the scripts patch the file IN THE SOURCE CHECKOUT, then the JOB copies it into the clone):
#   update-cask.sh patches packaging/homebrew/Casks/qmkonnect.rb; update-manifest.ps1 patches
#   packaging/scoop/qmkonnect.json — both in the dabstractor/qmkonnet checkout. The job then `cp`s the
#   patched file into the EXTERNAL repo clone at the correct path (Casks/qmkonnect.rb for the tap;
#   bucket/qmkonnect.json for the bucket — NOTE the `bucket/` subdir). Do NOT try to run the script
#   against the clone (the script's $SCRIPT_DIR/$PSScriptRoot is the source checkout).

# CRITICAL (the scripts do NOT push; the JOB owns clone→copy→commit→push + git identity): unlike the
#   AUR publish.sh (which does its OWN git commit inside the script), these two update scripts are PURE
#   local file updates. The job must: git clone the external repo → cp → `git config user.email/name` →
#   git add → git commit → git push. Without `git config user.*` the commit fails ("Author identity unknown").

# CRITICAL (pass the LOWERCASE 64-hex SHA256): `sha256sum file | awk '{print $1}'` yields lowercase hex.
#   Both scripts validate `^[0-9a-f]{64}$` (lowercase). Get-FileHash yields UPPERCASE by default — if you
#   ever compute the Scoop hash in pwsh, call `.ToLower()`. In bash on ubuntu, sha256sum already gives
#   lowercase, so pass it through unchanged.

# GOTCHA (version from the git tag, not cargo metadata): these jobs build NO Rust. Installing
#   dtolnay/rust-toolchain@stable purely to read a version is ~30s of waste. On a `v*` tag push,
#   GITHUB_REF_NAME is the tag (v0.2.8); `${GITHUB_REF_NAME#v}` is the bare version (0.2.8) both scripts
#   require (they REJECT a leading 'v'). The trigger `on: push: tags: - 'v*'` guarantees ref_name is a
#   v-tag. cargo-release cuts the tag from Cargo.toml, so this is transitively the Cargo.toml version.

# GOTCHA (pass the precomputed hash to AVOID a double download): both scripts accept the hash as a 2nd
#   arg (update-cask.sh <ver> <sha>) / param (update-manifest.ps1 -Sha256). With it, they SKIP their own
#   download. The job already downloaded+hashed the asset → pass the hash so the script does not download
#   again (one download total, not two).

# GOTCHA (the Scoop bucket's manifest lives in a `bucket/` subdir): the external repo layout is
#   scoop-qmkonnect/bucket/qmkonnect.json (BucketTemplate convention), NOT the repo root. Name the clone
#   dir `bucket-repo` (not `bucket`) to avoid the confusing `bucket/bucket/qmkonnect.json` path. Then
#   `cp packaging/scoop/qmkonnect.json bucket-repo/bucket/qmkonnect.json`.

# GOTCHA (the deploy key needs WRITE access): add the public half to the tap/bucket repo's
#   Settings → Deploy keys and CHECK "Allow write access". Without it, `git push` fails with
#   "ERROR: Permission to dabstractor/<repo>.git denied to deploy key". Document this in the inline block.

# GOTCHA (neither new job needs a `permissions:` escalation): they push to EXTERNAL repos over a deploy
#   key (ssh-agent), NOT via the GITHUB_TOKEN. The top-level `permissions: contents: read` suffices (only
#   `actions/checkout@v4` uses the default token, and read is enough for a public-repo checkout). Do NOT
#   add `permissions: contents: write` (that's only for the publish job, which creates the GitHub Release).

# GOTCHA (pwsh is preinstalled on ubuntu-latest): the Scoop job runs update-manifest.ps1 with
#   `shell: pwsh` on ubuntu-latest — no extra install needed. The script is cross-platform by design.
#   (The contract allows `runs-on: windows-latest` too; ubuntu is cheaper/faster and mirrors the other jobs.)

# GOTCHA (do NOT modify the update scripts or the cask/manifest): they are INPUT from P1.M2.T1.S2 /
#   P1.M3.T1.S2 (Complete). If you find a gap, compensate in the JOB, not by editing packaging/.

# GOTCHA (one file, two jobs): `git diff --stat` must show ONLY .github/workflows/release.yml. No Cargo.toml,
#   no packaging/, no docs/*. The two secret docs are INLINE COMMENT BLOCKS in the jobs (Mode A ride-along).
```

## Implementation Blueprint

### Data models and structure

No data models. The deliverable is two YAML jobs. The only "data": two secret names
(`HOMEBREW_TAP_DEPLOY_KEY`, `SCOOP_BUCKET_DEPLOY_KEY`), two external SSH remotes
(`git@github.com:dabstractor/homebrew-qmkonnect.git`, `git@github.com:dabstractor/scoop-qmkonnect.git`),
two in-clone destination paths (`Casks/qmkonnect.rb`, `bucket/qmkonnect.json`), two asset URLs, and the
version (from `GITHUB_REF_NAME#v`).

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: EDIT .github/workflows/release.yml — append the `homebrew-tap` job
  - PLACEMENT: at the very END of the file (after `publish`, or after `aur` if P1.M5.T1.S1 already
    landed), at the same 2-space indent as `publish:`. Append-only — do NOT insert between jobs.
  - IMPLEMENT: the verbatim job YAML from "Implementation Patterns → The `homebrew-tap` job" below.
  - STRUCTURE (the 7 steps):
      1. `uses: actions/checkout@v4`.
      2. `name: Determine version` (id: ver) — `echo "version=${GITHUB_REF_NAME#v}" >> "$GITHUB_OUTPUT"`.
      3. `name: Download release DMG and compute SHA256` (id: dmg) — curl the DMG, sha256sum, rm, output sha256.
      4. `name: Patch cask (update-cask.sh)` (working-directory: packaging/homebrew) —
         `./update-cask.sh "${{ steps.ver.outputs.version }}" "${{ steps.dmg.outputs.sha256 }}"`.
      5. `uses: webfactory/ssh-agent@v0.9.0` with `ssh-private-key: ${{ secrets.HOMEBREW_TAP_DEPLOY_KEY }}`.
      6. `name: Push patched cask to the tap repo` — git clone homebrew-qmkonnect → cp Casks/qmkonnect.rb →
         git config user.* → git add → git commit -m "qmkonnect cask v<ver>" → git push.
  - NAMING: job key `homebrew-tap`, `name: Publish to Homebrew tap (homebrew-qmkonnect)`.
  - PRESERVE: every existing job UNCHANGED. Do NOT reorder.

Task 2: EDIT .github/workflows/release.yml — append the `scoop-bucket` job (after homebrew-tap)
  - PLACEMENT: immediately after the `homebrew-tap` job block, same indent.
  - IMPLEMENT: the verbatim job YAML from "Implementation Patterns → The `scoop-bucket` job" below.
  - STRUCTURE (the 7 steps) — mirrors Task 1 with Scoop specifics:
      1. `uses: actions/checkout@v4`.
      2. `name: Determine version` (id: ver) — same ref_name#v as Task 1.
      3. `name: Download release installer and compute SHA256` (id: exe) — curl the exe, sha256sum, rm.
      4. `name: Patch manifest (update-manifest.ps1)` (working-directory: packaging/scoop, shell: pwsh) —
         `./update-manifest.ps1 -Version "${{ steps.ver.outputs.version }}" -Sha256 "${{ steps.exe.outputs.sha256 }}"`.
      5. `uses: webfactory/ssh-agent@v0.9.0` with `ssh-private-key: ${{ secrets.SCOOP_BUCKET_DEPLOY_KEY }}`.
      6. `name: Push patched manifest to the bucket repo` — git clone scoop-qmkonnect →
         cp qmkonnect.json → bucket-repo/bucket/qmkonnect.json → git config → commit → push.
  - NAMING: job key `scoop-bucket`, `name: Publish to Scoop bucket (scoop-qmkonnect)`.
  - PRESERVE: every existing job + the new homebrew-tap job UNCHANGED.

Task 3: ADD the two inline deploy-key documentation comment blocks (Mode A ride-along)
  - PLACEMENT: a `# ─── banner` + SECRET comment block immediately ABOVE each job key (mirror the macos
    job's APPLE_* inline doc, and the aur job's AUR_SSH_PRIVATE_KEY doc from P1.M5.T1.S1).
  - CONTENT (each): how to generate the ed25519 key pair
    (`ssh-keygen -t ed25519 -C "qmkonnect-<channel>-ci" -f qmkonnect-<channel>`), where to register the
    PUBLIC half (the tap/bucket repo → Settings → Deploy keys → CHECK "Allow write access"), and where to
    store the PRIVATE half (dabstractor/qmkonnect → Settings → Secrets and variables → Actions). State
    these are per-REPO GitHub deploy keys (not the AUR per-account model). (Verbatim text in Implementation Patterns.)

Task 4: VALIDATE (no edits)
  - YAML parse: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"` (or yq).
  - actionlint (if installed): `actionlint .github/workflows/release.yml`.
  - grep gates (Validation Level 3): two new job keys, both needs:[publish]+if push, webfactory/ssh-agent
    (NOT webfactory/agents), both secrets referenced + documented, both asset URLs, both clone URLs,
    both cp targets, git identity, update scripts invoked.
  - git diff --stat: ONLY .github/workflows/release.yml.
  - (DEFERRED) Real publish on a tag push with both secrets set — see Validation Level 4.

Task 5: NEVER do these (out of scope / forbidden)
  - DO NOT modify update-cask.sh / update-manifest.ps1 / qmkonnect.rb / qmkonnect.json (INPUT, Complete).
  - DO NOT add the Winget/Nix/asdf CI jobs (P1.M5.T2.* — separate work items).
  - DO NOT use `webfactory/agents/github-ssh-agent` (does not exist) — use `webfactory/ssh-agent`.
  - DO NOT add `permissions: contents: write` to either job (they push over a deploy key, not the GITHUB_TOKEN).
  - DO NOT depend on `[macos]`/`[windows]` (they upload workflow artifacts, not release assets) — use `[publish]`.
  - DO NOT drop the `if: github.event_name == 'push'` gate (assets 404 on workflow_dispatch dry-runs).
  - DO NOT run the update script against the external clone (run it against the SOURCE checkout, then cp).
  - DO NOT forget `git config user.email/user.name` in the clone (the scripts do NOT push; the job owns the commit).
  - DO NOT compute the Scoop hash with Get-FileHash without `.ToLower()` (it yields uppercase; use sha256sum in bash instead — already lowercase).
  - DO NOT change any Rust/Cargo/docs file or edit PRD.md/tasks.json/prd_snapshot.md.
```

### Implementation Patterns & Key Details

```yaml
# ===== The `homebrew-tap` job (VERBATIM — author exactly this, appended at the END of release.yml) =====

  # ─────────────────────────────────────────────────────────────────────────
  # Homebrew tap — publish the patched cask to dabstractor/homebrew-qmkonnect.
  #
  # Runs AFTER `publish` (the cask's sha256 is the hash of the release DMG,
  # which exists only once `publish` attaches it to the GitHub Release) and
  # only on real tag pushes (skipped for workflow_dispatch dry-runs, where the
  # release isn't published and the DMG download would 404).
  #
  # SECRET — HOMEBREW_TAP_DEPLOY_KEY (REQUIRED for this job to do anything):
  #   The tap repo is pushed via a GitHub deploy key (SSH, WRITE access).
  #
  #   One-time setup:
  #     1. Generate a dedicated ed25519 key pair (do NOT reuse a personal key):
  #          ssh-keygen -t ed25519 -C "qmkonnect-homebrew-ci" -f qmkonnect-homebrew
  #     2. Add the PUBLIC half (qmkonnect-homebrew.pub) to the tap repo
  #        dabstractor/homebrew-qmkonnect: Settings → Deploy keys → "Add new
  #        deploy key". CHECK "Allow write access" (or the push is denied).
  #     3. Store the PRIVATE half (qmkonnect-homebrew, the full PEM incl. the
  #        BEGIN/END lines) as the HOMEBREW_TAP_DEPLOY_KEY Actions secret in
  #        dabstractor/qmkonnect (Settings → Secrets and variables → Actions).
  #
  #   This is a per-REPO GitHub deploy key (the public half lives on the tap
  #   repo), NOT the AUR per-account SSH-key model. webfactory/ssh-agent loads
  #   the key into ssh-agent and pre-trusts github.com, so the git clone over
  #   SSH is non-interactive. Until this secret is set, the clone step fails
  #   with "Permission denied (publickey)" — expected on the very first run.
  # ─────────────────────────────────────────────────────────────────────────
  homebrew-tap:
    name: Publish to Homebrew tap (homebrew-qmkonnect)
    needs: [publish]
    if: github.event_name == 'push'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      # This job builds NO Rust. The workflow only triggers on `v*` tags, so
      # GITHUB_REF_NAME is the tag (v0.2.8); stripping the leading 'v' yields
      # the bare version (0.2.8) update-cask.sh requires (it rejects a 'v').
      # cargo-release cuts the tag from Cargo.toml, so this is transitively the
      # Cargo.toml version (architecture/external_deps.md "Version Source of Truth").
      - name: Determine version
        id: ver
        run: echo "version=${GITHUB_REF_NAME#v}" >> "$GITHUB_OUTPUT"

      # update-cask.sh CAN download the DMG itself, but the contract wants the
      # job to own the download + hash (single download; visible in CI logs).
      # sha256sum yields lowercase 64-hex, which update-cask.sh validates.
      - name: Download release DMG and compute SHA256
        id: dmg
        env:
          VERSION: ${{ steps.ver.outputs.version }}
        run: |
          url="https://github.com/dabstractor/qmkonnect/releases/download/v${VERSION}/QMKonnect-${VERSION}-macos.dmg"
          tmp="$(mktemp)"
          curl -fL "$url" -o "$tmp"
          echo "sha256=$(sha256sum "$tmp" | awk '{print $1}')" >> "$GITHUB_OUTPUT"
          rm -f "$tmp"

      # Patches version + sha256 in packaging/homebrew/Casks/qmkonnect.rb IN THE
      # SOURCE CHECKOUT. Passing the hash (2-arg form) skips the script's own
      # download. PURE local update — does NOT push (the job does, next).
      - name: Patch cask (update-cask.sh)
        working-directory: packaging/homebrew
        run: ./update-cask.sh "${{ steps.ver.outputs.version }}" "${{ steps.dmg.outputs.sha256 }}"

      # Loads the deploy key into ssh-agent + pre-trusts github.com (built-in
      # host-key list). NOTE: the repo is `webfactory/ssh-agent` — the
      # bucket-README's `webfactory/agents/github-ssh-agent` path DOES NOT EXIST.
      - name: Configure SSH deploy key for the tap repo
        uses: webfactory/ssh-agent@v0.9.0
        with:
          ssh-private-key: ${{ secrets.HOMEBREW_TAP_DEPLOY_KEY }}

      # update-cask.sh patched the SOURCE-checkout cask; copy it into the tap
      # clone, set a git identity (the script doesn't push — the job owns the
      # commit), commit, and push over the deploy key.
      - name: Push patched cask to the tap repo
        env:
          VERSION: ${{ steps.ver.outputs.version }}
        run: |
          set -euo pipefail
          git clone git@github.com:dabstractor/homebrew-qmkonnect.git tap-repo
          cp packaging/homebrew/Casks/qmkonnect.rb tap-repo/Casks/qmkonnect.rb
          cd tap-repo
          git config user.email "qmkonnect-bot@users.noreply.github.com"
          git config user.name  "QMKonnect release automation"
          git add Casks/qmkonnect.rb
          git commit -m "qmkonnect cask v${VERSION}"
          git push
```

```yaml
# ===== The `scoop-bucket` job (VERBATIM — author exactly this, appended after homebrew-tap) =====

  # ─────────────────────────────────────────────────────────────────────────
  # Scoop bucket — publish the patched manifest to dabstractor/scoop-qmkonnect.
  #
  # Runs AFTER `publish` (the manifest's hash is the SHA256 of the release
  # Inno installer, which exists only once `publish` attaches it) and only on
  # real tag pushes (skipped for workflow_dispatch dry-runs).
  #
  # SECRET — SCOOP_BUCKET_DEPLOY_KEY (REQUIRED for this job to do anything):
  #   The bucket repo is pushed via a GitHub deploy key (SSH, WRITE access).
  #
  #   One-time setup:
  #     1. Generate a dedicated ed25519 key pair:
  #          ssh-keygen -t ed25519 -C "qmkonnect-scoop-ci" -f qmkonnect-scoop
  #     2. Add the PUBLIC half (qmkonnect-scoop.pub) to the bucket repo
  #        dabstractor/scoop-qmkonnect: Settings → Deploy keys → "Add new
  #        deploy key". CHECK "Allow write access" (or the push is denied).
  #     3. Store the PRIVATE half (qmkonnect-scoop, full PEM) as the
  #        SCOOP_BUCKET_DEPLOY_KEY Actions secret in dabstractor/qmkonnect.
  #
  #   Per-REPO GitHub deploy key (public half on the bucket repo). Same model
  #   as the Homebrew tap key above. Until set, the clone step fails with
  #   "Permission denied (publickey)" — expected on the very first run.
  # ─────────────────────────────────────────────────────────────────────────
  scoop-bucket:
    name: Publish to Scoop bucket (scoop-qmkonnect)
    needs: [publish]
    if: github.event_name == 'push'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Determine version
        id: ver
        run: echo "version=${GITHUB_REF_NAME#v}" >> "$GITHUB_OUTPUT"

      # Download the Inno installer + hash it (sha256sum → lowercase 64-hex,
      # which update-manifest.ps1 validates via ^[0-9a-f]{64}$).
      - name: Download release installer and compute SHA256
        id: exe
        env:
          VERSION: ${{ steps.ver.outputs.version }}
        run: |
          url="https://github.com/dabstractor/qmkonnect/releases/download/v${VERSION}/QMKonnect-${VERSION}-windows-x64.exe"
          tmp="$(mktemp)"
          curl -fL "$url" -o "$tmp"
          echo "sha256=$(sha256sum "$tmp" | awk '{print $1}')" >> "$GITHUB_OUTPUT"
          rm -f "$tmp"

      # Patches version + concrete url + hash in packaging/scoop/qmkonnect.json
      # IN THE SOURCE CHECKOUT (leaves the autoupdate $version template). Passing
      # -Sha256 skips the script's own download. pwsh 7+ is preinstalled on
      # ubuntu-latest; the script is cross-platform by design. PURE local update.
      - name: Patch manifest (update-manifest.ps1)
        working-directory: packaging/scoop
        shell: pwsh
        run: ./update-manifest.ps1 -Version "${{ steps.ver.outputs.version }}" -Sha256 "${{ steps.exe.outputs.sha256 }}"

      - name: Configure SSH deploy key for the bucket repo
        uses: webfactory/ssh-agent@v0.9.0
        with:
          ssh-private-key: ${{ secrets.SCOOP_BUCKET_DEPLOY_KEY }}

      # Copy the patched manifest into the bucket clone. NOTE the external repo
      # layout: the manifest lives in a `bucket/` subdir (BucketTemplate
      # convention), so the destination is bucket-repo/bucket/qmkonnect.json
      # (clone dir named `bucket-repo` to avoid the confusing bucket/bucket path).
      - name: Push patched manifest to the bucket repo
        env:
          VERSION: ${{ steps.ver.outputs.version }}
        run: |
          set -euo pipefail
          git clone git@github.com:dabstractor/scoop-qmkonnect.git bucket-repo
          cp packaging/scoop/qmkonnect.json bucket-repo/bucket/qmkonnect.json
          cd bucket-repo
          git config user.email "qmkonnect-bot@users.noreply.github.com"
          git config user.name  "QMKonnect release automation"
          git add bucket/qmkonnect.json
          git commit -m "qmkonnect manifest v${VERSION}"
          git push
```

```yaml
# PATTERN (tag-only gate — identical to the publish + aur jobs):
needs: [publish]
if: github.event_name == 'push'

# PATTERN (version from the git tag — build-less jobs avoid the cargo toolchain):
echo "version=${GITHUB_REF_NAME#v}" >> "$GITHUB_OUTPUT"
# (consistency alternative: v=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name=="qmkonnect") | .version')
#  — but that needs a dtolnay/rust-toolchain@stable step BEFORE it; ref_name does not.)

# PATTERN (download + hash the release asset — one download, shared by both jobs):
tmp="$(mktemp)"; curl -fL "$url" -o "$tmp"; echo "sha256=$(sha256sum "$tmp" | awk '{print $1}')" >> "$GITHUB_OUTPUT"; rm -f "$tmp"

# PATTERN (GitHub deploy key via ssh-agent — canonical for GitHub→GitHub pushes):
- uses: webfactory/ssh-agent@v0.9.0
  with: { ssh-private-key: ${{ secrets.<KEY> }} }
# webfactory/ssh-agent pre-trusts github.com in known_hosts → the subsequent `git clone git@github.com:…`
# is non-interactive. The aur job uses raw ~/.ssh + ssh-keyscan only because it pushes to aur.archlinux.org.

# PATTERN (clone → cp patched file in → identity → commit → push — the job owns the push):
git clone git@github.com:dabstractor/<repo>.git <repo>-repo
cp packaging/<channel>/<file> <repo>-repo/<in-repo-path>
cd <repo>-repo
git config user.email "qmkonnect-bot@users.noreply.github.com"
git config user.name  "QMKonnect release automation"
git add <in-repo-path>
git commit -m "qmkonnect <cask|manifest> v${VERSION}"
git push

# WHY pass the hash to the script (not let it download): the job already downloaded+hashed the asset
#   for visibility/determinism; passing it (update-cask.sh <ver> <sha> / update-manifest.ps1 -Sha256)
#   makes the script SKIP its own download → exactly one download total.
```

### Integration Points

```yaml
GITHUB WORKFLOW:
  - add to: .github/workflows/release.yml (append both jobs at the END, after publish/aur)
  - job keys: `homebrew-tap` + `scoop-bucket`; each needs: [publish]; if: github.event_name == 'push';
    runs-on: ubuntu-latest
DEPLOY-KEY SECRETS (one-time, documented inline — Mode A):
  - HOMEBREW_TAP_DEPLOY_KEY: ssh-keygen ed25519; PUBLIC half → dabstractor/homebrew-qmkonnect Deploy
    keys (Allow write access); PRIVATE half → Actions secret in dabstractor/qmkonnect.
  - SCOOP_BUCKET_DEPLOY_KEY: same, public half on dabstractor/scoop-qmkonnect.
EXTERNAL REPOS (unchanged by this task; pre-exist for published channels):
  - git@github.com:dabstractor/homebrew-qmkonnect.git  (file: Casks/qmkonnect.rb)
  - git@github.com:dabstractor/scoop-qmkonnect.git     (file: bucket/qmkonnect.json)
CONSUMES:
  - the GitHub Release's DMG asset (homebrew-tap) + Inno-exe asset (scoop-bucket) — hence needs:[publish]
  - packaging/homebrew/update-cask.sh (P1.M2.T1.S2, Complete) — invoked verbatim, NOT modified
  - packaging/scoop/update-manifest.ps1 (P1.M3.T1.S2, Complete) — invoked verbatim, NOT modified
  - packaging/homebrew/Casks/qmkonnect.rb + packaging/scoop/qmkonnect.json — patched by the scripts, NOT the job
PRODUCES:
  - the `homebrew-tap` + `scoop-bucket` jobs in release.yml + their inline secret docs (ONE file changed)
PARALLEL / SIBLING (zero conflicts):
  - P1.M5.T1.S1 (aur job, parallel/in-progress): may append an `aur` job to the same release.yml — append
    my two jobs AFTER it (or after `publish` if aur isn't there yet). Independent siblings; append order
    is functionally irrelevant (each needs:[publish]).
  - P1.M5.T2.S1/S2 (Winget/Nix/asdf, downstream): separate jobs; no overlap with this task.
PLATFORM VALIDATION:
  - Linux dev box: YAML parse + actionlint + grep gates (no deploy keys needed locally).
  - Real tap/bucket publish: deferred to CI on a tag push with both secrets set (Validation Level 4).
```

## Validation Loop

> The implementing agent runs on a **Linux dev box** with NO deploy keys. The local gates prove the YAML
> is well-formed and both jobs are structurally correct. Both update scripts are already validated by
> P1.M2.T1.S2 / P1.M3.T1.S2 (Complete). The real tap/bucket pushes are deferred to CI.

### Level 1: YAML well-formedness (runs on Linux)
```bash
cd /home/dustin/projects/qmkonnect
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml')); print('release.yml: valid YAML')"
# Expected: "release.yml: valid YAML". If it errors → fix indentation (GH Actions YAML is indent-sensitive;
#   both new jobs must align at the same 2-space column as `publish:`).
actionlint .github/workflows/release.yml 2>/dev/null || echo "(actionlint not installed — YAML parse is the gate)"
# Expected: clean (or "not installed"). Address any actionlint errors (e.g. needs on an unknown job,
#   a malformed `uses:` reference, an invalid `if:`).
git diff --stat    # Expected: ONLY .github/workflows/release.yml (1 file). Nothing else.
```

### Level 2: Local script smoke (runs on Linux — proves the script path; no deploy key)
```bash
cd /home/dustin/projects/qmkonnect
# Confirm both scripts are intact + the wiring is sane (no network/deploy key needed):
test -x packaging/homebrew/update-cask.sh && echo "update-cask.sh present + executable"
grep -q 'git@github.com:dabstractor/homebrew-qmkonnect.git' packaging/homebrew/update-cask.sh && echo "tap remote referenced in script header"
grep -q 'Casks/qmkonnect.rb' packaging/homebrew/update-cask.sh && echo "cask path referenced"
test -f packaging/scoop/update-manifest.ps1 && echo "update-manifest.ps1 present"
grep -q 'git@github.com:dabstractor/scoop-qmkonnect.git' packaging/scoop/update-manifest.ps1 && echo "bucket remote referenced in script header"
grep -q 'bucket/qmkonnect.json' packaging/scoop/update-manifest.ps1 && echo "bucket subdir path referenced"
# (If you have a published release's DMG/exe SHA handy, you can locally dry-run a script:
#   ./packaging/homebrew/update-cask.sh <ver> <sha>   # patches the source-checkout cask; does NOT push
#   ./packaging/scoop/update-manifest.ps1 -Version <ver> -Sha256 <sha>   # patches the source-checkout manifest
#  then `git checkout packaging/` to discard. The CI job runs them WITHOUT a local repo to clean up.)
```

### Level 3: grep invariants — both jobs are structurally correct (runs on Linux)
```bash
cd /home/dustin/projects/qmkonnect
F=.github/workflows/release.yml
grep -nE '^\s+(homebrew-tap|scoop-bucket):' "$F"          # Expected: 2 (the two job keys)
grep -cE 'needs: \[publish\]' "$F"                         # Expected: >=3 (publish-deps line + publish itself counted once; aur + homebrew + scoop) — at least the 2 new ones
# (more precise: the two NEW needs:[publish] lines sit under the new job keys)
awk '/^  homebrew-tap:|^  scoop-bucket:/{j=$1; n=0} /needs: \[publish\]/{if(j)print FILENAME":"NR": "$0}' "$F"  # Expected: 2 lines (one per job)
grep -nE 'webfactory/ssh-agent@v0\.(9|10)\.0' "$F"         # Expected: 2 (one per job). MUST be ssh-agent, NOT agents/github-ssh-agent.
! grep -qE 'webfactory/agents/github-ssh-agent' "$F" && echo "OK: no wrong action path"   # Expected: OK
grep -nE 'HOMEBREW_TAP_DEPLOY_KEY|SCOOP_BUCKET_DEPLOY_KEY' "$F"   # Expected: >=4 (2× env/uses + 2× comment doc blocks)
grep -nE 'QMKonnect-\$\{VERSION\}-macos\.dmg' "$F"         # Expected: 1 (homebrew-tap download URL)
grep -nE 'QMKonnect-\$\{VERSION\}-windows-x64\.exe' "$F"   # Expected: 1 (scoop-bucket download URL)
grep -nE 'git@github.com:dabstractor/homebrew-qmkonnect\.git' "$F"  # Expected: 1 (clone)
grep -nE 'git@github.com:dabstractor/scoop-qmkonnect\.git' "$F"    # Expected: 1 (clone)
grep -nE 'tap-repo/Casks/qmkonnect\.rb' "$F"               # Expected: 1 (cp destination)
grep -nE 'bucket-repo/bucket/qmkonnect\.json' "$F"         # Expected: 1 (cp destination — NOTE the bucket/ subdir)
grep -nE '\./update-cask\.sh ' "$F"                         # Expected: 1 (homebrew-tap invocation, 2-arg form)
grep -nE 'update-manifest\.ps1 -Version' "$F"              # Expected: 1 (scoop-bucket invocation, -Sha256 form)
grep -nE 'git config user\.email' "$F"                     # Expected: >=2 (one per job — scripts don't push)
grep -nE '\$\{GITHUB_REF_NAME#v\}' "$F"                    # Expected: 2 (version extraction, both jobs)
grep -niE 'permissions:' "$F" | tail -5                    # the two new jobs should have NO permissions: line
# Confirm NO existing job was disturbed:
grep -cE '^\s+(macos|windows|linux-binary|arch|publish):' "$F"   # Expected: 5 (unchanged originals)
```

### Level 4: Real tap/bucket publish on a tag push (DEFERRED — CI, needs both secrets)
```bash
# NOT run from the dev box (no deploy keys). On a real release:
#   1. (once per channel) generate the ed25519 key; add PUBLIC half to the tap/bucket repo's Deploy keys
#      (CHECK "Allow write access"); store PRIVATE half as HOMEBREW_TAP_DEPLOY_KEY / SCOOP_BUCKET_DEPLOY_KEY.
#   2. git tag v0.2.9 && git push origin v0.2.9  -> triggers the full pipeline.
#   3. After macos/windows/linux-binary/arch + publish go green, homebrew-tap + scoop-bucket run.
# Verify post-publish:
#   Homebrew: curl -s https://raw.githubusercontent.com/dabstractor/homebrew-qmkonnect/main/Casks/qmkonnect.rb \
#               | grep -E 'version|sha256'      # Expected: version "0.2.9" + a real 64-hex sha256 (not :no_check)
#             brew info --cask qmkonnect (after `brew tap mulletware/qmkonnect https://github.com/dabstractor/homebrew-qmkonnect`)
#   Scoop:    curl -s https://raw.githubusercontent.com/dabstractor/scoop-qmkonnect/main/bucket/qmkonnect.json \
#               | jq '{version, hash: .architecture.64bit.hash}'   # Expected: 0.2.9 + a real 64-hex hash (not 64 zeros)
# If a clone fails with "Permission denied (publickey)" -> the secret is missing/wrong OR the public half
#   isn't on the repo's Deploy keys (or lacks write access). If the download 404s -> the release asset isn't
#   live yet (check the publish job succeeded + the DMG/exe is attached to the release).
```

## Final Validation Checklist

### Technical Validation
- [ ] Level 1: `release.yml` parses as valid YAML; `actionlint` clean (or "not installed"); `git diff --stat`
      = ONLY `.github/workflows/release.yml`.
- [ ] Level 3 grep gates pass (two job keys; both needs:[publish]+if push; `webfactory/ssh-agent@v0.9.0`
      used and `webfactory/agents/...` ABSENT; both secrets referenced + documented; both asset URLs,
      both clone URLs, both cp destinations with the correct `bucket/` subdir; both update scripts invoked;
      git identity set in both; ref_name version extraction in both).

### Feature Validation
- [ ] Both jobs have `needs: [publish]` (release assets live before the download) and
      `if: github.event_name == 'push'` (tag-only; skipped on workflow_dispatch dry-runs).
- [ ] Each job downloads the correct asset, computes a lowercase-64-hex SHA256, and passes it to the
      update script in the documented 2-arg / `-Sha256` form (script skips its own download → one download).
- [ ] `homebrew-tap` copies the patched cask into the tap clone at `Casks/qmkonnect.rb`; `scoop-bucket`
      copies the patched manifest into the bucket clone at `bucket/qmkonnect.json` (the `bucket/` subdir).
- [ ] Each job sets a git identity in the clone before committing (the scripts do NOT push; the job owns it).
- [ ] Each job uses `webfactory/ssh-agent@v0.9.0` with the correct deploy-key secret.
- [ ] The inline comment blocks document both deploy-key secrets (generate ed25519, register public half on
      the tap/bucket repo Deploy keys WITH write access, store private half as the Actions secret).

### Code Quality Validation
- [ ] Mirrors the existing `publish`/`aur` tag-only gate (`needs: [publish]; if: github.event_name == 'push'`).
- [ ] Mirrors the existing inline-secret-documentation idiom (the macos job's APPLE_* comments / aur's AUR_*).
- [ ] Neither new job adds a `permissions:` escalation (deploy-key push ≠ GITHUB_TOKEN push).
- [ ] Version derived from the git tag (`ref_name#v`) — no unnecessary Rust toolchain install.

### Documentation & Deployment
- [ ] Both Mode-A secret docs ride WITH the jobs (inline comment blocks), per the contract.
- [ ] No Rust/Cargo/packaging/docs changes; no PRD/tasks.json/prd_snapshot edits.

---

## Anti-Patterns to Avoid

- ❌ Don't use `webfactory/agents/github-ssh-agent` — that path DOES NOT EXIST (the bucket-README is wrong).
      Use `webfactory/ssh-agent@v0.9.0`. Verified via the GitHub API.
- ❌ Don't reimplement the cask/manifest patching in the workflow — `update-cask.sh` / `update-manifest.ps1`
      (Complete) already do it and are tested. The job runs the script, then owns the clone→cp→commit→push.
- ❌ Don't run the update script against the external clone — it patches `$SCRIPT_DIR`/`$PSScriptRoot`
      (the source checkout). Run it against the source, then `cp` the patched file into the clone.
- ❌ Don't forget `git config user.email/user.name` in the clone — the scripts do NOT push; the job owns the
      commit, and git refuses to commit without an identity.
- ❌ Don't depend on `[macos]`/`[windows]` — they upload workflow ARTIFACTS, not RELEASE assets. The download
      hits the release URL, which is live only after `publish`. Use `needs: [publish]`.
- ❌ Don't drop the `if: github.event_name == 'push'` gate — on workflow_dispatch the release isn't live and
      the asset download 404s.
- ❌ Don't add `permissions: contents: write` to either job — they push over a deploy key, not the GITHUB_TOKEN.
- ❌ Don't compute the Scoop hash with `Get-FileHash` without `.ToLower()` (uppercase output fails the
      script's `^[0-9a-f]{64}$` check). Prefer `sha256sum | awk '{print $1}'` in bash (already lowercase).
- ❌ Don't forget the `bucket/` subdir for the Scoop copy (`bucket-repo/bucket/qmkonnect.json`, not the root).
      Name the clone dir `bucket-repo` to avoid the confusing `bucket/bucket` path.
- ❌ Don't let the update script re-download the asset — pass the precomputed hash (2-arg / -Sha256 form) so
      the job's single download is the only one.
- ❌ Don't install the Rust toolchain just to read the version — these jobs build no Rust; use
      `${GITHUB_REF_NAME#v}` (the tag, which cargo-release cut from Cargo.toml).
- ❌ Don't add the Winget/Nix/asdf CI jobs (separate work items P1.M5.T2.S1 / P1.M5.T2.S2).
- ❌ Don't modify update-cask.sh / update-manifest.ps1 / the cask / the manifest (INPUT, Complete).
- ❌ Don't edit PRD.md, any tasks.json, or prd_snapshot.md.

---

## Confidence Score: 9/10

The task is small (two jobs in one file) and every fact is verified this session: both update-script
contracts (2-arg / -Sha256 form; patch in source checkout; do NOT push; lowercase SHA256); the external
repo layouts (clone URLs + in-repo file paths, incl. the Scoop `bucket/` subdir) from tap-README/bucket-README;
the deploy-key model (per-repo GitHub deploy keys with write access) from external_deps.md + both READMEs;
the CRITICAL ssh-action correction (`webfactory/ssh-agent`, not the bucket-README's wrong path, verified via
the GitHub API); the version-from-tag approach (no toolchain); and the parallel aur job's append-after-publish
coordination (zero logical conflict). The local gates (YAML parse + actionlint + grep) run on any Linux box;
the real tap/bucket pushes are clean deferred CI gates. The −1 reserves for: (a) the deploy keys needing
"Allow write access" checked (a setup step, documented inline — the job is correct but the push needs the
key configured); (b) the very-first-run "Permission denied (publickey)" until the secrets are set (expected,
documented in the inline blocks).