# PRP — P1.M7.T1.S2: Add `.deb` CI job to `release.yml`

---

## Goal

**Feature Goal**: Add a `deb` job to `.github/workflows/release.yml` that builds
the native Debian/Ubuntu/Mint package with `cargo-deb` on `ubuntu-22.04` and
attaches it (`qmkonnect-<ver>-linux-amd64.deb`) to the GitHub Release produced by
the existing `publish` job — wiring the `.deb` artifact produced by P1.M7.T1.S1's
`[package.metadata.deb]` block into the automated release pipeline (F15 /
`spec/PACKAGING.md` §9).

**Deliverable**: An updated `.github/workflows/release.yml` containing a new
`deb` job (a self-contained post-publish build job) plus a Mode-A documentation
comment block. **No other file is modified.**

**Success Definition**: On the next `v*` tag push, the `deb` job runs on
`ubuntu-22.04` after `publish`, produces `target/debian/qmkonnect_<ver>_amd64.deb`
via `cargo deb` (no `-lhidapi-hidraw`), renames it to
`qmkonnect-<ver>-linux-amd64.deb`, and attaches it to the GitHub Release alongside
the existing DMG/EXE/tarball/pkg assets — without disturbing them. A
`workflow_dispatch` dry-run does NOT run the `deb` job (it is gated on
`github.event_name == 'push'`). `actionlint` + YAML-parse pass on the file.

## User Persona

**Target User**: Ubuntu/Debian/Mint end users (the F15 Linux community channel for
`.deb`-based distros).

**Use Case**: `sudo apt install ./qmkonnect-<ver>-linux-amd64.deb` (or
`sudo dpkg -i …`) to install QMKonnect with full dependency resolution and the
postinst/postrm lifecycle defined by P1.M7.T1.S1.

**Pain Points Addressed**: Today these users have only the generic tarball (manual
file placement, no dependency resolution, no install/uninstall hooks). The `.deb`
gives them a one-line install with `apt` resolving `libhidapi-hidraw0`/`libxdo3`/
`zenity`/`libnotify-bin`/`systemd` and the maintainer scripts reloading udev +
instantiating the systemd user service.

## Why

- **F15 (community package-manager distribution)** lists native `.deb`/`.rpm`
  alongside AUR/Homebrew/Scoop/Winget/Nix/mise/asdf (`spec/PACKAGING.md` §6, F15
  row). The `.deb` recipe + metadata already exist (P1.M7.T1.S1); this task wires
  that recipe into CI so every release automatically ships a `.deb`.
- **Architectural fit (contract-driven):** the work item mandates a **post-publish**
  job (`needs: [publish]`, `if: github.event_name == 'push'`) rather than a
  build-job feeding `publish`. Rationale: `cargo install cargo-deb` + `cargo deb`
  is a slow build; keeping it OFF the `publish` critical path means a `.deb` build
  failure never blocks the core release (DMG/EXE/tarball/pkg are already published
  by `publish`). The `.deb` is additive to an already-created release.
- **Scope boundary:** this task adds ONLY the CI job. The `[package.metadata.deb]`
  Cargo metadata + the `packaging/debian/{postinst,prerm,postrm,...}` scripts are
  P1.M7.T1.S1 (consumed here as a build precondition). The `.rpm` CI job is
  P1.M7.T2.S2. The `publish` job is **not** modified (the `deb` job uploads
  directly).

## What

User-/maintainer-visible behavior:
- A new `deb` job appears in `release.yml`. On a `v*` tag push it: checks out the
  repo, installs the Rust toolchain + the full Linux build deps, installs
  `cargo-deb`, runs `cargo deb` (producing
  `target/debian/qmkonnect_<ver>_amd64.deb`), renames it to
  `qmkonnect-<ver>-linux-amd64.deb`, and attaches it to the GitHub Release created
  by `publish`.
- On `workflow_dispatch` (dry-run), the `deb` job is **skipped** (gated on
  `github.event_name == 'push'`, identical to `aur`/`homebrew-tap`/`scoop-bucket`/
  `winget`/`asdf-plugin`) — because `publish` doesn't create a release in a
  dry-run, so the upload target wouldn't exist.
- A Mode-A documentation comment block above the job explains what it builds, why
  `ubuntu-22.04`, why NO `-lhidapi-hidraw`, why it needs `contents: write`, and
  the glibc-2.35 baseline.

### Success Criteria

- [ ] `deb` job present in `.github/workflows/release.yml` with exactly
      `runs-on: ubuntu-22.04`, `needs: [publish]`, `if: github.event_name == 'push'`.
- [ ] Job installs the **full** Linux build-dep set (not just the contract's
      minimal 3 packages) — see Gotcha #1.
- [ ] Job installs `cargo-deb` (`cargo install --locked cargo-deb`) and runs
      `cargo deb` with **no** `-lhidapi-hidraw` link flag anywhere.
- [ ] Output renamed `target/debian/qmkonnect_<ver>_amd64.deb` →
      `qmkonnect-<ver>-linux-amd64.deb` and uploaded to the GitHub Release.
- [ ] Job declares `permissions: contents: write` (it writes to THIS repo's
      release — unlike the external-repo post-publish jobs).
- [ ] The `publish` job is **unchanged** (no edit to its `needs:` or `files:`).
- [ ] Mode-A comment block documents the job inline.
- [ ] `actionlint` clean; YAML parses; the `deb` job has a unique artifact path.

## All Needed Context

### Context Completeness Check

> "If someone knew nothing about this codebase, would they have everything needed
> to implement this successfully?" — **YES.** The exact job YAML, the apt package
> list, the rename mapping, the upload command, the version-detection pattern, and
> every gotcha are spelled out below with verbatim quotes from the existing
> `linux-binary` job to clone.

### Documentation & References

```yaml
# MUST READ — the authoritative CI job spec (the contract source)
- file: spec/PACKAGING.md
  section: "§9. CI Release (.github/workflows/release.yml)"
  why: Pins the .deb job contract: "on ubuntu-22.04 — apt install libhidapi-dev
       libxdo-dev pkg-config, cargo install cargo-deb, cargo deb (no
       -lhidapi-hidraw), rename to qmkonnect-<ver>-linux-amd64.deb, upload."
  critical: |
    The §9 apt list is the RUNTIME-relevant subset ONLY. cargo deb runs a full
    cargo build --release that ALSO links the tray stack (gtk3/glib/appindicator/
    x11/xcb/udev) — you MUST install the full linux-binary dep set (see Gotcha #1
    + Task 3) or the build fails with missing headers. This PRP supersedes the
    minimal list with the proven full set.

# MUST READ — the cargo-deb recipe consumed as a BUILD PRECONDITION
- file: spec/PACKAGING.md
  section: "§4.3 .deb via cargo-deb (packaging/debian/) — NEW"
  why: Defines [package.metadata.deb] (authored by P1.M7.T1.S1) — the metadata
       cargo-deb reads. Confirms the output path
       (target/debian/qmkonnect_<ver>_amd64.deb), the no-hidraw-flag rule, the
       glibc-2.35 baseline rationale, and the CI rename target
       (qmkonnect-<ver>-linux-amd64.deb).
  critical: |
    Build WITHOUT -lhidapi-hidraw — Debian/Ubuntu ship a UNIFIED hidapi (>=0.14)
    that auto-selects the hidraw backend; usage/usage_page matching works without
    the flag. (Arch needs the flag; Debian/Fedora do not. See spec §2.)

# MUST READ — the build precondition (parallel task P1.M7.T1.S1)
- file: plan/007_fb356ba503b4/P1M7T1S1/PRP.md
  why: This job assumes the [package.metadata.deb] block exists in Cargo.toml and
       packaging/debian/{long-description.txt,postinst,prerm,postrm} exist (the
       maintainer-scripts dir + extended-description-file the metadata references).
       If those are absent, cargo deb fails at asset/maintainer-script resolution.
  contract: |
    cargo deb output: target/debian/qmkonnect_0.2.8_amd64.deb (underscores).
    maintainer-scripts = "packaging/debian/"; depends line resolved by dpkg at
    install time, not at build. This CI job consumes the recipe verbatim.

# MUST CLONE — the job to mirror (ubuntu-22.04 + full Linux build deps)
- file: .github/workflows/release.yml
  section: "the `linux-binary` job (lines ~142–189)"
  why: Identical runner (ubuntu-22.04), identical apt install block (the FULL
       proven build-dep set), identical version-detection step (cargo metadata |
       jq), identical prologue (checkout → rust-toolchain → rust-cache). Clone its
       skeleton, then add cargo-deb install + cargo deb + rename + release upload.
  pattern: |
    prologue:  actions/checkout@v4 → dtolnay/rust-toolchain@stable → Swatinem/rust-cache@v2
    apt:       build-essential pkg-config libgtk-3-dev libglib2.0-dev
               libayatana-appindicator3-dev libx11-dev libxcb1-dev libxdo-dev
               libhidapi-dev libudev-dev  (sudo apt-get install -y --no-install-recommends)
    version:   v=$(cargo metadata --no-deps --format-version 1 | jq -r
                 '.packages[] | select(.name=="qmkonnect") | .version')
               echo "version=$v" >> "$GITHUB_OUTPUT"

# MUST CLONE — the post-publish job skeleton (needs/if/runs-on)
- file: .github/workflows/release.yml
  section: "any of: aur / homebrew-tap / scoop-bucket / asdf-plugin"
  why: All four share the identical post-publish header
       (needs: [publish] / if: github.event_name == 'push' / runs-on: ubuntu-latest).
       The deb job uses the same needs/if, but runs-on: ubuntu-22.04 AND adds
       permissions: contents: write (it writes to THIS release — see Gotcha #2).

# REFERENCE — the release-writer this job appends to
- file: .github/workflows/release.yml
  section: "the `publish` job (lines ~237–256)"
  why: publish creates the GitHub Release via softprops/action-gh-release@v2.
       The deb job runs AFTER it (needs: [publish]) and APPENDS the .deb to that
       existing release. DO NOT modify publish — it is the sole release creator.
  critical: |
    The deb job is the ONLY post-publish job that writes to THIS repo's release;
    the others (aur/homebrew/scoop/winget/asdf) push to EXTERNAL repos. Hence the
    deb job needs permissions: contents: write.

# EXTERNAL — cargo-deb (build tool) semantics
- url: https://github.com/kornelski/cargo-deb#configuration
  why: Confirms cargo deb (a) auto-runs cargo build --release, (b) emits
       target/debian/<name>_<version>_<arch>.deb by default.
  critical: |
    No separate `cargo build` step is required (cargo deb builds); but install the
    FULL build-dep set because cargo deb compiles the same Linux binary.

# EXTERNAL — appending an asset to an existing GitHub Release
- url: https://cli.github.com/manual/gh_release_upload
  why: `gh release upload <tag> <file> --clobber` is the purpose-built, explicitly
       additive way to attach a single asset to an already-created release without
       disturbing existing assets. gh is preinstalled on ubuntu runners.
  critical: |
    Requires write access — satisfied by permissions: contents: write + the default
    secrets.GITHUB_TOKEN (pass as env GH_TOKEN). The tag is ${GITHUB_REF_NAME}
    (e.g. v0.2.8) on a v* tag push.
```

### Current Codebase Tree (relevant slice)

```bash
.github/workflows/release.yml     # EDIT — add the `deb` job (this task)
Cargo.toml                        # [package.metadata.deb] added by P1.M7.T1.S1 (precondition)
packaging/debian/                 # postinst/prerm/postrm/long-description.txt added by P1.M7.T1.S1 (precondition)
spec/PACKAGING.md                 # READ-ONLY source of truth (§4.3, §9)
# (P1.M7.T1.S1 also ensures packaging/linux/xdg/qmkonnet.desktop exists — an asset
#  referenced by the metadata block; NOT this task's concern.)
```

### Desired Codebase Tree (files this task touches)

```bash
.github/workflows/release.yml     # EDIT ONLY: append the `deb` job + its comment block
# (no other file is created or modified by this task)
```

### Known Gotchas of our codebase & library quirks

```yaml
# GOTCHA #1 (CRITICAL — build will FAIL if ignored): the contract / spec §9 / §4.3
# list ONLY "libhidapi-dev libxdo-dev pkg-config" as the apt build-deps. That is the
# RUNTIME-relevant subset. cargo deb runs a full cargo build --release that ALSO
# links the tray stack (tao/tray-icon → gtk-3/glib/ayatana-appindicator) + x11/xcb
# + udev. The proven-working FULL set (from the existing linux-binary job) is:
#   build-essential pkg-config libgtk-3-dev libglib2.0-dev libayatana-appindicator3-dev
#   libx11-dev libxcb1-dev libxdo-dev libhidapi-dev libudev-dev
# Install that UNION (Task 3). Following the contract's 3-package list literally
# produces "gtk/glib/x11.h: No such file or directory" and a red CI job.

# GOTCHA #2 (CRITICAL — upload will 403 if ignored): this is the ONLY post-publish
# job that writes to THIS repo's GitHub Release (aur/homebrew/scoop/winget/asdf push
# to EXTERNAL repos, so they inherit the workflow-default contents: read). The deb
# job MUST declare its own `permissions: contents: write` or `gh release upload` /
# softprops/action-gh-release returns 403. (Only `publish` currently overrides
# permissions in this file; this job becomes the second.)

# GOTCHA #3 (build correctness): do NOT pass -lhidapi-hidraw anywhere in the deb
# job (no RUSTFLAGS). Debian/Ubuntu ship a UNIFIED hidapi (>=0.14) in libhidapi.so
# that auto-selects the hidraw backend at runtime; usage/usage_page matching works
# without the flag. (The Arch PKGBUILD sets the flag ONLY because Arch ships the
# hidraw/libusb backends as SEPARATE libs — spec §2. Getting this backwards breaks
# device matching at runtime.)

# GOTCHA #4 (rename: underscores → dashes): cargo-deb emits
# target/debian/qmkonnect_<ver>_amd64.deb (UNDERSCORES around the version). The
# release-asset name uses DASHES: qmkonnect-<ver>-linux-amd64.deb. The mv source
# must use qmkonnect_${VERSION}_amd64.deb; the target qmkonnect-${VERSION}-linux-amd64.deb.
# (The arch/aur naming uses qmkonnect-<ver>-x86_64.pkg.tar.zst; the .deb uses
#  -linux-amd64 per spec §9 to match Debian's amd64 arch + the linux-amd64 tag.)

# GOTCHA #5 (why needs: [publish], not a build job): the CONTRACT mandates
# needs: [publish] + if: github.event_name == 'push'. This keeps the slow
# cargo-deb build OFF the publish critical path (a .deb build failure never blocks
# the core release). Do NOT instead make deb a build job that feeds publish's
# needs/files — that would violate the contract. publish stays UNCHANGED.

# GOTCHA #6 (gating = dry-run skip): if: github.event_name == 'push' means the deb
# job does NOT run on workflow_dispatch dry-runs (publish creates no release then,
# so gh release upload would 404). This matches every other post-publish job.

# GOTCHA #7 (the upload target): gh release upload's first arg is the TAG NAME.
# On a v* tag push, GITHUB_REF_NAME == "v0.2.8" (the tag, WITH the v). gh accepts
# the tag and resolves the release on it. Pass "${GITHUB_REF_NAME}" verbatim.

# GOTCHA #8 (build precondition — P1.M7.T1.S1): cargo deb requires the
# [package.metadata.deb] block in Cargo.toml AND the maintainer-scripts dir
# (packaging/debian/{postinst,prerm,postrm}) AND extended-description-file
# (packaging/debian/long-description.txt) AND every assets[] source to exist
# (incl. packaging/linux/xdg/qmkonnect.desktop). These are P1.M7.T1.S1's
# deliverables. If they are absent at CI time, cargo deb fails before it builds —
# that is a P1.M7.T1.S1 defect, NOT this job's. This task assumes they exist.

# GOTCHA #9 (version detection variant): use the `cargo metadata | jq` variant
# (NOT the GITHUB_REF_NAME#v variant). This job BUILDS rust, so it reads the
# version from the compiled Cargo.toml (single source of truth), matching
# linux-binary/arch/aur. The #v variant is for build-less jobs only.
```

## Implementation Blueprint

### Data models and structure

_None._ This is a CI workflow edit (YAML). No Rust types, schemas, or runtime
models change. No Cargo.toml edit. No new files other than the workflow edit.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: VERIFY the build precondition (P1.M7.T1.S1) exists
  - RUN: grep -n '\[package.metadata.deb\]' Cargo.toml
  - RUN: ls packaging/debian/{postinst,prerm,postrm,long-description.txt} 2>/dev/null
  - EXPECT: the metadata block + the 4 maintainer-script/desc files present
    (authored by P1.M7.T1.S1). If absent, STOP — this CI job cannot succeed
    without them; flag a P1.M7.T1.S1 dependency gap (do NOT author them here).
  - NOTE: this is a read-only precondition check; it confirms the job will have
    something valid to build. No action if present.

Task 2: LOCATE the insertion point in release.yml
  - FIND the END of the `publish` job (the step block that uses
    softprops/action-gh-release@v2 and its trailing `files: |` glob list).
  - INSERT the new `deb` job IMMEDIATELY AFTER the `publish` job's closing lines
    and BEFORE the `aur` job. Rationale: it is a post-publish job (needs: [publish])
    that produces a release asset, so grouping it first among the post-publish
    jobs reads naturally; the `aur` job (external repo) follows it.
  - DO NOT modify the `publish` job, its needs:, or its files: list.

Task 3: ADD the `deb` job — use EXACTLY this YAML (comment block + job)
  - COMMENT BLOCK (Mode-A doc — explains what/why; keep it):
      # ───────────────────────────────────────────────────────────────────────
      # .deb — native Debian/Ubuntu/Mint package via cargo-deb
      # (spec/PACKAGING.md §4.3 recipe; the [package.metadata.deb] block in
      # Cargo.toml is authored by P1.M7.T1.S1). Built on ubuntu-22.04 for the
      # glibc 2.35 baseline (works on 22.04/24.04, Debian 12, Mint 21/22+).
      #
      # A POST-PUBLISH job: runs after `publish` creates the GitHub Release and
      # APPENDS qmkonnect-<ver>-linux-amd64.deb to it (it is the only post-publish
      # job that writes to THIS repo's release — the aur/homebrew/scoop/winget/asdf
      # jobs push to EXTERNAL repos — so it needs contents: write). Keeping it off
      # the publish critical path means a .deb build failure never blocks the core
      # release. Skipped on workflow_dispatch dry-runs (publish creates no release).
      #
      # Build uses NO -lhidapi-hidraw: Debian/Ubuntu ship a UNIFIED hidapi (>=0.14)
      # that auto-selects the hidraw backend at runtime (spec §2). (The Arch
      # PKGBUILD sets the flag only because Arch splits the hidraw/libusb libs.)
      # ───────────────────────────────────────────────────────────────────────
  - JOB YAML:
      deb:
        name: Linux (.deb via cargo-deb)
        needs: [publish]
        if: github.event_name == 'push'
        runs-on: ubuntu-22.04
        permissions:
          contents: write
        steps:
          - uses: actions/checkout@v4
          - uses: dtolnay/rust-toolchain@stable
          - uses: Swatinem/rust-cache@v2

          - name: Install Linux build dependencies
            run: |
              sudo apt-get update
              sudo apt-get install -y --no-install-recommends \
                build-essential pkg-config \
                libgtk-3-dev libglib2.0-dev libayatana-appindicator3-dev \
                libx11-dev libxcb1-dev libxdo-dev libhidapi-dev libudev-dev

          - name: Determine version
            id: ver
            run: |
              v=$(cargo metadata --no-deps --format-version 1 \
                  | jq -r '.packages[] | select(.name=="qmkonnect") | .version')
              echo "version=$v" >> "$GITHUB_OUTPUT"

          - name: Install cargo-deb
            run: cargo install --locked cargo-deb

          # cargo deb runs `cargo build --release` itself; NO -lhidapi-hidraw.
          - name: Build .deb
            run: cargo deb

          # cargo-deb emits target/debian/qmkonnect_<ver>_amd64.deb (underscores).
          # Release-asset name uses dashes: qmkonnect-<ver>-linux-amd64.deb.
          - name: Rename to release asset name
            env:
              VERSION: ${{ steps.ver.outputs.version }}
            run: |
              set -eux
              mv "target/debian/qmkonnect_${VERSION}_amd64.deb" \
                 "qmkonnect-${VERSION}-linux-amd64.deb"

          # Append the .deb to the release created by `publish` (additive;
          # --clobber makes it idempotent on re-runs). gh is preinstalled on
          # ubuntu runners; GH_TOKEN = the default GITHUB_TOKEN (write via
          # permissions: contents: write above).
          - name: Upload .deb to the GitHub Release
            env:
              GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
              VERSION: ${{ steps.ver.outputs.version }}
            run: |
              gh release upload "${GITHUB_REF_NAME}" \
                "qmkonnect-${VERSION}-linux-amd64.deb" --clobber
  - INDENTATION: 2-space YAML; the job key `deb:` is at the same indentation as
    `publish:`, `aur:`, etc. (2 spaces under `jobs:`). Steps are 4-space indented
    under `steps:`. Match the file's existing style exactly.
  - DEPENDENCIES: Task 2 (insertion point). The job is self-contained (no other
    workflow changes).

Task 4: VALIDATE (YAML + actionlint + structural)
  - SEE Validation Loop below. Run the YAML-parse, actionlint, and grep checks.
  - FIX any reported errors before considering the task done.

Task 5 (OPTIONAL — debuggability, NOT required by the contract): add an
  actions/upload-artifact@v4 step so a dry-run (if the gate were temporarily
  relaxed) or a failed upload leaves the .deb downloadable from the run. Only do
  this if the implementer wants belt-and-suspenders; the contract's deliverable is
  the release upload, so this is genuinely optional. If added:
      - uses: actions/upload-artifact@v4
        with:
          name: linux-deb
          path: qmkonnect-*-linux-amd64.deb
          if-no-files-found: error
  (Place it BEFORE the `gh release upload` step so a failed upload still captures
  the artifact. Do NOT add this artifact to publish's files: — the deb job is NOT
  a build job feeding publish.)
```

### Implementation Patterns & Key Details

```yaml
# The post-publish + release-writer skeleton (the ONLY deviation from the existing
# post-publish jobs is `runs-on: ubuntu-22.04` + the `permissions: contents: write`):
  deb:
    needs: [publish]                     # runs after the release is created
    if: github.event_name == 'push'      # skipped on workflow_dispatch dry-runs
    runs-on: ubuntu-22.04                # glibc 2.35 baseline (matches linux-binary)
    permissions:
      contents: write                    # UNIQUE among post-publish jobs (writes THIS release)

# Version step — the Rust-build variant (NOT the #v variant):
      - name: Determine version
        id: ver
        run: |
          v=$(cargo metadata --no-deps --format-version 1 \
              | jq -r '.packages[] | select(.name=="qmkonnect") | .version')
          echo "version=$v" >> "$GITHUB_OUTPUT"

# Rename — underscore source, dash target (the #1 rename pitfall):
      - name: Rename to release asset name
        env: { VERSION: ${{ steps.ver.outputs.version }} }
        run: |
          set -eux
          mv "target/debian/qmkonnect_${VERSION}_amd64.deb" \
             "qmkonnect-${VERSION}-linux-amd64.deb"

# Upload — additive append to the existing release; --clobber = idempotent:
      - name: Upload .deb to the GitHub Release
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          VERSION: ${{ steps.ver.outputs.version }}
        run: |
          gh release upload "${GITHUB_REF_NAME}" \
            "qmkonnect-${VERSION}-linux-amd64.deb" --clobber
```

### Integration Points

```yaml
WORKFLOW (.github/workflows/release.yml):
  - ADD: the `deb` job (Task 3) immediately after the `publish` job.
  - PRESERVE: every existing job (macos, windows, linux-binary, arch, publish,
    aur, homebrew-tap, scoop-bucket, winget, asdf-plugin) UNCHANGED.
  - DO NOT: add `deb` to publish.needs, add an artifacts/deb glob to publish.files,
    or touch any other job. (The deb job is a post-publish release-writer, not a
    build job feeding publish.)
  - DO NOT: add a `[package.metadata.deb]` edit, packaging/debian/ files, or
    packaging/linux/xdg/* — those are P1.M7.T1.S1 / P2.M6.T1.S1.

NO CHANGES TO:
  - Cargo.toml, any .rs, spec/PACKAGING.md (read-only), release.toml, .cargo/config.toml
  - the arch/aur/nix/homebrew/scoop/winget/asdf packaging
```

## Validation Loop

### Level 1: YAML & Workflow Lint (Immediate Feedback)

```bash
# 1a. YAML is well-formed (no tab/space/indent breakage from the insertion):
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml')); print('PASS yaml parse')"
# Expected: PASS yaml parse. If this raises, fix indentation before anything else.

# 1b. GitHub Actions workflow lint (catches malformed needs/if/permissions, bad
# action versions, expression errors). Install if missing:
#   (brew install actionlint)  OR  go install github.com/rhysd/actionlint/cmd/actionlint@latest
actionlint .github/workflows/release.yml && echo "PASS actionlint"
# Expected: PASS actionlint. actionlint is strict about the `permissions:` block
# and expression syntax — a clean run is strong evidence the job is well-formed.

# 1c. The workflow-level default is unchanged (permissions: contents: read) and
# ONLY publish + deb override it:
grep -nE '^[[:space:]]*permissions:' .github/workflows/release.yml
# Expected: exactly the top-level `permissions:` + two job-level `permissions:`
# blocks (publish + deb). If deb's is missing, upload will 403.
```

### Level 2: Structural Assertions (the job is correct by inspection)

```bash
# 2a. The deb job exists with the mandated header:
awk '/^  deb:/{f=1} f&&/^  [a-z]/{if(NR>1&&prev!="  deb:")exit; } {prev=$0}' \
  .github/workflows/release.yml >/dev/null; \
grep -nE '^  deb:' .github/workflows/release.yml && echo "PASS deb job present"

# 2b. Header fields are exactly as contracted:
python3 - <<'PY'
import yaml
w = yaml.safe_load(open('.github/workflows/release.yml'))
d = w['jobs']['deb']
assert d['runs-on'] == 'ubuntu-22.04', d['runs-on']
assert d['needs'] == ['publish'], d['needs']
assert d['if'] == 'github.event_name == \'push\'', d['if']
assert d['permissions'] == {'contents': 'write'}, d['permissions']
names = [s.get('name','<unnamed>') for s in d['steps']]
assert any('Install Linux build' in n for n in names), names
assert any('Install cargo-deb' in n for n in names), names
assert any('Build .deb' in n for n in names), names
assert any('Rename' in n for n in names), names
assert any('Upload .deb' in n for n in names), names
print("PASS deb job structure")
PY
# Expected: PASS deb job structure.

# 2c. NO -lhidapi-hidraw anywhere in the file's deb-relevant region:
grep -n 'hidapi-hidraw' .github/workflows/release.yml && echo "FAIL hidraw flag present" || echo "PASS no hidraw flag"
# Expected: PASS no hidraw flag.

# 2d. The publish job is UNCHANGED (still the sole softprops/action-gh-release user):
test "$(grep -c 'softprops/action-gh-release' .github/workflows/release.yml)" -eq 1 && echo "PASS publish sole release-writer" || echo "FAIL multiple release-writers"

# 2e. Full apt dep set is present (Gotcha #1):
grep -A4 'Install Linux build dependencies' .github/workflows/release.yml | grep -q 'libgtk-3-dev' \
  && grep -q 'libayatana-appindicator3-dev' .github/workflows/release.yml \
  && grep -q 'libx11-dev' .github/workflows/release.yml \
  && echo "PASS full build-dep set" || echo "FAIL minimal/missing deps"
```

### Level 3: Local Build Sanity (optional, proves `cargo deb` works on this host)

> This validates that the *recipe* (P1.M7.T1.S1's metadata) produces a .deb —
> NOT the CI job itself (which only runs on a tag push). Run only if the host is
> Debian/Ubuntu; on Arch, see the P1.M7.T1.S1 PRP's local-validation caveat.

```bash
# Only meaningful if [package.metadata.deb] exists (P1.M7.T1.S1 landed):
grep -q '\[package.metadata.deb\]' Cargo.toml || { echo "SKIP — P1.M7.T1.S1 not landed"; exit 0; }

cargo install --locked cargo-deb
cargo deb                                        # NO -lhidapi-hidraw
test -f target/debian/qmkonnect_*_amd64.deb && echo "PASS deb produced" || echo "FAIL no deb"

# Confirm the rename mapping the CI step assumes (underscore source path):
ls target/debian/qmkonnect_*_amd64.deb
# Expected: target/debian/qmkonnect_0.2.8_amd64.deb (matches the mv source glob).
```

### Level 4: End-to-End (only on a real release — cannot run locally)

> The authoritative validation is the next `v*` tag push. To dry-run the WHOLE
> pipeline WITHOUT publishing (and WITHOUT running the deb job, since it is
> push-gated), use `workflow_dispatch` from the Actions UI — that exercises every
> build job + publish's artifact-download but skips publish's release creation and
# every post-publish job (including deb). To actually validate deb, cut a release
# tag and confirm `qmkonnect-<ver>-linux-amd64.deb` appears on the Release page.

```bash
# After a real tag push, confirm the asset is attached (replace VERSION):
#   curl -fsSL https://github.com/dabstractor/qmkonnect/releases/download/v0.2.8/qmkonnect-0.2.8-linux-amd64.deb -o /tmp/q.deb
#   test -s /tmp/q.deb && echo "PASS release asset downloadable" || echo "FAIL"
# And confirm the other assets are UNTOUCHED (dmg/exe/tarball/pkg still present).
```

## Final Validation Checklist

### Technical Validation
- [ ] Level 1: `python3 … yaml.safe_load` passes; `actionlint` clean; exactly 2
      job-level `permissions:` blocks (publish + deb).
- [ ] Level 2: `deb` job present with `runs-on: ubuntu-22.04`,
      `needs: [publish]`, `if: github.event_name == 'push'`,
      `permissions: contents: write`; all 5 named steps present; NO hidraw flag;
      publish is still the sole `softprops/action-gh-release` user; full apt
      build-dep set installed.
- [ ] Level 3 (optional): `cargo deb` produces `target/debian/qmkonnect_*_amd64.deb`.
- [ ] Level 4: on the next tag push, `qmkonnect-<ver>-linux-amd64.deb` is attached
      to the Release and the pre-existing assets are untouched.

### Feature Validation
- [ ] `deb` job added after `publish`, before `aur`.
- [ ] Job installs the FULL Linux build-dep set (Gotcha #1) — not the contract's
      minimal 3-package list.
- [ ] Job runs `cargo deb` with NO `-lhidapi-hidraw`.
- [ ] Output renamed `qmkonnect_<ver>_amd64.deb` → `qmkonnect-<ver>-linux-amd64.deb`.
- [ ] `gh release upload "${GITHUB_REF_NAME}" … --clobber` appends to the release.
- [ ] Mode-A comment block documents the job (what/why/ubuntu-22.04/no-hidraw/
      contents:write/glibc baseline).

### Code Quality Validation
- [ ] Indentation matches the file's 2-space YAML style; `deb:` aligned with
      sibling job keys.
- [ ] No other job modified; `publish.needs`/`publish.files` untouched.
- [ ] Version detection uses the `cargo metadata | jq` variant (Rust-build form).
- [ ] Rename uses `${VERSION}` env var (not a bare `${{ }}` in shell — matches the
      `linux-binary` staging-step idiom; avoids expression-injection noise).

### Documentation & Deployment
- [ ] Mode-A comment block is self-contained (a reader understands the job without
      leaving the file).
- [ ] `spec/PACKAGING.md` NOT edited (read-only source of truth).
- [ ] Commit message notes: (a) the build precondition is P1.M7.T1.S1's
      `[package.metadata.deb]`; (b) the full apt set supersedes spec §9's minimal
      list because `cargo deb` runs a full release build; (c) this is a
      post-publish release-writer (the contract's `needs: [publish]`).

---

## Anti-Patterns to Avoid

- ❌ Don't install only `libhidapi-dev libxdo-dev pkg-config` — `cargo deb` runs a
  full `cargo build --release` that needs the tray/x11/gtk/udev dev headers.
  Install the full `linux-binary` set (Gotcha #1).
- ❌ Don't add `-lhidapi-hidraw` (RUSTFLAGS or otherwise) — Debian's unified hidapi
  auto-selects hidraw; the flag is Arch-only and would over-constrain the link
  (spec §2).
- ❌ Don't make `deb` a build job feeding `publish` (no edit to `publish.needs` or
  `publish.files`). The contract mandates `needs: [publish]` + direct release
  upload — it is a post-publish release-writer.
- ❌ Don't omit `permissions: contents: write` — this is the only post-publish job
  that writes to THIS repo's release; without it `gh release upload` returns 403.
- ❌ Don't use the `${GITHUB_REF_NAME#v}` version variant — that's for build-less
  jobs. This job builds Rust; use `cargo metadata | jq`.
- ❌ Don't confuse the rename source/target delimiters: cargo-deb emits
  `qmkonnect_<ver>_amd64.deb` (underscores); the release asset is
  `qmkonnect-<ver>-linux-amd64.deb` (dashes).
- ❌ Don't author the `[package.metadata.deb]` block or `packaging/debian/*` here —
  those are P1.M7.T1.S1. This task edits ONLY `release.yml`.
- ❌ Don't edit `spec/PACKAGING.md` — read-only source of truth.
- ❌ Don't skip `actionlint`/YAML-parse — a malformed job is the most likely
  one-pass failure for a pure-workflow task.

---

## Confidence Score: 9/10

**Why 9, not 10:** The job YAML, apt set, rename mapping, upload command, and
every gotcha are fully specified and cloned from the proven `linux-binary` +
post-publish patterns. The -1 is for the end-to-end validation, which can only run
on a real tag push (no local equivalent for the `gh release upload` path), and for
the implicit dependency on P1.M7.T1.S1's metadata block being correct at CI time
(a P1.M7.T1.S1 concern, documented as Gotcha #8). One-pass implementation success
is very high: the task is a single-file YAML append with a complete, copy-able
job body.